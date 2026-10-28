//! Section `[io]` (J02-B1): the I/O scheduler and its thread pool
//! (ARCHITECTURE.md section 4.3).

use super::{ConfigError, Values, invalid_value, value};

/// Parts per million in a whole: a [`Ratio`] of 1.
const PPM: u32 = 1_000_000;
/// Digits after the decimal point a ratio may have: one part per million.
const RATIO_DECIMALS: usize = 6;

/// A share from 0 to 1, kept exactly in parts per million (so it is `Eq` and
/// a rendered configuration reads back the same).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ratio(u32);

impl Ratio {
    /// A share of `ppm` parts per million, at most one million.
    pub fn from_ppm(ppm: u32) -> Self {
        Self(ppm.min(PPM))
    }

    /// Parts per million.
    pub fn ppm(self) -> u32 {
        self.0
    }

    /// The share as a number from 0 to 1.
    pub fn as_f64(self) -> f64 {
        f64::from(self.0) / f64::from(PPM)
    }
}

/// Value of `io.backend`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IoBackend {
    /// The best backend available: the thread pool until `io_uring` (J07).
    Auto,
    /// The thread pool (`pread`, `pwrite`, `fdatasync`).
    Threads,
}

/// Section `[io]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IoConfig {
    /// `io.backend`.
    pub backend: IoBackend,
    /// `io.queue_depth`: operations in flight per device, and I/O threads.
    pub queue_depth: u32,
    /// `io.background_bandwidth_share`: share of the measured bandwidth
    /// the background class may use.
    pub background_share: Ratio,
}

/// Choices of `io.backend`; `uring` arrives with J07.
pub(super) const BACKENDS: &[&str] = &["auto", "threads"];

/// `0`, `1`, or `0.` followed by 1 to 6 digits, or `1.` followed by zeros.
pub(super) fn parse_ratio(text: &str) -> Option<Ratio> {
    let (whole, fraction) = text.split_once('.').unwrap_or((text, "0"));
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    if !digits(whole) || !digits(fraction) || fraction.len() > RATIO_DECIMALS {
        return None;
    }
    let scale = 10u32.pow(u32::try_from(RATIO_DECIMALS - fraction.len()).ok()?);
    let parts = fraction.parse::<u32>().ok()? * scale;
    match whole {
        "0" => Some(Ratio(parts)),
        "1" if parts == 0 => Some(Ratio(PPM)),
        _ => None,
    }
}

/// The shortest text [`parse_ratio`] reads back as `ratio`.
pub(super) fn format_ratio(ratio: Ratio) -> String {
    match ratio.0 {
        0 => "0".into(),
        PPM => "1".into(),
        ppm => {
            let text = format!("0.{ppm:06}");
            text.trim_end_matches('0').to_owned()
        }
    }
}

/// An integer from `min` to `max`, digits only (`+3` is refused).
pub(super) fn parse_range(text: &str, min: u32, max: u32) -> Option<u32> {
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let n: u32 = text.parse().ok()?;
    (min..=max).contains(&n).then_some(n)
}

pub(super) fn build_io(values: &Values) -> Result<IoConfig, ConfigError> {
    let get = |key| value(values, key);
    let (backend, backend_origin) = get("io.backend");
    let (depth, depth_origin) = get("io.queue_depth");
    let (share, share_origin) = get("io.background_bandwidth_share");
    Ok(IoConfig {
        backend: match backend {
            "auto" => IoBackend::Auto,
            "threads" => IoBackend::Threads,
            _ => return Err(invalid_value("io.backend", backend_origin)),
        },
        queue_depth: parse_range(depth, MIN_DEPTH, MAX_DEPTH)
            .ok_or_else(|| invalid_value("io.queue_depth", depth_origin))?,
        background_share: parse_ratio(share)
            .ok_or_else(|| invalid_value("io.background_bandwidth_share", share_origin))?,
    })
}

/// Bounds of `io.queue_depth`: past 1024 threads, a pool costs more memory
/// than any device gains from the depth.
pub(super) const MIN_DEPTH: u32 = 1;
/// See [`MIN_DEPTH`].
pub(super) const MAX_DEPTH: u32 = 1024;

pub(super) fn io_entries(io: &IoConfig) -> Vec<(&'static str, String)> {
    let backend = match io.backend {
        IoBackend::Auto => "auto",
        IoBackend::Threads => "threads",
    };
    vec![
        ("io.backend", backend.to_owned()),
        ("io.queue_depth", io.queue_depth.to_string()),
        (
            "io.background_bandwidth_share",
            format_ratio(io.background_share),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_ratio_round_trips() {
        for ppm in [0, 1, 10, 125_000, 300_000, 999_999, PPM] {
            let ratio = Ratio(ppm);
            assert_eq!(parse_ratio(&format_ratio(ratio)), Some(ratio), "{ppm}");
        }
    }
}
