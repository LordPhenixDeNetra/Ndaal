//! Framed connections between nodes (ADR-014: persistent TCP connections
//! carrying frames). A frame is an opaque byte payload; the internal protocol
//! (`ndaal-rpc`) gives it a type and a request identifier.

use std::io;
use std::net::SocketAddr;

/// Largest frame accepted on either side; a larger announced length is
/// rejected before any allocation.
pub const MAX_FRAME_LEN: usize = 64 << 20;

/// A network on which this node can listen and connect.
pub trait Network {
    /// Listening socket.
    type Listener: Listener<Tx = Self::Tx, Rx = Self::Rx>;
    /// Sending half of a connection.
    type Tx: FrameTx;
    /// Receiving half of a connection.
    type Rx: FrameRx;

    /// Listens on `addr` (port 0: any free port, see [`Listener::local_addr`]).
    async fn bind(&self, addr: SocketAddr) -> io::Result<Self::Listener>;

    /// Opens a connection to `addr`.
    async fn connect(&self, addr: SocketAddr) -> io::Result<(Self::Tx, Self::Rx)>;
}

/// A listening socket.
pub trait Listener {
    /// Sending half of an accepted connection.
    type Tx: FrameTx;
    /// Receiving half of an accepted connection.
    type Rx: FrameRx;

    /// The address actually bound.
    fn local_addr(&self) -> SocketAddr;

    /// Waits for the next incoming connection and its peer address.
    async fn accept(&mut self) -> io::Result<(Self::Tx, Self::Rx, SocketAddr)>;
}

/// Sending half: frames arrive in the order they are sent.
pub trait FrameTx {
    /// Sends one frame; fails with `InvalidInput` above [`MAX_FRAME_LEN`].
    ///
    /// Not cancel-safe: a send dropped before completion may leave part of
    /// the frame on the wire; the connection must then be dropped.
    async fn send(&mut self, frame: &[u8]) -> io::Result<()>;
}

/// Receiving half.
pub trait FrameRx {
    /// Receives the next frame (cancel-safe: dropping the future loses no
    /// data). `UnexpectedEof` once the peer has closed its
    /// sending half, `InvalidData` for an announced length above [`MAX_FRAME_LEN`].
    async fn recv(&mut self) -> io::Result<Vec<u8>>;
}
