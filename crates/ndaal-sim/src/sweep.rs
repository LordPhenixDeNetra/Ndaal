//! Many seeds of a scenario (STRATEGIE_DE_TESTS.md section 13.2: at least
//! 10 000 seeds every night): each must finish and replay identically.

use crate::{Outcome, ping_pong};

/// What one run of a scenario gave.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeedResult {
    /// The scenario reached its end with every expected result.
    pub finished: bool,
    /// Trace fingerprint: two runs of a seed must give the same.
    pub fingerprint: u64,
    /// Why the run did not finish, when it did not.
    pub detail: String,
}

/// A seed that failed, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    /// The seed, to replay with `ndaal-sim run --seed`.
    pub seed: u64,
    /// What went wrong.
    pub reason: String,
}

/// Runs seeds `from..from + count`, each twice, and returns the failing
/// ones: not reproducible (fingerprints differ), or not finished. Every seed
/// is run even after a failure.
pub fn sweep(from: u64, count: u64, run: impl Fn(u64) -> SeedResult) -> Vec<Failure> {
    sweep_with(from, count, run, |_| {})
}

/// [`sweep`], calling `report` on each failure as soon as it is found: a
/// later seed may abort the process (`panic = "abort"`).
pub fn sweep_with(
    from: u64,
    count: u64,
    run: impl Fn(u64) -> SeedResult,
    mut report: impl FnMut(&Failure),
) -> Vec<Failure> {
    let mut failures = Vec::new();
    // Saturating: the caller rejects ranges past u64::MAX (`main` does).
    for seed in from..from.saturating_add(count) {
        let first = run(seed);
        let second = run(seed);
        let reason = if first != second {
            Some("not reproducible".to_owned())
        } else if !first.finished {
            Some(first.detail)
        } else {
            None
        };
        if let Some(reason) = reason {
            let failure = Failure { seed, reason };
            report(&failure);
            failures.push(failure);
        }
    }
    failures
}

/// `ping_pong` as a sweep result: finished with every reply.
pub fn ping_pong_result(seed: u64, messages: u32) -> SeedResult {
    let report = ping_pong(seed, messages);
    let finished = report.outcome == Outcome::Finished && report.replies == messages;
    let detail = if finished {
        String::new()
    } else {
        format!(
            "outcome {:?}, {}/{messages} replies",
            report.outcome, report.replies
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
    use std::cell::Cell;

    use super::{Failure, SeedResult, ping_pong_result, sweep, sweep_with};

    fn result(finished: bool, fingerprint: u64) -> SeedResult {
        SeedResult {
            finished,
            fingerprint,
            detail: if finished {
                String::new()
            } else {
                "stuck".into()
            },
        }
    }

    #[test]
    fn sweep_reports_unfinished_seeds() {
        let failures = sweep(10, 5, |seed| result(seed % 2 == 1, seed));
        let expected: Vec<Failure> = [10, 12, 14]
            .into_iter()
            .map(|seed| Failure {
                seed,
                reason: "stuck".into(),
            })
            .collect();
        assert_eq!(failures, expected);
    }

    #[test]
    fn sweep_reports_non_reproducible_seeds() {
        let calls = Cell::new(0u64);
        let failures = sweep(0, 3, |_| {
            calls.set(calls.get() + 1);
            result(true, calls.get())
        });
        assert_eq!(failures.len(), 3);
        assert!(failures.iter().all(|f| f.reason == "not reproducible"));
        assert_eq!(calls.get(), 6, "each seed runs twice");
    }

    /// Failures are reported as they happen: if a later seed aborts the
    /// process (panic = "abort"), the earlier ones are already printed.
    #[test]
    fn sweep_reports_each_failure_as_it_happens() {
        let seen = std::cell::RefCell::new(Vec::new());
        let run = |seed: u64| {
            // Seeds 0, 2, 4 fail: before seed s, those below s are reported.
            assert_eq!(
                seen.borrow().len() as u64,
                seed.div_ceil(2),
                "reported late"
            );
            result(seed % 2 == 1, seed)
        };
        let failures = sweep_with(0, 6, run, |failure| seen.borrow_mut().push(failure.seed));
        assert_eq!(*seen.borrow(), vec![0, 2, 4]);
        assert_eq!(failures.len(), 3);
    }

    #[test]
    fn sweep_of_ping_pong_is_clean() {
        let failures = sweep(0, 200, |seed| ping_pong_result(seed, 20));
        assert!(failures.is_empty(), "{failures:?}");
    }

    #[test]
    fn ping_pong_result_reports_missing_replies() {
        let ok = ping_pong_result(42, 20);
        assert!(ok.finished, "{ok:?}");
        assert_eq!(ok, ping_pong_result(42, 20));
    }
}
