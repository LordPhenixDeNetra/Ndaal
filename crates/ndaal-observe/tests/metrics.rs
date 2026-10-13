//! Lock-free per-thread metrics aggregated at read time (T01.11).

use std::thread;

use ndaal_observe::metrics::{LATENCY_BUCKETS, MetricsError, Registry};

fn line_value(text: &str, series: &str) -> Option<String> {
    text.lines()
        .find_map(|l| {
            l.strip_prefix(series)
                .and_then(|rest| rest.strip_prefix(' '))
        })
        .map(str::to_owned)
}

#[test]
fn counter_aggregates_across_threads() {
    let registry = Registry::default();
    let counter = registry
        .counter("ndaal_test_total", "Test.", &[])
        .expect("counter");
    thread::scope(|s| {
        for _ in 0..8 {
            s.spawn(|| (0..10_000).for_each(|_| counter.increment(1)));
        }
    });
    assert_eq!(
        line_value(&registry.render(), "ndaal_test_total").as_deref(),
        Some("80000")
    );
}

#[test]
fn more_threads_than_slots_stay_exact() {
    let registry = Registry::default();
    let counter = registry
        .counter("ndaal_many_total", "Test.", &[])
        .expect("counter");
    thread::scope(|s| {
        for _ in 0..100 {
            s.spawn(|| (0..100).for_each(|_| counter.increment(1)));
        }
    });
    assert_eq!(
        line_value(&registry.render(), "ndaal_many_total").as_deref(),
        Some("10000")
    );
}

#[test]
fn histogram_buckets_are_cumulative() {
    let registry = Registry::default();
    let latency = registry
        .histogram(
            "ndaal_test_seconds",
            "Test.",
            &[("proto", "resp")],
            &LATENCY_BUCKETS,
        )
        .expect("histogram");
    for v in [0.000_001_5, 0.5, 100.0] {
        latency.observe(v);
    }
    let text = registry.render();
    let bucket = |le: &str| {
        line_value(
            &text,
            &format!("ndaal_test_seconds_bucket{{proto=\"resp\",le=\"{le}\"}}"),
        )
    };
    assert_eq!(bucket("0.000001").as_deref(), Some("0"));
    assert_eq!(bucket("0.000002").as_deref(), Some("1"));
    assert_eq!(bucket("0.524288").as_deref(), Some("2"));
    assert_eq!(bucket("16.777216").as_deref(), Some("2"));
    assert_eq!(bucket("+Inf").as_deref(), Some("3"));
    let count = line_value(&text, "ndaal_test_seconds_count{proto=\"resp\"}");
    assert_eq!(count.as_deref(), Some("3"));
    let sum: f64 = line_value(&text, "ndaal_test_seconds_sum{proto=\"resp\"}")
        .expect("sum")
        .parse()
        .expect("number");
    assert!((sum - 100.500_001_5).abs() < 1e-9, "{sum}");
}

#[test]
fn series_are_shared_by_name_and_labels() {
    let registry = Registry::default();
    let a = registry
        .counter("ndaal_req_total", "Requests.", &[("result", "ok")])
        .expect("a");
    let b = registry
        .counter("ndaal_req_total", "Requests.", &[("result", "ok")])
        .expect("b");
    let c = registry
        .counter("ndaal_req_total", "Requests.", &[("result", "err")])
        .expect("c");
    a.increment(2);
    b.increment(3);
    c.increment(1);
    let text = registry.render();
    assert_eq!(
        line_value(&text, "ndaal_req_total{result=\"ok\"}").as_deref(),
        Some("5")
    );
    assert_eq!(
        line_value(&text, "ndaal_req_total{result=\"err\"}").as_deref(),
        Some("1")
    );
    assert_eq!(text.matches("# HELP ndaal_req_total ").count(), 1, "{text}");
    assert_eq!(
        text.matches("# TYPE ndaal_req_total counter").count(),
        1,
        "{text}"
    );
}

#[test]
fn label_values_are_escaped() {
    let registry = Registry::default();
    let gauge = registry
        .gauge("ndaal_test_gauge", "Test.", &[("path", "a\"b\\c\nd")])
        .expect("gauge");
    gauge.set(7);
    gauge.add(-2);
    let text = registry.render();
    assert_eq!(
        line_value(&text, "ndaal_test_gauge{path=\"a\\\"b\\\\c\\nd\"}").as_deref(),
        Some("5"),
        "{text}"
    );
}

#[test]
fn output_is_sorted_and_stable() {
    let registry = Registry::default();
    for name in ["ndaal_z_total", "ndaal_a_total", "ndaal_m_total"] {
        registry
            .counter(name, "Test.", &[])
            .expect("counter")
            .increment(1);
    }
    let first = registry.render();
    assert_eq!(first, registry.render());
    let names: Vec<&str> = first.lines().filter(|l| l.starts_with("# TYPE")).collect();
    let mut sorted = names.clone();
    sorted.sort_unstable();
    assert_eq!(names, sorted);
}

#[test]
fn invalid_names_and_kind_mismatches_are_errors() {
    let registry = Registry::default();
    assert!(matches!(
        registry.counter("bad name", "x", &[]),
        Err(MetricsError::InvalidName(_))
    ));
    assert!(matches!(
        registry.counter("ok_total", "x", &[("bad-label", "v")]),
        Err(MetricsError::InvalidName(_))
    ));
    registry.counter("ndaal_dual", "x", &[]).expect("counter");
    assert!(matches!(
        registry.gauge("ndaal_dual", "x", &[]),
        Err(MetricsError::KindMismatch(_))
    ));
}

// Review of lot 5: output must stay valid Prometheus whatever is registered.
#[test]
fn invalid_histogram_bounds_are_rejected() {
    let registry = Registry::default();
    for bounds in [
        &[2.0, 1.0][..],
        &[1.0, 1.0],
        &[1.0, f64::NAN],
        &[1.0, f64::INFINITY],
        &[],
    ] {
        let made = registry.histogram("ndaal_h_seconds", "x", &[], bounds);
        assert!(
            matches!(made, Err(MetricsError::InvalidBuckets)),
            "{bounds:?}"
        );
    }
}

#[test]
fn nan_observations_are_ignored() {
    let registry = Registry::default();
    let h = registry
        .histogram("ndaal_nan_seconds", "x", &[], &[1.0])
        .expect("histogram");
    h.observe(f64::NAN);
    h.observe(0.5);
    let text = registry.render();
    assert_eq!(
        line_value(&text, "ndaal_nan_seconds_sum").as_deref(),
        Some("0.5"),
        "{text}"
    );
    assert_eq!(
        line_value(&text, "ndaal_nan_seconds_count").as_deref(),
        Some("1"),
        "{text}"
    );
}

#[test]
fn duplicate_and_reserved_label_names_are_rejected() {
    let registry = Registry::default();
    let dup = registry.counter("ndaal_l_total", "x", &[("a", "1"), ("a", "2")]);
    assert!(matches!(dup, Err(MetricsError::InvalidName(_))));
    let reserved = registry.counter("ndaal_l_total", "x", &[("__name__", "v")]);
    assert!(matches!(reserved, Err(MetricsError::InvalidName(_))));
}

#[test]
fn help_text_is_escaped() {
    let registry = Registry::default();
    registry
        .counter("ndaal_help_total", "C:\\x\nnext", &[])
        .expect("counter");
    let text = registry.render();
    assert!(
        text.contains("# HELP ndaal_help_total C:\\\\x\\nnext\n"),
        "{text}"
    );
}
