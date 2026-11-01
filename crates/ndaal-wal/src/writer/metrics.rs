//! Metrics `ndaal_wal_*` of a shard's writer (EXPLOITATION.md section 8).

use std::time::Duration;

use ndaal_observe::metrics::{Counter, Gauge, Histogram, MetricsError, Registry};

/// Upper bounds of the sync latency buckets, in seconds: from 50 us (an
/// NVMe with power-loss protection) to 1 s (a device in trouble).
const SYNC_BUCKETS: [f64; 10] = [
    0.000_05, 0.000_1, 0.000_25, 0.000_5, 0.001, 0.002_5, 0.005, 0.01, 0.1, 1.0,
];
/// Upper bounds of the flush size buckets, in bytes: one page to 2 MiB.
const FLUSH_BUCKETS: [f64; 7] = [
    4096.0,
    16384.0,
    65536.0,
    262_144.0,
    524_288.0,
    1_048_576.0,
    2_097_152.0,
];

pub(super) struct Metrics {
    sync_seconds: Histogram,
    flush_bytes: Histogram,
    durable: Gauge,
    pub busy: Counter,
    errors: Counter,
    shard: u16,
}

impl Metrics {
    pub fn new(shard: u16, registry: &Registry) -> Result<Metrics, MetricsError> {
        let shard_no = shard;
        let shard = shard.to_string();
        let labels = [("shard", shard.as_str())];
        Ok(Metrics {
            sync_seconds: registry.histogram(
                "ndaal_wal_fsync_seconds",
                "Time of a log flush, write and sync, in seconds.",
                &labels,
                &SYNC_BUCKETS,
            )?,
            flush_bytes: registry.histogram(
                "ndaal_wal_flush_bytes",
                "Bytes written by a log flush.",
                &labels,
                &FLUSH_BUCKETS,
            )?,
            durable: registry.gauge(
                "ndaal_wal_durable_position_bytes",
                "Offset of the durable end in the current log file.",
                &labels,
            )?,
            busy: registry.counter(
                "ndaal_wal_append_busy_total",
                "Appends refused because too much was pending.",
                &labels,
            )?,
            errors: registry.counter(
                "ndaal_wal_flush_errors_total",
                "Log flushes or file preparations that failed; the log then fails.",
                &labels,
            )?,
            shard: shard_no,
        })
    }

    pub fn flushed(&self, bytes: usize, took: Duration) {
        self.flush_bytes.observe(bytes as f64);
        self.sync_seconds.observe(took.as_secs_f64());
    }

    /// An I/O error left the log failed: logged once and counted, since the
    /// waiters only see `Failed`.
    pub fn failed(&self, what: &str, kind: std::io::ErrorKind) {
        self.errors.increment(1);
        tracing::error!(shard = self.shard, error = %kind, "log {what} failed; the log takes no more records");
    }

    pub fn durable(&self, offset: u64) {
        self.durable.set(i64::try_from(offset).unwrap_or(i64::MAX));
    }
}
