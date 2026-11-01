//! Records and their frames (FORMATS_DE_DONNEES.md sections 2.3 and 2.4,
//! T02.07).

use std::borrow::Cow;

use ndaal_common::ids::{Lsn, PartitionId};
use ndaal_wal::{FRAME_HEADER_LEN, MAX_FRAGMENT, Record, RecordType, encode, encoded_len};

const FIRST: u8 = 1 << 3;
const LAST: u8 = 1 << 2;

fn record(payload: Vec<u8>) -> Record<'static> {
    Record {
        kind: RecordType::KvEffects,
        partition: PartitionId(9),
        lsn: Lsn(42),
        term: 3,
        timestamp: 1_767_225_600_000_000,
        payload: Cow::Owned(payload),
    }
}

/// `(offset, len, flags)` of each frame of `bytes`.
fn frames(bytes: &[u8]) -> Vec<(usize, usize, u8)> {
    let mut out = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        let len = u32::from_le_bytes(bytes[at + 4..at + 8].try_into().expect("4 bytes")) as usize;
        out.push((at, len, bytes[at + 9]));
        at += FRAME_HEADER_LEN + len;
    }
    out
}

#[test]
fn small_record_is_one_frame() {
    let mut out = Vec::new();
    encode(&record(b"hello".to_vec()), 1, &mut out);
    assert_eq!(out.len(), FRAME_HEADER_LEN + 5);
    assert_eq!(frames(&out), [(0, 5, FIRST | LAST)]);
    assert_eq!(out[8], RecordType::KvEffects.code());
    let mut empty = Vec::new();
    encode(&record(Vec::new()), 1, &mut empty);
    assert_eq!(frames(&empty), [(0, 0, FIRST | LAST)]);
}

#[test]
fn fragmentation_boundaries() {
    for (len, count) in [
        (MAX_FRAGMENT, 1),
        (MAX_FRAGMENT + 1, 2),
        (3 * MAX_FRAGMENT, 3),
    ] {
        let mut out = Vec::new();
        encode(&record(vec![7; len]), 1, &mut out);
        let f = frames(&out);
        assert_eq!(f.len(), count, "{len}");
        assert_eq!(out.len(), encoded_len(len), "{len}");
        assert!(f.iter().all(|(_, l, _)| *l <= MAX_FRAGMENT), "{len}");
        assert_eq!(f.iter().map(|(_, l, _)| l).sum::<usize>(), len);
        assert_ne!(f[0].2 & FIRST, 0);
        assert_ne!(f[count - 1].2 & LAST, 0);
        if count > 1 {
            assert_eq!(f[0].2 & LAST, 0);
            assert_eq!(f[count - 1].2 & FIRST, 0);
        }
    }
}

#[test]
fn epoch_low_bits_in_frame() {
    let mut out = Vec::new();
    encode(&record(b"x".to_vec()), 0x1_0005, &mut out);
    assert_eq!(u16::from_le_bytes([out[10], out[11]]), 0x0005);
}

#[test]
fn codes_round_trip() {
    for kind in RecordType::ALL {
        assert_eq!(RecordType::from_code(kind.code()), Some(kind));
    }
    assert_eq!(RecordType::from_code(0), None);
    assert_eq!(RecordType::Padding.code(), 0xF0);
    assert_eq!(RecordType::Tx.code(), 0x04);
}
