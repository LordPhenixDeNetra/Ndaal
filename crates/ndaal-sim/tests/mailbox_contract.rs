//! The messaging contract of ndaal-runtime on the simulated transport
//! (J02-A, T02.03): run on the simulation executor, from a seed.

use std::cell::{Cell, RefCell};
use std::future::Future;
use std::rc::Rc;
use std::time::Duration;

use ndaal_observe::metrics::Registry;
use ndaal_runtime::mailbox::contract::{self, CAPACITY, CORES};
use ndaal_runtime::mailbox::sim::{SimMailbox, sim_mailboxes};
use ndaal_runtime::mailbox::{CoreId, Incoming};
use ndaal_sim::{Limits, Outcome, Sim};

const LIMITS: Limits = Limits {
    max_steps: 100_000,
    max_virtual_time: Duration::from_secs(10),
};

type Boxes = Rc<Vec<SimMailbox<u32, u32>>>;

/// Runs `check` with fresh mailboxes and their pump tasks; the check must
/// finish and every task with it.
fn run<F, Fut>(seed: u64, registry: &Registry, check: F)
where
    F: FnOnce(Boxes) -> Fut,
    Fut: Future<Output = ()> + 'static,
{
    let sim = Sim::new(seed);
    let boxes: Boxes = Rc::new(sim_mailboxes(
        CORES,
        CAPACITY,
        sim.clock(),
        sim.rng("runtime"),
        registry,
    ));
    for i in 0..boxes.len() {
        let b = Rc::clone(&boxes);
        sim.spawn(async move { b[i].pump().await });
    }
    let done = Rc::new(Cell::new(false));
    let finished = Rc::clone(&done);
    let check = check(Rc::clone(&boxes));
    sim.spawn(async move {
        check.await;
        finished.set(true);
    });
    assert_eq!(sim.run(LIMITS), Outcome::Finished, "seed {seed}");
    assert!(done.get(), "seed {seed}: the check did not complete");
}

macro_rules! contract_test {
    ($name:ident) => {
        #[test]
        fn $name() {
            for seed in 0..20 {
                run(seed, &Registry::default(), |b| async move {
                    contract::$name(&b).await
                });
            }
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

#[test]
fn full_queue_is_counted() {
    let registry = Registry::default();
    run(1, &registry, |b| async move {
        contract::full_returns_the_message(&b).await
    });
    let text = registry.render();
    assert!(
        text.contains("ndaal_runtime_queue_full_total{to=\"1\"} 1"),
        "{text}"
    );
}

#[test]
fn dropped_replies_are_counted() {
    let registry = Registry::default();
    run(1, &registry, |b| async move {
        contract::dropped_replier(&b).await
    });
    let text = registry.render();
    let expected = format!("ndaal_runtime_replies_dropped_total {}", CAPACITY + 1);
    assert!(text.contains(&expected), "{text}");
}

/// Order of reception on core 1 from two senders.
fn arrival_order(seed: u64) -> Vec<(u16, u32)> {
    let order = Rc::new(RefCell::new(Vec::new()));
    let seen = Rc::clone(&order);
    run(seed, &Registry::default(), |b| async move {
        for n in 0..CAPACITY as u32 {
            b[0].try_send(CoreId(1), n).expect("room");
            b[2].try_send(CoreId(1), 100 + n).expect("room");
        }
        for _ in 0..2 * CAPACITY {
            match b[1].recv().await {
                Some(Incoming::Message { from, msg }) => seen.borrow_mut().push((from.0, msg)),
                other => panic!("{other:?}"),
            }
        }
        b[0].shutdown();
    });
    order.take()
}

#[test]
fn same_seed_same_delivery() {
    for seed in 0..20 {
        let order = arrival_order(seed);
        assert_eq!(order, arrival_order(seed), "seed {seed}");
        let from0: Vec<u32> = order
            .iter()
            .filter(|(f, _)| *f == 0)
            .map(|(_, m)| *m)
            .collect();
        assert_eq!(from0, vec![0, 1], "FIFO per pair, seed {seed}");
    }
    let orders: std::collections::BTreeSet<_> = (0..20).map(arrival_order).collect();
    assert!(orders.len() > 1, "the seed must change the interleaving");
}
