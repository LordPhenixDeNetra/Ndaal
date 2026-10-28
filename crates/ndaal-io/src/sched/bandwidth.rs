//! Measured bandwidth of a device, and the token bucket of the background
//! class that follows it.

use std::time::Duration;

/// Estimate before the first measurement: a modest SATA SSD, in bytes per
/// second (the spec's starting value, 100 MiB/s).
const START_BANDWIDTH: f64 = 100.0 * 1024.0 * 1024.0;
/// Busy time a measurement window covers.
const WINDOW: Duration = Duration::from_millis(100);
/// Weight of a new window in the moving average: an estimate off by half
/// is within 5 % after 10 windows (0.75^10 * 0.5 < 0.03).
const NEW_WINDOW_WEIGHT: f64 = 0.25;
/// The bucket holds this much time of its rate: short bursts pass, long
/// ones are smoothed.
const BUCKET_SPAN: Duration = Duration::from_millis(100);

/// Bytes served per second of busy device time, as a moving average over
/// windows of [`WINDOW`]. Idle time is not counted: a device that waits for
/// work is not a slow device.
#[derive(Debug)]
pub(super) struct Meter {
    estimate: f64,
    busy_since: Option<Duration>,
    window_busy: Duration,
    window_bytes: u64,
}

impl Meter {
    pub(super) fn new() -> Self {
        Self {
            estimate: START_BANDWIDTH,
            busy_since: None,
            window_busy: Duration::ZERO,
            window_bytes: 0,
        }
    }

    /// Bytes per second.
    pub(super) fn estimate(&self) -> f64 {
        self.estimate
    }

    /// An operation starts; `idle` if none was in flight.
    pub(super) fn start(&mut self, idle: bool, now: Duration) {
        if idle {
            self.busy_since = Some(now);
        }
    }

    /// An operation that moved `bytes` ended; `idle` if none is left.
    pub(super) fn complete(&mut self, bytes: u64, idle: bool, now: Duration) {
        self.window_bytes = self.window_bytes.saturating_add(bytes);
        if let Some(since) = self.busy_since {
            self.window_busy += now.saturating_sub(since);
        }
        self.busy_since = (!idle).then_some(now);
        if self.window_busy >= WINDOW {
            let sample = self.window_bytes as f64 / self.window_busy.as_secs_f64();
            self.estimate += NEW_WINDOW_WEIGHT * (sample - self.estimate);
            self.window_busy = Duration::ZERO;
            self.window_bytes = 0;
        }
    }
}

/// Tokens of the background class, in bytes. It may go below zero: an
/// operation larger than the whole bucket still passes, and the class then
/// waits until the debt is repaid, so the rate holds on average.
#[derive(Debug)]
pub(super) struct Bucket {
    share: f64,
    tokens: f64,
    last: Duration,
}

impl Bucket {
    /// A full bucket.
    pub(super) fn new(share: f64, bandwidth: f64, now: Duration) -> Self {
        let share = share.clamp(0.0, 1.0);
        Self {
            share,
            tokens: share * bandwidth * BUCKET_SPAN.as_secs_f64(),
            last: now,
        }
    }

    fn rate(&self, bandwidth: f64) -> f64 {
        self.share * bandwidth
    }

    /// Adds the tokens earned since the last refill.
    pub(super) fn refill(&mut self, bandwidth: f64, now: Duration) {
        let rate = self.rate(bandwidth);
        let elapsed = now.saturating_sub(self.last).as_secs_f64();
        let capacity = rate * BUCKET_SPAN.as_secs_f64();
        self.tokens = (self.tokens + rate * elapsed).min(capacity);
        self.last = self.last.max(now);
    }

    /// Whether an operation may start now.
    pub(super) fn ready(&self) -> bool {
        self.tokens > 0.0
    }

    /// Charges an operation that starts.
    pub(super) fn take(&mut self, cost: u64) {
        self.tokens -= cost as f64;
    }

    /// When the bucket will be ready again, if it ever will.
    pub(super) fn ready_at(&self, bandwidth: f64, now: Duration) -> Option<Duration> {
        if self.ready() {
            return Some(now);
        }
        let rate = self.rate(bandwidth);
        if rate <= 0.0 {
            return None;
        }
        // One nanosecond past the debt, so the bucket is strictly positive.
        // A wait too long for a `Duration` is as good as never.
        let wait = Duration::try_from_secs_f64(-self.tokens / rate).ok()?;
        let at = self
            .last
            .checked_add(wait)?
            .checked_add(Duration::from_nanos(1))?;
        Some(at.max(now))
    }
}
