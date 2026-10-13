//! Structured logs with a level changed at runtime (T01.10).

use std::io::Write;
use std::sync::{Arc, Mutex, PoisonError};

use ndaal_observe::logs::{LogError, LogFormat, subscriber};
use tracing::{debug, info};

/// In-memory log output shared with the subscriber.
#[derive(Clone, Default)]
struct Buffer(Arc<Mutex<Vec<u8>>>);

impl Buffer {
    fn text(&self) -> String {
        let bytes = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        String::from_utf8_lossy(&bytes).into_owned()
    }
}

impl Write for Buffer {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .extend_from_slice(data);
        Ok(data.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn setup(
    format: LogFormat,
) -> (
    Buffer,
    impl tracing::Subscriber + Send + Sync,
    ndaal_observe::logs::LogHandle,
) {
    let buffer = Buffer::default();
    let writer = buffer.clone();
    let (sub, handle) = subscriber(format, "info", move || writer.clone()).expect("subscriber");
    (buffer, sub, handle)
}

#[test]
fn level_changes_without_restart() {
    let (buffer, sub, handle) = setup(LogFormat::Text);
    tracing::subscriber::with_default(sub, || {
        debug!("hidden at info");
        info!("shown at info");
        handle.set_level("debug").expect("set level");
        debug!("shown after the change");
    });
    let out = buffer.text();
    assert!(!out.contains("hidden at info"), "{out}");
    assert!(out.contains("shown at info"), "{out}");
    assert!(out.contains("shown after the change"), "{out}");
}

#[test]
fn invalid_levels_are_rejected_and_the_level_kept() {
    let (_buffer, _sub, handle) = setup(LogFormat::Text);
    for bad in ["verbose", "info,", "ndaal_wal=loud", "=debug", ""] {
        assert!(
            matches!(handle.set_level(bad), Err(LogError::InvalidLevel(_))),
            "{bad:?} accepted"
        );
    }
    assert_eq!(handle.level(), "info");
    handle
        .set_level("ndaal_wal=debug,info")
        .expect("per-target directive");
    handle.set_level("WARN").expect("case-insensitive level");
}

#[test]
fn json_lines_are_objects_with_level_and_fields() {
    let (buffer, sub, _handle) = setup(LogFormat::Json);
    tracing::subscriber::with_default(sub, || {
        info!(answer = 42, "structured");
    });
    let out = buffer.text();
    let line = out.lines().next().expect("one line");
    assert!(line.starts_with('{') && line.ends_with('}'), "{line}");
    for needle in [
        "\"level\":\"INFO\"",
        "\"fields\"",
        "\"answer\":42",
        "structured",
    ] {
        assert!(line.contains(needle), "missing {needle} in {line}");
    }
}

#[test]
fn text_lines_are_readable() {
    let (buffer, sub, _handle) = setup(LogFormat::Text);
    tracing::subscriber::with_default(sub, || info!("plain message"));
    let out = buffer.text();
    assert!(
        out.contains("INFO") && out.contains("plain message"),
        "{out}"
    );
    assert!(
        !out.contains('\u{1b}'),
        "no terminal colors in logs: {out:?}"
    );
}

// Review of lot 5 (I3): the request id must survive child spans in JSON.
#[test]
fn json_keeps_the_request_id_inside_child_spans() {
    let (buffer, sub, _handle) = setup(LogFormat::Json);
    tracing::subscriber::with_default(sub, || {
        let request = tracing::info_span!("request", request_id = "abc123");
        let _r = request.enter();
        let child = tracing::info_span!("wal_append", lsn = 5);
        let _c = child.enter();
        info!("deep event");
    });
    let out = buffer.text();
    assert!(
        out.contains("abc123"),
        "request id lost in a child span: {out}"
    );
}
