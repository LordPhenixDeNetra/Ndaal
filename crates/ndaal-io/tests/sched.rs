//! The I/O scheduler as a pure state machine (T02.05, ARCHITECTURE.md
//! section 4.3): time is a parameter, so every test is exact.

use std::collections::BTreeSet;
use std::time::Duration;

use ndaal_io::sched::{IoClass, Op, OpId, Scheduler, SchedulerConfig, UNSIZED_COST};
use ndaal_io::{Rng, Xoshiro256PlusPlus};
use proptest::prelude::*;

const KIB: u64 = 1024;
const MIB: u64 = 1024 * KIB;
/// Speed of the device the tests model.
const DEVICE_RATE: u64 = 200 * MIB;

const WEIGHTED: [IoClass; 4] = [
    IoClass::High,
    IoClass::Medium,
    IoClass::Low,
    IoClass::Background,
];

fn config(depth: usize, background_share: f64) -> SchedulerConfig {
    SchedulerConfig {
        depth,
        background_share,
    }
}

/// Time a device at [`DEVICE_RATE`] takes to move `bytes`.
fn service(bytes: u64) -> Duration {
    Duration::from_secs_f64(bytes as f64 / DEVICE_RATE as f64)
}

struct Ids(u64);

impl Ids {
    fn op(&mut self, class: IoClass, cost: u64) -> Op {
        self.0 += 1;
        Op {
            id: OpId(self.0),
            class,
            cost,
        }
    }
}

/// Serves the scheduler on a serial device of depth 1 until `until`, or
/// until nothing is ready; `refill` runs after each completion. Returns the
/// time reached.
fn serve(
    sched: &mut Scheduler,
    mut now: Duration,
    until: Duration,
    mut done: impl FnMut(&Op, &mut Scheduler),
) -> Duration {
    while now < until {
        let Some(op) = sched.next(now) else {
            match sched.next_wakeup(now) {
                Some(at) if at > now => {
                    now = at;
                    continue;
                }
                _ => break,
            }
        };
        now += service(op.cost);
        sched.complete(op.cost, now);
        done(&op, sched);
    }
    now
}

#[test]
fn critical_first() {
    let mut ids = Ids(0);
    let mut sched = Scheduler::new(config(1, 1.0), Duration::ZERO);
    for class in WEIGHTED {
        sched.submit(ids.op(class, 4 * KIB));
    }
    let critical = ids.op(IoClass::Critical, 4 * KIB);
    sched.submit(critical);
    assert_eq!(sched.next(Duration::ZERO), Some(critical));
}

#[test]
fn weighted_shares() {
    let mut ids = Ids(0);
    let mut rng = Xoshiro256PlusPlus::from_seed(7);
    let mut size = move || 4 * KIB + rng.next_u64() % (MIB - 4 * KIB + 1);
    // Share 1: the bucket never limits the background class here.
    let mut sched = Scheduler::new(config(1, 1.0), Duration::ZERO);
    // Saturated: two ops queued per class, so no queue ever runs empty.
    for class in WEIGHTED {
        sched.submit(ids.op(class, size()));
        sched.submit(ids.op(class, size()));
    }
    let mut served = [0u64; 4];
    let mut count = 0;
    serve(&mut sched, Duration::ZERO, Duration::MAX, |op, sched| {
        let slot = WEIGHTED.iter().position(|c| *c == op.class).expect("class");
        served[slot] += op.cost;
        count += 1;
        if count < 100_000 {
            sched.submit(ids.op(op.class, size()));
        }
    });
    let total: u64 = served.iter().sum();
    for (slot, weight) in [50.0, 25.0, 15.0, 10.0].into_iter().enumerate() {
        let share = served[slot] as f64 / total as f64 * 100.0;
        assert!(
            (share - weight).abs() <= weight * 0.05,
            "{:?}: {share:.2} % for {weight} %",
            WEIGHTED[slot]
        );
    }
}

#[test]
fn background_bucket_caps_rate() {
    let mut ids = Ids(0);
    let share = 0.3;
    let mut sched = Scheduler::new(config(1, share), Duration::ZERO);
    sched.submit(ids.op(IoClass::Background, MIB));
    let mut served = 0;
    let end = Duration::from_secs(10);
    serve(&mut sched, Duration::ZERO, end, |op, sched| {
        served += op.cost;
        sched.submit(ids.op(IoClass::Background, MIB));
    });
    let allowed = share * DEVICE_RATE as f64 * end.as_secs_f64() * 1.05;
    assert!(
        (served as f64) <= allowed,
        "served {served} bytes, allowed {allowed}"
    );
    // The bucket limits, it does not stop: the class still gets most of it.
    assert!(served as f64 >= allowed * 0.5, "served only {served} bytes");
}

#[test]
fn oversized_background_op_passes() {
    let mut ids = Ids(0);
    let mut sched = Scheduler::new(config(1, 0.3), Duration::ZERO);
    let huge = ids.op(IoClass::Background, 64 * MIB);
    sched.submit(huge);
    let mut now = Duration::ZERO;
    for _ in 0..1000 {
        if let Some(op) = sched.next(now) {
            assert_eq!(op, huge);
            return;
        }
        now = sched
            .next_wakeup(now)
            .expect("a background op waits for tokens");
    }
    panic!("a background op larger than the bucket never passed");
}

#[test]
fn background_waits_after_its_debt() {
    let mut ids = Ids(0);
    let mut sched = Scheduler::new(config(4, 0.3), Duration::ZERO);
    let huge = ids.op(IoClass::Background, 64 * MIB);
    sched.submit(huge);
    assert_eq!(sched.next(Duration::ZERO), Some(huge));
    sched.submit(ids.op(IoClass::Background, 4 * KIB));
    assert_eq!(sched.next(Duration::ZERO), None, "the debt is repaid first");
    let wake = sched.next_wakeup(Duration::ZERO).expect("wakeup");
    assert!(wake > Duration::ZERO);
    // Others are never held back by the background debt.
    let high = ids.op(IoClass::High, 4 * KIB);
    sched.submit(high);
    assert_eq!(sched.next(Duration::ZERO), Some(high));
}

#[test]
fn empty_class_keeps_no_credit() {
    let mut ids = Ids(0);
    let mut sched = Scheduler::new(config(1, 1.0), Duration::ZERO);
    let mut now = Duration::ZERO;
    // 64 KiB ops: one Low quantum (60 KiB) is less than an op, three High
    // quanta' worth fit in one High visit. Low runs alone for a long time
    // first: it must not bank the unused part of its quanta.
    let size = 64 * KIB;
    for _ in 0..1000 {
        sched.submit(ids.op(IoClass::Low, size));
        let op = sched.next(now).expect("low");
        now += service(op.cost);
        sched.complete(op.cost, now);
    }
    for _ in 0..20 {
        sched.submit(ids.op(IoClass::Low, size));
        sched.submit(ids.op(IoClass::High, size));
    }
    let mut low = 0;
    for _ in 0..20 {
        let op = sched.next(now).expect("op");
        if op.class == IoClass::Low {
            low += 1;
        }
        now += service(op.cost);
        sched.complete(op.cost, now);
    }
    // 15 / (50 + 15) of 20 is about 5; banked credit would let Low run
    // its whole queue in one visit.
    assert!(low <= 7, "low served {low} of the first 20");
}

#[test]
fn depth_is_never_exceeded() {
    let mut ids = Ids(0);
    let mut sched = Scheduler::new(config(3, 1.0), Duration::ZERO);
    for class in [IoClass::Critical, IoClass::High, IoClass::Low] {
        for _ in 0..5 {
            sched.submit(ids.op(class, 4 * KIB));
        }
    }
    let mut running = Vec::new();
    while let Some(op) = sched.next(Duration::ZERO) {
        running.push(op);
    }
    assert_eq!(running.len(), 3);
    assert_eq!(sched.inflight(), 3);
    sched.complete(4 * KIB, Duration::from_millis(1));
    assert!(sched.next(Duration::from_millis(1)).is_some());
    assert!(sched.next(Duration::from_millis(1)).is_none());
}

#[test]
fn bandwidth_converges() {
    let mut ids = Ids(0);
    let mut sched = Scheduler::new(config(1, 1.0), Duration::ZERO);
    assert_eq!(sched.bandwidth(), 100 * MIB, "starting estimate");
    sched.submit(ids.op(IoClass::High, MIB));
    serve(
        &mut sched,
        Duration::ZERO,
        Duration::from_secs(1),
        |op, sched| {
            sched.submit(ids.op(op.class, MIB));
        },
    );
    let measured = sched.bandwidth() as f64;
    let error = (measured - DEVICE_RATE as f64).abs() / DEVICE_RATE as f64;
    assert!(error <= 0.05, "measured {measured} for {DEVICE_RATE}");
}

#[test]
fn idle_time_does_not_lower_the_bandwidth() {
    let mut ids = Ids(0);
    let mut sched = Scheduler::new(config(1, 1.0), Duration::ZERO);
    let mut now = Duration::ZERO;
    // One op every 100 ms: the device is idle 95 % of the time, yet each op
    // ran at full speed. 400 ops are 2 s of busy time: 20 windows.
    for _ in 0..400 {
        sched.submit(ids.op(IoClass::High, MIB));
        let op = sched.next(now).expect("op");
        sched.complete(op.cost, now + service(op.cost));
        now += Duration::from_millis(100);
    }
    let measured = sched.bandwidth() as f64;
    assert!(measured >= DEVICE_RATE as f64 * 0.95, "measured {measured}");
}

#[test]
fn drain_returns_every_queued_op() {
    let mut ids = Ids(0);
    let mut sched = Scheduler::new(config(1, 0.3), Duration::ZERO);
    let ops: Vec<Op> = [IoClass::Critical, IoClass::Medium, IoClass::Background]
        .into_iter()
        .map(|class| ids.op(class, UNSIZED_COST))
        .collect();
    for op in &ops {
        sched.submit(*op);
    }
    let first = sched.next(Duration::ZERO).expect("one runs");
    let mut drained = sched.drain();
    drained.push(first);
    drained.sort_by_key(|op| op.id.0);
    assert_eq!(drained, ops);
    assert!(sched.next(Duration::ZERO).is_none());
}

#[derive(Debug, Clone)]
enum Step {
    Submit(u8, u64),
    Next,
    Complete(usize),
    Wait(u64),
}

fn step() -> impl Strategy<Value = Step> {
    prop_oneof![
        (0u8..5, 1u64..=4 * MIB).prop_map(|(c, cost)| Step::Submit(c, cost)),
        Just(Step::Next),
        any::<usize>().prop_map(Step::Complete),
        (0u64..200_000).prop_map(Step::Wait),
    ]
}

fn class(n: u8) -> IoClass {
    [
        IoClass::Critical,
        IoClass::High,
        IoClass::Medium,
        IoClass::Low,
        IoClass::Background,
    ][usize::from(n % 5)]
}

proptest! {
    /// Every submitted op comes out exactly once, through `next` or `drain`.
    #[test]
    fn every_op_comes_out_once(
        depth in 1usize..5,
        steps in proptest::collection::vec(step(), 0..200),
    ) {
        let mut ids = Ids(0);
        let mut sched = Scheduler::new(config(depth, 0.3), Duration::ZERO);
        let mut now = Duration::ZERO;
        let mut submitted = BTreeSet::new();
        let mut out = Vec::new();
        let mut running: Vec<Op> = Vec::new();
        for step in steps {
            match step {
                Step::Submit(c, cost) => {
                    let op = ids.op(class(c), cost);
                    submitted.insert(op.id.0);
                    sched.submit(op);
                }
                Step::Next => {
                    if let Some(op) = sched.next(now) {
                        prop_assert!(sched.inflight() <= depth);
                        out.push(op.id.0);
                        running.push(op);
                    }
                }
                Step::Complete(i) if !running.is_empty() => {
                    let op = running.swap_remove(i % running.len());
                    sched.complete(op.cost, now);
                }
                Step::Complete(_) => {}
                Step::Wait(micros) => now += Duration::from_micros(micros),
            }
        }
        out.extend(sched.drain().iter().map(|op| op.id.0));
        let unique: BTreeSet<u64> = out.iter().copied().collect();
        prop_assert_eq!(unique.len(), out.len(), "an op came out twice");
        prop_assert_eq!(unique, submitted);
    }
}
