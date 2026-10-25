//! Requests between cores in bursts (J02-A): every core sends requests to
//! cores drawn from the seed over queues of capacity 2, retrying after a
//! short sleep when full. Checks that no request is lost or duplicated,
//! every reply reaches its requester, and nothing locks up.

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use ndaal_io::{Clock, Rng};
use ndaal_observe::metrics::Registry;
use ndaal_runtime::mailbox::sim::sim_mailboxes;
use ndaal_runtime::mailbox::{CoreId, Incoming, SendError};

use crate::sweep::SeedResult;
use crate::{Limits, Outcome, Sim};

/// Capacity of each queue: small, so queues fill up.
const CAPACITY: usize = 2;
/// Sleep of a sender whose queue is full.
const RETRY_AFTER: Duration = Duration::from_micros(10);
/// How often the coordinator checks that every reply arrived.
const CHECK_EVERY: Duration = Duration::from_micros(100);
const LIMITS: Limits = Limits {
    max_steps: 2_000_000,
    max_virtual_time: Duration::from_secs(60),
};

/// What happened during one run of [`cross_core`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CrossCoreReport {
    /// How the run ended.
    pub outcome: Outcome,
    /// Requests accepted by a queue.
    pub sent: u32,
    /// Replies received.
    pub replied: u32,
    /// Replies that did not echo their request.
    pub mismatched: u32,
    /// Trace fingerprint.
    pub fingerprint: u64,
}

#[derive(Default)]
struct Counters {
    sent: Cell<u32>,
    replied: Cell<u32>,
    mismatched: Cell<u32>,
}

fn bump(counter: &Cell<u32>) {
    counter.set(counter.get() + 1);
}

/// Runs `cores` cores sending `requests` requests each.
pub fn cross_core(seed: u64, cores: u16, requests: u32) -> CrossCoreReport {
    let sim = Sim::new(seed);
    let registry = Registry::default();
    let boxes = Rc::new(sim_mailboxes::<(u16, u32), (u16, u32)>(
        cores,
        CAPACITY,
        sim.clock(),
        sim.rng("runtime"),
        &registry,
    ));
    let counters = Rc::new(Counters::default());
    for core in 0..cores {
        let me = usize::from(core);
        let b = Rc::clone(&boxes);
        sim.spawn(async move { b[me].pump().await });
        let b = Rc::clone(&boxes);
        sim.spawn(async move {
            while let Some(incoming) = b[me].recv().await {
                if let Incoming::Request { msg, replier, .. } = incoming {
                    replier.reply(msg);
                }
            }
        });
        let (b, c, s) = (Rc::clone(&boxes), Rc::clone(&counters), sim.clone());
        sim.spawn(async move {
            let mut rng = s.rng(&format!("cross_core-{core}"));
            let clock = s.clock();
            for seq in 0..requests {
                let to = CoreId((rng.next_u64() % u64::from(cores)) as u16);
                let mut msg = (core, seq);
                let pending = loop {
                    match b[me].request(to, msg) {
                        Ok(pending) => break pending,
                        Err(SendError::Full(back)) => {
                            msg = back;
                            clock.sleep(RETRY_AFTER).await;
                        }
                        Err(SendError::ShuttingDown(_)) => return,
                    }
                };
                bump(&c.sent);
                let (c, s) = (Rc::clone(&c), s.clone());
                s.clone().spawn(async move {
                    match pending.await {
                        Ok(reply) if reply == (core, seq) => {
                            s.trace(format!("{core}:{seq}->{}", to.0));
                            bump(&c.replied);
                        }
                        _ => bump(&c.mismatched),
                    }
                });
            }
        });
    }
    let (b, c, s) = (Rc::clone(&boxes), Rc::clone(&counters), sim.clone());
    let total = u32::from(cores) * requests;
    sim.spawn(async move {
        let clock = s.clock();
        while c.replied.get() + c.mismatched.get() < total {
            clock.sleep(CHECK_EVERY).await;
        }
        b[0].shutdown();
    });
    let outcome = sim.run(LIMITS);
    CrossCoreReport {
        outcome,
        sent: counters.sent.get(),
        replied: counters.replied.get(),
        mismatched: counters.mismatched.get(),
        fingerprint: sim.fingerprint(),
    }
}

/// The nightly form: 4 cores, 50 requests each.
pub fn cross_core_result(seed: u64) -> SeedResult {
    let report = cross_core(seed, 4, 50);
    let finished = report.outcome == Outcome::Finished
        && report.sent == 200
        && report.replied == 200
        && report.mismatched == 0;
    let detail = if finished {
        String::new()
    } else {
        format!(
            "outcome {:?}, sent {}, replied {}, mismatched {}",
            report.outcome, report.sent, report.replied, report.mismatched
        )
    };
    SeedResult {
        finished,
        fingerprint: report.fingerprint,
        detail,
    }
}

#[cfg(test)]
mod tests {
    use super::{cross_core, cross_core_result};

    #[test]
    fn cross_core_completes() {
        for seed in 0..50 {
            let result = cross_core_result(seed);
            assert!(result.finished, "seed {seed}: {}", result.detail);
        }
    }

    #[test]
    fn cross_core_is_reproducible() {
        assert_eq!(cross_core(7, 4, 50), cross_core(7, 4, 50));
        assert_ne!(
            cross_core(7, 4, 50).fingerprint,
            cross_core(8, 4, 50).fingerprint
        );
    }
}
