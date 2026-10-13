//! What each command does, and how its errors reach the user.

use std::io::{self, Write};
use std::path::Path;

use clap::ArgMatches;
use ndaal_common::config::{self, Config, LogFormatChoice, Sources};
use ndaal_io::real::RealDisk;
use ndaal_io::{Disk, File, OpenOptions};
use ndaal_observe::logs::{self, LogFormat};
use ndaal_observe::{metrics::Registry, panic};

use crate::cli::Globals;
use crate::error::CliError;

/// Largest configuration file read (1 MiB); real ones are a few KiB, and a
/// bound keeps a wrong path (a disk image) from filling the memory.
const MAX_CONFIG_SIZE: u64 = 1 << 20;

/// Size of each read of the configuration file (64 KiB).
const READ_CHUNK: usize = 64 * 1024;

/// Contents of the configuration file, read through `ndaal-io` (ADR-017),
/// read-only: the file may be on a read-only mount. Read until the end
/// rather than by its length: a pipe has none, and must fail rather than
/// look like an empty file.
pub(crate) async fn read_config(path: &Path) -> Result<String, CliError> {
    let read_error = |error| CliError::Read {
        path: path.to_owned(),
        error,
    };
    let options = OpenOptions {
        read_only: true,
        ..OpenOptions::default()
    };
    let file = RealDisk.open(path, options).await.map_err(read_error)?;
    let mut bytes = Vec::new();
    let mut chunk = vec![0; READ_CHUNK];
    loop {
        let offset = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        let n = file.read_at(&mut chunk, offset).await.map_err(read_error)?;
        if n == 0 {
            break;
        }
        bytes.extend_from_slice(&chunk[..n]);
        if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_CONFIG_SIZE {
            return Err(CliError::TooLarge {
                path: path.to_owned(),
                max: MAX_CONFIG_SIZE,
            });
        }
    }
    String::from_utf8(bytes).map_err(|_| CliError::NotUtf8 {
        path: path.to_owned(),
    })
}

/// The configuration from every source (EF-GEN-05); `file` is the contents
/// of `--config`, already read.
pub(crate) fn load_config(
    file: Option<&str>,
    env: &[(String, String)],
    globals: &Globals,
) -> Result<Config, CliError> {
    let mut overrides = Vec::new();
    for set in &globals.sets {
        let (key, value) = set
            .split_once('=')
            .ok_or_else(|| CliError::Set(set.clone()))?;
        overrides.push((key.to_owned(), value.to_owned()));
    }
    // The dedicated option comes last: it wins over `--set node.data_dir`.
    if let Some(dir) = &globals.data_dir {
        overrides.push(("node.data_dir".to_owned(), dir.clone()));
    }
    let sources = Sources {
        file,
        env: env.to_vec(),
        overrides,
    };
    config::load(&sources).map_err(CliError::Config)
}

/// Writes `text` to the standard output.
pub(crate) fn print(text: &str) -> Result<(), CliError> {
    io::stdout()
        .lock()
        .write_all(text.as_bytes())
        .map_err(|e| CliError::Runtime(e.to_string()))
}

/// Runs the parsed command. `config` is the configuration loaded from the
/// global options, used only by the commands that need it.
pub(crate) async fn dispatch(
    matches: &ArgMatches,
    config: Result<Config, CliError>,
) -> Result<(), CliError> {
    match matches.subcommand() {
        Some(("server", _)) => server(&config?).await,
        Some(("config", sub)) => match sub.subcommand_name() {
            Some("show") => print(&config::render(&config?)),
            _ => print(&config::reference()),
        },
        Some(("admin", _)) => Err(CliError::NotAvailable("admin")),
        Some(("backup", _)) => Err(CliError::NotAvailable("backup")),
        Some(("restore", _)) => Err(CliError::NotAvailable("restore")),
        Some(("bench", _)) => Err(CliError::NotAvailable("bench")),
        _ => Err(CliError::NotAvailable("doctor")),
    }
}

/// Starts a node: logs, panic policy, then waits for Ctrl-C.
async fn server(config: &Config) -> Result<(), CliError> {
    let format = match config.log.format {
        LogFormatChoice::Json => LogFormat::Json,
        LogFormatChoice::Text => LogFormat::Text,
    };
    let _logs =
        logs::init(format, &config.log.level).map_err(|e| CliError::Runtime(e.to_string()))?;
    let registry = Registry::default();
    panic::install(&registry);
    tracing::info!(
        data_dir = %config.node.data_dir.display(),
        listen = %config.http.listen,
        "node started"
    );
    // TODO(T01.22): serve /health/live, /health/ready and /metrics (P-27).
    tokio::signal::ctrl_c()
        .await
        .map_err(|e| CliError::Runtime(e.to_string()))?;
    tracing::info!("node stopped");
    Ok(())
}
