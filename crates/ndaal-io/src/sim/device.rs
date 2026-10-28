//! A simulated device: the I/O scheduler in front of a serial disk with a
//! service time drawn from the seed (J02-B1, T02.05).
//!
//! Operations the scheduler starts enter the device's hardware queue and
//! are served one after the other; each takes the model's latency plus its
//! size divided by the throughput, varied by +/-10 %. Time moves only
//! through the [`SimClock`]: an idle device registers no timer, so a
//! simulation without work ends.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::future::{Future, poll_fn};
use std::rc::Rc;
use std::task::{Poll, Waker};
use std::time::Duration;

use super::SimClock;
use crate::sched::{IoClass, Op, OpId, Scheduler, SchedulerConfig};
use crate::{Clock, Rng, Xoshiro256PlusPlus};

/// Spread of the service time around the model: +/-10 %.
const JITTER: f64 = 0.10;

/// Speed of a simulated device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceModel {
    /// Fixed time of every operation.
    pub latency: Duration,
    /// Throughput, in bytes per second.
    pub bytes_per_second: u64,
}

impl DeviceModel {
    /// Service time of an operation of `cost` bytes, without jitter.
    pub fn service(&self, cost: u64) -> Duration {
        let transfer = cost as f64 / self.bytes_per_second.max(1) as f64;
        self.latency + Duration::try_from_secs_f64(transfer).unwrap_or(Duration::MAX)
    }

    /// The longest service time of an operation of `cost` bytes, jitter
    /// included.
    pub fn max_service(&self, cost: u64) -> Duration {
        self.service(cost).mul_f64(1.0 + JITTER)
    }
}

/// Order in which waiting operations start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Policy {
    /// The I/O scheduler.
    Priority,
    /// Arrival order, with the same depth: the control that shows the
    /// scheduler is what keeps high-priority reads fast.
    Fifo,
}

enum Queue {
    Priority(Box<Scheduler>),
    Fifo {
        waiting: VecDeque<Op>,
        inflight: usize,
        depth: usize,
    },
}

impl Queue {
    fn submit(&mut self, op: Op) {
        match self {
            Queue::Priority(sched) => sched.submit(op),
            Queue::Fifo { waiting, .. } => waiting.push_back(op),
        }
    }

    fn next(&mut self, now: Duration) -> Option<Op> {
        match self {
            Queue::Priority(sched) => sched.next(now),
            Queue::Fifo {
                waiting,
                inflight,
                depth,
            } => {
                if *inflight >= *depth {
                    return None;
                }
                let op = waiting.pop_front()?;
                *inflight += 1;
                Some(op)
            }
        }
    }

    fn complete(&mut self, bytes: u64, now: Duration) {
        match self {
            Queue::Priority(sched) => sched.complete(bytes, now),
            Queue::Fifo { inflight, .. } => *inflight = inflight.saturating_sub(1),
        }
    }

    fn next_wakeup(&self, now: Duration) -> Option<Duration> {
        match self {
            Queue::Priority(sched) => sched.next_wakeup(now),
            Queue::Fifo { .. } => None,
        }
    }
}

struct State {
    queue: Queue,
    rng: Xoshiro256PlusPlus,
    model: DeviceModel,
    next_id: u64,
    /// When the hardware queue is empty again.
    free_at: Duration,
    /// Operations started, with the time their service ends.
    started: BTreeMap<OpId, Duration>,
    /// Tasks waiting for their operation to start.
    waiters: BTreeMap<OpId, Waker>,
    /// Operations whose task was dropped before they started: they give
    /// their slot back as soon as the queue starts them.
    cancelled: BTreeSet<OpId>,
}

/// A device shared by its files; clones share it.
#[derive(Clone)]
pub struct SimDevice {
    clock: SimClock,
    state: Rc<RefCell<State>>,
}

impl SimDevice {
    /// An idle device.
    pub fn new(
        clock: SimClock,
        rng: Xoshiro256PlusPlus,
        model: DeviceModel,
        config: SchedulerConfig,
        policy: Policy,
    ) -> Self {
        let queue = match policy {
            Policy::Priority => {
                Queue::Priority(Box::new(Scheduler::new(config, clock.monotonic())))
            }
            Policy::Fifo => Queue::Fifo {
                waiting: VecDeque::new(),
                inflight: 0,
                depth: config.depth.max(1),
            },
        };
        Self {
            clock,
            state: Rc::new(RefCell::new(State {
                queue,
                rng,
                model,
                next_id: 0,
                free_at: Duration::ZERO,
                started: BTreeMap::new(),
                waiters: BTreeMap::new(),
                cancelled: BTreeSet::new(),
            })),
        }
    }

    /// Runs `work` as an operation of `class` moving `cost` bytes: waits
    /// for the scheduler to start it, then for the device to serve it.
    pub async fn run<T>(&self, class: IoClass, cost: u64, work: impl Future<Output = T>) -> T {
        let id = {
            let mut st = self.state.borrow_mut();
            st.next_id += 1;
            let id = OpId(st.next_id);
            st.queue.submit(Op { id, class, cost });
            id
        };
        // A task dropped mid-way (a timeout, a simulated crash) must give
        // its slot back, or the device would lock up after `depth` of them.
        let mut guard = Slot {
            device: self,
            id,
            cost,
            started: false,
        };
        let done_at = self.wait_start(id).await;
        guard.started = true;
        self.clock
            .sleep(done_at.saturating_sub(self.clock.monotonic()))
            .await;
        let output = work.await;
        drop(guard);
        output
    }

    /// Releases the slot of operation `id`: completes it if it started,
    /// cancels it otherwise.
    fn release(&self, id: OpId, cost: u64, started: bool) {
        {
            let now = self.clock.monotonic();
            let mut st = self.state.borrow_mut();
            st.waiters.remove(&id);
            if started || st.started.remove(&id).is_some() {
                st.queue.complete(cost, now);
            } else {
                st.cancelled.insert(id);
            }
        }
        self.dispatch();
    }

    async fn wait_start(&self, id: OpId) -> Duration {
        poll_fn(|cx| {
            self.dispatch();
            let now = self.clock.monotonic();
            let mut st = self.state.borrow_mut();
            if let Some(done_at) = st.started.remove(&id) {
                return Poll::Ready(done_at);
            }
            st.waiters.insert(id, cx.waker().clone());
            // Only the background bucket makes time matter while waiting.
            if let Some(at) = st.queue.next_wakeup(now).filter(|at| *at > now) {
                self.clock.wake_at(at, cx.waker().clone());
            }
            Poll::Pending
        })
        .await
    }

    /// Starts every operation the queue allows now and wakes their tasks.
    fn dispatch(&self) {
        let now = self.clock.monotonic();
        let mut wake = Vec::new();
        {
            let mut st = self.state.borrow_mut();
            while let Some(op) = st.queue.next(now) {
                if st.cancelled.remove(&op.id) {
                    st.queue.complete(0, now);
                    continue;
                }
                let service = st.service(op.cost);
                let done_at = st.free_at.max(now) + service;
                st.free_at = done_at;
                st.started.insert(op.id, done_at);
                if let Some(waker) = st.waiters.remove(&op.id) {
                    wake.push(waker);
                }
            }
        }
        // Wake outside the borrow: a waker may poll the device again.
        for waker in wake {
            waker.wake();
        }
    }
}

impl State {
    fn service(&mut self, cost: u64) -> Duration {
        // 53 random bits as a fraction in [0, 1).
        let unit = (self.rng.next_u64() >> 11) as f64 / (1u64 << 53) as f64;
        let factor = 1.0 - JITTER + 2.0 * JITTER * unit;
        self.model.service(cost).mul_f64(factor)
    }
}

/// The slot of one operation of [`SimDevice::run`], released on drop.
struct Slot<'a> {
    device: &'a SimDevice,
    id: OpId,
    cost: u64,
    started: bool,
}

impl Drop for Slot<'_> {
    fn drop(&mut self) {
        self.device.release(self.id, self.cost, self.started);
    }
}
