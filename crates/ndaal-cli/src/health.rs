//! `ndaal health`: asks a node whether it is ready, for container health
//! checks (EF-OPS-07). A minimal HTTP/1.1 client: one request, one status.

use std::io;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::time::Duration;

use ndaal_io::Clock;
use ndaal_io::real::{ByteStream, SystemClock};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Longest wait for the answer; Docker's own timeout is set above it.
const HEALTH_TIMEOUT: Duration = Duration::from_secs(5);

/// Bytes read from the answer: the status line is all that matters.
const MAX_RESPONSE: u64 = 4096;

const REQUEST: &[u8] =
    b"GET /health/ready HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n";

/// Why a node is not considered ready.
#[derive(Debug)]
pub(crate) enum HealthError {
    /// Connection refused, reset...
    Unreachable(io::Error),
    /// No answer within [`HEALTH_TIMEOUT`].
    Timeout,
    /// An HTTP status other than 200.
    Status(String),
    /// Not an HTTP answer.
    Invalid,
}

/// The address to ask: a node listening on every interface (`0.0.0.0`,
/// `[::]`) is reached on the loopback of the same family.
pub(crate) fn probe_addr(listen: SocketAddr) -> SocketAddr {
    let ip = match listen.ip() {
        IpAddr::V4(ip) if ip.is_unspecified() => IpAddr::V4(Ipv4Addr::LOCALHOST),
        IpAddr::V6(ip) if ip.is_unspecified() => IpAddr::V6(Ipv6Addr::LOCALHOST),
        ip => ip,
    };
    SocketAddr::new(ip, listen.port())
}

/// Ok when `GET /health/ready` answers 200 within [`HEALTH_TIMEOUT`].
pub(crate) async fn check(addr: SocketAddr) -> Result<(), HealthError> {
    let clock = SystemClock::new();
    tokio::select! {
        result = ask(addr) => result,
        () = clock.sleep(HEALTH_TIMEOUT) => Err(HealthError::Timeout),
    }
}

async fn ask(addr: SocketAddr) -> Result<(), HealthError> {
    let mut stream = ByteStream::connect(addr)
        .await
        .map_err(HealthError::Unreachable)?;
    stream
        .write_all(REQUEST)
        .await
        .map_err(HealthError::Unreachable)?;
    let mut response = Vec::new();
    (&mut stream)
        .take(MAX_RESPONSE)
        .read_to_end(&mut response)
        .await
        .map_err(HealthError::Unreachable)?;
    status(&response)
}

/// Ready when the status line says 200.
fn status(response: &[u8]) -> Result<(), HealthError> {
    let text = String::from_utf8_lossy(response);
    let status_line = text.lines().next().unwrap_or_default();
    let mut words = status_line.split(' ');
    match (words.next(), words.next()) {
        (Some("HTTP/1.1" | "HTTP/1.0"), Some("200")) => Ok(()),
        (Some("HTTP/1.1" | "HTTP/1.0"), Some(code)) => Err(HealthError::Status(code.to_owned())),
        _ => Err(HealthError::Invalid),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_200_status_line_is_ready() {
        assert!(
            status(
                b"HTTP/1.1 200 OK
content-length: 3

ok
"
            )
            .is_ok()
        );
        assert!(
            status(
                b"HTTP/1.0 200 OK

"
            )
            .is_ok()
        );
        assert!(matches!(
            status(b"HTTP/1.1 503 Service Unavailable

"),
            Err(HealthError::Status(code)) if code == "503"
        ));
        for invalid in [
            &b""[..],
            b"SSH-2.0-OpenSSH
",
            b"HTTP/1.1",
        ] {
            assert!(matches!(status(invalid), Err(HealthError::Invalid)));
        }
    }

    #[test]
    fn unspecified_addresses_become_loopback() {
        let cases = [
            ("0.0.0.0:8080", "127.0.0.1:8080"),
            ("[::]:8080", "[::1]:8080"),
            ("127.0.0.2:9", "127.0.0.2:9"),
            ("[::1]:9", "[::1]:9"),
        ];
        for (listen, expected) in cases {
            let listen: SocketAddr = listen.parse().expect("listen");
            assert_eq!(probe_addr(listen), expected.parse().expect("expected"));
        }
    }
}
