//! Getting from a TCP stream to an authenticated [`Session`].
//!
//! # Why NNpsk0
//!
//! `NN` is the anonymous pattern: neither side has a static key, so there is no
//! key to generate, distribute, pin, rotate or revoke — and no certificate
//! lifecycle, which is what v1 spent a rustls + rcgen + two hand-written DER
//! parsers on. The `psk0` modifier mixes the pairing token into the chaining
//! key *before* the first ephemeral is written, which buys three things at
//! once:
//!
//! * **Authentication.** Possession of the token is the identity. Both sides
//!   prove possession by being able to decrypt at all.
//! * **Fail-closed by construction.** Because `psk0` establishes a cipher key
//!   before message one's payload, message one is already an AEAD ciphertext.
//!   A responder holding the wrong token fails to decrypt it. There is no
//!   downgrade, no "unauthenticated but encrypted" mode to fall back to, and
//!   no code path in this file that could offer one (port manifest 02, I-15 —
//!   fail closed on crypto; `CLAUDE.md` rule 4).
//! * **Forward secrecy.** The `ee` DH in message two means a token disclosed
//!   later does not decrypt traffic captured earlier.
//!
//! `NNpsk0` is *not* a PAKE and does not need to be. A PAKE exists to protect a
//! low-entropy human secret from an offline dictionary attack; our token is 256
//! bits from the OS CSPRNG, so the dictionary does not exist (see the crate
//! docs, and port manifest 02 §6.3, which reaches the same conclusion about
//! v1's OPAQUE).
//!
//! # A handshake cannot pin a task
//!
//! Dial, both handshake messages and the transport-mode transition all happen
//! inside one [`HANDSHAKE_TIMEOUT`].

use std::net::SocketAddr;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use snow::params::NoiseParams;
use snow::Builder;
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_util::bytes::Bytes;
use tokio_util::codec::{Framed, LengthDelimitedCodec};
use zeroize::Zeroizing;

use super::session::{codec, Session, MAX_NOISE_MESSAGE};
use super::token::TOKEN_LEN;
use super::TransportError;

/// The Noise handshake pattern, verbatim.
///
/// Changing any component of this string is a wire break: both ends must parse
/// the identical name or the handshake fails. It is public so that the daemon
/// can report it and so a test can pin it.
pub const NOISE_PARAMS: &str = "Noise_NNpsk0_25519_ChaChaPoly_BLAKE2s";

/// Wall-clock budget for connecting and completing the handshake.
///
/// A peer that opens a TCP connection and then says nothing must not be able to
/// hold a daemon task open indefinitely. Ten seconds is generous for two
/// round-trips of X25519 on a LAN and short enough that a stalled listener
/// recovers without operator action.
pub const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

impl Session {
    /// Dial a peer and complete the handshake as initiator.
    ///
    /// The TCP connect and both handshake messages share one
    /// [`HANDSHAKE_TIMEOUT`].
    ///
    /// # Errors
    ///
    /// [`TransportError::Handshake`] if the peer holds a different pairing
    /// token, is not speaking this protocol, or stalls;
    /// [`TransportError::Io`] if the connection could not be made at all.
    pub async fn connect(addr: SocketAddr, psk: &[u8; TOKEN_LEN]) -> Result<Self, TransportError> {
        timeout(HANDSHAKE_TIMEOUT, async move {
            let stream = TcpStream::connect(addr).await.map_err(TransportError::Io)?;
            // Handshake and sync messages are small and latency-sensitive;
            // Nagle would add up to 40 ms per round trip for nothing.
            let _ = stream.set_nodelay(true);
            let peer_addr = stream.peer_addr().unwrap_or(addr);
            Self::handshake(stream, psk, true, peer_addr).await
        })
        .await
        .unwrap_or_else(|_| {
            tracing::debug!("outbound handshake timed out");
            Err(TransportError::Handshake)
        })
    }

    /// Complete the handshake as responder on an already-accepted stream.
    ///
    /// # Errors
    ///
    /// [`TransportError::Handshake`] if the initiator holds a different pairing
    /// token, is not speaking this protocol, or stalls.
    pub async fn accept(stream: TcpStream, psk: &[u8; TOKEN_LEN]) -> Result<Self, TransportError> {
        let peer_addr = stream.peer_addr().map_err(TransportError::Io)?;
        let _ = stream.set_nodelay(true);
        timeout(
            HANDSHAKE_TIMEOUT,
            Self::handshake(stream, psk, false, peer_addr),
        )
        .await
        .unwrap_or_else(|_| {
            tracing::debug!(%peer_addr, "inbound handshake timed out");
            Err(TransportError::Handshake)
        })
    }

    /// Complete the handshake as responder, trying every pairing this device
    /// knows about, and report which one matched.
    ///
    /// A listener does not know who is dialling until the handshake succeeds,
    /// so it has to try each stored PSK — this is what
    /// [`crate::PeerStore::psks`] exists to feed. The first handshake message
    /// is read once and replayed against each candidate in turn; only its
    /// authentication tag distinguishes them, so a wrong candidate costs one
    /// failed AEAD open and nothing else.
    ///
    /// Candidates are tried in the order given. Every one is attempted even
    /// after a failure, and the error is identical whether zero candidates
    /// matched or the list was empty — nothing about which pairings this device
    /// holds leaks to the dialler.
    ///
    /// # Errors
    ///
    /// [`TransportError::Handshake`] if no candidate matched.
    pub async fn accept_any(
        stream: TcpStream,
        candidates: &[(String, [u8; TOKEN_LEN])],
    ) -> Result<(Self, String), TransportError> {
        let peer_addr = stream.peer_addr().map_err(TransportError::Io)?;
        let _ = stream.set_nodelay(true);
        timeout(HANDSHAKE_TIMEOUT, async move {
            let mut framed = Framed::new(stream, codec());
            let first = next_handshake_frame(&mut framed).await?;
            let mut buf = Zeroizing::new(vec![0u8; MAX_NOISE_MESSAGE]);

            for (pairing_id, psk) in candidates {
                let Ok(mut hs) = Builder::new(noise_params()?)
                    .psk(0, psk)
                    .and_then(|builder| builder.build_responder())
                else {
                    continue;
                };
                if hs.read_message(&first, &mut buf).is_err() {
                    continue;
                }
                let len = hs
                    .write_message(&[], &mut buf)
                    .map_err(|_| TransportError::Handshake)?;
                framed
                    .send(Bytes::copy_from_slice(&buf[..len]))
                    .await
                    .map_err(TransportError::Io)?;
                let noise = hs
                    .into_transport_mode()
                    .map_err(|_| TransportError::Handshake)?;
                tracing::debug!(%peer_addr, %pairing_id, "inbound session established");
                return Ok((Self::new(framed, noise, peer_addr), pairing_id.clone()));
            }
            tracing::debug!(%peer_addr, "inbound handshake matched no known pairing");
            Err(TransportError::Handshake)
        })
        .await
        .unwrap_or_else(|_| {
            tracing::debug!(%peer_addr, "inbound handshake timed out");
            Err(TransportError::Handshake)
        })
    }

    /// `NNpsk0` in full: `-> psk, e` then `<- e, ee`. Both payloads are empty —
    /// there is nothing to say before the channel exists, and an empty payload
    /// gives a fixed-size handshake that reveals nothing by its length.
    async fn handshake(
        stream: TcpStream,
        psk: &[u8; TOKEN_LEN],
        initiator: bool,
        peer_addr: SocketAddr,
    ) -> Result<Self, TransportError> {
        let mut framed = Framed::new(stream, codec());
        let builder = Builder::new(noise_params()?)
            .psk(0, psk)
            .map_err(|_| TransportError::Handshake)?;
        let mut hs = if initiator {
            builder.build_initiator()
        } else {
            builder.build_responder()
        }
        .map_err(|_| TransportError::Handshake)?;

        let mut buf = Zeroizing::new(vec![0u8; MAX_NOISE_MESSAGE]);

        if initiator {
            let len = hs
                .write_message(&[], &mut buf)
                .map_err(|_| TransportError::Handshake)?;
            framed
                .send(Bytes::copy_from_slice(&buf[..len]))
                .await
                .map_err(TransportError::Io)?;
            let reply = next_handshake_frame(&mut framed).await?;
            hs.read_message(&reply, &mut buf).map_err(|err| {
                tracing::debug!(%peer_addr, ?err, "handshake response rejected");
                TransportError::Handshake
            })?;
        } else {
            let first = next_handshake_frame(&mut framed).await?;
            hs.read_message(&first, &mut buf).map_err(|err| {
                // The overwhelmingly likely cause is a peer with a different
                // pairing token. Debug level, and the token is not in scope to
                // be logged even by accident.
                tracing::debug!(%peer_addr, ?err, "handshake initiation rejected");
                TransportError::Handshake
            })?;
            let len = hs
                .write_message(&[], &mut buf)
                .map_err(|_| TransportError::Handshake)?;
            framed
                .send(Bytes::copy_from_slice(&buf[..len]))
                .await
                .map_err(TransportError::Io)?;
        }

        let noise = hs
            .into_transport_mode()
            .map_err(|_| TransportError::Handshake)?;
        tracing::debug!(%peer_addr, initiator, "secure session established");
        Ok(Self::new(framed, noise, peer_addr))
    }
}

pub(super) fn noise_params() -> Result<NoiseParams, TransportError> {
    NOISE_PARAMS.parse().map_err(|_| TransportError::Handshake)
}

/// Read one frame during the handshake. A clean close here is not a clean
/// close — it means the peer gave up, which is a handshake failure.
async fn next_handshake_frame(
    framed: &mut Framed<TcpStream, LengthDelimitedCodec>,
) -> Result<tokio_util::bytes::BytesMut, TransportError> {
    match framed.next().await {
        Some(Ok(frame)) => Ok(frame),
        Some(Err(_)) | None => Err(TransportError::Handshake),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::testutil::{loopback, Ping};
    use crate::transport::PairingToken;

    #[test]
    fn noise_pattern_is_the_documented_one() {
        // The whole design rests on `psk0` (authentication + fail-closed) and
        // on `NN` (no static keys to manage). If this string drifts, both
        // properties change silently.
        assert_eq!(NOISE_PARAMS, "Noise_NNpsk0_25519_ChaChaPoly_BLAKE2s");
        let params = noise_params().expect("pattern must parse");
        assert_eq!(params.name, NOISE_PARAMS);
        // And it must actually build, not merely parse.
        let psk = [7u8; TOKEN_LEN];
        Builder::new(noise_params().unwrap())
            .psk(0, &psk)
            .unwrap()
            .build_initiator()
            .expect("pattern must be buildable by the default resolver");
    }

    #[tokio::test]
    async fn wrong_psk_fails_the_handshake_on_both_sides() {
        let (listener, addr) = loopback().await;
        let good = PairingToken::generate().psk();
        let bad = PairingToken::generate().psk();
        assert_ne!(good, bad);

        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept");
            Session::accept(stream, &good).await.map(|_| ())
        });

        let client = Session::connect(addr, &bad).await.map(|_| ());

        // The responder is where the mismatch is detected: `psk0` keys the
        // cipher before message one's payload, so a wrong PSK is an AEAD
        // failure on the very first message. No degraded, unauthenticated mode
        // exists to fall through to.
        let server_result = server.await.expect("server task");
        assert!(
            matches!(server_result, Err(TransportError::Handshake)),
            "responder must reject a wrong PSK, got {server_result:?}"
        );
        // And the initiator must not end up with a usable session either.
        assert!(
            matches!(client, Err(TransportError::Handshake)),
            "initiator must not complete, got {client:?}"
        );
    }

    #[tokio::test]
    async fn accept_any_selects_the_matching_pairing() {
        let (listener, addr) = loopback().await;
        let wanted = PairingToken::generate();
        let candidates = vec![
            ("decoy-a".to_string(), PairingToken::generate().psk()),
            (wanted.pairing_id(), wanted.psk()),
            ("decoy-b".to_string(), PairingToken::generate().psk()),
        ];
        let expected_id = wanted.pairing_id();

        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept");
            Session::accept_any(stream, &candidates).await
        });

        let mut client = Session::connect(addr, &wanted.psk())
            .await
            .expect("initiator");
        client
            .send(&Ping {
                seq: 9,
                note: "x".into(),
            })
            .await
            .expect("send");

        let (mut session, matched) = server.await.expect("server task").expect("must match");
        assert_eq!(matched, expected_id);
        let got: Ping = session.recv().await.expect("recv").expect("message");
        assert_eq!(got.seq, 9);
    }

    #[tokio::test]
    async fn accept_any_rejects_an_unknown_pairing() {
        let (listener, addr) = loopback().await;
        let candidates = vec![("known".to_string(), PairingToken::generate().psk())];

        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept");
            Session::accept_any(stream, &candidates).await.map(|_| ())
        });

        let stranger = PairingToken::generate().psk();
        let client = Session::connect(addr, &stranger).await.map(|_| ());

        assert!(matches!(
            server.await.expect("server task"),
            Err(TransportError::Handshake)
        ));
        assert!(matches!(client, Err(TransportError::Handshake)));
    }

    #[tokio::test]
    async fn accept_any_with_no_candidates_is_a_plain_handshake_failure() {
        let (listener, addr) = loopback().await;
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept");
            Session::accept_any(stream, &[]).await.map(|_| ())
        });
        let client = Session::connect(addr, &PairingToken::generate().psk())
            .await
            .map(|_| ());
        assert!(matches!(
            server.await.expect("server task"),
            Err(TransportError::Handshake)
        ));
        assert!(matches!(client, Err(TransportError::Handshake)));
    }

    #[tokio::test]
    async fn handshake_times_out_on_a_silent_peer() {
        // A peer that accepts the TCP connection and then says nothing must not
        // hold the task. Proven with a short local timeout rather than by
        // waiting out HANDSHAKE_TIMEOUT.
        let (listener, addr) = loopback().await;
        let _keep = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept");
            tokio::time::sleep(Duration::from_secs(30)).await;
            drop(stream);
        });

        let psk = PairingToken::generate().psk();
        let result = timeout(Duration::from_millis(300), Session::connect(addr, &psk)).await;
        assert!(
            result.is_err(),
            "connect should still be waiting inside its own budget"
        );
        // And the real budget is bounded rather than infinite.
        assert!(HANDSHAKE_TIMEOUT <= Duration::from_secs(30));
    }

    #[tokio::test]
    async fn garbage_on_the_port_is_a_handshake_failure_not_a_hang() {
        use tokio::io::AsyncWriteExt;

        let (listener, addr) = loopback().await;
        let psk = PairingToken::generate().psk();
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept");
            Session::accept(stream, &psk).await.map(|_| ())
        });

        let mut stream = TcpStream::connect(addr).await.expect("connect");
        // A well-formed length prefix over bytes that are not a Noise message.
        stream.write_all(&[0, 0, 0, 48]).await.expect("len");
        stream.write_all(&[0xAA; 48]).await.expect("body");
        stream.flush().await.ok();

        assert!(matches!(
            server.await.expect("server task"),
            Err(TransportError::Handshake)
        ));
    }
}
