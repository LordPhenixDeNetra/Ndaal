//! System clock.

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::Clock;

/// Monotonic time from [`Instant`], wall-clock time from the system.
#[derive(Debug, Clone, Copy)]
pub struct SystemClock {
    origin: Instant,
}

impl SystemClock {
    /// A clock whose monotonic origin is now.
    pub fn new() -> Self {
        Self {
            origin: Instant::now(),
        }
    }
}

impl Default for SystemClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for SystemClock {
    fn monotonic(&self) -> Duration {
        self.origin.elapsed()
    }

    fn unix_micros(&self) -> u64 {
        // A clock set before 1970 reads as the epoch; u64 microseconds last 584,000 years.
        let since_epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        u64::try_from(since_epoch.as_micros()).unwrap_or(u64::MAX)
    }
}
