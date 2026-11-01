//! Compatibility corpus (ENF-COMPAT-04, FORMATS_DE_DONNEES.md section 14):
//! a log file written by format version 1 must always read back the same.
//! The file is versioned; never regenerate it, add a new one per version.

use ndaal_common::ids::NodeId;
use ndaal_wal::{FORMAT, FileHeader, FileReader, Reassembler, RecordType, Step};

const V1: &[u8] = include_bytes!("data/v1.wal");

#[test]
fn version_1_file_reads_back() {
    assert_eq!(FORMAT, 1);
    let mut reader = FileReader::new(V1).expect("header");
    assert_eq!(
        reader.header(),
        &FileHeader {
            shard: 3,
            file_no: 0x2a,
            node: NodeId(*b"ndaal-compat-v1!"),
            created_at: 1_767_225_600_000_000,
            recycle_epoch: 2,
            recycled: true,
        }
    );
    let mut glue = Reassembler::default();
    let mut got = Vec::new();
    let end = loop {
        match reader.next_step().expect("readable") {
            Step::Frame(frame) => {
                let record = glue.push(frame).expect("glue").expect("whole record");
                got.push((
                    record.kind,
                    record.partition.0,
                    record.lsn.0,
                    record.timestamp - 1_767_225_600_000_000,
                    record.payload.into_owned(),
                ));
            }
            other => break other,
        }
    };
    let expected = vec![
        (RecordType::KvEffects, 1, 1, 1, b"set k v".to_vec()),
        (RecordType::Tx, 2, 1, 1, vec![0xAB; 3000]),
        (RecordType::CheckpointBegin, 1, 2, 2, Vec::new()),
        (RecordType::KvEffects, 1, 3, 3, (0u8..=255).collect()),
    ];
    assert_eq!(got, expected, "the padding record is skipped");
    assert_eq!(
        end,
        Step::End {
            valid_len: V1.len() as u64
        }
    );
}
