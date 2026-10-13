//! `axum` serves from a listener of its own trait; this adapts the
//! `ndaal-io` listener (ADR-017: only `ndaal-io` opens sockets).

use std::io;
use std::net::SocketAddr;
use std::time::Duration;

use ndaal_io::Clock;
use ndaal_io::real::{ByteStream, SystemClock, TcpByteListener};
use ndaal_observe::metrics::{Counter, Registry};

/// Pause after a failed `accept` (out of file descriptors, for instance):
/// retrying at once would spin.
const ACCEPT_RETRY_DELAY: Duration = Duration::from_millis(100);

const ACCEPT_ERRORS_METRIC: &str = "ndaal_http_accept_errors_total";

/// An `ndaal-io` listener seen by `axum`.
pub(crate) struct HttpListener {
    listener: TcpByteListener,
    errors: Counter,
    clock: SystemClock,
}

impl HttpListener {
    pub(crate) fn new(listener: TcpByteListener, registry: &Registry) -> Self {
        let errors = registry
            .counter(
                ACCEPT_ERRORS_METRIC,
                "Failed accepts on the HTTP port.",
                &[],
            )
            .expect("constant metric name is valid and registered as a counter");
        Self {
            listener,
            errors,
            clock: SystemClock::new(),
        }
    }
}

impl axum::serve::Listener for HttpListener {
    type Io = ByteStream;
    type Addr = SocketAddr;

    async fn accept(&mut self) -> (ByteStream, SocketAddr) {
        loop {
            match self.listener.accept().await {
                Ok(accepted) => return accepted,
                Err(error) => {
                    // Not returned: axum's contract is to log and retry.
                    self.errors.increment(1);
                    tracing::warn!(%error, "http accept failed");
                    self.clock.sleep(ACCEPT_RETRY_DELAY).await;
                }
            }
        }
    }

    fn local_addr(&self) -> io::Result<SocketAddr> {
        self.listener.local_addr()
    }
}
