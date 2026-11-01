//! Encoding and reading back small log records (T02.07): a trend to watch.
//! The writer's throughput goal (T02.08) is measured with the writer.

use std::borrow::Cow;
use std::hint::black_box;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use ndaal_common::ids::{Lsn, NodeId, PartitionId};
use ndaal_wal::{FileHeader, FileReader, Record, RecordType, Step, encode};

const RECORDS: u64 = 10_000;
const PAYLOAD: usize = 100;

fn file() -> Vec<u8> {
    let header = FileHeader {
        shard: 0,
        file_no: 0,
        node: NodeId([0; 16]),
        created_at: 0,
        recycle_epoch: 1,
        recycled: false,
    };
    let mut out = header.encode().to_vec();
    for lsn in 1..=RECORDS {
        encode(&record(lsn), 1, &mut out);
    }
    out
}

fn record(lsn: u64) -> Record<'static> {
    Record {
        kind: RecordType::KvEffects,
        partition: PartitionId(1),
        lsn: Lsn(lsn),
        term: 0,
        timestamp: lsn,
        payload: Cow::Owned(vec![7; PAYLOAD]),
    }
}

fn bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("wal");
    group.throughput(Throughput::Elements(RECORDS));
    let records: Vec<_> = (1..=RECORDS).map(record).collect();
    group.bench_function("encode", |b| {
        let mut out = Vec::with_capacity(4 << 20);
        b.iter(|| {
            out.clear();
            for r in &records {
                encode(r, 1, &mut out);
            }
            black_box(out.len())
        });
    });
    let bytes = file();
    group.bench_function("decode", |b| {
        b.iter(|| {
            let mut reader = FileReader::new(black_box(&bytes)).expect("header");
            let mut n = 0u64;
            while let Ok(Step::Frame(_)) = reader.next_step() {
                n += 1;
            }
            assert_eq!(n, RECORDS);
        });
    });
    group.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);
