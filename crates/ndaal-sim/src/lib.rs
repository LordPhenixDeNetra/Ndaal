//! Deterministic simulation harness (ADR-017): runs tasks on a virtual clock,
//! a simulated network and simulated disks, all driven by one seed.

#![forbid(unsafe_code)]

mod cross_core;
mod executor;
mod scenario;
mod sweep;
mod trace;

pub use cross_core::{CrossCoreReport, cross_core, cross_core_result};
pub use executor::{Limits, Outcome, Sim, TaskId};
pub use scenario::{Report, ping_pong, with_timeout};
pub use sweep::{Failure, SeedResult, combine, ping_pong_result, sweep, sweep_with};
pub use trace::Fnv;
