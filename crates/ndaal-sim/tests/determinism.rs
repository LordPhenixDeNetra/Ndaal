//! J01 exit criterion: two tasks exchanging messages under network faults,
//! replayed identically from the seed (T01.09 done criterion).

use std::cell::Cell;
use std::future::pending;
use std::rc::Rc;
use std::time::Duration;

use ndaal_io::Clock;
use ndaal_sim::{Limits, Outcome, Sim, ping_pong, with_timeout};

#[test]
fn every_seed_replays_identically_and_completes() {
    let mut partitions = 0;
    let mut reconnects = 0;
    for seed in 0..100 {
        let first = ping_pong(seed, 20);
        let second = ping_pong(seed, 20);
        assert_eq!(first, second, "seed {seed} is not reproducible");
        assert_eq!(first.outcome, Outcome::Finished, "seed {seed}");
        assert_eq!(first.replies, 20, "seed {seed}: missing replies");
        partitions += first.partitions;
        reconnects += first.reconnects;
    }
    assert!(
        partitions > 0,
        "the chaos task never partitioned the network"
    );
    assert!(reconnects > 0, "no client ever had to reconnect");
}

#[test]
fn different_seeds_give_different_traces() {
    assert_ne!(ping_pong(1, 20).fingerprint, ping_pong(2, 20).fingerprint);
}

#[test]
fn timeout_returns_the_value_when_ready_first() {
    let sim = Sim::new(1);
    let got = Rc::new(Cell::new(None));
    {
        let (clock, got) = (sim.clock(), Rc::clone(&got));
        sim.spawn(async move {
            got.set(with_timeout(&clock, Duration::from_secs(1), async { 7 }).await);
        });
    }
    let limits = Limits {
        max_steps: 100,
        max_virtual_time: Duration::from_secs(10),
    };
    assert_eq!(sim.run(limits), Outcome::Finished);
    assert_eq!(got.get(), Some(7));
}

#[test]
fn timeout_gives_up_at_the_deadline() {
    let sim = Sim::new(2);
    let got = Rc::new(Cell::new(Some(())));
    {
        let (clock, got) = (sim.clock(), Rc::clone(&got));
        sim.spawn(async move {
            let waited = with_timeout(&clock, Duration::from_millis(30), pending::<()>()).await;
            got.set(waited);
            assert_eq!(clock.monotonic(), Duration::from_millis(30));
        });
    }
    let limits = Limits {
        max_steps: 100,
        max_virtual_time: Duration::from_secs(10),
    };
    assert_eq!(sim.run(limits), Outcome::Finished);
    assert_eq!(got.get(), None);
}

// Replays must be identical on every machine, not only twice in one process:
// CI runs this on Linux (x86_64, arm64), macOS and Windows. Update the value
// only when a change of simulation behavior is intended, and say so in the
// commit message (a seed from an old CI log would no longer replay).
#[test]
fn seed_42_fingerprint_is_pinned() {
    let report = ping_pong(42, 20);
    assert_eq!(report.outcome, Outcome::Finished);
    assert_eq!(format!("{:016x}", report.fingerprint), "c1dc39e914cc4199");
}
