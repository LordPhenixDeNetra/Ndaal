//! Simulated network of framed connections (decision 1 of the J01 lot 4 plan).
//!
//! It models what TCP can do: per-frame latency with order kept on each
//! connection, connection reset, refused connection, partitions (existing
//! connections cut, new ones refused) and stopped hosts. Frames in flight on a
//! cut connection are lost. Every random choice comes from the seed.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::future::poll_fn;
use std::io;
use std::net::{IpAddr, SocketAddr};
use std::rc::Rc;
use std::task::{Poll, Waker};
use std::time::Duration;

mod frames;

use frames::cut;

use super::SimClock;
use crate::{Listener, Network, Rng, Xoshiro256PlusPlus};

/// First port handed out for port 0 and for client sockets.
const FIRST_EPHEMERAL_PORT: u16 = 49152;

/// A simulated network; clones share the same state.
#[derive(Debug, Clone)]
pub struct SimNetwork {
    state: Rc<RefCell<NetState>>,
}

/// One host of a [`SimNetwork`]: binds and connects from its own address.
#[derive(Debug, Clone)]
pub struct SimHost {
    net: SimNetwork,
    ip: IpAddr,
}

/// A simulated listening socket; dropping it stops listening.
#[derive(Debug)]
pub struct SimListener {
    net: SimNetwork,
    id: u64,
    local: SocketAddr,
}

/// Sending half of a simulated connection; dropping it closes this direction.
#[derive(Debug)]
pub struct SimTx {
    net: SimNetwork,
    conn: u64,
    pipe: usize,
}

/// Receiving half of a simulated connection.
#[derive(Debug)]
pub struct SimRx {
    net: SimNetwork,
    conn: u64,
    pipe: usize,
}

#[derive(Debug)]
struct NetState {
    clock: SimClock,
    rng: Xoshiro256PlusPlus,
    latency: (Duration, Duration),
    reset_probability: f64,
    listeners: BTreeMap<SocketAddr, u64>,
    queues: BTreeMap<u64, ListenQueue>,
    conns: BTreeMap<u64, Conn>,
    partitions: BTreeSet<(IpAddr, IpAddr)>,
    stopped: BTreeSet<IpAddr>,
    next_id: u64,
    next_port: BTreeMap<IpAddr, u16>,
}

#[derive(Debug, Default)]
struct ListenQueue {
    /// Accepted-to-be connections: (connection, client address).
    pending: VecDeque<(u64, SocketAddr)>,
    waker: Option<Waker>,
    closed: bool,
}

/// `pipes[0]` carries frames from the client (end 0) to the server (end 1),
/// `pipes[1]` the other way.
#[derive(Debug)]
struct Conn {
    ends: [IpAddr; 2],
    pipes: [Pipe; 2],
    broken: bool,
}

#[derive(Debug, Default)]
struct Pipe {
    /// (delivery time, frame), delivery times non-decreasing.
    frames: VecDeque<(Duration, Vec<u8>)>,
    last_delivery: Duration,
    waker: Option<Waker>,
    sender_closed: bool,
    /// The receiving half is gone: like TCP, further sends fail.
    receiver_closed: bool,
}

fn pair(a: IpAddr, b: IpAddr) -> (IpAddr, IpAddr) {
    if a <= b { (a, b) } else { (b, a) }
}

fn error(kind: io::ErrorKind) -> io::Error {
    io::Error::from(kind)
}

fn wake_all(wakers: Vec<Waker>) {
    for waker in wakers {
        waker.wake();
    }
}

impl NetState {
    fn blocked(&self, a: IpAddr, b: IpAddr) -> bool {
        self.stopped.contains(&a)
            || self.stopped.contains(&b)
            || self.partitions.contains(&pair(a, b))
    }

    fn next_port(&mut self, ip: IpAddr) -> u16 {
        loop {
            let port = self.next_port.entry(ip).or_insert(FIRST_EPHEMERAL_PORT);
            let candidate = *port;
            *port = port.checked_add(1).unwrap_or(FIRST_EPHEMERAL_PORT);
            if !self.listeners.contains_key(&SocketAddr::new(ip, candidate)) {
                return candidate;
            }
        }
    }

    /// Cuts every connection matching `cut`; returns the wakers to wake once
    /// the state is no longer borrowed.
    fn break_conns(&mut self, matches: impl Fn(&Conn) -> bool) -> Vec<Waker> {
        let ids: Vec<u64> = self
            .conns
            .iter()
            .filter(|(_, c)| !c.broken && matches(c))
            .map(|(id, _)| *id)
            .collect();
        ids.into_iter().flat_map(|id| cut(self, id)).collect()
    }

    fn latency(&mut self) -> Duration {
        let (min, max) = self.latency;
        let spread = u64::try_from(max.saturating_sub(min).as_nanos()).unwrap_or(u64::MAX);
        let extra = if spread == 0 {
            0
        } else {
            self.rng.next_u64() % (spread + 1)
        };
        min + Duration::from_nanos(extra)
    }

    fn roll_reset(&mut self) -> bool {
        // 53 random bits as a fraction of 1, compared with the probability.
        self.reset_probability > 0.0
            && ((self.rng.next_u64() >> 11) as f64 / (1u64 << 53) as f64) < self.reset_probability
    }
}

impl SimNetwork {
    /// An empty network with zero latency; `seed` drives latencies and resets.
    pub fn new(seed: u64, clock: SimClock) -> Self {
        let state = NetState {
            clock,
            rng: Xoshiro256PlusPlus::from_seed(seed),
            latency: (Duration::ZERO, Duration::ZERO),
            reset_probability: 0.0,
            listeners: BTreeMap::new(),
            queues: BTreeMap::new(),
            conns: BTreeMap::new(),
            partitions: BTreeSet::new(),
            stopped: BTreeSet::new(),
            next_id: 0,
            next_port: BTreeMap::new(),
        };
        Self {
            state: Rc::new(RefCell::new(state)),
        }
    }

    /// The host with address `ip`.
    pub fn host(&self, ip: IpAddr) -> SimHost {
        SimHost {
            net: self.clone(),
            ip,
        }
    }

    /// Each frame takes a latency drawn uniformly in `[min, max]`.
    pub fn set_latency(&self, min: Duration, max: Duration) {
        self.state.borrow_mut().latency = (min, max.max(min));
    }

    /// Probability that a send cuts its connection (`ConnectionReset`).
    pub fn set_reset_probability(&self, probability: f64) {
        self.state.borrow_mut().reset_probability = probability.clamp(0.0, 1.0);
    }

    /// Cuts connections between `a` and `b` and refuses new ones until [`SimNetwork::heal`].
    pub fn partition(&self, a: IpAddr, b: IpAddr) {
        let wakers = {
            let mut st = self.state.borrow_mut();
            st.partitions.insert(pair(a, b));
            st.break_conns(|c| pair(c.ends[0], c.ends[1]) == pair(a, b))
        };
        wake_all(wakers);
    }

    /// Ends a partition; existing (cut) connections stay cut.
    pub fn heal(&self, a: IpAddr, b: IpAddr) {
        self.state.borrow_mut().partitions.remove(&pair(a, b));
    }

    /// Stops a host: its listeners close, its connections are cut, it is unreachable.
    pub fn stop_host(&self, ip: IpAddr) {
        let wakers = {
            let mut st = self.state.borrow_mut();
            st.stopped.insert(ip);
            let closed: Vec<u64> = st
                .listeners
                .iter()
                .filter(|(addr, _)| addr.ip() == ip)
                .map(|(_, id)| *id)
                .collect();
            st.listeners.retain(|addr, _| addr.ip() != ip);
            let mut wakers = Vec::new();
            for id in closed {
                if let Some(queue) = st.queues.get_mut(&id) {
                    queue.closed = true;
                    wakers.extend(queue.waker.take());
                }
            }
            wakers.extend(st.break_conns(|c| c.ends.contains(&ip)));
            wakers
        };
        wake_all(wakers);
    }

    /// Makes a stopped host reachable again (it must bind again).
    pub fn start_host(&self, ip: IpAddr) {
        self.state.borrow_mut().stopped.remove(&ip);
    }
}

impl Network for SimHost {
    type Listener = SimListener;
    type Tx = SimTx;
    type Rx = SimRx;

    async fn bind(&self, addr: SocketAddr) -> io::Result<SimListener> {
        let mut st = self.net.state.borrow_mut();
        if addr.ip() != self.ip || st.stopped.contains(&self.ip) {
            return Err(error(io::ErrorKind::AddrNotAvailable));
        }
        let port = if addr.port() == 0 {
            st.next_port(self.ip)
        } else {
            addr.port()
        };
        let local = SocketAddr::new(self.ip, port);
        if st.listeners.contains_key(&local) {
            return Err(error(io::ErrorKind::AddrInUse));
        }
        let id = st.next_id;
        st.next_id += 1;
        st.listeners.insert(local, id);
        st.queues.insert(id, ListenQueue::default());
        Ok(SimListener {
            net: self.net.clone(),
            id,
            local,
        })
    }

    async fn connect(&self, addr: SocketAddr) -> io::Result<(SimTx, SimRx)> {
        let refused = || Err(error(io::ErrorKind::ConnectionRefused));
        let (id, waker) = {
            let mut st = self.net.state.borrow_mut();
            if st.blocked(self.ip, addr.ip()) {
                return refused();
            }
            let Some(&listener) = st.listeners.get(&addr) else {
                return refused();
            };
            let id = st.next_id;
            st.next_id += 1;
            let local = SocketAddr::new(self.ip, st.next_port(self.ip));
            let conn = Conn {
                ends: [self.ip, addr.ip()],
                pipes: [Pipe::default(), Pipe::default()],
                broken: false,
            };
            st.conns.insert(id, conn);
            let Some(queue) = st.queues.get_mut(&listener) else {
                return refused();
            };
            queue.pending.push_back((id, local));
            (id, queue.waker.take())
        };
        wake_all(waker.into_iter().collect());
        let net = self.net.clone();
        Ok((
            SimTx {
                net: net.clone(),
                conn: id,
                pipe: 0,
            },
            SimRx {
                net,
                conn: id,
                pipe: 1,
            },
        ))
    }
}

impl Listener for SimListener {
    type Tx = SimTx;
    type Rx = SimRx;

    fn local_addr(&self) -> SocketAddr {
        self.local
    }

    async fn accept(&mut self) -> io::Result<(SimTx, SimRx, SocketAddr)> {
        let (id, peer) = poll_fn(|cx| {
            let mut st = self.net.state.borrow_mut();
            let Some(queue) = st.queues.get_mut(&self.id) else {
                return Poll::Ready(Err(error(io::ErrorKind::ConnectionAborted)));
            };
            if queue.closed {
                return Poll::Ready(Err(error(io::ErrorKind::ConnectionAborted)));
            }
            match queue.pending.pop_front() {
                Some(next) => Poll::Ready(Ok(next)),
                None => {
                    queue.waker = Some(cx.waker().clone());
                    Poll::Pending
                }
            }
        })
        .await?;
        let net = self.net.clone();
        Ok((
            SimTx {
                net: net.clone(),
                conn: id,
                pipe: 1,
            },
            SimRx {
                net,
                conn: id,
                pipe: 0,
            },
            peer,
        ))
    }
}

impl Drop for SimListener {
    /// Like TCP, connections not yet accepted are reset, not left waiting.
    fn drop(&mut self) {
        let wakers = {
            let mut st = self.net.state.borrow_mut();
            st.listeners.retain(|_, id| *id != self.id);
            let pending = st
                .queues
                .remove(&self.id)
                .map(|q| q.pending)
                .unwrap_or_default();
            let mut wakers = Vec::new();
            for (conn, _client) in pending {
                wakers.extend(cut(&mut st, conn));
            }
            wakers
        };
        wake_all(wakers);
    }
}
