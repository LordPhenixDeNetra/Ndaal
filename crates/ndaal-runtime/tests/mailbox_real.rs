//! The real transport: the messaging contract on one thread, then wake-ups
//! between two threads (J02-A, T02.03).

use std::future::Future;
use std::rc::Rc;
use std::sync::{Arc, Barrier};
use std::time::Duration;

use ndaal_io::Clock;
use ndaal_io::real::SystemClock;
use ndaal_observe::metrics::Registry;
use ndaal_runtime::mailbox::contract::{self, CAPACITY, CORES};
use ndaal_runtime::mailbox::real::{RealMailbox, real_endpoints};
use ndaal_runtime::mailbox::{CoreId, Incoming};

type Boxes = Rc<Vec<RealMailbox<u32, u32>>>;

/// Runs `check` on one thread with fresh mailboxes and their pump tasks.
fn run<F, Fut>(check: F)
where
    F: FnOnce(Boxes) -> Fut,
    Fut: Future<Output = ()>,
{
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .expect("runtime");
    let local = tokio::task::LocalSet::new();
    local.block_on(&runtime, async {
        let registry = Registry::default();
        let boxes: Boxes = Rc::new(
            real_endpoints(CORES, CAPACITY)
                .into_iter()
                .map(|e| e.into_mailbox(Duration::from_micros(50), SystemClock::new(), &registry))
                .collect(),
        );
        let pumps: Vec<_> = (0..boxes.len())
            .map(|i| {
                let b = Rc::clone(&boxes);
                tokio::task::spawn_local(async move { b[i].pump().await })
            })
            .collect();
        check(Rc::clone(&boxes)).await;
        for pump in pumps {
            pump.await.expect("pump task");
        }
    });
}

macro_rules! contract_test {
    ($name:ident) => {
        #[test]
        fn $name() {
            run(|b| async move { contract::$name(&b).await });
        }
    };
}

contract_test!(fifo_per_pair);
contract_test!(full_returns_the_message);
contract_test!(reply_always_delivered);
contract_test!(request_without_reply_slot_is_refused);
contract_test!(dropped_replier);
contract_test!(crossed_requests_do_not_deadlock);
contract_test!(shutdown);
contract_test!(replier_dropped_by_shutdown);

/// Requests from core 0 on this thread, echoed by core 1 on another;
/// returns the registry and the rounds that missed `deadline`.
fn ping(rounds: u32, idle_spin: Duration, deadline: Duration) -> (Registry, Vec<u32>) {
    let registry = Registry::default();
    let mut endpoints = real_endpoints::<u32, u32>(2, 4);
    let echo_end = endpoints.pop().expect("core 1");
    let ping_end = endpoints.pop().expect("core 0");
    let start = Arc::new(Barrier::new(2));
    let echo_registry = registry.clone();
    let echo_start = Arc::clone(&start);
    let echo = std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .expect("runtime");
        let local = tokio::task::LocalSet::new();
        local.block_on(&runtime, async {
            let b = Rc::new(echo_end.into_mailbox(idle_spin, SystemClock::new(), &echo_registry));
            let pump = Rc::clone(&b);
            let pump = tokio::task::spawn_local(async move { pump.pump().await });
            echo_start.wait();
            while let Some(incoming) = b.recv().await {
                if let Incoming::Request { msg, replier, .. } = incoming {
                    replier.reply(msg);
                }
            }
            pump.await.expect("pump");
        });
    });
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .expect("runtime");
    let local = tokio::task::LocalSet::new();
    let late = local.block_on(&runtime, async {
        let clock = SystemClock::new();
        let b = Rc::new(ping_end.into_mailbox(idle_spin, clock, &registry));
        let pump = Rc::clone(&b);
        let pump = tokio::task::spawn_local(async move { pump.pump().await });
        start.wait();
        let mut late = Vec::new();
        for round in 0..rounds {
            let pending = b.request(CoreId(1), round).expect("one request in flight");
            tokio::select! {
                answer = pending => assert_eq!(answer, Ok(round)),
                () = clock.sleep(deadline) => late.push(round),
            }
            if late.len() > 10 {
                break;
            }
        }
        b.shutdown();
        pump.await.expect("pump");
        late
    });
    echo.join().expect("echo thread");
    (registry, late)
}

/// Every message sent while the receiver goes to sleep is still received:
/// with no idle spin, each request finds core 1 asleep or about to be.
#[test]
fn wake_is_never_lost() {
    let (_, late) = ping(100_000, Duration::ZERO, Duration::from_secs(1));
    assert!(late.is_empty(), "round trips past 1 s: {late:?}");
}

/// While requests keep coming, the idle spin keeps core 1's receiving task
/// awake, however long the test takes (its reply pump, which never gets a
/// reply, may sleep: that is counted apart).
#[test]
fn spin_avoids_parking() {
    let (registry, late) = ping(1_000, Duration::from_secs(2), Duration::from_secs(1));
    assert!(late.is_empty(), "{late:?}");
    let text = registry.render();
    assert!(
        text.contains("ndaal_runtime_core_parks_total{core=\"1\",queue=\"messages\"} 0"),
        "{text}"
    );
}

/// A core handling a queue that never runs dry still lets its other tasks
/// run (its reply pump first): core 0 has 256 messages waiting and sends a
/// request to core 1 for each; replies must complete while it works, not
/// only once its queue is empty. One thread, so the order is exact.
#[test]
fn recv_lets_the_core_run_its_other_tasks() {
    const QUEUED: u32 = 256;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .expect("runtime");
    let local = tokio::task::LocalSet::new();
    local.block_on(&runtime, async {
        let registry = Registry::default();
        let boxes: Boxes = Rc::new(
            real_endpoints(3, QUEUED as usize)
                .into_iter()
                .map(|e| e.into_mailbox(Duration::from_micros(50), SystemClock::new(), &registry))
                .collect(),
        );
        for n in 0..QUEUED {
            boxes[2]
                .try_send(CoreId(0), n)
                .expect("room for every message");
        }
        for i in 0..boxes.len() {
            let b = Rc::clone(&boxes);
            tokio::task::spawn_local(async move { b[i].pump().await });
        }
        let echo = Rc::clone(&boxes);
        tokio::task::spawn_local(async move {
            while let Some(incoming) = echo[1].recv().await {
                if let Incoming::Request { msg, replier, .. } = incoming {
                    replier.reply(msg);
                }
            }
        });
        let completed = Rc::new(std::cell::Cell::new(0u32));
        for n in 0..QUEUED {
            match boxes[0].recv().await {
                Some(Incoming::Message { .. }) => {}
                other => panic!("{other:?}"),
            }
            let pending = boxes[0].request(CoreId(1), n).expect("a reply slot");
            let done = Rc::clone(&completed);
            tokio::task::spawn_local(async move {
                if pending.await.is_ok() {
                    done.set(done.get() + 1);
                }
            });
        }
        let during = completed.get();
        boxes[0].shutdown();
        assert!(during > 0, "no reply completed while core 0 was busy");
    });
}
