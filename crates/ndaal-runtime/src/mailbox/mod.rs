//! Bounded messaging between cores (ARCHITECTURE.md section 4.2, T02.03).
//!
//! One logic, two transports: real (`rtrb` queues between threads) and
//! simulated (queues with a delivery time drawn from the seed, ADR-017).
//! Per pair of cores and per direction, one queue for messages and requests
//! and one for replies. A request reserves its reply slot before it leaves,
//! so a reply always fits and two cores requesting each other never lock up.
//!
//! Each core has a single consumer per queue: the task calling [`recv`]
//! (messages) and the task running [`pump`] (replies, which complete
//! [`Pending`] requests).
//!
//! [`recv`]: CoreMailbox::recv
//! [`pump`]: CoreMailbox::pump

mod shared;

pub mod real;
pub mod sim;

#[cfg(feature = "test-contract")]
pub mod contract;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use ndaal_observe::metrics::{Counter, Registry};

pub use shared::{Pending, Replier};
use shared::{PendingTable, ReplySink};

/// Index of a core in its runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CoreId(pub u16);

/// A message that was not sent; it is given back.
#[derive(Debug, PartialEq, Eq)]
pub enum SendError<M> {
    /// The queue to that core is full: answer "busy" (P8).
    Full(M),
    /// The runtime is stopping.
    ShuttingDown(M),
}

/// Why a request got no answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplyError {
    /// The receiver dropped the request without replying (a bug).
    Dropped,
    /// The runtime stopped first.
    ShuttingDown,
}

/// What a core receives.
#[derive(Debug)]
pub enum Incoming<M, R> {
    /// A message that expects no answer.
    Message {
        /// Sender.
        from: CoreId,
        /// Content.
        msg: M,
    },
    /// A request: answer through `replier`, once.
    Request {
        /// Sender.
        from: CoreId,
        /// Content.
        msg: M,
        /// Sends the answer back.
        replier: Replier<R>,
    },
}

/// A message or a request on the wire (internal to the transports).
pub enum Envelope<M> {
    /// Expects no answer.
    Message(M),
    /// Expects an answer under this request number.
    Request(u64, M),
}

/// A reply on the wire: the request number and the answer.
pub struct ReplyWire<R> {
    pub(crate) id: u64,
    pub(crate) result: Result<R, ReplyError>,
}

/// Moves envelopes and replies between cores. Implemented by the real and
/// the simulated transports only; public because it bounds the methods of
/// [`CoreMailbox`].
#[allow(async_fn_in_trait)] // Futures stay on their core (ADR-003).
pub trait Transport<M, R> {
    /// Sends to `to`; gives the envelope back when the queue is full.
    fn send(&self, to: CoreId, envelope: Envelope<M>) -> Result<(), Envelope<M>>;
    /// Sends a reply; gives it back only if the queue is full.
    fn send_reply(&self, to: CoreId, reply: ReplyWire<R>) -> Result<(), ReplyWire<R>>;
    /// Next envelope that arrived, FIFO per sender.
    fn take_message(&self) -> Option<(CoreId, Envelope<M>)>;
    /// Next reply that arrived, FIFO per sender.
    fn take_reply(&self) -> Option<(CoreId, ReplyWire<R>)>;
    /// Completes when a message may have arrived, or on shutdown.
    async fn wait_messages(&self);
    /// Completes when a reply may have arrived, or on shutdown.
    async fn wait_replies(&self);
    /// Stops every core of the runtime and wakes them.
    fn shutdown_all(&self);
    /// Whether the runtime is stopping.
    fn is_shut_down(&self) -> bool;
    /// Number of cores.
    fn cores(&self) -> u16;
}

/// The messaging endpoint of one core. Not `Send`: it lives on its core.
pub struct CoreMailbox<M, R, T> {
    inner: Rc<Inner<M, R, T>>,
}

struct Inner<M, R, T> {
    me: CoreId,
    transport: T,
    capacity: usize,
    in_flight: RefCell<Vec<usize>>,
    next_id: Cell<u64>,
    received: Cell<u32>,
    pending: Rc<PendingTable<R>>,
    queue_full: Vec<Counter>,
    dropped: Counter,
    _message: std::marker::PhantomData<M>,
}

/// Messages a core takes in a row before letting its other tasks run.
const RECV_BATCH: u32 = 64;

const QUEUE_FULL: &str = "ndaal_runtime_queue_full_total";
const REPLIES_DROPPED: &str = "ndaal_runtime_replies_dropped_total";

impl<M: 'static, R: 'static, T: Transport<M, R> + 'static> CoreMailbox<M, R, T> {
    pub(crate) fn new(me: CoreId, transport: T, capacity: usize, registry: &Registry) -> Self {
        let cores = transport.cores();
        let queue_full = (0..cores)
            .map(|to| {
                registry
                    .counter(
                        QUEUE_FULL,
                        "Messages refused because the queue to a core was full.",
                        &[("to", &to.to_string())],
                    )
                    .expect("constant metric name, registered as a counter")
            })
            .collect();
        let dropped = registry
            .counter(
                REPLIES_DROPPED,
                "Requests dropped by their receiver without a reply.",
                &[],
            )
            .expect("constant metric name, registered as a counter");
        Self {
            inner: Rc::new(Inner {
                me,
                transport,
                capacity,
                in_flight: RefCell::new(vec![0; usize::from(cores)]),
                next_id: Cell::new(0),
                received: Cell::new(0),
                pending: Rc::new(PendingTable::default()),
                queue_full,
                dropped,
                _message: std::marker::PhantomData,
            }),
        }
    }

    /// This core.
    pub fn core(&self) -> CoreId {
        self.inner.me
    }

    /// Sends a message expecting no answer; never waits.
    pub fn try_send(&self, to: CoreId, msg: M) -> Result<(), SendError<M>> {
        let inner = &self.inner;
        if inner.transport.is_shut_down() {
            return Err(SendError::ShuttingDown(msg));
        }
        inner
            .transport
            .send(to, Envelope::Message(msg))
            .map_err(|envelope| {
                inner.queue_full[usize::from(to.0)].increment(1);
                SendError::Full(envelope.into_msg())
            })
    }

    /// Sends a request; its reply slot is reserved first, so `Full` also
    /// means "too many requests in flight to that core".
    pub fn request(&self, to: CoreId, msg: M) -> Result<Pending<R>, SendError<M>> {
        let inner = &self.inner;
        if inner.transport.is_shut_down() {
            return Err(SendError::ShuttingDown(msg));
        }
        let slot = usize::from(to.0);
        if inner.in_flight.borrow()[slot] >= inner.capacity {
            inner.queue_full[slot].increment(1);
            return Err(SendError::Full(msg));
        }
        let id = inner.next_id.get();
        match inner.transport.send(to, Envelope::Request(id, msg)) {
            Ok(()) => {
                inner.next_id.set(id + 1);
                inner.in_flight.borrow_mut()[slot] += 1;
                Ok(Pending::new(Rc::clone(&inner.pending), id))
            }
            Err(envelope) => {
                inner.queue_full[slot].increment(1);
                Err(SendError::Full(envelope.into_msg()))
            }
        }
    }

    /// The next message or request; `None` once the runtime stops and the
    /// queues are empty. One task per core calls it.
    pub async fn recv(&self) -> Option<Incoming<M, R>> {
        let inner = &self.inner;
        // A steady flood would let `recv` return at once forever, and the
        // core's other tasks (its reply pump first) would never run.
        let received = inner.received.get().wrapping_add(1);
        inner.received.set(received);
        if received.is_multiple_of(RECV_BATCH) {
            tokio::task::yield_now().await;
        }
        loop {
            if let Some((from, envelope)) = inner.transport.take_message() {
                return Some(match envelope {
                    Envelope::Message(msg) => Incoming::Message { from, msg },
                    Envelope::Request(id, msg) => Incoming::Request {
                        from,
                        msg,
                        replier: Replier::new(
                            Rc::clone(&self.inner) as Rc<dyn ReplySink<R>>,
                            from,
                            id,
                        ),
                    },
                });
            }
            if inner.transport.is_shut_down() {
                return None;
            }
            inner.transport.wait_messages().await;
        }
    }

    /// Takes the replies and completes their requests; returns once the
    /// runtime stops, after failing the requests still waiting. One task
    /// per core runs it.
    pub async fn pump(&self) {
        let inner = &self.inner;
        loop {
            while let Some((from, reply)) = inner.transport.take_reply() {
                inner.in_flight.borrow_mut()[usize::from(from.0)] -= 1;
                inner.pending.complete(reply.id, reply.result);
            }
            if inner.transport.is_shut_down() {
                inner.pending.fail_all(ReplyError::ShuttingDown);
                return;
            }
            inner.transport.wait_replies().await;
        }
    }

    /// Stops the whole runtime: sends are refused, waiting requests fail,
    /// `recv` returns `None` once the queues are empty.
    pub fn shutdown(&self) {
        self.inner.transport.shutdown_all();
    }
}

impl<M, R, T: Transport<M, R>> ReplySink<R> for Inner<M, R, T> {
    fn send_reply(&self, to: CoreId, id: u64, mut result: Result<R, ReplyError>) {
        if matches!(result, Err(ReplyError::Dropped)) {
            if self.transport.is_shut_down() {
                // Abandoned because the runtime stops: not a bug, and the
                // requester sees the same answer as if its pump had failed
                // it first.
                result = Err(ReplyError::ShuttingDown);
            } else {
                self.dropped.increment(1);
            }
        }
        let sent = self.transport.send_reply(to, ReplyWire { id, result });
        // The requester reserved this slot before sending (`request`).
        assert!(sent.is_ok(), "reply queue full despite the reserved slot");
    }
}

impl<M> Envelope<M> {
    fn into_msg(self) -> M {
        match self {
            Envelope::Message(msg) | Envelope::Request(_, msg) => msg,
        }
    }
}
