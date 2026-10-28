//! The I/O thread pool of one device (T02.06, ADR-016 thread-pool backend).
//!
//! A dispatcher thread owns the device's [`Scheduler`]. Each core submits
//! through its own bounded lock-free queue (`rtrb`), read by the dispatcher
//! only: no lock on the cores' path (ADR-003). The dispatcher hands the
//! operations the scheduler picks to `depth` I/O threads, which run them
//! and report back. A full queue answers `WouldBlock` ("busy", P8); a
//! stopped pool answers `Interrupted`.

use std::cell::RefCell;
use std::future::Future;
use std::io;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering, fence};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::task::{Context, Poll};
use std::thread::{self, JoinHandle, Thread};
use std::time::Duration;

use ndaal_observe::metrics::Registry;
use rtrb::{Consumer, Producer, RingBuffer};
use tokio::sync::oneshot;

use super::SystemClock;
use super::dispatcher::{Dispatcher, worker};
use super::pool_metrics::{IoKind, PoolMetrics};
use crate::Clock;
use crate::sched::{IoClass, SchedulerConfig};

/// Submissions each core may have waiting for the dispatcher, per class:
/// room for a burst of a core, small enough to bound memory (P8). One queue
/// per class, so a class that waits (the background bucket) never holds
/// back the others.
const SUBMIT_CAPACITY: usize = 256;

/// The queues of one submitter, by [`IoClass::index`].
pub(super) type ClassQueues = [Consumer<Request>; 5];
/// Longest wait for the threads at shutdown (spec section 3.3).
const STOP_TIMEOUT: Duration = Duration::from_secs(10);
/// How often shutdown checks whether the threads ended.
const STOP_POLL: Duration = Duration::from_millis(1);

pub(super) type Job = Box<dyn FnOnce() + Send>;

pub(super) struct Request {
    pub(super) class: IoClass,
    pub(super) kind: IoKind,
    pub(super) cost: u64,
    pub(super) job: Job,
}

/// An operation handed to an I/O thread.
pub(super) struct Running {
    pub(super) job: Job,
    pub(super) done: Done,
}

/// What the dispatcher needs when an operation ends.
pub(super) struct Done {
    pub(super) class: IoClass,
    pub(super) kind: IoKind,
    pub(super) cost: u64,
    pub(super) queued_at: Duration,
}

pub(super) struct Shared {
    pub(super) stopping: AtomicBool,
    dispatcher: OnceLock<Thread>,
    /// Queues of new submitters; locked only when one is created.
    pub(super) new_queues: Mutex<Vec<ClassQueues>>,
    pub(super) depth: usize,
}

impl Shared {
    pub(super) fn wake(&self) {
        // `unpark` keeps a token when the dispatcher is not parked yet: a
        // wakeup is never lost.
        if let Some(thread) = self.dispatcher.get() {
            thread.unpark();
        }
    }
}

/// The I/O threads of one device.
pub struct IoPool {
    shared: Arc<Shared>,
    threads: Vec<JoinHandle<()>>,
}

/// Why [`IoPool::shutdown`] did not end cleanly.
#[derive(Debug, thiserror::Error)]
pub enum PoolShutdownError {
    /// Operations still running after [`STOP_TIMEOUT`]; their threads are
    /// left behind.
    #[error("i/o threads still running after {0:?}")]
    Timeout(Duration),
    /// A thread of the pool panicked.
    #[error("an i/o thread panicked")]
    Panicked,
}

impl IoPool {
    /// Starts the dispatcher and `config.depth` I/O threads of `device`,
    /// pinned to `cpus` in turn when there are any (the `io` cores).
    pub fn start(
        device: &str,
        config: SchedulerConfig,
        cpus: &[usize],
        registry: &Registry,
    ) -> io::Result<IoPool> {
        let depth = config.depth.max(1);
        let metrics = PoolMetrics::new(device, registry)?;
        let shared = Arc::new(Shared {
            stopping: AtomicBool::new(false),
            dispatcher: OnceLock::new(),
            new_queues: Mutex::new(Vec::new()),
            depth,
        });
        let (work_tx, work_rx) = mpsc::sync_channel::<Running>(depth);
        let (done_tx, done_rx) = mpsc::sync_channel::<Done>(depth);
        let work_rx = Arc::new(Mutex::new(work_rx));
        let mut threads = Vec::with_capacity(depth + 1);
        for i in 0..depth {
            let (work_rx, done_tx, shared) =
                (Arc::clone(&work_rx), done_tx.clone(), Arc::clone(&shared));
            let cpu = (!cpus.is_empty()).then(|| cpus[i % cpus.len()]);
            threads.push(
                thread::Builder::new()
                    .name(format!("ndaal-io-{device}-{i}"))
                    .spawn(move || worker(cpu, &work_rx, &done_tx, &shared))?,
            );
        }
        let dispatcher = Dispatcher::new(Arc::clone(&shared), config, work_tx, done_rx, metrics);
        let handle = thread::Builder::new()
            .name(format!("ndaal-io-{device}"))
            .spawn(move || dispatcher.run())?;
        // Set before any submitter exists: `pool.submitter()` needs `&self`.
        let _ = shared.dispatcher.set(handle.thread().clone());
        threads.push(handle);
        Ok(IoPool { shared, threads })
    }

    /// A submitter for one core, with its own queue.
    pub fn submitter(&self) -> IoSubmitter {
        let (producers, consumers): (Vec<_>, Vec<_>) = IoClass::ALL
            .iter()
            .map(|_| RingBuffer::new(SUBMIT_CAPACITY))
            .unzip();
        let consumers: ClassQueues = consumers
            .try_into()
            .unwrap_or_else(|_| unreachable!("one queue per class"));
        let producers: [Producer<Request>; 5] = producers
            .try_into()
            .unwrap_or_else(|_| unreachable!("one queue per class"));
        self.shared
            .new_queues
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(consumers);
        IoSubmitter {
            shared: Arc::clone(&self.shared),
            queues: RefCell::new(producers),
        }
    }

    /// Stops the pool: waiting operations fail with `Interrupted`, those in
    /// flight complete, then the threads end, within [`STOP_TIMEOUT`].
    pub fn shutdown(self) -> Result<(), PoolShutdownError> {
        self.shared.stopping.store(true, Ordering::SeqCst);
        self.shared.wake();
        let clock = SystemClock::new();
        while !self.threads.iter().all(JoinHandle::is_finished) {
            if clock.monotonic() >= STOP_TIMEOUT {
                return Err(PoolShutdownError::Timeout(STOP_TIMEOUT));
            }
            thread::sleep(STOP_POLL);
        }
        let mut result = Ok(());
        for handle in self.threads {
            if handle.join().is_err() {
                result = Err(PoolShutdownError::Panicked);
            }
        }
        result
    }
}

/// The submission end of one core. `Send`, so it can be built on the main
/// thread and moved to its core; not `Sync`: one core uses it.
pub struct IoSubmitter {
    shared: Arc<Shared>,
    queues: RefCell<[Producer<Request>; 5]>,
}

/// The result of a submitted operation.
pub struct Completion<T> {
    reply: oneshot::Receiver<io::Result<T>>,
}

impl<T> Future for Completion<T> {
    type Output = io::Result<T>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<T>> {
        // A dropped reply means the pool stopped before running the work.
        Pin::new(&mut self.reply)
            .poll(cx)
            .map(|reply| reply.unwrap_or_else(|_| Err(io::ErrorKind::Interrupted.into())))
    }
}

impl IoSubmitter {
    /// Queues `work`, an operation of `kind` moving `cost` bytes, in
    /// `class`. Refused at once with `WouldBlock` when the queues are full
    /// and `Interrupted` when the pool stops.
    pub fn submit<T, F>(
        &self,
        class: IoClass,
        kind: IoKind,
        cost: u64,
        work: F,
    ) -> io::Result<Completion<T>>
    where
        T: Send + 'static,
        F: FnOnce() -> io::Result<T> + Send + 'static,
    {
        if self.shared.stopping.load(Ordering::SeqCst) {
            return Err(io::ErrorKind::Interrupted.into());
        }
        let (tx, reply) = oneshot::channel();
        let job: Job = Box::new(move || {
            let _ = tx.send(work());
        });
        let request = Request {
            class,
            kind,
            cost,
            job,
        };
        if self.queues.borrow_mut()[class.index()]
            .push(request)
            .is_err()
        {
            return Err(io::ErrorKind::WouldBlock.into());
        }
        // Pairs with the fence of the dispatcher's last pass: either it
        // reads this request, or this load sees `stopping`.
        fence(Ordering::SeqCst);
        if self.shared.stopping.load(Ordering::SeqCst) {
            // The last pass may be over: the request might never be read.
            // Failing is the safe side: a caller never takes an operation
            // that did not run for done.
            return Err(io::ErrorKind::Interrupted.into());
        }
        self.shared.wake();
        Ok(Completion { reply })
    }
}
