//! Header and footer of a segment (FORMATS_DE_DONNEES.md sections 3.1 and
//! 3.4): pure functions over bytes.

use ndaal_common::ids::{Lsn, PartitionId, SegmentId};
use ndaal_wal::masked_crc;

use crate::segment::SegmentError;

/// Size of the header, at offset 0 (one 4 KiB page, for direct I/O).
pub const HEADER_LEN: usize = 4096;
/// Size of the footer, the last bytes of the file.
pub const FOOTER_LEN: usize = 4096;
/// Version of the envelope this code writes and reads.
pub const FORMAT: u16 = 1;

const HEADER_MAGIC: &[u8; 8] = b"NDLSEG01";
const FOOTER_MAGIC: &[u8; 8] = b"STRSEGND";
/// Header bytes 0..68 hold the fields; the rest up to the CRC is reserved.
const HEADER_FIELDS_END: usize = 68;
const HEADER_CRC_AT: usize = HEADER_LEN - 4;
/// Footer bytes 0..18 hold the fields; then reserved, CRC, magic.
const FOOTER_FIELDS_END: usize = 18;
pub(crate) const FOOTER_CRC_AT: usize = FOOTER_LEN - 12;

/// What a segment holds; the engine owning it interprets its sections.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum SegmentKind {
    /// Key-value SST (T02.15).
    Sst = 1,
    /// Vector segment (T05.10).
    Vector = 2,
    /// Object metadata.
    ObjectMeta = 3,
    /// Values over 1 MiB, stored out of line (EF-KV-16).
    LargeValues = 4,
    /// Vector payloads.
    Payload = 5,
}

impl SegmentKind {
    fn from_u16(value: u16) -> Option<Self> {
        Some(match value {
            1 => Self::Sst,
            2 => Self::Vector,
            3 => Self::ObjectMeta,
            4 => Self::LargeValues,
            5 => Self::Payload,
            _ => return None,
        })
    }
}

/// The header fields (FORMATS_DE_DONNEES.md section 3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SegmentHeader {
    /// What the segment holds.
    pub kind: SegmentKind,
    /// Version of the engine's own format, checked by the engine.
    pub kind_format: u16,
    /// The segment's identifier; it also seeds its block CRCs (ADR-035).
    pub segment_id: SegmentId,
    /// Partition the segment belongs to.
    pub partition: PartitionId,
    /// Lowest LSN covered.
    pub min_lsn: Lsn,
    /// Highest LSN covered.
    pub max_lsn: Lsn,
}

impl SegmentHeader {
    /// The 4 096 header bytes, unencrypted (`key_id` zero).
    pub fn encode(&self) -> [u8; HEADER_LEN] {
        let mut out = [0u8; HEADER_LEN];
        out[0..8].copy_from_slice(HEADER_MAGIC);
        out[8..10].copy_from_slice(&FORMAT.to_le_bytes());
        out[10..12].copy_from_slice(&(self.kind as u16).to_le_bytes());
        out[12..14].copy_from_slice(&self.kind_format.to_le_bytes());
        out[16..32].copy_from_slice(&self.segment_id.to_bytes());
        out[32..36].copy_from_slice(&self.partition.0.to_le_bytes());
        out[36..44].copy_from_slice(&self.min_lsn.0.to_le_bytes());
        out[44..52].copy_from_slice(&self.max_lsn.0.to_le_bytes());
        let crc = masked_crc(&out[..HEADER_CRC_AT]);
        out[HEADER_CRC_AT..].copy_from_slice(&crc.to_le_bytes());
        out
    }

    /// Checks and decodes a header of a segment of kind `expected`; also
    /// returns the envelope format, which the footer must repeat.
    pub fn decode(bytes: &[u8], expected: SegmentKind) -> Result<(Self, u16), SegmentError> {
        let bytes = sealed(bytes, HEADER_LEN, HEADER_MAGIC, 0, HEADER_CRC_AT)?;
        let format = u16_at(bytes, 8);
        if format != FORMAT {
            return Err(SegmentError::UnsupportedFormat { found: format });
        }
        if u16_at(bytes, 14) != 0 || !zero(&bytes[HEADER_FIELDS_END..HEADER_CRC_AT]) {
            return Err(corrupt("header reserved bytes", 0));
        }
        let found = u16_at(bytes, 10);
        let kind = SegmentKind::from_u16(found)
            .filter(|&k| k == expected)
            .ok_or(SegmentError::WrongKind { expected, found })?;
        if !zero(&bytes[52..68]) {
            return Err(SegmentError::Encrypted);
        }
        let header = SegmentHeader {
            kind,
            kind_format: u16_at(bytes, 12),
            segment_id: SegmentId::from_bytes(array(&bytes[16..32])),
            partition: PartitionId(u32::from_le_bytes(array(&bytes[32..36]))),
            min_lsn: Lsn(u64_at(bytes, 36)),
            max_lsn: Lsn(u64_at(bytes, 44)),
        };
        if header.min_lsn > header.max_lsn {
            return Err(corrupt("header lsn range", 36));
        }
        Ok((header, format))
    }
}

/// The footer fields (FORMATS_DE_DONNEES.md section 3.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Footer {
    /// Offset of the section directory.
    pub directory_offset: u64,
    /// Size of the directory in bytes.
    pub directory_len: u32,
    /// Seeded CRC of the directory (ADR-035).
    pub directory_crc: u32,
    /// Envelope format; must equal the header's.
    pub format: u16,
}

impl Footer {
    /// The 4 096 footer bytes.
    pub fn encode(&self) -> [u8; FOOTER_LEN] {
        let mut out = [0u8; FOOTER_LEN];
        out[0..8].copy_from_slice(&self.directory_offset.to_le_bytes());
        out[8..12].copy_from_slice(&self.directory_len.to_le_bytes());
        out[12..16].copy_from_slice(&self.directory_crc.to_le_bytes());
        out[16..18].copy_from_slice(&self.format.to_le_bytes());
        let crc = masked_crc(&out[..FOOTER_CRC_AT]);
        out[FOOTER_CRC_AT..FOOTER_CRC_AT + 4].copy_from_slice(&crc.to_le_bytes());
        out[FOOTER_LEN - 8..].copy_from_slice(FOOTER_MAGIC);
        out
    }

    /// Checks and decodes a footer.
    pub fn decode(bytes: &[u8]) -> Result<Self, SegmentError> {
        let bytes = sealed(
            bytes,
            FOOTER_LEN,
            FOOTER_MAGIC,
            FOOTER_LEN - 8,
            FOOTER_CRC_AT,
        )?;
        let format = u16_at(bytes, 16);
        if format != FORMAT {
            return Err(SegmentError::UnsupportedFormat { found: format });
        }
        if !zero(&bytes[FOOTER_FIELDS_END..FOOTER_CRC_AT]) {
            return Err(corrupt("footer reserved bytes", 0));
        }
        Ok(Footer {
            directory_offset: u64_at(bytes, 0),
            directory_len: u32::from_le_bytes(array(&bytes[8..12])),
            directory_crc: u32::from_le_bytes(array(&bytes[12..16])),
            format,
        })
    }
}

/// `bytes` cut to `len`, once its length, magic (at `magic_at`) and masked
/// CRC (at `crc_at`, over the bytes before it) are checked.
fn sealed<'a>(
    bytes: &'a [u8],
    len: usize,
    magic: &[u8; 8],
    magic_at: usize,
    crc_at: usize,
) -> Result<&'a [u8], SegmentError> {
    let bytes = bytes.get(..len).ok_or(corrupt("short read", 0))?;
    if &bytes[magic_at..magic_at + 8] != magic {
        return Err(corrupt("magic", 0));
    }
    if masked_crc(&bytes[..crc_at]) != u32::from_le_bytes(array(&bytes[crc_at..crc_at + 4])) {
        return Err(corrupt("crc", 0));
    }
    Ok(bytes)
}

pub(crate) fn corrupt(what: &'static str, offset: u64) -> SegmentError {
    SegmentError::Corrupt { what, offset }
}

fn zero(bytes: &[u8]) -> bool {
    bytes.iter().all(|&b| b == 0)
}

fn array<const N: usize>(bytes: &[u8]) -> [u8; N] {
    bytes.try_into().expect("callers slice exactly N bytes")
}

fn u16_at(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes(array(&bytes[at..at + 2]))
}

fn u64_at(bytes: &[u8], at: usize) -> u64 {
    u64::from_le_bytes(array(&bytes[at..at + 8]))
}

#[cfg(test)]
mod tests {
    use ndaal_common::ids::{Lsn, PartitionId, SegmentId};

    use super::*;
    use crate::segment::SegmentError;

    fn header() -> SegmentHeader {
        SegmentHeader {
            kind: SegmentKind::Sst,
            kind_format: 3,
            segment_id: SegmentId {
                partition: PartitionId(4),
                lsn: Lsn(10),
                counter: 2,
            },
            partition: PartitionId(4),
            min_lsn: Lsn(1),
            max_lsn: Lsn(10),
        }
    }

    fn footer() -> Footer {
        Footer {
            directory_offset: 8192,
            directory_len: 37,
            directory_crc: 0xdead_beef,
            format: FORMAT,
        }
    }

    /// Recomputes the header CRC, to test a rule behind it.
    fn reseal_header(bytes: &mut [u8]) {
        let crc = ndaal_wal::masked_crc(&bytes[..HEADER_LEN - 4]);
        bytes[HEADER_LEN - 4..].copy_from_slice(&crc.to_le_bytes());
    }

    fn reseal_footer(bytes: &mut [u8]) {
        let crc = ndaal_wal::masked_crc(&bytes[..FOOTER_CRC_AT]);
        bytes[FOOTER_CRC_AT..FOOTER_CRC_AT + 4].copy_from_slice(&crc.to_le_bytes());
    }

    #[test]
    fn header_round_trip() {
        let bytes = header().encode();
        let (back, format) = SegmentHeader::decode(&bytes, SegmentKind::Sst).expect("valid");
        assert_eq!(back, header());
        assert_eq!(format, FORMAT);
        assert_eq!(&bytes[..8], b"NDLSEG01");
    }

    #[test]
    fn footer_round_trip() {
        let bytes = footer().encode();
        assert_eq!(Footer::decode(&bytes).expect("valid"), footer());
        assert_eq!(&bytes[FOOTER_LEN - 8..], b"STRSEGND");
    }

    #[test]
    fn every_header_bit_flip_is_refused() {
        let bytes = header().encode();
        for bit in 0..HEADER_LEN * 8 {
            let mut bad = bytes;
            bad[bit / 8] ^= 1 << (bit % 8);
            assert!(
                SegmentHeader::decode(&bad, SegmentKind::Sst).is_err(),
                "bit {bit}"
            );
        }
    }

    #[test]
    fn every_footer_bit_flip_is_refused() {
        let bytes = footer().encode();
        for bit in 0..FOOTER_LEN * 8 {
            let mut bad = bytes;
            bad[bit / 8] ^= 1 << (bit % 8);
            assert!(Footer::decode(&bad).is_err(), "bit {bit}");
        }
    }

    #[test]
    fn reserved_bytes_are_checked() {
        for at in [14, 68, 2000, HEADER_LEN - 5] {
            let mut bad = header().encode();
            bad[at] = 1;
            reseal_header(&mut bad);
            let err = SegmentHeader::decode(&bad, SegmentKind::Sst).expect_err("reserved");
            assert!(matches!(err, SegmentError::Corrupt { .. }), "{at}: {err:?}");
        }
        for at in [18, 3000, FOOTER_CRC_AT - 1] {
            let mut bad = footer().encode();
            bad[at] = 1;
            reseal_footer(&mut bad);
            let err = Footer::decode(&bad).expect_err("reserved");
            assert!(matches!(err, SegmentError::Corrupt { .. }), "{at}: {err:?}");
        }
    }

    #[test]
    fn format_kind_and_key_are_checked() {
        let decode = |bytes: &[u8]| SegmentHeader::decode(bytes, SegmentKind::Sst);

        let mut bad = header().encode();
        bad[8..10].copy_from_slice(&2u16.to_le_bytes());
        reseal_header(&mut bad);
        assert!(matches!(
            decode(&bad),
            Err(SegmentError::UnsupportedFormat { found: 2 })
        ));

        let bytes = header().encode();
        assert!(matches!(
            SegmentHeader::decode(&bytes, SegmentKind::Vector),
            Err(SegmentError::WrongKind {
                expected: SegmentKind::Vector,
                found: 1
            })
        ));

        let mut bad = header().encode();
        bad[10..12].copy_from_slice(&9u16.to_le_bytes());
        reseal_header(&mut bad);
        assert!(matches!(
            decode(&bad),
            Err(SegmentError::WrongKind { found: 9, .. })
        ));

        let mut bad = header().encode();
        bad[60] = 1;
        reseal_header(&mut bad);
        assert!(matches!(decode(&bad), Err(SegmentError::Encrypted)));

        let mut swapped = header();
        swapped.min_lsn = Lsn(11);
        assert!(matches!(
            decode(&swapped.encode()),
            Err(SegmentError::Corrupt { .. })
        ));

        let mut bad = footer().encode();
        bad[16..18].copy_from_slice(&2u16.to_le_bytes());
        reseal_footer(&mut bad);
        assert!(matches!(
            Footer::decode(&bad),
            Err(SegmentError::UnsupportedFormat { found: 2 })
        ));
    }

    #[test]
    fn short_input_is_refused() {
        assert!(SegmentHeader::decode(&[0; 100], SegmentKind::Sst).is_err());
        assert!(Footer::decode(&[0; 100]).is_err());
    }
}
