//! Reading a log file: frames, end of log, corruption and the torn zone
//! (FORMATS_DE_DONNEES.md section 2.3, ADR-033, T02.07, T02.10).

use std::borrow::Cow;

use ndaal_common::ids::{Lsn, NodeId, PartitionId};
use ndaal_wal::{
    FRAME_HEADER_LEN, FileHeader, FileReader, HEADER_LEN, MAX_FLUSH, MAX_FRAGMENT, Record,
    RecordType, Step, WalError, encode, seeded_crc,
};

const EPOCH: u64 = 5;
const FILE_NO: u64 = 7;

fn header(epoch: u64) -> FileHeader {
    FileHeader {
        shard: 1,
        file_no: FILE_NO,
        node: NodeId([1; 16]),
        created_at: 0,
        recycle_epoch: epoch,
        recycled: epoch > 0,
    }
}

fn record(lsn: u64, payload: Vec<u8>) -> Record<'static> {
    Record {
        kind: RecordType::KvEffects,
        partition: PartitionId(2),
        lsn: Lsn(lsn),
        term: 0,
        timestamp: 100 + lsn,
        payload: Cow::Owned(payload),
    }
}

/// A file of `records`, and the offset where each frame starts.
fn file(records: &[Record<'_>]) -> (Vec<u8>, Vec<usize>) {
    let mut bytes = header(EPOCH).encode().to_vec();
    let mut starts = Vec::new();
    for r in records {
        starts.push(bytes.len());
        encode(r, EPOCH, FILE_NO, &mut bytes);
    }
    (bytes, starts)
}

/// `(lsn, payload)` of a frame.
type Read = (u64, Vec<u8>);

/// Every frame until the end, and the final step.
fn read_all(bytes: &[u8]) -> (Vec<Read>, Result<Step<'_>, WalError>) {
    let mut reader = FileReader::new(bytes).expect("header");
    let mut frames = Vec::new();
    loop {
        match reader.next_step() {
            Ok(Step::Frame(f)) => frames.push((f.lsn.0, f.payload.to_vec())),
            other => return (frames, other),
        }
    }
}

fn small(n: u64) -> Vec<Record<'static>> {
    (1..=n).map(|i| record(i, vec![i as u8; 8])).collect()
}

fn expected(records: &[Record<'_>]) -> Vec<Read> {
    records
        .iter()
        .map(|r| (r.lsn.0, r.payload.to_vec()))
        .collect()
}

#[test]
fn reads_back_what_was_written() {
    let records = small(3);
    let (mut bytes, _) = file(&records);
    let mut padding = record(0, vec![0; 16]);
    padding.kind = RecordType::Padding;
    encode(&padding, EPOCH, FILE_NO, &mut bytes);
    let (frames, end) = read_all(&bytes);
    assert_eq!(frames, expected(&records), "padding is skipped");
    assert_eq!(
        end,
        Ok(Step::End {
            valid_len: bytes.len() as u64
        })
    );
    let reader = FileReader::new(&bytes).expect("header");
    assert_eq!(reader.header(), &header(EPOCH));
}

#[test]
fn zeros_after_the_log_are_the_end() {
    let (mut bytes, _) = file(&small(3));
    let valid = bytes.len() as u64;
    bytes.resize(8 << 20, 0);
    let (frames, end) = read_all(&bytes);
    assert_eq!(frames.len(), 3);
    assert_eq!(end, Ok(Step::End { valid_len: valid }));
}

#[test]
fn old_epoch_is_the_end() {
    let (mut bytes, _) = file(&small(3));
    let valid = bytes.len() as u64;
    // Three MiB of records from before the file was recycled.
    for i in 0..3 {
        encode(
            &record(100 + i, vec![1; MAX_FRAGMENT]),
            EPOCH - 1,
            FILE_NO,
            &mut bytes,
        );
    }
    let (frames, end) = read_all(&bytes);
    assert_eq!(frames.len(), 3);
    assert_eq!(end, Ok(Step::End { valid_len: valid }));
}

#[test]
fn torn_zone_is_the_end() {
    let (mut bytes, starts) = file(&small(5));
    // A torn page in the second frame; valid frames follow closely.
    bytes[starts[1] + 45] ^= 0xFF;
    let (frames, end) = read_all(&bytes);
    assert_eq!(frames, expected(&small(1)));
    assert_eq!(
        end,
        Ok(Step::End {
            valid_len: starts[1] as u64
        })
    );
}

#[test]
fn corruption_beyond_the_torn_zone() {
    let mut records = small(2);
    records.extend((3..6).map(|i| record(i, vec![3; MAX_FRAGMENT])));
    let (mut bytes, starts) = file(&records);
    assert!(bytes.len() - starts[1] > MAX_FLUSH);
    bytes[starts[1] + 45] ^= 0xFF;
    let (frames, end) = read_all(&bytes);
    assert_eq!(frames, expected(&small(1)));
    assert_eq!(
        end,
        Ok(Step::Corrupt {
            offset: starts[1] as u64
        })
    );
}

#[test]
fn absurd_length_is_invalid() {
    let (mut bytes, starts) = file(&small(2));
    bytes[starts[1] + 8..starts[1] + 12].copy_from_slice(&u32::MAX.to_le_bytes());
    let (frames, end) = read_all(&bytes);
    assert_eq!(frames.len(), 1);
    assert_eq!(
        end,
        Ok(Step::End {
            valid_len: starts[1] as u64
        })
    );
    // A length that runs past the end of the file.
    let (mut bytes, starts) = file(&small(2));
    bytes[starts[1] + 8..starts[1] + 12].copy_from_slice(&1000u32.to_le_bytes());
    assert_eq!(
        read_all(&bytes).1,
        Ok(Step::End {
            valid_len: starts[1] as u64
        })
    );
}

#[test]
fn every_flipped_bit_is_caught() {
    let records = small(3);
    let (bytes, _) = file(&records);
    let all = expected(&records);
    for at in HEADER_LEN..bytes.len() {
        for bit in 0..8 {
            let mut flipped = bytes.clone();
            flipped[at] ^= 1 << bit;
            let (frames, end) = read_all(&flipped);
            assert!(all.starts_with(&frames), "byte {at} bit {bit}: {frames:?}");
            assert!(frames.len() < all.len(), "byte {at} bit {bit} not caught");
            assert!(matches!(
                end,
                Ok(Step::End { .. }) | Ok(Step::Corrupt { .. }) | Err(_)
            ));
        }
    }
}

#[test]
fn every_truncation_is_a_prefix() {
    let records = small(4);
    let (bytes, starts) = file(&records);
    let all = expected(&records);
    for cut in HEADER_LEN..=bytes.len() {
        let (frames, end) = read_all(&bytes[..cut]);
        let whole = starts
            .iter()
            .chain([&bytes.len()])
            .skip(1)
            .filter(|end| **end <= cut)
            .count();
        assert_eq!(frames, all[..whole], "cut {cut}");
        let valid = if whole == 0 {
            HEADER_LEN
        } else {
            *starts.get(whole).unwrap_or(&bytes.len())
        };
        assert_eq!(
            end,
            Ok(Step::End {
                valid_len: valid as u64
            }),
            "cut {cut}"
        );
    }
}

#[test]
fn compressed_flag_is_unsupported() {
    let (mut bytes, starts) = file(&small(1));
    let at = starts[0];
    bytes[at + 13] |= 1;
    let header_crc = seeded_crc(EPOCH, FILE_NO, &bytes[at + 8..at + FRAME_HEADER_LEN]);
    bytes[at + 4..at + 8].copy_from_slice(&header_crc.to_le_bytes());
    let crc = seeded_crc(EPOCH, FILE_NO, &bytes[at + 4..]);
    bytes[at..at + 4].copy_from_slice(&crc.to_le_bytes());
    assert!(matches!(read_all(&bytes).1, Err(WalError::Unsupported(_))));
}

#[test]
fn end_is_sticky() {
    let (bytes, _) = file(&small(1));
    let mut reader = FileReader::new(&bytes).expect("header");
    assert!(matches!(reader.next_step(), Ok(Step::Frame(_))));
    let end = reader.next_step();
    assert_eq!(reader.next_step(), end);
}

/// Acknowledged frames after a damaged one are contiguous, so one of them
/// starts within a fragment of `o + MAX_FLUSH`. A lone valid frame further
/// away (bytes a client wrote in an older life of the file) is not proof
/// of corruption, and the scan never looks that far.
#[test]
fn scan_stops_one_fragment_past_the_torn_zone() {
    let (mut bytes, starts) = file(&small(2));
    bytes[starts[1] + 45] ^= 0xFF;
    let far = starts[1] + MAX_FLUSH + MAX_FRAGMENT + FRAME_HEADER_LEN + 1;
    bytes.resize(far, 0);
    encode(&record(9, b"stray".to_vec()), EPOCH, FILE_NO, &mut bytes);
    assert_eq!(
        read_all(&bytes).1,
        Ok(Step::End {
            valid_len: starts[1] as u64
        })
    );
    // At the edge of the window, it is still found.
    let (mut bytes, starts) = file(&small(2));
    bytes[starts[1] + 45] ^= 0xFF;
    bytes.resize(starts[1] + MAX_FLUSH + MAX_FRAGMENT + FRAME_HEADER_LEN, 0);
    encode(&record(9, b"edge".to_vec()), EPOCH, FILE_NO, &mut bytes);
    assert_eq!(
        read_all(&bytes).1,
        Ok(Step::Corrupt {
            offset: starts[1] as u64
        })
    );
}

/// CRCs are seeded by the full epoch and the file number: a frame from an
/// older life of the file, or copied from another file, never reads back,
/// even when the low 16 bits of its epoch match (P-59).
#[test]
fn other_epoch_is_rejected_even_with_same_low_bits() {
    let mut bytes = header(0x1_0005).encode().to_vec();
    let valid = bytes.len() as u64;
    for r in small(3) {
        encode(&r, 5, FILE_NO, &mut bytes);
    }
    let (frames, end) = read_all(&bytes);
    assert!(frames.is_empty(), "{frames:?}");
    assert_eq!(end, Ok(Step::End { valid_len: valid }));
}

#[test]
fn frame_copied_from_another_file_is_rejected() {
    let mut bytes = header(EPOCH).encode().to_vec();
    let valid = bytes.len() as u64;
    for r in small(3) {
        encode(&r, EPOCH, FILE_NO + 1, &mut bytes);
    }
    let (frames, end) = read_all(&bytes);
    assert!(frames.is_empty(), "{frames:?}");
    assert_eq!(end, Ok(Step::End { valid_len: valid }));
}
