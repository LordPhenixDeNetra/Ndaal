//! Section `[runtime]` (J02-A): cores per load class, pinning, idle spin.

use std::time::Duration;

/// Longest `runtime.idle_spin`: past it, a core burns CPU for nothing.
const MAX_DURATION: Duration = Duration::from_secs(10);
/// Largest core count accepted (`runtime.cores.*`).
pub(super) const MAX_COUNT: u32 = 4096;

/// A number of cores, or `auto` (ARCHITECTURE.md section 4.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreCount {
    /// From the default table.
    Auto,
    /// Set by the configuration.
    Fixed(u32),
}

/// Section `[runtime.cores]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoresConfig {
    /// Cores used by the node; `auto`: the cores allowed to the process.
    pub total: CoreCount,
    /// Cores of class `shard`.
    pub shard: CoreCount,
    /// Cores of class `vector`.
    pub vector: CoreCount,
    /// Cores of class `transfer`.
    pub transfer: CoreCount,
    /// Cores of class `background`.
    pub background: CoreCount,
    /// Cores of class `io`.
    pub io: CoreCount,
    /// Pin each core thread to its CPU.
    pub pin: bool,
}

/// Section `[runtime]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeConfig {
    /// `[runtime.cores]`.
    pub cores: CoresConfig,
    /// How long an idle core keeps polling its queues before sleeping.
    pub idle_spin: Duration,
}

/// `0`, or an integer followed by `us`, `ms` or `s`, at most 10 s.
pub(super) fn parse_duration(text: &str) -> Option<Duration> {
    if text == "0" {
        return Some(Duration::ZERO);
    }
    let (digits, unit) = text.split_at(text.find(|c: char| !c.is_ascii_digit())?);
    let n: u64 = digits.parse().ok()?;
    let duration = match unit {
        "us" => Duration::from_micros(n),
        "ms" => Duration::from_millis(n),
        "s" => Duration::from_secs(n),
        _ => return None,
    };
    (duration <= MAX_DURATION).then_some(duration)
}

/// The shortest text [`parse_duration`] reads back as `duration`.
pub(super) fn format_duration(duration: Duration) -> String {
    let micros = duration.as_micros();
    if micros == 0 {
        "0".into()
    } else if micros.is_multiple_of(1_000_000) {
        format!("{}s", micros / 1_000_000)
    } else if micros.is_multiple_of(1_000) {
        format!("{}ms", micros / 1_000)
    } else {
        format!("{micros}us")
    }
}

/// `auto` or an integer from 0 to [`MAX_COUNT`].
pub(super) fn parse_count(text: &str) -> Option<CoreCount> {
    if text == "auto" {
        return Some(CoreCount::Auto);
    }
    // Digits only: `+3` and `-1` are refused, though `u32` accepts `+3`.
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let n: u32 = text.parse().ok()?;
    (n <= MAX_COUNT).then_some(CoreCount::Fixed(n))
}

/// The text of a count, read back by [`parse_count`].
pub(super) fn format_count(count: CoreCount) -> String {
    match count {
        CoreCount::Auto => "auto".into(),
        CoreCount::Fixed(n) => n.to_string(),
    }
}
