//! Durability modes of the log (J02-C2, T02.11): when each mode answers.

mod wal_common;

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use ndaal_io::Clock;
use ndaal_sim::{Outcome, Sim};
use ndaal_wal::Durability;
use wal_common::{LIMITS, Setup, read_back, record};

#[test]
fn modes_ack_at_the_right_time() {
    let sim = Sim::new(1);
    let latency = Duration::from_millis(2);
    let setup = Setup::new(&sim, latency);
    let (writer, clock) = (setup.writer(), sim.clock());
    let took = Rc::new(Cell::new([Duration::ZERO; 3]));
    let t = Rc::clone(&took);
    sim.spawn(async move {
        let mut times = [Duration::ZERO; 3];
        for (i, mode) in [Durability::None, Durability::Async, Durability::Sync]
            .into_iter()
            .enumerate()
        {
            let start = clock.monotonic();
            let at = writer
                .submit(&record(i as u64 + 1, 10), mode)
                .await
                .expect("submit");
            assert_eq!(at.is_none(), mode == Durability::None, "{mode:?}");
            times[i] = clock.monotonic() - start;
        }
        t.set(times);
    });
    assert_eq!(sim.run(LIMITS), Outcome::Finished);
    let [none, asynchronous, sync] = took.get();
    assert_eq!(none, Duration::ZERO);
    assert_eq!(asynchronous, Duration::ZERO);
    assert!(sync >= latency.mul_f64(0.9), "sync answered after {sync:?}");
    // `None` wrote nothing; the two others are in the log.
    let lsns: Vec<u64> = read_back(&setup.disk).iter().map(|r| r.0).collect();
    assert_eq!(lsns, [2, 3]);
}

#[test]
fn lonely_async_reaches_disk_within_the_interval() {
    let sim = Sim::new(2);
    let setup = Setup::new(&sim, Duration::from_micros(200));
    let (writer, clock) = (setup.writer(), sim.clock());
    let durable_in_time = Rc::new(Cell::new(false));
    let d = Rc::clone(&durable_in_time);
    sim.spawn(async move {
        let at = writer
            .submit(&record(1, 10), Durability::Async)
            .await
            .expect("submit")
            .expect("logged");
        // Nobody waits for it: the flusher may batch, but not past the
        // interval (100 ms) plus one flush.
        clock.sleep(Duration::from_millis(105)).await;
        d.set(writer.durable_position().is_some_and(|p| p >= at));
    });
    assert_eq!(sim.run(LIMITS), Outcome::Finished);
    assert!(
        durable_in_time.get(),
        "async record not on disk after the interval"
    );
}
