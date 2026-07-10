//! Transport-wide tuning constants + the length-delimited codec / TCP
//! keepalive helpers shared by [`super::PeerTransport::accept`] /
//! [`super::PeerTransport::connect`].

use std::time::Duration;

use socket2::{SockRef, TcpKeepalive};
use tokio::net::TcpStream;
use tokio_util::codec::LengthDelimitedCodec;

/// Maximum time we will wait for the TCP SYN/ACK connect phase to complete.
/// Kept shorter than [`TLS_HANDSHAKE_TIMEOUT`] so the retry budget in
/// [`super::PeerTransport::connect_with_retry`] is spent on the brief mDNS-announce →
/// listener race rather than waiting 10 s per attempt on a dead peer. A
/// TCP-connect timeout maps to [`super::error::TransportError::Io`] (kind `TimedOut`) so it
/// is classified **transient** and retried; a post-TCP TLS-handshake timeout
/// maps to [`super::error::TransportError::HandshakeTimeout`] (permanent — slowloris guard).
pub const TCP_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// Maximum time we will wait for a TLS handshake (client or server side) to
/// complete before giving up. Protects against dead sockets and slowloris-style
/// stalls during handshake.
pub const TLS_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

/// Fixed SNI sentinel used for all P2P TLS handshakes.
///
/// rustls requires a `ServerName` even though peer identity is established by
/// certificate-fingerprint pinning, not hostname. The client always sets this
/// exact value (see [`super::PeerTransport::connect`]) and the client-side verifier
/// (`verifier::PeerCertVerifier`) compares the presented SNI against it as
/// defense-in-depth, rejecting any mismatch.
pub const P2P_SNI_SENTINEL: &str = "copypaste.peer";

/// Default number of times [`super::PeerTransport::connect_with_retry`] will retry a
/// transient network error before propagating it. The first attempt counts —
/// i.e. `MAX_CONNECT_ATTEMPTS = 4` means 1 initial attempt + 3 retries.
pub const MAX_CONNECT_ATTEMPTS: u32 = 4;

/// Delay between transient-error retries in [`super::PeerTransport::connect_with_retry`].
/// Kept short (100 ms) because the typical trigger is a peer that just
/// announced over mDNS but hasn't bound its listener yet, or a brief network
/// blip on the LAN — not a peer that genuinely needs minutes of backoff
/// (that's the relay client's job, see `copypaste_sync::backoff`).
pub const CONNECT_RETRY_DELAY: Duration = Duration::from_millis(100);

/// Maximum size of a single length-delimited data-plane frame (16 MiB).
///
/// The data plane carries serialized `WireItem`s. The largest payload is an
/// image item whose ciphertext the relay caps at 10 MiB
/// (`copypaste_ipc::RELAY_MAX_ITEM_BYTES`); base64/JSON framing of that blob
/// plus item metadata can roughly inflate it, so we size the ceiling to match
/// `copypaste_sync::engine::MAX_FRAME_BYTES` (16 MiB) rather than relying on
/// tokio-util's silent 8 MiB `LengthDelimitedCodec::new()` default, which would
/// truncate large images and stall the link. A peer that sends a frame above
/// this ceiling has its connection torn down (DoS guard).
///
/// Re-exported from [`copypaste_ipc::MAX_FRAME_BYTES`] (CopyPaste-1d5l.59) —
/// the same canonical value `copypaste_sync::engine::MAX_FRAME_BYTES` aliases,
/// so the two crates (which do not depend on each other) cannot drift. A
/// compile-time equality assertion also lives in
/// `copypaste-daemon/tests/frame_consts.rs` (CopyPaste-w47w #1) as a belt-
/// and-suspenders regression guard.
pub const MAX_FRAME_BYTES: usize = copypaste_ipc::MAX_FRAME_BYTES;

/// Build the length-delimited codec used for every data-plane stream, with the
/// frame ceiling explicitly set to [`MAX_FRAME_BYTES`] (16 MiB).
///
/// The bootstrap handshake uses a separate, tighter 64 KiB codec
/// (`bootstrap::framing::MAX_HANDSHAKE_FRAME_BYTES`); this is the data-plane
/// codec that carries `WireItem` payloads after the handshake completes.
pub(super) fn length_codec() -> LengthDelimitedCodec {
    LengthDelimitedCodec::builder()
        .max_frame_length(MAX_FRAME_BYTES)
        .new_codec()
}

/// Idle time before the OS starts sending TCP keepalive probes.
///
/// `pub` (CopyPaste-vgpy) so `copypaste-daemon`'s `p2p::framed_pump::WRITE_TIMEOUT`
/// can assert its ordering against this constant at compile time — see that
/// assertion for the invariant this protects.
pub const TCP_KEEPALIVE_TIME: Duration = Duration::from_secs(20);

/// Interval between successive TCP keepalive probes once they start.
const TCP_KEEPALIVE_INTERVAL: Duration = Duration::from_secs(10);

/// Enable TCP keepalive on an established P2P socket.
///
/// Defense-in-depth alongside the daemon-side write timeout: if a peer vanishes
/// with **no** FIN (Wi-Fi drop, app killed, cable yanked) there is no EOF to
/// observe, so without keepalive the kernel would never error the socket and
/// the pump's read/write arms would block indefinitely. Keepalive probes force
/// the socket into an error state after `TCP_KEEPALIVE_TIME` +
/// N×`TCP_KEEPALIVE_INTERVAL`, which surfaces as a read/write error and tears
/// the connection down. Best-effort: a failure to set the option is logged and
/// ignored rather than dropping an otherwise-usable connection.
pub(super) fn enable_tcp_keepalive(stream: &TcpStream) {
    let keepalive = TcpKeepalive::new()
        .with_time(TCP_KEEPALIVE_TIME)
        .with_interval(TCP_KEEPALIVE_INTERVAL);
    if let Err(e) = SockRef::from(stream).set_tcp_keepalive(&keepalive) {
        tracing::warn!("failed to enable TCP keepalive on peer socket: {e}");
    }
}
