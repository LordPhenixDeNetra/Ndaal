//! The TOML file: reading its tables into keys, writing a configuration back.

use std::fmt::Write;

use super::{Config, ConfigError, Expected, Origin, PARAMS, entries, invalid, params};

/// `(key, value)` pairs of the file, keys with every level joined by dots
/// (`[runtime.cores] shard = 4` is `runtime.cores.shard`). Scalars (`42`,
/// `true`, `0.5`) are read as their text, then checked by the parameter's
/// kind like the environment and the command line, which only carry text.
pub(super) fn file_entries(text: &str) -> Result<Vec<(String, String)>, ConfigError> {
    let table: toml::Table = text
        .parse()
        .map_err(|e: toml::de::Error| ConfigError::Syntax {
            message: e.to_string(),
        })?;
    let mut out = Vec::new();
    collect(&table, "", &mut out)?;
    Ok(out)
}

fn collect(
    table: &toml::Table,
    prefix: &str,
    out: &mut Vec<(String, String)>,
) -> Result<(), ConfigError> {
    for (name, value) in table {
        let key = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}.{name}")
        };
        if params::find(&key).is_some() {
            let text = match value {
                toml::Value::String(text) => text.clone(),
                toml::Value::Integer(n) => n.to_string(),
                toml::Value::Float(x) => x.to_string(),
                toml::Value::Boolean(b) => b.to_string(),
                toml::Value::Datetime(d) => d.to_string(),
                toml::Value::Array(_) | toml::Value::Table(_) => {
                    return Err(invalid(&key, Origin::File, Expected::Scalar));
                }
            };
            out.push((key, text));
            continue;
        }
        let section = format!("{key}.");
        if !PARAMS.iter().any(|p| p.key.starts_with(&section)) {
            return Err(ConfigError::Unknown {
                key,
                origin: Origin::File,
            });
        }
        match value {
            toml::Value::Table(inner) => collect(inner, &key, out)?,
            _ => return Err(invalid(&key, Origin::File, Expected::Table)),
        }
    }
    Ok(())
}

/// `config` as a TOML file (`ndaal config show`); [`super::load`] reads it
/// back to the same configuration.
pub fn render(config: &Config) -> String {
    let mut out = String::new();
    let mut current = "";
    for (key, value) in entries(config) {
        let (section, name) = key.rsplit_once('.').unwrap_or(("", key));
        if section != current {
            if !current.is_empty() {
                out.push('\n');
            }
            // Writing to a String cannot fail.
            let _ = writeln!(out, "[{section}]");
            current = section;
        }
        let _ = writeln!(out, "{name} = {}", toml::Value::String(value));
    }
    out
}
