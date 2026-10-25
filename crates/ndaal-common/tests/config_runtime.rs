//! `[runtime]` parameters (J02-A, T02.01 to T02.03): the first keys with
//! two levels (`runtime.cores.shard`).

use std::time::Duration;

use ndaal_common::config::{self, Config, ConfigError, CoreCount, Expected, Kind, Origin, Sources};

fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect()
}

fn load(
    file: Option<&str>,
    env: &[(&str, &str)],
    cli: &[(&str, &str)],
) -> Result<Config, ConfigError> {
    config::load(&Sources {
        file,
        env: pairs(env),
        overrides: pairs(cli),
        ..Sources::default()
    })
}

#[test]
fn runtime_defaults() {
    let runtime = load(None, &[], &[]).expect("defaults").runtime;
    let cores = runtime.cores;
    for count in [
        cores.total,
        cores.shard,
        cores.vector,
        cores.transfer,
        cores.background,
        cores.io,
    ] {
        assert_eq!(count, CoreCount::Auto);
    }
    assert!(cores.pin);
    assert_eq!(runtime.idle_spin, Duration::from_micros(50));
}

#[test]
fn nested_table_and_env() {
    let file = "[runtime]\nidle_spin = \"1ms\"\n[runtime.cores]\nshard = 4\n";
    let c = load(
        Some(file),
        &[("NDAAL_RUNTIME__CORES__PIN", "false")],
        &[("runtime.idle_spin", "2ms")],
    )
    .expect("valid");
    assert_eq!(c.runtime.cores.shard, CoreCount::Fixed(4));
    assert!(!c.runtime.cores.pin);
    assert_eq!(c.runtime.idle_spin, Duration::from_millis(2));
    assert_eq!(
        config::env_name("runtime.cores.shard"),
        "NDAAL_RUNTIME__CORES__SHARD"
    );
}

#[test]
fn unknown_nested_keys_are_errors() {
    let cases = [
        "[runtime.cores]\nshards = 4\n",
        "[runtime.core]\nshard = 4\n",
        "[runtime]\ncores = 4\n",
    ];
    for file in cases {
        assert!(load(Some(file), &[], &[]).is_err(), "{file}");
    }
    let error = load(None, &[("NDAAL_RUNTIME__CORES__SHARDS", "4")], &[]).expect_err("env");
    assert!(matches!(
        error,
        ConfigError::Unknown {
            origin: Origin::Env,
            ..
        }
    ));
}

#[test]
fn durations() {
    let ok = [
        ("0", Duration::ZERO),
        ("250us", Duration::from_micros(250)),
        ("10ms", Duration::from_millis(10)),
        ("2s", Duration::from_secs(2)),
    ];
    for (text, expected) in ok {
        let c = load(None, &[], &[("runtime.idle_spin", text)]).expect(text);
        assert_eq!(c.runtime.idle_spin, expected, "{text}");
    }
    for bad in ["5", "1h", "-1ms", "20s", "1.5ms", "ms", ""] {
        match load(None, &[], &[("runtime.idle_spin", bad)]) {
            Err(ConfigError::Invalid { expected, .. }) => {
                assert_eq!(expected, Expected::Value(Kind::Duration), "{bad}");
            }
            other => panic!("{bad}: {other:?}"),
        }
    }
}

#[test]
fn counts() {
    for (text, expected) in [
        ("auto", CoreCount::Auto),
        ("0", CoreCount::Fixed(0)),
        ("64", CoreCount::Fixed(64)),
    ] {
        let c = load(None, &[], &[("runtime.cores.vector", text)]).expect(text);
        assert_eq!(c.runtime.cores.vector, expected, "{text}");
    }
    for bad in ["-1", "x", "5000", "1.5"] {
        match load(None, &[], &[("runtime.cores.vector", bad)]) {
            Err(ConfigError::Invalid { expected, .. }) => {
                assert_eq!(expected, Expected::Value(Kind::Count), "{bad}");
            }
            other => panic!("{bad}: {other:?}"),
        }
    }
}

#[test]
fn nested_render_loads_back() {
    let c = load(
        None,
        &[],
        &[
            ("runtime.cores.shard", "3"),
            ("runtime.cores.pin", "false"),
            ("runtime.idle_spin", "1500us"),
        ],
    )
    .expect("valid");
    let text = config::render(&c);
    assert!(text.contains("[runtime.cores]\n"), "{text}");
    assert_eq!(load(Some(&text), &[], &[]).expect("reload"), c, "{text}");
}
