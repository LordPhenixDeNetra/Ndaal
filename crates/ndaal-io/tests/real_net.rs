//! Real TCP network (T01.07).

// Raw TCP peers stand in for hostile or broken remote nodes (ADR-017 allows
// direct network access in ndaal-io).
#![allow(clippy::disallowed_methods, clippy::disallowed_types)]

mod common;

use std::io::ErrorKind;
use std::net::SocketAddr;

use ndaal_io::real::RealNetwork;
use ndaal_io::{FrameRx, Listener, Network};
use tokio::io::AsyncWriteExt;

fn local(port: u16) -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], port))
}

#[tokio::test(flavor = "current_thread")]
async fn real_network_meets_the_contract() {
    // A port that was free a moment ago: nothing listens there.
    let free = RealNetwork.bind(local(0)).await.expect("bind").local_addr();
    common::net::net_contract(&RealNetwork, &RealNetwork, local(0), free).await;
}

/// A raw peer that writes `bytes` then closes.
async fn raw_peer_then_recv(bytes: &[u8]) -> std::io::Result<Vec<u8>> {
    let listener = tokio::net::TcpListener::bind(local(0)).await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    let peer = async {
        let (mut socket, _) = listener.accept().await.expect("accept");
        socket.write_all(bytes).await.expect("write");
        socket.shutdown().await.expect("shutdown");
        socket
    };
    let client = async {
        let (_tx, mut rx) = RealNetwork.connect(addr).await.expect("connect");
        rx.recv().await
    };
    let (_socket, result) = tokio::join!(peer, client);
    result
}

#[tokio::test(flavor = "current_thread")]
async fn hostile_frame_length_is_rejected_without_allocation() {
    let result = raw_peer_then_recv(&u32::MAX.to_le_bytes()).await;
    assert_eq!(result.err().map(|e| e.kind()), Some(ErrorKind::InvalidData));
}

#[tokio::test(flavor = "current_thread")]
async fn connection_cut_mid_frame_is_unexpected_eof() {
    let mut bytes = 10u32.to_le_bytes().to_vec();
    bytes.extend_from_slice(b"abc");
    let result = raw_peer_then_recv(&bytes).await;
    assert_eq!(
        result.err().map(|e| e.kind()),
        Some(ErrorKind::UnexpectedEof)
    );
}

// Review of lot 4: a recv cancelled mid-frame (timeout) must not lose the
// frame boundaries; the next recv still returns the whole frame.
#[tokio::test(flavor = "current_thread")]
async fn recv_is_cancel_safe() {
    let listener = tokio::net::TcpListener::bind(local(0)).await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    let peer = async {
        let (mut socket, _) = listener.accept().await.expect("accept");
        let mut frame = 3u32.to_le_bytes().to_vec();
        frame.extend_from_slice(b"abc");
        socket
            .write_all(&frame[..2])
            .await
            .expect("first half of the length");
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        socket.write_all(&frame[2..]).await.expect("rest");
        socket
    };
    let client = async {
        let (_tx, mut rx) = RealNetwork.connect(addr).await.expect("connect");
        let early = tokio::time::timeout(std::time::Duration::from_millis(30), rx.recv()).await;
        assert!(early.is_err(), "the frame was not complete yet");
        rx.recv().await
    };
    let (_socket, result) = tokio::join!(peer, client);
    assert_eq!(result.expect("frame after the cancelled recv"), b"abc");
}
