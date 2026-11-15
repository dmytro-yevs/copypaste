//! Authenticate a short invitation with SPAKE2 before transferring a full PSK.
//! Noise confirms the PAKE key and binds the encrypted token to this channel.

use futures_util::SinkExt;
use serde::{Deserialize, Serialize};
use spake2::{Ed25519Group, Identity, Password, Spake2};
use tokio::time::timeout;
use tokio_util::bytes::{Bytes, BytesMut};
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

use super::{
    conn::{Conn, Endpoint},
    handshake::next_handshake_frame,
    PairingCode, PairingToken, PskCandidate, Session, TransportError, HANDSHAKE_TIMEOUT, TOKEN_LEN,
};

pub(super) const PREFIX: &[u8] = b"CopyPaste-PAKE-v1:";
const CLIENT: &[u8] = b"copypaste/pairing/v1/initiator";
const SERVER: &[u8] = b"copypaste/pairing/v1/responder";

pub(crate) struct PairingCandidate {
    pub code: PairingCode,
    pub token: PskCandidate,
}

#[derive(Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
struct BootstrapToken {
    token: [u8; TOKEN_LEN],
}

impl Session {
    pub(crate) async fn connect_pairing(
        endpoint: &Endpoint,
        code: &PairingCode,
    ) -> Result<(Self, PairingToken), TransportError> {
        timeout(HANDSHAKE_TIMEOUT, async {
            let mut framed = Conn::open(endpoint).await?;
            let (state, outgoing) = Spake2::<Ed25519Group>::start_a(
                &Password::new(code.password()),
                &Identity::new(CLIENT),
                &Identity::new(SERVER),
            );
            framed
                .frames
                .send(message(&outgoing))
                .await
                .map_err(TransportError::Io)?;
            let incoming = next_handshake_frame(&mut framed).await?;
            let key = Zeroizing::new(
                state
                    .finish(payload(&incoming)?)
                    .map_err(|_| TransportError::Handshake)?,
            );
            let psk: &[u8; TOKEN_LEN] = key
                .as_slice()
                .try_into()
                .map_err(|_| TransportError::Handshake)?;
            let mut session = Self::handshake_framed(framed, psk, true).await?;
            let bootstrap = session
                .recv::<BootstrapToken>()
                .await?
                .ok_or(TransportError::Handshake)?;
            Ok((session, PairingToken::from_bytes(&bootstrap.token)))
        })
        .await
        .unwrap_or(Err(TransportError::Handshake))
    }
}

pub(super) async fn accept(
    mut framed: Conn,
    incoming: &BytesMut,
    candidate: &PairingCandidate,
) -> Result<(Session, String), TransportError> {
    let (state, outgoing) = Spake2::<Ed25519Group>::start_b(
        &Password::new(candidate.code.password()),
        &Identity::new(CLIENT),
        &Identity::new(SERVER),
    );
    let key = Zeroizing::new(
        state
            .finish(payload(incoming)?)
            .map_err(|_| TransportError::Handshake)?,
    );
    framed
        .frames
        .send(message(&outgoing))
        .await
        .map_err(TransportError::Io)?;
    let psk: &[u8; TOKEN_LEN] = key
        .as_slice()
        .try_into()
        .map_err(|_| TransportError::Handshake)?;
    let mut session = Session::handshake_framed(framed, psk, false).await?;
    session
        .send(&BootstrapToken {
            token: candidate.token.psk,
        })
        .await?;
    Ok((session, candidate.token.pairing_id.clone()))
}

fn message(payload: &[u8]) -> Bytes {
    let mut message = Vec::with_capacity(PREFIX.len() + payload.len());
    message.extend_from_slice(PREFIX);
    message.extend_from_slice(payload);
    Bytes::from(message)
}

fn payload(message: &[u8]) -> Result<&[u8], TransportError> {
    let payload = message
        .strip_prefix(PREFIX)
        .ok_or(TransportError::Handshake)?;
    if payload.len() != 33 {
        return Err(TransportError::Handshake);
    }
    Ok(payload)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::testutil::loopback;

    #[tokio::test]
    async fn a_short_code_transfers_the_full_random_token_inside_the_bound_channel() {
        let (listener, addr) = loopback().await;
        let code = PairingCode::generate();
        let client_code = code.copy_secret();
        let token = PairingToken::generate();
        let expected = token.pairing_id();
        let candidate = PairingCandidate {
            code,
            token: PskCandidate {
                pairing_id: expected.clone(),
                psk: token.psk(),
            },
        };
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let (session, id) =
                Session::accept_with_pairing(Conn::tcp(stream).unwrap(), &[], Some(&candidate))
                    .await
                    .unwrap();
            (id, session.pairing_sas())
        });
        let (session, received) = Session::connect_pairing(&Endpoint::Tcp(addr), &client_code)
            .await
            .unwrap();
        let (id, sas) = server.await.unwrap();
        assert_eq!(id, expected);
        assert_eq!(received, token);
        assert_eq!(session.pairing_sas(), sas);
    }

    #[tokio::test]
    async fn a_wrong_short_code_never_yields_a_session_or_token() {
        let (listener, addr) = loopback().await;
        let candidate = PairingCandidate {
            code: PairingCode::parse("12345678").unwrap(),
            token: PskCandidate {
                pairing_id: "invite".into(),
                psk: PairingToken::generate().psk(),
            },
        };
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            Session::accept_with_pairing(Conn::tcp(stream).unwrap(), &[], Some(&candidate))
                .await
                .map(|_| ())
        });
        let wrong = PairingCode::parse("87654321").unwrap();
        assert!(Session::connect_pairing(&Endpoint::Tcp(addr), &wrong)
            .await
            .is_err());
        assert!(matches!(
            server.await.unwrap(),
            Err(TransportError::Handshake)
        ));
    }
}
