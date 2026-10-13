//! Identifiers of FORMATS_DE_DONNEES.md section 10.
//!
//! 16-byte composite identifiers are big-endian, an exception to the
//! little-endian rule of section 1: their byte order (file names, sorted keys,
//! base32 strings) then follows their value order.

use std::fmt;
use std::str::FromStr;

use crate::base32;
use crate::bytes::DecodeError;

pub use crate::ulid::{StagedId, UploadId};

macro_rules! u32_id {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(pub u32);

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }
    };
}

u32_id!(
    /// Namespace, assigned by the placement driver.
    NamespaceId
);
u32_id!(
    /// Partition, assigned by the placement driver.
    PartitionId
);
u32_id!(
    /// Vector collection, assigned by the placement driver.
    CollectionId
);
u32_id!(
    /// Object bucket, assigned by the placement driver.
    BucketId
);

/// Log sequence number, per partition, starting at 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Lsn(pub u64);

impl Lsn {
    /// The first LSN of a partition.
    pub const FIRST: Lsn = Lsn(1);

    /// The following LSN.
    ///
    /// # Panics
    ///
    /// On `Lsn(u64::MAX)`: unreachable by counting (2^64 writes), but reachable
    /// from corrupted data. Code that got the LSN from disk or the network must
    /// use [`Lsn::checked_next`].
    pub fn next(self) -> Lsn {
        self.checked_next()
            .expect("LSN overflow: 2^64 writes on one partition")
    }

    /// The following LSN, or `None` after `u64::MAX`.
    pub fn checked_next(self) -> Option<Lsn> {
        self.0.checked_add(1).map(Lsn)
    }
}

macro_rules! uuid_id {
    ($name:ident) => {
        impl $name {
            /// Builds a UUID v4 from random bytes (sets the version and RFC 4122 variant bits).
            pub fn from_random_bytes(mut bytes: [u8; 16]) -> Self {
                bytes[6] = (bytes[6] & 0x0F) | 0x40;
                bytes[8] = (bytes[8] & 0x3F) | 0x80;
                Self(bytes)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                for (i, byte) in self.0.iter().enumerate() {
                    if matches!(i, 4 | 6 | 8 | 10) {
                        f.write_str("-")?;
                    }
                    write!(f, "{byte:02x}")?;
                }
                Ok(())
            }
        }

        impl FromStr for $name {
            type Err = DecodeError;

            fn from_str(s: &str) -> Result<Self, DecodeError> {
                parse_uuid(s).map(Self)
            }
        }
    };
}

uuid_id!(NodeId);
uuid_id!(ClusterId);

/// Parses the `8-4-4-4-12` hexadecimal form, either case.
fn parse_uuid(s: &str) -> Result<[u8; 16], DecodeError> {
    const INVALID: DecodeError = DecodeError::InvalidValue("UUID must be 8-4-4-4-12 hex digits");
    let text = s.as_bytes();
    if text.len() != 36 || [8, 13, 18, 23].iter().any(|&i| text[i] != b'-') {
        return Err(INVALID);
    }
    let digits: Vec<u8> = text.iter().copied().filter(|&c| c != b'-').collect();
    // from_str_radix accepts a sign: check the digits first.
    if digits.len() != 32 || !digits.iter().all(u8::is_ascii_hexdigit) {
        return Err(INVALID);
    }
    let mut out = [0u8; 16];
    for (byte, pair) in out.iter_mut().zip(digits.as_chunks::<2>().0) {
        let hex = std::str::from_utf8(pair).map_err(|_| INVALID)?;
        *byte = u8::from_str_radix(hex, 16).map_err(|_| INVALID)?;
    }
    Ok(out)
}

/// Node identifier: UUID v4, generated at first start.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeId(pub [u8; 16]);

/// Cluster identifier: UUID v4, generated at cluster initialization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClusterId(pub [u8; 16]);

/// Segment identifier: `partition (4) . lsn (8) . counter (4)`, big-endian.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SegmentId {
    /// Partition that created the segment.
    pub partition: PartitionId,
    /// LSN at creation.
    pub lsn: Lsn,
    /// Disambiguates segments created at the same LSN.
    pub counter: u32,
}

/// Object chunk identifier, same layout as [`SegmentId`]; unique without coordination.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChunkId {
    /// Partition that created the chunk.
    pub partition: PartitionId,
    /// LSN at creation.
    pub lsn: Lsn,
    /// Disambiguates chunks created at the same LSN.
    pub counter: u32,
}

/// Object version: `timestamp (8) . lsn (8)`, big-endian, shown as 26 base32 characters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VersionId {
    /// Microseconds since the Unix epoch.
    pub timestamp_micros: u64,
    /// LSN of the write.
    pub lsn: Lsn,
}

macro_rules! composite_id {
    ($name:ident) => {
        impl $name {
            /// `partition (4) . lsn (8) . counter (4)`, big-endian.
            pub fn to_bytes(self) -> [u8; 16] {
                let mut out = [0u8; 16];
                out[0..4].copy_from_slice(&self.partition.0.to_be_bytes());
                out[4..12].copy_from_slice(&self.lsn.0.to_be_bytes());
                out[12..16].copy_from_slice(&self.counter.to_be_bytes());
                out
            }

            /// Inverse of `to_bytes`.
            pub fn from_bytes(bytes: [u8; 16]) -> Self {
                let value = u128::from_be_bytes(bytes);
                // Truncations intended: split the 128 bits into their fields.
                Self {
                    partition: PartitionId((value >> 96) as u32),
                    lsn: Lsn((value >> 32) as u64),
                    counter: value as u32,
                }
            }
        }
    };
}

composite_id!(SegmentId);
composite_id!(ChunkId);

impl VersionId {
    fn to_bytes(self) -> [u8; 16] {
        (u128::from(self.timestamp_micros) << 64 | u128::from(self.lsn.0)).to_be_bytes()
    }
}

impl fmt::Display for VersionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&base32::encode_crockford(&self.to_bytes()))
    }
}

impl FromStr for VersionId {
    type Err = DecodeError;

    fn from_str(s: &str) -> Result<Self, DecodeError> {
        let value = u128::from_be_bytes(base32::decode_crockford(s)?);
        // Truncations intended: split the 128 bits in two halves.
        Ok(Self {
            timestamp_micros: (value >> 64) as u64,
            lsn: Lsn(value as u64),
        })
    }
}

/// Transaction identifier: partition in bits 64 to 95, LSN in bits 0 to 63.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TxId(pub u128);

impl TxId {
    /// The transaction committed at `lsn` on `partition`.
    pub fn new(partition: PartitionId, lsn: Lsn) -> TxId {
        TxId(u128::from(partition.0) << 64 | u128::from(lsn.0))
    }

    /// Partition of the transaction.
    pub fn partition(self) -> PartitionId {
        // Truncation intended: bits 64 to 95.
        PartitionId((self.0 >> 64) as u32)
    }

    /// LSN of the transaction.
    pub fn lsn(self) -> Lsn {
        // Truncation intended: bits 0 to 63.
        Lsn(self.0 as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn segment(p: u32, l: u64, c: u32) -> SegmentId {
        SegmentId {
            partition: PartitionId(p),
            lsn: Lsn(l),
            counter: c,
        }
    }

    #[test]
    fn lsn_checked_next() {
        assert_eq!(Lsn(1).checked_next(), Some(Lsn(2)));
        assert_eq!(Lsn(u64::MAX).checked_next(), None);
    }

    #[test]
    fn lsn_sequence() {
        assert_eq!(Lsn::FIRST, Lsn(1));
        assert_eq!(Lsn::FIRST.next(), Lsn(2));
    }

    #[test]
    fn uuid_v4_bits_and_format() {
        let id = NodeId::from_random_bytes([0xFF; 16]);
        assert_eq!(id.to_string(), "ffffffff-ffff-4fff-bfff-ffffffffffff");
        let id = ClusterId::from_random_bytes([0; 16]);
        assert_eq!(id.to_string(), "00000000-0000-4000-8000-000000000000");
    }

    #[test]
    fn uuid_parsing_rejects_malformed() {
        assert!(
            "ffffffff-ffff-4fff-bfff-ffffffffffff"
                .parse::<NodeId>()
                .is_ok()
        );
        assert!(
            "FFFFFFFF-FFFF-4FFF-BFFF-FFFFFFFFFFFF"
                .parse::<NodeId>()
                .is_ok()
        );
        for bad in [
            "",
            "ffffffff-ffff-4fff-bfff-fffffffffff",
            "ffffffffffff-4fff-bfff-ffffffffffff-",
            "gfffffff-ffff-4fff-bfff-ffffffffffff",
            "ffffffff+ffff-4fff-bfff-ffffffffffff",
            "+fffffff-+fff-4fff-bfff-ffffffffffff",
            "-fffffff-ffff-4fff-bfff-ffffffffffff",
        ] {
            assert!(bad.parse::<NodeId>().is_err(), "{bad:?}");
        }
    }

    #[test]
    fn segment_layout_is_big_endian() {
        let bytes = segment(1, 2, 3).to_bytes();
        assert_eq!(bytes, [0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 3]);
    }

    #[test]
    fn tx_id_layout() {
        let tx = TxId::new(PartitionId(7), Lsn(9));
        assert_eq!(tx.0, (7u128 << 64) | 9);
        assert_eq!((tx.partition(), tx.lsn()), (PartitionId(7), Lsn(9)));
    }

    proptest! {
        #[test]
        fn uuid_round_trip(b: [u8; 16]) {
            let id = NodeId::from_random_bytes(b);
            prop_assert_eq!(id.to_string().parse::<NodeId>(), Ok(id));
        }

        #[test]
        fn segment_and_chunk_round_trip_and_order(
            a in (any::<u32>(), any::<u64>(), any::<u32>()),
            b in (any::<u32>(), any::<u64>(), any::<u32>()),
        ) {
            let (sa, sb) = (segment(a.0, a.1, a.2), segment(b.0, b.1, b.2));
            prop_assert_eq!(SegmentId::from_bytes(sa.to_bytes()), sa);
            prop_assert_eq!(sa.cmp(&sb), sa.to_bytes().cmp(&sb.to_bytes()));
            let c = ChunkId { partition: PartitionId(a.0), lsn: Lsn(a.1), counter: a.2 };
            prop_assert_eq!(ChunkId::from_bytes(c.to_bytes()), c);
        }

        #[test]
        fn version_round_trip_and_chronological_strings(
            t1: u64, l1: u64, t2: u64, l2: u64
        ) {
            let v1 = VersionId { timestamp_micros: t1, lsn: Lsn(l1) };
            let v2 = VersionId { timestamp_micros: t2, lsn: Lsn(l2) };
            prop_assert_eq!(v1.to_string().parse::<VersionId>(), Ok(v1));
            prop_assert_eq!(v1.to_string().len(), 26);
            prop_assert_eq!(v1.cmp(&v2), v1.to_string().cmp(&v2.to_string()));
        }

        #[test]
        fn tx_id_round_trip(p: u32, l: u64) {
            let tx = TxId::new(PartitionId(p), Lsn(l));
            prop_assert_eq!((tx.partition(), tx.lsn()), (PartitionId(p), Lsn(l)));
        }
    }
}
