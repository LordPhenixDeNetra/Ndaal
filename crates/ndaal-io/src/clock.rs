//! Time source.

use std::time::Duration;

/// Monotonic and wall-clock time.
pub trait Clock {
    /// Monotonic time since an arbitrary origin; never goes backwards.
    fn monotonic(&self) -> Duration;

    /// Wall-clock time, microseconds since the Unix epoch (FORMATS_DE_DONNEES.md section 1).
    fn unix_micros(&self) -> u64;
}
