//! Errors of a partition.

use ndaal_observe::metrics::MetricsError;
use ndaal_wal::WalError;

/// Why a partition refused or failed an operation.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PartitionError {
    /// The shard's log has too much pending: nothing changed, retry later
    /// (P8).
    #[error("partition busy: the log has too much pending")]
    Busy,
    /// The engine failed to apply a logged record, the log failed, or the
    /// LSNs ran out: the partition takes no more writes until the node
    /// restarts from its log. The outcome of the write that hit it is
    /// unknown: its record may be in the log, and the restart replays it.
    #[error("partition failed: it takes no more writes")]
    Failed,
    /// A wait went past its deadline.
    #[error("waiting for the partition timed out")]
    Timeout,
    /// The partition's metrics could not be registered.
    #[error("cannot register the partition metrics")]
    Metrics(#[source] MetricsError),
    /// The shard's log refused the record.
    #[error("partition log error: {0}")]
    Wal(#[source] WalError),
}
