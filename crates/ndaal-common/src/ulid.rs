//! ULID-based identifiers of FORMATS_DE_DONNEES.md section 10: `stg_` and `mpu_`.

use std::fmt;
use std::str::FromStr;

use crate::base32;
use crate::bytes::DecodeError;

/// Staged object identifier: `stg_` followed by a ULID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StagedId(Ulid);

/// Multipart upload identifier: `mpu_` followed by 26 base32 characters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UploadId(Ulid);

/// 48-bit millisecond timestamp and 80 random bits, big-endian.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct Ulid([u8; 16]);

impl Ulid {
    fn new(timestamp_ms: u64, random: [u8; 10]) -> Result<Self, DecodeError> {
        if timestamp_ms >= 1 << 48 {
            return Err(DecodeError::InvalidValue("ULID timestamp exceeds 48 bits"));
        }
        let mut bytes = [0u8; 16];
        bytes[0..6].copy_from_slice(&timestamp_ms.to_be_bytes()[2..8]);
        bytes[6..16].copy_from_slice(&random);
        Ok(Self(bytes))
    }

    fn timestamp_ms(self) -> u64 {
        let mut be = [0u8; 8];
        be[2..8].copy_from_slice(&self.0[0..6]);
        u64::from_be_bytes(be)
    }
}

macro_rules! prefixed_ulid {
    ($name:ident, $prefix:literal) => {
        impl $name {
            /// Builds the identifier; the caller supplies the clock and the randomness.
            pub fn new(timestamp_ms: u64, random: [u8; 10]) -> Result<Self, DecodeError> {
                Ulid::new(timestamp_ms, random).map(Self)
            }

            /// Creation time, milliseconds since the Unix epoch.
            pub fn timestamp_ms(self) -> u64 {
                self.0.timestamp_ms()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}{}", $prefix, base32::encode_crockford(&self.0.0))
            }
        }

        impl FromStr for $name {
            type Err = DecodeError;

            fn from_str(s: &str) -> Result<Self, DecodeError> {
                let body = s
                    .strip_prefix($prefix)
                    .ok_or(DecodeError::InvalidValue(concat!(
                        "missing ", $prefix, " prefix"
                    )))?;
                base32::decode_crockford(body).map(|bytes| Self(Ulid(bytes)))
            }
        }
    };
}

prefixed_ulid!(StagedId, "stg_");
prefixed_ulid!(UploadId, "mpu_");

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn ulid_known_value_and_prefixes() {
        let zero = StagedId::new(0, [0; 10]).expect("valid");
        assert_eq!(zero.to_string(), format!("stg_{}", "0".repeat(26)));
        let up = UploadId::new(0, [0; 10]).expect("valid");
        assert_eq!(up.to_string(), format!("mpu_{}", "0".repeat(26)));
        assert!(StagedId::new(1 << 48, [0; 10]).is_err());
        assert!(
            format!("mpu_{}", "0".repeat(26))
                .parse::<StagedId>()
                .is_err()
        );
        assert!("0".repeat(26).parse::<UploadId>().is_err());
    }

    proptest! {
        #[test]
        fn staged_and_upload_round_trip(ts in 0u64..(1 << 48), r: [u8; 10]) {
            let s = StagedId::new(ts, r).expect("valid");
            prop_assert_eq!(s.to_string().parse::<StagedId>(), Ok(s));
            prop_assert_eq!(s.timestamp_ms(), ts);
            let u = UploadId::new(ts, r).expect("valid");
            prop_assert_eq!(u.to_string().parse::<UploadId>(), Ok(u));
        }

    }
}
