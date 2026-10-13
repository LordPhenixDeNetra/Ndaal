//! Configuration sources and their priority (T01.06, EF-GEN-04, EF-GEN-05).

use std::net::SocketAddr;
use std::path::PathBuf;

use ndaal_common::config::{
    self, Config, ConfigError, Expected, Kind, LangChoice, LogFormatChoice, Origin, Sources,
};

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
    })
}

#[test]
fn defaults_without_any_source() {
    let c = load(None, &[], &[]).expect("defaults are valid");
    assert_eq!(c.node.data_dir, PathBuf::from("/var/lib/ndaal"));
    assert_eq!(c.node.name, "");
    assert_eq!(c.node.lang, LangChoice::Auto);
    assert_eq!(
        c.http.listen,
        "127.0.0.1:8080".parse::<SocketAddr>().expect("addr")
    );
    assert_eq!(c.log.level, "info");
    assert_eq!(c.log.format, LogFormatChoice::Json);
}

#[test]
fn command_line_beats_environment_beats_file() {
    let file = "[node]\ndata_dir = \"/file\"\nname = \"from-file\"\nlang = \"fr\"\n";
    let env = [
        ("NDAAL_NODE__DATA_DIR", "/env"),
        ("NDAAL_NODE__NAME", "from-env"),
    ];
    let cli = [("node.data_dir", "/cli")];
    let c = load(Some(file), &env, &cli).expect("valid");
    assert_eq!(c.node.data_dir, PathBuf::from("/cli"));
    assert_eq!(c.node.name, "from-env");
    assert_eq!(c.node.lang, LangChoice::Fr);
}

#[test]
fn unknown_keys_name_the_key_and_the_source() {
    let cases = [
        (
            load(Some("[node]\ndata_dirr = \"/x\"\n"), &[], &[]),
            "node.data_dirr",
            Origin::File,
        ),
        (
            load(Some("[nodes]\nname = \"x\"\n"), &[], &[]),
            "nodes",
            Origin::File,
        ),
        (
            load(None, &[("NDAAL_NODE__DATADIR", "/x")], &[]),
            "node.datadir",
            Origin::Env,
        ),
        (
            load(None, &[], &[("log.lvl", "info")]),
            "log.lvl",
            Origin::CommandLine,
        ),
    ];
    for (result, key, origin) in cases {
        match result {
            Err(ConfigError::Unknown { key: k, origin: o }) => {
                assert_eq!((k.as_str(), o), (key, origin));
            }
            other => panic!("{key}: {other:?}"),
        }
    }
}

#[test]
fn toml_scalars_are_read_as_text_and_checked_by_kind() {
    let c = load(Some("[node]\nname = 42\n"), &[], &[]).expect("integer name");
    assert_eq!(c.node.name, "42");
    let c = load(Some("[node]\nname = true\n"), &[], &[]).expect("boolean name");
    assert_eq!(c.node.name, "true");
    let cases = [
        (
            "[http]\nlisten = 8080\n",
            "http.listen",
            Expected::Value(Kind::Addr),
        ),
        ("[node]\nname = [1]\n", "node.name", Expected::Scalar),
        ("[node]\nname = { a = 1 }\n", "node.name", Expected::Scalar),
        ("node = 3\n", "node", Expected::Table),
    ];
    for (file, key, expected) in cases {
        match load(Some(file), &[], &[]) {
            Err(ConfigError::Invalid {
                key: k,
                origin: Origin::File,
                expected: e,
            }) => assert_eq!((k.as_str(), e), (key, expected), "{file}"),
            other => panic!("{file}: {other:?}"),
        }
    }
}

#[test]
fn errors_never_repeat_the_rejected_value() {
    let secret = "s3cr3t-value";
    let error = load(None, &[], &[("http.listen", secret)]).expect_err("invalid");
    assert!(!error.to_string().contains(secret), "{error}");
    assert_eq!(
        error,
        ConfigError::Invalid {
            key: "http.listen".into(),
            origin: Origin::CommandLine,
            expected: Expected::Value(Kind::Addr),
        }
    );
}

#[test]
fn mistyped_values_are_errors() {
    let cases = [
        (
            load(Some("[http]\nlisten = 8080\n"), &[], &[]),
            "http.listen",
            Origin::File,
        ),
        (
            load(None, &[("NDAAL_HTTP__LISTEN", "localhost")], &[]),
            "http.listen",
            Origin::Env,
        ),
        (
            load(None, &[], &[("log.level", "loud")]),
            "log.level",
            Origin::CommandLine,
        ),
        (
            load(None, &[], &[("log.format", "xml")]),
            "log.format",
            Origin::CommandLine,
        ),
        (
            load(None, &[], &[("node.lang", "de")]),
            "node.lang",
            Origin::CommandLine,
        ),
        (
            load(None, &[], &[("node.data_dir", "")]),
            "node.data_dir",
            Origin::CommandLine,
        ),
        (load(Some("node = 3\n"), &[], &[]), "node", Origin::File),
    ];
    for (result, key, origin) in cases {
        match result {
            Err(ConfigError::Invalid {
                key: k, origin: o, ..
            }) => {
                assert_eq!((k.as_str(), o), (key, origin));
            }
            other => panic!("{key}: {other:?}"),
        }
    }
}

#[test]
fn log_directives_per_target_are_accepted() {
    let c = load(None, &[], &[("log.level", "ndaal_wal=debug,info")]).expect("valid");
    assert_eq!(c.log.level, "ndaal_wal=debug,info");
}

#[test]
fn foreign_environment_variables_are_ignored() {
    let env = [("NDAAL_HOME", "/x"), ("NDAAL_LANG", "fr"), ("PATH", "/bin")];
    assert_eq!(
        load(None, &env, &[]).expect("valid"),
        load(None, &[], &[]).expect("valid")
    );
}

#[test]
fn invalid_toml_is_a_syntax_error() {
    assert!(matches!(
        load(Some("[node\n"), &[], &[]),
        Err(ConfigError::Syntax { .. })
    ));
}

#[test]
fn reference_lists_every_parameter() {
    let reference = config::reference();
    for param in config::PARAMS {
        let line = reference
            .lines()
            .find(|line| line.starts_with(param.key))
            .unwrap_or_else(|| panic!("{} missing", param.key));
        assert!(line.contains(param.kind.name()), "{line}");
        assert!(line.contains(&format!("{:?}", param.default)), "{line}");
        assert!(line.contains(&config::env_name(param.key)), "{line}");
    }
    assert!(reference.contains("NDAAL_NODE__DATA_DIR"));
}

#[test]
fn rendered_configuration_loads_back_identically() {
    let cli = [
        ("node.data_dir", "C:\\data \"quoted\""),
        ("node.name", "n1"),
        ("log.format", "text"),
    ];
    let original = load(None, &[], &cli).expect("valid");
    let rendered = config::render(&original);
    assert_eq!(load(Some(&rendered), &[], &[]).expect("valid"), original);
}
