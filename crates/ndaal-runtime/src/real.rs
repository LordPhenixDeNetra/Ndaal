//! One thread per core, each running a `current_thread` tokio runtime,
//! pinned to its CPU when the system allows it (ADR-003, T02.01).

use std::future::Future;
use std::io;
use std::rc::Rc;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use ndaal_io::Clock;
use ndaal_io::real::SystemClock;
use ndaal_observe::metrics::Registry;
use tokio::sync::mpsc;

use crate::layout::{Class, CoreSpec, Layout};
use crate::mailbox::CoreId;
use crate::mailbox::real::{RealMailbox, ShutdownHandle, real_endpoints};

/// Longest wait for every core to be ready.
const START_TIMEOUT: Duration = Duration::from_secs(5);
/// Longest wait for every core to stop.
const STOP_TIMEOUT: Duration = Duration::from_secs(10);

/// How the runtime runs.
pub struct RuntimeOptions {
    /// Pin each core thread to its CPU (`runtime.cores.pin`).
    pub pin: bool,
    /// Polling before an idle core sleeps (`runtime.idle_spin`).
    pub idle_spin: Duration,
    /// Capacity of each queue between two cores.
    pub capacity: usize,
    /// Where the runtime metrics go.
    pub registry: Registry,
}

/// What the task of one core gets.
pub struct CoreContext<M, R> {
    /// This core.
    pub id: CoreId,
    /// Its place in the layout.
    pub spec: CoreSpec,
    /// Its messaging endpoint; its reply pump already runs.
    pub mailbox: Rc<RealMailbox<M, R>>,
}

/// Why the runtime did not start.
#[derive(Debug, thiserror::Error)]
pub enum StartError {
    /// A thread or its tokio runtime could not be created.
    #[error("cannot start a core thread: {0}")]
    Spawn(#[source] io::Error),
    /// A thread could not be pinned to its CPU.
    #[error("cannot pin a core thread to cpu {cpu}")]
    Pin {
        /// The CPU of the layout.
        cpu: usize,
    },
    /// Not every core was ready in time.
    #[error("the cores were not ready within {0:?}")]
    Timeout(Duration),
}

/// Cores that did not stop within the stop timeout.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("cores did not stop in time: {stuck:?}")]
pub struct ShutdownError {
    /// The cores still running.
    pub stuck: Vec<CoreId>,
}

/// What a core thread reports to the starting thread.
enum Event {
    Ready,
    Failed(StartError),
    Done(CoreId),
}

/// Sends `Done` when the thread ends, even by a panic (tests unwind).
struct DoneGuard(CoreId, mpsc::UnboundedSender<Event>);

impl Drop for DoneGuard {
    fn drop(&mut self) {
        let _ = self.1.send(Event::Done(self.0));
    }
}

/// The running cores.
pub struct Runtime {
    threads: Vec<(CoreId, JoinHandle<()>)>,
    events: Events,
    stop: ShutdownHandle,
}

/// Events of the core threads, read with a deadline on the system clock
/// (ADR-017: timed waits go through `ndaal_io::Clock`).
struct Events {
    rx: mpsc::UnboundedReceiver<Event>,
    waiter: tokio::runtime::Runtime,
    clock: SystemClock,
}

impl Events {
    fn new(rx: mpsc::UnboundedReceiver<Event>) -> io::Result<Self> {
        let waiter = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()?;
        Ok(Self {
            rx,
            waiter,
            clock: SystemClock::new(),
        })
    }

    /// The next event before `deadline` (on `self.clock`), or `None`.
    fn next_before(&mut self, deadline: Duration) -> Option<Event> {
        let left = deadline.saturating_sub(self.clock.monotonic());
        let (rx, clock) = (&mut self.rx, self.clock);
        self.waiter.block_on(async move {
            tokio::select! {
                event = rx.recv() => event,
                () = clock.sleep(left) => None,
            }
        })
    }
}

/// CPUs this process may run on (cgroup, `taskset`), or `0..n` when the
/// system does not tell.
pub fn allowed_cpus() -> Vec<usize> {
    match core_affinity::get_core_ids() {
        Some(ids) if !ids.is_empty() => ids.into_iter().map(|c| c.id).collect(),
        _ => (0..std::thread::available_parallelism().map_or(1, usize::from)).collect(),
    }
}

impl Runtime {
    /// Starts one thread per core of `layout`; each runs `factory(context)`
    /// until it returns (on `recv() == None` after shutdown, usually).
    pub fn start<M, R, F, Fut>(
        layout: &Layout,
        options: RuntimeOptions,
        factory: F,
    ) -> Result<Runtime, StartError>
    where
        M: Send + 'static,
        R: Send + 'static,
        F: Fn(CoreContext<M, R>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + 'static,
    {
        let count = u16::try_from(layout.cores.len()).expect("a layout has at most 4096 cores");
        let endpoints = real_endpoints::<M, R>(count, options.capacity);
        let stop = endpoints.first().map(|e| e.shutdown_handle());
        let pin = options.pin && !cfg!(target_os = "macos");
        if options.pin && !pin {
            tracing::warn!("runtime pinning unsupported");
        }
        set_core_gauges(layout, &options.registry);
        let factory = Arc::new(factory);
        let (events_tx, events_rx) = mpsc::unbounded_channel();
        let events = Events::new(events_rx).map_err(StartError::Spawn)?;
        let mut threads = Vec::new();
        for (endpoint, spec) in endpoints.into_iter().zip(layout.cores.iter().cloned()) {
            let id = CoreId(threads.len() as u16);
            let (factory, events_tx) = (Arc::clone(&factory), events_tx.clone());
            let (registry, idle_spin) = (options.registry.clone(), options.idle_spin);
            let spawned = std::thread::Builder::new()
                .name(format!("ndaal-core-{}", id.0))
                .spawn(move || {
                    let _done = DoneGuard(id, events_tx.clone());
                    if pin
                        && !core_affinity::set_for_current(core_affinity::CoreId { id: spec.cpu })
                    {
                        let _ = events_tx.send(Event::Failed(StartError::Pin { cpu: spec.cpu }));
                        return;
                    }
                    let runtime = match tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                    {
                        Ok(runtime) => runtime,
                        Err(e) => {
                            let _ = events_tx.send(Event::Failed(StartError::Spawn(e)));
                            return;
                        }
                    };
                    let local = tokio::task::LocalSet::new();
                    local.block_on(&runtime, async move {
                        let mailbox = Rc::new(endpoint.into_mailbox(
                            idle_spin,
                            SystemClock::new(),
                            &registry,
                        ));
                        let pump = Rc::clone(&mailbox);
                        let pump = tokio::task::spawn_local(async move { pump.pump().await });
                        let _ = events_tx.send(Event::Ready);
                        factory(CoreContext { id, spec, mailbox }).await;
                        let _ = pump.await;
                    });
                });
            match spawned {
                Ok(handle) => threads.push((id, handle)),
                Err(e) => {
                    let started = Runtime {
                        threads,
                        events,
                        stop: stop.clone().expect("threads exist"),
                    };
                    let _ = started.shutdown();
                    return Err(StartError::Spawn(e));
                }
            }
        }
        let Some(stop) = stop else {
            return Err(StartError::Timeout(Duration::ZERO));
        };
        let mut runtime = Runtime {
            threads,
            events,
            stop,
        };
        match runtime.wait_ready(usize::from(count)) {
            Ok(()) => Ok(runtime),
            Err(error) => {
                let _ = runtime.shutdown();
                Err(error)
            }
        }
    }

    fn wait_ready(&mut self, count: usize) -> Result<(), StartError> {
        let deadline = self.events.clock.monotonic() + START_TIMEOUT;
        let mut ready = 0;
        while ready < count {
            match self.events.next_before(deadline) {
                Some(Event::Ready) => ready += 1,
                Some(Event::Failed(error)) => return Err(error),
                // A core ended before being ready: it reported why first.
                Some(Event::Done(_)) => {}
                None => return Err(StartError::Timeout(START_TIMEOUT)),
            }
        }
        Ok(())
    }

    /// Stops every core: sends are refused, waiting requests fail, each
    /// core's task ends; waits at most the stop timeout.
    pub fn shutdown(mut self) -> Result<(), ShutdownError> {
        self.stop.shutdown();
        let deadline = self.events.clock.monotonic() + STOP_TIMEOUT;
        let mut running: Vec<CoreId> = self.threads.iter().map(|(id, _)| *id).collect();
        while !running.is_empty() {
            match self.events.next_before(deadline) {
                Some(Event::Done(id)) => running.retain(|r| *r != id),
                Some(_) => {}
                None => break,
            }
        }
        for (id, handle) in self.threads {
            if !running.contains(&id) {
                let _ = handle.join();
            }
        }
        if running.is_empty() {
            return Ok(());
        }
        for id in &running {
            tracing::error!(core = id.0, "runtime core did not stop");
        }
        Err(ShutdownError { stuck: running })
    }
}

fn set_core_gauges(layout: &Layout, registry: &Registry) {
    let classes = [
        (Class::Shard, "shard"),
        (Class::Vector, "vector"),
        (Class::Transfer, "transfer"),
        (Class::Background, "background"),
        (Class::Io, "io"),
    ];
    for (class, name) in classes {
        let gauge = registry
            .gauge(
                "ndaal_runtime_cores",
                "Cores dedicated to each load class.",
                &[("class", name)],
            )
            .expect("constant metric name, registered as a gauge");
        gauge.set(layout.count(class) as i64);
    }
}
