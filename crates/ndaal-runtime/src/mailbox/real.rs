//! Real transport: `rtrb` single-producer single-consumer queues between
//! core threads; an idle core polls for `idle_spin`, then sleeps until a
//! sender wakes it (only if it sleeps: no system call on the busy path).

use std::cell::{Cell, RefCell};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering, fence};
use std::time::Duration;

use ndaal_io::Clock;
use ndaal_io::real::SystemClock;
use ndaal_observe::metrics::{Counter, Registry};
use rtrb::{Consumer, Producer, RingBuffer};
use tokio::sync::Notify;

use super::{CoreId, CoreMailbox, Envelope, ReplyWire, Transport};

/// The messaging endpoint of a real core.
pub type RealMailbox<M, R> = CoreMailbox<M, R, RealTransport<M, R>>;

/// Sleep state of one queue set of one core.
#[derive(Default)]
struct Signal {
    sleeping: AtomicBool,
    notify: Notify,
}

impl Signal {
    /// Called by a sender after publishing.
    fn wake(&self) {
        // Pairs with the fence in `RealTransport::wait`: either the sender
        // sees `sleeping`, or the core sees the published item.
        fence(Ordering::SeqCst);
        if self.sleeping.load(Ordering::SeqCst) {
            // Stores a permit if the core is not waiting yet: never lost.
            self.notify.notify_one();
        }
    }
}

/// State shared by every core of a runtime.
struct Shared {
    cores: u16,
    messages: Vec<Signal>,
    replies: Vec<Signal>,
    shutdown: AtomicBool,
}

/// The queues of one core, built on the main thread and moved to the core
/// thread, where [`Endpoint::into_mailbox`] turns it into its mailbox.
pub struct Endpoint<M, R> {
    me: CoreId,
    shared: Arc<Shared>,
    capacity: usize,
    out_messages: Vec<Producer<Envelope<M>>>,
    out_replies: Vec<Producer<ReplyWire<R>>>,
    in_messages: Vec<Consumer<Envelope<M>>>,
    in_replies: Vec<Consumer<ReplyWire<R>>>,
}

/// Endpoints of `cores` cores, one queue of `capacity` items per pair and
/// direction, for messages and for replies.
pub fn real_endpoints<M: Send, R: Send>(cores: u16, capacity: usize) -> Vec<Endpoint<M, R>> {
    let n = usize::from(cores);
    let shared = Arc::new(Shared {
        cores,
        messages: (0..n).map(|_| Signal::default()).collect(),
        replies: (0..n).map(|_| Signal::default()).collect(),
        shutdown: AtomicBool::new(false),
    });
    // queue[from][to]
    let mut messages: Vec<Vec<_>> = (0..n)
        .map(|_| (0..n).map(|_| Some(RingBuffer::new(capacity))).collect())
        .collect();
    let mut replies: Vec<Vec<_>> = (0..n)
        .map(|_| (0..n).map(|_| Some(RingBuffer::new(capacity))).collect())
        .collect();
    let mut endpoints: Vec<Endpoint<M, R>> = (0..n)
        .map(|me| Endpoint {
            me: CoreId(me as u16),
            shared: Arc::clone(&shared),
            capacity,
            out_messages: Vec::with_capacity(n),
            out_replies: Vec::with_capacity(n),
            in_messages: Vec::with_capacity(n),
            in_replies: Vec::with_capacity(n),
        })
        .collect();
    for from in 0..n {
        for to in 0..n {
            let (tx, rx) = messages[from][to].take().expect("each pair is taken once");
            endpoints[from].out_messages.push(tx);
            endpoints[to].in_messages.push(rx);
            let (tx, rx) = replies[from][to].take().expect("each pair is taken once");
            endpoints[from].out_replies.push(tx);
            endpoints[to].in_replies.push(rx);
        }
    }
    endpoints
}

/// Stops a real runtime from any thread (the mailboxes stay on theirs).
#[derive(Clone)]
pub struct ShutdownHandle(Arc<Shared>);

impl ShutdownHandle {
    /// Same as [`CoreMailbox::shutdown`] on any core.
    pub fn shutdown(&self) {
        shutdown_all(&self.0);
    }
}

fn shutdown_all(shared: &Shared) {
    shared.shutdown.store(true, Ordering::SeqCst);
    for signal in shared.messages.iter().chain(&shared.replies) {
        // A permit for each core, waiting or not.
        signal.notify.notify_one();
    }
}

impl<M, R> Endpoint<M, R> {
    /// Stops the runtime this endpoint belongs to.
    pub fn shutdown_handle(&self) -> ShutdownHandle {
        ShutdownHandle(Arc::clone(&self.shared))
    }
}

impl<M: 'static, R: 'static> Endpoint<M, R> {
    /// The mailbox of this core; call it on the core thread.
    pub fn into_mailbox(
        self,
        idle_spin: Duration,
        clock: SystemClock,
        registry: &Registry,
    ) -> RealMailbox<M, R> {
        let core = self.me.0.to_string();
        let parks = |queue: &str| {
            registry
                .counter(
                    "ndaal_runtime_core_parks_total",
                    "Times a core went to sleep waiting on its messages or replies.",
                    &[("core", &core), ("queue", queue)],
                )
                .expect("constant metric name, registered as a counter")
        };
        let transport = RealTransport {
            me: self.me,
            shared: self.shared,
            out_messages: RefCell::new(self.out_messages),
            out_replies: RefCell::new(self.out_replies),
            in_messages: RefCell::new(self.in_messages),
            in_replies: RefCell::new(self.in_replies),
            next_message: Cell::new(0),
            next_reply: Cell::new(0),
            idle_spin,
            clock,
            message_parks: parks("messages"),
            reply_parks: parks("replies"),
        };
        CoreMailbox::new(self.me, transport, self.capacity, registry)
    }
}

/// The real transport of one core.
pub struct RealTransport<M, R> {
    me: CoreId,
    shared: Arc<Shared>,
    out_messages: RefCell<Vec<Producer<Envelope<M>>>>,
    out_replies: RefCell<Vec<Producer<ReplyWire<R>>>>,
    in_messages: RefCell<Vec<Consumer<Envelope<M>>>>,
    in_replies: RefCell<Vec<Consumer<ReplyWire<R>>>>,
    next_message: Cell<usize>,
    next_reply: Cell<usize>,
    idle_spin: Duration,
    clock: SystemClock,
    message_parks: Counter,
    reply_parks: Counter,
}

/// Pops from the senders in turn, starting after the last one served, so
/// no sender starves the others.
fn take_round_robin<T>(
    queues: &RefCell<Vec<Consumer<T>>>,
    next: &Cell<usize>,
) -> Option<(CoreId, T)> {
    let mut queues = queues.borrow_mut();
    let n = queues.len();
    for k in 0..n {
        let from = (next.get() + k) % n;
        if let Ok(item) = queues[from].pop() {
            next.set(from + 1);
            return Some((CoreId(from as u16), item));
        }
    }
    None
}

impl<M, R> RealTransport<M, R> {
    /// Polls `ready` for `idle_spin`, letting the core's other tasks run,
    /// then sleeps until woken.
    async fn wait(&self, signal: &Signal, parks: &Counter, ready: impl Fn() -> bool) {
        let ready = || ready() || self.shared.shutdown.load(Ordering::SeqCst);
        let start = self.clock.monotonic();
        while self.clock.monotonic() - start < self.idle_spin {
            if ready() {
                return;
            }
            tokio::task::yield_now().await;
        }
        signal.sleeping.store(true, Ordering::SeqCst);
        fence(Ordering::SeqCst);
        if !ready() {
            parks.increment(1);
            signal.notify.notified().await;
        }
        signal.sleeping.store(false, Ordering::SeqCst);
    }
}

impl<M, R> Transport<M, R> for RealTransport<M, R> {
    fn send(&self, to: CoreId, envelope: Envelope<M>) -> Result<(), Envelope<M>> {
        let pushed = self.out_messages.borrow_mut()[usize::from(to.0)].push(envelope);
        pushed.map_err(|rtrb::PushError::Full(envelope)| envelope)?;
        self.shared.messages[usize::from(to.0)].wake();
        Ok(())
    }

    fn send_reply(&self, to: CoreId, reply: ReplyWire<R>) -> Result<(), ReplyWire<R>> {
        let pushed = self.out_replies.borrow_mut()[usize::from(to.0)].push(reply);
        pushed.map_err(|rtrb::PushError::Full(reply)| reply)?;
        self.shared.replies[usize::from(to.0)].wake();
        Ok(())
    }

    fn take_message(&self) -> Option<(CoreId, Envelope<M>)> {
        take_round_robin(&self.in_messages, &self.next_message)
    }

    fn take_reply(&self) -> Option<(CoreId, ReplyWire<R>)> {
        take_round_robin(&self.in_replies, &self.next_reply)
    }

    async fn wait_messages(&self) {
        let signal = &self.shared.messages[usize::from(self.me.0)];
        self.wait(signal, &self.message_parks, || {
            self.in_messages.borrow().iter().any(|q| !q.is_empty())
        })
        .await;
    }

    async fn wait_replies(&self) {
        let signal = &self.shared.replies[usize::from(self.me.0)];
        self.wait(signal, &self.reply_parks, || {
            self.in_replies.borrow().iter().any(|q| !q.is_empty())
        })
        .await;
    }

    fn shutdown_all(&self) {
        shutdown_all(&self.shared);
    }

    fn is_shut_down(&self) -> bool {
        self.shared.shutdown.load(Ordering::SeqCst)
    }

    fn cores(&self) -> u16 {
        self.shared.cores
    }
}
