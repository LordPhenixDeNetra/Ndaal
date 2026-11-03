//! Blocks of a section (FORMATS_DE_DONNEES.md section 3.2): stored data,
//! compression byte, CRC seeded by the segment and the block's offset
//! (ADR-035). Pure functions over bytes.

use ndaal_common::ids::SegmentId;
use ndaal_wal::seeded_crc_with;

use crate::segment::SegmentError;
use crate::segment::format::corrupt;

/// Largest block, stored or decompressed: 16 MiB. Bounds what a corrupt
/// size can make the reader allocate.
pub const MAX_BLOCK_LEN: usize = 16 << 20;
/// Compression byte and CRC after the stored data.
pub const TRAILER_LEN: usize = 5;
/// LZ4 blocks start with their decompressed size.
const SIZE_LEN: usize = 4;

const RAW: u8 = 0;
const LZ4: u8 = 1;

/// How a block is stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compression {
    /// As given.
    None,
    /// LZ4 (ADR-018), unless it does not shrink the block.
    Lz4,
}

/// Seed of the CRC of the bytes at `offset` in segment `segment` (ADR-035):
/// `segment_id` big-endian, then `offset` little-endian.
pub fn block_seed(segment: SegmentId, offset: u64) -> [u8; 24] {
    let mut seed = [0u8; 24];
    seed[..16].copy_from_slice(&segment.to_bytes());
    seed[16..].copy_from_slice(&offset.to_le_bytes());
    seed
}

/// Appends the block of `data` to `out`, stored raw when LZ4 does not
/// shrink it.
pub fn encode_block(
    data: &[u8],
    compression: Compression,
    seed: &[u8; 24],
    out: &mut Vec<u8>,
) -> Result<(), SegmentError> {
    if data.is_empty() {
        return Err(SegmentError::Misuse("empty block"));
    }
    if data.len() > MAX_BLOCK_LEN {
        return Err(SegmentError::Misuse("block over MAX_BLOCK_LEN"));
    }
    let start = out.len();
    let compressed = match compression {
        Compression::None => None,
        Compression::Lz4 => {
            Some(lz4_flex::block::compress(data)).filter(|c| SIZE_LEN + c.len() < data.len())
        }
    };
    match compressed {
        Some(compressed) => {
            let size = u32::try_from(data.len()).expect("checked against MAX_BLOCK_LEN");
            out.extend_from_slice(&size.to_le_bytes());
            out.extend_from_slice(&compressed);
            out.push(LZ4);
        }
        None => {
            out.extend_from_slice(data);
            out.push(RAW);
        }
    }
    let crc = seeded_crc_with(seed, &out[start..]);
    out.extend_from_slice(&crc.to_le_bytes());
    Ok(())
}

/// The data of `stored` (a whole block, trailer included), once its CRC
/// under `seed` and its compression are checked.
pub fn decode_block(stored: &[u8], seed: &[u8; 24]) -> Result<Vec<u8>, SegmentError> {
    if stored.len() <= TRAILER_LEN {
        return Err(corrupt("block shorter than its trailer", 0));
    }
    let body_len = stored.len() - 4;
    let (body, crc) = stored.split_at(body_len);
    let crc = u32::from_le_bytes(crc.try_into().expect("split 4 bytes from the end"));
    if seeded_crc_with(seed, body) != crc {
        return Err(corrupt("block crc", 0));
    }
    let (data, compression) = body.split_at(body_len - 1);
    match compression[0] {
        RAW => Ok(data.to_vec()),
        LZ4 => decompress(data),
        _ => Err(corrupt("block compression", 0)),
    }
}

fn decompress(data: &[u8]) -> Result<Vec<u8>, SegmentError> {
    let Some((size, stream)) = data.split_first_chunk::<SIZE_LEN>() else {
        return Err(corrupt("lz4 block without its size", 0));
    };
    let size = usize::try_from(u32::from_le_bytes(*size)).unwrap_or(usize::MAX);
    if size == 0 || size > MAX_BLOCK_LEN {
        return Err(corrupt("lz4 block size", 0));
    }
    let mut out = vec![0u8; size];
    match lz4_flex::block::decompress_into(stream, &mut out) {
        Ok(n) if n == size => Ok(out),
        _ => Err(corrupt("lz4 stream", 0)),
    }
}

#[cfg(test)]
mod tests {
    use ndaal_common::ids::{Lsn, PartitionId, SegmentId};

    use super::*;

    const ID: SegmentId = SegmentId {
        partition: PartitionId(1),
        lsn: Lsn(2),
        counter: 3,
    };

    fn encoded(data: &[u8], compression: Compression, offset: u64) -> Vec<u8> {
        let mut out = Vec::new();
        encode_block(data, compression, &block_seed(ID, offset), &mut out).expect("encode");
        out
    }

    /// A block with the given stored bytes and compression byte, CRC valid.
    fn forged(stored: &[u8], compression: u8) -> Vec<u8> {
        let mut out = stored.to_vec();
        out.push(compression);
        let crc = ndaal_wal::seeded_crc_with(&block_seed(ID, 0), &out);
        out.extend_from_slice(&crc.to_le_bytes());
        out
    }

    fn text(len: usize) -> Vec<u8> {
        (0..len).map(|i| b"segment envelope "[i % 17]).collect()
    }

    #[test]
    fn round_trip_both_compressions() {
        for compression in [Compression::None, Compression::Lz4] {
            for data in [vec![7u8], text(300), text(100_000)] {
                let block = encoded(&data, compression, 4096);
                let back = decode_block(&block, &block_seed(ID, 4096)).expect("decode");
                assert_eq!(back, data);
            }
        }
    }

    #[test]
    fn lz4_shrinks_repetitive_data() {
        let block = encoded(&vec![0u8; 1 << 20], Compression::Lz4, 0);
        assert!(block.len() < 10 * 1024, "{}", block.len());
        assert_eq!(block[block.len() - 5], 1);
    }

    #[test]
    fn incompressible_is_stored_raw() {
        let mut x = 0x9e37_79b9_7f4a_7c15u64;
        let data: Vec<u8> = (0..4096)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                x.to_le_bytes()[0]
            })
            .collect();
        let block = encoded(&data, Compression::Lz4, 0);
        assert_eq!(block.len(), data.len() + TRAILER_LEN);
        assert_eq!(block[data.len()], 0);
    }

    #[test]
    fn every_bit_flip_is_refused() {
        for compression in [Compression::None, Compression::Lz4] {
            let block = encoded(&text(300), compression, 0);
            for bit in 0..block.len() * 8 {
                let mut bad = block.clone();
                bad[bit / 8] ^= 1 << (bit % 8);
                assert!(decode_block(&bad, &block_seed(ID, 0)).is_err(), "bit {bit}");
            }
        }
    }

    #[test]
    fn other_seed_is_refused() {
        let block = encoded(&text(300), Compression::Lz4, 8192);
        let other = SegmentId { counter: 4, ..ID };
        assert!(decode_block(&block, &block_seed(other, 8192)).is_err());
        assert!(decode_block(&block, &block_seed(ID, 8193)).is_err());
    }

    #[test]
    fn huge_announced_size_is_refused() {
        // u32::MAX would make an unbounded reader allocate 4 GiB.
        for size in [u32::try_from(MAX_BLOCK_LEN + 1).expect("fits"), u32::MAX] {
            let mut stored = size.to_le_bytes().to_vec();
            stored.extend_from_slice(&[0x10, b'a']);
            let err = decode_block(&forged(&stored, 1), &block_seed(ID, 0)).expect_err("too big");
            assert!(matches!(err, SegmentError::Corrupt { .. }), "{err:?}");
        }
    }

    #[test]
    fn wrong_decompressed_size_is_refused() {
        let real = encoded(&text(300), Compression::Lz4, 0);
        let mut stored = real[..real.len() - TRAILER_LEN].to_vec();
        stored[..4].copy_from_slice(&299u32.to_le_bytes());
        assert!(decode_block(&forged(&stored, 1), &block_seed(ID, 0)).is_err());
        stored[..4].copy_from_slice(&301u32.to_le_bytes());
        assert!(decode_block(&forged(&stored, 1), &block_seed(ID, 0)).is_err());
    }

    #[test]
    fn unknown_compression_is_refused() {
        let err = decode_block(&forged(b"data", 2), &block_seed(ID, 0)).expect_err("unknown");
        assert!(matches!(err, SegmentError::Corrupt { .. }), "{err:?}");
    }

    #[test]
    fn short_blocks_are_refused() {
        for len in 0..=TRAILER_LEN {
            assert!(
                decode_block(&vec![0; len], &block_seed(ID, 0)).is_err(),
                "{len}"
            );
        }
    }

    #[test]
    fn empty_and_oversized_are_misuse() {
        let mut out = Vec::new();
        let seed = block_seed(ID, 0);
        for data in [vec![], vec![0; MAX_BLOCK_LEN + 1]] {
            let err = encode_block(&data, Compression::Lz4, &seed, &mut out).expect_err("misuse");
            assert!(matches!(err, SegmentError::Misuse(_)), "{err:?}");
        }
        assert!(out.is_empty());
    }
}
