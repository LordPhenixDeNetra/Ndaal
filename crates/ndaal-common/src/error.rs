//! Error classification of ARCHITECTURE.md section 18, and the error guide.
//!
//! # Error guide (ARCHITECTURE.md sections 18.1 and 18.2, CLAUDE.md section 4.2)
//!
//! - **Typed per crate** with `thiserror`; no `anyhow` in libraries.
//! - **Keep the cause**: wrap with `#[source]` / `#[from]`, add context, never
//!   replace the original error.
//! - **Classify**: every error type implements [`Classify`] ([`ErrorClass`]:
//!   `Retryable`, `Client`, `Fatal`). Bytes decoded from disk that fail to
//!   decode are `Fatal` (corruption), even though [`DecodeError`] says `Client`.
//! - **Handle once**: an error is either returned or logged (and counted),
//!   never both at every level.
//! - **Messages**: English, lowercase, no trailing period; say what failed and
//!   with which value; never a secret, a key or user data.
//! - **Clients** get a stable code, a short message and the request
//!   identifier ([`crate::request`]), never paths, stack traces or internals.
//!   Mapping to protocol codes happens only in the frontends.
//! - **Panics are bugs** (ADR-030): never panic on external input; binaries
//!   abort on panic and the node restarts from its log; `catch_unwind` must not
//!   hide one. `ndaal_observe::panic::install` logs the panic with its stack
//!   trace and request first.

use crate::bytes::DecodeError;

/// How a caller should react to an error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorClass {
    /// Transient: overload, not leader, timeout; retrying may succeed.
    Retryable,
    /// The request is wrong: invalid input, failed precondition, permission.
    Client,
    /// Corruption or bug: never retried, always reported.
    Fatal,
}

/// Errors that know their class.
pub trait Classify {
    /// The class of this error.
    fn class(&self) -> ErrorClass;
}

/// Decoding errors are `Client`: the decoder cannot tell client input from disk
/// data. Code that decodes bytes read from disk must convert the error into its
/// own `Fatal` (corruption) error.
impl Classify for DecodeError {
    fn class(&self) -> ErrorClass {
        ErrorClass::Client
    }
}

impl Classify for std::io::Error {
    fn class(&self) -> ErrorClass {
        use std::io::ErrorKind;
        match self.kind() {
            ErrorKind::Interrupted
            | ErrorKind::WouldBlock
            | ErrorKind::TimedOut
            | ErrorKind::StorageFull
            | ErrorKind::QuotaExceeded => ErrorClass::Retryable,
            _ => ErrorClass::Fatal,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Error, ErrorKind};

    #[test]
    fn decode_errors_are_client_errors() {
        assert_eq!(DecodeError::VarintTooLong.class(), ErrorClass::Client);
        assert_eq!(DecodeError::InvalidUtf8.class(), ErrorClass::Client);
    }

    #[test]
    fn io_error_classes() {
        let cases = [
            (ErrorKind::Interrupted, ErrorClass::Retryable),
            (ErrorKind::WouldBlock, ErrorClass::Retryable),
            (ErrorKind::TimedOut, ErrorClass::Retryable),
            (ErrorKind::StorageFull, ErrorClass::Retryable),
            (ErrorKind::QuotaExceeded, ErrorClass::Retryable),
            (ErrorKind::NotFound, ErrorClass::Fatal),
            (ErrorKind::PermissionDenied, ErrorClass::Fatal),
            (ErrorKind::InvalidData, ErrorClass::Fatal),
            (ErrorKind::Other, ErrorClass::Fatal),
        ];
        for (kind, class) in cases {
            assert_eq!(Error::from(kind).class(), class, "{kind:?}");
        }
    }
}
