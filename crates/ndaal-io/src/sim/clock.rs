//! Virtual clock: time moves only when the simulator advances it.
//!
//! Timers live here, not in the scheduler: `sleep` and delayed network
//! deliveries register a waker with [`SimClock::wake_at`], and the scheduler
//! (`ndaal-sim`) jumps to [`SimClock::next_deadline`] when no task is ready.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll, Waker};
use std::time::Duration;

use crate::Clock;

/// A clock shared by every clone; advancing one advances all of them.
#[derive(Debug, Clone)]
pub struct SimClock {
    inner: Rc<RefCell<Inner>>,
    start_unix_micros: u64,
}

#[derive(Debug, Default)]
struct Inner {
    now: Duration,
    /// Keyed by (deadline, registration number): equal deadlines fire in
    /// registration order, deterministically.
    timers: BTreeMap<(Duration, u64), Waker>,
    next_timer: u64,
}

impl SimClock {
    /// A clock at monotonic zero whose wall clock reads `start_unix_micros`.
    pub fn new(start_unix_micros: u64) -> Self {
        Self {
            inner: Rc::new(RefCell::new(Inner::default())),
            start_unix_micros,
        }
    }

    /// Moves virtual time forward by `by`.
    pub fn advance(&self, by: Duration) {
        let now = self.inner.borrow().now;
        self.advance_to(now + by);
    }

    /// Moves virtual time to `t` (never backwards) and wakes the timers due.
    pub fn advance_to(&self, t: Duration) {
        let due = {
            let mut inner = self.inner.borrow_mut();
            if t <= inner.now {
                return;
            }
            inner.now = t;
            let later = inner.timers.split_off(&(t, u64::MAX));
            std::mem::replace(&mut inner.timers, later)
        };
        // Wake outside the borrow: a waker may re-enter the clock.
        for waker in due.into_values() {
            waker.wake();
        }
    }

    /// Wakes `waker` once virtual time reaches `deadline` (at once if it has).
    pub fn wake_at(&self, deadline: Duration, waker: Waker) {
        let mut inner = self.inner.borrow_mut();
        if deadline <= inner.now {
            drop(inner);
            waker.wake();
            return;
        }
        let key = (deadline, inner.next_timer);
        inner.next_timer += 1;
        inner.timers.insert(key, waker);
    }

    /// The earliest pending timer, if any.
    pub fn next_deadline(&self) -> Option<Duration> {
        self.inner
            .borrow()
            .timers
            .keys()
            .next()
            .map(|(deadline, _)| *deadline)
    }
}

impl Clock for SimClock {
    fn monotonic(&self) -> Duration {
        self.inner.borrow().now
    }

    fn unix_micros(&self) -> u64 {
        let elapsed = u64::try_from(self.monotonic().as_micros()).unwrap_or(u64::MAX);
        self.start_unix_micros.saturating_add(elapsed)
    }

    async fn sleep(&self, duration: Duration) {
        let deadline = self.monotonic().saturating_add(duration);
        Sleep {
            clock: self.clone(),
            deadline,
            registered: false,
        }
        .await;
    }
}

/// Ready once virtual time reaches `deadline`.
struct Sleep {
    clock: SimClock,
    deadline: Duration,
    registered: bool,
}

impl Future for Sleep {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        if self.clock.monotonic() >= self.deadline {
            return Poll::Ready(());
        }
        if !self.registered {
            self.registered = true;
            self.clock.wake_at(self.deadline, cx.waker().clone());
        }
        Poll::Pending
    }
}
