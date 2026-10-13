//! Panic policy (ADR-030): a panic is a bug. The hook logs it with its
//! location, its stack trace and the current request (the `request` span),
//! counts it, flushes the output, and the process then aborts
//! (`panic = "abort"`); the node restarts from its log.

use std::backtrace::Backtrace;
use std::io::Write;
use std::panic::PanicHookInfo;

use crate::metrics::Registry;

/// Name of the panic counter.
pub const PANICS_METRIC: &str = "ndaal_panics_total";

/// Replaces the panic hook of the process.
pub fn install(registry: &Registry) {
    let panics = registry
        .counter(PANICS_METRIC, "Panics; each one stops the process.", &[])
        .expect("constant metric name is valid and registered as a counter");
    std::panic::set_hook(Box::new(move |info| {
        panics.increment(1);
        let backtrace = Backtrace::force_capture();
        let (payload, location) = (payload(info), location(info));
        // The payload is the event message itself: a separate `message` field
        // would collide with it and produce a duplicate JSON key.
        tracing::error!(location = %location, backtrace = %backtrace, "panic: {payload}");
        // Also written raw: the log event is lost if the level filters it out,
        // if logging is not set up yet, or if the panic happened inside logging.
        let _ = writeln!(
            std::io::stderr(),
            "panic: {payload} at {location}\n{backtrace}"
        );
        // The process aborts right after the hook: nothing may stay buffered.
        let _ = std::io::stdout().flush();
        let _ = std::io::stderr().flush();
    }));
}

fn payload(info: &PanicHookInfo<'_>) -> String {
    let payload = info.payload();
    payload
        .downcast_ref::<&str>()
        .map(|s| (*s).to_owned())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "non-string panic payload".to_owned())
}

fn location(info: &PanicHookInfo<'_>) -> String {
    info.location()
        .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
        .unwrap_or_else(|| "unknown".to_owned())
}
