//! Metrics of a partition (EXPLOITATION.md section 8).

use ndaal_common::ids::Lsn;
use ndaal_observe::metrics::{Counter, Gauge, MetricsError, Registry};

pub(crate) struct Metrics {
    applied: Gauge,
    durable: Gauge,
    pub failed: Counter,
}

fn gauge_value(lsn: Lsn) -> i64 {
    i64::try_from(lsn.0).unwrap_or(i64::MAX)
}

impl Metrics {
    pub fn new(partition: u32, registry: &Registry) -> Result<Metrics, MetricsError> {
        let partition = partition.to_string();
        let labels = [("partition", partition.as_str())];
        Ok(Metrics {
            applied: registry.gauge(
                "ndaal_partition_applied_lsn",
                "Last LSN applied to the partition's engine.",
                &labels,
            )?,
            durable: registry.gauge(
                "ndaal_wal_durable_lsn",
                "Last LSN of the partition durable in the log.",
                &labels,
            )?,
            failed: registry.counter(
                "ndaal_partition_failed_total",
                "Partitions that stopped taking writes after an apply or log failure.",
                &labels,
            )?,
        })
    }

    pub fn applied(&self, lsn: Lsn) {
        self.applied.set(gauge_value(lsn));
    }

    pub fn durable(&self, lsn: Lsn) {
        self.durable.set(gauge_value(lsn));
    }
}
