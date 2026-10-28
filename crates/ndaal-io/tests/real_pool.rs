//! The I/O thread pool of a device (T02.06): the disk contract through the
//! scheduler, priorities on real threads, bounded queues and shutdown.

mod common;

use std::io::ErrorKind;
use std::sync::mpsc;
use std::sync::{Arc, Mutex, PoisonError};

use ndaal_io::real::{IoKind, IoPool, PoolDisk};
use ndaal_io::sched::{IoClass, SchedulerConfig};
use ndaal_observe::metrics::Registry;

const MIB: u64 = 1024 * 1024;

fn run<F: std::future::Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .expect("runtime")
        .block_on(future)
}

fn pool(depth: usize, registry: &Registry) -> IoPool {
    let config = SchedulerConfig {
        depth,
        background_share: 0.3,
    };
    IoPool::start("test", config, &[], registry).expect("start")
}

/// An operation that tells when it starts, then holds its I/O thread until
/// the returned sender sends or is dropped; it succeeds.
struct Blocker {
    release: mpsc::Sender<()>,
    started: mpsc::Receiver<()>,
}

fn blocker() -> (
    Blocker,
    impl FnOnce() -> std::io::Result<()> + Send + 'static,
) {
    let (release, wait) = mpsc::channel::<()>();
    let (start, started) = mpsc::channel::<()>();
    let job = move || {
        let _ = start.send(());
        let _ = wait.recv();
        Ok(())
    };
    (Blocker { release, started }, job)
}

#[test]
fn disk_contract_on_the_pool() {
    let registry = Registry::default();
    let pool = pool(4, &registry);
    let dir = common::test_dir();
    for class in [IoClass::Critical, IoClass::High, IoClass::Background] {
        let root = dir.path().join(class.name());
        run(async {
            let disk = PoolDisk::new(pool.submitter(), class);
            ndaal_io::Disk::create_dir_all(&disk, &root)
                .await
                .expect("mkdir");
            common::disk_contract(&disk, &root).await;
        });
    }
    pool.shutdown().expect("shutdown");
}

#[test]
fn high_overtakes_queued_low() {
    let registry = Registry::default();
    let pool = pool(1, &registry);
    let submitter = pool.submitter();
    let order = Arc::new(Mutex::new(Vec::new()));
    let log = |label: &'static str| {
        let order = Arc::clone(&order);
        move || {
            order
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(label);
            Ok(())
        }
    };
    let (blocker, block) = blocker();
    let first = submitter.submit(IoClass::Low, IoKind::Write, MIB, block);
    let lows: Vec<_> = (0..20)
        .map(|_| submitter.submit(IoClass::Low, IoKind::Write, MIB, log("low")))
        .collect();
    let high = submitter.submit(IoClass::High, IoKind::Read, 4096, log("high"));
    let (first, high) = (first.expect("submit"), high.expect("submit"));
    blocker.release.send(()).expect("release");
    run(async {
        first.await.expect("first");
        high.await.expect("high");
        for low in lows {
            low.expect("submit").await.expect("low");
        }
    });
    let order = order.lock().unwrap_or_else(PoisonError::into_inner).clone();
    assert_eq!(order.first(), Some(&"high"), "{order:?}");
    pool.shutdown().expect("shutdown");
}

#[test]
fn full_submission_queue_is_would_block() {
    let registry = Registry::default();
    let pool = pool(1, &registry);
    let submitter = pool.submitter();
    let (blocker, block) = blocker();
    let first = submitter
        .submit(IoClass::Low, IoKind::Write, 4096, block)
        .expect("submit");
    let mut accepted = Vec::new();
    let mut refused = false;
    for _ in 0..100_000 {
        match submitter.submit(IoClass::Low, IoKind::Write, 4096, || Ok(())) {
            Ok(op) => accepted.push(op),
            Err(e) if e.kind() == ErrorKind::WouldBlock => {
                refused = true;
                break;
            }
            Err(e) => panic!("unexpected {e}"),
        }
    }
    assert!(refused, "the queues never filled up");
    blocker.release.send(()).expect("release");
    run(async {
        first.await.expect("first");
        for op in accepted {
            op.await.expect("accepted ops still run");
        }
    });
    pool.shutdown().expect("shutdown");
}

#[test]
fn shutdown_fails_waiting_ops() {
    let registry = Registry::default();
    let pool = pool(1, &registry);
    let submitter = pool.submitter();
    let (blocker, block) = blocker();
    let first = submitter
        .submit(IoClass::Low, IoKind::Write, 4096, block)
        .expect("submit");
    let waiting: Vec<_> = (0..5)
        .map(|_| {
            submitter
                .submit(IoClass::Low, IoKind::Write, 4096, || Ok(()))
                .expect("submit")
        })
        .collect();
    blocker.started.recv().expect("the first op runs");
    let stopper = std::thread::spawn(move || pool.shutdown());
    run(async {
        for op in waiting {
            let error = op.await.expect_err("waiting ops fail");
            assert_eq!(error.kind(), ErrorKind::Interrupted);
        }
    });
    blocker.release.send(()).expect("release");
    run(async { first.await.expect("the op in flight completes") });
    stopper.join().expect("join").expect("stops in time");
    let late = submitter.submit(IoClass::High, IoKind::Read, 4096, || Ok(()));
    let error = late.err().expect("a stopped pool refuses work");
    assert_eq!(error.kind(), ErrorKind::Interrupted);
}

#[test]
fn metrics_are_exposed() {
    let registry = Registry::default();
    let pool = pool(2, &registry);
    let submitter = pool.submitter();
    run(async {
        submitter
            .submit(IoClass::High, IoKind::Read, 4096, || Ok(()))
            .expect("submit")
            .await
            .expect("read");
        submitter
            .submit(IoClass::Critical, IoKind::Sync, 0, || Ok(()))
            .expect("submit")
            .await
            .expect("sync");
    });
    pool.shutdown().expect("shutdown");
    let text = registry.render();
    for needle in [
        "ndaal_io_ops_total{class=\"high\",device=\"test\",op=\"read\"} 1",
        "ndaal_io_ops_total{class=\"critical\",device=\"test\",op=\"sync\"} 1",
        "ndaal_io_latency_seconds_count{class=\"high\",device=\"test\"} 1",
        "ndaal_io_inflight{device=\"test\"} 0",
        "ndaal_io_bandwidth_bytes_per_second{device=\"test\"}",
        "ndaal_io_throttled_total{device=\"test\"} 0",
    ] {
        assert!(text.contains(needle), "{needle} missing:\n{text}");
    }
}

/// Background ops held by an empty bucket fill the dispatcher's queue: a
/// critical op submitted after them must still be read and served.
#[test]
fn critical_is_never_stuck_behind_background() {
    let registry = Registry::default();
    let config = SchedulerConfig {
        depth: 1,
        // Share 0: the background class is paused, its ops never start.
        background_share: 0.0,
    };
    let pool = IoPool::start("test", config, &[], &registry).expect("start");
    let submitter = pool.submitter();
    let mut parked = Vec::new();
    for _ in 0..100_000 {
        match submitter.submit(IoClass::Background, IoKind::Write, 4096, || Ok(())) {
            Ok(op) => parked.push(op),
            Err(e) if e.kind() == ErrorKind::WouldBlock => break,
            Err(e) => panic!("unexpected {e}"),
        }
    }
    let critical = submitter
        .submit(IoClass::Critical, IoKind::Sync, 0, || Ok(()))
        .expect("critical accepted");
    let clock = ndaal_io::real::SystemClock::new();
    run(async {
        tokio::select! {
            result = critical => result.expect("critical served"),
            () = ndaal_io::Clock::sleep(&clock, std::time::Duration::from_secs(5)) => {
                panic!("critical op stuck behind paused background ops")
            }
        }
    });
    drop(parked);
    pool.shutdown().expect("shutdown");
}
