//! Behavior every `Network` implementation must share (real and simulated).
//! Run with zero latency: both sides progress through `tokio::join!`.

#![allow(dead_code)] // each test binary uses a subset

use std::io::ErrorKind;
use std::net::SocketAddr;

use ndaal_io::{FrameRx, FrameTx, Listener, MAX_FRAME_LEN, Network};

/// `server` listens on `bind_addr`; `client` connects. `refused_addr` has no listener.
pub async fn net_contract<N: Network>(
    server: &N,
    client: &N,
    bind_addr: SocketAddr,
    refused_addr: SocketAddr,
) {
    echo_in_order_then_clean_close(server, client, bind_addr).await;
    connect_without_listener_is_refused(client, refused_addr).await;
}

async fn echo_in_order_then_clean_close<N: Network>(server: &N, client: &N, addr: SocketAddr) {
    let mut listener = server.bind(addr).await.expect("bind");
    let target = listener.local_addr();
    let frames: Vec<Vec<u8>> = vec![Vec::new(), b"a".to_vec(), vec![7u8; 1 << 20]];

    let serve = async {
        let (mut tx, mut rx, _peer) = listener.accept().await.expect("accept");
        loop {
            match rx.recv().await {
                Ok(frame) => tx.send(&frame).await.expect("echo"),
                Err(e) => return e.kind(),
            }
        }
    };
    let talk = async {
        let (mut tx, mut rx) = client.connect(target).await.expect("connect");
        for frame in &frames {
            tx.send(frame).await.expect("send");
        }
        for frame in &frames {
            assert_eq!(&rx.recv().await.expect("recv"), frame);
        }
        let too_big = vec![0u8; MAX_FRAME_LEN + 1];
        let refused = tx.send(&too_big).await;
        assert_eq!(
            refused.err().map(|e| e.kind()),
            Some(ErrorKind::InvalidInput)
        );
        drop(tx);
        drop(rx);
    };
    let (server_end, ()) = tokio::join!(serve, talk);
    assert_eq!(server_end, ErrorKind::UnexpectedEof);
}

async fn connect_without_listener_is_refused<N: Network>(client: &N, addr: SocketAddr) {
    let refused = client.connect(addr).await;
    assert_eq!(
        refused.err().map(|e| e.kind()),
        Some(ErrorKind::ConnectionRefused)
    );
}
