//! Candidate against reference, at the ENF-COMP tolerance (ADR-027).

use serde_json::Value;

use super::memtier::Sample;

/// The candidate keeps at least 95 % of the reference throughput (ADR-027).
pub const MIN_THROUGHPUT_RATIO: f64 = 0.95;
/// The candidate p99 latency is at most 105 % of the reference (ADR-027).
pub const MAX_P99_RATIO: f64 = 1.05;
/// Ratios are quotients of floats: a candidate exactly at a threshold must
/// not fail on a rounding error.
const RATIO_EPSILON: f64 = 1e-9;

/// Medians of both sides and how they compare.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Verdict {
    /// Median of the reference runs.
    pub reference: Sample,
    /// Median of the candidate runs.
    pub candidate: Sample,
    /// Candidate throughput / reference throughput.
    pub throughput_ratio: f64,
    /// Candidate p99 / reference p99.
    pub p99_ratio: f64,
    /// Both ratios within the tolerance.
    pub within_tolerance: bool,
    /// Lowest and highest values of the reference runs, per metric.
    pub reference_spread: (Sample, Sample),
    /// Lowest and highest values of the candidate runs, per metric.
    pub candidate_spread: (Sample, Sample),
    /// Runs per side.
    pub runs: usize,
}

/// Compares the medians of `reference` and `candidate`; both sides need the
/// same, non-zero number of runs (they alternate).
pub fn compare(reference: &[Sample], candidate: &[Sample]) -> Result<Verdict, String> {
    if reference.is_empty() || candidate.is_empty() {
        return Err("no run on one side".into());
    }
    if reference.len() != candidate.len() {
        return Err(format!(
            "{} reference runs but {} candidate runs",
            reference.len(),
            candidate.len()
        ));
    }
    let runs = reference.len();
    let reference_spread = spread(reference);
    let candidate_spread = spread(candidate);
    let reference = median_sample(reference);
    let candidate = median_sample(candidate);
    let throughput_ratio = candidate.ops_per_sec / reference.ops_per_sec;
    let p99_ratio = candidate.p99_ms / reference.p99_ms;
    let within_tolerance = throughput_ratio >= MIN_THROUGHPUT_RATIO - RATIO_EPSILON
        && p99_ratio <= MAX_P99_RATIO + RATIO_EPSILON;
    Ok(Verdict {
        reference,
        candidate,
        throughput_ratio,
        p99_ratio,
        within_tolerance,
        reference_spread,
        candidate_spread,
        runs,
    })
}

/// Lowest and highest value of each metric; `samples` not empty.
fn spread(samples: &[Sample]) -> (Sample, Sample) {
    let fold = |pick: fn(f64, f64) -> f64| Sample {
        ops_per_sec: samples
            .iter()
            .map(|s| s.ops_per_sec)
            .reduce(pick)
            .unwrap_or(0.0),
        p99_ms: samples.iter().map(|s| s.p99_ms).reduce(pick).unwrap_or(0.0),
    };
    (fold(f64::min), fold(f64::max))
}

/// Median of each metric on its own (5 repetitions: STRATEGIE section 9.1).
fn median_sample(samples: &[Sample]) -> Sample {
    Sample {
        ops_per_sec: median(samples.iter().map(|s| s.ops_per_sec).collect()),
        p99_ms: median(samples.iter().map(|s| s.p99_ms).collect()),
    }
}

/// Middle value, or the mean of the two middle values; `values` not empty.
fn median(mut values: Vec<f64>) -> f64 {
    values.sort_by(f64::total_cmp);
    let mid = values.len() / 2;
    if values.len() % 2 == 1 {
        values[mid]
    } else {
        (values[mid - 1] + values[mid]) / 2.0
    }
}

/// Markdown report: values, ratios, thresholds, verdict, and the metadata
/// recorded with the runs (versions, parameters: STRATEGIE section 9.4).
pub fn report(bench: &str, metadata: &Value, verdict: &Verdict) -> String {
    let (r, c) = (verdict.reference, verdict.candidate);
    let ((r_min, r_max), (c_min, c_max)) = (verdict.reference_spread, verdict.candidate_spread);
    let runs = verdict.runs;
    let outcome = if verdict.within_tolerance {
        "**Within tolerance**"
    } else {
        "**Beyond tolerance**"
    };
    let metadata = serde_json::to_string_pretty(metadata).unwrap_or_default();
    format!(
        "# Comparison: {bench}\n\n\
         Medians of {runs} runs per side; tolerance of ADR-027.\n\n\
         | Metric | Reference | Candidate | Ratio | Threshold |\n\
         |---|---|---|---|---|\n\
         | Throughput (ops/s) | {:.0} | {:.0} | {:.1} % | >= {:.1} % |\n\
         | p99 latency (ms) | {:.3} | {:.3} | {:.1} % | <= {:.1} % |\n\
         | Throughput min - max (ops/s) | {:.0} - {:.0} | {:.0} - {:.0} | | |\n\
         | p99 min - max (ms) | {:.3} - {:.3} | {:.3} - {:.3} | | |\n\n\
         {outcome}\n\n\
         ## Runs\n\n\
         ```json\n{metadata}\n```\n",
        r.ops_per_sec,
        c.ops_per_sec,
        verdict.throughput_ratio * 100.0,
        MIN_THROUGHPUT_RATIO * 100.0,
        r.p99_ms,
        c.p99_ms,
        verdict.p99_ratio * 100.0,
        MAX_P99_RATIO * 100.0,
        r_min.ops_per_sec,
        r_max.ops_per_sec,
        c_min.ops_per_sec,
        c_max.ops_per_sec,
        r_min.p99_ms,
        r_max.p99_ms,
        c_min.p99_ms,
        c_max.p99_ms,
    )
}

#[cfg(test)]
mod tests {
    use super::{Sample, compare, report};

    fn sample(ops_per_sec: f64, p99_ms: f64) -> Sample {
        Sample {
            ops_per_sec,
            p99_ms,
        }
    }

    #[test]
    fn tolerance_boundaries() {
        let reference = [sample(1000.0, 2.0)];
        let cases = [
            (sample(950.0, 2.0), true),
            (sample(949.0, 2.0), false),
            (sample(1000.0, 2.1), true),
            (sample(1000.0, 2.102), false),
            (sample(1200.0, 1.0), true),
        ];
        for (candidate, expected) in cases {
            let verdict = compare(&reference, &[candidate]).expect("valid");
            assert_eq!(verdict.within_tolerance, expected, "{candidate:?}");
        }
    }

    #[test]
    fn medians_resist_one_outlier() {
        let reference = [
            sample(1000.0, 2.0),
            sample(990.0, 2.1),
            sample(10.0, 90.0),
            sample(1010.0, 1.9),
            sample(1005.0, 2.0),
        ];
        let candidate = [sample(1000.0, 2.0); 5];
        let verdict = compare(&reference, &candidate).expect("valid");
        assert_eq!(verdict.reference, sample(1000.0, 2.0));
        assert!(verdict.within_tolerance);
    }

    #[test]
    fn unequal_or_empty_sides_are_errors() {
        let one = [sample(1.0, 1.0)];
        let two = [sample(1.0, 1.0); 2];
        assert!(compare(&[], &[]).is_err());
        assert!(compare(&one, &[]).is_err());
        assert!(compare(&one, &two).is_err());
    }

    /// STRATEGIE section 9.1: median and dispersion are published; the
    /// spread is what tells noise (P-50) from a regression.
    #[test]
    fn report_shows_the_spread() {
        let reference = [sample(1000.0, 2.0), sample(800.0, 2.5), sample(1100.0, 1.5)];
        let candidate = [sample(990.0, 2.0), sample(1000.0, 2.1), sample(1010.0, 1.9)];
        let verdict = compare(&reference, &candidate).expect("valid");
        let text = report("kv", &serde_json::json!({}), &verdict);
        for expected in [
            "| Throughput min - max (ops/s) | 800 - 1100 | 990 - 1010 | | |",
            "| p99 min - max (ms) | 1.500 - 2.500 | 1.900 - 2.100 | | |",
            "3 runs per side",
        ] {
            assert!(text.contains(expected), "missing {expected:?} in\n{text}");
        }
    }

    #[test]
    fn report_lists_values_ratios_and_metadata() {
        let verdict = compare(&[sample(1000.0, 2.0)], &[sample(900.0, 2.5)]).expect("valid");
        let metadata = serde_json::json!({"kernel": "6.8.0-test", "repeats": 1});
        let text = report("kv", &metadata, &verdict);
        for expected in [
            "# Comparison: kv",
            "| Throughput (ops/s) | 1000 | 900 | 90.0 % | >= 95.0 % |",
            "| p99 latency (ms) | 2.000 | 2.500 | 125.0 % | <= 105.0 % |",
            "**Beyond tolerance**",
            "6.8.0-test",
        ] {
            assert!(text.contains(expected), "missing {expected:?} in\n{text}");
        }
        // An indented line would turn the table into a code block.
        let before_code = text.split("```json").next().unwrap_or_default();
        assert!(before_code.lines().all(|l| !l.starts_with(' ')), "{text}");
    }
}
