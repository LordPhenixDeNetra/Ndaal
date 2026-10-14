//! Results of one `memtier_benchmark` run (`--json-out-file`).

use serde_json::Value;

/// What a comparison reads from one run.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sample {
    /// Operations per second, all commands.
    pub ops_per_sec: f64,
    /// 99th percentile latency, milliseconds.
    pub p99_ms: f64,
}

/// Reads `ALL STATS > Totals > Ops/sec` and
/// `ALL STATS > Totals > Percentile Latencies > p99.00`. A missing, non
/// numeric, negative or zero value is an error naming the field: a run cut
/// short must never compare as zeros.
pub fn parse(json: &str) -> Result<Sample, String> {
    let root: Value = serde_json::from_str(json).map_err(|e| format!("invalid json: {e}"))?;
    let totals = root
        .get("ALL STATS")
        .and_then(|stats| stats.get("Totals"))
        .ok_or("missing ALL STATS > Totals")?;
    let ops_per_sec = positive(totals.get("Ops/sec"), "Ops/sec")?;
    let p99 = totals
        .get("Percentile Latencies")
        .and_then(|latencies| latencies.get("p99.00"));
    let p99_ms = positive(p99, "p99.00")?;
    if let Some(gets) = root.get("ALL STATS").and_then(|stats| stats.get("Gets")) {
        let rate = |field| gets.get(field).and_then(Value::as_f64).unwrap_or(0.0);
        if rate("Ops/sec") > 0.0 && rate("Hits/sec") <= 0.0 {
            return Err(
                "Gets > Hits/sec is 0: every GET missed, the keyspace was not preloaded".into(),
            );
        }
    }
    Ok(Sample {
        ops_per_sec,
        p99_ms,
    })
}

/// `value` as a finite number above zero.
fn positive(value: Option<&Value>, field: &str) -> Result<f64, String> {
    match value.and_then(Value::as_f64) {
        Some(number) if number.is_finite() && number > 0.0 => Ok(number),
        Some(number) => Err(format!("{field} must be above zero, got {number}")),
        None => Err(format!("missing or non numeric {field}")),
    }
}

#[cfg(test)]
mod tests {
    use super::{Sample, parse};

    const FIXTURE: &str = include_str!("../../tests/fixtures/memtier-2.5.2.json");

    #[test]
    fn parses_memtier_output() {
        let expected = Sample {
            ops_per_sec: 26999.25,
            p99_ms: 11.647,
        };
        assert_eq!(parse(FIXTURE), Ok(expected));
    }

    /// A load whose GETs all miss measures the "absent key" path, not a
    /// read: the keyspace was not preloaded.
    #[test]
    fn gets_that_all_miss_are_errors() {
        let json = r#"{"ALL STATS": {
            "Gets": {"Ops/sec": 900.0, "Hits/sec": 0.0, "Misses/sec": 900.0},
            "Totals": {"Ops/sec": 1000.0, "Percentile Latencies": {"p99.00": 1.0}}}}"#;
        let error = parse(json).expect_err("all GETs miss");
        assert!(error.contains("Hits/sec"), "{error}");
        let no_gets = r#"{"ALL STATS": {
            "Gets": {"Ops/sec": 0.0, "Hits/sec": 0.0},
            "Totals": {"Ops/sec": 1000.0, "Percentile Latencies": {"p99.00": 1.0}}}}"#;
        assert!(parse(no_gets).is_ok(), "a SET-only load has no GET to hit");
    }

    #[test]
    fn missing_or_invalid_fields_are_errors() {
        let cases = [
            ("not json", "json"),
            (r#"{"ALL STATS": {}}"#, "Totals"),
            (
                r#"{"ALL STATS": {"Totals": {"Percentile Latencies": {"p99.00": 1.0}}}}"#,
                "Ops/sec",
            ),
            (
                r#"{"ALL STATS": {"Totals": {"Ops/sec": "fast", "Percentile Latencies": {"p99.00": 1.0}}}}"#,
                "Ops/sec",
            ),
            (
                r#"{"ALL STATS": {"Totals": {"Ops/sec": 0.0, "Percentile Latencies": {"p99.00": 1.0}}}}"#,
                "Ops/sec",
            ),
            (
                r#"{"ALL STATS": {"Totals": {"Ops/sec": 10.0, "Percentile Latencies": {}}}}"#,
                "p99.00",
            ),
            (
                r#"{"ALL STATS": {"Totals": {"Ops/sec": 10.0, "Percentile Latencies": {"p99.00": -1}}}}"#,
                "p99.00",
            ),
        ];
        for (json, field) in cases {
            let error = parse(json).expect_err(json);
            assert!(error.contains(field), "{json}: {error}");
        }
    }
}
