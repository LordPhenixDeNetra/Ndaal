//! The I/O scheduler of one device (ARCHITECTURE.md section 4.3, T02.05).
//!
//! A pure state machine: the caller passes the time, so the same logic runs
//! the real thread pool and the simulated device, and the simulation checks
//! it exactly. It only sees descriptors (class, cost, number); the data of
//! each operation stays with its caller.
//!
//! - `Critical` (the log) is always served first.
//! - The other classes share the device by deficit round robin in bytes,
//!   with quanta proportional to 50/25/15/10.
//! - `Background` also draws from a token bucket filled at a share of the
//!   measured bandwidth; it waits without tokens even on an idle device.
//! - At most `depth` operations are in flight.

mod bandwidth;
mod scheduler;

pub use scheduler::Scheduler;

/// Cost of an operation without a size (`sync_data`, `allocate`, renaming):
/// one page, so it still takes its turn.
pub const UNSIZED_COST: u64 = 4096;

/// Priority class of a disk operation, given when a file is opened.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IoClass {
    /// The log and the manifests: always first.
    Critical,
    /// Reads of the short path (`GET`, small objects).
    High,
    /// Vector search and index reads.
    Medium,
    /// Object transfers.
    Low,
    /// Compaction and maintenance, also limited by a token bucket.
    Background,
}

impl IoClass {
    /// Every class, `Critical` first.
    pub const ALL: [IoClass; 5] = [
        IoClass::Critical,
        IoClass::High,
        IoClass::Medium,
        IoClass::Low,
        IoClass::Background,
    ];

    /// Position in [`IoClass::ALL`].
    pub fn index(self) -> usize {
        match self {
            IoClass::Critical => 0,
            IoClass::High => 1,
            IoClass::Medium => 2,
            IoClass::Low => 3,
            IoClass::Background => 4,
        }
    }

    /// The name used in metric labels.
    pub fn name(self) -> &'static str {
        match self {
            IoClass::Critical => "critical",
            IoClass::High => "high",
            IoClass::Medium => "medium",
            IoClass::Low => "low",
            IoClass::Background => "background",
        }
    }
}

/// Number of an operation, unique per scheduler (chosen by the caller).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OpId(pub u64);

/// What the scheduler knows of an operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Op {
    /// Its number.
    pub id: OpId,
    /// Its class.
    pub class: IoClass,
    /// Bytes it moves; at least [`UNSIZED_COST`] is charged.
    pub cost: u64,
}

/// Settings of one device's scheduler (from `[io]`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SchedulerConfig {
    /// Operations in flight at most (`io.queue_depth`), at least 1.
    pub depth: usize,
    /// Share of the measured bandwidth left to `Background`, from 0 to 1
    /// (`io.background_bandwidth_share`). At 0 the class is paused.
    pub background_share: f64,
}
