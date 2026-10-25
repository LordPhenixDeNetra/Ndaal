//! Requests waiting for their reply, and the answering side.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll, Waker};

use super::{CoreId, ReplyError};

/// Sends a reply back to the requesting core.
pub(crate) trait ReplySink<R> {
    fn send_reply(&self, to: CoreId, id: u64, result: Result<R, ReplyError>);
}

/// One waiting request: its answer once arrived, and the task to wake.
struct Slot<R> {
    result: Option<Result<R, ReplyError>>,
    waker: Option<Waker>,
}

/// Requests of one core waiting for their reply, by request number.
/// Bounded: at most `capacity` requests in flight per destination.
pub(crate) struct PendingTable<R> {
    slots: RefCell<BTreeMap<u64, Slot<R>>>,
}

impl<R> Default for PendingTable<R> {
    fn default() -> Self {
        Self {
            slots: RefCell::new(BTreeMap::new()),
        }
    }
}

impl<R> PendingTable<R> {
    /// Records the answer of request `id`; ignored if it was dropped.
    pub(crate) fn complete(&self, id: u64, result: Result<R, ReplyError>) {
        let waker = match self.slots.borrow_mut().get_mut(&id) {
            Some(slot) => {
                slot.result = Some(result);
                slot.waker.take()
            }
            None => None,
        };
        // Woken outside the borrow: the task may poll at once.
        if let Some(waker) = waker {
            waker.wake();
        }
    }

    /// Fails every request still waiting.
    pub(crate) fn fail_all(&self, error: ReplyError) {
        let wakers: Vec<Waker> = self
            .slots
            .borrow_mut()
            .values_mut()
            .filter(|slot| slot.result.is_none())
            .filter_map(|slot| {
                slot.result = Some(Err(error));
                slot.waker.take()
            })
            .collect();
        for waker in wakers {
            waker.wake();
        }
    }
}

/// A request waiting for its reply.
pub struct Pending<R> {
    table: Rc<PendingTable<R>>,
    id: u64,
}

impl<R> Pending<R> {
    pub(crate) fn new(table: Rc<PendingTable<R>>, id: u64) -> Self {
        table.slots.borrow_mut().insert(
            id,
            Slot {
                result: None,
                waker: None,
            },
        );
        Self { table, id }
    }
}

impl<R> Future for Pending<R> {
    type Output = Result<R, ReplyError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut slots = self.table.slots.borrow_mut();
        let Some(slot) = slots.get_mut(&self.id) else {
            // Removed only by `Drop`: a pending request polled after it.
            return Poll::Ready(Err(ReplyError::Dropped));
        };
        match slot.result.take() {
            Some(result) => {
                slots.remove(&self.id);
                Poll::Ready(result)
            }
            None => {
                slot.waker = Some(cx.waker().clone());
                Poll::Pending
            }
        }
    }
}

impl<R> Drop for Pending<R> {
    fn drop(&mut self) {
        // The reply may still come: its slot in the queue stays reserved
        // until then, and `complete` ignores it.
        self.table.slots.borrow_mut().remove(&self.id);
    }
}

/// Answers one request, once. Dropping it without replying sends
/// [`ReplyError::Dropped`], so the requester never waits forever.
pub struct Replier<R> {
    sink: Rc<dyn ReplySink<R>>,
    to: CoreId,
    id: u64,
    done: bool,
}

impl<R> Replier<R> {
    pub(crate) fn new(sink: Rc<dyn ReplySink<R>>, to: CoreId, id: u64) -> Self {
        Self {
            sink,
            to,
            id,
            done: false,
        }
    }

    /// Sends the answer; it always fits (the slot was reserved).
    pub fn reply(mut self, value: R) {
        self.done = true;
        self.sink.send_reply(self.to, self.id, Ok(value));
    }
}

impl<R> std::fmt::Debug for Replier<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Replier")
            .field("to", &self.to)
            .field("id", &self.id)
            .finish()
    }
}

impl<R> Drop for Replier<R> {
    fn drop(&mut self) {
        if !self.done {
            self.sink
                .send_reply(self.to, self.id, Err(ReplyError::Dropped));
        }
    }
}
