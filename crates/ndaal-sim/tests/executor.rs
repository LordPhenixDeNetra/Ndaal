//! Deterministic scheduler (T01.09).

use std::cell::{Cell, RefCell};
use std::future::{Future, pending, poll_fn};
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll, Waker};
use std::time::Duration;

use ndaal_io::Clock;
use ndaal_sim::{Limits, Outcome, Sim};

const LIMITS: Limits = Limits {
    max_steps: 10_000,
    max_virtual_time: Duration::from_secs(60),
};

/// Pending once, waking itself: hands control back to the scheduler.
struct YieldNow(bool);

impl Future for YieldNow {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        if self.0 {
            return Poll::Ready(());
        }
        self.0 = true;
        cx.waker().wake_by_ref();
        Poll::Pending
    }
}

fn yield_now() -> YieldNow {
    YieldNow(false)
}

#[test]
fn every_task_runs_to_completion() {
    let sim = Sim::new(1);
    let done = Rc::new(Cell::new(0));
    for _ in 0..3 {
        let done = Rc::clone(&done);
        sim.spawn(async move {
            yield_now().await;
            done.set(done.get() + 1);
        });
    }
    assert_eq!(sim.run(LIMITS), Outcome::Finished);
    assert_eq!(done.get(), 3);
}

#[test]
fn sleeps_complete_in_deadline_order_at_virtual_times() {
    let sim = Sim::new(2);
    let log = Rc::new(RefCell::new(Vec::new()));
    for (name, ms) in [("slow", 10), ("fast", 5)] {
        let (clock, log) = (sim.clock(), Rc::clone(&log));
        sim.spawn(async move {
            clock.sleep(Duration::from_millis(ms)).await;
            log.borrow_mut().push((name, clock.monotonic()));
        });
    }
    assert_eq!(sim.run(LIMITS), Outcome::Finished);
    let expected = [
        ("fast", Duration::from_millis(5)),
        ("slow", Duration::from_millis(10)),
    ];
    assert_eq!(*log.borrow(), expected);
}

#[test]
fn waiting_forever_without_timers_is_a_deadlock() {
    let sim = Sim::new(3);
    sim.spawn(pending::<()>());
    sim.spawn(async {});
    assert_eq!(sim.run(LIMITS), Outcome::Deadlock { blocked: 1 });
}

#[test]
fn endless_self_wakeups_hit_the_step_limit() {
    let sim = Sim::new(4);
    sim.spawn(async {
        loop {
            yield_now().await;
        }
    });
    assert_eq!(sim.run(LIMITS), Outcome::StepLimit);
}

#[test]
fn endless_sleeping_hits_the_time_limit() {
    let sim = Sim::new(5);
    let clock = sim.clock();
    sim.spawn(async move {
        loop {
            clock.sleep(Duration::from_secs(1)).await;
        }
    });
    assert_eq!(sim.run(LIMITS), Outcome::TimeLimit);
    assert!(sim.clock().monotonic() <= LIMITS.max_virtual_time);
}

#[test]
fn double_wakeups_and_wakeups_after_completion_do_nothing() {
    let sim = Sim::new(6);
    let polls = Rc::new(Cell::new(0));
    let stored: Rc<RefCell<Option<Waker>>> = Rc::new(RefCell::new(None));
    {
        let (polls, stored) = (Rc::clone(&polls), Rc::clone(&stored));
        sim.spawn(poll_fn(move |cx| {
            polls.set(polls.get() + 1);
            if polls.get() == 1 {
                *stored.borrow_mut() = Some(cx.waker().clone());
                return Poll::Pending;
            }
            Poll::Ready(())
        }));
    }
    {
        let stored = Rc::clone(&stored);
        sim.spawn(async move {
            yield_now().await;
            if let Some(waker) = stored.borrow().as_ref() {
                waker.wake_by_ref();
                waker.wake_by_ref();
            }
        });
    }
    assert_eq!(sim.run(LIMITS), Outcome::Finished);
    assert_eq!(
        polls.get(),
        2,
        "woken twice while pending, polled once more"
    );
    if let Some(waker) = stored.borrow().as_ref() {
        waker.wake_by_ref();
    }
    assert_eq!(sim.run(LIMITS), Outcome::Finished);
    assert_eq!(polls.get(), 2, "a finished task is never polled again");
}

/// Five tasks that yield a few times and record their execution order.
fn interleaving(seed: u64) -> (Vec<usize>, u64) {
    let sim = Sim::new(seed);
    let order = Rc::new(RefCell::new(Vec::new()));
    for task in 0..5 {
        let order = Rc::clone(&order);
        sim.spawn(async move {
            for _ in 0..3 {
                order.borrow_mut().push(task);
                yield_now().await;
            }
        });
    }
    assert_eq!(sim.run(LIMITS), Outcome::Finished);
    let order = order.borrow().clone();
    (order, sim.fingerprint())
}

#[test]
fn same_seed_same_schedule_and_fingerprint() {
    let mut distinct = std::collections::BTreeSet::new();
    for seed in 0..50 {
        let first = interleaving(seed);
        assert_eq!(first, interleaving(seed), "seed {seed}");
        distinct.insert(first.0);
    }
    assert!(distinct.len() > 1, "the seed must change the schedule");
}

#[test]
fn user_trace_events_change_the_fingerprint() {
    let a = Sim::new(7);
    a.trace("event");
    let b = Sim::new(7);
    b.trace("other");
    assert_ne!(a.fingerprint(), b.fingerprint());
}

#[test]
fn named_rngs_are_reproducible_and_independent() {
    use ndaal_io::Rng;
    let draw = |seed: u64, name: &str| {
        let mut rng = Sim::new(seed).rng(name);
        (0..4).map(|_| rng.next_u64()).collect::<Vec<_>>()
    };
    assert_eq!(draw(8, "chaos"), draw(8, "chaos"));
    assert_ne!(draw(8, "chaos"), draw(8, "client"));
    assert_ne!(draw(8, "chaos"), draw(9, "chaos"));
}

// Review of lot 4: stopping on the step limit must not lose the task that
// was about to run; a second run finishes it.
#[test]
fn a_run_stopped_by_the_step_limit_can_resume() {
    let sim = Sim::new(9);
    sim.spawn(async {
        for _ in 0..5 {
            yield_now().await;
        }
    });
    let tight = Limits {
        max_steps: 2,
        max_virtual_time: Duration::from_secs(1),
    };
    assert_eq!(sim.run(tight), Outcome::StepLimit);
    assert_eq!(sim.run(LIMITS), Outcome::Finished);
}
