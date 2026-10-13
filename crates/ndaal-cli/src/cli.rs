//! The command line interface, with every text translated (ADR-029).
//!
//! The language must be known before the interface is built, since the help
//! itself is translated: [`early`] reads the global options first.

use std::ffi::OsString;
use std::path::PathBuf;

use clap::{Arg, ArgAction, Command};
use ndaal_i18n::Translator;

/// Global options, read before `clap` parses the command line.
#[derive(Debug, Default)]
pub(crate) struct Globals {
    /// `--lang`.
    pub(crate) lang: Option<String>,
    /// `--config`.
    pub(crate) config: Option<PathBuf>,
    /// `--set` values, in order.
    pub(crate) sets: Vec<String>,
    /// `--data-dir`.
    pub(crate) data_dir: Option<String>,
}

/// Reads the global options as `clap` does (`--opt value` or `--opt=value`,
/// anywhere before `--`). `clap` still parses and validates the whole line:
/// a malformed one is reported by it, whatever this function read.
pub(crate) fn early(args: &[OsString]) -> Globals {
    let mut globals = Globals::default();
    let mut words = args
        .iter()
        .skip(1)
        .map(|a| a.to_string_lossy().into_owned());
    while let Some(word) = words.next() {
        if word == "--" {
            break;
        }
        let (name, inline) = match word.split_once('=') {
            Some((name, value)) => (name.to_owned(), Some(value.to_owned())),
            None => (word, None),
        };
        if !["--lang", "--config", "--set", "--data-dir"].contains(&name.as_str()) {
            continue;
        }
        let Some(value) = inline.or_else(|| words.next()) else {
            break;
        };
        match name.as_str() {
            "--lang" => globals.lang = Some(value),
            "--config" => globals.config = Some(PathBuf::from(value)),
            "--set" => globals.sets.push(value),
            _ => globals.data_dir = Some(value),
        }
    }
    globals
}

/// Help layout with the translated "Usage" heading; `clap` writes its own
/// heading in English otherwise.
fn template(t: &Translator) -> String {
    format!(
        "{{about-with-newline}}\n{}: {{usage}}\n\n{{all-args}}{{after-help}}",
        t.t("cli-help-usage")
    )
}

/// A subcommand with the translated layout.
fn sub(t: &Translator, name: &'static str, about_key: &str) -> Command {
    Command::new(name)
        .about(t.t(about_key))
        .help_template(template(t))
        .subcommand_help_heading(t.t("cli-help-commands"))
}

fn global(t: &Translator, id: &'static str, help_key: &str, value_key: &str) -> Arg {
    Arg::new(id)
        .long(id)
        .global(true)
        .value_name(t.t(value_key))
        .help(t.t(help_key))
        .help_heading(t.t("cli-help-options"))
}

/// The `ndaal` command, in the language of `t`.
pub(crate) fn command(t: &Translator) -> Command {
    let options = t.t("cli-help-options");
    let config = sub(t, "config", "cli-cmd-config")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(sub(t, "reference", "cli-cmd-config-reference"))
        .subcommand(sub(t, "show", "cli-cmd-config-show"));
    Command::new("ndaal")
        .bin_name("ndaal")
        .version(env!("CARGO_PKG_VERSION"))
        .about(t.t("cli-about"))
        .help_template(template(t))
        .subcommand_help_heading(t.t("cli-help-commands"))
        .disable_help_flag(true)
        .disable_version_flag(true)
        .disable_help_subcommand(true)
        .subcommand_required(true)
        .arg_required_else_help(true)
        .arg(global(t, "config", "cli-opt-config", "cli-value-file"))
        .arg(global(t, "lang", "cli-opt-lang", "cli-value-lang"))
        .arg(global(t, "set", "cli-opt-set", "cli-value-setting").action(ArgAction::Append))
        .arg(global(t, "data-dir", "cli-opt-data-dir", "cli-value-dir"))
        .arg(
            Arg::new("help")
                .short('h')
                .long("help")
                .global(true)
                .action(ArgAction::Help)
                .help(t.t("cli-opt-help"))
                .help_heading(options.clone()),
        )
        .arg(
            Arg::new("version")
                .short('V')
                .long("version")
                .action(ArgAction::Version)
                .help(t.t("cli-opt-version"))
                .help_heading(options),
        )
        .subcommand(sub(t, "server", "cli-cmd-server"))
        .subcommand(sub(t, "admin", "cli-cmd-admin"))
        .subcommand(sub(t, "backup", "cli-cmd-backup"))
        .subcommand(sub(t, "restore", "cli-cmd-restore"))
        .subcommand(sub(t, "bench", "cli-cmd-bench"))
        .subcommand(config)
        .subcommand(sub(t, "doctor", "cli-cmd-doctor"))
        .subcommand(sub(t, "health", "cli-cmd-health"))
}

/// A `clap` parse error in the language of `t`: `clap` writes its own
/// messages in English only.
pub(crate) fn usage_error(error: &clap::Error, t: &Translator) -> String {
    use clap::error::{ContextKind, ErrorKind};
    let context = |kind| error.get(kind).map(ToString::to_string).unwrap_or_default();
    let line = match error.kind() {
        ErrorKind::InvalidSubcommand => t.t_args(
            "cli-error-unknown-command",
            &[("name", &context(ContextKind::InvalidSubcommand))],
        ),
        ErrorKind::UnknownArgument => t.t_args(
            "cli-error-unknown-option",
            &[("name", &context(ContextKind::InvalidArg))],
        ),
        ErrorKind::InvalidValue
        | ErrorKind::NoEquals
        | ErrorKind::ValueValidation
        | ErrorKind::TooManyValues
        | ErrorKind::TooFewValues
        | ErrorKind::WrongNumberOfValues => t.t_args(
            "cli-error-invalid-value",
            &[("name", &context(ContextKind::InvalidArg))],
        ),
        _ => t.t("cli-error-usage"),
    };
    format!("{line}\n{}\n", t.t("cli-error-hint"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(words: &[&str]) -> Vec<OsString> {
        std::iter::once("ndaal")
            .chain(words.iter().copied())
            .map(OsString::from)
            .collect()
    }

    #[test]
    fn early_reads_both_spellings_anywhere_before_double_dash() {
        let g = early(&args(&[
            "config",
            "--lang=fr",
            "show",
            "--set",
            "a.b=1",
            "--config",
            "c.toml",
            "--set=d.e=2",
            "--data-dir",
            "/d",
            "--",
            "--lang",
            "en",
        ]));
        assert_eq!(g.lang.as_deref(), Some("fr"));
        assert_eq!(g.config, Some(PathBuf::from("c.toml")));
        assert_eq!(g.sets, ["a.b=1", "d.e=2"]);
        assert_eq!(g.data_dir.as_deref(), Some("/d"));
    }

    #[test]
    fn command_is_well_formed() {
        command(&Translator::new(ndaal_i18n::EN)).debug_assert();
    }
}
