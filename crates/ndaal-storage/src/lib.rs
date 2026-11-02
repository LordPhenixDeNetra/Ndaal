//! Partitions, manifest, segments, compaction, checkpoints.
//!
//! T02.12: [`Partition`] assembles the shard's log and an engine: it gives
//! LSNs, logs, applies effects through [`Apply`], and tracks its applied
//! and durable LSNs.

#![forbid(unsafe_code)]

mod apply;
mod error;
mod metrics;
mod partition;
mod wait;

pub use apply::{Apply, ApplyError};
pub use error::PartitionError;
pub use partition::Partition;
