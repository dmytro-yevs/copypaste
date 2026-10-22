//! Nearby bootstrap is untrusted until both users confirm the bound SAS.
//! This explicit protocol cannot fall through into a synchronization session.

use super::{
    handshake::next_handshake_frame, session::codec, PairingCandidate, PairingToken, Session,
    TransportError, HANDSHAKE_TIMEOUT,
};
use futures_util::SinkExt;
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use snow::Builder;
use std::net::SocketAddr;
use subtle::ConstantTimeEq;
use tokio::net::TcpStream;
use tokio_util::{
    bytes::Bytes,
    codec::{Framed, LengthDelimitedCodec},
};
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

pub(super) const PREFIX: &[u8] = b"CopyPaste-Nearby-v1:";
const PATTERN: &str = "Noise_NN_25519_ChaChaPoly_BLAKE2s";

#[derive(Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
struct Token {
    bytes: [u8; 32],
}

impl Session {
    pub(crate) async fn connect_nearby(
        addr: SocketAddr,
    ) -> Result<(Self, PairingToken), TransportError> {
        tokio::time::timeout(HANDSHAKE_TIMEOUT, async {
            let stream = TcpStream::connect(addr).await.map_err(TransportError::Io)?;
            let _ = stream.set_nodelay(true);
            let mut framed = Framed::new(stream, codec());
            framed
                .send(Bytes::from_static(PREFIX))
                .await
                .map_err(TransportError::Io)?;
            let mut session = handshake(framed, addr, true).await?;
            let token = session
                .recv::<Token>()
                .await?
                .ok_or(TransportError::Handshake)?;
            Ok((session, PairingToken::from_bytes(&token.bytes)))
        })
        .await
        .unwrap_or(Err(TransportError::Handshake))
    }
}

pub(super) async fn accept<F>(
    framed: Framed<TcpStream, LengthDelimitedCodec>,
    addr: SocketAddr,
    reserve: F,
) -> Result<(Session, String), TransportError>
where
    F: FnOnce() -> Option<PairingCandidate>,
{
    let mut session = handshake(framed, addr, false).await?;
    // Reserve only after the bounded cryptographic exchange. The manager
    // permits one ceremony and rate-limits unsolicited requests.
    let candidate = reserve().ok_or(TransportError::Handshake)?;
    session
        .send(&Token {
            bytes: candidate.token.psk,
        })
        .await?;
    Ok((session, candidate.token.pairing_id.clone()))
}

async fn handshake(
    mut framed: Framed<TcpStream, LengthDelimitedCodec>,
    addr: SocketAddr,
    initiator: bool,
) -> Result<Session, TransportError> {
    let params = PATTERN.parse().map_err(|_| TransportError::Handshake)?;
    let builder = Builder::new(params)
        .prologue(PREFIX)
        .map_err(|_| TransportError::Handshake)?;
    let mut state = if initiator {
        builder.build_initiator()
    } else {
        builder.build_responder()
    }
    .map_err(|_| TransportError::Handshake)?;
    let mut buffer = Zeroizing::new([0u8; 128]);
    if initiator {
        let len = state
            .write_message(&[], &mut *buffer)
            .map_err(|_| TransportError::Handshake)?;
        framed
            .send(Bytes::copy_from_slice(&buffer[..len]))
            .await
            .map_err(TransportError::Io)?;
    }
    let incoming = next_handshake_frame(&mut framed).await?;
    state
        .read_message(&incoming, &mut *buffer)
        .map_err(|_| TransportError::Handshake)?;
    if !initiator {
        let len = state
            .write_message(&[], &mut *buffer)
            .map_err(|_| TransportError::Handshake)?;
        framed
            .send(Bytes::copy_from_slice(&buffer[..len]))
            .await
            .map_err(TransportError::Io)?;
    }
    let hash: [u8; 32] = state
        .get_handshake_hash()
        .try_into()
        .map_err(|_| TransportError::Handshake)?;
    let noise = state
        .into_transport_mode()
        .map_err(|_| TransportError::Handshake)?;
    let mut session = Session::new(framed, noise, addr, hash);
    session.restrict_to_pairing();
    // Commit before revealing either nonce. A MITM cannot choose its nonce
    // or handshake after learning the honest nonce to grind matching digits.
    let mut nonce = Zeroizing::new([0u8; 32]);
    OsRng.fill_bytes(&mut *nonce);
    let commitment: [u8; 32] = Sha256::digest(*nonce).into();
    session.send(&commitment).await?;
    let remote_commitment = session
        .recv::<[u8; 32]>()
        .await?
        .ok_or(TransportError::Handshake)?;
    session.send(&*nonce).await?;
    let remote = Zeroizing::new(
        session
            .recv::<[u8; 32]>()
            .await?
            .ok_or(TransportError::Handshake)?,
    );
    let actual: [u8; 32] = Sha256::digest(*remote).into();
    if !bool::from(actual.ct_eq(&remote_commitment)) {
        return Err(TransportError::Handshake);
    }
    let mut sas = Sha256::new();
    sas.update(PREFIX);
    sas.update(hash);
    let (first, second) = if initiator {
        (&*nonce, &*remote)
    } else {
        (&*remote, &*nonce)
    };
    sas.update(first);
    sas.update(second);
    session.bind_pairing_sas(sas.finalize().into());
    Ok(session)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn nearby_sas_is_channel_bound_and_pairing_messages_are_small() {
        let (listener, addr) = super::super::testutil::loopback().await;
        let server = tokio::spawn(async move {
            let (stream, peer) = listener.accept().await.unwrap();
            let mut framed = Framed::new(stream, codec());
            assert_eq!(
                next_handshake_frame(&mut framed).await.unwrap().as_ref(),
                PREFIX
            );
            let mut session = handshake(framed, peer, false).await.unwrap();
            let code = session.pairing_sas();
            assert!(matches!(
                session.send(&"x".repeat(8192)).await,
                Err(TransportError::TooLarge)
            ));
            session.send(&code).await.unwrap();
            code
        });
        let stream = TcpStream::connect(addr).await.unwrap();
        let mut framed = Framed::new(stream, codec());
        framed.send(Bytes::from_static(PREFIX)).await.unwrap();
        let mut session = handshake(framed, addr, true).await.unwrap();
        let remote = session.recv::<String>().await.unwrap().unwrap();
        assert_eq!(session.pairing_sas(), remote);
        assert_eq!(server.await.unwrap(), remote);
    }
}
