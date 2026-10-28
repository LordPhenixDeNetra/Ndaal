//! `[io]` parameters (J02-B1, T02.05, T02.06): the I/O scheduler and its
//! thread pool.

use ndaal_common::config::{self, Config, ConfigError, Expected, IoBackend, Kind, Ratio, Sources};

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
fn io_defaults() {
    let io = load(None, &[]).expect("defaults").io;
    assert_eq!(io.backend, IoBackend::Auto);
    assert_eq!(io.queue_depth, 32);
    assert_eq!(io.background_share, Ratio::from_ppm(300_000));
}

#[test]
fn ratios() {
    let ok = [
        ("0", 0),
        ("0.3", 300_000),
        ("0.30", 300_000),
        ("1", 1_000_000),
        ("1.0", 1_000_000),
        ("0.000001", 1),
    ];
    for (text, ppm) in ok {
        let c = load(None, &[("io.background_bandwidth_share", text)]).expect(text);
        assert_eq!(c.io.background_share, Ratio::from_ppm(ppm), "{text}");
    }
    for bad in ["1.1", "-0.1", ".5", "0.1234567", "x", "", "1.", "+0.5", "2"] {
        assert_eq!(
            expected_of("io.background_bandwidth_share", bad),
            Expected::Value(Kind::Ratio),
            "{bad}"
        );
    }
}

#[test]
fn ratio_as_f64() {
    assert!((Ratio::from_ppm(300_000).as_f64() - 0.3).abs() < 1e-12);
}

#[test]
fn ratio_from_a_toml_float() {
    let c = load(Some("[io]\nbackground_bandwidth_share = 0.25\n"), &[]).expect("float");
    assert_eq!(c.io.background_share, Ratio::from_ppm(250_000));
}

#[test]
fn queue_depth_range() {
    for (text, depth) in [("1", 1), ("1024", 1024)] {
        let c = load(None, &[("io.queue_depth", text)]).expect(text);
        assert_eq!(c.io.queue_depth, depth);
    }
    for bad in ["0", "1025", "-1", "x", "+3", ""] {
        assert_eq!(
            expected_of("io.queue_depth", bad),
            Expected::Value(Kind::Range(1, 1024)),
            "{bad}"
        );
    }
}

#[test]
fn io_backend_choice() {
    let c = load(None, &[("io.backend", "threads")]).expect("threads");
    assert_eq!(c.io.backend, IoBackend::Threads);
    // io_uring arrives with J07.
    assert!(matches!(
        expected_of("io.backend", "uring"),
        Expected::Value(Kind::Choice(_))
    ));
}

#[test]
fn io_render_loads_back() {
    let c = load(
        None,
        &[
            ("io.backend", "threads"),
            ("io.queue_depth", "7"),
            ("io.background_bandwidth_share", "0.125"),
        ],
    )
    .expect("valid");
    let text = config::render(&c);
    assert!(text.contains("[io]\n"), "{text}");
    assert_eq!(load(Some(&text), &[]).expect("reload"), c, "{text}");
}
