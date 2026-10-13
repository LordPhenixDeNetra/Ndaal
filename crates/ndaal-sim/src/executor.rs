//! Single-threaded deterministic scheduler (ADR-017, T01.09).
//!
//! Ready tasks are picked at random from the seed; when none is ready, virtual
//! time jumps to the next timer. Everything random derives from the seed, in a
//! fixed order, so two runs with the same seed produce the same trace.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::{Arc, Mutex, PoisonError};
use std::task::{Context, Wake, Waker};
use std::time::Duration;

use ndaal_io::sim::{SimClock, SimDisk, SimNetwork};
use ndaal_io::{Clock, Rng, Xoshiro256PlusPlus};

use crate::trace::Fnv;

/// Wall clock of a simulation at virtual time zero: 2026-01-01T00:00:00Z.
const START_UNIX_MICROS: u64 = 1_767_225_600_000_000;

/// When to stop a run that does not finish by itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Maximum number of task polls.
    pub max_steps: u64,
    /// Maximum virtual time.
    pub max_virtual_time: Duration,
}

/// How a run ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Every task completed.
    Finished,
    /// Tasks remain, none is ready and no timer is pending.
    Deadlock {
        /// Number of tasks still waiting.
        blocked: usize,
    },
    /// [`Limits::max_steps`] polls were made.
    StepLimit,
    /// The next timer is past [`Limits::max_virtual_time`].
    TimeLimit,
}

/// Identifier of a spawned task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TaskId(u64);

type Task = Pin<Box<dyn Future<Output = ()>>>;

/// A simulation; clones share it, so tasks can capture one and spawn more.
#[derive(Clone)]
pub struct Sim {
    core: Rc<Core>,
}

struct Core {
    scheduler: RefCell<Xoshiro256PlusPlus>,
    disk_seed: u64,
    rng_seed: u64,
    clock: SimClock,
    network: SimNetwork,
    disks: RefCell<BTreeMap<String, SimDisk>>,
    tasks: RefCell<BTreeMap<TaskId, Task>>,
    next_task: Cell<u64>,
    ready: Arc<Mutex<Ready>>,
    trace: Cell<Fnv>,
}

/// Tasks to poll; the set makes a double wake-up queue a task only once.
#[derive(Default)]
struct Ready {
    queue: Vec<TaskId>,
    queued: BTreeSet<TaskId>,
}

struct TaskWaker {
    id: TaskId,
    ready: Arc<Mutex<Ready>>,
}

impl Wake for TaskWaker {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        // Single-threaded: the lock is never contended, and never poisoned
        // because nothing panics while holding it.
        let mut ready = self.ready.lock().unwrap_or_else(PoisonError::into_inner);
        if ready.queued.insert(self.id) {
            ready.queue.push(self.id);
        }
    }
}

impl Sim {
    /// A simulation driven entirely by `seed`.
    pub fn new(seed: u64) -> Sim {
        // Sub-seeds in a fixed order: adding a component later must not
        // change the seeds of the existing ones.
        let mut root = Xoshiro256PlusPlus::from_seed(seed);
        let scheduler = Xoshiro256PlusPlus::from_seed(root.next_u64());
        let network_seed = root.next_u64();
        let disk_seed = root.next_u64();
        let rng_seed = root.next_u64();
        let clock = SimClock::new(START_UNIX_MICROS);
        let core = Core {
            scheduler: RefCell::new(scheduler),
            disk_seed,
            rng_seed,
            network: SimNetwork::new(network_seed, clock.clone()),
            clock,
            disks: RefCell::new(BTreeMap::new()),
            tasks: RefCell::new(BTreeMap::new()),
            next_task: Cell::new(0),
            ready: Arc::new(Mutex::new(Ready::default())),
            trace: Cell::new(Fnv::default()),
        };
        Sim {
            core: Rc::new(core),
        }
    }

    /// The virtual clock.
    pub fn clock(&self) -> SimClock {
        self.core.clock.clone()
    }

    /// The simulated network.
    pub fn network(&self) -> SimNetwork {
        self.core.network.clone()
    }

    /// The disk called `name` (created on first use, seeded from its name).
    pub fn disk(&self, name: &str) -> SimDisk {
        let seed = self.core.disk_seed ^ Fnv::of(name.as_bytes());
        self.core
            .disks
            .borrow_mut()
            .entry(name.to_owned())
            .or_insert_with(|| SimDisk::new(seed))
            .clone()
    }

    /// A random generator for one purpose, e.g. `"chaos"`: same seed and
    /// name, same sequence; different names give independent sequences.
    pub fn rng(&self, name: &str) -> Xoshiro256PlusPlus {
        Xoshiro256PlusPlus::from_seed(self.core.rng_seed ^ Fnv::of(name.as_bytes()))
    }

    /// Adds a task; it runs during [`Sim::run`].
    pub fn spawn(&self, future: impl Future<Output = ()> + 'static) -> TaskId {
        let id = TaskId(self.core.next_task.get());
        self.core.next_task.set(id.0 + 1);
        self.core.tasks.borrow_mut().insert(id, Box::pin(future));
        self.waker(id).wake();
        id
    }

    /// Adds an event to the trace.
    pub fn trace(&self, event: impl AsRef<[u8]>) {
        let mut trace = self.core.trace.get();
        trace.event(event.as_ref());
        self.core.trace.set(trace);
    }

    /// Fingerprint of the trace so far.
    pub fn fingerprint(&self) -> u64 {
        self.core.trace.get().value()
    }

    /// Runs until every task finishes, or a limit, or a deadlock.
    pub fn run(&self, limits: Limits) -> Outcome {
        let mut steps = 0;
        loop {
            // Checked before picking: a picked task must be polled, or it
            // would be lost for a later run.
            if steps >= limits.max_steps && self.has_ready() {
                return Outcome::StepLimit;
            }
            if let Some(id) = self.pick() {
                steps += 1;
                self.poll(id);
                continue;
            }
            let remaining = self.core.tasks.borrow().len();
            if remaining == 0 {
                return Outcome::Finished;
            }
            match self.core.clock.next_deadline() {
                Some(deadline) if deadline > limits.max_virtual_time => return Outcome::TimeLimit,
                Some(deadline) => self.core.clock.advance_to(deadline),
                None => return Outcome::Deadlock { blocked: remaining },
            }
        }
    }

    fn waker(&self, id: TaskId) -> Waker {
        Waker::from(Arc::new(TaskWaker {
            id,
            ready: Arc::clone(&self.core.ready),
        }))
    }

    fn has_ready(&self) -> bool {
        let ready = self
            .core
            .ready
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        !ready.queue.is_empty()
    }

    /// Removes a random ready task from the queue (by the seed).
    fn pick(&self) -> Option<TaskId> {
        let mut ready = self
            .core
            .ready
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if ready.queue.is_empty() {
            return None;
        }
        let len = ready.queue.len() as u64;
        // Bounded by the queue length: the cast back cannot truncate.
        let index = (self.core.scheduler.borrow_mut().next_u64() % len) as usize;
        let id = ready.queue.swap_remove(index);
        ready.queued.remove(&id);
        Some(id)
    }

    fn poll(&self, id: TaskId) {
        // Taken out of the map while polled, so the task may spawn others.
        let Some(mut task) = self.core.tasks.borrow_mut().remove(&id) else {
            return; // woken after it finished
        };
        let now = self.core.clock.monotonic().as_nanos();
        let mut event = id.0.to_le_bytes().to_vec();
        event.extend_from_slice(&now.to_le_bytes());
        self.trace(event);
        let waker = self.waker(id);
        if task
            .as_mut()
            .poll(&mut Context::from_waker(&waker))
            .is_pending()
        {
            self.core.tasks.borrow_mut().insert(id, task);
        }
    }
}
