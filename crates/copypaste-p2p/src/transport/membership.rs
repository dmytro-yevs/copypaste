//! A separately keyed, membership-only channel for learning changed epochs.
use super::{session::codec, Session, TransportError, HANDSHAKE_TIMEOUT, TOKEN_LEN};
use futures_util::SinkExt;
use std::net::SocketAddr;
use tokio::net::TcpStream;
use tokio_util::{bytes::Bytes, codec::Framed};

pub(super) const PREFIX: &[u8] = b"CPGROUP1";
impl Session {
    pub(crate) async fn connect_membership(
        addr: SocketAddr,
        key: &[u8; TOKEN_LEN],
    ) -> Result<Self, TransportError> {
        tokio::time::timeout(HANDSHAKE_TIMEOUT, async {
            let stream = TcpStream::connect(addr).await.map_err(TransportError::Io)?;
            let _ = stream.set_nodelay(true);
            let peer_addr = stream.peer_addr().unwrap_or(addr);
            let mut framed = Framed::new(stream, codec());
            framed
                .send(Bytes::from_static(PREFIX))
                .await
                .map_err(TransportError::Io)?;
            let mut session = Self::handshake_framed(framed, key, true, peer_addr).await?;
            session.restrict_to_membership();
            Ok(session)
        })
        .await
        .unwrap_or(Err(TransportError::Handshake))
    }
}
