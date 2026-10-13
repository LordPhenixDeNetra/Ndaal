//! Request identifier (ARCHITECTURE section 18.1).
//!
//! Propagation convention: a frontend opens a `tracing` span named
//! [`REQUEST_SPAN`] with the field [`REQUEST_ID_FIELD`]; every log emitted
//! while serving the request, panics included, then carries the identifier.
//! A thread-local would name the wrong request, since several requests run
//! interleaved on each core.

use std::fmt;
use std::str::FromStr;

use crate::bytes::DecodeError;

/// Name of the span that wraps the handling of one request.
pub const REQUEST_SPAN: &str = "request";

/// Field of [`REQUEST_SPAN`] holding the [`RequestId`].
pub const REQUEST_ID_FIELD: &str = "request_id";

/// 128-bit request identifier, shown as 32 lowercase hexadecimal digits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RequestId(pub [u8; 16]);

impl RequestId {
    /// An identifier from random bytes (the caller owns the randomness).
    pub fn from_random_bytes(bytes: [u8; 16]) -> Self {
        RequestId(bytes)
    }
}

impl fmt::Display for RequestId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.iter().try_for_each(|byte| write!(f, "{byte:02x}"))
    }
}

impl FromStr for RequestId {
    type Err = DecodeError;

    fn from_str(s: &str) -> Result<Self, DecodeError> {
        const INVALID: DecodeError = DecodeError::InvalidValue("request id must be 32 hex digits");
        // Checked first: from_str_radix would accept a sign.
        if s.len() != 32 || !s.bytes().all(|c| c.is_ascii_hexdigit()) {
            return Err(INVALID);
        }
        let mut out = [0u8; 16];
        for (byte, pair) in out.iter_mut().zip(s.as_bytes().as_chunks::<2>().0) {
            let hex = std::str::from_utf8(pair).map_err(|_| INVALID)?;
            *byte = u8::from_str_radix(hex, 16).map_err(|_| INVALID)?;
        }
        Ok(RequestId(out))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn format_is_32_lowercase_hex_digits() {
        let id = RequestId::from_random_bytes([0xAB; 16]);
        assert_eq!(id.to_string(), "ab".repeat(16));
    }

    #[test]
    fn malformed_strings_are_rejected() {
        let good = "0123456789abcdef0123456789ABCDEF";
        assert!(good.parse::<RequestId>().is_ok());
        for bad in [
            "",
            &good[..31],
            &format!("{good}0"),
            "+123456789abcdef0123456789abcdef",
            "g123456789abcdef0123456789abcdef",
        ] {
            assert!(bad.parse::<RequestId>().is_err(), "{bad:?}");
        }
    }

    proptest! {
        #[test]
        fn round_trip(bytes: [u8; 16]) {
            let id = RequestId::from_random_bytes(bytes);
            prop_assert_eq!(id.to_string().parse::<RequestId>(), Ok(id));
        }
    }
}
