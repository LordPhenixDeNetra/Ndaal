//! Virtual clock: time moves only when the simulator advances it.

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use crate::Clock;

/// A clock shared by every clone; [`SimClock::advance`] moves all of them.
#[derive(Debug, Clone)]
pub struct SimClock {
    elapsed: Rc<Cell<Duration>>,
    start_unix_micros: u64,
}

impl SimClock {
    /// A clock at monotonic zero whose wall clock reads `start_unix_micros`.
    pub fn new(start_unix_micros: u64) -> Self {
        Self {
            elapsed: Rc::new(Cell::new(Duration::ZERO)),
            start_unix_micros,
        }
    }

    /// Moves virtual time forward.
    pub fn advance(&self, by: Duration) {
        self.elapsed.set(self.elapsed.get() + by);
    }
}

impl Clock for SimClock {
    fn monotonic(&self) -> Duration {
        self.elapsed.get()
    }

    fn unix_micros(&self) -> u64 {
        let elapsed = u64::try_from(self.elapsed.get().as_micros()).unwrap_or(u64::MAX);
        self.start_unix_micros.saturating_add(elapsed)
    }
}
