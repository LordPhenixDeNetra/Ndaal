//! Deterministic simulation harness (ADR-017): runs tasks on a virtual clock,
//! a simulated network and simulated disks, all driven by one seed.

#![forbid(unsafe_code)]

mod cross_core;
mod executor;
mod io_priority;
mod scenario;
mod sweep;
mod trace;
mod wal_crash;

pub use cross_core::{CrossCoreReport, cross_core, cross_core_result};
pub use executor::{Limits, Outcome, Sim, TaskId};
pub use io_priority::{IoPriorityReport, high_p99_bound, io_priority, io_priority_result};
pub use scenario::{Report, ping_pong, with_timeout};
pub use sweep::{Failure, SeedResult, combine, ping_pong_result, sweep, sweep_with};
pub use trace::Fnv;
pub use wal_crash::{Control, WalCrashReport, wal_crash, wal_crash_result, wal_crash_with};
