//! Errors of the log format.

/// Why log bytes cannot be read. Never a panic: these bytes come from disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum WalError {
    /// The file is shorter than its header.
    #[error("log file shorter than its header")]
    ShortHeader,
    /// The file does not start with the log magic.
    #[error("not a log file: bad magic")]
    BadMagic,
    /// The header checksum does not match.
    #[error("log file header checksum mismatch")]
    BadHeaderCrc,
    /// A format version this build cannot read.
    #[error("unknown log format version {0}")]
    UnknownFormat(u16),
    /// A feature of the format this build does not support yet.
    #[error("unsupported log feature: {0}")]
    Unsupported(&'static str),
    /// Fragments that do not form a record.
    #[error("log record fragments do not match: {0}")]
    Fragment(FragmentError),
}

/// How fragments of a record fail to fit together.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum FragmentError {
    /// A fragment without a first one, or a first one inside a record.
    #[error("unexpected fragment")]
    Unexpected,
    /// A fragment of another record (partition, LSN, type, term or time).
    #[error("fragment of another record")]
    Mismatch,
}
