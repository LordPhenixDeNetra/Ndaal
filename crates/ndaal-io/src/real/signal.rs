//! Process shutdown requests.

use std::io;

/// Completes at the first Ctrl-C or, on Unix, SIGTERM: `docker stop` and
/// service managers send SIGTERM, and a container's PID 1 ignores it unless
/// it is handled.
pub async fn shutdown_signal() -> io::Result<()> {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        let mut term = signal(SignalKind::terminate())?;
        tokio::select! {
            result = tokio::signal::ctrl_c() => result,
            _ = term.recv() => Ok(()),
        }
    }
    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c().await
    }
}
