//! Simulated network (T01.08).

mod common;

use std::future::Future;
use std::io::ErrorKind;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::pin::pin;
use std::task::{Context, Poll, Waker};
use std::time::Duration;

use ndaal_io::sim::{SimClock, SimHost, SimNetwork};
use ndaal_io::{FrameRx, FrameTx, Listener, Network};

fn ip(n: u8) -> IpAddr {
    IpAddr::V4(Ipv4Addr::new(10, 0, 0, n))
}

fn addr(n: u8, port: u16) -> SocketAddr {
    SocketAddr::new(ip(n), port)
}

/// Polls once; the operation must complete without waiting.
fn ready<F: Future>(future: F) -> F::Output {
    match pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("expected the operation to complete at once"),
    }
}

/// Two hosts, `a` listening on 10.0.0.1:7000 and `b` connected to it.
struct Pair {
    clock: SimClock,
    net: SimNetwork,
    b: SimHost,
    server: (<SimHost as Network>::Tx, <SimHost as Network>::Rx),
    client: (<SimHost as Network>::Tx, <SimHost as Network>::Rx),
    listener: <SimHost as Network>::Listener,
}

fn pair(seed: u64) -> Pair {
    let clock = SimClock::new(0);
    let net = SimNetwork::new(seed, clock.clone());
    let (a, b) = (net.host(ip(1)), net.host(ip(2)));
    let mut listener = ready(a.bind(addr(1, 7000))).expect("bind");
    let client = ready(b.connect(addr(1, 7000))).expect("connect");
    let (tx, rx, peer) = ready(listener.accept()).expect("accept");
    assert_eq!(peer.ip(), ip(2));
    Pair {
        clock,
        net,
        b,
        server: (tx, rx),
        client,
        listener,
    }
}

#[tokio::test(flavor = "current_thread")]
async fn sim_network_meets_the_contract() {
    let net = SimNetwork::new(1, SimClock::new(0));
    let (a, b) = (net.host(ip(1)), net.host(ip(2)));
    common::net::net_contract(&a, &b, addr(1, 7000), addr(1, 7001)).await;
}

#[test]
fn latency_is_respected_and_order_kept() {
    let mut p = pair(2);
    p.net
        .set_latency(Duration::from_millis(10), Duration::from_millis(20));
    ready(p.client.0.send(b"first")).expect("send");
    ready(p.client.0.send(b"second")).expect("send");
    {
        let mut recv = pin!(p.server.1.recv());
        let poll =
            |rx: std::pin::Pin<&mut _>| Future::poll(rx, &mut Context::from_waker(Waker::noop()));
        assert!(poll(recv.as_mut()).is_pending());
        p.clock
            .advance_to(Duration::from_millis(10) - Duration::from_nanos(1));
        assert!(
            poll(recv.as_mut()).is_pending(),
            "delivered before the minimum latency"
        );
        p.clock.advance_to(Duration::from_millis(20));
        match poll(recv.as_mut()) {
            Poll::Ready(frame) => assert_eq!(frame.expect("frame"), b"first"),
            Poll::Pending => panic!("not delivered after the maximum latency"),
        }
    }
    assert_eq!(ready(p.server.1.recv()).expect("second"), b"second");
}

#[test]
fn partition_cuts_connections_and_refuses_new_ones_until_healed() {
    let mut p = pair(3);
    p.net
        .set_latency(Duration::from_millis(10), Duration::from_millis(10));
    ready(p.client.0.send(b"in flight")).expect("send");
    p.net.partition(ip(1), ip(2));
    let reset = Some(ErrorKind::ConnectionReset);
    // The partition itself cuts the connection: the receiver sees it before
    // anyone sends again.
    assert_eq!(ready(p.server.1.recv()).err().map(|e| e.kind()), reset);
    assert_eq!(ready(p.client.0.send(b"x")).err().map(|e| e.kind()), reset);
    let refused = ready(p.b.connect(addr(1, 7000))).err().map(|e| e.kind());
    assert_eq!(refused, Some(ErrorKind::ConnectionRefused));

    p.net.heal(ip(1), ip(2));
    let (mut tx, _rx) = ready(p.b.connect(addr(1, 7000))).expect("reconnect");
    let (_stx, mut srx, _) = ready(p.listener.accept()).expect("accept");
    ready(tx.send(b"after")).expect("send");
    p.clock.advance(Duration::from_millis(50));
    assert_eq!(ready(srx.recv()).expect("new frame"), b"after");
}

#[test]
fn reset_probability_one_cuts_at_first_send() {
    let mut p = pair(4);
    p.net.set_reset_probability(1.0);
    let sent = ready(p.client.0.send(b"x")).err().map(|e| e.kind());
    assert_eq!(sent, Some(ErrorKind::ConnectionReset));
}

#[test]
fn stopped_host_refuses_and_resets() {
    let mut p = pair(5);
    p.net.stop_host(ip(1));
    let reset = ready(p.client.0.send(b"x")).err().map(|e| e.kind());
    assert_eq!(reset, Some(ErrorKind::ConnectionReset));
    assert!(ready(p.listener.accept()).is_err());
    let refused = ready(p.b.connect(addr(1, 7000))).err().map(|e| e.kind());
    assert_eq!(refused, Some(ErrorKind::ConnectionRefused));
}

#[test]
fn bind_rules() {
    let net = SimNetwork::new(6, SimClock::new(0));
    let a = net.host(ip(1));
    let elsewhere = ready(a.bind(addr(2, 7000))).err().map(|e| e.kind());
    assert_eq!(elsewhere, Some(ErrorKind::AddrNotAvailable));
    let _first = ready(a.bind(addr(1, 7000))).expect("bind");
    let twice = ready(a.bind(addr(1, 7000))).err().map(|e| e.kind());
    assert_eq!(twice, Some(ErrorKind::AddrInUse));
    let any = ready(a.bind(addr(1, 0))).expect("bind port 0");
    assert_ne!(any.local_addr().port(), 0);
}

// Review of lot 4: like TCP, a listener dropped with connections not yet
// accepted resets them instead of leaving the clients waiting forever.
#[test]
fn dropping_a_listener_resets_its_pending_connections() {
    let net = SimNetwork::new(7, SimClock::new(0));
    let (a, b) = (net.host(ip(1)), net.host(ip(2)));
    let listener = ready(a.bind(addr(1, 7000))).expect("bind");
    let (_tx, mut rx) = ready(b.connect(addr(1, 7000))).expect("connect");
    drop(listener);
    let got = ready(rx.recv()).err().map(|e| e.kind());
    assert_eq!(got, Some(ErrorKind::ConnectionReset));
}

// Review of lot 4: like TCP, sending to a peer that closed its receiving
// half fails instead of queueing frames forever.
#[test]
fn sending_to_a_closed_receiver_fails() {
    let p = pair(8);
    let Pair {
        mut client, server, ..
    } = p;
    drop(server);
    let sent = ready(client.0.send(b"x")).err().map(|e| e.kind());
    assert_eq!(sent, Some(ErrorKind::ConnectionReset));
}
