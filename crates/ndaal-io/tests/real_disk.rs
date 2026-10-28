//! Real implementations against the shared contract (T01.07).

mod common;

use ndaal_io::real::{OsRng, RealDisk, SystemClock};
use ndaal_io::{Clock, Rng};

#[tokio::test(flavor = "current_thread")]
async fn real_disk_meets_the_contract() {
    let dir = common::test_dir();
    common::disk_contract(&RealDisk, dir.path()).await;
}

#[test]
fn system_clock_is_monotonic_and_after_2026() {
    let clock = SystemClock::new();
    let a = clock.monotonic();
    let b = clock.monotonic();
    assert!(b >= a);
    // 2026-01-01T00:00:00Z in microseconds.
    assert!(clock.unix_micros() > 1_767_225_600_000_000);
}

#[test]
fn os_rng_instances_differ() {
    let mut a = OsRng::new().expect("rng");
    let mut b = OsRng::new().expect("rng");
    let sa: Vec<u64> = (0..4).map(|_| a.next_u64()).collect();
    let sb: Vec<u64> = (0..4).map(|_| b.next_u64()).collect();
    assert_ne!(sa, sb);
}
