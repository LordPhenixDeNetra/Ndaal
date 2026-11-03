//! Fuzz target bodies (STRATEGIE_DE_TESTS.md section 5, ENF-SEC-03).
//!
//! Each target panics when a property is violated: that is how libFuzzer
//! reports a finding. The bodies live here, in the stable workspace, so the
//! versioned corpus is replayed by `cargo test` on every system; `fuzz/`
//! only wraps them for `cargo fuzz`.

#![forbid(unsafe_code)]

/// Every target, by name: the name of its `fuzz/fuzz_targets/` file and of
/// its corpus directory.
pub const TARGETS: &[(&str, Target)] = &[
    ("common_codec", common_codec),
    ("segment_reader", segment_reader),
    ("wal_reader", wal_reader),
];

/// A target body: panics when a property is violated.
pub type Target = fn(&[u8]);

use ndaal_common::base32::{decode_crockford, encode_crockford};
use ndaal_common::bytes::{self, DecodeError};
use ndaal_common::ids::{Lsn, NodeId, PartitionId, SegmentId};
use ndaal_storage::segment::{
    Compression, FOOTER_LEN, Footer, HEADER_LEN, SegmentHeader, SegmentKind, block_seed,
    decode_block, decode_directory, encode_block,
};
use ndaal_wal::{FileHeader, FileReader, Reassembler, Record, Step, encode, seeded_crc_with};

/// Log reader (ndaal-wal): never panics on any file, and every record it
/// returns re-encodes into frames that read back to the same record. Bytes
/// without a valid file header are read behind a valid one, so the fuzzer
/// spends its time on frames rather than on the header CRC.
pub fn wal_reader(data: &[u8]) {
    let file = if FileHeader::decode(data).is_ok() {
        data.to_vec()
    } else {
        let header = FileHeader {
            shard: 0,
            file_no: 0,
            node: NodeId([0; 16]),
            created_at: 0,
            recycle_epoch: 0,
            recycled: false,
        };
        let mut file = header.encode().to_vec();
        file.extend_from_slice(data);
        file
    };
    let Ok(mut reader) = FileReader::new(&file) else {
        return;
    };
    let header = *reader.header();
    let mut glue = Reassembler::default();
    while let Ok(Step::Frame(frame)) = reader.next_step() {
        if let Ok(Some(record)) = glue.push(frame) {
            assert_eq!(read_one(&header, &record).as_ref(), Some(&record));
        }
    }
}

/// `record` written alone in a file with `header`, then read back.
fn read_one(header: &FileHeader, record: &Record<'_>) -> Option<Record<'static>> {
    let mut file = header.encode().to_vec();
    encode(record, header.recycle_epoch, header.file_no, &mut file);
    let mut reader = FileReader::new(&file).ok()?;
    let mut glue = Reassembler::default();
    while let Ok(Step::Frame(frame)) = reader.next_step() {
        if let Ok(Some(back)) = glue.push(frame) {
            return Some(Record {
                payload: back.payload.into_owned().into(),
                ..back
            });
        }
    }
    None
}

/// Decoders of `ndaal_common::bytes` and `ndaal_common::base32`: never panic,
/// and whatever decodes re-encodes to exactly the bytes it consumed (every
/// encoding is canonical: varints are minimal).
pub fn common_codec(data: &[u8]) {
    round_trip(data, bytes::get_varint, |out, v| bytes::put_varint(out, *v));
    round_trip(data, bytes::get_u16_le, |out, v| bytes::put_u16_le(out, *v));
    round_trip(data, bytes::get_u32_le, |out, v| bytes::put_u32_le(out, *v));
    round_trip(data, bytes::get_u64_le, |out, v| bytes::put_u64_le(out, *v));
    round_trip(data, bytes::get_bytes, |out, v| bytes::put_bytes(out, v));
    round_trip(data, bytes::get_str, |out, v| bytes::put_str(out, v));
    let mut input = data;
    if let Ok(v) = bytes::get_varint(&mut input) {
        assert_eq!(bytes::zigzag_encode(bytes::zigzag_decode(v)), v);
    }
    if let Ok(text) = std::str::from_utf8(data)
        && let Ok(value) = decode_crockford(text)
    {
        // Decoding accepts lower case and I/L/O aliases; encoding is canonical.
        let canonical = encode_crockford(&value);
        assert_eq!(decode_crockford(&canonical), Ok(value), "{text}");
    }
}

/// Decodes one value from `data`; on success, `encode` must rebuild the
/// consumed prefix exactly.
fn round_trip<'a, T>(
    data: &'a [u8],
    decode: impl Fn(&mut &'a [u8]) -> Result<T, DecodeError>,
    encode: impl Fn(&mut Vec<u8>, &T),
) {
    let mut input = data;
    if let Ok(value) = decode(&mut input) {
        let consumed = &data[..data.len() - input.len()];
        let mut out = Vec::new();
        encode(&mut out, &value);
        assert_eq!(out, consumed, "re-encoding differs from the input");
    }
}

/// Segment envelope (ndaal-storage): no decoder panics, whatever the bytes.
/// The first byte picks a compression and the rest is sealed into a block
/// with a valid CRC, so the fuzzer reaches the LZ4 decoder; a block that
/// decodes re-encodes into a block that decodes to the same data. The whole
/// input is also read as a segment file and as a directory.
pub fn segment_reader(data: &[u8]) {
    let seed = block_seed(FUZZ_SEGMENT, 4096);
    if let Some((&compression, body)) = data.split_first() {
        let mut stored = body.to_vec();
        stored.push(compression % 3);
        let crc = seeded_crc_with(&seed, &stored);
        stored.extend_from_slice(&crc.to_le_bytes());
        if let Ok(block) = decode_block(&stored, &seed) {
            let mut again = Vec::new();
            encode_block(&block, Compression::Lz4, &seed, &mut again)
                .expect("a decoded block re-encodes");
            assert_eq!(decode_block(&again, &seed).ok(), Some(block));
        }
    }
    // Errors are expected: only a panic is a finding.
    let _ = decode_directory(data, u64::MAX);
    if data.len() >= HEADER_LEN + FOOTER_LEN {
        let _ = SegmentHeader::decode(&data[..HEADER_LEN], SegmentKind::Sst);
        if let Ok(footer) = Footer::decode(&data[data.len() - FOOTER_LEN..]) {
            let start = usize::try_from(footer.directory_offset).unwrap_or(usize::MAX);
            let end = start.saturating_add(footer.directory_len as usize);
            if let Some(directory) = data.get(start..end) {
                let _ = decode_directory(directory, footer.directory_offset);
            }
        }
    }
}

/// Segment identifier of the fuzzed blocks.
const FUZZ_SEGMENT: SegmentId = SegmentId {
    partition: PartitionId(1),
    lsn: Lsn(1),
    counter: 0,
};
