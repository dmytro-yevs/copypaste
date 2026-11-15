//! The framed connection a Noise session runs over: a direct TCP stream or a
//! relayed WebSocket. Either way one frame carries one Noise message, so the
//! handshake and record layers never know which one they have.

use std::io;
use std::net::SocketAddr;

use futures_util::{Sink, Stream};
use tokio::net::TcpStream;
use tokio_util::bytes::{Bytes, BytesMut};
use tokio_util::codec::Framed;

use super::session::codec;
use super::TransportError;

/// Frames in, frames out. Implemented by `Framed<TcpStream, _>` and by the
/// relay's WebSocket adapter.
pub(crate) trait FrameIo:
    Stream<Item = io::Result<BytesMut>> + Sink<Bytes, Error = io::Error> + Send + Sync + Unpin
{
}

impl<T> FrameIo for T where
    T: Stream<Item = io::Result<BytesMut>> + Sink<Bytes, Error = io::Error> + Send + Sync + Unpin
{
}

/// Where to open a connection to a peer.
#[derive(Clone)]
pub enum Endpoint {
    Tcp(SocketAddr),
    Relay(crate::relay::RelayRoute),
}

impl std::fmt::Debug for Endpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Tcp(addr) => write!(f, "Tcp({addr})"),
            Self::Relay(_) => f.write_str("Relay"),
        }
    }
}

pub struct Conn {
    pub(super) frames: Box<dyn FrameIo>,
    pub(super) peer_addr: SocketAddr,
    pub(super) local_addr: Option<SocketAddr>,
    pub(super) relayed: bool,
}

impl Conn {
    /// Wrap an accepted or dialled TCP stream.
    ///
    /// # Errors
    ///
    /// When the socket no longer has a peer address.
    pub fn tcp(stream: TcpStream) -> io::Result<Self> {
        // Handshake and sync messages are small and latency-sensitive;
        // Nagle would add up to 40 ms per round trip for nothing.
        let _ = stream.set_nodelay(true);
        let peer_addr = stream.peer_addr()?;
        let local_addr = stream.local_addr().ok();
        Ok(Self {
            frames: Box::new(Framed::new(stream, codec())),
            peer_addr,
            local_addr,
            relayed: false,
        })
    }

    /// A connection spliced through a relay. `relay_addr` is the relay's
    /// socket, which is never a dialable address of the peer.
    pub(crate) fn relayed(frames: Box<dyn FrameIo>, relay_addr: SocketAddr) -> Self {
        Self {
            frames,
            peer_addr: relay_addr,
            local_addr: None,
            relayed: true,
        }
    }

    #[must_use]
    pub fn peer_addr(&self) -> SocketAddr {
        self.peer_addr
    }

    #[must_use]
    pub fn is_relayed(&self) -> bool {
        self.relayed
    }

    pub(super) async fn open(endpoint: &Endpoint) -> Result<Self, TransportError> {
        match endpoint {
            Endpoint::Tcp(addr) => {
                let stream = TcpStream::connect(addr).await.map_err(TransportError::Io)?;
                Self::tcp(stream).map_err(TransportError::Io)
            }
            Endpoint::Relay(route) => route.open().await,
        }
    }
}
