//! The secure channel between two paired devices.
//!
//! # What this module is
//!
//! One thing: a mutually-authenticated, forward-secret, message-oriented
//! channel over TCP, built from a 256-bit pairing token.
//!
//! ```text
//! PairingToken (32 B, OS CSPRNG)
//!   ├─ to_code()      -> "0G7M-XQ4V-…"   human-transferable, Crockford base32
//!   ├─ psk()          -> Noise pre-shared key
//!   └─ pairing_id()   -> BLAKE2s(domain || token)[..16], hex — safe to log
//!
//! Session::connect / Session::accept
//!   └─ Noise_NNpsk0_25519_ChaChaPoly_BLAKE2s
//!        └─ LengthDelimitedCodec frames  ->  send::<T> / recv::<T> (serde JSON)
//! ```
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
//!
//! # Rules this module holds itself to
//!
//! * **No secret in `Debug`, `Display` or any log line.** [`PairingToken`]'s
//!   `Debug` prints its `pairing_id` and nothing else; [`Session`]'s prints the
//!   peer address. Tests pin both.
//! * **No filesystem path in any error** (`CLAUDE.md` rule 4). Structurally
//!   enforced: no variant of [`TransportError`] has a payload that can hold
//!   one, and this module never opens a file.
//! * **Constant-time comparison of secrets** via `subtle` (port manifest 02,
//!   I-13). `PairingToken`'s `PartialEq` routes through it, so there is no
//!   short-circuiting comparison of token bytes reachable from outside.
//! * **Zeroize on drop** for the token and for every buffer that holds
//!   plaintext or key material (I-12).
//! * **A handshake cannot pin a task.** Dial, both handshake messages and the
//!   transport-mode transition all happen inside one
//!   [`HANDSHAKE_TIMEOUT`].

use std::fmt;
use std::net::SocketAddr;
use std::sync::OnceLock;
use std::time::Duration;

use data_encoding::{Encoding, Specification};
use futures_util::{SinkExt, StreamExt};
use rand::rngs::OsRng;
use rand::RngCore;
use snow::params::{HashChoice, NoiseParams};
use snow::resolvers::{CryptoResolver, DefaultResolver};
use snow::{Builder, TransportState};
use subtle::ConstantTimeEq;
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_util::bytes::Bytes;
use tokio_util::codec::{Framed, LengthDelimitedCodec};
use zeroize::{Zeroize, Zeroizing};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// The Noise handshake pattern, verbatim.
///
/// Changing any component of this string is a wire break: both ends must parse
/// the identical name or the handshake fails. It is public so that the daemon
/// can report it and so a test can pin it.
pub const NOISE_PARAMS: &str = "Noise_NNpsk0_25519_ChaChaPoly_BLAKE2s";

/// Length of the pairing token, and of the Noise pre-shared key it becomes.
pub const TOKEN_LEN: usize = 32;

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

/// Wall-clock budget for connecting and completing the handshake.
///
/// A peer that opens a TCP connection and then says nothing must not be able to
/// hold a daemon task open indefinitely. Ten seconds is generous for two
/// round-trips of X25519 on a LAN and short enough that a stalled listener
/// recovers without operator action.
pub const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

/// Characters per group in the printed pairing code.
const CODE_GROUP: usize = 4;

/// Group separator in the printed pairing code. Ignored on parse, along with
/// whitespace and `_`, so a user may retype the code however they like.
const CODE_SEPARATOR: char = '-';

/// Domain separator for [`PairingToken::pairing_id`]. Distinct from every other
/// use of the token, so the public id and the secret PSK are not derived by the
/// same function (port manifest 02, I-16).
const PAIRING_ID_DOMAIN: &[u8] = b"copypaste/v2/pairing-id|";

/// Bytes of the pairing-id digest that are kept. 128 bits of a 256-bit digest:
/// long enough that a collision between a user's handful of devices is not a
/// thing that happens, short enough to read in a log line.
const PAIRING_ID_LEN: usize = 16;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Every failure this module can produce.
///
/// No variant carries a `String`, a `PathBuf` or a foreign error's `Display`
/// output. That is deliberate on two counts: `CLAUDE.md` rule 4 forbids showing
/// a user a filesystem path (the daemon's socket path discloses the local
/// username), and a closed set of literal messages cannot leak one by accident.
/// Where a cause is worth keeping it is attached as a `#[source]`, which the
/// daemon can log but which never appears in this error's own `Display`.
#[derive(Debug, thiserror::Error)]
pub enum TransportError {
    /// The Noise handshake did not complete: wrong pairing token, a peer that
    /// is not speaking this protocol, a peer that stalled past
    /// [`HANDSHAKE_TIMEOUT`], or a peer that hung up mid-handshake.
    ///
    /// These are **deliberately indistinguishable**. Reporting *which* one
    /// happened would tell an attacker probing the port whether a guessed token
    /// was structurally accepted, which is the decryption-oracle shape port
    /// manifest 02 I-15 exists to prevent. The only correct handling is to drop
    /// the connection.
    #[error("secure handshake failed")]
    Handshake,

    /// The underlying socket failed.
    ///
    /// The cause is a `#[source]`, not interpolated into the message: this type
    /// must never be able to render a path, and while this module only ever
    /// touches sockets, an `io::Error` from elsewhere in a future refactor
    /// could.
    #[error("network connection failed")]
    Io(#[source] std::io::Error),

    /// The message could not be serialised to, or deserialised from, JSON.
    ///
    /// A local encoding bug or a peer sending a shape this build does not know.
    /// Distinct from [`TransportError::Malformed`] because the channel is still
    /// intact — the bytes authenticated, they just did not parse — so the
    /// caller may keep using the session.
    #[error("message payload could not be encoded or decoded")]
    Codec,

    /// A frame failed authentication, carried an unknown record marker, or the
    /// stream ended in the middle of a logical message.
    ///
    /// The session is poisoned once this is returned: an authentication failure
    /// desynchronises the Noise nonce sequence, and continuing would produce a
    /// cascade of failures that look like corruption. Drop the session and
    /// reconnect.
    #[error("received frame was malformed or failed authentication")]
    Malformed,

    /// A logical message exceeded [`MAX_MESSAGE_BYTES`], on send or on
    /// reassembly.
    ///
    /// Returned rather than truncating. Silent truncation of clipboard content
    /// is data loss, which `CLAUDE.md` rule 4 ranks as the worst outcome.
    #[error("message exceeds the {MAX_MESSAGE_BYTES}-byte limit")]
    TooLarge,

    /// A pairing code did not decode to a [`TOKEN_LEN`]-byte token.
    ///
    /// Wrong length, a character outside the alphabet, or non-canonical
    /// trailing bits. Carries no detail about what the user typed — the input
    /// is a secret.
    #[error("pairing code is not valid")]
    InvalidCode,
}

// ---------------------------------------------------------------------------
// Pairing token
// ---------------------------------------------------------------------------

/// The shared secret that makes two devices a pair.
///
/// 256 bits from the OS CSPRNG. Possession of it *is* the authentication: there
/// is no account, no certificate and nothing else to check. It is zeroized on
/// drop, compared in constant time, and never rendered by `Debug`.
///
/// It is intentionally not `Clone` (port manifest 02, I-12). Handing out copies
/// of key material should be an explicit act, which is what [`PairingToken::psk`]
/// is — the [`crate::PeerStore`] needs a plain `[u8; 32]` to persist.
pub struct PairingToken(Zeroizing<[u8; TOKEN_LEN]>);

impl PairingToken {
    /// Mint a fresh token from the OS CSPRNG.
    ///
    /// `OsRng`, never `thread_rng` — one entropy source across the whole crypto
    /// surface (port manifest 02, I-11).
    #[must_use]
    pub fn generate() -> Self {
        let mut bytes = Zeroizing::new([0u8; TOKEN_LEN]);
        OsRng.fill_bytes(bytes.as_mut_slice());
        Self(bytes)
    }

    /// Build a token from bytes that came from somewhere else.
    ///
    /// The caller is asserting these bytes are 256 bits of CSPRNG output. The
    /// security of everything downstream rests on that.
    #[must_use]
    pub fn from_bytes(bytes: &[u8; TOKEN_LEN]) -> Self {
        Self(Zeroizing::new(*bytes))
    }

    /// The human-transferable form: 52 Crockford base32 characters in groups of
    /// four, e.g. `0G7M-XQ4V-…` (64 characters including separators).
    ///
    /// Crockford's alphabet rather than RFC 4648's, because this string gets
    /// read aloud and retyped: it has no `I`, `L`, `O` or `U`, so there is no
    /// `0`/`O` or `1`/`I`/`l` confusion to resolve and no accidental profanity.
    /// [`PairingToken::parse`] additionally folds case and translates the
    /// ambiguous glyphs anyway, so a user who types `O` for `0` still succeeds.
    ///
    /// The code is a secret. It is safe to *show* — that is its job — but never
    /// log it, and never put it in an error.
    #[must_use]
    pub fn to_code(&self) -> String {
        let raw = code_encoding().encode(&self.0[..]);
        let groups = raw.len().div_ceil(CODE_GROUP);
        let mut out = String::with_capacity(raw.len() + groups);
        for (i, chunk) in raw.as_bytes().chunks(CODE_GROUP).enumerate() {
            if i > 0 {
                out.push(CODE_SEPARATOR);
            }
            // `chunk` came out of `encode`, which only emits ASCII symbols.
            out.push_str(std::str::from_utf8(chunk).unwrap_or_default());
        }
        out
    }

    /// Parse a pairing code produced by [`PairingToken::to_code`].
    ///
    /// Tolerant of how a human retypes it: case is folded, `-`, `_` and
    /// whitespace are ignored wherever they appear, and `O`/`o` → `0`,
    /// `I`/`i`/`L`/`l` → `1`. Not tolerant of anything else — a code with a
    /// character outside the alphabet, the wrong decoded length, or non-zero
    /// trailing bits is rejected rather than silently coerced.
    ///
    /// # Errors
    ///
    /// [`TransportError::InvalidCode`], with no detail about the input.
    pub fn parse(code: &str) -> Result<Self, TransportError> {
        let mut decoded = code_encoding()
            .decode(code.as_bytes())
            .map_err(|_| TransportError::InvalidCode)?;
        if decoded.len() != TOKEN_LEN {
            decoded.zeroize();
            return Err(TransportError::InvalidCode);
        }
        let mut bytes = Zeroizing::new([0u8; TOKEN_LEN]);
        bytes.copy_from_slice(&decoded);
        decoded.zeroize();
        Ok(Self(bytes))
    }

    /// The token as a Noise pre-shared key.
    ///
    /// This is a copy of the secret and is *not* zeroized for you. Hold it in a
    /// `zeroize::Zeroizing` if it outlives the call that needs it.
    #[must_use]
    pub fn psk(&self) -> [u8; TOKEN_LEN] {
        *self.0
    }

    /// A stable, non-secret identifier for this pairing: 32 lowercase hex
    /// characters, safe to log and to use as the [`crate::PeerStore`] key.
    ///
    /// `BLAKE2s(domain || token)`, truncated to 128 bits. BLAKE2s because it is
    /// already in the tree as the Noise suite's hash — reaching it through
    /// `snow`'s resolver avoids adding a second hash implementation for one
    /// call (`CLAUDE.md` rule 1).
    ///
    /// Inverting this to recover the token would require preimaging a 256-bit
    /// random input, so the id is safe in log files and in a world-readable
    /// place. It is one-way on purpose: the id identifies a pairing, it does not
    /// stand in for it.
    #[must_use]
    pub fn pairing_id(&self) -> String {
        let digest = blake2s(&[PAIRING_ID_DOMAIN, &self.0[..]]);
        hex::encode(&digest[..PAIRING_ID_LEN])
    }
}

/// Constant-time. `==` on key bytes short-circuits at the first differing byte
/// and leaks the matching-prefix length by timing (port manifest 02, I-13);
/// routing `PartialEq` through `subtle` means no caller can reach a
/// short-circuiting comparison of token bytes.
impl PartialEq for PairingToken {
    fn eq(&self, other: &Self) -> bool {
        self.0[..].ct_eq(&other.0[..]).into()
    }
}

impl Eq for PairingToken {}

/// Prints the pairing id and nothing else.
///
/// v1's rule was that `PairingToken` has no `Debug` at all. A `Debug` that is
/// structurally incapable of reaching the secret gets the same property while
/// still letting the type sit inside a `#[derive(Debug)]` struct, which is
/// where an accidental `{:?}` on a secret normally comes from.
impl fmt::Debug for PairingToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PairingToken")
            .field("pairing_id", &self.pairing_id())
            .finish_non_exhaustive()
    }
}

/// Crockford base32, built once.
///
/// * symbols — digits plus the 22 letters that are not `I`, `L`, `O` or `U`.
/// * translate — lowercase folds to uppercase; `O`/`o` → `0`; `I`/`i`/`L`/`l`
///   → `1`. These are the substitutions a human actually makes.
/// * ignore — `-`, `_`, space, tab, CR and LF, so grouping and line wrapping
///   survive a round trip through a chat window.
/// * no padding, and `check_trailing_bits` left at its default of `true`, so
///   there is exactly one valid encoding of a given token.
fn code_encoding() -> &'static Encoding {
    static ENCODING: OnceLock<Encoding> = OnceLock::new();
    ENCODING.get_or_init(|| {
        let mut spec = Specification::new();
        spec.symbols.push_str("0123456789ABCDEFGHJKMNPQRSTVWXYZ");
        spec.translate.from.push_str("abcdefghjkmnpqrstvwxyzOoIiLl");
        spec.translate.to.push_str("ABCDEFGHJKMNPQRSTVWXYZ001111");
        spec.ignore.push_str("-_ \t\r\n");
        // The specification is a compile-time constant expressed in data; a
        // failure here is a typo in the three literals above, caught by this
        // module's tests on every build, and unreachable from any input.
        spec.encoding()
            .expect("the pairing-code alphabet is well-formed")
    })
}

/// BLAKE2s-256 over the concatenation of `parts`, via `snow`'s resolver.
fn blake2s(parts: &[&[u8]]) -> [u8; 32] {
    let mut hasher = DefaultResolver
        .resolve_hash(&HashChoice::Blake2s)
        .expect("snow's default resolver always provides BLAKE2s");
    hasher.reset();
    for part in parts {
        hasher.input(part);
    }
    let mut out = [0u8; 32];
    hasher.result(&mut out);
    out
}

// ---------------------------------------------------------------------------
// Session
// ---------------------------------------------------------------------------

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

    fn new(
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
fn codec() -> LengthDelimitedCodec {
    LengthDelimitedCodec::builder()
        .length_field_length(4)
        .big_endian()
        .max_frame_length(MAX_NOISE_MESSAGE)
        .new_codec()
}

fn noise_params() -> Result<NoiseParams, TransportError> {
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    // -- pairing token ------------------------------------------------------

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

    #[test]
    fn code_round_trips() {
        let token = PairingToken::generate();
        let code = token.to_code();
        let parsed = PairingToken::parse(&code).expect("own code must parse");
        assert_eq!(token, parsed);
        assert_eq!(token.psk(), parsed.psk());
        assert_eq!(token.pairing_id(), parsed.pairing_id());
    }

    #[test]
    fn code_shape_is_grouped_crockford_base32() {
        let token = PairingToken::from_bytes(&[0u8; TOKEN_LEN]);
        let code = token.to_code();
        // 32 bytes -> 52 base32 symbols -> 13 groups of 4, 12 separators.
        assert_eq!(code.len(), 52 + 12);
        let groups: Vec<&str> = code.split(CODE_SEPARATOR).collect();
        assert_eq!(groups.len(), 13);
        assert!(groups.iter().all(|g| g.len() == CODE_GROUP));
        // Crockford excludes these four letters entirely.
        for bad in ['I', 'L', 'O', 'U'] {
            assert!(
                !PairingToken::generate().to_code().contains(bad),
                "ambiguous glyph {bad} must not be emitted"
            );
        }
    }

    #[test]
    fn code_parses_lowercase_ungrouped_and_respaced() {
        let token = PairingToken::generate();
        let code = token.to_code();
        let bare: String = code.chars().filter(|c| *c != CODE_SEPARATOR).collect();

        for variant in [
            code.clone(),
            code.to_lowercase(),
            bare.clone(),
            bare.to_lowercase(),
            format!("  {code}  "),
            code.replace(CODE_SEPARATOR, " "),
            code.replace(CODE_SEPARATOR, "_"),
            bare.chars()
                .collect::<Vec<_>>()
                .chunks(8)
                .map(|c| c.iter().collect::<String>())
                .collect::<Vec<_>>()
                .join("\n"),
        ] {
            let parsed = PairingToken::parse(&variant)
                .unwrap_or_else(|_| panic!("variant must parse: {variant:?}"));
            assert_eq!(token, parsed, "variant changed the token: {variant:?}");
        }
    }

    #[test]
    fn code_parse_translates_ambiguous_glyphs() {
        // A user reading the code aloud will say "oh" for zero and "el" for
        // one. Both must land on the same token.
        let token = PairingToken::from_bytes(&[0u8; TOKEN_LEN]);
        let code = token.to_code(); // all zeroes -> all '0' symbols
        assert!(code.contains('0'));
        let mistyped = code.replace('0', "O");
        assert_eq!(
            PairingToken::parse(&mistyped).expect("O must translate to 0"),
            token
        );
        let one = PairingToken::from_bytes(&{
            let mut b = [0u8; TOKEN_LEN];
            // 0b00001000 00... -> second symbol is '1'
            b[0] = 0b0000_1000;
            b
        });
        let code = one.to_code();
        assert!(code.contains('1'));
        for glyph in ["I", "i", "L", "l"] {
            assert_eq!(
                PairingToken::parse(&code.replacen('1', glyph, 1))
                    .unwrap_or_else(|_| panic!("{glyph} must translate to 1")),
                one
            );
        }
    }

    #[test]
    fn code_parse_rejects_bad_input() {
        let good = PairingToken::generate().to_code();
        let cases = [
            String::new(),
            "not a pairing code".to_string(),
            good[..good.len() - 5].to_string(),        // too short
            format!("{good}ABCD"),                     // too long
            good.replacen(|c: char| c != '-', "U", 1), // U is not a symbol
            good.replacen(|c: char| c != '-', "$", 1), // outside the alphabet
        ];
        for case in cases {
            assert!(
                matches!(PairingToken::parse(&case), Err(TransportError::InvalidCode)),
                "must reject: {case:?}"
            );
        }
    }

    #[test]
    fn code_parse_rejects_non_canonical_trailing_bits() {
        // 52 symbols carry 260 bits; the last 4 must be zero. A code whose
        // trailing bits are set decodes to the same 32 bytes but is a second
        // valid spelling, which would make the pairing id ambiguous.
        let token = PairingToken::from_bytes(&[0u8; TOKEN_LEN]);
        let code = token.to_code();
        let bare: String = code.chars().filter(|c| *c != CODE_SEPARATOR).collect();
        let mut chars: Vec<char> = bare.chars().collect();
        chars[51] = '1'; // sets a trailing bit
        let tweaked: String = chars.into_iter().collect();
        assert!(matches!(
            PairingToken::parse(&tweaked),
            Err(TransportError::InvalidCode)
        ));
    }

    #[test]
    fn pairing_id_is_stable_derived_and_not_the_token() {
        let token = PairingToken::generate();
        let id = token.pairing_id();
        assert_eq!(id, token.pairing_id(), "must be deterministic");
        assert_eq!(id.len(), PAIRING_ID_LEN * 2);
        assert!(id
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_uppercase()));

        // It must not be, or contain, the secret in any obvious encoding.
        let psk = token.psk();
        assert!(!id.contains(&hex::encode(psk)));
        assert!(!id.contains(&token.to_code()));
        assert_ne!(id.as_bytes(), &psk[..]);
        // Nor may it be the first half of the token.
        assert_ne!(id, hex::encode(&psk[..PAIRING_ID_LEN]));

        // Different tokens, different ids.
        let other = PairingToken::generate();
        assert_ne!(id, other.pairing_id());

        // A one-bit change in the token changes the id.
        let mut flipped = psk;
        flipped[0] ^= 1;
        assert_ne!(id, PairingToken::from_bytes(&flipped).pairing_id());
    }

    #[test]
    fn token_debug_never_prints_key_material() {
        let token = PairingToken::generate();
        let rendered = format!("{token:?}");
        assert_no_secret(&rendered, &token);
        assert!(rendered.contains(&token.pairing_id()));
    }

    #[test]
    fn token_equality_is_by_value() {
        let a = PairingToken::generate();
        let b = PairingToken::from_bytes(&a.psk());
        assert_eq!(a, b);
        let mut different = a.psk();
        different[31] ^= 0x80;
        assert_ne!(a, PairingToken::from_bytes(&different));
    }

    /// Fails if `text` contains the token in any encoding this codebase uses.
    fn assert_no_secret(text: &str, token: &PairingToken) {
        let psk = token.psk();
        assert!(!text.contains(&hex::encode(psk)), "leaked hex token");
        assert!(
            !text.contains(&hex::encode_upper(psk)),
            "leaked upper-hex token"
        );
        assert!(!text.contains(&token.to_code()), "leaked pairing code");
        // Rust's default `{:?}` for a byte array.
        let debug_bytes = format!("{:?}", &psk[..]);
        assert!(!text.contains(&debug_bytes), "leaked debug byte array");
        // Any run of four consecutive token bytes rendered as decimal, which is
        // what a derived Debug on `[u8; 32]` would produce.
        let decimal = psk
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        assert!(!text.contains(&decimal), "leaked decimal byte array");
    }

    // -- session ------------------------------------------------------------

    #[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
    struct Ping {
        seq: u64,
        note: String,
    }

    /// Bind a listener on loopback and hand back its address.
    async fn loopback() -> (TcpListener, SocketAddr) {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("loopback bind");
        let addr = listener.local_addr().expect("local addr");
        (listener, addr)
    }

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

    #[test]
    fn error_messages_contain_no_paths() {
        // `CLAUDE.md` rule 4. The variants have no payload that could hold one,
        // but the literals themselves must also stay clean.
        let errors: Vec<TransportError> = vec![
            TransportError::Handshake,
            TransportError::Io(std::io::Error::other("socket")),
            TransportError::Codec,
            TransportError::Malformed,
            TransportError::TooLarge,
            TransportError::InvalidCode,
        ];
        for err in errors {
            let text = err.to_string();
            assert!(!text.contains('/'), "path-like error text: {text}");
            assert!(!text.contains('\\'), "path-like error text: {text}");
            assert!(!text.is_empty());
        }
        // The io cause is a source, not part of Display.
        let io = TransportError::Io(std::io::Error::other("/home/someone/socket"));
        assert!(!io.to_string().contains("/home"));
    }
}
