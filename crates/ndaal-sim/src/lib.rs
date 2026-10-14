//! Deterministic simulation harness (ADR-017): runs tasks on a virtual clock,
//! a simulated network and simulated disks, all driven by one seed.

#![forbid(unsafe_code)]

mod executor;
mod scenario;
mod sweep;
mod trace;

pub use executor::{Limits, Outcome, Sim, TaskId};
pub use scenario::{Report, ping_pong, with_timeout};
pub use sweep::{Failure, SeedResult, ping_pong_result, sweep, sweep_with};
pub use trace::Fnv;
