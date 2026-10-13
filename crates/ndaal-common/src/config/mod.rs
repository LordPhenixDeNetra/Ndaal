//! Node configuration (T01.06, EF-GEN-04, EF-GEN-05).
//!
//! A pure function: the caller reads the file and the environment (through
//! `ndaal-io`, ADR-017) and passes their contents. Priority, lowest first:
//! defaults, TOML file, `NDAAL_<SECTION>__<KEY>` variables, command line.
//! Every value is validated and an unknown key is an error, so a typo never
//! silently falls back to a default.

mod params;

use std::collections::BTreeMap;
use std::fmt;
use std::net::SocketAddr;
use std::path::PathBuf;

pub use params::{Kind, LOG_LEVELS, PARAMS, Param, env_name, is_log_directive, reference};

use crate::error::{Classify, ErrorClass};

/// Where a value comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// Built-in default.
    Default,
    /// TOML configuration file.
    File,
    /// `NDAAL_<SECTION>__<KEY>` environment variable.
    Env,
    /// Command line option.
    CommandLine,
}

impl fmt::Display for Origin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Default => "defaults",
            Self::File => "configuration file",
            Self::Env => "environment",
            Self::CommandLine => "command line",
        })
    }
}

/// A configuration that cannot be used.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConfigError {
    /// The file is not valid TOML.
    #[error("invalid configuration file: {message}")]
    Syntax {
        /// Parser message, with the line.
        message: String,
    },
    /// A key no parameter has (often a typo).
    #[error("unknown configuration key {key:?} in the {origin}")]
    Unknown {
        /// The key, `section.key` or a section name.
        key: String,
        /// Where it was found.
        origin: Origin,
    },
    /// A value of the wrong type or out of range. The value itself is not
    /// kept: it may be a secret.
    #[error("invalid value for {key:?} in the {origin}: expected {expected}")]
    Invalid {
        /// The key, `section.key` or a section name.
        key: String,
        /// Where it was found.
        origin: Origin,
        /// What was expected, for a translated message.
        expected: Expected,
    },

    /// A public listen address without authentication (ENF-SEC-04).
    #[error(
        "{key:?} in the {origin} listens on a non-loopback address without authentication; set auth.allow_insecure = true to allow it"
    )]
    PublicWithoutAuth {
        /// The listen address parameter.
        key: String,
        /// Where its value comes from.
        origin: Origin,
    },
}

/// What an invalid value should have been.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Expected {
    /// A value of the parameter's kind.
    Value(Kind),
    /// A `[section]` table.
    Table,
    /// A single value, not an array or a table.
    Scalar,
}

impl fmt::Display for Expected {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Value(kind) => f.write_str(&kind.expected()),
            Self::Table => f.write_str("a table"),
            Self::Scalar => f.write_str("a single value"),
        }
    }
}

impl Classify for ConfigError {
    fn class(&self) -> ErrorClass {
        ErrorClass::Client
    }
}

/// The configuration sources, read by the caller.
#[derive(Debug, Default)]
pub struct Sources<'a> {
    /// Contents of the TOML file, if there is one.
    pub file: Option<&'a str>,
    /// Environment variables; those not named `NDAAL_<SECTION>__<KEY>` are
    /// ignored.
    pub env: Vec<(String, String)>,
    /// Command line values, `(section.key, value)`, in order.
    pub overrides: Vec<(String, String)>,
    /// Defaults that depend on the system (`node.data_dir`, P-34), applied
    /// over the built-in ones, below every other source.
    pub defaults: Vec<(String, String)>,
}

/// Value of `node.lang`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LangChoice {
    /// Follow the system language.
    Auto,
    /// English.
    En,
    /// French.
    Fr,
}

/// Value of `log.format`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFormatChoice {
    /// One readable line per event.
    Text,
    /// One JSON object per line.
    Json,
}

/// Section `[node]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeConfig {
    /// `node.data_dir`.
    pub data_dir: PathBuf,
    /// `node.name`; empty means the host name.
    pub name: String,
    /// `node.lang`.
    pub lang: LangChoice,
}

/// Section `[http]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpConfig {
    /// `http.listen`.
    pub listen: SocketAddr,
}

/// Section `[auth]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthConfig {
    /// `auth.allow_insecure`: listen on a public address without
    /// authentication.
    pub allow_insecure: bool,
}

/// Section `[log]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogConfig {
    /// `log.level`, validated by [`is_log_directive`].
    pub level: String,
    /// `log.format`.
    pub format: LogFormatChoice,
}

/// The validated configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// Section `[node]`.
    pub node: NodeConfig,
    /// Section `[http]`.
    pub http: HttpConfig,
    /// Section `[auth]`.
    pub auth: AuthConfig,
    /// Section `[log]`.
    pub log: LogConfig,
}

/// Resolved values: the raw text of each parameter and where it came from.
type Values = BTreeMap<&'static str, (String, Origin)>;

/// Merges the sources by priority and validates the result.
pub fn load(sources: &Sources<'_>) -> Result<Config, ConfigError> {
    let mut values: Values = PARAMS
        .iter()
        .map(|p| (p.key, (p.default.to_owned(), Origin::Default)))
        .collect();
    for (key, value) in &sources.defaults {
        set(&mut values, key, value.clone(), Origin::Default)?;
    }
    if let Some(text) = sources.file {
        for (key, value) in file_entries(text)? {
            set(&mut values, &key, value, Origin::File)?;
        }
    }
    for (name, value) in &sources.env {
        if let Some(key) = params::env_key(name) {
            set(&mut values, &key, value.clone(), Origin::Env)?;
        }
    }
    for (key, value) in &sources.overrides {
        set(&mut values, key, value.clone(), Origin::CommandLine)?;
    }
    build(&values)
}

fn set(values: &mut Values, key: &str, value: String, origin: Origin) -> Result<(), ConfigError> {
    let Some(param) = params::find(key) else {
        return Err(ConfigError::Unknown {
            key: key.to_owned(),
            origin,
        });
    };
    if !param.kind.check(&value) {
        return Err(invalid(key, origin, Expected::Value(param.kind)));
    }
    values.insert(param.key, (value, origin));
    Ok(())
}

fn invalid(key: &str, origin: Origin, expected: Expected) -> ConfigError {
    ConfigError::Invalid {
        key: key.to_owned(),
        origin,
        expected,
    }
}

/// Invalid value of a known parameter.
fn invalid_value(key: &str, origin: Origin) -> ConfigError {
    let kind = params::find(key).map_or(Kind::Text, |p| p.kind);
    invalid(key, origin, Expected::Value(kind))
}

/// `(section.key, value)` pairs of the file. Scalars (`42`, `true`, `0.5`)
/// are read as their text, then checked by the parameter's kind like the
/// environment and the command line, which only carry text.
fn file_entries(text: &str) -> Result<Vec<(String, String)>, ConfigError> {
    let table: toml::Table = text
        .parse()
        .map_err(|e: toml::de::Error| ConfigError::Syntax {
            message: e.to_string(),
        })?;
    let mut entries = Vec::new();
    for (section, value) in table {
        let is_section = PARAMS
            .iter()
            .any(|p| p.key.split('.').next() == Some(&section));
        if !is_section {
            return Err(unknown_in_file(&section));
        }
        let toml::Value::Table(keys) = value else {
            return Err(invalid(&section, Origin::File, Expected::Table));
        };
        for (name, value) in keys {
            let key = format!("{section}.{name}");
            if params::find(&key).is_none() {
                return Err(unknown_in_file(&key));
            }
            let text = match value {
                toml::Value::String(text) => text,
                toml::Value::Integer(n) => n.to_string(),
                toml::Value::Float(x) => x.to_string(),
                toml::Value::Boolean(b) => b.to_string(),
                toml::Value::Datetime(d) => d.to_string(),
                toml::Value::Array(_) | toml::Value::Table(_) => {
                    return Err(invalid(&key, Origin::File, Expected::Scalar));
                }
            };
            entries.push((key, text));
        }
    }
    Ok(entries)
}

fn unknown_in_file(key: &str) -> ConfigError {
    ConfigError::Unknown {
        key: key.to_owned(),
        origin: Origin::File,
    }
}

fn build(values: &Values) -> Result<Config, ConfigError> {
    // Every value passed `Kind::check`; the conversions below still return
    // errors rather than assume it.
    let get = |key: &'static str| {
        values
            .get(key)
            .map_or(("", Origin::Default), |(v, o)| (v.as_str(), *o))
    };
    let (listen, listen_origin) = get("http.listen");
    let (lang, lang_origin) = get("node.lang");
    let (format, format_origin) = get("log.format");
    let (allow_insecure, allow_origin) = get("auth.allow_insecure");
    let allow_insecure = match allow_insecure {
        "true" => true,
        "false" => false,
        _ => return Err(invalid_value("auth.allow_insecure", allow_origin)),
    };
    let listen: std::net::SocketAddr = listen
        .parse()
        .map_err(|_| invalid_value("http.listen", listen_origin))?;
    if !listen.ip().is_loopback() && !allow_insecure {
        return Err(ConfigError::PublicWithoutAuth {
            key: "http.listen".to_owned(),
            origin: listen_origin,
        });
    }
    Ok(Config {
        node: NodeConfig {
            data_dir: PathBuf::from(get("node.data_dir").0),
            name: get("node.name").0.to_owned(),
            lang: match lang {
                "auto" => LangChoice::Auto,
                "en" => LangChoice::En,
                "fr" => LangChoice::Fr,
                _ => return Err(invalid_value("node.lang", lang_origin)),
            },
        },
        http: HttpConfig { listen },
        auth: AuthConfig { allow_insecure },
        log: LogConfig {
            level: get("log.level").0.to_owned(),
            format: match format {
                "json" => LogFormatChoice::Json,
                "text" => LogFormatChoice::Text,
                _ => return Err(invalid_value("log.format", format_origin)),
            },
        },
    })
}

/// The values of `config`, by key, in [`PARAMS`] order.
pub fn entries(config: &Config) -> Vec<(&'static str, String)> {
    let lang = match config.node.lang {
        LangChoice::Auto => "auto",
        LangChoice::En => "en",
        LangChoice::Fr => "fr",
    };
    let format = match config.log.format {
        LogFormatChoice::Json => "json",
        LogFormatChoice::Text => "text",
    };
    vec![
        (
            "node.data_dir",
            config.node.data_dir.to_string_lossy().into_owned(),
        ),
        ("node.name", config.node.name.clone()),
        ("node.lang", lang.to_owned()),
        ("http.listen", config.http.listen.to_string()),
        (
            "auth.allow_insecure",
            config.auth.allow_insecure.to_string(),
        ),
        ("log.level", config.log.level.clone()),
        ("log.format", format.to_owned()),
    ]
}

/// `config` as a TOML file (`ndaal config show`); [`load`] reads it back to
/// the same configuration.
pub fn render(config: &Config) -> String {
    let mut out = String::new();
    let mut current = "";
    for (key, value) in entries(config) {
        let (section, name) = key.split_once('.').unwrap_or(("", key));
        if section != current {
            if !current.is_empty() {
                out.push('\n');
            }
            out.push_str(&format!("[{section}]\n"));
            current = section;
        }
        out.push_str(&format!("{name} = {}\n", toml::Value::String(value)));
    }
    out
}
