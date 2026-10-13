//! Structured logs (EF-OPS-03): text or JSON, level changed without restart.

use std::sync::{Mutex, PoisonError};

use tracing::Subscriber;
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::{EnvFilter, Layer, Registry, fmt, reload};

/// Output format of the logs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFormat {
    /// One readable line per event.
    Text,
    /// One JSON object per line, with the fields of the current span.
    Json,
}

/// Errors of the log setup.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LogError {
    /// Not a level (`trace` to `error`, `off`) or a `target=level` list.
    #[error("invalid log level {0:?}")]
    InvalidLevel(String),
    /// A global subscriber is already installed.
    #[error("logging is already initialized")]
    AlreadyInitialized,
    /// The filter could not be swapped (the subscriber was dropped).
    #[error("cannot change the log level: {0}")]
    Reload(String),
}

/// Changes the level of a running subscriber.
pub struct LogHandle {
    reload: reload::Handle<EnvFilter, Registry>,
    current: Mutex<String>,
}

impl LogHandle {
    /// Sets the level: `"debug"`, or per target `"ndaal_wal=debug,info"`.
    pub fn set_level(&self, directive: &str) -> Result<(), LogError> {
        let filter = parse(directive)?;
        self.reload
            .reload(filter)
            .map_err(|e| LogError::Reload(e.to_string()))?;
        *self.current.lock().unwrap_or_else(PoisonError::into_inner) = directive.to_owned();
        Ok(())
    }

    /// The directive in force.
    pub fn level(&self) -> String {
        self.current
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

/// Validates a directive strictly, then builds the filter. `EnvFilter` alone
/// would read a mistyped level such as `verbose` as a target name and accept it.
fn parse(directive: &str) -> Result<EnvFilter, LogError> {
    let invalid = || LogError::InvalidLevel(directive.to_owned());
    if !ndaal_common::config::is_log_directive(directive) {
        return Err(invalid());
    }
    EnvFilter::try_new(directive.to_ascii_lowercase()).map_err(|_| invalid())
}

/// A subscriber writing to `writer`, and the handle that changes its level.
/// Tests install it with `tracing::subscriber::with_default`; servers use [`init`].
pub fn subscriber<W>(
    format: LogFormat,
    level: &str,
    writer: W,
) -> Result<(impl Subscriber + Send + Sync, LogHandle), LogError>
where
    W: for<'a> MakeWriter<'a> + Send + Sync + 'static,
{
    let (filter, reload) = reload::Layer::new(parse(level)?);
    // No terminal colors: logs go to files and collectors, not only terminals.
    let output = match format {
        LogFormat::Text => fmt::layer().with_ansi(false).with_writer(writer).boxed(),
        LogFormat::Json => fmt::layer()
            .json()
            .with_current_span(true)
            // The whole span stack: the request id must survive child spans.
            .with_span_list(true)
            .with_writer(writer)
            .boxed(),
    };
    let subscriber = Registry::default().with(filter).with(output);
    let handle = LogHandle {
        reload,
        current: Mutex::new(level.to_owned()),
    };
    Ok((subscriber, handle))
}

/// Installs the process-wide subscriber, writing to the standard output.
pub fn init(format: LogFormat, level: &str) -> Result<LogHandle, LogError> {
    let (subscriber, handle) = subscriber(format, level, std::io::stdout)?;
    tracing::subscriber::set_global_default(subscriber)
        .map_err(|_| LogError::AlreadyInitialized)?;
    Ok(handle)
}
