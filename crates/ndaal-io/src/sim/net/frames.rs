//! Sending and receiving frames on simulated connections.

use std::future::poll_fn;
use std::io;
use std::task::{Poll, Waker};

use super::{NetState, SimRx, SimTx, error, wake_all};
use crate::{Clock, FrameRx, FrameTx, MAX_FRAME_LEN};

impl FrameTx for SimTx {
    async fn send(&mut self, frame: &[u8]) -> io::Result<()> {
        if frame.len() > MAX_FRAME_LEN {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "frame too large",
            ));
        }
        let (result, wakers) = enqueue(
            &mut self.net.state.borrow_mut(),
            self.conn,
            self.pipe,
            frame,
        );
        wake_all(wakers);
        result
    }
}

/// Queues `frame` on a pipe, or cuts the connection (partition, stopped host,
/// random reset). Returns the result and the wakers to wake after the borrow.
fn enqueue(st: &mut NetState, id: u64, pipe: usize, frame: &[u8]) -> (io::Result<()>, Vec<Waker>) {
    let reset = || Err(error(io::ErrorKind::ConnectionReset));
    let Some(conn) = st.conns.get(&id) else {
        return (reset(), Vec::new());
    };
    if conn.broken {
        return (reset(), Vec::new());
    }
    if conn.pipes[pipe].receiver_closed {
        return (reset(), cut(st, id));
    }
    let [a, b] = conn.ends;
    if st.blocked(a, b) || st.roll_reset() {
        return (reset(), cut(st, id));
    }
    let delivery = st.clock.monotonic() + st.latency();
    let Some(conn) = st.conns.get_mut(&id) else {
        return (reset(), Vec::new());
    };
    let pipe = &mut conn.pipes[pipe];
    let delivery = delivery.max(pipe.last_delivery);
    pipe.last_delivery = delivery;
    pipe.frames.push_back((delivery, frame.to_vec()));
    (Ok(()), pipe.waker.take().into_iter().collect())
}

/// Cuts one connection; returns the wakers to wake.
pub(super) fn cut(st: &mut NetState, id: u64) -> Vec<Waker> {
    let mut wakers = Vec::new();
    if let Some(conn) = st.conns.get_mut(&id) {
        conn.broken = true;
        for pipe in &mut conn.pipes {
            pipe.frames.clear();
            wakers.extend(pipe.waker.take());
        }
    }
    wakers
}

impl Drop for SimRx {
    /// Frames still queued for this half are discarded; the peer's next send
    /// fails (see `enqueue`).
    fn drop(&mut self) {
        let mut st = self.net.state.borrow_mut();
        if let Some(conn) = st.conns.get_mut(&self.conn) {
            let pipe = &mut conn.pipes[self.pipe];
            pipe.receiver_closed = true;
            pipe.frames.clear();
        }
    }
}

impl Drop for SimTx {
    fn drop(&mut self) {
        let waker = {
            let mut st = self.net.state.borrow_mut();
            st.conns.get_mut(&self.conn).and_then(|conn| {
                let pipe = &mut conn.pipes[self.pipe];
                pipe.sender_closed = true;
                pipe.waker.take()
            })
        };
        wake_all(waker.into_iter().collect());
    }
}

impl FrameRx for SimRx {
    async fn recv(&mut self) -> io::Result<Vec<u8>> {
        poll_fn(|cx| {
            let mut st = self.net.state.borrow_mut();
            let now = st.clock.monotonic();
            let clock = st.clock.clone();
            let Some(conn) = st.conns.get_mut(&self.conn) else {
                return Poll::Ready(Err(error(io::ErrorKind::ConnectionReset)));
            };
            if conn.broken {
                return Poll::Ready(Err(error(io::ErrorKind::ConnectionReset)));
            }
            let pipe = &mut conn.pipes[self.pipe];
            match pipe.frames.front() {
                Some((delivery, _)) if *delivery <= now => {
                    let frame = pipe.frames.pop_front().map(|(_, f)| f).unwrap_or_default();
                    Poll::Ready(Ok(frame))
                }
                Some((delivery, _)) => {
                    let delivery = *delivery;
                    drop(st);
                    clock.wake_at(delivery, cx.waker().clone());
                    Poll::Pending
                }
                None if pipe.sender_closed => Poll::Ready(Err(error(io::ErrorKind::UnexpectedEof))),
                None => {
                    pipe.waker = Some(cx.waker().clone());
                    Poll::Pending
                }
            }
        })
        .await
    }
}
