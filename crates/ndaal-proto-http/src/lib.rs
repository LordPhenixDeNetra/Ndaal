//! HTTP and gRPC API: vectors, transactions, administration.
//!
//! For now: the probes and the metrics (P-27), on `axum` (ADR-032).

#![forbid(unsafe_code)]

mod health;
mod listener;

use std::future::IntoFuture;
use std::io;
use std::time::Duration;

use axum::Router;
use ndaal_io::Clock;
use ndaal_io::real::{SystemClock, TcpByteListener};
use ndaal_observe::metrics::Registry;
use tokio::sync::oneshot;

pub use health::{NodeState, router};

/// Longest wait for open connections once shutdown starts: below the 10 s
/// Docker waits before SIGKILL, so a stuck client never turns a clean stop
/// into a kill (CLAUDE.md section 4.4: every wait has a deadline).
const SHUTDOWN_GRACE: Duration = Duration::from_secs(5);

/// Serves `router` on `listener` until `shutdown` completes, then lets the
/// requests in progress finish for at most [`SHUTDOWN_GRACE`]. `registry`
/// counts failed accepts.
pub async fn serve(
    listener: TcpByteListener,
    router: Router,
    registry: Registry,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> io::Result<()> {
    let (started, shutdown_started) = oneshot::channel();
    let signal = async move {
        shutdown.await;
        let _ = started.send(());
    };
    let server = axum::serve(listener::HttpListener::new(listener, &registry), router)
        .with_graceful_shutdown(signal)
        .into_future();
    let grace = async {
        if shutdown_started.await.is_ok() {
            SystemClock::new().sleep(SHUTDOWN_GRACE).await;
        } else {
            // The server ended before any shutdown: nothing to bound.
            std::future::pending::<()>().await;
        }
    };
    tokio::select! {
        result = server => result,
        () = grace => {
            tracing::warn!(
                grace_secs = SHUTDOWN_GRACE.as_secs(),
                "connections still open after the shutdown grace period, closing them"
            );
            Ok(())
        }
    }
}
