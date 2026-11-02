//! The `Partition` abstraction (T02.12, EF-TX-09): LSNs, application of
//! effects, applied and durable LSNs, waiting for an LSN.

mod wal_common;

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use ndaal_common::ids::{Lsn, PartitionId};
use ndaal_io::Clock;
use ndaal_io::sim::SimClock;
use ndaal_observe::metrics::Registry;
use ndaal_sim::{Outcome, Sim};
use ndaal_storage::{Apply, ApplyError, Partition, PartitionError};
use ndaal_wal::{Durability, RecordType, WalWriter};
use wal_common::{LIMITS, Setup, TestDisk, read_back, record};

/// What the test engine applied: `(lsn, payload)`.
type Applied = Rc<RefCell<Vec<(u64, Vec<u8>)>>>;

/// Records every application; fails at `fail_at` if set.
struct TestEngine {
    applied: Applied,
    fail_at: Option<u64>,
}

impl Apply for TestEngine {
    fn apply(&mut self, lsn: Lsn, _kind: RecordType, payload: &[u8]) -> Result<(), ApplyError> {
        if self.fail_at == Some(lsn.0) {
            return Err(ApplyError("test engine fails here".into()));
        }
        self.applied.borrow_mut().push((lsn.0, payload.to_vec()));
        Ok(())
    }
}

type TestPartition = Partition<TestDisk, SimClock, TestEngine>;

fn partition(
    sim: &Sim,
    writer: WalWriter<TestDisk, SimClock>,
    id: u32,
    fail_at: Option<u64>,
) -> (TestPartition, Applied) {
    let applied = Applied::default();
    let engine = TestEngine {
        applied: Rc::clone(&applied),
        fail_at,
    };
    let p = Partition::new(
        PartitionId(id),
        writer,
        engine,
        sim.clock(),
        &Registry::default(),
    )
    .expect("partition");
    (p, applied)
}

fn payload(n: u64) -> Vec<u8> {
    n.to_le_bytes().to_vec()
}

#[test]
fn submits_apply_in_order() {
    let sim = Sim::new(1);
    let setup = Setup::new(&sim, Duration::from_micros(100));
    let (p, applied) = partition(&sim, setup.writer(), 1, None);
    sim.spawn(async move {
        for n in 1..=100 {
            let lsn = p
                .submit(RecordType::KvEffects, &payload(n), Durability::Async)
                .await;
            assert_eq!(lsn, Ok(Lsn(n)));
        }
        assert_eq!(p.applied_lsn(), Some(Lsn(100)));
    });
    assert_eq!(sim.run(LIMITS), Outcome::Finished);
    let got = applied.borrow().clone();
    assert_eq!(got, (1..=100).map(|n| (n, payload(n))).collect::<Vec<_>>());
}

#[test]
fn busy_consumes_no_lsn() {
    let sim = Sim::new(2);
    let setup = Setup::new(&sim, Duration::from_millis(20));
    let writer = setup.writer();
    let filler = writer.clone();
    let (p, applied) = partition(&sim, writer, 1, None);
    sim.spawn(async move {
        assert_eq!(
            p.submit(RecordType::KvEffects, b"a", Durability::Async)
                .await,
            Ok(Lsn(1))
        );
        // Fill the shared log until it answers busy.
        let mut n = 0;
        while filler.append(&record(1_000_000 + n, 512 * 1024)).is_ok() {
            n += 1;
        }
        // Then small ones, until even the smallest record does not fit.
        while filler.append(&record(1_000_000 + n, 0)).is_ok() {
            n += 1;
        }
        let refused = p
            .submit(RecordType::KvEffects, b"b", Durability::Async)
            .await;
        assert_eq!(refused, Err(PartitionError::Busy));
        assert_eq!(p.last_lsn(), Some(Lsn(1)), "a refusal takes no LSN");
        drop(filler);
        // Once the log drained, the next submit gets LSN 2.
        loop {
            match p
                .submit(RecordType::KvEffects, b"c", Durability::Sync)
                .await
            {
                Ok(lsn) => {
                    assert_eq!(lsn, Lsn(2));
                    break;
                }
                Err(PartitionError::Busy) => {
                    sim_sleep(&p).await;
                }
                Err(e) => panic!("{e}"),
            }
        }
    });
    assert_eq!(sim.run(LIMITS), Outcome::Finished);
    let lsns: Vec<u64> = applied.borrow().iter().map(|a| a.0).collect();
    assert_eq!(lsns, [1, 2]);
}

/// Lets virtual time pass, through the partition's clock.
async fn sim_sleep(p: &TestPartition) {
    p.clock().sleep(Duration::from_millis(5)).await;
}

#[test]
fn modes() {
    let sim = Sim::new(3);
    let latency = Duration::from_millis(2);
    let setup = Setup::new(&sim, latency);
    let (p, applied) = partition(&sim, setup.writer(), 7, None);
    let clock = sim.clock();
    let took = Rc::new(RefCell::new(Vec::new()));
    let t = Rc::clone(&took);
    sim.spawn(async move {
        for mode in [Durability::None, Durability::Async, Durability::Sync] {
            let start = clock.monotonic();
            p.submit(RecordType::KvEffects, b"x", mode)
                .await
                .expect("submit");
            t.borrow_mut().push(clock.monotonic() - start);
        }
    });
    assert_eq!(sim.run(LIMITS), Outcome::Finished);
    let took = took.borrow();
    assert_eq!(took[0], Duration::ZERO, "none");
    assert_eq!(took[1], Duration::ZERO, "async");
    assert!(
        took[2] >= latency.mul_f64(0.9),
        "sync answered after {:?}",
        took[2]
    );
    assert_eq!(applied.borrow().len(), 3, "every mode is applied");
    // `None` is not in the log: two records of partition 7 read back.
    assert_eq!(read_back(&setup.disk).len(), 2);
}

#[test]
fn wait_applied_wakes_on_a_future_lsn() {
    let sim = Sim::new(4);
    let setup = Setup::new(&sim, Duration::from_micros(100));
    let (p, _) = partition(&sim, setup.writer(), 1, None);
    let p = Rc::new(p);
    let (waiter, clock) = (Rc::clone(&p), sim.clock());
    let woke_at = Rc::new(RefCell::new(None));
    let w = Rc::clone(&woke_at);
    sim.spawn(async move {
        // Opening the log took virtual time: measure from here.
        let start = clock.monotonic();
        waiter
            .wait_applied(Lsn(5), Duration::from_secs(1))
            .await
            .expect("lsn 5 applied");
        *w.borrow_mut() = Some((clock.monotonic() - start, waiter.applied_lsn()));
    });
    let (writer_p, clock) = (Rc::clone(&p), sim.clock());
    sim.spawn(async move {
        for n in 1..=8 {
            clock.sleep(Duration::from_millis(1)).await;
            writer_p
                .submit(RecordType::KvEffects, &payload(n), Durability::Async)
                .await
                .expect("submit");
        }
    });
    drop(p);
    assert_eq!(sim.run(LIMITS), Outcome::Finished);
    let (at, applied) = woke_at.borrow().expect("woke");
    assert_eq!(at, Duration::from_millis(5), "woke at the fifth submit");
    assert!(applied >= Some(Lsn(5)));
}

#[test]
fn wait_times_out() {
    let sim = Sim::new(5);
    let setup = Setup::new(&sim, Duration::from_micros(100));
    let (p, _) = partition(&sim, setup.writer(), 1, None);
    let clock = sim.clock();
    sim.spawn(async move {
        let start = clock.monotonic();
        let waited = p.wait_applied(Lsn(50), Duration::from_millis(10)).await;
        assert_eq!(waited, Err(PartitionError::Timeout));
        assert_eq!(clock.monotonic() - start, Duration::from_millis(10));
        let waited = p.wait_durable(Lsn(50), Duration::from_millis(10)).await;
        assert_eq!(waited, Err(PartitionError::Timeout));
    });
    assert_eq!(sim.run(LIMITS), Outcome::Finished);
}

#[test]
fn wait_durable_after_the_sync() {
    let sim = Sim::new(6);
    let latency = Duration::from_millis(3);
    let setup = Setup::new(&sim, latency);
    let (p, _) = partition(&sim, setup.writer(), 1, None);
    let clock = sim.clock();
    sim.spawn(async move {
        let lsn = p
            .submit(RecordType::KvEffects, b"x", Durability::Async)
            .await
            .expect("submit");
        assert_eq!(p.durable_lsn(), None, "not durable at once");
        let start = clock.monotonic();
        p.wait_durable(lsn, Duration::from_secs(1))
            .await
            .expect("durable");
        assert!(clock.monotonic() - start >= latency.mul_f64(0.9));
        assert_eq!(p.durable_lsn(), Some(lsn));
    });
    assert_eq!(sim.run(LIMITS), Outcome::Finished);
}

#[test]
fn durable_skips_none_entries() {
    let sim = Sim::new(7);
    let setup = Setup::new(&sim, Duration::from_micros(200));
    let (p, _) = partition(&sim, setup.writer(), 1, None);
    sim.spawn(async move {
        let mut last_durable = None;
        for (n, mode) in [Durability::Async, Durability::None, Durability::Async]
            .into_iter()
            .enumerate()
        {
            p.submit(RecordType::KvEffects, &payload(n as u64), mode)
                .await
                .expect("submit");
            let durable = p.durable_lsn();
            assert!(durable >= last_durable, "durable lsn went back");
            assert!(durable <= p.applied_lsn(), "durable past applied");
            last_durable = durable;
        }
        p.wait_durable(Lsn(3), Duration::from_secs(1))
            .await
            .expect("durable");
        assert_eq!(
            p.durable_lsn(),
            Some(Lsn(3)),
            "the None entry does not hold it back"
        );
    });
    assert_eq!(sim.run(LIMITS), Outcome::Finished);
}

#[test]
fn two_partitions_one_log() {
    let sim = Sim::new(8);
    let setup = Setup::new(&sim, Duration::from_millis(1));
    let writer = setup.writer();
    let (a, _) = partition(&sim, writer.clone(), 1, None);
    let (b, _) = partition(&sim, writer, 2, None);
    sim.spawn(async move {
        let a1 = a
            .submit(RecordType::KvEffects, b"a1", Durability::Sync)
            .await;
        let b1 = b
            .submit(RecordType::KvEffects, b"b1", Durability::Async)
            .await;
        let a2 = a
            .submit(RecordType::KvEffects, b"a2", Durability::Async)
            .await;
        assert_eq!((a1, b1, a2), (Ok(Lsn(1)), Ok(Lsn(1)), Ok(Lsn(2))));
        // A's first record is durable; B's is not yet: A's sync must not
        // make B's LSN 1 durable.
        assert_eq!(a.durable_lsn(), Some(Lsn(1)));
        assert_eq!(b.durable_lsn(), None);
        b.wait_durable(Lsn(1), Duration::from_secs(1))
            .await
            .expect("durable");
        a.wait_durable(Lsn(2), Duration::from_secs(1))
            .await
            .expect("durable");
        assert_eq!(
            (a.durable_lsn(), b.durable_lsn()),
            (Some(Lsn(2)), Some(Lsn(1)))
        );
    });
    assert_eq!(sim.run(LIMITS), Outcome::Finished);
}

#[test]
fn apply_failure_fails_the_partition() {
    let sim = Sim::new(9);
    let setup = Setup::new(&sim, Duration::from_micros(100));
    let registry = Registry::default();
    let applied = Applied::default();
    let engine = TestEngine {
        applied: Rc::clone(&applied),
        fail_at: Some(3),
    };
    let p = Rc::new(
        Partition::new(
            PartitionId(4),
            setup.writer(),
            engine,
            sim.clock(),
            &registry,
        )
        .expect("partition"),
    );
    let waiter = Rc::clone(&p);
    let waited = Rc::new(RefCell::new(None));
    let w = Rc::clone(&waited);
    sim.spawn(async move {
        *w.borrow_mut() = Some(waiter.wait_applied(Lsn(10), Duration::from_secs(1)).await);
    });
    sim.spawn(async move {
        for n in 1..=2 {
            p.submit(RecordType::KvEffects, &payload(n), Durability::Async)
                .await
                .expect("submit");
        }
        let third = p
            .submit(RecordType::KvEffects, b"3", Durability::Async)
            .await;
        assert_eq!(third, Err(PartitionError::Failed));
        let fourth = p
            .submit(RecordType::KvEffects, b"4", Durability::Async)
            .await;
        assert_eq!(fourth, Err(PartitionError::Failed));
    });
    assert_eq!(sim.run(LIMITS), Outcome::Finished);
    assert_eq!(*waited.borrow(), Some(Err(PartitionError::Failed)));
    assert_eq!(applied.borrow().len(), 2);
    let metrics = registry.render();
    assert!(
        metrics.contains("ndaal_partition_failed_total{partition=\"4\"} 1"),
        "{metrics}"
    );
}

#[test]
fn resume_continues_after_the_given_lsn() {
    let sim = Sim::new(10);
    let setup = Setup::new(&sim, Duration::from_micros(100));
    let (p, applied) = partition(&sim, setup.writer(), 1, None);
    sim.spawn(async move {
        p.resume(Lsn(41));
        assert_eq!(p.applied_lsn(), Some(Lsn(41)));
        let lsn = p
            .submit(RecordType::KvEffects, b"x", Durability::Sync)
            .await;
        assert_eq!(lsn, Ok(Lsn(42)));
        assert_eq!(p.durable_lsn(), Some(Lsn(42)));
    });
    assert_eq!(sim.run(LIMITS), Outcome::Finished);
    assert_eq!(applied.borrow()[0].0, 42);
}

mod property {
    use proptest::prelude::*;

    use super::*;

    fn mode() -> impl Strategy<Value = Durability> {
        prop_oneof![
            Just(Durability::None),
            Just(Durability::Async),
            Just(Durability::Sync)
        ]
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(32))]
        /// Random submits and waits: the durable LSN never passes the
        /// applied one, and every acknowledged `Sync` record is durable.
        #[test]
        fn random_submits_and_waits(
            seed in any::<u64>(),
            steps in proptest::collection::vec((mode(), 0u64..300), 1..60),
        ) {
            let sim = Sim::new(seed);
            let setup = Setup::new(&sim, Duration::from_micros(300));
            let (p, _) = partition(&sim, setup.writer(), 1, None);
            let clock = sim.clock();
            sim.spawn(async move {
                for (mode, pause) in steps {
                    let lsn = p.submit(RecordType::KvEffects, b"r", mode).await.expect("submit");
                    assert!(p.durable_lsn() <= p.applied_lsn());
                    if mode == Durability::Sync {
                        assert!(p.durable_lsn() >= Some(lsn), "sync {lsn:?} not durable");
                    }
                    if pause % 3 == 0 {
                        p.wait_durable(lsn, Duration::from_secs(1)).await.expect("durable");
                    }
                    clock.sleep(Duration::from_micros(pause)).await;
                }
            });
            prop_assert_eq!(sim.run(LIMITS), Outcome::Finished);
        }
    }
}

#[test]
fn log_failure_during_sync_fails_the_partition() {
    let sim = Sim::new(11);
    let setup = Setup::new(&sim, Duration::from_micros(100));
    let registry = Registry::default();
    let engine = TestEngine {
        applied: Applied::default(),
        fail_at: None,
    };
    let p = Partition::new(
        PartitionId(5),
        setup.writer(),
        engine,
        sim.clock(),
        &registry,
    )
    .expect("partition");
    setup.fail_next_write(std::io::ErrorKind::Other);
    sim.spawn(async move {
        let first = p
            .submit(RecordType::KvEffects, b"x", Durability::Sync)
            .await;
        assert_eq!(
            first,
            Err(PartitionError::Failed),
            "same error as wait_durable"
        );
        let next = p
            .submit(RecordType::KvEffects, b"y", Durability::Async)
            .await;
        assert_eq!(next, Err(PartitionError::Failed));
    });
    assert_eq!(sim.run(LIMITS), Outcome::Finished);
    let metrics = registry.render();
    assert!(
        metrics.contains("ndaal_partition_failed_total{partition=\"5\"} 1"),
        "{metrics}"
    );
}

#[test]
fn lsn_overflow_fails_instead_of_panicking() {
    let sim = Sim::new(12);
    let setup = Setup::new(&sim, Duration::from_micros(100));
    let (p, applied) = partition(&sim, setup.writer(), 1, None);
    sim.spawn(async move {
        // An LSN read from disk may be anything.
        p.resume(Lsn(u64::MAX));
        let next = p
            .submit(RecordType::KvEffects, b"x", Durability::Async)
            .await;
        assert_eq!(next, Err(PartitionError::Failed));
    });
    assert_eq!(sim.run(LIMITS), Outcome::Finished);
    assert!(applied.borrow().is_empty());
}

#[test]
fn durable_gauge_follows_without_a_reader() {
    let sim = Sim::new(13);
    let setup = Setup::new(&sim, Duration::from_micros(100));
    let registry = Registry::default();
    let engine = TestEngine {
        applied: Applied::default(),
        fail_at: None,
    };
    let p = Partition::new(
        PartitionId(6),
        setup.writer(),
        engine,
        sim.clock(),
        &registry,
    )
    .expect("partition");
    sim.spawn(async move {
        for n in 1..=3 {
            p.submit(RecordType::KvEffects, &payload(n), Durability::Sync)
                .await
                .expect("submit");
        }
    });
    assert_eq!(sim.run(LIMITS), Outcome::Finished);
    let metrics = registry.render();
    assert!(
        metrics.contains("ndaal_wal_durable_lsn{partition=\"6\"} 3"),
        "{metrics}"
    );
}
