//! Prometheus text exposition format 0.0.4.

use std::collections::BTreeMap;
use std::fmt::Write;

use super::{Family, Labels, Metric};

/// `\`, `"` (label values only) and line feeds escaped, as the format requires
/// in label values and HELP texts.
fn escape_with(value: &str, quote: bool) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' if quote => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            other => out.push(other),
        }
    }
    out
}

fn escape(value: &str) -> String {
    escape_with(value, true)
}

/// `{a="1",b="2"}` (with `extra` appended), or nothing when there is no label.
fn label_set(labels: &Labels, extra: Option<(&str, &str)>) -> String {
    let pairs: Vec<String> = labels
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .chain(extra)
        .map(|(k, v)| format!("{k}=\"{}\"", escape(v)))
        .collect();
    if pairs.is_empty() {
        String::new()
    } else {
        format!("{{{}}}", pairs.join(","))
    }
}

pub(super) fn render(families: &BTreeMap<String, Family>) -> String {
    let mut out = String::new();
    for (name, family) in families {
        let Some(kind) = family.series.values().next().map(Metric::kind) else {
            continue;
        };
        // Writing to a String cannot fail.
        let _ = writeln!(out, "# HELP {name} {}", escape_with(&family.help, false));
        let _ = writeln!(out, "# TYPE {name} {kind}");
        for (labels, metric) in &family.series {
            series(&mut out, name, labels, metric);
        }
    }
    out
}

fn series(out: &mut String, name: &str, labels: &Labels, metric: &Metric) {
    match metric {
        Metric::Counter(c) => {
            let _ = writeln!(out, "{name}{} {}", label_set(labels, None), c.value());
        }
        Metric::Gauge(g) => {
            let _ = writeln!(out, "{name}{} {}", label_set(labels, None), g.value());
        }
        Metric::Histogram(h) => {
            let (counts, sum) = h.snapshot();
            let mut cumulative = 0;
            for (bound, count) in h.bounds.iter().zip(&counts) {
                cumulative += count;
                let le = bound.to_string();
                let set = label_set(labels, Some(("le", &le)));
                let _ = writeln!(out, "{name}_bucket{set} {cumulative}");
            }
            cumulative += counts.last().copied().unwrap_or(0);
            let set = label_set(labels, Some(("le", "+Inf")));
            let _ = writeln!(out, "{name}_bucket{set} {cumulative}");
            let _ = writeln!(out, "{name}_sum{} {sum}", label_set(labels, None));
            let _ = writeln!(out, "{name}_count{} {cumulative}", label_set(labels, None));
        }
    }
}
