//! The established channel: framing, chunking, poisoning.
//!
//! # Framing
//!
//! Two layers, neither hand-rolled:
//!
//! 1. [`tokio_util::codec::LengthDelimitedCodec`] puts a 4-byte big-endian
//!    length in front of every Noise message and enforces
//!    [`MAX_NOISE_MESSAGE`] on the way in, so an oversized declaration is
//!    rejected before anything is allocated for it.
//! 2. Noise messages are capped at 65535 bytes by the specification, which is
//!    smaller than a clipboard image. A logical message is therefore split
//!    across Noise messages, each carrying a one-byte record marker
//!    (`RECORD_MORE` / `RECORD_FINAL`) *inside* the AEAD, so the marker is
//!    authenticated and an attacker cannot truncate a message by dropping the
//!    final record — the receiver would see the stream end mid-message and
//!    return [`TransportError::Malformed`].
//!
//! That one byte is the only framing invented here. Reassembly is bounded by
//! [`MAX_MESSAGE_BYTES`]; past it the peer gets [`TransportError::TooLarge`]
//! rather than an unbounded allocation.

use std::fmt;
use std::net::SocketAddr;

use futures_util::{SinkExt, StreamExt};
use snow::TransportState;
use tokio::net::TcpStream;
use tokio_util::bytes::Bytes;
use tokio_util::codec::{Framed, LengthDelimitedCodec};
use zeroize::Zeroizing;

use super::TransportError;

/// Poly1305 tag length, appended to every Noise message by the AEAD.
const NOISE_TAG_LEN: usize = 16;

/// Maximum size of a single Noise message, fixed by the Noise specification.
pub const MAX_NOISE_MESSAGE: usize = 65535;

/// Maximum plaintext that fits in one Noise message.
const MAX_NOISE_PLAINTEXT: usize = MAX_NOISE_MESSAGE - NOISE_TAG_LEN;

/// One byte of record marker precedes the payload inside each Noise message.
const RECORD_HEADER_LEN: usize = 1;

/// Payload bytes carried per Noise message.
const MAX_RECORD_PAYLOAD: usize = MAX_NOISE_PLAINTEXT - RECORD_HEADER_LEN;

/// Record marker: more records follow, this logical message is not complete.
const RECORD_MORE: u8 = 0x01;

/// Record marker: last record of this logical message.
const RECORD_FINAL: u8 = 0x02;

/// Largest logical message this channel will send or reassemble.
///
/// A resource guard, not a front-line defence: the peer is authenticated by the
/// time this matters, since an unpaired attacker never gets past the handshake.
/// It bounds what a single `recv` can allocate for a buggy or hostile *paired*
/// device.
///
/// Deliberately equal to [`crate::protocol::MAX_MESSAGE_BYTES`], and it must
/// never be smaller. The protocol layer owns message-size policy — it caps one
/// item at 4 MiB and allows 8× that for JSON escape inflation. If the transport
/// were the tighter limit, a legitimate message full of control characters
/// would be refused down here, with a transport error, instead of by the layer
/// that decided the budget. The two constants are kept separate rather than
/// aliased because the crate docs make these modules independently
/// substitutable; a test pins them together so the pair cannot drift silently.
pub const MAX_MESSAGE_BYTES: usize = 32 * 1024 * 1024;

/// An established, encrypted, message-oriented channel with one peer.
///
/// Obtained from [`Session::connect`] or [`Session::accept`]; both return only
/// after the handshake has completed, so a `Session` that exists is a session
/// that authenticated.
///
/// Not `Clone` and not shareable: [`Session::send`] and [`Session::recv`] take
/// `&mut self` because the Noise transport state carries a per-direction nonce
/// counter that must advance in lockstep with the wire. Give each connection
/// its own task.
pub struct Session {
    framed: Framed<TcpStream, LengthDelimitedCodec>,
    noise: TransportState,
    peer_addr: SocketAddr,
    /// Reused plaintext staging buffer, allocated once at
    /// [`MAX_NOISE_PLAINTEXT`] so it never reallocates — a growing `Vec` leaves
    /// copies of old plaintext in freed heap that nothing will ever zeroize.
    plain: Zeroizing<Vec<u8>>,
    /// Reused ciphertext staging buffer. Not secret, but sized once for the
    /// same reason.
    cipher: Vec<u8>,
    /// Set after any authentication or framing failure. The nonce sequence is
    /// no longer trustworthy at that point, so every later call fails fast
    /// instead of emitting garbage that looks like corruption.
    poisoned: bool,
}

impl Session {
    /// Send one logical message, JSON-encoded, split across as many Noise
    /// messages as it needs.
    ///
    /// # Errors
    ///
    /// [`TransportError::TooLarge`] if the encoded message exceeds
    /// [`MAX_MESSAGE_BYTES`] — nothing is written in that case, so the channel
    /// stays usable; [`TransportError::Codec`] if `msg` does not serialise;
    /// [`TransportError::Io`] if the socket fails.
    pub async fn send<T: serde::Serialize>(&mut self, msg: &T) -> Result<(), TransportError> {
        if self.poisoned {
            return Err(TransportError::Malformed);
        }
        let payload = Zeroizing::new(serde_json::to_vec(msg).map_err(|_| TransportError::Codec)?);
        if payload.len() > MAX_MESSAGE_BYTES {
            return Err(TransportError::TooLarge);
        }

        // `chunks` yields nothing for an empty slice, and an empty logical
        // message still has to produce one record or the peer's `recv` would
        // block. `serde_json` never emits zero bytes for a valid value, so this
        // is belt and braces rather than a live path.
        let mut records = payload.chunks(MAX_RECORD_PAYLOAD).peekable();
        if records.peek().is_none() {
            return self.write_record(RECORD_FINAL, &[]).await;
        }
        while let Some(chunk) = records.next() {
            let marker = if records.peek().is_some() {
                RECORD_MORE
            } else {
                RECORD_FINAL
            };
            self.write_record(marker, chunk).await?;
        }
        Ok(())
    }

    /// Receive one logical message.
    ///
    /// Returns `Ok(None)` on a clean close — the peer hung up at a message
    /// boundary, which is how a session ends normally. A close *inside* a
    /// message is [`TransportError::Malformed`], not `None`: truncation must
    /// never be mistaken for a well-formed short message.
    ///
    /// # Errors
    ///
    /// [`TransportError::Malformed`] on an authentication failure, an unknown
    /// record marker or a truncated message — the session is poisoned and must
    /// be dropped. [`TransportError::TooLarge`] if reassembly would exceed
    /// [`MAX_MESSAGE_BYTES`]. [`TransportError::Codec`] if the authenticated
    /// bytes did not parse as `T`, which leaves the session usable.
    pub async fn recv<T: serde::de::DeserializeOwned>(
        &mut self,
    ) -> Result<Option<T>, TransportError> {
        if self.poisoned {
            return Err(TransportError::Malformed);
        }
        let mut message: Zeroizing<Vec<u8>> = Zeroizing::new(Vec::new());
        let mut started = false;

        loop {
            let frame = match self.framed.next().await {
                None => {
                    if started {
                        // FIN arrived between records. Someone truncated the
                        // message — either the peer crashed or an attacker cut
                        // the connection. Either way it is not an empty message.
                        self.poisoned = true;
                        return Err(TransportError::Malformed);
                    }
                    return Ok(None);
                }
                Some(Ok(frame)) => frame,
                Some(Err(err)) => {
                    self.poisoned = true;
                    return Err(TransportError::Io(err));
                }
            };

            // Smallest legal Noise message here is marker + tag. Rejecting
            // short frames up front keeps `read_message` off obvious garbage.
            if frame.len() < RECORD_HEADER_LEN + NOISE_TAG_LEN || frame.len() > MAX_NOISE_MESSAGE {
                self.poisoned = true;
                return Err(TransportError::Malformed);
            }

            self.plain.resize(MAX_NOISE_PLAINTEXT, 0);
            let opened = self.noise.read_message(&frame, &mut self.plain);
            let len = match opened {
                Ok(len) => len,
                Err(_) => {
                    // Wrong key, tampering, replay, or a desynchronised nonce.
                    // Which one is deliberately not distinguishable (port
                    // manifest 02, I-15).
                    self.poisoned = true;
                    return Err(TransportError::Malformed);
                }
            };
            if len < RECORD_HEADER_LEN {
                self.poisoned = true;
                return Err(TransportError::Malformed);
            }

            let marker = self.plain[0];
            let body = &self.plain[RECORD_HEADER_LEN..len];
            if message.len() + body.len() > MAX_MESSAGE_BYTES {
                self.poisoned = true;
                return Err(TransportError::TooLarge);
            }
            message.extend_from_slice(body);
            started = true;

            match marker {
                RECORD_MORE => continue,
                RECORD_FINAL => break,
                _ => {
                    self.poisoned = true;
                    return Err(TransportError::Malformed);
                }
            }
        }

        serde_json::from_slice(&message)
            .map(Some)
            .map_err(|_| TransportError::Codec)
    }

    /// The peer's address, as the socket reports it.
    #[must_use]
    pub fn peer_addr(&self) -> SocketAddr {
        self.peer_addr
    }

    /// Whether an authentication or framing failure has made this session
    /// unusable. A poisoned session returns [`TransportError::Malformed`] from
    /// every call; reconnect instead.
    #[must_use]
    pub fn is_poisoned(&self) -> bool {
        self.poisoned
    }

    /// Flush and shut the write half down, so the peer's next `recv` returns
    /// `Ok(None)` rather than an abrupt reset.
    ///
    /// # Errors
    ///
    /// [`TransportError::Io`] if the socket could not be shut down.
    pub async fn close(mut self) -> Result<(), TransportError> {
        self.framed.close().await.map_err(TransportError::Io)
    }

    // -- internals ----------------------------------------------------------

    /// Wrap a completed handshake. Called only by [`super::handshake`], which
    /// is the sole way a `Session` comes into existence.
    pub(super) fn new(
        framed: Framed<TcpStream, LengthDelimitedCodec>,
        noise: TransportState,
        peer_addr: SocketAddr,
    ) -> Self {
        Self {
            framed,
            noise,
            peer_addr,
            plain: Zeroizing::new(vec![0u8; MAX_NOISE_PLAINTEXT]),
            cipher: vec![0u8; MAX_NOISE_MESSAGE],
            poisoned: false,
        }
    }

    async fn write_record(&mut self, marker: u8, body: &[u8]) -> Result<(), TransportError> {
        debug_assert!(body.len() <= MAX_RECORD_PAYLOAD);
        self.plain.clear();
        self.plain.push(marker);
        self.plain.extend_from_slice(body);
        self.cipher.resize(MAX_NOISE_MESSAGE, 0);

        let sealed = self.noise.write_message(&self.plain, &mut self.cipher);
        let len = match sealed {
            Ok(len) => len,
            Err(_) => {
                // Reachable only if the nonce sequence is exhausted (2^64
                // messages) or the buffer sizing above is wrong. Never from
                // peer input.
                self.poisoned = true;
                return Err(TransportError::TooLarge);
            }
        };
        self.framed
            .send(Bytes::copy_from_slice(&self.cipher[..len]))
            .await
            .map_err(TransportError::Io)
    }
}

/// Prints the peer address and nothing else. No key material, no buffer
/// contents, and no path.
impl fmt::Debug for Session {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Session")
            .field("peer_addr", &self.peer_addr)
            .field("poisoned", &self.poisoned)
            .finish_non_exhaustive()
    }
}

/// One codec configuration, used by both ends and by both phases.
///
/// `max_frame_length` is the Noise message limit, so a peer that declares a
/// larger frame is cut off by the codec before a buffer is reserved for it.
pub(super) fn codec() -> LengthDelimitedCodec {
    LengthDelimitedCodec::builder()
        .length_field_length(4)
        .big_endian()
        .max_frame_length(MAX_NOISE_MESSAGE)
        .new_codec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::handshake::noise_params;
    use crate::transport::testutil::{assert_no_secret, loopback, Ping};
    use crate::transport::PairingToken;
    use snow::Builder;
    use std::time::Duration;

    /// The protocol layer owns message-size policy. If this transport were the
    /// narrower of the two, a message the protocol considers legal would be
    /// refused down here, and the caller would get a transport error for a
    /// decision made a layer up.
    ///
    /// A compile-time guard rather than a runtime assertion — the same shape as
    /// port manifest 02 I-27's constant guards — so drift between the two
    /// numbers is a build failure with the reason attached. It lives inside the
    /// test module so the shipped `transport` stays free of any reference to
    /// `protocol`; the crate docs describe the two as independently
    /// substitutable, and that should remain true of the artifact even though
    /// the numbers have to agree.
    const _: () = assert!(
        MAX_MESSAGE_BYTES >= crate::protocol::MAX_MESSAGE_BYTES,
        "the transport must never impose a tighter message limit than the protocol"
    );

    #[tokio::test]
    async fn round_trip_over_loopback_in_both_directions() {
        let (listener, addr) = loopback().await;
        let psk = PairingToken::generate().psk();

        let server_psk = psk;
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept");
            let mut session = Session::accept(stream, &server_psk)
                .await
                .expect("responder");
            let got: Ping = session
                .recv()
                .await
                .expect("recv ok")
                .expect("a message, not a close");
            assert_eq!(
                got,
                Ping {
                    seq: 1,
                    note: "hello".into()
                }
            );
            session
                .send(&Ping {
                    seq: 2,
                    note: "hi back".into(),
                })
                .await
                .expect("send back");
            // Closing cleanly is what makes the client's second recv return
            // Ok(None) rather than an error.
            session.close().await.expect("clean close");
        });

        let mut client = Session::connect(addr, &psk).await.expect("initiator");
        assert_eq!(client.peer_addr().ip(), addr.ip());
        client
            .send(&Ping {
                seq: 1,
                note: "hello".into(),
            })
            .await
            .expect("send");
        let reply: Ping = client.recv().await.expect("recv ok").expect("a message");
        assert_eq!(
            reply,
            Ping {
                seq: 2,
                note: "hi back".into()
            }
        );
        // Clean close at a message boundary is `Ok(None)`, not an error.
        let end: Option<Ping> = client.recv().await.expect("clean close is not an error");
        assert!(end.is_none());

        server.await.expect("server task");
    }

    #[tokio::test]
    async fn a_message_larger_than_one_noise_frame_round_trips() {
        // The point of the record marker: 400 KiB is roughly seven Noise
        // messages, and it must arrive as one logical message.
        let (listener, addr) = loopback().await;
        let psk = PairingToken::generate().psk();
        let big = "z".repeat(400 * 1024);
        let expected = big.clone();

        let server_psk = psk;
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept");
            let mut session = Session::accept(stream, &server_psk)
                .await
                .expect("responder");
            let got: String = session.recv().await.expect("recv").expect("message");
            assert_eq!(got.len(), expected.len());
            assert_eq!(got, expected);
            // Echo it back so both directions are exercised at size.
            session.send(&got).await.expect("echo");
            session.close().await.expect("close");
        });

        let mut client = Session::connect(addr, &psk).await.expect("initiator");
        client.send(&big).await.expect("send big");
        let echoed: String = client.recv().await.expect("recv").expect("message");
        assert_eq!(echoed, big);
        server.await.expect("server task");
    }

    #[tokio::test]
    async fn oversized_message_is_rejected_without_touching_the_wire() {
        let (listener, addr) = loopback().await;
        let psk = PairingToken::generate().psk();

        let server_psk = psk;
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept");
            let mut session = Session::accept(stream, &server_psk)
                .await
                .expect("responder");
            // The oversized send must never reach here; the small one must.
            let got: String = session.recv().await.expect("recv").expect("message");
            assert_eq!(got, "still fine");
        });

        let mut client = Session::connect(addr, &psk).await.expect("initiator");
        // JSON of a String is the string plus two quotes, so this is just over
        // the cap.
        let huge = "q".repeat(MAX_MESSAGE_BYTES);
        assert!(
            matches!(client.send(&huge).await, Err(TransportError::TooLarge)),
            "must refuse rather than truncate"
        );
        assert!(
            !client.is_poisoned(),
            "refusing to send must not break the session"
        );
        // Nothing was written, so the channel is still in step.
        client.send(&"still fine".to_string()).await.expect("send");
        server.await.expect("server task");
    }

    #[tokio::test]
    async fn a_message_at_the_limit_is_accepted() {
        let (listener, addr) = loopback().await;
        let psk = PairingToken::generate().psk();
        // MAX_MESSAGE_BYTES exactly, once JSON quoting is accounted for.
        let at_limit = "w".repeat(MAX_MESSAGE_BYTES - 2);
        let expected_len = at_limit.len();

        let server_psk = psk;
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept");
            let mut session = Session::accept(stream, &server_psk)
                .await
                .expect("responder");
            let got: String = session.recv().await.expect("recv").expect("message");
            assert_eq!(got.len(), expected_len);
        });

        let mut client = Session::connect(addr, &psk).await.expect("initiator");
        client
            .send(&at_limit)
            .await
            .expect("exactly at the limit must send");
        server.await.expect("server task");
    }

    #[tokio::test]
    async fn tampered_frame_fails_authentication_and_poisons_the_session() {
        let (listener, addr) = loopback().await;
        let psk = PairingToken::generate().psk();

        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept");
            // Speak the handshake honestly, then send a frame that is the right
            // shape but not a valid Noise message.
            let mut framed = Framed::new(stream, codec());
            let first = framed.next().await.expect("frame").expect("ok");
            let mut buf = vec![0u8; MAX_NOISE_MESSAGE];
            let mut hs = Builder::new(noise_params().unwrap())
                .psk(0, &psk)
                .unwrap()
                .build_responder()
                .unwrap();
            hs.read_message(&first, &mut buf).expect("valid initiation");
            let len = hs.write_message(&[], &mut buf).expect("response");
            framed
                .send(Bytes::copy_from_slice(&buf[..len]))
                .await
                .expect("send response");
            let mut noise = hs.into_transport_mode().expect("transport");
            let len = noise.write_message(b"\x02garbage", &mut buf).expect("seal");
            // Flip a bit in the ciphertext.
            buf[len / 2] ^= 0x01;
            framed
                .send(Bytes::copy_from_slice(&buf[..len]))
                .await
                .expect("send tampered");
            // Hold the connection open so the failure is authentication, not EOF.
            tokio::time::sleep(Duration::from_millis(200)).await;
            drop(framed);
        });

        let mut client = Session::connect(addr, &psk).await.expect("initiator");
        let result: Result<Option<Ping>, _> = client.recv().await;
        assert!(
            matches!(result, Err(TransportError::Malformed)),
            "tampering must fail closed, got {result:?}"
        );
        assert!(
            client.is_poisoned(),
            "a failed frame must poison the session"
        );
        // Every later call fails fast rather than emitting garbage.
        assert!(matches!(
            client
                .send(&Ping {
                    seq: 0,
                    note: String::new()
                })
                .await,
            Err(TransportError::Malformed)
        ));
        server.await.expect("server task");
    }

    #[tokio::test]
    async fn truncation_mid_message_is_not_a_clean_close() {
        let (listener, addr) = loopback().await;
        let psk = PairingToken::generate().psk();

        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept");
            let mut session = Session::accept(stream, &psk).await.expect("responder");
            // One record marked "more follows", then hang up.
            session
                .write_record(RECORD_MORE, b"half a message")
                .await
                .expect("partial record");
            drop(session);
        });

        let mut client = Session::connect(addr, &psk).await.expect("initiator");
        let result: Result<Option<Ping>, _> = client.recv().await;
        assert!(
            matches!(result, Err(TransportError::Malformed)),
            "a truncated message must not read as a close, got {result:?}"
        );
        server.await.expect("server task");
    }

    #[tokio::test]
    async fn unparseable_payload_is_codec_and_leaves_the_session_usable() {
        let (listener, addr) = loopback().await;
        let psk = PairingToken::generate().psk();

        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept");
            let mut session = Session::accept(stream, &psk).await.expect("responder");
            // Authenticates fine; is not a `Ping`.
            session.send(&"not a ping").await.expect("send string");
            session
                .send(&Ping {
                    seq: 5,
                    note: "recovered".into(),
                })
                .await
                .expect("send ping");
            session.close().await.expect("close");
        });

        let mut client = Session::connect(addr, &psk).await.expect("initiator");
        let bad: Result<Option<Ping>, _> = client.recv().await;
        assert!(matches!(bad, Err(TransportError::Codec)), "got {bad:?}");
        assert!(
            !client.is_poisoned(),
            "a parse failure is not a channel failure"
        );
        let good: Ping = client.recv().await.expect("recv").expect("message");
        assert_eq!(good.seq, 5);
        server.await.expect("server task");
    }

    #[tokio::test]
    async fn session_debug_shows_no_key_material() {
        let (listener, addr) = loopback().await;
        let token = PairingToken::generate();
        let psk = token.psk();

        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept");
            let session = Session::accept(stream, &psk).await.expect("responder");
            format!("{session:?}")
        });

        let client = Session::connect(addr, &psk).await.expect("initiator");
        let rendered = format!("{client:?}");
        assert_no_secret(&rendered, &token);
        assert!(rendered.contains("peer_addr"));

        let server_rendered = server.await.expect("server task");
        assert_no_secret(&server_rendered, &token);
    }
}
