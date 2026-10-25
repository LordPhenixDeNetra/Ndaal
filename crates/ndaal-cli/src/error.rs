//! Errors of the commands, and their translated messages (ADR-029).

use std::io;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::ExitCode;

use ndaal_common::config::{ConfigError, Expected, Kind, LOG_LEVELS, Origin};
use ndaal_i18n::Translator;

use crate::health::HealthError;

/// Exit code of usage and configuration errors, as `clap` uses.
pub(crate) const USAGE_ERROR: u8 = 2;

/// A failed command.
#[derive(Debug)]
pub(crate) enum CliError {
    /// The configuration file cannot be read.
    Read { path: PathBuf, error: io::Error },
    /// The configuration file is larger than `max` bytes.
    TooLarge { path: PathBuf, max: u64 },
    /// The configuration file is not UTF-8 text.
    NotUtf8 { path: PathBuf },
    /// A `--set` value without `=`.
    Set(String),
    /// An invalid configuration.
    Config(ConfigError),
    /// A command that later tasks will build.
    NotAvailable(&'static str),
    /// The data directory cannot be created.
    DataDir { path: PathBuf, error: io::Error },
    /// The HTTP address cannot be bound.
    Listen { addr: SocketAddr, error: io::Error },
    /// The node at `addr` is not ready (`ndaal health`).
    NotReady {
        addr: SocketAddr,
        error: HealthError,
    },
    /// Anything else: logging, signals, output.
    Runtime(String),
}

impl CliError {
    pub(crate) fn exit_code(&self) -> ExitCode {
        match self {
            Self::Runtime(_)
            | Self::DataDir { .. }
            | Self::Listen { .. }
            | Self::NotReady { .. } => ExitCode::FAILURE,
            _ => ExitCode::from(USAGE_ERROR),
        }
    }

    pub(crate) fn message(&self, t: &Translator) -> String {
        match self {
            Self::Read { path, error } => t.t_args(
                "cli-config-read-error",
                &[
                    ("path", &path.display().to_string()),
                    ("error", &error.to_string()),
                ],
            ),
            Self::TooLarge { path, max } => t.t_args(
                "cli-config-too-large",
                &[
                    ("path", &path.display().to_string()),
                    ("max", &max.to_string()),
                ],
            ),
            Self::NotUtf8 { path } => t.t_args(
                "cli-config-not-utf8",
                &[("path", &path.display().to_string())],
            ),
            Self::Set(value) => t.t_args("cli-set-syntax", &[("value", value)]),
            Self::Config(error) => config_message(error, t),
            Self::NotAvailable(command) => t.t_args("cli-not-available", &[("command", command)]),
            Self::DataDir { path, error } => t.t_args(
                "cli-data-dir-error",
                &[
                    ("path", &path.display().to_string()),
                    ("error", &error.to_string()),
                ],
            ),
            Self::Listen { addr, error } => t.t_args(
                "cli-listen-error",
                &[("addr", &addr.to_string()), ("error", &error.to_string())],
            ),
            Self::NotReady { addr, error } => t.t_args(
                "cli-health-not-ready",
                &[
                    ("addr", &addr.to_string()),
                    ("reason", &health_reason(error, t)),
                ],
            ),
            Self::Runtime(error) => t.t_args("cli-runtime-error", &[("error", error)]),
        }
    }
}

fn health_reason(error: &HealthError, t: &Translator) -> String {
    match error {
        HealthError::Unreachable(error) => {
            t.t_args("cli-health-unreachable", &[("error", &error.to_string())])
        }
        HealthError::Timeout => t.t("cli-health-timeout"),
        HealthError::Status(code) => t.t_args("cli-health-status", &[("status", code)]),
        HealthError::Invalid => t.t("cli-health-invalid"),
    }
}

fn config_message(error: &ConfigError, t: &Translator) -> String {
    match error {
        // The parser's detail (line, column, excerpt) exists in English only.
        ConfigError::Syntax { message } => t.t_args("cli-config-syntax", &[("error", message)]),
        ConfigError::PublicWithoutAuth { key, origin } => t.t_args(
            "cli-config-public-without-auth",
            &[("key", key), ("origin", &origin_name(*origin, t))],
        ),
        ConfigError::Unknown { key, origin } => t.t_args(
            "cli-config-unknown",
            &[("key", key), ("origin", &origin_name(*origin, t))],
        ),
        ConfigError::Invalid {
            key,
            origin,
            expected,
        } => t.t_args(
            "cli-config-invalid",
            &[
                ("key", key),
                ("origin", &origin_name(*origin, t)),
                ("expected", &expected_text(*expected, t)),
            ],
        ),
    }
}

fn origin_name(origin: Origin, t: &Translator) -> String {
    t.t(match origin {
        Origin::Default => "cli-origin-default",
        Origin::File => "cli-origin-file",
        Origin::Env => "cli-origin-env",
        Origin::CommandLine => "cli-origin-command-line",
    })
}

fn expected_text(expected: Expected, t: &Translator) -> String {
    match expected {
        Expected::Value(Kind::Path) => t.t("cli-expected-path"),
        Expected::Value(Kind::Text) => t.t("cli-expected-text"),
        Expected::Value(Kind::Choice(words)) => {
            t.t_args("cli-expected-choice", &[("choices", &words.join(", "))])
        }
        Expected::Value(Kind::Addr) => t.t("cli-expected-addr"),
        Expected::Value(Kind::Bool) => t.t("cli-expected-bool"),
        Expected::Value(Kind::Duration) => t.t("cli-expected-duration"),
        Expected::Value(Kind::Count) => t.t("cli-expected-count"),
        Expected::Value(Kind::LogLevel) => t.t_args(
            "cli-expected-log-level",
            &[("levels", &LOG_LEVELS.join(", "))],
        ),
        Expected::Table => t.t("cli-expected-table"),
        Expected::Scalar => t.t("cli-expected-scalar"),
    }
}
