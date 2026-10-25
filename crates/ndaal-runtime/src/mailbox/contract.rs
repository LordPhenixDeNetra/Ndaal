//! The messaging contract, run on both transports (feature `test-contract`):
//! the real one in `ndaal-runtime`'s tests, the simulated one in
//! `ndaal-sim`'s, which has the executor. Each check takes fresh mailboxes
//! (3 cores, capacity [`CAPACITY`]) whose `pump` tasks the caller runs, and
//! shuts the runtime down at the end so those tasks finish.

use super::{CoreId, CoreMailbox, Incoming, ReplyError, SendError, Transport};

/// Queue capacity the checks expect.
pub const CAPACITY: usize = 2;
/// Cores the checks expect.
pub const CORES: u16 = 3;

/// Mailboxes the checks use.
pub type Boxes<T> = [CoreMailbox<u32, u32, T>];

async fn message<T: Transport<u32, u32> + 'static>(b: &CoreMailbox<u32, u32, T>) -> (CoreId, u32) {
    match b.recv().await {
        Some(Incoming::Message { from, msg }) => (from, msg),
        other => panic!("expected a message, got {other:?}"),
    }
}

async fn request<T: Transport<u32, u32> + 'static>(
    b: &CoreMailbox<u32, u32, T>,
) -> (CoreId, u32, super::Replier<u32>) {
    match b.recv().await {
        Some(Incoming::Request { from, msg, replier }) => (from, msg, replier),
        other => panic!("expected a request, got {other:?}"),
    }
}

/// Messages from one core to another arrive in order, with their sender.
pub async fn fifo_per_pair<T: Transport<u32, u32> + 'static>(b: &Boxes<T>) {
    b[0].try_send(CoreId(1), 10).expect("room");
    b[0].try_send(CoreId(1), 11).expect("room");
    assert_eq!(message(&b[1]).await, (CoreId(0), 10));
    assert_eq!(message(&b[1]).await, (CoreId(0), 11));
    b[0].shutdown();
}

/// A full queue gives the message back.
pub async fn full_returns_the_message<T: Transport<u32, u32> + 'static>(b: &Boxes<T>) {
    for n in 0..CAPACITY {
        b[0].try_send(CoreId(1), n as u32).expect("room");
    }
    assert_eq!(b[0].try_send(CoreId(1), 99), Err(SendError::Full(99)));
    // Other pairs are not affected.
    b[2].try_send(CoreId(1), 7)
        .expect("another sender has its own queue");
    b[0].shutdown();
}

/// Requests get their replies.
pub async fn reply_always_delivered<T: Transport<u32, u32> + 'static>(b: &Boxes<T>) {
    let first = b[0].request(CoreId(1), 5).expect("room");
    let second = b[0].request(CoreId(1), 6).expect("room");
    for _ in 0..2 {
        let (from, msg, replier) = request(&b[1]).await;
        assert_eq!(from, CoreId(0));
        replier.reply(msg * 10);
    }
    assert_eq!(first.await, Ok(50));
    assert_eq!(second.await, Ok(60));
    b[0].shutdown();
}

/// Without a free reply slot, a request is refused before leaving, even
/// when the message queue has room.
pub async fn request_without_reply_slot_is_refused<T: Transport<u32, u32> + 'static>(b: &Boxes<T>) {
    let first = b[0].request(CoreId(1), 1).expect("room");
    let second = b[0].request(CoreId(1), 2).expect("room");
    let (_, _, r1) = request(&b[1]).await;
    let (_, _, r2) = request(&b[1]).await;
    // The message queue is empty again; the reply slots are not.
    assert!(matches!(
        b[0].request(CoreId(1), 3),
        Err(SendError::Full(3))
    ));
    r1.reply(1);
    r2.reply(2);
    assert_eq!(first.await, Ok(1));
    assert_eq!(second.await, Ok(2));
    let third = b[0]
        .request(CoreId(1), 4)
        .expect("slots freed by the replies");
    let (_, _, r3) = request(&b[1]).await;
    r3.reply(4);
    assert_eq!(third.await, Ok(4));
    b[0].shutdown();
}

/// A request dropped without reply fails the requester and frees the slot.
pub async fn dropped_replier<T: Transport<u32, u32> + 'static>(b: &Boxes<T>) {
    for round in 0..=CAPACITY as u32 {
        let pending = b[0].request(CoreId(1), round).expect("slot free");
        let (_, _, replier) = request(&b[1]).await;
        drop(replier);
        assert_eq!(pending.await, Err(ReplyError::Dropped), "round {round}");
    }
    b[0].shutdown();
}

/// Two cores requesting each other up to capacity both get every reply.
pub async fn crossed_requests_do_not_deadlock<T: Transport<u32, u32> + 'static>(b: &Boxes<T>) {
    let mut waiting = Vec::new();
    for n in 0..CAPACITY as u32 {
        waiting.push(b[0].request(CoreId(1), n).expect("room"));
        waiting.push(b[1].request(CoreId(0), n).expect("room"));
    }
    for _ in 0..CAPACITY {
        let (_, msg, replier) = request(&b[1]).await;
        replier.reply(msg);
        let (_, msg, replier) = request(&b[0]).await;
        replier.reply(msg);
    }
    for pending in waiting {
        assert!(pending.await.is_ok());
    }
    b[0].shutdown();
}

/// After shutdown: sends refused, waiting requests failed, `recv` ends.
pub async fn shutdown<T: Transport<u32, u32> + 'static>(b: &Boxes<T>) {
    let pending = b[0].request(CoreId(1), 1).expect("room");
    b[2].shutdown();
    assert_eq!(pending.await, Err(ReplyError::ShuttingDown));
    assert_eq!(b[0].try_send(CoreId(1), 2), Err(SendError::ShuttingDown(2)));
    assert!(matches!(
        b[0].request(CoreId(1), 3),
        Err(SendError::ShuttingDown(3))
    ));
    assert!(b[2].recv().await.is_none());
}

/// A request abandoned because the runtime stops is not a bug: the
/// requester gets `ShuttingDown`, not `Dropped`, whichever it sees first.
pub async fn replier_dropped_by_shutdown<T: Transport<u32, u32> + 'static>(b: &Boxes<T>) {
    let pending = b[0].request(CoreId(1), 1).expect("room");
    let (_, _, replier) = request(&b[1]).await;
    b[1].shutdown();
    drop(replier);
    assert_eq!(pending.await, Err(ReplyError::ShuttingDown));
}
