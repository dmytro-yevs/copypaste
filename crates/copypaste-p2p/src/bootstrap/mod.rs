//! Unauthenticated bootstrap TLS channel for PAKE pairing (P2P Phase 1).
//!
//! # Why a separate, unauthenticated channel?
//!
//! The production [`crate::transport::PeerTransport`] requires *mutual*
//! certificate-fingerprint pinning: both ends must already know the other's
//! fingerprint (it lives in [`crate::transport::PairedPeers`]). That is a
//! chicken-and-egg problem for *first* pairing — neither side knows the other
//! yet.
//!
//! The bootstrap channel breaks the cycle. It is a TCP+TLS channel where
//! **both sides accept any certificate** (no pinning). Authentication is
//! provided out-of-band by the PAKE handshake: both ends derive the same
//! 32-byte [`crate::pake::SessionKey`] only if they share the QR pairing
//! secret, so a man-in-the-middle who cannot read the QR cannot complete the
//! handshake. The cert fingerprints are exchanged over this same channel and
//! their authenticity follows from PAKE success.
//!
//! TLS is still used (rather than plain TCP) so the PAKE messages and the
//! exchanged fingerprints are encrypted in transit on the LAN, and so the same
//! self-signed device certificate is presented that the *subsequent* pinned
//! mTLS sessions will use — letting each side learn the cert fingerprint it
//! must pin later.
//!
//! Channel binding (S3): after PAKE completes, each side mixes the RFC 5705 TLS
//! exporter for *this* TLS session into the PAKE key
//! ([`SessionKey::bind_to_tls_channel`]) and the two ends exchange
//! role-separated confirmation tags ([`crate::pake::channel_confirmation_tag`]),
//! compared in constant time. This binds pairing authenticity to the specific
//! bootstrap TLS session: a relay/MitM that bridges PAKE over two separate TLS
//! connections derives a different binder per leg, so the tags never match and
//! pairing is aborted.
//!
//! # Wire protocol (over the framed TLS stream)
//!
//! Length-delimited frames (same codec as [`crate::transport`]):
//!
//! ```text
//! Initiator (client)                         Responder (server)
//!   | --- 1. PAKE message1            -->  |
//!   | --- 2. own cert fingerprint     -->  |
//!   | --- 3. own P2P sync addr        -->  |
//!   | <-- 4. PAKE message2             --- |
//!   | <-- 5. own cert fingerprint      --- |
//!   | <-- 6. own P2P sync addr         --- |
//!   | --- 7. PAKE message3            -->  |
//!   | == both sides hold the same SessionKey == |
//!   | <-- 8. responder confirm tag     --- |
//!   | --- 9. initiator confirm tag    -->  |
//!   | == both confirm tags verified (constant-time) == |
//! ```
//!
//! On success each side returns the *peer's* cert fingerprint, the peer's P2P
//! sync-listener address, and the derived [`crate::pake::SessionKey`]. The peer
//! fingerprint sent in the frame is cross-checked against the fingerprint of the
//! certificate actually presented during the TLS handshake, so a peer cannot
//! advertise one fingerprint in the frame while presenting a different
//! certificate.
//!
//! ## Wire protocol version
//!
//! The sync-address frames (3 and 6) were added in P2P Phase 2. Both ends are
//! shipped together (there is no mixed-version pairing across hosts at this
//! stage), so the frame order is fixed rather than negotiated; the address frame
//! immediately follows each side's fingerprint frame.
//!
//! ## Device-metadata extension (appended after frame 9)
//!
//! P2P Phase 4 appends an OPTIONAL metadata exchange AFTER the 9-frame PAKE +
//! channel-binding handshake has fully completed (both confirmation tags
//! verified). Because it comes strictly after the original protocol terminates,
//! an OLD peer (which closes / stops reading at frame 9) is unaffected — it
//! never sees these frames and pairing still succeeds for both. A new peer:
//!
//! ```text
//!   | --- 10. BOOTSTRAP_PROTO_VERSION byte  -->  (and the mirror <-- )
//!   | --- 11. compact JSON {model,os_version,app_version,local_ip}  --> (mirror <--)
//!   | --- 12. (proto >= 2) SyncProvisioning JSON  --> (mirror <--)
//! ```
//!
//! Each side first sends its own version byte then reads the peer's. If the
//! peer's version frame is absent (connection closed → old peer) the metadata
//! step is skipped entirely and `peer_*` fields are left `None`. The metadata
//! frame (11) is read whenever a version byte was received.
//!
//! ## Sync-provisioning extension (frame 12, proto version 2)
//!
//! Proto version 2 appends ONE further OPTIONAL frame after the metadata frame:
//! a [`SyncProvisioning`] JSON ("QR fully provisions all sync" — carry the
//! Supabase/relay config + the DERIVED cloud sync key over the already
//! authenticated tunnel, NEVER in the QR image). It is version-gated by the
//! frame-10 version byte: a side advertising `>= 2` sends frame 12 and, when the
//! PEER also advertised `>= 2`, reads the peer's frame 12; a side talking to a
//! version-1 peer neither expects nor reads it (so the stream never desyncs).
//! An unconfigured side sends an all-`None` `SyncProvisioning`. All
//! send/receive errors are swallowed: the pairing is already authenticated and
//! complete, so a provisioning hiccup must never fail it.

// Re-use the project-wide QR pairing TTL from copypaste-ipc — the single
// source of truth for the window within which the user must scan the QR,
// confirm, and have the initiator connect. Both the daemon's
// `generate_pairing_qr` handler (which stamps `expires_at = now + this`) and
// the bootstrap responder accept timeout are derived from this value so they
// cannot drift independently. Previously this crate carried a local copy
// (`const QR_PAIRING_TTL_SECS: u64 = 120`) — removed by CopyPaste-ijm0.
use copypaste_ipc::QR_PAIRING_TTL_SECS;
use std::time::Duration;

mod framing;
mod initiator;
mod meta;
mod responder;
mod tls;
mod types;

#[cfg(test)]
mod tests;

// ── Public re-exports ─────────────────────────────────────────────────────────

pub use initiator::{run_initiator, run_initiator_with_confirm};
pub use responder::BootstrapResponder;
pub use types::{BootstrapPairing, PeerMeta, SyncProvisioning};

/// Maximum time the responder bootstrap listener waits for the single inbound
/// pairing connection before giving up.
///
/// Derived from [`QR_PAIRING_TTL_SECS`] so it cannot drift from the QR token's
/// time-to-live: the user scans the QR, confirms on their device, and the
/// initiator connects — all within this window.
pub const BOOTSTRAP_ACCEPT_TIMEOUT: Duration = Duration::from_secs(QR_PAIRING_TTL_SECS);

/// Maximum total time allowed for the 9-frame post-TLS PAKE exchange (both
/// sides). A peer that completes TLS but then dribbles frames would otherwise
/// pin the single-shot responder indefinitely (slowloris-style DoS). 30 s is
/// ample for an honest peer on a LAN; a stalled peer is evicted after this.
pub const PAKE_EXCHANGE_TIMEOUT: Duration = Duration::from_secs(30);

/// Bootstrap wire-protocol version advertised in the post-handshake metadata
/// extension (frame 10). Bumped when the metadata frame layout changes. A peer
/// that does not send a version frame at all is treated as a pre-extension
/// (legacy) peer and the metadata step is skipped.
///
/// History:
/// * `1` — device-metadata extension (`PeerMeta`, frames 10/11).
/// * `2` — sync-provisioning extension (`SyncProvisioning`, frames 12/13),
///   exchanged AFTER the `PeerMeta` frames. The version byte sent in frame 10
///   gates whether the peer also sends/expects the provisioning frames: a peer
///   advertising `>= 2` participates; a peer advertising `1` (or no version
///   frame at all) neither sends nor reads them, so pairing still succeeds with
///   `peer_provisioning = None` on both sides.
pub const BOOTSTRAP_PROTO_VERSION: u8 = 2;

/// Fixed, well-known PAKE password for the QR-less LAN/SAS *discovery* pairing
/// path — shared by every initiator and responder (macOS daemon and Android).
///
/// The discovery path has NO pre-shared secret, so PAKE alone cannot
/// authenticate — the human SAS comparison does. opaque-ke is an ASYMMETRIC
/// PAKE: the initiator's `ClientLogin` only `finish`es against a `PasswordFile`
/// registered for the IDENTICAL password (a mismatch fails at frame 7, before
/// any SAS is derived). Both ends therefore agree on this constant up front and
/// rely ENTIRELY on the post-channel-binding SAS compare for authentication
/// (Bluetooth numeric-comparison / Magic-Wormhole verifier pattern): a MitM
/// substituting its own per-leg session yields a different `bound_key` → a
/// different SAS → the two humans see a mismatch and abort.
///
/// This is NON-SECRET by design — publishing it changes nothing, because the
/// SAS, not the password, gates trust and persistence. It MUST NOT be logged
/// (not for secrecy, but to keep pairing logs clean), and it MUST be byte-equal
/// across all platforms so the OPAQUE inputs / channel-binding checksums match.
/// QR pairing does NOT use this — that path derives its password from the QR
/// token (`PairingToken::to_pake_password`) and is unaffected.
pub const DISCOVERY_PAIRING_PASSWORD: &str = "copypaste/p2p/lan-sas-discovery/v1";
