//! TCP byte streams, for protocols that frame their own messages (HTTP).
//!
//! Real network only: the simulated network carries frames, not bytes, so
//! code on these streams does not run in the simulator yet (P-37).

use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::{TcpListener, TcpStream};

/// A listening TCP socket that yields byte streams.
#[derive(Debug)]
pub struct TcpByteListener {
    listener: TcpListener,
}

impl TcpByteListener {
    /// Listens on `addr`; port 0 picks a free port.
    pub async fn bind(addr: SocketAddr) -> io::Result<Self> {
        Ok(Self {
            listener: TcpListener::bind(addr).await?,
        })
    }

    /// The next connection and the peer address.
    pub async fn accept(&self) -> io::Result<(ByteStream, SocketAddr)> {
        let (stream, peer) = self.listener.accept().await?;
        Ok((ByteStream { stream }, peer))
    }

    /// The address actually bound (with the chosen port).
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.listener.local_addr()
    }
}

/// A TCP connection read and written as bytes.
#[derive(Debug)]
pub struct ByteStream {
    stream: TcpStream,
}

impl ByteStream {
    /// Connects to `addr`.
    pub async fn connect(addr: SocketAddr) -> io::Result<Self> {
        Ok(Self {
            stream: TcpStream::connect(addr).await?,
        })
    }
}

impl AsyncRead for ByteStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().stream).poll_read(cx, buf)
    }
}

impl AsyncWrite for ByteStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.get_mut().stream).poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().stream).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().stream).poll_shutdown(cx)
    }
}
