//! A burst of low-priority writes must not delay high-priority reads past a
//! bound (T02.05): writers keep a 200 MiB/s device busy with 1 MiB writes
//! for one second while a 4 KiB high read starts every millisecond.
//!
//! The bound is what the scheduler guarantees: a read waits at most for the
//! operations already in the device (`depth`), each served in full. With
//! arrival order ([`Policy::Fifo`]) the read also waits for every queued
//! write, and the same scenario breaks the bound: the test discriminates.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use ndaal_io::Clock;
use ndaal_io::sched::{IoClass, SchedulerConfig};
use ndaal_io::sim::{DeviceModel, Policy, SimDevice};

use crate::sweep::SeedResult;
use crate::{Limits, Outcome, Sim};

const MIB: u64 = 1024 * 1024;
const MODEL: DeviceModel = DeviceModel {
    latency: Duration::from_micros(100),
    bytes_per_second: 200 * MIB,
};
/// Operations in the device at once.
const DEPTH: usize = 4;
/// Size of a low write.
const WRITE: u64 = MIB;
/// Size of a high read.
const READ: u64 = 4096;
/// Tasks writing at once: four times the depth, so writes always queue.
const WRITERS: u32 = 16;
/// How long the writers keep writing.
const BURST: Duration = Duration::from_secs(1);
/// One high read starts every millisecond during the burst.
const READ_EVERY: Duration = Duration::from_millis(1);
const READS: u32 = 1000;
/// Slack for the read's own service and the dispatch around it.
const MARGIN: Duration = Duration::from_millis(2);
const LIMITS: Limits = Limits {
    max_steps: 1_000_000,
    max_virtual_time: Duration::from_secs(10),
};

/// The bound on the 99th percentile of high-read latency: `depth` writes
/// served in full at their slowest, plus [`MARGIN`].
pub fn high_p99_bound() -> Duration {
    MODEL.max_service(WRITE) * u32::try_from(DEPTH).unwrap_or(u32::MAX) + MARGIN
}

/// What happened during one run of [`io_priority`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IoPriorityReport {
    /// How the run ended.
    pub outcome: Outcome,
    /// 99th percentile of the high reads' latency.
    pub high_p99: Duration,
    /// High reads completed.
    pub high_reads: u32,
    /// Trace fingerprint.
    pub fingerprint: u64,
}

/// Runs the scenario on a device ordered by `policy`.
pub fn io_priority(seed: u64, policy: Policy) -> IoPriorityReport {
    let sim = Sim::new(seed);
    let config = SchedulerConfig {
        depth: DEPTH,
        background_share: 0.3,
    };
    let device = SimDevice::new(sim.clock(), sim.rng("io_device"), MODEL, config, policy);
    for _ in 0..WRITERS {
        let (device, clock) = (device.clone(), sim.clock());
        sim.spawn(async move {
            while clock.monotonic() < BURST {
                device.run(IoClass::Low, WRITE, async {}).await;
            }
        });
    }
    let latencies = Rc::new(RefCell::new(Vec::new()));
    let (s, d, l) = (sim.clone(), device.clone(), Rc::clone(&latencies));
    sim.spawn(async move {
        let clock = s.clock();
        for _ in 0..READS {
            let (s2, d, l) = (s.clone(), d.clone(), Rc::clone(&l));
            s.spawn(async move {
                let clock = s2.clock();
                let start = clock.monotonic();
                d.run(IoClass::High, READ, async {}).await;
                let latency = clock.monotonic() - start;
                s2.trace(format!("read {}us", latency.as_micros()));
                l.borrow_mut().push(latency);
            });
            clock.sleep(READ_EVERY).await;
        }
    });
    let outcome = sim.run(LIMITS);
    let mut latencies = latencies.take();
    latencies.sort();
    IoPriorityReport {
        outcome,
        high_p99: percentile_99(&latencies),
        high_reads: u32::try_from(latencies.len()).unwrap_or(u32::MAX),
        fingerprint: sim.fingerprint(),
    }
}

/// The 99th percentile of sorted values (zero if there are none).
fn percentile_99(sorted: &[Duration]) -> Duration {
    let rank = (sorted.len() * 99).div_ceil(100);
    sorted
        .get(rank.saturating_sub(1))
        .copied()
        .unwrap_or_default()
}

/// The nightly form: the scheduler keeps every read count and the p99
/// under [`high_p99_bound`].
pub fn io_priority_result(seed: u64) -> SeedResult {
    let report = io_priority(seed, Policy::Priority);
    let bound = high_p99_bound();
    let finished = report.outcome == Outcome::Finished
        && report.high_reads == READS
        && report.high_p99 <= bound;
    let detail = if finished {
        String::new()
    } else {
        format!(
            "outcome {:?}, {}/{READS} reads, p99 {:?} for a bound of {bound:?}",
            report.outcome, report.high_reads, report.high_p99
        )
    };
    SeedResult {
        finished,
        fingerprint: report.fingerprint,
        detail,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn high_reads_stay_under_the_bound() {
        for seed in 0..20 {
            let result = io_priority_result(seed);
            assert!(result.finished, "seed {seed}: {}", result.detail);
        }
    }

    #[test]
    fn fifo_breaks_the_bound() {
        let report = io_priority(3, Policy::Fifo);
        assert_eq!(report.outcome, Outcome::Finished);
        assert!(
            report.high_p99 > high_p99_bound(),
            "fifo p99 {:?} is under the bound {:?}",
            report.high_p99,
            high_p99_bound()
        );
    }

    #[test]
    fn io_priority_is_reproducible() {
        assert_eq!(
            io_priority(7, Policy::Priority),
            io_priority(7, Policy::Priority)
        );
        assert_ne!(
            io_priority(7, Policy::Priority).fingerprint,
            io_priority(8, Policy::Priority).fingerprint
        );
    }

    #[test]
    fn idle_device_lets_the_sim_finish() {
        let sim = Sim::new(1);
        let config = SchedulerConfig {
            depth: DEPTH,
            background_share: 0.3,
        };
        let device = SimDevice::new(sim.clock(), sim.rng("d"), MODEL, config, Policy::Priority);
        let d = device.clone();
        sim.spawn(async move {
            d.run(IoClass::Background, WRITE, async {}).await;
        });
        assert_eq!(sim.run(LIMITS), Outcome::Finished);
        // Idle again: nothing is left to wake.
        assert_eq!(sim.clock().next_deadline(), None);
    }
}
