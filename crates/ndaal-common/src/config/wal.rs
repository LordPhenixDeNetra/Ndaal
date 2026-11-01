//! Sections `[wal]` and `[durability]` (J02-C2): the log files and the
//! group commit (ARCHITECTURE.md section 6).

use std::time::Duration;

use super::runtime::{format_duration, parse_duration};
use super::{ConfigError, Values, invalid_value, value};

const KIB: u64 = 1 << 10;
const MIB: u64 = 1 << 20;
const GIB: u64 = 1 << 30;
/// A disk page: log files hold a whole number of them.
const PAGE: u64 = 4096;

/// Bounds of `wal.file_size`: a file must hold several 2 MiB flushes, and
/// past 1 GiB a file only delays the reuse of log space.
pub(super) const MIN_FILE_SIZE: u64 = 4 * MIB;
/// See [`MIN_FILE_SIZE`].
pub(super) const MAX_FILE_SIZE: u64 = GIB;
/// Bounds of `durability.group_commit_max_bytes`: one page, up to the
/// largest flush (ADR-033).
pub(super) const MIN_GROUP_BYTES: u64 = 4 * KIB;
/// See [`MIN_GROUP_BYTES`].
pub(super) const MAX_GROUP_BYTES: u64 = 2 * MIB;

/// Section `[wal]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WalConfig {
    /// `wal.file_size`: bytes reserved for each log file.
    pub file_size: u64,
}

/// Section `[durability]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DurabilityConfig {
    /// `durability.async_flush_interval`: longest delay before an `async`
    /// record is on disk.
    pub async_flush_interval: Duration,
    /// `durability.group_commit_max_delay`: longest wait of a record for
    /// others to share its flush.
    pub group_commit_max_delay: Duration,
    /// `durability.group_commit_max_bytes`: buffered bytes that start a
    /// flush at once.
    pub group_commit_max_bytes: u64,
}

/// An integer followed by `B`, `KiB`, `MiB` or `GiB`, from `min` to `max`
/// bytes.
pub(super) fn parse_size(text: &str, min: u64, max: u64) -> Option<u64> {
    let split = text.find(|c: char| !c.is_ascii_digit())?;
    let (digits, unit) = text.split_at(split);
    if digits.is_empty() {
        return None;
    }
    let unit = match unit {
        "B" => 1,
        "KiB" => KIB,
        "MiB" => MIB,
        "GiB" => GIB,
        _ => return None,
    };
    let bytes = digits.parse::<u64>().ok()?.checked_mul(unit)?;
    (min..=max).contains(&bytes).then_some(bytes)
}

/// `bytes` as the shortest size text the configuration reads back:
/// `64MiB`, `4097B`.
pub fn format_size(bytes: u64) -> String {
    for (unit, name) in [(GIB, "GiB"), (MIB, "MiB"), (KIB, "KiB")] {
        if bytes != 0 && bytes.is_multiple_of(unit) {
            return format!("{}{name}", bytes / unit);
        }
    }
    format!("{bytes}B")
}

pub(super) fn build_wal(values: &Values) -> Result<WalConfig, ConfigError> {
    let (size, origin) = value(values, "wal.file_size");
    Ok(WalConfig {
        // Flushes start and end on 4 KiB pages: a file is a whole number
        // of them.
        file_size: parse_size(size, MIN_FILE_SIZE, MAX_FILE_SIZE)
            .filter(|bytes| bytes.is_multiple_of(PAGE))
            .ok_or_else(|| invalid_value("wal.file_size", origin))?,
    })
}

pub(super) fn build_durability(values: &Values) -> Result<DurabilityConfig, ConfigError> {
    let duration = |key: &'static str| {
        let (text, origin) = value(values, key);
        parse_duration(text).ok_or_else(|| invalid_value(key, origin))
    };
    let (bytes, bytes_origin) = value(values, "durability.group_commit_max_bytes");
    Ok(DurabilityConfig {
        async_flush_interval: duration("durability.async_flush_interval")?,
        group_commit_max_delay: duration("durability.group_commit_max_delay")?,
        group_commit_max_bytes: parse_size(bytes, MIN_GROUP_BYTES, MAX_GROUP_BYTES)
            .ok_or_else(|| invalid_value("durability.group_commit_max_bytes", bytes_origin))?,
    })
}

pub(super) fn wal_entries(
    wal: &WalConfig,
    durability: &DurabilityConfig,
) -> Vec<(&'static str, String)> {
    vec![
        ("wal.file_size", format_size(wal.file_size)),
        (
            "durability.async_flush_interval",
            format_duration(durability.async_flush_interval),
        ),
        (
            "durability.group_commit_max_delay",
            format_duration(durability.group_commit_max_delay),
        ),
        (
            "durability.group_commit_max_bytes",
            format_size(durability.group_commit_max_bytes),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_size_round_trips() {
        for bytes in [4096, 4097, 256 * KIB, 3 * MIB, 64 * MIB, GIB] {
            assert_eq!(
                parse_size(&format_size(bytes), 1, GIB),
                Some(bytes),
                "{bytes}"
            );
        }
    }

    #[test]
    fn overflow_is_refused() {
        assert_eq!(parse_size("99999999999999999999GiB", 1, u64::MAX), None);
        assert_eq!(parse_size("17179869184GiB", 1, u64::MAX), None);
    }
}
