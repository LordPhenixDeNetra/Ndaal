//! Thread-per-core runtime: shards, cross-core messaging, pools, admission.

// ARCHITECTURE.md section 3 allows unsafe here; none is needed: pinning
// (core_affinity) and the lock-free queues (rtrb) carry their own.
#![forbid(unsafe_code)]

pub mod layout;
pub mod mailbox;
mod real;

pub use real::{CoreContext, Runtime, RuntimeOptions, ShutdownError, StartError, allowed_cpus};
