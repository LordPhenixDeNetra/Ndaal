//! The log writer on a simulated device (J02-C2, T02.08): appends never
//! wait, one flush at a time, durability only after the sync.

mod wal_common;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use ndaal_io::Clock;
use ndaal_sim::{Outcome, Sim};
use ndaal_wal::{MAX_FLUSH, WalError};
use proptest::prelude::*;
use wal_common::{LIMITS, Recorder, Setup, read_back, record};

#[test]
fn appended_records_read_back() {
    let sim = Sim::new(1);
    let setup = Setup::new(&sim, Duration::from_micros(100));
    let writer = setup.writer();
    sim.spawn(async move {
        let mut last = None;
        for lsn in 1..=50 {
            last = Some(writer.append(&record(lsn, 100)).expect("append"));
        }
        writer
            .durable(last.expect("appended"))
            .await
            .expect("durable");
    });
    assert_eq!(sim.run(LIMITS), Outcome::Finished);
    let lsns: Vec<u64> = read_back(&setup.disk).iter().map(|r| r.0).collect();
    assert_eq!(lsns, (1..=50).collect::<Vec<_>>());
}

#[test]
fn durable_waits_for_the_sync() {
    let sim = Sim::new(2);
    let latency = Duration::from_millis(5);
    let setup = Setup::new(&sim, latency);
    let (writer, clock) = (setup.writer(), sim.clock());
    let done_at = Rc::new(Cell::new(Duration::ZERO));
    let d = Rc::clone(&done_at);
    sim.spawn(async move {
        let at = writer.append(&record(1, 10)).expect("append");
        let start = clock.monotonic();
        writer.durable(at).await.expect("durable");
        d.set(clock.monotonic() - start);
    });
    assert_eq!(sim.run(LIMITS), Outcome::Finished);
    assert!(
        done_at.get() >= latency.mul_f64(0.9), // the device varies by up to 10 %
        "durable after {:?}",
        done_at.get()
    );
}

#[test]
fn append_never_waits_during_a_flush() {
    let sim = Sim::new(3);
    let setup = Setup::new(&sim, Duration::from_millis(1));
    let (writer, clock) = (setup.writer(), sim.clock());
    let neighbour_ran = Rc::new(Cell::new(false));
    let n = Rc::clone(&neighbour_ran);
    sim.spawn(async move {
        n.set(true);
    });
    sim.spawn(async move {
        let first = writer.append(&record(1, 100)).expect("append");
        let waiting = writer.durable(first);
        let before = clock.monotonic();
        for lsn in 2..=1000 {
            writer.append(&record(lsn, 100)).expect("append");
        }
        assert_eq!(clock.monotonic(), before, "appends took virtual time");
        waiting.await.expect("durable");
    });
    assert_eq!(sim.run(LIMITS), Outcome::Finished);
    assert!(neighbour_ran.get());
}

#[test]
fn last_append_before_flush_is_not_lost() {
    let sim = Sim::new(4);
    let setup = Setup::new(&sim, Duration::from_millis(1));
    let writer = setup.writer();
    sim.spawn(async move {
        let a = writer.append(&record(1, 10)).expect("append");
        let wait_a = writer.durable(a);
        // Lands in the other buffer while the first flush is in flight.
        let b = writer.append(&record(2, 10)).expect("append");
        wait_a.await.expect("durable a");
        writer.durable(b).await.expect("durable b");
    });
    assert_eq!(sim.run(LIMITS), Outcome::Finished);
    assert_eq!(read_back(&setup.disk).len(), 2);
}

#[test]
fn failed_sync_fails_waiters_and_appends() {
    let sim = Sim::new(5);
    let setup = Setup::new(&sim, Duration::from_micros(100));
    let writer = setup.writer();
    setup.fail_next_write(std::io::ErrorKind::Other);
    let result = Rc::new(RefCell::new(None));
    let r = Rc::clone(&result);
    sim.spawn(async move {
        let at = writer.append(&record(1, 10)).expect("append");
        let waited = writer.durable(at).await;
        let after = writer.append(&record(2, 10));
        *r.borrow_mut() = Some((waited, after.err()));
    });
    assert_eq!(sim.run(LIMITS), Outcome::Finished);
    let (waited, after) = result.borrow_mut().take().expect("ran");
    assert!(matches!(waited, Err(WalError::Failed)), "{waited:?}");
    assert_eq!(after, Some(WalError::Failed));
    // The error is not swallowed: counted (and logged).
    let metrics = setup.registry.render();
    assert!(
        metrics.contains("ndaal_wal_flush_errors_total{shard=\"0\"} 1"),
        "{metrics}"
    );
}

#[test]
fn busy_beyond_the_bound() {
    let sim = Sim::new(6);
    let setup = Setup::new(&sim, Duration::from_millis(50));
    let writer = setup.writer();
    let busy = Rc::new(Cell::new(false));
    let b = Rc::clone(&busy);
    sim.spawn(async move {
        for lsn in 1.. {
            match writer.append(&record(lsn, 64 * 1024)) {
                Ok(_) => {}
                Err(WalError::Busy) => break,
                Err(e) => panic!("{e}"),
            }
            assert!(lsn < 1000, "never busy");
        }
        b.set(true);
    });
    sim.run(LIMITS);
    assert!(busy.get());
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(24))]
    /// One flush at a time, each at most MAX_FLUSH bytes from a page
    /// boundary, ending on a frame boundary then zeros to the page; every
    /// position reported durable was covered by a finished write and sync.
    #[test]
    fn flush_rules(
        seed in any::<u64>(),
        sizes in proptest::collection::vec(0usize..300_000, 1..60),
        pauses in proptest::collection::vec(0u64..400, 1..60),
    ) {
        let sim = Sim::new(seed);
        let recorder = Recorder::default();
        let setup = Setup::recording(&sim, Duration::from_micros(300), recorder.clone());
        let (writer, clock) = (setup.writer(), sim.clock());
        let rec = recorder.clone();
        sim.spawn(async move {
            let mut last = None;
            for (i, size) in sizes.iter().enumerate() {
                let at = writer.append(&record(i as u64 + 1, *size)).expect("append");
                last = Some(at);
                if i % 3 == 0 {
                    let durable = writer.durable(at).await;
                    prop_assert_ok(durable);
                    rec.check_durable(at);
                }
                let pause = pauses[i % pauses.len()];
                clock.sleep(Duration::from_micros(pause)).await;
            }
            writer.durable(last.expect("one")).await.expect("durable");
        });
        prop_assert_eq!(sim.run(LIMITS), Outcome::Finished);
        recorder.check_writes(MAX_FLUSH);
    }
}

fn prop_assert_ok(result: Result<(), WalError>) {
    assert!(result.is_ok(), "{result:?}");
}

/// A record larger than the pending bound is accepted when nothing else is
/// pending: `Busy` means "retry later", so it must not answer it forever.
#[test]
fn oversized_record_is_accepted_when_alone() {
    let sim = Sim::new(7);
    let setup = Setup::new(&sim, Duration::from_micros(100));
    let writer = setup.writer();
    sim.spawn(async move {
        let at = writer.append(&record(1, 5 << 20)).expect("accepted alone");
        writer.durable(at).await.expect("durable");
    });
    assert_eq!(sim.run(LIMITS), Outcome::Finished);
    assert_eq!(read_back(&setup.disk), [(1, 5 << 20)]);
}
