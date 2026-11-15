//! A separately keyed, membership-only channel for learning changed epochs.
use super::{
    conn::{Conn, Endpoint},
    Session, TransportError, HANDSHAKE_TIMEOUT, TOKEN_LEN,
};
use futures_util::SinkExt;
use tokio_util::bytes::Bytes;

pub(super) const PREFIX: &[u8] = b"CPGROUP1";
impl Session {
    pub(crate) async fn connect_membership(
        endpoint: &Endpoint,
        key: &[u8; TOKEN_LEN],
    ) -> Result<Self, TransportError> {
        tokio::time::timeout(HANDSHAKE_TIMEOUT, async {
            let mut framed = Conn::open(endpoint).await?;
            framed
                .frames
                .send(Bytes::from_static(PREFIX))
                .await
                .map_err(TransportError::Io)?;
            let mut session = Self::handshake_framed(framed, key, true).await?;
            session.restrict_to_membership();
            Ok(session)
        })
        .await
        .unwrap_or(Err(TransportError::Handshake))
    }
}
