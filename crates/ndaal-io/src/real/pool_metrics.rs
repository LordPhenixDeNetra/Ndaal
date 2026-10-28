//! Metrics `ndaal_io_*` of a device's pool (spec section 3.7,
//! EXPLOITATION.md section 8). Updated by the dispatcher thread only.

use std::io;
use std::time::Duration;

use ndaal_observe::metrics::{Counter, Gauge, Histogram, Registry};

use crate::sched::IoClass;

/// Upper bounds of the latency buckets, in seconds: from 100 us (a fast
/// NVMe read) to 1 s (a device in trouble).
const LATENCY_BUCKETS: [f64; 13] = [
    0.000_1, 0.000_25, 0.000_5, 0.001, 0.002_5, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0,
];

/// What an operation does, for the `op` label of `ndaal_io_ops_total`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IoKind {
    /// Reads data.
    Read,
    /// Writes data.
    Write,
    /// Makes data or names durable (`sync_data`, `sync_dir`).
    Sync,
    /// Anything else: open, rename, remove, list, length, allocate.
    Meta,
}

impl IoKind {
    const ALL: [IoKind; 4] = [IoKind::Read, IoKind::Write, IoKind::Sync, IoKind::Meta];

    fn name(self) -> &'static str {
        match self {
            IoKind::Read => "read",
            IoKind::Write => "write",
            IoKind::Sync => "sync",
            IoKind::Meta => "meta",
        }
    }
}

pub(super) struct PoolMetrics {
    pub(super) inflight: Gauge,
    pub(super) bandwidth: Gauge,
    pub(super) throttled: Counter,
    /// By class, then by kind.
    ops: Vec<Vec<Counter>>,
    /// By class: from submission to completion.
    latency: Vec<Histogram>,
}

fn other(error: impl std::fmt::Display) -> io::Error {
    io::Error::other(format!("cannot register i/o metrics: {error}"))
}

impl PoolMetrics {
    pub(super) fn new(device: &str, registry: &Registry) -> io::Result<Self> {
        let dev = [("device", device)];
        let mut ops = Vec::new();
        let mut latency = Vec::new();
        for class in IoClass::ALL {
            let mut by_kind = Vec::new();
            for kind in IoKind::ALL {
                let labels = [
                    ("device", device),
                    ("class", class.name()),
                    ("op", kind.name()),
                ];
                let help = "Disk operations completed.";
                by_kind.push(
                    registry
                        .counter("ndaal_io_ops_total", help, &labels)
                        .map_err(other)?,
                );
            }
            ops.push(by_kind);
            let labels = [("device", device), ("class", class.name())];
            let help = "Time from submission to completion of a disk operation, in seconds.";
            latency.push(
                registry
                    .histogram("ndaal_io_latency_seconds", help, &labels, &LATENCY_BUCKETS)
                    .map_err(other)?,
            );
        }
        Ok(Self {
            inflight: registry
                .gauge("ndaal_io_inflight", "Disk operations in flight.", &dev)
                .map_err(other)?,
            bandwidth: registry
                .gauge(
                    "ndaal_io_bandwidth_bytes_per_second",
                    "Measured bandwidth of the device.",
                    &dev,
                )
                .map_err(other)?,
            throttled: registry
                .counter(
                    "ndaal_io_throttled_total",
                    "Waits of the background class for tokens.",
                    &dev,
                )
                .map_err(other)?,
            ops,
            latency,
        })
    }

    pub(super) fn record(&self, class: IoClass, kind: IoKind, latency: Duration) {
        let class = class.index();
        let kind = IoKind::ALL.iter().position(|k| *k == kind).unwrap_or(0);
        self.ops[class][kind].increment(1);
        self.latency[class].observe(latency.as_secs_f64());
    }
}
