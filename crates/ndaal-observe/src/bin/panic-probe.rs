//! Test fixture for `tests/panic.rs`: panics on purpose, inside a request
//! (`panic-probe request`), at startup (`panic-probe startup`), or with
//! logging off (`panic-probe silent`).

#![forbid(unsafe_code)]

use ndaal_common::request::{REQUEST_SPAN, RequestId};
use ndaal_observe::logs::{LogFormat, init};
use ndaal_observe::metrics::Registry;

/// Fixed so the test can look for it in the logs.
const REQUEST_ID: &str = "0123456789abcdef0123456789abcdef";

fn main() {
    let mode = std::env::args().nth(1);
    // `silent`: logging turned off, the raw stderr report must remain.
    let level = if mode.as_deref() == Some("silent") {
        "off"
    } else {
        "info"
    };
    let _logs = init(LogFormat::Json, level).expect("first and only log setup");
    ndaal_observe::panic::install(&Registry::default());
    match mode.as_deref() {
        Some("request") => {
            let id: RequestId = REQUEST_ID.parse().expect("constant id is valid");
            let span = tracing::info_span!(REQUEST_SPAN, request_id = %id);
            let _entered = span.enter();
            panic!("probe panic inside a request");
        }
        Some("startup") => panic!("probe panic at startup"),
        Some("silent") => panic!("probe panic with logging off"),
        _ => std::process::exit(2),
    }
}
