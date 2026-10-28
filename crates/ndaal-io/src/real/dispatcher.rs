//! The dispatcher of a device's pool and its I/O threads.

use std::collections::BTreeMap;
use std::sync::atomic::{Ordering, fence};
use std::sync::mpsc::{Receiver, SyncSender};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::Duration;

use super::SystemClock;
use super::pool::{ClassQueues, Done, Job, Running, Shared};
use super::pool_metrics::PoolMetrics;
use crate::Clock;
use crate::sched::{Op, OpId, Scheduler, SchedulerConfig};

/// Operations of one class the dispatcher keeps queued in its scheduler, per
/// unit of depth; past it, it stops reading that class, so the cores'
/// queues of that class fill up (P8) and the other classes keep flowing.
const QUEUED_PER_DEPTH: usize = 64;
/// Longest sleep of the dispatcher waiting for the last completions at
/// shutdown.
const INFLIGHT_POLL: Duration = Duration::from_millis(10);

struct Pending {
    job: Job,
    done: Done,
}

pub(super) struct Dispatcher {
    shared: Arc<Shared>,
    clock: SystemClock,
    sched: Scheduler,
    queues: Vec<ClassQueues>,
    /// Operations read and not started, by class.
    queued: [usize; 5],
    waiting: BTreeMap<OpId, Pending>,
    next_id: u64,
    work_tx: SyncSender<Running>,
    done_rx: Receiver<Done>,
    metrics: PoolMetrics,
}

impl Dispatcher {
    pub(super) fn new(
        shared: Arc<Shared>,
        config: SchedulerConfig,
        work_tx: SyncSender<Running>,
        done_rx: Receiver<Done>,
        metrics: PoolMetrics,
    ) -> Self {
        Self {
            shared,
            clock: SystemClock::new(),
            sched: Scheduler::new(config, Duration::ZERO),
            queues: Vec::new(),
            queued: [0; 5],
            waiting: BTreeMap::new(),
            next_id: 0,
            work_tx,
            done_rx,
            metrics,
        }
    }

    pub(super) fn run(mut self) {
        loop {
            let stopping = self.shared.stopping.load(Ordering::SeqCst);
            fence(Ordering::SeqCst);
            let now = self.clock.monotonic();
            // Completions before submissions: a request queued before an
            // operation ended is read before the slot it frees is reused
            // (the completion message orders the two).
            self.read_completions(now);
            self.read_submissions(now);
            if stopping {
                self.fail_waiting();
                self.finish_inflight();
                return;
            }
            self.start_ready(now);
            self.update_gauges();
            match self.sched.next_wakeup(now) {
                Some(at) if at <= now => {}
                Some(at) => {
                    self.metrics.throttled.increment(1);
                    thread::park_timeout(at - now);
                }
                None => thread::park(),
            }
        }
    }

    /// Waits for the operations in flight, so their completions are
    /// counted; `IoPool::shutdown` bounds the wait.
    fn finish_inflight(&mut self) {
        while self.sched.inflight() > 0 {
            // Each I/O thread wakes the dispatcher after reporting; the
            // timeout only guards against a thread that died.
            thread::park_timeout(INFLIGHT_POLL);
            let now = self.clock.monotonic();
            self.read_completions(now);
        }
        self.update_gauges();
    }

    fn update_gauges(&self) {
        self.metrics.inflight.set(gauge(self.sched.inflight()));
        self.metrics
            .bandwidth
            .set(i64::try_from(self.sched.bandwidth()).unwrap_or(i64::MAX));
    }

    fn read_submissions(&mut self, now: Duration) {
        self.queues.extend(
            self.shared
                .new_queues
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .drain(..),
        );
        let limit = self.shared.depth * QUEUED_PER_DEPTH;
        for queues in &mut self.queues {
            for (class, queue) in queues.iter_mut().enumerate() {
                while self.queued[class] < limit {
                    let Ok(request) = queue.pop() else { break };
                    self.queued[class] += 1;
                    self.next_id += 1;
                    let id = OpId(self.next_id);
                    self.sched.submit(Op {
                        id,
                        class: request.class,
                        cost: request.cost,
                    });
                    let done = Done {
                        class: request.class,
                        kind: request.kind,
                        cost: request.cost,
                        queued_at: now,
                    };
                    let pending = Pending {
                        job: request.job,
                        done,
                    };
                    self.waiting.insert(id, pending);
                }
            }
        }
    }

    fn read_completions(&mut self, now: Duration) {
        while let Ok(done) = self.done_rx.try_recv() {
            self.sched.complete(done.cost, now);
            self.metrics
                .record(done.class, done.kind, now.saturating_sub(done.queued_at));
        }
    }

    fn start_ready(&mut self, now: Duration) {
        while let Some(op) = self.sched.next(now) {
            let Some(pending) = self.waiting.remove(&op.id) else {
                // Every op the scheduler holds was read with its job; give
                // the slot back rather than lose it if that ever breaks.
                debug_assert!(false, "op {:?} started without its job", op.id);
                self.sched.complete(0, now);
                continue;
            };
            self.queued[op.class.index()] -= 1;
            let running = Running {
                job: pending.job,
                done: pending.done,
            };
            // At most `depth` operations run, and the channel holds
            // `depth`: this never waits. It fails only if every I/O thread
            // is gone, and then the job is dropped: its caller gets
            // `Interrupted`.
            let _ = self.work_tx.send(running);
        }
    }

    /// Drops every operation not started, and every submission not read:
    /// their callers get `Interrupted`.
    fn fail_waiting(&mut self) {
        self.sched.drain();
        self.waiting.clear();
        self.queued = [0; 5];
        for queue in self.queues.iter_mut().flatten() {
            while queue.pop().is_ok() {}
        }
    }
}

fn gauge(n: usize) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

pub(super) fn worker(
    cpu: Option<usize>,
    work_rx: &Mutex<Receiver<Running>>,
    done_tx: &SyncSender<Done>,
    shared: &Shared,
) {
    if let Some(id) = cpu {
        // Best effort: a refused pinning leaves the thread unpinned.
        let _ = core_affinity::set_for_current(core_affinity::CoreId { id });
    }
    loop {
        // The lock is shared by the I/O threads only, never by the cores.
        let next = work_rx
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .recv();
        let Ok(running) = next else { return };
        (running.job)();
        if done_tx.send(running.done).is_err() {
            return;
        }
        shared.wake();
    }
}
