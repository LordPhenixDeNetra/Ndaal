//! The J01 exit scenario: a client exchanges numbered frames with an echo
//! server while a chaos task partitions the network and cuts connections.
//! The client retries until every frame is answered.

use std::cell::Cell;
use std::future::{Future, poll_fn};
use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::pin::pin;
use std::rc::Rc;
use std::task::Poll;
use std::time::Duration;

use ndaal_io::sim::SimHost;
use ndaal_io::{Clock, FrameRx, FrameTx, Listener, Network, Rng, Xoshiro256PlusPlus};

use crate::{Limits, Outcome, Sim};

const SERVER: IpAddr = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
const CLIENT: IpAddr = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2));
const PORT: u16 = 7000;
const REPLY_TIMEOUT: Duration = Duration::from_millis(100);
const MAX_BACKOFF: Duration = Duration::from_millis(500);
const LIMITS: Limits = Limits {
    max_steps: 2_000_000,
    max_virtual_time: Duration::from_secs(600),
};

/// What happened during one run of [`ping_pong`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Report {
    /// How the run ended.
    pub outcome: Outcome,
    /// Frames answered.
    pub replies: u32,
    /// Partitions injected by the chaos task.
    pub partitions: u32,
    /// Connections the client had to abandon and reopen.
    pub reconnects: u32,
    /// Virtual time at the end of the run.
    pub virtual_time: Duration,
    /// Trace fingerprint.
    pub fingerprint: u64,
}

#[derive(Default)]
struct Counters {
    replies: Cell<u32>,
    partitions: Cell<u32>,
    reconnects: Cell<u32>,
    done: Cell<bool>,
}

fn bump(counter: &Cell<u32>) {
    counter.set(counter.get() + 1);
}

/// `Some(output)` if `future` completes within `timeout` on `clock`, else `None`.
pub async fn with_timeout<C: Clock, F: Future>(
    clock: &C,
    timeout: Duration,
    future: F,
) -> Option<F::Output> {
    let mut future = pin!(future);
    let mut sleep = pin!(clock.sleep(timeout));
    poll_fn(|cx| {
        if let Poll::Ready(output) = future.as_mut().poll(cx) {
            return Poll::Ready(Some(output));
        }
        if sleep.as_mut().poll(cx).is_ready() {
            return Poll::Ready(None);
        }
        Poll::Pending
    })
    .await
}

/// Runs the scenario with `messages` frames; deterministic for a given seed.
pub fn ping_pong(seed: u64, messages: u32) -> Report {
    let sim = Sim::new(seed);
    let net = sim.network();
    net.set_latency(Duration::from_millis(1), Duration::from_millis(20));
    let counters = Rc::new(Counters::default());
    sim.spawn(server(sim.clone(), net.host(SERVER)));
    sim.spawn(client(
        sim.clone(),
        net.host(CLIENT),
        messages,
        Rc::clone(&counters),
    ));
    sim.spawn(chaos(sim.clone(), Rc::clone(&counters)));
    let outcome = sim.run(LIMITS);
    Report {
        outcome,
        replies: counters.replies.get(),
        partitions: counters.partitions.get(),
        reconnects: counters.reconnects.get(),
        virtual_time: sim.clock().monotonic(),
        fingerprint: sim.fingerprint(),
    }
}

async fn server(sim: Sim, host: SimHost) {
    let Ok(mut listener) = host.bind(SocketAddr::new(SERVER, PORT)).await else {
        sim.trace("server: bind failed");
        return;
    };
    // Ends when the host is stopped at the end of the scenario.
    while let Ok((tx, rx, _peer)) = listener.accept().await {
        sim.trace("server: accept");
        sim.spawn(echo(tx, rx));
    }
}

async fn echo(mut tx: impl FrameTx, mut rx: impl FrameRx) {
    while let Ok(frame) = rx.recv().await {
        if tx.send(&frame).await.is_err() {
            return;
        }
    }
}

async fn client(sim: Sim, host: SimHost, messages: u32, counters: Rc<Counters>) {
    let clock = sim.clock();
    let target = SocketAddr::new(SERVER, PORT);
    let mut conn = None;
    let mut attempt = 0;
    let mut next = 0u32;
    while next < messages {
        let (tx, rx) = match conn.as_mut() {
            Some(pair) => pair,
            None => match host.connect(target).await {
                Ok(pair) => conn.insert(pair),
                Err(e) => {
                    sim.trace(format!("client: connect {:?}", e.kind()));
                    backoff(&clock, &mut attempt).await;
                    continue;
                }
            },
        };
        let frame = next.to_le_bytes();
        sim.trace(format!("client: send {next}"));
        let reply = match tx.send(&frame).await {
            Ok(()) => with_timeout(&clock, REPLY_TIMEOUT, rx.recv())
                .await
                .unwrap_or_else(|| Err(io::ErrorKind::TimedOut.into())),
            Err(e) => Err(e),
        };
        match reply {
            Ok(got) if got == frame => {
                sim.trace(format!("client: reply {next}"));
                bump(&counters.replies);
                next += 1;
                attempt = 0;
            }
            other => {
                let kind = other.err().map(|e| e.kind());
                sim.trace(format!("client: lost {next} ({kind:?})"));
                conn = None;
                bump(&counters.reconnects);
                backoff(&clock, &mut attempt).await;
            }
        }
    }
    counters.done.set(true);
    sim.trace("client: done");
    // Lets the server and its echo tasks end, so the run can finish.
    sim.network().stop_host(SERVER);
}

/// Exponential backoff, 10 ms doubling up to [`MAX_BACKOFF`].
async fn backoff(clock: &impl Clock, attempt: &mut u32) {
    let delay = Duration::from_millis(10).saturating_mul(1 << (*attempt).min(6));
    *attempt += 1;
    clock.sleep(delay.min(MAX_BACKOFF)).await;
}

fn between(rng: &mut Xoshiro256PlusPlus, low_ms: u64, high_ms: u64) -> Duration {
    Duration::from_millis(low_ms + rng.next_u64() % (high_ms - low_ms + 1))
}

/// Alternates partitions and bursts of connection resets until the client is done.
async fn chaos(sim: Sim, counters: Rc<Counters>) {
    let (clock, net, mut rng) = (sim.clock(), sim.network(), sim.rng("chaos"));
    loop {
        clock.sleep(between(&mut rng, 20, 300)).await;
        if counters.done.get() {
            return;
        }
        let fault = between(&mut rng, 50, 300);
        if rng.next_u64() % 2 == 0 {
            net.partition(SERVER, CLIENT);
            bump(&counters.partitions);
            sim.trace("chaos: partition");
            clock.sleep(fault).await;
            net.heal(SERVER, CLIENT);
            sim.trace("chaos: heal");
        } else {
            net.set_reset_probability(0.3);
            sim.trace("chaos: resets on");
            clock.sleep(fault).await;
            net.set_reset_probability(0.0);
            sim.trace("chaos: resets off");
        }
    }
}
