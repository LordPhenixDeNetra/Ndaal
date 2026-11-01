//! Gluing fragments back into records, across files (FORMATS_DE_DONNEES.md
//! section 2.3, T02.07).

use std::borrow::Cow;

use ndaal_common::ids::{Lsn, NodeId, PartitionId};
use ndaal_wal::{
    FileHeader, FileReader, FragmentError, Frame, MAX_FRAGMENT, Reassembler, Record, RecordType,
    Step, WalError, encode,
};
use proptest::prelude::*;

const EPOCH: u64 = 1;

fn record(lsn: u64, payload: Vec<u8>) -> Record<'static> {
    Record {
        kind: RecordType::Tx,
        partition: PartitionId(4),
        lsn: Lsn(lsn),
        term: 2,
        timestamp: 77,
        payload: Cow::Owned(payload),
    }
}

fn header(file_no: u64) -> Vec<u8> {
    FileHeader {
        shard: 0,
        file_no,
        node: NodeId([0; 16]),
        created_at: 0,
        recycle_epoch: EPOCH,
        recycled: false,
    }
    .encode()
    .to_vec()
}

fn frames(bytes: &[u8]) -> Vec<Frame<'_>> {
    let mut reader = FileReader::new(bytes).expect("header");
    let mut out = Vec::new();
    while let Ok(Step::Frame(f)) = reader.next_step() {
        out.push(f);
    }
    out
}

/// Records written one after the other, split into files of `file_len`
/// bytes at most (a record may straddle two files).
fn files(records: &[Record<'_>], file_len: usize) -> Vec<Vec<u8>> {
    let mut stream = Vec::new();
    for r in records {
        encode(r, EPOCH, &mut stream);
    }
    let mut out = vec![header(0)];
    let mut at = 0;
    while at < stream.len() {
        // Cut only between frames.
        let len = u32::from_le_bytes(stream[at + 4..at + 8].try_into().expect("4 bytes")) as usize;
        let frame = &stream[at..at + 40 + len];
        if out.last().is_some_and(|f| f.len() + frame.len() > file_len) {
            out.push(header(out.len() as u64));
        }
        out.last_mut().expect("a file").extend_from_slice(frame);
        at += 40 + len;
    }
    out
}

fn read_back(files: &[Vec<u8>]) -> Vec<Record<'static>> {
    let mut glue = Reassembler::default();
    let mut out = Vec::new();
    for file in files {
        for frame in frames(file) {
            if let Some(r) = glue.push(frame).expect("glue") {
                out.push(Record {
                    payload: Cow::Owned(r.payload.into_owned()),
                    ..r
                });
            }
        }
    }
    assert!(!glue.pending());
    out
}

#[test]
fn whole_frame_is_borrowed() {
    let mut bytes = header(0);
    encode(&record(1, b"abc".to_vec()), EPOCH, &mut bytes);
    let mut glue = Reassembler::default();
    let r = glue.push(frames(&bytes)[0]).expect("glue").expect("record");
    assert!(matches!(r.payload, Cow::Borrowed(b"abc")));
}

#[test]
fn fragments_across_two_files() {
    let big = record(1, (0..(5 * MAX_FRAGMENT / 2)).map(|i| i as u8).collect());
    let files = files(std::slice::from_ref(&big), 2 * MAX_FRAGMENT);
    assert_eq!(files.len(), 2, "the record straddles two files");
    assert_eq!(read_back(&files), [big]);
}

#[test]
fn stray_and_duplicate_fragments() {
    let mut bytes = header(0);
    encode(&record(1, vec![1; 2 * MAX_FRAGMENT]), EPOCH, &mut bytes);
    let f = frames(&bytes);
    let unexpected = Err(WalError::Fragment(FragmentError::Unexpected));
    // A last fragment without its first one.
    assert_eq!(Reassembler::default().push(f[1]), unexpected);
    // A first fragment while a record is in progress.
    let mut glue = Reassembler::default();
    assert_eq!(glue.push(f[0]), Ok(None));
    assert!(glue.pending());
    assert_eq!(glue.push(f[0]), unexpected);
}

#[test]
fn mismatched_fragment() {
    let mut a = header(0);
    encode(&record(1, vec![1; 2 * MAX_FRAGMENT]), EPOCH, &mut a);
    let mut b = header(0);
    encode(&record(2, vec![1; 2 * MAX_FRAGMENT]), EPOCH, &mut b);
    let mut glue = Reassembler::default();
    assert_eq!(glue.push(frames(&a)[0]), Ok(None));
    assert_eq!(
        glue.push(frames(&b)[1]),
        Err(WalError::Fragment(FragmentError::Mismatch))
    );
}

fn any_record() -> impl Strategy<Value = Record<'static>> {
    (
        proptest::sample::select(RecordType::ALL.to_vec())
            .prop_filter("padding is skipped by readers", |k| {
                *k != RecordType::Padding
            }),
        any::<u32>(),
        any::<u64>(),
        any::<u64>(),
        any::<u64>(),
        prop_oneof![
            proptest::collection::vec(any::<u8>(), 0..64),
            (0usize..3 * MAX_FRAGMENT + 7).prop_map(|n| vec![0xA5; n]),
        ],
    )
        .prop_map(|(kind, p, lsn, term, timestamp, payload)| Record {
            kind,
            partition: PartitionId(p),
            lsn: Lsn(lsn),
            term,
            timestamp,
            payload: Cow::Owned(payload),
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]
    #[test]
    fn round_trip(records in proptest::collection::vec(any_record(), 0..6)) {
        let files = files(&records, 3 * MAX_FRAGMENT);
        prop_assert_eq!(read_back(&files), records);
    }
}
