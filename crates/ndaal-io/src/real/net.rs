//! TCP with length-prefixed frames: `u32` little-endian length, then payload.

use std::io;
use std::net::SocketAddr;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{TcpListener, TcpStream};

use crate::{FrameRx, FrameTx, Listener, MAX_FRAME_LEN, Network};

/// The real network.
#[derive(Debug, Clone, Copy, Default)]
pub struct RealNetwork;

/// A real listening socket.
#[derive(Debug)]
pub struct RealListener {
    listener: TcpListener,
    local: SocketAddr,
}

/// Sending half of a real connection.
#[derive(Debug)]
pub struct RealTx {
    writer: OwnedWriteHalf,
}

/// Receiving half of a real connection.
#[derive(Debug)]
pub struct RealRx {
    reader: OwnedReadHalf,
    /// Bytes read but not yet returned: keeps a cancelled `recv` from losing
    /// part of a frame.
    buffer: Vec<u8>,
}

fn split(stream: TcpStream) -> io::Result<(RealTx, RealRx)> {
    // Frames are small and latency-sensitive: do not wait to coalesce them.
    stream.set_nodelay(true)?;
    let (reader, writer) = stream.into_split();
    Ok((
        RealTx { writer },
        RealRx {
            reader,
            buffer: Vec::new(),
        },
    ))
}

impl Network for RealNetwork {
    type Listener = RealListener;
    type Tx = RealTx;
    type Rx = RealRx;

    async fn bind(&self, addr: SocketAddr) -> io::Result<RealListener> {
        let listener = TcpListener::bind(addr).await?;
        let local = listener.local_addr()?;
        Ok(RealListener { listener, local })
    }

    async fn connect(&self, addr: SocketAddr) -> io::Result<(RealTx, RealRx)> {
        split(TcpStream::connect(addr).await?)
    }
}

impl Listener for RealListener {
    type Tx = RealTx;
    type Rx = RealRx;

    fn local_addr(&self) -> SocketAddr {
        self.local
    }

    async fn accept(&mut self) -> io::Result<(RealTx, RealRx, SocketAddr)> {
        let (stream, peer) = self.listener.accept().await?;
        let (tx, rx) = split(stream)?;
        Ok((tx, rx, peer))
    }
}

impl FrameTx for RealTx {
    async fn send(&mut self, frame: &[u8]) -> io::Result<()> {
        if frame.len() > MAX_FRAME_LEN {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "frame too large",
            ));
        }
        // Fits in u32: MAX_FRAME_LEN is 64 MiB.
        let len = (frame.len() as u32).to_le_bytes();
        self.writer.write_all(&len).await?;
        self.writer.write_all(frame).await
    }
}

impl FrameRx for RealRx {
    /// Cancel-safe: bytes are kept in `buffer` until a whole frame is there,
    /// and `AsyncReadExt::read` itself loses nothing when cancelled.
    async fn recv(&mut self) -> io::Result<Vec<u8>> {
        loop {
            if let Some(frame) = self.take_frame()? {
                return Ok(frame);
            }
            let mut chunk = [0u8; READ_CHUNK];
            let n = self.reader.read(&mut chunk).await?;
            if n == 0 {
                return Err(io::ErrorKind::UnexpectedEof.into());
            }
            self.buffer.extend_from_slice(&chunk[..n]);
        }
    }
}

/// Bytes read per system call.
const READ_CHUNK: usize = 64 << 10;

impl RealRx {
    /// The first complete frame of the buffer, if any.
    fn take_frame(&mut self) -> io::Result<Option<Vec<u8>>> {
        let Some(header) = self.buffer.first_chunk::<4>() else {
            return Ok(None);
        };
        let len = u32::from_le_bytes(*header) as usize;
        if len > MAX_FRAME_LEN {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "announced frame too large",
            ));
        }
        if self.buffer.len() < 4 + len {
            return Ok(None);
        }
        let frame = self.buffer[4..4 + len].to_vec();
        self.buffer.drain(..4 + len);
        Ok(Some(frame))
    }
}
