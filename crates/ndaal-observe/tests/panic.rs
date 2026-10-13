//! A panic stops the process after logging it (ADR-030, T01.24 done criterion).
//! `panic-probe` is a real binary built with `panic = "abort"`; tests
//! themselves unwind, so the abort can only be observed in a child process.

use std::process::{Command, Output};

const PROBE: &str = env!("CARGO_BIN_EXE_panic-probe");
const REQUEST_ID: &str = "0123456789abcdef0123456789abcdef";

/// The process ended by `abort`, not by a normal exit (even a failing one).
fn aborted(output: &Output) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        output.status.signal() == Some(6) // SIGABRT
    }
    #[cfg(windows)]
    {
        // STATUS_STACK_BUFFER_OVERRUN: the fast-fail code of abort on Windows.
        output.status.code() == Some(0xC000_0409_u32 as i32)
    }
    #[cfg(not(any(unix, windows)))]
    {
        !output.status.success()
    }
}

fn probe(mode: &str) -> (Output, String) {
    let output = Command::new(PROBE)
        .arg(mode)
        .output()
        .expect("run panic-probe");
    let logs = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    (output, logs)
}

#[test]
fn panic_in_a_request_is_logged_with_its_id_and_stops_the_process() {
    let (output, logs) = probe("request");
    assert!(
        !output.status.success(),
        "the process must stop in failure: {logs}"
    );
    assert!(logs.contains("probe panic inside a request"), "{logs}");
    assert!(logs.contains(REQUEST_ID), "request id missing: {logs}");
    assert!(
        logs.contains("panic_probe::main"),
        "no stack trace frames: {logs}"
    );
    let line = logs
        .lines()
        .find(|l| l.contains("\"level\":\"ERROR\""))
        .expect("error line");
    let messages = line.matches("\"message\":").count();
    assert_eq!(messages, 1, "duplicate JSON key: {line}");
}

#[test]
fn panic_outside_requests_is_logged_without_an_id() {
    let (output, logs) = probe("startup");
    assert!(aborted(&output), "{:?} {logs}", output.status);
    assert!(logs.contains("probe panic at startup"), "{logs}");
    assert!(!logs.contains(REQUEST_ID), "{logs}");
}

// Review of lot 5 (I4): with logging off (or not yet set up), the panic
// report must still reach stderr.
#[test]
fn panic_report_survives_logging_turned_off() {
    let (output, logs) = probe("silent");
    assert!(aborted(&output), "{:?} {logs}", output.status);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("probe panic with logging off"), "{stderr}");
    assert!(
        stderr.contains("panic_probe::main"),
        "no stack trace: {stderr}"
    );
}
