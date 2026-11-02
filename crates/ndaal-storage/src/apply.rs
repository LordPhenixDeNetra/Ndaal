//! How a partition hands effects to its engine.

use ndaal_common::ids::Lsn;
use ndaal_wal::RecordType;

/// An engine that applies the effects of log records: key-value, vector
/// and object engines (J03 to J05), and test engines.
///
/// The same call serves a live write, the replay at start-up (T02.18) and
/// Raft (T09.03): what is written and what is replayed cannot diverge. It
/// must be deterministic, and idempotent by version (ADR-007): applying a
/// record whose effects are already there changes nothing.
pub trait Apply {
    /// Applies the effects of the record `lsn` of type `kind`. The caller
    /// validated them before logging: an error here is a bug, and the
    /// partition stops accepting writes.
    fn apply(&mut self, lsn: Lsn, kind: RecordType, payload: &[u8]) -> Result<(), ApplyError>;
}

/// Why an engine could not apply a record. The message is in English and
/// carries no user data (CLAUDE.md section 6).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("cannot apply log record: {0}")]
pub struct ApplyError(pub String);
