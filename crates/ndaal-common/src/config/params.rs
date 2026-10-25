//! The table of configuration parameters (EXPLOITATION.md section 3).
//!
//! Every parameter the node reads is declared here once: its type, default
//! and help text drive validation, the environment variable name and
//! `ndaal config reference`. A task that needs a new parameter adds a row.

use std::fmt::Write;
use std::net::SocketAddr;

/// Log levels accepted alone or as `target=level` (EF-OPS-03).
pub const LOG_LEVELS: [&str; 6] = ["trace", "debug", "info", "warn", "error", "off"];

/// The type of a parameter value; values are TOML strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A non-empty file system path.
    Path,
    /// Any text, possibly empty.
    Text,
    /// One of a fixed list of words.
    Choice(&'static [&'static str]),
    /// An IP address and a port: `127.0.0.1:8080`, `[::1]:8080`.
    Addr,
    /// A level (`info`) or a list of `target=level` directives.
    LogLevel,
    /// `true` or `false`.
    Bool,
    /// `0`, or an integer followed by `us`, `ms` or `s` (at most 10 s).
    Duration,
    /// `auto` or a number of cores from 0 to 4096.
    Count,
}

impl Kind {
    /// The name shown by `ndaal config reference`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Path => "path",
            Self::Text => "string",
            Self::Choice(_) => "choice",
            Self::Addr => "address",
            Self::LogLevel => "log-level",
            Self::Bool => "bool",
            Self::Duration => "duration",
            Self::Count => "count",
        }
    }

    /// Whether `value` is valid for this kind.
    pub(super) fn check(self, value: &str) -> bool {
        match self {
            Self::Path => !value.is_empty(),
            Self::Text => true,
            Self::Choice(words) => words.contains(&value),
            Self::Addr => value.parse::<SocketAddr>().is_ok(),
            Self::LogLevel => is_log_directive(value),
            Self::Bool => value == "true" || value == "false",
            Self::Duration => super::runtime::parse_duration(value).is_some(),
            Self::Count => super::runtime::parse_count(value).is_some(),
        }
    }

    /// What a valid value looks like, in English (logs, `Display`).
    pub(super) fn expected(self) -> String {
        match self {
            Self::Path => "a non-empty path".into(),
            Self::Text => "a string".into(),
            Self::Choice(words) => format!("one of {}", words.join(", ")),
            Self::Addr => "an address such as 127.0.0.1:8080".into(),
            Self::LogLevel => format!("a level ({}) or target=level list", LOG_LEVELS.join(", ")),
            Self::Bool => "true or false".into(),
            Self::Duration => "a duration such as 0, 250us, 10ms or 2s (at most 10s)".into(),
            Self::Count => format!("auto or a number from 0 to {}", super::runtime::MAX_COUNT),
        }
    }
}

/// One configuration parameter.
#[derive(Debug, Clone, Copy)]
pub struct Param {
    /// `section.key`, as written in the TOML file.
    pub key: &'static str,
    /// Type of the value.
    pub kind: Kind,
    /// Value used when no source sets it.
    pub default: &'static str,
    /// One-line description, in English (ENF-I18N-02: names and reference
    /// output are not translated).
    pub help: &'static str,
}

/// Language choices of `node.lang` (ENF-I18N-03).
pub(super) const LANGS: &[&str] = &["auto", "en", "fr"];
/// Log formats of `log.format`.
pub(super) const LOG_FORMATS: &[&str] = &["json", "text"];

/// Every parameter implemented so far, in reference order.
pub const PARAMS: &[Param] = &[
    Param {
        key: "node.data_dir",
        kind: Kind::Path,
        default: "/var/lib/ndaal",
        help: "directory of the data files",
    },
    Param {
        key: "node.name",
        kind: Kind::Text,
        default: "",
        help: "node name; empty means the host name",
    },
    Param {
        key: "node.lang",
        kind: Kind::Choice(LANGS),
        default: "auto",
        help: "language of the CLI messages; auto follows the system",
    },
    Param {
        key: "http.listen",
        kind: Kind::Addr,
        default: "127.0.0.1:8080",
        help: "address of the HTTP API, health and metrics endpoints",
    },
    Param {
        key: "auth.allow_insecure",
        kind: Kind::Bool,
        default: "false",
        help: "allow a non-loopback listen address without authentication (ENF-SEC-04)",
    },
    Param {
        key: "log.level",
        kind: Kind::LogLevel,
        default: "info",
        help: "log level, changeable without restart",
    },
    Param {
        key: "log.format",
        kind: Kind::Choice(LOG_FORMATS),
        default: "json",
        help: "log format",
    },
    Param {
        key: "runtime.idle_spin",
        kind: Kind::Duration,
        default: "50us",
        help: "how long an idle core polls its queues before sleeping",
    },
    Param {
        key: "runtime.cores.total",
        kind: Kind::Count,
        default: "auto",
        help: "cores used by the node (auto: the cores allowed to the process)",
    },
    Param {
        key: "runtime.cores.shard",
        kind: Kind::Count,
        default: "auto",
        help: "cores of class shard (partitions, key-value, transactions)",
    },
    Param {
        key: "runtime.cores.vector",
        kind: Kind::Count,
        default: "auto",
        help: "cores of class vector (search, index building)",
    },
    Param {
        key: "runtime.cores.transfer",
        kind: Kind::Count,
        default: "auto",
        help: "cores of class transfer (object streams)",
    },
    Param {
        key: "runtime.cores.background",
        kind: Kind::Count,
        default: "auto",
        help: "cores of class background (compaction, maintenance)",
    },
    Param {
        key: "runtime.cores.io",
        kind: Kind::Count,
        default: "auto",
        help: "cores of class io (disk operations without io_uring)",
    },
    Param {
        key: "runtime.cores.pin",
        kind: Kind::Bool,
        default: "true",
        help: "pin each core thread to its CPU (ignored on macOS)",
    },
];

/// The parameter named `key`.
pub(super) fn find(key: &str) -> Option<&'static Param> {
    PARAMS.iter().find(|p| p.key == key)
}

/// Environment variable of a parameter: `node.data_dir` is
/// `NDAAL_NODE__DATA_DIR`, `runtime.cores.pin` is
/// `NDAAL_RUNTIME__CORES__PIN` (EF-GEN-05).
pub fn env_name(key: &str) -> String {
    format!("NDAAL_{}", key.to_ascii_uppercase().replace('.', "__"))
}

/// The parameter key of an environment variable, or `None` when the variable
/// is not a configuration variable (`NDAAL_LANG`, `NDAAL_HOME`, `PATH`).
pub(super) fn env_key(name: &str) -> Option<String> {
    let rest = name.strip_prefix("NDAAL_")?;
    rest.contains("__")
        .then(|| rest.to_ascii_lowercase().replace("__", "."))
}

/// A log level or a comma-separated list of levels and `target=level`
/// directives. Strict: a mistyped level such as `verbose` is refused, where
/// `tracing`'s own parser would read it as a target name.
pub fn is_log_directive(directive: &str) -> bool {
    let is_level = |s: &str| LOG_LEVELS.contains(&s.to_ascii_lowercase().as_str());
    directive.split(',').all(|part| match part.split_once('=') {
        Some((target, level)) => !target.is_empty() && is_level(level),
        None => is_level(part),
    })
}

/// `ndaal config reference`: one line per parameter (key, type, default,
/// environment variable, help). English and stable: scripts may read it.
pub fn reference() -> String {
    let mut out = String::new();
    for p in PARAMS {
        let kind = match p.kind {
            Kind::Choice(words) => format!("{} ({})", p.kind.name(), words.join("|")),
            other => other.name().to_owned(),
        };
        // Writing to a String cannot fail.
        let _ = writeln!(
            out,
            "{}  {kind}  default={:?}  env={}  {}",
            p.key,
            p.default,
            env_name(p.key),
            p.help
        );
    }
    out
}
