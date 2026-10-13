//! Metrics kept per thread and summed when read (ARCHITECTURE section 16).
//!
//! Each counter and histogram has [`SLOTS`] slots of atomics; a thread writes
//! only to its own slot, with relaxed atomics and no lock, so cores do not
//! contend. Reading (rendering) sums the slots. Only registration and
//! rendering take the registry lock, never the write path.

mod render;

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicI64, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

/// Slots per metric; threads beyond this share slots (still exact, atomics).
const SLOTS: usize = 64;

/// Latency buckets in seconds: 1 us * 2^k for k = 0..24 (1 us to about 16.8 s).
pub const LATENCY_BUCKETS: [f64; 25] = {
    let mut buckets = [0.0; 25];
    let mut k = 0;
    while k < 25 {
        buckets[k] = (1u64 << k) as f64 * 1e-6;
        k += 1;
    }
    buckets
};

/// Errors of metric registration.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MetricsError {
    /// Not a valid Prometheus metric or label name.
    #[error("invalid metric or label name {0:?}")]
    InvalidName(String),
    /// The name is already registered with another kind.
    #[error("metric {0:?} is already registered with another kind")]
    KindMismatch(String),
    /// Histogram bounds must be finite, strictly increasing, and at least one.
    #[error("histogram buckets must be finite and strictly increasing")]
    InvalidBuckets,
}

/// One cache line per slot, so two threads never write the same line.
#[repr(align(64))]
#[derive(Default)]
struct Padded<T>(T);

fn slot() -> usize {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    thread_local! {
        static SLOT: usize = NEXT.fetch_add(1, Ordering::Relaxed) % SLOTS;
    }
    SLOT.with(|slot| *slot)
}

/// A monotonically increasing count.
#[derive(Clone)]
pub struct Counter(Arc<[Padded<AtomicU64>]>);

impl Counter {
    fn new() -> Self {
        Counter((0..SLOTS).map(|_| Padded::default()).collect())
    }

    /// Adds `by`.
    pub fn increment(&self, by: u64) {
        self.0[slot()].0.fetch_add(by, Ordering::Relaxed);
    }

    fn value(&self) -> u64 {
        self.0.iter().map(|s| s.0.load(Ordering::Relaxed)).sum()
    }
}

/// A value that goes up and down (set rarely, from any thread).
#[derive(Clone)]
pub struct Gauge(Arc<AtomicI64>);

impl Gauge {
    /// Sets the value.
    pub fn set(&self, value: i64) {
        self.0.store(value, Ordering::Relaxed);
    }

    /// Adds `delta` (may be negative).
    pub fn add(&self, delta: i64) {
        self.0.fetch_add(delta, Ordering::Relaxed);
    }

    fn value(&self) -> i64 {
        self.0.load(Ordering::Relaxed)
    }
}

/// Per-slot state of a histogram: one count per bucket plus the overflow.
#[derive(Default)]
struct HistogramSlot {
    counts: Box<[AtomicU64]>,
    sum_bits: AtomicU64,
}

/// Distribution of observed values in fixed buckets.
#[derive(Clone)]
pub struct Histogram {
    bounds: Arc<[f64]>,
    slots: Arc<[Padded<HistogramSlot>]>,
}

impl Histogram {
    fn new(bounds: &[f64]) -> Self {
        let slots = (0..SLOTS)
            .map(|_| {
                let counts = (0..=bounds.len()).map(|_| AtomicU64::new(0)).collect();
                Padded(HistogramSlot {
                    counts,
                    sum_bits: AtomicU64::new(0),
                })
            })
            .collect();
        Histogram {
            bounds: bounds.into(),
            slots,
        }
    }

    /// Records one value; NaN is ignored (it would poison the sum forever).
    pub fn observe(&self, value: f64) {
        if value.is_nan() {
            return;
        }
        let bucket = self
            .bounds
            .iter()
            .position(|b| value <= *b)
            .unwrap_or(self.bounds.len());
        let slot = &self.slots[slot()].0;
        slot.counts[bucket].fetch_add(1, Ordering::Relaxed);
        // The slot belongs to this thread (or a few), so the loop rarely retries.
        let _ = slot
            .sum_bits
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |bits| {
                Some((f64::from_bits(bits) + value).to_bits())
            });
    }

    /// Per-bucket counts (last = above every bound) and the sum.
    fn snapshot(&self) -> (Vec<u64>, f64) {
        let mut counts = vec![0u64; self.bounds.len() + 1];
        let mut sum = 0.0;
        for slot in self.slots.iter() {
            for (total, count) in counts.iter_mut().zip(slot.0.counts.iter()) {
                *total += count.load(Ordering::Relaxed);
            }
            sum += f64::from_bits(slot.0.sum_bits.load(Ordering::Relaxed));
        }
        (counts, sum)
    }
}

#[derive(Clone)]
enum Metric {
    Counter(Counter),
    Gauge(Gauge),
    Histogram(Histogram),
}

impl Metric {
    fn kind(&self) -> &'static str {
        match self {
            Metric::Counter(_) => "counter",
            Metric::Gauge(_) => "gauge",
            Metric::Histogram(_) => "histogram",
        }
    }
}

type Labels = Vec<(String, String)>;

#[derive(Default)]
struct Family {
    help: String,
    series: BTreeMap<Labels, Metric>,
}

/// All metrics of a process; clones share it.
#[derive(Clone, Default)]
pub struct Registry {
    families: Arc<Mutex<BTreeMap<String, Family>>>,
}

fn valid_name(name: &str, colon_allowed: bool) -> bool {
    let mut chars = name.chars();
    let ok = |c: char| c.is_ascii_alphanumeric() || c == '_' || (colon_allowed && c == ':');
    chars.next().is_some_and(|c| !c.is_ascii_digit() && ok(c)) && chars.all(ok)
}

impl Registry {
    /// The counter `name{labels}`, created on first use.
    pub fn counter(
        &self,
        name: &str,
        help: &str,
        labels: &[(&str, &str)],
    ) -> Result<Counter, MetricsError> {
        match self.get_or_insert(name, help, labels, || Metric::Counter(Counter::new()))? {
            Metric::Counter(c) => Ok(c),
            _ => Err(MetricsError::KindMismatch(name.to_owned())),
        }
    }

    /// The gauge `name{labels}`, created on first use.
    pub fn gauge(
        &self,
        name: &str,
        help: &str,
        labels: &[(&str, &str)],
    ) -> Result<Gauge, MetricsError> {
        match self.get_or_insert(name, help, labels, || Metric::Gauge(Gauge(Arc::default())))? {
            Metric::Gauge(g) => Ok(g),
            _ => Err(MetricsError::KindMismatch(name.to_owned())),
        }
    }

    /// The histogram `name{labels}` with `buckets` (upper bounds, increasing).
    pub fn histogram(
        &self,
        name: &str,
        help: &str,
        labels: &[(&str, &str)],
        buckets: &[f64],
    ) -> Result<Histogram, MetricsError> {
        let increasing = buckets.windows(2).all(|pair| pair[0] < pair[1]);
        if buckets.is_empty() || !increasing || !buckets.iter().all(|b| b.is_finite()) {
            return Err(MetricsError::InvalidBuckets);
        }
        match self.get_or_insert(name, help, labels, || {
            Metric::Histogram(Histogram::new(buckets))
        })? {
            Metric::Histogram(h) => Ok(h),
            _ => Err(MetricsError::KindMismatch(name.to_owned())),
        }
    }

    /// Prometheus text exposition format 0.0.4, sorted by name then labels.
    pub fn render(&self) -> String {
        let families = self.families.lock().unwrap_or_else(PoisonError::into_inner);
        render::render(&families)
    }

    fn get_or_insert(
        &self,
        name: &str,
        help: &str,
        labels: &[(&str, &str)],
        make: impl FnOnce() -> Metric,
    ) -> Result<Metric, MetricsError> {
        if !valid_name(name, true) {
            return Err(MetricsError::InvalidName(name.to_owned()));
        }
        let mut sorted: Labels = Vec::with_capacity(labels.len());
        for (key, value) in labels {
            // `le` is the bucket label; `__` names are reserved by Prometheus.
            if !valid_name(key, false) || *key == "le" || key.starts_with("__") {
                return Err(MetricsError::InvalidName((*key).to_owned()));
            }
            sorted.push(((*key).to_owned(), (*value).to_owned()));
        }
        sorted.sort();
        if let Some(pair) = sorted.windows(2).find(|pair| pair[0].0 == pair[1].0) {
            return Err(MetricsError::InvalidName(pair[0].0.clone()));
        }
        let mut families = self.families.lock().unwrap_or_else(PoisonError::into_inner);
        let family = families.entry(name.to_owned()).or_insert_with(|| Family {
            help: help.to_owned(),
            series: BTreeMap::new(),
        });
        let fresh = make();
        let kind_taken = family.series.values().next().map(Metric::kind);
        if kind_taken.is_some_and(|kind| kind != fresh.kind()) {
            return Err(MetricsError::KindMismatch(name.to_owned()));
        }
        Ok(family.series.entry(sorted).or_insert(fresh).clone())
    }
}
