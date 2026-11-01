//! `[wal]` and `[durability]` parameters (J02-C2, T02.08, T02.11).

use std::time::Duration;

use ndaal_common::config::{self, Config, ConfigError, Expected, Kind, Sources};

const MIB: u64 = 1 << 20;

fn load(file: Option<&str>, cli: &[(&str, &str)]) -> Result<Config, ConfigError> {
    config::load(&Sources {
        file,
        overrides: cli
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect(),
        ..Sources::default()
    })
}

fn expected_of(key: &str, value: &str) -> Expected {
    match load(None, &[(key, value)]) {
        Err(ConfigError::Invalid { expected, .. }) => expected,
        other => panic!("{key}={value}: {other:?}"),
    }
}

#[test]
fn wal_defaults() {
    let c = load(None, &[]).expect("defaults");
    assert_eq!(c.wal.file_size, 64 * MIB);
    assert_eq!(
        c.durability.async_flush_interval,
        Duration::from_millis(100)
    );
    assert_eq!(
        c.durability.group_commit_max_delay,
        Duration::from_micros(200)
    );
    assert_eq!(c.durability.group_commit_max_bytes, 256 * 1024);
}

#[test]
fn sizes() {
    for (text, bytes) in [("4MiB", 4 * MIB), ("64MiB", 64 * MIB), ("1GiB", 1024 * MIB)] {
        let c = load(None, &[("wal.file_size", text)]).expect(text);
        assert_eq!(c.wal.file_size, bytes, "{text}");
    }
    for bad in [
        "3MiB", "2GiB", "64MB", "64", "-1MiB", "1.5MiB", "x", "", "MiB",
        // Not a whole number of 4 KiB pages.
        "4194305B",
    ] {
        assert!(
            matches!(
                expected_of("wal.file_size", bad),
                Expected::Value(Kind::Size(..))
            ),
            "{bad}"
        );
    }
}

#[test]
fn group_commit_max_bytes_range() {
    for (text, bytes) in [("4KiB", 4096), ("4096B", 4096), ("2MiB", 2 * MIB)] {
        let c = load(None, &[("durability.group_commit_max_bytes", text)]).expect(text);
        assert_eq!(c.durability.group_commit_max_bytes, bytes, "{text}");
    }
    for bad in ["4095B", "3KiB", "3MiB"] {
        assert!(
            matches!(
                expected_of("durability.group_commit_max_bytes", bad),
                Expected::Value(Kind::Size(..))
            ),
            "{bad}"
        );
    }
}

#[test]
fn wal_render_loads_back() {
    let c = load(
        None,
        &[
            ("wal.file_size", "8MiB"),
            ("durability.async_flush_interval", "50ms"),
            ("durability.group_commit_max_delay", "1ms"),
            ("durability.group_commit_max_bytes", "64KiB"),
        ],
    )
    .expect("valid");
    let text = config::render(&c);
    assert!(
        text.contains("[wal]\n") && text.contains("[durability]\n"),
        "{text}"
    );
    assert_eq!(load(Some(&text), &[]).expect("reload"), c, "{text}");
}
