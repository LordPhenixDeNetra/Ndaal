//! Throughput of the log writer on the real I/O pool (T02.08): small
//! records appended as `Async`, then the last one awaited. The criterion of
//! T02.08 (500,000 records per second per shard) is judged on the
//! reference machine (D-05); this bench gives the local trend.

use std::borrow::Cow;
use std::hint::black_box;
use std::time::Duration;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use ndaal_common::ids::{Lsn, NodeId, PartitionId};
use ndaal_io::real::{IoPool, PoolDisk, SystemClock};
use ndaal_io::sched::{IoClass, SchedulerConfig};
use ndaal_observe::metrics::Registry;
use ndaal_wal::{Record, RecordType, WalWriter, WriterConfig};

const RECORDS: u64 = 100_000;
const PAYLOAD: usize = 100;

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
    let registry = Registry::default();
    let scheduler = SchedulerConfig {
        depth: 32,
        background_share: 0.3,
    };
    let pool = IoPool::start("bench", scheduler, &[], &registry).expect("pool");
    let dir = tempfile::tempdir().expect("tempdir");
    let config = WriterConfig {
        file_size: 64 << 20,
        group_commit_max_bytes: 256 * 1024,
        group_commit_max_delay: Duration::from_micros(200),
        async_flush_interval: Duration::from_millis(100),
    };
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .expect("runtime");
    let local = tokio::task::LocalSet::new();
    let disk = PoolDisk::new(pool.submitter(), IoClass::Critical);
    let (writer, flusher) = local
        .block_on(&runtime, async {
            WalWriter::open(
                disk,
                dir.path(),
                0,
                NodeId([0; 16]),
                config,
                SystemClock::new(),
                &registry,
            )
            .await
        })
        .expect("open the log");
    local.spawn_local(flusher.run());
    let records: Vec<Record<'static>> = (1..=RECORDS).map(record).collect();
    let mut group = c.benchmark_group("wal");
    group.throughput(Throughput::Elements(RECORDS));
    group.sample_size(10);
    group.bench_function("append", |b| {
        b.iter(|| {
            local.block_on(&runtime, async {
                let mut last = None;
                for r in &records {
                    loop {
                        match writer.append(r) {
                            Ok(at) => {
                                last = Some(at);
                                break;
                            }
                            // Busy: let the flusher catch up.
                            Err(_) => {
                                if let Some(at) = last {
                                    writer.durable(at).await.expect("durable");
                                }
                            }
                        }
                    }
                }
                writer
                    .durable(black_box(last.expect("appended")))
                    .await
                    .expect("durable");
            });
        });
    });
    group.finish();
    drop(writer);
    local.block_on(&runtime, async {});
    pool.shutdown().expect("shutdown");
}

criterion_group!(benches, bench);
criterion_main!(benches);
