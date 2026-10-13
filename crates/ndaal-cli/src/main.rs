//! The ndaal binary: server, client, administration, backup, benchmarks.

#![forbid(unsafe_code)]

mod cli;
mod commands;
mod error;
mod health;

use std::ffi::OsString;
use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::error::ErrorKind;
use ndaal_common::config::LangChoice;
use ndaal_i18n::{EN, Translator};

use crate::error::CliError;

/// Environment variable naming the configuration file, below `--config`.
const CONFIG_ENV: &str = "NDAAL_CONFIG";

fn main() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().collect();
    // Variables that are not valid Unicode cannot be configuration values.
    let env: Vec<(String, String)> = std::env::vars_os()
        .filter_map(|(k, v)| Some((k.into_string().ok()?, v.into_string().ok()?)))
        .collect();
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(e) => return report(&Translator::new(EN), &CliError::Runtime(e.to_string())),
    };
    runtime.block_on(run(args, env))
}

async fn run(args: Vec<OsString>, env: Vec<(String, String)>) -> ExitCode {
    let globals = cli::early(&args);
    // `--config`, else `NDAAL_CONFIG`: a container sets the file once for the
    // server and its health probe.
    let config_path = globals.config.clone().or_else(|| {
        env.iter()
            .find(|(name, _)| name == CONFIG_ENV)
            .map(|(_, path)| PathBuf::from(path))
    });
    let file = match &config_path {
        Some(path) => Some(commands::read_config(path).await),
        None => None,
    };
    let config = match file {
        Some(Err(e)) => Err(e),
        Some(Ok(text)) => commands::load_config(Some(&text), &env, &globals),
        None => commands::load_config(None, &env, &globals),
    };
    // A configuration error is reported by the command that needs it; the
    // language then comes from the other sources.
    let t = Translator::new(language(&globals, &env, config.as_ref().ok()));

    let matches = match cli::command(&t).try_get_matches_from(&args) {
        Ok(matches) => matches,
        Err(e) => {
            // Nothing is left to report a failed write of the help or of the
            // error to.
            return match e.kind() {
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => {
                    let _ = e.print();
                    ExitCode::SUCCESS
                }
                ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand => {
                    let _ = e.print();
                    ExitCode::from(2)
                }
                _ => {
                    let _ = write!(std::io::stderr(), "{}", cli::usage_error(&e, &t));
                    ExitCode::from(2)
                }
            };
        }
    };
    match commands::dispatch(&matches, config).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => report(&t, &e),
    }
}

/// ENF-I18N-03: `--lang` > `NDAAL_LANG` > `node.lang` > system > English.
fn language(
    globals: &cli::Globals,
    env: &[(String, String)],
    config: Option<&ndaal_common::config::Config>,
) -> &'static ndaal_i18n::Locale {
    let ndaal_lang = env
        .iter()
        .find(|(k, _)| k == "NDAAL_LANG")
        .map(|(_, v)| v.as_str());
    let node_lang = config.and_then(|c| match c.node.lang {
        LangChoice::Auto => None,
        LangChoice::En => Some("en"),
        LangChoice::Fr => Some("fr"),
    });
    let system = sys_locale::get_locale();
    ndaal_i18n::choose(
        globals.lang.as_deref(),
        ndaal_lang,
        node_lang,
        system.as_deref(),
    )
}

fn report(t: &Translator, error: &CliError) -> ExitCode {
    // Nothing is left to report a failed write to the error output to.
    let _ = writeln!(std::io::stderr(), "{}", error.message(t));
    error.exit_code()
}
