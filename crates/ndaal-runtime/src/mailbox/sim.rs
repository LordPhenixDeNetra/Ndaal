//! Simulated transport (ADR-017): every core runs in the single thread of
//! `ndaal-sim`; queues have the real capacity, and each message gets a
//! delivery time drawn from the seed, FIFO per pair of cores.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;
use std::time::Duration;

use ndaal_io::sim::SimClock;
use ndaal_io::{Clock, Rng, Xoshiro256PlusPlus};
use ndaal_observe::metrics::Registry;
use tokio::sync::Notify;

use super::{CoreId, CoreMailbox, Envelope, ReplyWire, Transport};

/// Longest delivery delay drawn per message, in virtual time.
const SIM_MAX_DELIVERY_DELAY_US: u64 = 50;

/// The messaging endpoint of a simulated core.
pub type SimMailbox<M, R> = CoreMailbox<M, R, SimTransport<M, R>>;

/// One queue per (sender, receiver) pair, each item with its delivery
/// time. Only heads are delivered, so order per pair is FIFO even when a
/// later item draws an earlier time.
struct Queues<T> {
    items: Vec<VecDeque<(Duration, T)>>,
}

impl<T> Queues<T> {
    fn new(pairs: usize) -> Self {
        Self {
            items: (0..pairs).map(|_| VecDeque::new()).collect(),
        }
    }
}

struct Net<M, R> {
    cores: u16,
    capacity: usize,
    clock: SimClock,
    rng: RefCell<Xoshiro256PlusPlus>,
    messages: RefCell<Queues<Envelope<M>>>,
    replies: RefCell<Queues<ReplyWire<R>>>,
    message_arrived: Vec<Notify>,
    reply_arrived: Vec<Notify>,
    shutdown: Cell<bool>,
}

impl<M, R> Net<M, R> {
    fn pair(&self, from: CoreId, to: CoreId) -> usize {
        usize::from(from.0) * usize::from(self.cores) + usize::from(to.0)
    }

    fn delay(&self) -> Duration {
        let draw = self.rng.borrow_mut().next_u64() % (SIM_MAX_DELIVERY_DELAY_US + 1);
        Duration::from_micros(draw)
    }

    /// Pushes `item` from `from` to `to`; `Err` when the queue is full.
    fn push<T>(
        &self,
        queues: &RefCell<Queues<T>>,
        from: CoreId,
        to: CoreId,
        item: T,
    ) -> Result<(), T> {
        let pair = self.pair(from, to);
        let mut q = queues.borrow_mut();
        if q.items[pair].len() >= self.capacity {
            return Err(item);
        }
        let due = self.clock.monotonic() + self.delay();
        q.items[pair].push_back((due, item));
        Ok(())
    }

    /// Earliest head addressed to `to`: (delivery time, sender), ties by
    /// sender, so the order depends on the seed only.
    fn earliest<T>(&self, queues: &RefCell<Queues<T>>, to: CoreId) -> Option<(Duration, u16)> {
        let q = queues.borrow();
        (0..self.cores)
            .filter_map(|from| {
                let head = q.items[self.pair(CoreId(from), to)].front()?;
                Some((head.0, from))
            })
            .min()
    }

    fn take<T>(&self, queues: &RefCell<Queues<T>>, to: CoreId) -> Option<(CoreId, T)> {
        let (due, from) = self.earliest(queues, to)?;
        if due > self.clock.monotonic() {
            return None;
        }
        let pair = self.pair(CoreId(from), to);
        let (_, item) = queues.borrow_mut().items[pair].pop_front()?;
        Some((CoreId(from), item))
    }

    /// Completes when an item for `to` is due, or on shutdown.
    async fn wait<T>(&self, queues: &RefCell<Queues<T>>, arrived: &Notify, to: CoreId) {
        let notified = arrived.notified();
        tokio::pin!(notified);
        notified.as_mut().enable();
        if self.shutdown.get() {
            return;
        }
        match self.earliest(queues, to) {
            None => notified.await,
            Some((due, _)) => {
                let now = self.clock.monotonic();
                if due <= now {
                    return;
                }
                tokio::select! {
                    () = self.clock.sleep(due - now) => {}
                    () = notified => {}
                }
            }
        }
    }
}

/// The simulated transport of one core.
pub struct SimTransport<M, R> {
    me: CoreId,
    net: Rc<Net<M, R>>,
}

impl<M, R> Transport<M, R> for SimTransport<M, R> {
    fn send(&self, to: CoreId, envelope: Envelope<M>) -> Result<(), Envelope<M>> {
        let net = &self.net;
        net.push(&net.messages, self.me, to, envelope)?;
        net.message_arrived[usize::from(to.0)].notify_waiters();
        Ok(())
    }

    fn send_reply(&self, to: CoreId, reply: ReplyWire<R>) -> Result<(), ReplyWire<R>> {
        let net = &self.net;
        net.push(&net.replies, self.me, to, reply)?;
        net.reply_arrived[usize::from(to.0)].notify_waiters();
        Ok(())
    }

    fn take_message(&self) -> Option<(CoreId, Envelope<M>)> {
        self.net.take(&self.net.messages, self.me)
    }

    fn take_reply(&self) -> Option<(CoreId, ReplyWire<R>)> {
        self.net.take(&self.net.replies, self.me)
    }

    async fn wait_messages(&self) {
        let net = &self.net;
        net.wait(
            &net.messages,
            &net.message_arrived[usize::from(self.me.0)],
            self.me,
        )
        .await;
    }

    async fn wait_replies(&self) {
        let net = &self.net;
        net.wait(
            &net.replies,
            &net.reply_arrived[usize::from(self.me.0)],
            self.me,
        )
        .await;
    }

    fn shutdown_all(&self) {
        self.net.shutdown.set(true);
        for notify in self
            .net
            .message_arrived
            .iter()
            .chain(&self.net.reply_arrived)
        {
            notify.notify_waiters();
        }
    }

    fn is_shut_down(&self) -> bool {
        self.net.shutdown.get()
    }

    fn cores(&self) -> u16 {
        self.net.cores
    }
}

/// Mailboxes of `cores` simulated cores, queues of `capacity` items. Each
/// core needs a task calling `recv` and a task running `pump`, spawned on
/// the simulation.
pub fn sim_mailboxes<M: 'static, R: 'static>(
    cores: u16,
    capacity: usize,
    clock: SimClock,
    rng: Xoshiro256PlusPlus,
    registry: &Registry,
) -> Vec<SimMailbox<M, R>> {
    let pairs = usize::from(cores) * usize::from(cores);
    let net = Rc::new(Net {
        cores,
        capacity,
        clock,
        rng: RefCell::new(rng),
        messages: RefCell::new(Queues::new(pairs)),
        replies: RefCell::new(Queues::new(pairs)),
        message_arrived: (0..cores).map(|_| Notify::new()).collect(),
        reply_arrived: (0..cores).map(|_| Notify::new()).collect(),
        shutdown: Cell::new(false),
    });
    (0..cores)
        .map(|me| {
            let transport = SimTransport {
                me: CoreId(me),
                net: Rc::clone(&net),
            };
            CoreMailbox::new(CoreId(me), transport, capacity, registry)
        })
        .collect()
}
