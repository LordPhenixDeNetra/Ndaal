//! Health and metrics endpoints on a real server (P-27, T01.22).

use std::net::SocketAddr;

use ndaal_io::real::{ByteStream, TcpByteListener};
use ndaal_observe::metrics::Registry;
use ndaal_proto_http::{NodeState, router, serve};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::oneshot;

/// A running server, and the sender that stops it.
struct Server {
    addr: SocketAddr,
    state: NodeState,
    registry: Registry,
    stop: Option<oneshot::Sender<()>>,
    task: tokio::task::JoinHandle<std::io::Result<()>>,
}

async fn start() -> Server {
    let listener = TcpByteListener::bind("127.0.0.1:0".parse().expect("addr"))
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("local addr");
    let registry = Registry::default();
    let state = NodeState::new(registry.clone());
    let (stop, stopped) = oneshot::channel::<()>();
    let task = tokio::spawn(serve(
        listener,
        router(state.clone()),
        registry.clone(),
        async {
            let _ = stopped.await;
        },
    ));
    Server {
        addr,
        state,
        registry,
        stop: Some(stop),
        task,
    }
}

/// Status line, headers and body of `GET path`.
async fn get(addr: SocketAddr, path: &str) -> String {
    let mut stream = ByteStream::connect(addr).await.expect("connect");
    let request = format!("GET {path} HTTP/1.1\r\nHost: test\r\nConnection: close\r\n\r\n");
    stream.write_all(request.as_bytes()).await.expect("write");
    let mut response = String::new();
    stream.read_to_string(&mut response).await.expect("read");
    response
}

#[tokio::test(flavor = "current_thread")]
async fn live_is_always_ok() {
    let server = start().await;
    let response = get(server.addr, "/health/live").await;
    assert!(response.starts_with("HTTP/1.1 200"), "{response}");
    assert!(response.ends_with("ok\n"), "{response}");
}

#[tokio::test(flavor = "current_thread")]
async fn ready_is_503_until_set_ready() {
    let server = start().await;
    let before = get(server.addr, "/health/ready").await;
    assert!(before.starts_with("HTTP/1.1 503"), "{before}");
    server.state.set_ready();
    let after = get(server.addr, "/health/ready").await;
    assert!(after.starts_with("HTTP/1.1 200"), "{after}");
}

#[tokio::test(flavor = "current_thread")]
async fn metrics_are_prometheus_text() {
    let server = start().await;
    get(server.addr, "/health/live").await;
    let response = get(server.addr, "/metrics").await.to_ascii_lowercase();
    assert!(response.starts_with("http/1.1 200"), "{response}");
    assert!(
        response.contains("content-type: text/plain; version=0.0.4"),
        "{response}"
    );
    let line = "ndaal_http_requests_total{route=\"/health/live\",status=\"200\"} 1";
    assert!(response.contains(line), "{response}");
}

#[tokio::test(flavor = "current_thread")]
async fn unknown_paths_are_404_and_grouped() {
    let server = start().await;
    for path in ["/nope", "/also/nope?x=1"] {
        let response = get(server.addr, path).await;
        assert!(response.starts_with("HTTP/1.1 404"), "{response}");
    }
    let text = server.registry.render();
    let line = "ndaal_http_requests_total{route=\"other\",status=\"404\"} 2";
    assert!(text.contains(line), "{text}");
    assert!(!text.contains("nope"), "{text}");
}

#[tokio::test(flavor = "current_thread")]
async fn serve_returns_after_shutdown() {
    let mut server = start().await;
    get(server.addr, "/health/live").await;
    server.stop.take().expect("sender").send(()).expect("send");
    server.task.await.expect("join").expect("serve");
    assert!(ByteStream::connect(server.addr).await.is_err());
}

/// A client that sends half a request and waits must not keep the server
/// from stopping: the grace period bounds the wait (CLAUDE.md section 4.4).
#[tokio::test(flavor = "current_thread")]
async fn shutdown_does_not_wait_for_stuck_clients() {
    let mut server = start().await;
    let mut stuck = ByteStream::connect(server.addr).await.expect("connect");
    stuck
        .write_all(
            b"GET /health/live HTTP/1.1
",
        )
        .await
        .expect("write");
    // Let the server accept and start reading the request.
    let response = get(server.addr, "/health/live").await;
    assert!(response.starts_with("HTTP/1.1 200"), "{response}");
    server.stop.take().expect("sender").send(()).expect("send");
    server.task.await.expect("join").expect("serve");
    drop(stuck);
}
