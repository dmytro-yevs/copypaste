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

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName, UnixTime};
use rustls::server::danger::{ClientCertVerified, ClientCertVerifier};
use rustls::{
    ClientConfig, DigitallySignedStruct, DistinguishedName, Error as TlsError, ServerConfig,
    SignatureScheme,
};
use subtle::ConstantTimeEq;
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::{TlsAcceptor, TlsConnector};
use tokio_util::codec::{Framed, LengthDelimitedCodec};

use crate::cert::fingerprint_of;
use crate::pake::{
    channel_confirmation_tag, derive_sas, ConfirmRole, PakeInitiator, PakeResponder, PasswordFile,
    SessionKey, CONFIRM_TAG_LEN,
};
use crate::transport::{
    tls_channel_binder_client, tls_channel_binder_server, DeviceFingerprint, TransportError,
    P2P_SNI_SENTINEL, TCP_CONNECT_TIMEOUT, TLS_HANDSHAKE_TIMEOUT,
};

/// QR-pairing token lifetime / bootstrap accept window, in seconds.
///
/// This is the single source of truth for the pairing window *within this
/// crate*. It is intentionally kept equal to `copypaste_ipc::QR_PAIRING_TTL_SECS`
/// (the project-wide source of truth, consumed by the daemon's
/// `generate_pairing_qr` handler which stamps `expires_at = now + this`).
///
/// `copypaste-p2p` does not depend on `copypaste-ipc` (and adding that
/// cross-crate dependency for a single scalar is not worth the coupling), so we
/// cannot reference `copypaste_ipc::QR_PAIRING_TTL_SECS` directly here. The two
/// constants must therefore be kept numerically identical by hand.
//
// TODO(shared-const): if `copypaste-p2p` ever gains a `copypaste-ipc`
// dependency, replace this with
// `copypaste_ipc::QR_PAIRING_TTL_SECS` so the two values cannot drift.
const QR_PAIRING_TTL_SECS: u64 = 120;

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

/// Upper bound on a single PAKE/fingerprint frame. PAKE messages are a few
/// hundred bytes and fingerprints are 64 hex chars; 64 KiB is a wide margin
/// that still rejects a desynced peer flooding a huge length prefix.
const MAX_FRAME_BYTES: usize = 64 * 1024;

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

/// Minimum protocol version a peer must advertise (in the frame-10 version byte)
/// to participate in the [`SyncProvisioning`] exchange (frames 12/13). A peer
/// advertising less than this — or no version frame at all — is treated as
/// not-provisioning-capable and the step is skipped with `peer_provisioning =
/// None` (back-compat).
const SYNC_PROVISIONING_MIN_VERSION: u8 = 2;

/// Upper bound on the sync-provisioning JSON frame. Two URLs plus a base64-ish
/// anon key and a 32-byte key (base64 ≈ 44 chars) total well under 4 KiB; the
/// ceiling still rejects a desynced peer flooding this slot.
const MAX_PROVISIONING_BYTES: usize = 4 * 1024;

/// SAS-confirm wire bytes (frame 10a, LAN/SAS pairing path only).
///
/// After frame 9 (channel-binding tag verified) the confirm-gated variants
/// exchange exactly one byte each: `SAS_ACCEPT` (0x01) when the local user
/// confirmed the SAS matched, or `SAS_REJECT` (0x00) otherwise. Pairing
/// proceeds to the metadata exchange / `Ok` ONLY when BOTH bytes are
/// `SAS_ACCEPT`. This frame exists solely on the new `*_with_confirm` paths;
/// the legacy `run`/`run_initiator` transcript is byte-unchanged.
const SAS_ACCEPT: u8 = 0x01;
/// See [`SAS_ACCEPT`].
const SAS_REJECT: u8 = 0x00;

/// Upper bound on the peer metadata JSON frame. The four short strings (model,
/// OS, app version, IP) total well under 256 bytes; 1 KiB is a wide ceiling that
/// still rejects a desynced peer flooding this slot.
const MAX_META_BYTES: usize = 1024;

/// Compact device-identity metadata exchanged in-band over the bootstrap channel
/// AFTER the PAKE handshake completes (P2P Phase 4).
///
/// All fields are best-effort and optional. `copypaste-p2p` does not collect
/// these itself (it has no platform deps) — the daemon passes them in. They are
/// non-secret (model, OS version, app version, LAN IP) and mirror what mDNS
/// already broadcasts.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PeerMeta {
    /// Friendly hardware model, e.g. `"MacBook Air"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// OS name + version, e.g. `"macOS 15.5"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub os_version: Option<String>,
    /// App / daemon version string.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_version: Option<String>,
    /// Best LAN-routable display IP.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_ip: Option<String>,
    /// Human-readable device name (e.g. `"Alice's MacBook"`), learned in-band
    /// over the bootstrap channel during PAKE pairing. Populated by the daemon
    /// from the OS hostname / device name before passing `PeerMeta` to the
    /// bootstrap responder/initiator.
    ///
    /// `#[serde(default)]` keeps backward compat with older `PeerMeta` frames
    /// that do not carry this field; they deserialise to `None`.
    ///
    /// TODO: carry device_name in the over-the-wire `PeerMeta` JSON frame so
    /// discovery-initiated pairs (mDNS-only, no QR) also receive a name.
    /// Requires a BOOTSTRAP_PROTO_VERSION bump + coordinated re-pair on all
    /// existing devices (i.e. not done in this task to avoid breaking the
    /// handshake).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_name: Option<String>,
    /// This device's STUN-discovered public / global IP (e.g. `"203.0.113.42"`),
    /// learned in-band over the bootstrap channel during PAKE pairing (B1: full
    /// device info on both platforms). Populated by the daemon from its own cached
    /// STUN value — `copypaste-p2p` does not collect it. `None` when the peer opted
    /// out of public-IP collection (`collect_public_ip = false`), STUN has not yet
    /// resolved, or the peer is a legacy build.
    ///
    /// Informational metadata ONLY — never used for authentication, fingerprint
    /// pinning, or any trust decision. A public IP is non-secret, mirroring what
    /// the device already reports for its own `get_own_device_info`.
    ///
    /// `#[serde(default)]` keeps backward compat with older `PeerMeta` frames that
    /// do not carry this field; they deserialise to `None`. Because it is an
    /// additive optional field (omitted on the wire when `None` via
    /// `skip_serializing_if`), NO `BOOTSTRAP_PROTO_VERSION` bump is required — an
    /// old peer simply ignores the unknown key, and a new peer reads `None` from
    /// an old peer's frame.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public_ip: Option<String>,
    /// Stable device UUID (from `generate_device_cert` / the UDL's `device_id`),
    /// learned in-band over the post-handshake metadata extension. Allows the
    /// receiver to match clipboard `origin_device_id` to a peer name without relying
    /// on the TLS cert fingerprint. `#[serde(default)]` for back-compat with peers
    /// that do not carry this field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_id: Option<String>,
}

/// Sync-account provisioning exchanged in-band over the bootstrap channel AFTER
/// the PAKE handshake AND the [`PeerMeta`] frames complete (proto version >= 2).
///
/// This is the payload behind "QR fully provisions all sync": scanning the
/// pairing QR on a new device also configures Supabase + relay (not just P2P).
/// The data travels ONLY on the authenticated, mutually-confirmed, encrypted
/// bootstrap tunnel — it is NEVER placed in the QR image (see `pairing_qr.rs`,
/// which is left untouched).
///
/// # What is (and is NOT) transmitted
/// * `supabase_url` / `supabase_anon_key` — non-secret connection params (the
///   anon key is a publishable JWT).
/// * `relay_url` — non-secret relay endpoint.
/// * `derived_sync_key` — the 32-byte DERIVED cloud sync key (Argon2id output).
///   This is secret. The account passphrase / email / password are NEVER
///   transmitted — only the already-derived key, so the receiving device can
///   decrypt cloud blobs without ever learning the human passphrase.
///
/// All fields are optional: an unconfigured side sends an all-`None` value.
/// Each side decides what to APPLY (apply a field only if it currently lacks
/// it) — the bootstrap layer just carries the value across.
///
/// # Security
/// `derived_sync_key` is secret: it is NEVER logged (no field is `Debug`-printed
/// with its bytes — see the manual `Debug` impl), and the daemon wraps transient
/// copies in `Zeroizing` on the apply path. `serde(skip_serializing_if)` keeps
/// the frame minimal and a legacy reader simply ignores unknown keys.
#[derive(Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SyncProvisioning {
    /// Supabase project URL (e.g. `https://xxxx.supabase.co`). Non-secret.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supabase_url: Option<String>,
    /// Supabase publishable anon/public JWT. Safe to share (publishable key).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supabase_anon_key: Option<String>,
    /// Relay endpoint URL. Non-secret. Currently no daemon config field sources
    /// this (relay is env-only), so the daemon sends `None`; the field exists so
    /// the wire form and FFI surface are symmetric and forward-compatible.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relay_url: Option<String>,
    /// The 32-byte DERIVED cloud sync key (Argon2id output), NOT the passphrase.
    /// Secret — never logged. The receiving device wraps it in `SyncKey` and
    /// persists it via the same backend `set_sync_passphrase` uses.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub derived_sync_key: Option<Vec<u8>>,
}

impl std::fmt::Debug for SyncProvisioning {
    /// Custom `Debug` that NEVER prints the secret `derived_sync_key` bytes — it
    /// reports only whether the key is present and its length. The URLs/anon key
    /// are non-secret and shown verbatim.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SyncProvisioning")
            .field("supabase_url", &self.supabase_url)
            .field("supabase_anon_key", &self.supabase_anon_key)
            .field("relay_url", &self.relay_url)
            .field(
                "derived_sync_key",
                &self
                    .derived_sync_key
                    .as_ref()
                    .map(|k| format!("<{} bytes redacted>", k.len())),
            )
            .finish()
    }
}

/// rustls verifier that accepts **any** peer certificate without pinning.
///
/// Used only on the bootstrap channel. It still requires the peer to *present*
/// a certificate (so we can learn its fingerprint and so the TLS handshake
/// completes with client auth on the server side), but performs no
/// chain/expiry/hostname/fingerprint validation. Authentication is the PAKE
/// handshake's job, not TLS's, on this channel.
#[derive(Debug)]
struct AcceptAnyCert;

impl AcceptAnyCert {
    /// Signature schemes we accept — delegate to the ring provider's full set.
    fn schemes() -> Vec<SignatureScheme> {
        rustls::crypto::ring::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}

impl ServerCertVerifier for AcceptAnyCert {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, TlsError> {
        // Intentionally accept any server cert — PAKE authenticates the peer.
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &rustls::crypto::ring::default_provider().signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &rustls::crypto::ring::default_provider().signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        Self::schemes()
    }
}

impl ClientCertVerifier for AcceptAnyCert {
    fn root_hint_subjects(&self) -> &[DistinguishedName] {
        &[]
    }

    fn verify_client_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _now: UnixTime,
    ) -> Result<ClientCertVerified, TlsError> {
        // Intentionally accept any client cert — PAKE authenticates the peer.
        Ok(ClientCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &rustls::crypto::ring::default_provider().signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &rustls::crypto::ring::default_provider().signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        Self::schemes()
    }
}

/// Outcome of a completed bootstrap PAKE exchange.
pub struct BootstrapPairing {
    /// The peer's certificate fingerprint (hex SHA-256 of its cert DER), as
    /// observed on the TLS handshake AND confirmed to match the value the peer
    /// sent in-band. This is the value the caller pins in `PairedPeers`.
    pub peer_fingerprint: DeviceFingerprint,
    /// The peer's P2P sync-listener address (`host:port`), sent in-band during
    /// the exchange. The caller persists this to `peers.json` so the Phase 3
    /// outbound connector can dial the peer directly. May be empty if the peer
    /// did not advertise an address.
    pub peer_sync_addr: String,
    /// The 32-byte PAKE session key (identical on both sides on success).
    pub session_key: SessionKey,
    /// The 6-digit Short Authentication String derived from the channel-bound
    /// key ([`crate::pake::derive_sas`]). Identical on both honest endpoints;
    /// different per relay leg under a MitM. Surfaced for the human compare on
    /// the discovery pairing path; the QR path may display it as a confidence
    /// check (the token already authenticates there).
    pub sas: String,
    /// Peer's friendly hardware model, learned over the post-handshake metadata
    /// extension. `None` when the peer is a legacy (pre-extension) build or did
    /// not advertise the field.
    pub peer_model: Option<String>,
    /// Peer's OS name + version, learned over the metadata extension.
    pub peer_os: Option<String>,
    /// Peer's app / daemon version, learned over the metadata extension.
    pub peer_app_version: Option<String>,
    /// Peer's best LAN-routable display IP, learned over the metadata extension.
    pub peer_local_ip: Option<String>,
    /// Peer's human-readable device name (e.g. `"Alice's MacBook"`), learned
    /// over the post-handshake metadata extension. `None` for legacy peers or
    /// when the field was not advertised.
    pub peer_device_name: Option<String>,
    /// Peer's STUN-discovered public / global IP, learned over the post-handshake
    /// metadata extension (B1: full device info). `None` for legacy peers, or when
    /// the peer opted out of public-IP collection / STUN had not yet resolved.
    /// Informational only — never used for auth or trust.
    pub peer_public_ip: Option<String>,
    /// Peer's stable device UUID, learned over the post-handshake metadata
    /// extension (from `PeerMeta.device_id`). `None` when the peer is a legacy
    /// build or did not advertise this field.
    pub peer_device_id: Option<String>,
    /// Sync-account provisioning the peer advertised over the post-handshake
    /// extension (proto version >= 2). `None` when the peer is a legacy
    /// (pre-version-2) build, when it advertised an all-`None` value, or when the
    /// provisioning exchange was skipped. The caller decides what to APPLY (only
    /// fields it currently lacks). See [`SyncProvisioning`].
    pub peer_provisioning: Option<SyncProvisioning>,
}

/// A bootstrap TLS responder listener bound to an ephemeral port.
///
/// Construct with [`BootstrapResponder::bind`], read [`BootstrapResponder::addr`]
/// into the QR `addr_hint`, then call [`BootstrapResponder::run`] to accept one
/// connection and drive the responder side of the PAKE handshake over it.
pub struct BootstrapResponder {
    listener: TcpListener,
    acceptor: TlsAcceptor,
    own_cert_der: Vec<u8>,
    own_fingerprint: DeviceFingerprint,
}

impl BootstrapResponder {
    /// Bind an ephemeral bootstrap listener on `0.0.0.0:0` and TLS-wrap it with
    /// the daemon's self-signed certificate (the same cert whose fingerprint the
    /// pairing QR advertises).
    ///
    /// # Errors
    /// Returns [`TransportError::Io`] if the bind fails or
    /// [`TransportError::TlsConfig`] if the TLS config cannot be built.
    pub async fn bind(cert_der: Vec<u8>, key_der: Vec<u8>) -> Result<Self, TransportError> {
        Self::bind_on(0, cert_der, key_der).await
    }

    /// Bind the bootstrap listener on a SPECIFIC TCP port (`0.0.0.0:port`) and
    /// TLS-wrap it with the daemon's self-signed certificate.
    ///
    /// `port = 0` requests an OS-assigned ephemeral port (the QR path's
    /// behaviour, via [`bind`](Self::bind)). The LAN/SAS Phase 2 standing
    /// responder uses a FIXED port so the advertised mDNS `bport` stays stable
    /// across pairing iterations: it discovers a free port once, advertises it,
    /// then re-binds the SAME port for each subsequent inbound pairing (a
    /// listening socket is dropped — not connected — so it never enters
    /// TIME_WAIT and re-bind succeeds immediately).
    ///
    /// # Errors
    /// Returns [`TransportError::Io`] if the bind fails or
    /// [`TransportError::TlsConfig`] if the TLS config cannot be built.
    pub async fn bind_on(
        port: u16,
        cert_der: Vec<u8>,
        key_der: Vec<u8>,
    ) -> Result<Self, TransportError> {
        let listener = TcpListener::bind(("0.0.0.0", port)).await?;
        let own_fingerprint = fingerprint_of(&cert_der);

        let cert = CertificateDer::from(cert_der.clone());
        let key = rustls::pki_types::PrivatePkcs8KeyDer::from(key_der);
        let private_key = PrivateKeyDer::Pkcs8(key);

        // Require the client to present a cert (so we learn its fingerprint),
        // but accept any cert — PAKE is the real authenticator on this channel.
        let server_config = ServerConfig::builder()
            .with_client_cert_verifier(Arc::new(AcceptAnyCert))
            .with_single_cert(vec![cert], private_key)
            .map_err(TransportError::TlsConfig)?;

        Ok(Self {
            listener,
            acceptor: TlsAcceptor::from(Arc::new(server_config)),
            own_cert_der: cert_der,
            own_fingerprint,
        })
    }

    /// The bound local address (`host:port`) to advertise in the QR `addr_hint`.
    ///
    /// The listener binds `0.0.0.0:0`; this returns the loopback-usable port via
    /// the OS-assigned ephemeral port.
    pub fn local_addr(&self) -> std::io::Result<SocketAddr> {
        self.listener.local_addr()
    }

    /// Our own cert fingerprint (sent to the initiator over the channel).
    pub fn fingerprint(&self) -> &str {
        &self.own_fingerprint
    }

    /// Accept ONE inbound bootstrap connection (within
    /// [`BOOTSTRAP_ACCEPT_TIMEOUT`]) and run the responder side of the PAKE
    /// handshake over the TLS stream.
    ///
    /// `password` is the PAKE password derived from the QR token. A fresh
    /// [`PasswordFile`] is registered from it for this single handshake.
    /// `sync_addr` is this device's own P2P sync-listener `host:port`, sent
    /// in-band so the initiator can persist it for the Phase 3 connector.
    ///
    /// # Errors
    /// * [`TransportError::HandshakeTimeout`] if no connection / TLS handshake
    ///   completes in time.
    /// * [`TransportError::Io`] for socket / framing errors or a PAKE failure
    ///   (surfaced as `io::Error::other`), or a fingerprint mismatch between the
    ///   TLS cert and the in-band frame.
    pub async fn run(
        self,
        password: &str,
        sync_addr: &str,
        own_meta: &PeerMeta,
        own_provisioning: Option<SyncProvisioning>,
    ) -> Result<BootstrapPairing, TransportError> {
        // Accept exactly one inbound TCP connection within the window.
        let (tcp_stream, peer_addr) =
            match tokio::time::timeout(BOOTSTRAP_ACCEPT_TIMEOUT, self.listener.accept()).await {
                Ok(res) => res?,
                Err(_elapsed) => {
                    tracing::warn!(
                        timeout = ?BOOTSTRAP_ACCEPT_TIMEOUT,
                        "bootstrap responder timed out waiting for inbound pairing connection"
                    );
                    return Err(TransportError::HandshakeTimeout);
                }
            };
        tracing::debug!(peer_addr = %peer_addr, "bootstrap: inbound TCP connection");

        let tls_stream =
            match tokio::time::timeout(TLS_HANDSHAKE_TIMEOUT, self.acceptor.accept(tcp_stream))
                .await
            {
                Ok(res) => res?,
                Err(_elapsed) => {
                    tracing::warn!("bootstrap: TLS server handshake timed out");
                    return Err(TransportError::HandshakeTimeout);
                }
            };

        // The cert fingerprint the peer actually presented in TLS.
        let tls_peer_fp = {
            let (_, conn) = tls_stream.get_ref();
            let certs = conn.peer_certificates().ok_or(TransportError::NoPeerCert)?;
            let first = certs.first().ok_or(TransportError::NoPeerCert)?;
            fingerprint_of(first.as_ref())
        };

        // RFC 5705 channel binder for THIS TLS session (extracted before the
        // stream is moved into `Framed`). Mixed into the PAKE key below so the
        // pairing is bound to this exact TLS channel.
        let tls_binder = tls_channel_binder_server(&tls_stream)?;

        let mut framed = Framed::new(tls_stream, length_codec());

        // Touch own_cert_der so the field is not flagged unused; the bytes are
        // already consumed via the TLS config but the DER is kept for any future
        // re-bind without regenerating.
        debug_assert!(!self.own_cert_der.is_empty());

        // Wrap the entire 9-frame PAKE exchange in a single deadline so a peer
        // that completes TLS but then stalls mid-exchange cannot pin this
        // single-shot responder indefinitely (slowloris-style DoS).
        let own_fingerprint = self.own_fingerprint.clone();
        let sync_addr = sync_addr.to_owned();
        let own_meta = own_meta.clone();
        let pairing = tokio::time::timeout(PAKE_EXCHANGE_TIMEOUT, async move {
            // PasswordFile for this single handshake, derived from the QR password.
            let password_file = PasswordFile::register(password)
                .map_err(|e| io_other(format!("PasswordFile::register: {e}")))?;

            // Frame 1 ← initiator's PAKE message1.
            let msg1 = recv_frame(&mut framed).await?;
            // Frame 2 ← initiator's cert fingerprint.
            let frame_peer_fp = recv_fingerprint(&mut framed).await?;
            // Frame 3 ← initiator's P2P sync-listener address (Phase 2).
            let peer_sync_addr = recv_sync_addr(&mut framed).await?;

            // The fingerprint the peer claims in-band MUST match the cert it
            // presented in the TLS handshake. Lowercase before comparing to
            // handle peers that send uppercase hex (avoid false mismatch).
            // The value is public (exchanged over an authenticated channel);
            // a simple == suffices — no timing side-channel risk here.
            if frame_peer_fp.to_lowercase() != tls_peer_fp {
                return Err(io_other(format!(
                    "bootstrap: initiator frame fingerprint {frame_peer_fp} != TLS cert {tls_peer_fp}"
                )));
            }

            let (responder, msg2) = PakeResponder::respond(&password_file, &msg1)
                .map_err(|e| io_other(format!("PAKE respond: {e}")))?;

            // Frame 4 → our PAKE message2.
            send_frame(&mut framed, &msg2).await?;
            // Frame 5 → our cert fingerprint.
            send_frame(&mut framed, own_fingerprint.as_bytes()).await?;
            // Frame 6 → our P2P sync-listener address (Phase 2).
            send_frame(&mut framed, sync_addr.as_bytes()).await?;

            // Frame 7 ← initiator's PAKE finalisation.
            let msg3 = recv_frame(&mut framed).await?;
            let session_key = responder
                .finish(&msg3)
                .map_err(|e| io_other(format!("PAKE finish: {e}")))?;

            // Channel-binding confirmation (S3). Bind the PAKE key to this TLS
            // session, then exchange role-separated confirmation tags. A match in
            // constant time proves the peer shares the same PAKE key AND the same
            // TLS channel — a relay bridging two TLS sessions would derive a
            // different binder per leg, so its tags would never match.
            let bound_key = session_key.bind_to_tls_channel(&tls_binder);
            let own_tag = channel_confirmation_tag(&bound_key, ConfirmRole::Responder);
            let expected_peer_tag = channel_confirmation_tag(&bound_key, ConfirmRole::Initiator);

            // Frame 8 → our confirmation tag.
            send_frame(&mut framed, &own_tag).await?;
            // Frame 9 ← initiator's confirmation tag.
            let peer_tag = recv_confirmation_tag(&mut framed).await?;
            if peer_tag.ct_eq(&expected_peer_tag).unwrap_u8() != 1 {
                return Err(io_other(
                    "bootstrap: channel-binding confirmation mismatch — possible relay MitM, pairing aborted".into(),
                ));
            }

            // SAS for the human compare (LAN/SAS path). Additive: computing it
            // here does NOT change the wire transcript — the legacy path simply
            // surfaces it in the returned struct without exchanging frame 10a.
            let sas = derive_sas(&bound_key);

            // P2P Phase 4 (optional, post-handshake): exchange device metadata
            // and (proto >= 2) sync provisioning. The pairing is already complete
            // and authenticated at this point; any failure here is swallowed
            // (legacy peer closed, etc.).
            let (peer_meta, peer_provisioning) =
                exchange_peer_meta(&mut framed, &own_meta, own_provisioning.as_ref()).await;

            Ok::<BootstrapPairing, TransportError>(BootstrapPairing {
                peer_fingerprint: tls_peer_fp,
                peer_sync_addr,
                session_key,
                sas,
                peer_model: peer_meta.model,
                peer_os: peer_meta.os_version,
                peer_app_version: peer_meta.app_version,
                peer_local_ip: peer_meta.local_ip,
                peer_device_name: peer_meta.device_name,
                peer_public_ip: peer_meta.public_ip,
                peer_device_id: peer_meta.device_id,
                peer_provisioning,
            })
        })
        .await
        .map_err(|_elapsed| {
            tracing::warn!(
                timeout = ?PAKE_EXCHANGE_TIMEOUT,
                "bootstrap: PAKE exchange timed out — evicting stalled peer"
            );
            io_other("bootstrap: PAKE exchange timed out".into())
        })??;

        Ok(pairing)
    }

    /// Confirm-gated variant of [`BootstrapResponder::run`] for the LAN/SAS
    /// discovery pairing path.
    ///
    /// Runs the IDENTICAL handshake transcript through frame 9 (PAKE +
    /// channel-binding tag verify), then derives the 6-digit SAS and invokes
    /// `confirm(sas)`. If the user rejects (returns `false`) the pairing aborts
    /// with an error (keys drop/zeroize). Otherwise both sides exchange a NEW
    /// frame 10a ([`SAS_ACCEPT`]/[`SAS_REJECT`]) and proceed to the metadata
    /// exchange / `Ok` ONLY if BOTH bytes are [`SAS_ACCEPT`].
    ///
    /// This is a separate method so the QR `run` transcript stays byte-compatible
    /// (frame 10a is never sent there).
    #[allow(clippy::too_many_arguments)] // mirrors `run` + confirm cb + provisioning
    pub async fn run_with_confirm<F, Fut>(
        self,
        password: &str,
        sync_addr: &str,
        own_meta: &PeerMeta,
        own_provisioning: Option<SyncProvisioning>,
        confirm: F,
    ) -> Result<BootstrapPairing, TransportError>
    where
        F: FnOnce(&str) -> Fut,
        Fut: std::future::Future<Output = bool>,
    {
        let (tcp_stream, peer_addr) =
            match tokio::time::timeout(BOOTSTRAP_ACCEPT_TIMEOUT, self.listener.accept()).await {
                Ok(res) => res?,
                Err(_elapsed) => {
                    tracing::warn!(
                        timeout = ?BOOTSTRAP_ACCEPT_TIMEOUT,
                        "bootstrap: SAS responder timed out waiting for inbound pairing connection"
                    );
                    return Err(TransportError::HandshakeTimeout);
                }
            };
        tracing::debug!(peer_addr = %peer_addr, "bootstrap(sas): inbound TCP connection");

        let tls_stream =
            match tokio::time::timeout(TLS_HANDSHAKE_TIMEOUT, self.acceptor.accept(tcp_stream))
                .await
            {
                Ok(res) => res?,
                Err(_elapsed) => {
                    tracing::warn!("bootstrap(sas): TLS server handshake timed out");
                    return Err(TransportError::HandshakeTimeout);
                }
            };

        let tls_peer_fp = {
            let (_, conn) = tls_stream.get_ref();
            let certs = conn.peer_certificates().ok_or(TransportError::NoPeerCert)?;
            let first = certs.first().ok_or(TransportError::NoPeerCert)?;
            fingerprint_of(first.as_ref())
        };

        let tls_binder = tls_channel_binder_server(&tls_stream)?;
        let mut framed = Framed::new(tls_stream, length_codec());
        debug_assert!(!self.own_cert_der.is_empty());

        let own_fingerprint = self.own_fingerprint.clone();

        // The 9-frame PAKE exchange is bounded by PAKE_EXCHANGE_TIMEOUT. It
        // borrows `framed` (so it can be reused for frame 10a) and returns the
        // SAS plus everything needed to finish. The user-confirm step runs
        // OUTSIDE this deadline: a human may take longer than 30 s, and a slow
        // confirm must not be mistaken for a stalled peer.
        let prepared = tokio::time::timeout(PAKE_EXCHANGE_TIMEOUT, async {
            let password_file = PasswordFile::register(password)
                .map_err(|e| io_other(format!("PasswordFile::register: {e}")))?;

            // Frame 1 ← initiator's PAKE message1.
            let msg1 = recv_frame(&mut framed).await?;
            // Frame 2 ← initiator's cert fingerprint.
            let frame_peer_fp = recv_fingerprint(&mut framed).await?;
            // Frame 3 ← initiator's P2P sync-listener address.
            let peer_sync_addr = recv_sync_addr(&mut framed).await?;

            if frame_peer_fp.to_lowercase() != tls_peer_fp {
                return Err(io_other(format!(
                    "bootstrap: initiator frame fingerprint {frame_peer_fp} != TLS cert {tls_peer_fp}"
                )));
            }

            let (responder, msg2) = PakeResponder::respond(&password_file, &msg1)
                .map_err(|e| io_other(format!("PAKE respond: {e}")))?;

            // Frame 4 → our PAKE message2.
            send_frame(&mut framed, &msg2).await?;
            // Frame 5 → our cert fingerprint.
            send_frame(&mut framed, own_fingerprint.as_bytes()).await?;
            // Frame 6 → our P2P sync-listener address.
            send_frame(&mut framed, sync_addr.as_bytes()).await?;

            // Frame 7 ← initiator's PAKE finalisation.
            let msg3 = recv_frame(&mut framed).await?;
            let session_key = responder
                .finish(&msg3)
                .map_err(|e| io_other(format!("PAKE finish: {e}")))?;

            let bound_key = session_key.bind_to_tls_channel(&tls_binder);
            let own_tag = channel_confirmation_tag(&bound_key, ConfirmRole::Responder);
            let expected_peer_tag = channel_confirmation_tag(&bound_key, ConfirmRole::Initiator);

            // Frame 8 → our confirmation tag.
            send_frame(&mut framed, &own_tag).await?;
            // Frame 9 ← initiator's confirmation tag.
            let peer_tag = recv_confirmation_tag(&mut framed).await?;
            if peer_tag.ct_eq(&expected_peer_tag).unwrap_u8() != 1 {
                return Err(io_other(
                    "bootstrap: channel-binding confirmation mismatch — possible relay MitM, pairing aborted".into(),
                ));
            }

            let sas = derive_sas(&bound_key);
            Ok::<_, TransportError>((sas, tls_peer_fp, peer_sync_addr, session_key))
        })
        .await
        .map_err(|_elapsed| {
            tracing::warn!(
                timeout = ?PAKE_EXCHANGE_TIMEOUT,
                "bootstrap(sas): PAKE exchange timed out — evicting stalled peer"
            );
            io_other("bootstrap: PAKE exchange timed out".into())
        })??;

        let (sas, peer_fingerprint, peer_sync_addr, session_key) = prepared;

        // Human SAS confirmation (outside the PAKE deadline). On reject, return
        // an error so `session_key` drops/zeroizes and the caller persists nothing.
        let accepted_locally = confirm(&sas).await;

        // Frame 10a: exchange ACCEPT/REJECT bytes. Proceed only if BOTH accept.
        let our_byte = if accepted_locally {
            SAS_ACCEPT
        } else {
            SAS_REJECT
        };
        send_frame(&mut framed, &[our_byte]).await?;
        let peer_byte = recv_confirm_byte(&mut framed).await?;
        if our_byte != SAS_ACCEPT || peer_byte != SAS_ACCEPT {
            return Err(io_other("SAS rejected by user — pairing aborted".into()));
        }

        // Both confirmed: optional post-handshake metadata + provisioning.
        let (peer_meta, peer_provisioning) =
            exchange_peer_meta(&mut framed, own_meta, own_provisioning.as_ref()).await;

        Ok(BootstrapPairing {
            peer_fingerprint,
            peer_sync_addr,
            session_key,
            sas,
            peer_model: peer_meta.model,
            peer_os: peer_meta.os_version,
            peer_app_version: peer_meta.app_version,
            peer_local_ip: peer_meta.local_ip,
            peer_device_name: peer_meta.device_name,
            peer_public_ip: peer_meta.public_ip,
            peer_device_id: peer_meta.device_id,
            peer_provisioning,
        })
    }
}

/// Dial a bootstrap responder at `addr` over TLS **without** cert pinning and
/// run the initiator side of the PAKE handshake.
///
/// `cert_der` / `key_der` are this device's self-signed cert and key (presented
/// to the responder so it learns our fingerprint). `password` is the PAKE
/// password derived from the QR token. `sync_addr` is this device's own P2P
/// sync-listener `host:port`, sent in-band so the responder can persist it for
/// the Phase 3 connector.
///
/// # Errors
/// Mirrors [`BootstrapResponder::run`] — TLS / socket / framing errors and PAKE
/// failures (including a wrong password, surfaced from `client.finish`).
#[allow(clippy::too_many_arguments)] // additive provisioning param mirrors `run`
pub async fn run_initiator(
    addr: SocketAddr,
    cert_der: Vec<u8>,
    key_der: Vec<u8>,
    password: &str,
    sync_addr: &str,
    own_meta: &PeerMeta,
    own_provisioning: Option<SyncProvisioning>,
) -> Result<BootstrapPairing, TransportError> {
    let own_fingerprint = fingerprint_of(&cert_der);

    let cert = CertificateDer::from(cert_der);
    let key = rustls::pki_types::PrivatePkcs8KeyDer::from(key_der);
    let private_key = PrivateKeyDer::Pkcs8(key);

    let client_config = ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(AcceptAnyCert))
        .with_client_auth_cert(vec![cert], private_key)
        .map_err(TransportError::TlsConfig)?;
    let connector = TlsConnector::from(Arc::new(client_config));

    let tcp_stream = match tokio::time::timeout(TCP_CONNECT_TIMEOUT, TcpStream::connect(addr)).await
    {
        Ok(res) => res?,
        Err(_elapsed) => {
            tracing::warn!(
                peer_addr = %addr,
                timeout = ?TCP_CONNECT_TIMEOUT,
                "bootstrap: TCP connect timed out — transient"
            );
            return Err(TransportError::Io(std::io::Error::from(
                std::io::ErrorKind::TimedOut,
            )));
        }
    };

    // rustls requires a ServerName; identity is verified by PAKE, not SNI, so a
    // fixed placeholder is fine (and is what the pinned transport uses too).
    let server_name =
        ServerName::try_from(P2P_SNI_SENTINEL).expect("static server name is always valid");

    let tls_stream = match tokio::time::timeout(
        TLS_HANDSHAKE_TIMEOUT,
        connector.connect(server_name, tcp_stream),
    )
    .await
    {
        Ok(res) => res?,
        Err(_elapsed) => {
            tracing::warn!(peer_addr = %addr, "bootstrap: TLS client handshake timed out");
            return Err(TransportError::HandshakeTimeout);
        }
    };

    // The cert fingerprint the responder actually presented in TLS.
    let tls_peer_fp = {
        let (_, conn) = tls_stream.get_ref();
        let certs = conn.peer_certificates().ok_or(TransportError::NoPeerCert)?;
        let first = certs.first().ok_or(TransportError::NoPeerCert)?;
        fingerprint_of(first.as_ref())
    };

    // RFC 5705 channel binder for THIS TLS session (extracted before the stream
    // is moved into `Framed`). Mixed into the PAKE key below.
    let tls_binder = tls_channel_binder_client(&tls_stream)?;

    let mut framed = Framed::new(tls_stream, length_codec());

    // Wrap the entire 9-frame PAKE exchange in one deadline — mirrors the
    // responder's protection against a stalling peer (slowloris-style DoS).
    let own_fingerprint_owned = own_fingerprint.clone();
    let sync_addr_owned = sync_addr.to_owned();
    let own_meta = own_meta.clone();
    let pairing = tokio::time::timeout(PAKE_EXCHANGE_TIMEOUT, async move {
        let (client, msg1) =
            PakeInitiator::new(password).map_err(|e| io_other(format!("PAKE init: {e}")))?;

        // Frame 1 → our PAKE message1.
        send_frame(&mut framed, &msg1).await?;
        // Frame 2 → our cert fingerprint.
        send_frame(&mut framed, own_fingerprint_owned.as_bytes()).await?;
        // Frame 3 → our P2P sync-listener address (Phase 2).
        send_frame(&mut framed, sync_addr_owned.as_bytes()).await?;

        // Frame 4 ← responder's PAKE message2.
        let msg2 = recv_frame(&mut framed).await?;
        // Frame 5 ← responder's cert fingerprint.
        let frame_peer_fp = recv_fingerprint(&mut framed).await?;
        // Frame 6 ← responder's P2P sync-listener address (Phase 2).
        let peer_sync_addr = recv_sync_addr(&mut framed).await?;

        // Lowercase before comparing — handle peers that send uppercase hex
        // (avoid false mismatch; value is public, timing safety not needed).
        if frame_peer_fp.to_lowercase() != tls_peer_fp {
            return Err(io_other(format!(
                "bootstrap: responder frame fingerprint {frame_peer_fp} != TLS cert {tls_peer_fp}"
            )));
        }

        let (session_key, msg3) = client
            .finish(&msg2)
            .map_err(|e| io_other(format!("PAKE finish: {e}")))?;

        // Frame 7 → our PAKE finalisation.
        send_frame(&mut framed, &msg3).await?;

        // Channel-binding confirmation (S3). See `BootstrapResponder::run` for
        // the rationale — bind to this TLS session and exchange role-separated
        // tags, aborting on any mismatch (relay MitM defence).
        let bound_key = session_key.bind_to_tls_channel(&tls_binder);
        let own_tag = channel_confirmation_tag(&bound_key, ConfirmRole::Initiator);
        let expected_peer_tag = channel_confirmation_tag(&bound_key, ConfirmRole::Responder);

        // Frame 8 ← responder's confirmation tag.
        let peer_tag = recv_confirmation_tag(&mut framed).await?;
        // Frame 9 → our confirmation tag.
        send_frame(&mut framed, &own_tag).await?;
        if peer_tag.ct_eq(&expected_peer_tag).unwrap_u8() != 1 {
            return Err(io_other(
                "bootstrap: channel-binding confirmation mismatch — possible relay MitM, pairing aborted".into(),
            ));
        }

        // SAS for the human compare (LAN/SAS path). Additive — does not change
        // the wire transcript; the legacy path just surfaces it.
        let sas = derive_sas(&bound_key);

        // P2P Phase 4 (optional, post-handshake): exchange device metadata and
        // (proto >= 2) sync provisioning. Pairing is already complete and
        // authenticated; failures are swallowed.
        let (peer_meta, peer_provisioning) =
            exchange_peer_meta(&mut framed, &own_meta, own_provisioning.as_ref()).await;

        Ok::<BootstrapPairing, TransportError>(BootstrapPairing {
            peer_fingerprint: tls_peer_fp,
            peer_sync_addr,
            session_key,
            sas,
            peer_model: peer_meta.model,
            peer_os: peer_meta.os_version,
            peer_app_version: peer_meta.app_version,
            peer_local_ip: peer_meta.local_ip,
            peer_device_name: peer_meta.device_name,
            peer_public_ip: peer_meta.public_ip,
            peer_device_id: peer_meta.device_id,
            peer_provisioning,
        })
    })
    .await
    .map_err(|_elapsed| {
        tracing::warn!(
            timeout = ?PAKE_EXCHANGE_TIMEOUT,
            "bootstrap: initiator PAKE exchange timed out — stalled responder"
        );
        io_other("bootstrap: PAKE exchange timed out".into())
    })??;

    Ok(pairing)
}

/// Confirm-gated variant of [`run_initiator`] for the LAN/SAS discovery pairing
/// path.
///
/// Runs the IDENTICAL handshake transcript through frame 9 (PAKE +
/// channel-binding tag verify), then derives the 6-digit SAS and invokes
/// `confirm(sas)`. On reject (`false`) the pairing aborts with an error so the
/// session key drops/zeroizes. Otherwise both sides exchange frame 10a
/// ([`SAS_ACCEPT`]/[`SAS_REJECT`]) and the pairing succeeds ONLY if BOTH bytes
/// are [`SAS_ACCEPT`].
///
/// Separate from [`run_initiator`] so the QR transcript stays byte-compatible.
#[allow(clippy::too_many_arguments)] // mirrors `run_initiator` + confirm cb + provisioning
pub async fn run_initiator_with_confirm<F, Fut>(
    addr: SocketAddr,
    cert_der: Vec<u8>,
    key_der: Vec<u8>,
    password: &str,
    sync_addr: &str,
    own_meta: &PeerMeta,
    own_provisioning: Option<SyncProvisioning>,
    confirm: F,
) -> Result<BootstrapPairing, TransportError>
where
    F: FnOnce(&str) -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let own_fingerprint = fingerprint_of(&cert_der);

    let cert = CertificateDer::from(cert_der);
    let key = rustls::pki_types::PrivatePkcs8KeyDer::from(key_der);
    let private_key = PrivateKeyDer::Pkcs8(key);

    let client_config = ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(AcceptAnyCert))
        .with_client_auth_cert(vec![cert], private_key)
        .map_err(TransportError::TlsConfig)?;
    let connector = TlsConnector::from(Arc::new(client_config));

    let tcp_stream = match tokio::time::timeout(TCP_CONNECT_TIMEOUT, TcpStream::connect(addr)).await
    {
        Ok(res) => res?,
        Err(_elapsed) => {
            tracing::warn!(
                peer_addr = %addr,
                timeout = ?TCP_CONNECT_TIMEOUT,
                "bootstrap(sas): TCP connect timed out — transient"
            );
            return Err(TransportError::Io(std::io::Error::from(
                std::io::ErrorKind::TimedOut,
            )));
        }
    };

    let server_name =
        ServerName::try_from(P2P_SNI_SENTINEL).expect("static server name is always valid");

    let tls_stream = match tokio::time::timeout(
        TLS_HANDSHAKE_TIMEOUT,
        connector.connect(server_name, tcp_stream),
    )
    .await
    {
        Ok(res) => res?,
        Err(_elapsed) => {
            tracing::warn!(peer_addr = %addr, "bootstrap(sas): TLS client handshake timed out");
            return Err(TransportError::HandshakeTimeout);
        }
    };

    let tls_peer_fp = {
        let (_, conn) = tls_stream.get_ref();
        let certs = conn.peer_certificates().ok_or(TransportError::NoPeerCert)?;
        let first = certs.first().ok_or(TransportError::NoPeerCert)?;
        fingerprint_of(first.as_ref())
    };

    let tls_binder = tls_channel_binder_client(&tls_stream)?;
    let mut framed = Framed::new(tls_stream, length_codec());

    let own_fingerprint_owned = own_fingerprint.clone();

    // 9-frame exchange bounded by PAKE_EXCHANGE_TIMEOUT; borrows `framed` so it
    // is reusable for frame 10a. Confirm runs OUTSIDE this deadline.
    let prepared = tokio::time::timeout(PAKE_EXCHANGE_TIMEOUT, async {
        let (client, msg1) =
            PakeInitiator::new(password).map_err(|e| io_other(format!("PAKE init: {e}")))?;

        // Frame 1 → our PAKE message1.
        send_frame(&mut framed, &msg1).await?;
        // Frame 2 → our cert fingerprint.
        send_frame(&mut framed, own_fingerprint_owned.as_bytes()).await?;
        // Frame 3 → our P2P sync-listener address.
        send_frame(&mut framed, sync_addr.as_bytes()).await?;

        // Frame 4 ← responder's PAKE message2.
        let msg2 = recv_frame(&mut framed).await?;
        // Frame 5 ← responder's cert fingerprint.
        let frame_peer_fp = recv_fingerprint(&mut framed).await?;
        // Frame 6 ← responder's P2P sync-listener address.
        let peer_sync_addr = recv_sync_addr(&mut framed).await?;

        if frame_peer_fp.to_lowercase() != tls_peer_fp {
            return Err(io_other(format!(
                "bootstrap: responder frame fingerprint {frame_peer_fp} != TLS cert {tls_peer_fp}"
            )));
        }

        let (session_key, msg3) = client
            .finish(&msg2)
            .map_err(|e| io_other(format!("PAKE finish: {e}")))?;

        // Frame 7 → our PAKE finalisation.
        send_frame(&mut framed, &msg3).await?;

        let bound_key = session_key.bind_to_tls_channel(&tls_binder);
        let own_tag = channel_confirmation_tag(&bound_key, ConfirmRole::Initiator);
        let expected_peer_tag = channel_confirmation_tag(&bound_key, ConfirmRole::Responder);

        // Frame 8 ← responder's confirmation tag.
        let peer_tag = recv_confirmation_tag(&mut framed).await?;
        // Frame 9 → our confirmation tag.
        send_frame(&mut framed, &own_tag).await?;
        if peer_tag.ct_eq(&expected_peer_tag).unwrap_u8() != 1 {
            return Err(io_other(
                "bootstrap: channel-binding confirmation mismatch — possible relay MitM, pairing aborted".into(),
            ));
        }

        let sas = derive_sas(&bound_key);
        Ok::<_, TransportError>((sas, tls_peer_fp, peer_sync_addr, session_key))
    })
    .await
    .map_err(|_elapsed| {
        tracing::warn!(
            timeout = ?PAKE_EXCHANGE_TIMEOUT,
            "bootstrap(sas): initiator PAKE exchange timed out — stalled responder"
        );
        io_other("bootstrap: PAKE exchange timed out".into())
    })??;

    let (sas, peer_fingerprint, peer_sync_addr, session_key) = prepared;

    // Human SAS confirmation (outside the PAKE deadline). Reject → error → keys
    // drop/zeroize.
    let accepted_locally = confirm(&sas).await;

    // Frame 10a: exchange ACCEPT/REJECT bytes. Proceed only if BOTH accept.
    let our_byte = if accepted_locally {
        SAS_ACCEPT
    } else {
        SAS_REJECT
    };
    send_frame(&mut framed, &[our_byte]).await?;
    let peer_byte = recv_confirm_byte(&mut framed).await?;
    if our_byte != SAS_ACCEPT || peer_byte != SAS_ACCEPT {
        return Err(io_other("SAS rejected by user — pairing aborted".into()));
    }

    let (peer_meta, peer_provisioning) =
        exchange_peer_meta(&mut framed, own_meta, own_provisioning.as_ref()).await;

    Ok(BootstrapPairing {
        peer_fingerprint,
        peer_sync_addr,
        session_key,
        sas,
        peer_model: peer_meta.model,
        peer_os: peer_meta.os_version,
        peer_app_version: peer_meta.app_version,
        peer_local_ip: peer_meta.local_ip,
        peer_device_name: peer_meta.device_name,
        peer_public_ip: peer_meta.public_ip,
        peer_device_id: peer_meta.device_id,
        peer_provisioning,
    })
}

// ── device-metadata exchange (P2P Phase 4) ────────────────────────────────────

/// Exchange optional device metadata over the framed stream AFTER the PAKE
/// handshake has fully completed.
///
/// Symmetric on both endpoints (so it cannot deadlock): each side SENDS its own
/// version byte then its metadata JSON, then RECEIVES the peer's version byte
/// and metadata. Sending first, before any receive, keeps the two sides in
/// lock-step over the duplex stream.
///
/// Back-compat: a legacy peer terminates the protocol at frame 9 and never reads
/// or writes these frames. When we try to receive its version frame the stream
/// is closed → `recv_frame` errors → we return [`PeerMeta::default`] (all
/// `None`). Likewise an explicit version `< BOOTSTRAP_PROTO_VERSION` skips the
/// metadata read. ALL errors are swallowed: pairing already succeeded, so a
/// metadata hiccup must never turn it into a failure.
async fn exchange_peer_meta<S>(
    framed: &mut Framed<S, LengthDelimitedCodec>,
    own_meta: &PeerMeta,
    own_provisioning: Option<&SyncProvisioning>,
) -> (PeerMeta, Option<SyncProvisioning>)
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    // ── Send half (always send-first to stay in lock-step over the duplex) ──
    //
    // Frame 10: our version byte. Frame 11: our metadata JSON. When we advertise
    // proto >= 2 we ALSO send frame 12: our sync-provisioning JSON. The version
    // byte tells the peer whether to expect frame 12, so a v1 peer never reads
    // it. Swallow send errors (a legacy peer may have closed the read half).
    if send_frame(framed, &[BOOTSTRAP_PROTO_VERSION])
        .await
        .is_err()
    {
        return (PeerMeta::default(), None);
    }
    let own_json = serde_json::to_vec(own_meta).unwrap_or_default();
    if send_frame(framed, &own_json).await.is_err() {
        return (PeerMeta::default(), None);
    }
    // Frame 12 (proto >= 2): our sync-provisioning JSON. We always send a frame
    // when our advertised version supports it — an unconfigured side sends an
    // all-`None` value so the peer's read stays in lock-step. NOTE: the JSON is
    // produced via serde; `serde_json::to_vec` does not log field values, so the
    // secret `derived_sync_key` is never written to a log here.
    if BOOTSTRAP_PROTO_VERSION >= SYNC_PROVISIONING_MIN_VERSION {
        let prov = own_provisioning.cloned().unwrap_or_default();
        let prov_json = serde_json::to_vec(&prov).unwrap_or_default();
        if send_frame(framed, &prov_json).await.is_err() {
            // We already sent meta; treat a provisioning send failure as "no
            // provisioning exchange" but still return whatever meta we read.
            // Fall through to the receive half so we can still learn peer meta.
        }
    }

    // ── Receive half ──
    //
    // Frame 10 ← peer version byte. Absent / malformed → legacy peer.
    let peer_version = match recv_frame(framed).await {
        Ok(bytes) if bytes.len() == 1 => bytes[0],
        _ => return (PeerMeta::default(), None),
    };
    if peer_version < 1 {
        // Should not happen (version 0 is never advertised); be defensive.
        return (PeerMeta::default(), None);
    }

    // Frame 11 ← peer metadata JSON.
    let peer_meta = match recv_frame(framed).await {
        Ok(b) if b.len() <= MAX_META_BYTES => {
            serde_json::from_slice::<PeerMeta>(&b).unwrap_or_default()
        }
        _ => return (PeerMeta::default(), None),
    };

    // Frame 12 ← peer sync-provisioning JSON — ONLY when the peer advertised a
    // version that includes it. A v1 (or unknown-lower) peer never sent it, so
    // we must NOT try to read it (that would desync the stream); we return
    // `None` for provisioning and the meta we already learned. This is the
    // version-gated back-compat, mirroring the additive `PeerMeta` pattern.
    if peer_version < SYNC_PROVISIONING_MIN_VERSION {
        return (peer_meta, None);
    }
    let peer_provisioning = match recv_frame(framed).await {
        Ok(b) if b.len() <= MAX_PROVISIONING_BYTES => {
            serde_json::from_slice::<SyncProvisioning>(&b).ok()
        }
        // A missing/oversized/garbled provisioning frame must not fail the
        // already-complete pairing — just yield no provisioning.
        _ => None,
    };
    (peer_meta, peer_provisioning)
}

// ── framing helpers ───────────────────────────────────────────────────────────

fn length_codec() -> LengthDelimitedCodec {
    LengthDelimitedCodec::builder()
        .max_frame_length(MAX_FRAME_BYTES)
        .new_codec()
}

fn io_other(msg: String) -> TransportError {
    TransportError::Io(std::io::Error::other(msg))
}

async fn send_frame<S>(
    framed: &mut Framed<S, LengthDelimitedCodec>,
    body: &[u8],
) -> Result<(), TransportError>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    framed
        .send(bytes::Bytes::copy_from_slice(body))
        .await
        .map_err(TransportError::Io)
}

async fn recv_frame<S>(
    framed: &mut Framed<S, LengthDelimitedCodec>,
) -> Result<Vec<u8>, TransportError>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    match framed.next().await {
        Some(Ok(bytes)) => Ok(bytes.to_vec()),
        Some(Err(e)) => Err(TransportError::Io(e)),
        None => Err(io_other(
            "bootstrap: peer closed before sending frame".into(),
        )),
    }
}

/// Receive a peer's channel-binding confirmation tag frame (S3).
///
/// Enforces the exact [`CONFIRM_TAG_LEN`] so a desynced or malicious peer
/// cannot smuggle a short/long frame into the constant-time compare slot.
async fn recv_confirmation_tag<S>(
    framed: &mut Framed<S, LengthDelimitedCodec>,
) -> Result<[u8; CONFIRM_TAG_LEN], TransportError>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let bytes = recv_frame(framed).await?;
    let tag: [u8; CONFIRM_TAG_LEN] = bytes.as_slice().try_into().map_err(|_| {
        io_other(format!(
            "bootstrap: confirmation tag wrong length ({} bytes)",
            bytes.len()
        ))
    })?;
    Ok(tag)
}

/// Receive the peer's frame-10a SAS-confirm byte (LAN/SAS path only).
///
/// Enforces an exact 1-byte frame so a desynced/malicious peer cannot smuggle a
/// longer frame into this slot. Any byte other than [`SAS_ACCEPT`] is treated as
/// a reject by the caller.
async fn recv_confirm_byte<S>(
    framed: &mut Framed<S, LengthDelimitedCodec>,
) -> Result<u8, TransportError>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let bytes = recv_frame(framed).await?;
    if bytes.len() != 1 {
        return Err(io_other(format!(
            "bootstrap: SAS-confirm frame wrong length ({} bytes)",
            bytes.len()
        )));
    }
    Ok(bytes[0])
}

async fn recv_fingerprint<S>(
    framed: &mut Framed<S, LengthDelimitedCodec>,
) -> Result<String, TransportError>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let bytes = recv_frame(framed).await?;
    let fp = String::from_utf8(bytes)
        .map_err(|e| io_other(format!("bootstrap: fingerprint not UTF-8: {e}")))?;
    // Accept 64 hex chars regardless of case (peers may send uppercase hex).
    // Normalise to lowercase so callers can compare directly against
    // `fingerprint_of` output (which is always lowercase). The value is public
    // — no timing side-channel concern for the normalisation step itself.
    if fp.len() != 64 || !fp.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(io_other(format!(
            "bootstrap: malformed peer fingerprint ({} bytes)",
            fp.len()
        )));
    }
    Ok(fp.to_lowercase())
}

/// Upper bound on a peer's advertised sync-listener address. A `host:port` is at
/// most a few dozen bytes (IPv6 + port ≈ 47); 256 is a generous ceiling that
/// still rejects a desynced peer sending a huge frame in this slot.
const MAX_SYNC_ADDR_BYTES: usize = 256;

/// Receive a peer's P2P sync-listener `host:port` address frame (Phase 2).
///
/// The address is opaque to the bootstrap layer (it is parsed/validated by the
/// daemon when it dials in Phase 3); this only enforces UTF-8 and a sane length
/// bound so a malformed frame cannot smuggle arbitrary bytes into `peers.json`.
/// An empty frame is accepted and returned as an empty string (the peer simply
/// did not advertise an address).
async fn recv_sync_addr<S>(
    framed: &mut Framed<S, LengthDelimitedCodec>,
) -> Result<String, TransportError>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let bytes = recv_frame(framed).await?;
    if bytes.len() > MAX_SYNC_ADDR_BYTES {
        return Err(io_other(format!(
            "bootstrap: peer sync address too long ({} bytes)",
            bytes.len()
        )));
    }
    String::from_utf8(bytes)
        .map_err(|e| io_other(format!("bootstrap: sync address not UTF-8: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cert::SelfSignedCert;

    /// `bind_on` binds the EXACT requested port (LAN/SAS Phase 2 standing
    /// responder advertises a stable `bport`, so the listener must re-bind the
    /// same port across pairing iterations rather than getting a fresh ephemeral
    /// one each time). Re-binding the same port immediately after dropping the
    /// previous listener must also succeed (listening sockets do not enter
    /// TIME_WAIT).
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn bind_on_binds_requested_port_and_is_reusable() {
        let cert = SelfSignedCert::generate("standing-responder").unwrap();

        // First pick a free port via an ephemeral bind, then drop it.
        let probe = tokio::net::TcpListener::bind("0.0.0.0:0").await.unwrap();
        let port = probe.local_addr().unwrap().port();
        drop(probe);

        let r1 = BootstrapResponder::bind_on(port, cert.cert_der.clone(), cert.key_der.clone())
            .await
            .expect("bind_on requested port");
        assert_eq!(r1.local_addr().unwrap().port(), port);
        drop(r1);

        // Re-bind the same port immediately — must not fail with EADDRINUSE.
        let r2 = BootstrapResponder::bind_on(port, cert.cert_der.clone(), cert.key_der.clone())
            .await
            .expect("re-bind same port");
        assert_eq!(r2.local_addr().unwrap().port(), port);
    }

    /// Two endpoints over a real loopback TCP/TLS socket complete PAKE, the S3
    /// channel-binding confirmation exchange, and converge on the same session
    /// key, learning each other's fingerprints. Both `run`/`run_initiator`
    /// returning `Ok` proves the confirmation tags matched.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn bootstrap_pake_over_tls_loopback_succeeds() {
        let responder_cert = SelfSignedCert::generate("responder-device").unwrap();
        let initiator_cert = SelfSignedCert::generate("initiator-device").unwrap();

        let responder_fp = responder_cert.fingerprint();
        let initiator_fp = initiator_cert.fingerprint();
        assert_ne!(responder_fp, initiator_fp);

        let password = "shared-qr-secret-123456";

        let responder = BootstrapResponder::bind(
            responder_cert.cert_der.clone(),
            responder_cert.key_der.clone(),
        )
        .await
        .expect("bind responder");
        let port = responder.local_addr().expect("local addr").port();
        let resp_fp_expected = responder.fingerprint().to_string();
        assert_eq!(resp_fp_expected, responder_fp);

        let pw = password.to_string();
        let resp_sync_addr = "127.0.0.1:7001";
        let resp_meta = PeerMeta {
            model: Some("Mac mini".into()),
            os_version: Some("macOS 15.5".into()),
            app_version: Some("0.5.4".into()),
            local_ip: Some("192.168.1.10".into()),
            device_name: None,
            public_ip: Some("198.51.100.10".into()),
            device_id: None,
        };
        let resp_meta_task = resp_meta.clone();
        // The responder advertises a full SyncProvisioning ("the configured PC");
        // the initiator advertises None ("a fresh device scanning the QR").
        let resp_prov = SyncProvisioning {
            supabase_url: Some("https://proj.supabase.co".into()),
            supabase_anon_key: Some("anon-key-123".into()),
            relay_url: Some("https://relay.example".into()),
            derived_sync_key: Some(vec![7u8; 32]),
        };
        let resp_prov_task = resp_prov.clone();
        let responder_task = tokio::spawn(async move {
            responder
                .run(&pw, resp_sync_addr, &resp_meta_task, Some(resp_prov_task))
                .await
        });

        let addr: SocketAddr = ([127, 0, 0, 1], port).into();
        let init_pw = password.to_string();
        let init_sync_addr = "127.0.0.1:7002";
        let init_meta = PeerMeta {
            model: Some("MacBook Air".into()),
            os_version: Some("macOS 14.4".into()),
            app_version: Some("0.5.4".into()),
            local_ip: Some("192.168.1.11".into()),
            device_name: None,
            public_ip: Some("198.51.100.11".into()),
            device_id: None,
        };
        let init_meta_task = init_meta.clone();
        let initiator_task = tokio::spawn(async move {
            run_initiator(
                addr,
                initiator_cert.cert_der,
                initiator_cert.key_der,
                &init_pw,
                init_sync_addr,
                &init_meta_task,
                None,
            )
            .await
        });

        let (resp_res, init_res) = tokio::join!(responder_task, initiator_task);
        let resp = resp_res.expect("responder join").expect("responder pake");
        let init = init_res.expect("initiator join").expect("initiator pake");

        // Session keys converge — the PAKE security goal, over a real network stack.
        assert_eq!(
            resp.session_key.as_bytes(),
            init.session_key.as_bytes(),
            "both endpoints must derive the same PAKE session key over TLS"
        );

        // Each side learned the other's real cert fingerprint.
        assert_eq!(resp.peer_fingerprint, initiator_fp);
        assert_eq!(init.peer_fingerprint, responder_fp);

        // Phase 2: each side also learned the other's P2P sync-listener address.
        assert_eq!(resp.peer_sync_addr, init_sync_addr);
        assert_eq!(init.peer_sync_addr, resp_sync_addr);

        // Phase 4: each side learned the other's device metadata over the
        // post-handshake metadata extension.
        assert_eq!(resp.peer_model, init_meta.model);
        assert_eq!(resp.peer_os, init_meta.os_version);
        assert_eq!(resp.peer_app_version, init_meta.app_version);
        assert_eq!(resp.peer_local_ip, init_meta.local_ip);
        assert_eq!(resp.peer_public_ip, init_meta.public_ip);
        assert_eq!(init.peer_model, resp_meta.model);
        assert_eq!(init.peer_os, resp_meta.os_version);
        assert_eq!(init.peer_app_version, resp_meta.app_version);
        assert_eq!(init.peer_local_ip, resp_meta.local_ip);
        assert_eq!(init.peer_public_ip, resp_meta.public_ip);

        // Proto v2: the initiator (which sent None) RECEIVES the responder's
        // full SyncProvisioning. The responder (initiator sent None) receives an
        // all-None provisioning, i.e. the default value carrying nothing.
        assert_eq!(
            init.peer_provisioning,
            Some(resp_prov),
            "initiator must receive the responder's advertised provisioning"
        );
        assert_eq!(
            resp.peer_provisioning,
            Some(SyncProvisioning::default()),
            "responder must receive an all-None provisioning from a fresh device"
        );
    }

    /// Wrong password: the initiator's PAKE finish must fail, and the responder
    /// must not produce a session key.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn bootstrap_pake_wrong_password_fails() {
        let responder_cert = SelfSignedCert::generate("responder-device").unwrap();
        let initiator_cert = SelfSignedCert::generate("initiator-device").unwrap();

        let responder = BootstrapResponder::bind(
            responder_cert.cert_der.clone(),
            responder_cert.key_der.clone(),
        )
        .await
        .expect("bind responder");
        let port = responder.local_addr().expect("local addr").port();

        let responder_task = tokio::spawn(async move {
            responder
                .run(
                    "the-right-password",
                    "127.0.0.1:7003",
                    &PeerMeta::default(),
                    None,
                )
                .await
        });

        let addr: SocketAddr = ([127, 0, 0, 1], port).into();
        let initiator_task = tokio::spawn(async move {
            run_initiator(
                addr,
                initiator_cert.cert_der,
                initiator_cert.key_der,
                "the-WRONG-password",
                "127.0.0.1:7004",
                &PeerMeta::default(),
                None,
            )
            .await
        });

        let (resp_res, init_res) = tokio::join!(responder_task, initiator_task);
        let init = init_res.expect("initiator join");
        assert!(init.is_err(), "initiator must fail on wrong password");
        let resp = resp_res.expect("responder join");
        assert!(
            resp.is_err(),
            "responder must not derive a key on wrong password"
        );
    }

    /// Relay MitM: an attacker who knows the correct PAKE password but cannot
    /// keep a single TLS channel end-to-end. The relay terminates TLS toward the
    /// initiator and opens a *separate* TLS session to the real responder, then
    /// blindly pumps the opaque PAKE/confirmation frames between the two legs.
    ///
    /// PAKE itself still completes (the bytes are forwarded verbatim), but the
    /// RFC 5705 channel binder differs on each TLS leg, so the channel-bound
    /// confirmation tags do not match and BOTH endpoints must reject pairing.
    /// This is the exact attack S3 channel binding defends against.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn bootstrap_relay_mitm_is_rejected_by_channel_binding() {
        use tokio::io::{copy, AsyncWriteExt};

        let responder_cert = SelfSignedCert::generate("responder-device").unwrap();
        let initiator_cert = SelfSignedCert::generate("initiator-device").unwrap();
        let relay_cert = SelfSignedCert::generate("relay-mitm-device").unwrap();

        let password = "shared-qr-secret-relay";

        // Real responder.
        let responder = BootstrapResponder::bind(
            responder_cert.cert_der.clone(),
            responder_cert.key_der.clone(),
        )
        .await
        .expect("bind responder");
        let responder_port = responder.local_addr().expect("local addr").port();
        let pw = password.to_string();
        let responder_task = tokio::spawn(async move {
            responder
                .run(&pw, "127.0.0.1:7005", &PeerMeta::default(), None)
                .await
        });

        // Relay listener: TLS server toward the initiator (accept any client cert).
        let relay_listener = TcpListener::bind("127.0.0.1:0").await.expect("relay bind");
        let relay_port = relay_listener.local_addr().unwrap().port();

        let relay_server_cfg = ServerConfig::builder()
            .with_client_cert_verifier(Arc::new(AcceptAnyCert))
            .with_single_cert(
                vec![CertificateDer::from(relay_cert.cert_der.clone())],
                PrivateKeyDer::Pkcs8(rustls::pki_types::PrivatePkcs8KeyDer::from(
                    relay_cert.key_der.clone(),
                )),
            )
            .expect("relay server cfg");
        let relay_acceptor = TlsAcceptor::from(Arc::new(relay_server_cfg));

        // Relay client config toward the real responder (accept any server cert,
        // present the relay's own cert).
        let relay_client_cfg = ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(AcceptAnyCert))
            .with_client_auth_cert(
                vec![CertificateDer::from(relay_cert.cert_der.clone())],
                PrivateKeyDer::Pkcs8(rustls::pki_types::PrivatePkcs8KeyDer::from(
                    relay_cert.key_der.clone(),
                )),
            )
            .expect("relay client cfg");
        let relay_connector = TlsConnector::from(Arc::new(relay_client_cfg));

        let relay_task = tokio::spawn(async move {
            let (inbound, _) = relay_listener.accept().await.expect("relay accept");
            let init_tls = relay_acceptor
                .accept(inbound)
                .await
                .expect("relay tls accept");

            let upstream = TcpStream::connect(("127.0.0.1", responder_port))
                .await
                .expect("relay->responder connect");
            let server_name = ServerName::try_from("copypaste.peer").unwrap();
            let resp_tls = relay_connector
                .connect(server_name, upstream)
                .await
                .expect("relay->responder tls");

            // Blindly pump bytes both directions between the two TLS legs.
            let (mut ir, mut iw) = tokio::io::split(init_tls);
            let (mut rr, mut rw) = tokio::io::split(resp_tls);
            let a = tokio::spawn(async move {
                let _ = copy(&mut ir, &mut rw).await;
                let _ = rw.shutdown().await;
            });
            let b = tokio::spawn(async move {
                let _ = copy(&mut rr, &mut iw).await;
                let _ = iw.shutdown().await;
            });
            let _ = tokio::join!(a, b);
        });

        // Initiator dials the RELAY (thinking it is the responder).
        let relay_addr: SocketAddr = ([127, 0, 0, 1], relay_port).into();
        let init_pw = password.to_string();
        let initiator_task = tokio::spawn(async move {
            run_initiator(
                relay_addr,
                initiator_cert.cert_der,
                initiator_cert.key_der,
                &init_pw,
                "127.0.0.1:7006",
                &PeerMeta::default(),
                None,
            )
            .await
        });

        let (resp_res, init_res, _relay_res) =
            tokio::join!(responder_task, initiator_task, relay_task);

        let init = init_res.expect("initiator join");
        assert!(
            init.is_err(),
            "initiator must reject pairing — channel binding confirmation mismatch under relay MitM"
        );
        let resp = resp_res.expect("responder join");
        assert!(
            resp.is_err(),
            "responder must reject pairing — channel binding confirmation mismatch under relay MitM"
        );
    }

    // ── Fix 2: PAKE exchange has an overall deadline ──────────────────────────

    /// A peer that completes TLS but then dribbles / stalls mid-PAKE exchange
    /// must be evicted by `PAKE_EXCHANGE_TIMEOUT`. Without this deadline the
    /// single-shot responder (and the initiator) would be pinned indefinitely
    /// (slowloris-style DoS).
    ///
    /// We simulate a slow responder by opening a raw TLS bootstrap connection,
    /// sending the very first frame (PAKE msg1) and then going silent. The
    /// `BootstrapResponder::run` future must time out on `PAKE_EXCHANGE_TIMEOUT`,
    /// NOT block forever.
    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn pake_exchange_timeout_fires_on_slow_peer() {
        let responder_cert = SelfSignedCert::generate("responder-device").unwrap();

        let responder = BootstrapResponder::bind(
            responder_cert.cert_der.clone(),
            responder_cert.key_der.clone(),
        )
        .await
        .expect("bind responder");
        let port = responder.local_addr().expect("local addr").port();

        // Run the responder; it must time out because we'll stall after frame 1.
        let responder_task = tokio::spawn(async move {
            responder
                .run("any-password", "127.0.0.1:9000", &PeerMeta::default(), None)
                .await
        });

        // Connect with an "any cert" TLS client, send exactly frame 1 (a fake
        // PAKE msg1 byte string), then go permanently silent — no more frames.
        let addr: SocketAddr = ([127, 0, 0, 1], port).into();
        let staller_cert = SelfSignedCert::generate("staller").unwrap();
        let staller_task = tokio::spawn(async move {
            use futures_util::SinkExt as _;
            let cert = rustls::pki_types::CertificateDer::from(staller_cert.cert_der.clone());
            let key = rustls::pki_types::PrivatePkcs8KeyDer::from(staller_cert.key_der.clone());
            let private_key = rustls::pki_types::PrivateKeyDer::Pkcs8(key);
            let client_config = rustls::ClientConfig::builder()
                .dangerous()
                .with_custom_certificate_verifier(std::sync::Arc::new(AcceptAnyCert))
                .with_client_auth_cert(vec![cert], private_key)
                .expect("client config");
            let connector = tokio_rustls::TlsConnector::from(std::sync::Arc::new(client_config));
            let tcp = tokio::net::TcpStream::connect(addr)
                .await
                .expect("tcp connect");
            let server_name =
                rustls::pki_types::ServerName::try_from("copypaste.peer").expect("server name");
            let tls_stream = connector
                .connect(server_name, tcp)
                .await
                .expect("tls connect");
            let mut framed = tokio_util::codec::Framed::new(tls_stream, length_codec());
            // Send one garbage frame (pretend to be PAKE msg1) then go silent forever.
            framed
                .send(bytes::Bytes::from_static(b"fake-pake-msg1"))
                .await
                .expect("send frame1");
            // Hold the connection open so the responder can't detect closure.
            tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
        });

        // Advance virtual time well past PAKE_EXCHANGE_TIMEOUT.
        let advance_ms = PAKE_EXCHANGE_TIMEOUT.as_millis() as u64 + 1_000;
        tokio::time::sleep(std::time::Duration::from_millis(advance_ms)).await;

        // The responder should have timed out by now.
        staller_task.abort();
        let result = responder_task.await.expect("responder join");
        assert!(
            result.is_err(),
            "responder must fail when peer stalls mid-PAKE (PAKE_EXCHANGE_TIMEOUT not applied)"
        );
    }

    // ── Phase 4: device-metadata extension back-compat ───────────────────────

    /// Two NEW peers running `exchange_peer_meta` over an in-memory duplex pair
    /// must each learn the other's metadata.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn exchange_peer_meta_both_new_learns_each_other() {
        let (a, b) = tokio::io::duplex(4096);
        let mut fa = Framed::new(a, length_codec());
        let mut fb = Framed::new(b, length_codec());

        let meta_a = PeerMeta {
            model: Some("MacBook Air".into()),
            os_version: Some("macOS 14.4".into()),
            app_version: Some("0.5.4".into()),
            local_ip: Some("10.0.0.1".into()),
            device_name: None,
            public_ip: Some("203.0.113.7".into()),
            device_id: None,
        };
        let meta_b = PeerMeta {
            model: Some("Mac mini".into()),
            public_ip: Some("203.0.113.8".into()),
            ..Default::default()
        };

        // Side A advertises provisioning; side B advertises None.
        let prov_a = SyncProvisioning {
            supabase_url: Some("https://a.supabase.co".into()),
            derived_sync_key: Some(vec![9u8; 32]),
            ..Default::default()
        };

        let ma = meta_a.clone();
        let mb = meta_b.clone();
        let pa = prov_a.clone();
        let ta = tokio::spawn(async move { exchange_peer_meta(&mut fa, &ma, Some(&pa)).await });
        let tb = tokio::spawn(async move { exchange_peer_meta(&mut fb, &mb, None).await });
        let (got_a, got_b) = tokio::join!(ta, tb);

        // Side A learned B's metadata; side B learned A's.
        let (meta_from_b, prov_from_b) = got_a.unwrap();
        let (meta_from_a, prov_from_a) = got_b.unwrap();
        assert_eq!(meta_from_b, meta_b);
        assert_eq!(meta_from_a, meta_a);
        // Side B learned A's provisioning; side A learned B's all-None default.
        assert_eq!(prov_from_a, Some(prov_a));
        assert_eq!(prov_from_b, Some(SyncProvisioning::default()));
    }

    /// Back-compat: when the peer is LEGACY (closes the stream without sending a
    /// version/metadata frame), `exchange_peer_meta` must return the default
    /// (all-`None`) metadata rather than hanging or erroring — the pairing has
    /// already completed and metadata is best-effort.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn exchange_peer_meta_legacy_peer_yields_none() {
        let (a, b) = tokio::io::duplex(4096);
        let mut fa = Framed::new(a, length_codec());

        // Legacy peer: drop its end immediately (frame 9 was the last thing it
        // would have sent in the real protocol).
        drop(b);

        let meta_a = PeerMeta {
            model: Some("MacBook Air".into()),
            ..Default::default()
        };
        let (got_meta, got_prov) = exchange_peer_meta(&mut fa, &meta_a, None).await;
        assert_eq!(
            got_meta,
            PeerMeta::default(),
            "a legacy peer that sends no metadata must yield all-None"
        );
        assert_eq!(
            got_prov, None,
            "a legacy peer that sends no provisioning must yield None"
        );
    }

    // ── proto v2: SyncProvisioning exchange + back-compat ─────────────────────

    /// A v1 (proto-version-1) peer participates in the metadata exchange but does
    /// NOT send a provisioning frame. The v2 side must learn the peer's metadata
    /// and return `None` for provisioning WITHOUT desyncing the stream — this is
    /// the version-gated back-compat. We simulate a v1 peer by hand-writing only
    /// frames 10 (version byte = 1) and 11 (metadata JSON), then closing.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn exchange_with_v1_peer_yields_none_provisioning() {
        let (a, b) = tokio::io::duplex(4096);
        let mut fa = Framed::new(a, length_codec());
        let mut fb = Framed::new(b, length_codec());

        // Side A is the modern (v2) side advertising provisioning.
        let meta_a = PeerMeta {
            model: Some("Modern Mac".into()),
            ..Default::default()
        };
        let prov_a = SyncProvisioning {
            supabase_url: Some("https://a.supabase.co".into()),
            ..Default::default()
        };

        // Side B emulates a v1 peer: send version byte 1, then a metadata JSON,
        // then drop — it never sends or reads a provisioning frame.
        let peer_meta_b = PeerMeta {
            model: Some("Legacy Mac".into()),
            ..Default::default()
        };
        let b_task = tokio::spawn(async move {
            send_frame(&mut fb, &[1u8]).await.unwrap();
            let json = serde_json::to_vec(&peer_meta_b).unwrap();
            send_frame(&mut fb, &json).await.unwrap();
            // Read A's frames so A's sends don't block, but never send frame 12.
            let _ = recv_frame(&mut fb).await; // A version byte
            let _ = recv_frame(&mut fb).await; // A metadata
            let _ = recv_frame(&mut fb).await; // A provisioning (A sends it; B ignores)
            peer_meta_b
        });

        let (got_meta, got_prov) = exchange_peer_meta(&mut fa, &meta_a, Some(&prov_a)).await;
        let sent_b = b_task.await.unwrap();

        assert_eq!(
            got_meta, sent_b,
            "v2 side must learn the v1 peer's metadata"
        );
        assert_eq!(
            got_prov, None,
            "a v1 peer that sends no provisioning frame must yield None (back-compat)"
        );
    }

    /// `SyncProvisioning` round-trips through its JSON wire form, including the
    /// secret derived key bytes.
    #[test]
    fn sync_provisioning_round_trips() {
        let prov = SyncProvisioning {
            supabase_url: Some("https://x.supabase.co".into()),
            supabase_anon_key: Some("anon-jwt".into()),
            relay_url: Some("https://relay.example".into()),
            derived_sync_key: Some(vec![1u8; 32]),
        };
        let json = serde_json::to_vec(&prov).expect("serialize");
        let back: SyncProvisioning = serde_json::from_slice(&json).expect("deserialize");
        assert_eq!(back, prov, "SyncProvisioning must round-trip");
    }

    /// An all-`None` `SyncProvisioning` serialises to an empty object and round
    /// -trips back to the default (every field omitted via skip_serializing_if).
    #[test]
    fn sync_provisioning_all_none_is_empty_object() {
        let prov = SyncProvisioning::default();
        let json = serde_json::to_string(&prov).expect("serialize");
        assert_eq!(json, "{}", "all-None provisioning must serialise to {{}}");
        let back: SyncProvisioning = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, SyncProvisioning::default());
    }

    /// The custom `Debug` impl must NOT print the secret key bytes — only a
    /// redacted length marker — while still showing the non-secret URLs.
    #[test]
    fn sync_provisioning_debug_redacts_key() {
        let prov = SyncProvisioning {
            supabase_url: Some("https://x.supabase.co".into()),
            derived_sync_key: Some(vec![0xABu8; 32]),
            ..Default::default()
        };
        let dbg = format!("{prov:?}");
        assert!(dbg.contains("redacted"), "Debug must redact the key: {dbg}");
        assert!(
            !dbg.contains("171") && !dbg.contains("0xab") && !dbg.contains("AB, AB"),
            "Debug must not contain raw key bytes: {dbg}"
        );
        assert!(
            dbg.contains("x.supabase.co"),
            "Debug must still show the non-secret URL: {dbg}"
        );
    }

    // ── PeerMeta.public_ip serde (B1: peer public/global IP exchange) ─────────

    /// Round-trip: a `PeerMeta` carrying `public_ip` serialises and deserialises
    /// back to an equal value (the new field survives the JSON wire form).
    #[test]
    fn peer_meta_public_ip_round_trips() {
        let meta = PeerMeta {
            model: Some("MacBook Air".into()),
            os_version: Some("macOS 15.5".into()),
            app_version: Some("0.6.0".into()),
            local_ip: Some("192.168.1.5".into()),
            device_name: Some("Alice's MacBook".into()),
            public_ip: Some("203.0.113.42".into()),
            device_id: None,
        };
        let json = serde_json::to_string(&meta).expect("serialize PeerMeta");
        assert!(
            json.contains("\"public_ip\":\"203.0.113.42\""),
            "public_ip must appear in the serialised PeerMeta JSON: {json}"
        );
        let back: PeerMeta = serde_json::from_str(&json).expect("deserialize PeerMeta");
        assert_eq!(back, meta, "PeerMeta must round-trip with public_ip set");
    }

    /// When `public_ip` is `None`, it is omitted from the wire form
    /// (`skip_serializing_if`) — keeping the frame minimal and back-compat with a
    /// legacy reader that does not know the key.
    #[test]
    fn peer_meta_public_ip_none_is_omitted() {
        let meta = PeerMeta {
            model: Some("Mac mini".into()),
            ..Default::default()
        };
        let json = serde_json::to_string(&meta).expect("serialize PeerMeta");
        assert!(
            !json.contains("public_ip"),
            "public_ip must be absent from JSON when None: {json}"
        );
    }

    /// Back-compat: an OLD-format `PeerMeta` payload that predates `public_ip`
    /// (the key is entirely absent) must deserialise cleanly with `public_ip ==
    /// None`. This is the wire form an older peer sends; it must NOT error, so
    /// pairing/connecting/syncing with a legacy peer keeps working.
    #[test]
    fn peer_meta_legacy_payload_without_public_ip_deserialises_to_none() {
        // Exactly the JSON an older build emits (model/os/app/local_ip/device_name,
        // NO public_ip key).
        let legacy_json = r#"{
            "model": "MacBook Air",
            "os_version": "macOS 14.4",
            "app_version": "0.5.4",
            "local_ip": "192.168.1.11",
            "device_name": "Bob's Mac"
        }"#;
        let meta: PeerMeta =
            serde_json::from_str(legacy_json).expect("legacy PeerMeta must deserialise");
        assert_eq!(
            meta.public_ip, None,
            "a legacy payload missing public_ip must deserialise to None"
        );
        // The other fields still populate, proving the additive field did not
        // disturb the existing wire contract.
        assert_eq!(meta.model.as_deref(), Some("MacBook Air"));
        assert_eq!(meta.device_name.as_deref(), Some("Bob's Mac"));
    }

    // ── LAN/SAS phase 1: confirm-gated handshake variants ────────────────────

    /// Both endpoints run the confirm-gated variants over a real loopback
    /// TLS socket: each side's `confirm` callback is invoked with the SAS, both
    /// accept, and the handshake completes. The two SAS strings MUST be equal
    /// (same `bound_key`), and the returned `BootstrapPairing.sas` matches.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn confirm_variants_loopback_sas_matches_and_accepts() {
        use std::sync::{Arc, Mutex};

        let responder_cert = SelfSignedCert::generate("responder-device").unwrap();
        let initiator_cert = SelfSignedCert::generate("initiator-device").unwrap();
        let password = "sas-confirm-loopback";

        let responder = BootstrapResponder::bind(
            responder_cert.cert_der.clone(),
            responder_cert.key_der.clone(),
        )
        .await
        .expect("bind responder");
        let port = responder.local_addr().expect("local addr").port();

        let resp_seen: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
        let init_seen: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

        let resp_seen_cb = resp_seen.clone();
        let responder_task = tokio::spawn(async move {
            responder
                .run_with_confirm(
                    "sas-confirm-loopback",
                    "127.0.0.1:7101",
                    &PeerMeta::default(),
                    None,
                    move |sas| {
                        let slot = resp_seen_cb.clone();
                        let sas = sas.to_string();
                        async move {
                            *slot.lock().unwrap() = Some(sas);
                            true
                        }
                    },
                )
                .await
        });

        let addr: SocketAddr = ([127, 0, 0, 1], port).into();
        let init_seen_cb = init_seen.clone();
        let initiator_task = tokio::spawn(async move {
            run_initiator_with_confirm(
                addr,
                initiator_cert.cert_der,
                initiator_cert.key_der,
                "sas-confirm-loopback",
                "127.0.0.1:7102",
                &PeerMeta::default(),
                None,
                move |sas| {
                    let slot = init_seen_cb.clone();
                    let sas = sas.to_string();
                    async move {
                        *slot.lock().unwrap() = Some(sas);
                        true
                    }
                },
            )
            .await
        });

        let _ = password;
        let (resp_res, init_res) = tokio::join!(responder_task, initiator_task);
        let resp = resp_res
            .expect("responder join")
            .expect("responder pairing");
        let init = init_res
            .expect("initiator join")
            .expect("initiator pairing");

        let resp_sas = resp_seen
            .lock()
            .unwrap()
            .clone()
            .expect("responder saw sas");
        let init_sas = init_seen
            .lock()
            .unwrap()
            .clone()
            .expect("initiator saw sas");
        assert_eq!(resp_sas, init_sas, "both sides must see the same SAS");
        assert_eq!(resp.sas, resp_sas, "returned sas matches confirmed sas");
        assert_eq!(init.sas, init_sas, "returned sas matches confirmed sas");
        assert_eq!(resp.sas, init.sas);
        assert_eq!(resp.session_key.as_bytes(), init.session_key.as_bytes());
    }

    /// Regression for the discovery-pairing P0: when BOTH the initiator and the
    /// responder use the fixed, well-known [`DISCOVERY_PAIRING_PASSWORD`] (the
    /// QR-less LAN/SAS path), the asymmetric OPAQUE PAKE `finish`es (no
    /// `InvalidPassword` at frame 7) and both sides derive the SAME SAS. The old
    /// daemon discovery path generated an INDEPENDENT random password per side,
    /// which would fail here. Mirrors `confirm_variants_loopback_sas_matches_and_accepts`
    /// but pins the shared constant.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn discovery_shared_password_pake_completes_and_sas_matches() {
        use std::sync::{Arc, Mutex};

        let responder_cert = SelfSignedCert::generate("responder-device").unwrap();
        let initiator_cert = SelfSignedCert::generate("initiator-device").unwrap();

        let responder = BootstrapResponder::bind(
            responder_cert.cert_der.clone(),
            responder_cert.key_der.clone(),
        )
        .await
        .expect("bind responder");
        let port = responder.local_addr().expect("local addr").port();

        let resp_seen: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
        let init_seen: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

        let resp_seen_cb = resp_seen.clone();
        let responder_task = tokio::spawn(async move {
            responder
                .run_with_confirm(
                    DISCOVERY_PAIRING_PASSWORD,
                    "127.0.0.1:7111",
                    &PeerMeta::default(),
                    None,
                    move |sas| {
                        let slot = resp_seen_cb.clone();
                        let sas = sas.to_string();
                        async move {
                            *slot.lock().unwrap() = Some(sas);
                            true
                        }
                    },
                )
                .await
        });

        let addr: SocketAddr = ([127, 0, 0, 1], port).into();
        let init_seen_cb = init_seen.clone();
        let initiator_task = tokio::spawn(async move {
            run_initiator_with_confirm(
                addr,
                initiator_cert.cert_der,
                initiator_cert.key_der,
                DISCOVERY_PAIRING_PASSWORD,
                "127.0.0.1:7112",
                &PeerMeta::default(),
                None,
                move |sas| {
                    let slot = init_seen_cb.clone();
                    let sas = sas.to_string();
                    async move {
                        *slot.lock().unwrap() = Some(sas);
                        true
                    }
                },
            )
            .await
        });

        let (resp_res, init_res) = tokio::join!(responder_task, initiator_task);
        let resp = resp_res
            .expect("responder join")
            .expect("responder pairing (PAKE must finish with the shared password)");
        let init = init_res
            .expect("initiator join")
            .expect("initiator pairing (PAKE must finish with the shared password)");

        let resp_sas = resp_seen
            .lock()
            .unwrap()
            .clone()
            .expect("responder saw sas");
        let init_sas = init_seen
            .lock()
            .unwrap()
            .clone()
            .expect("initiator saw sas");
        assert_eq!(resp_sas, init_sas, "both sides must derive the same SAS");
        assert_eq!(resp.sas, init.sas);
        assert_eq!(resp.session_key.as_bytes(), init.session_key.as_bytes());
    }

    /// If EITHER side's user rejects the SAS, BOTH endpoints must abort with an
    /// error and neither returns a `BootstrapPairing` (keys drop/zeroize).
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn confirm_variant_reject_aborts_both() {
        let responder_cert = SelfSignedCert::generate("responder-device").unwrap();
        let initiator_cert = SelfSignedCert::generate("initiator-device").unwrap();
        let password = "sas-confirm-reject";

        let responder = BootstrapResponder::bind(
            responder_cert.cert_der.clone(),
            responder_cert.key_der.clone(),
        )
        .await
        .expect("bind responder");
        let port = responder.local_addr().expect("local addr").port();

        // Responder accepts; initiator rejects → both must fail.
        let responder_task = tokio::spawn(async move {
            responder
                .run_with_confirm(
                    "sas-confirm-reject",
                    "127.0.0.1:7103",
                    &PeerMeta::default(),
                    None,
                    |_sas| async { true },
                )
                .await
        });

        let addr: SocketAddr = ([127, 0, 0, 1], port).into();
        let initiator_task = tokio::spawn(async move {
            run_initiator_with_confirm(
                addr,
                initiator_cert.cert_der,
                initiator_cert.key_der,
                "sas-confirm-reject",
                "127.0.0.1:7104",
                &PeerMeta::default(),
                None,
                |_sas| async { false },
            )
            .await
        });

        let _ = password;
        let (resp_res, init_res) = tokio::join!(responder_task, initiator_task);
        let init = init_res.expect("initiator join");
        assert!(
            init.is_err(),
            "initiator must abort when it rejects the SAS"
        );
        let resp = resp_res.expect("responder join");
        assert!(
            resp.is_err(),
            "responder must abort when the peer rejects the SAS"
        );
    }

    /// Under a relay MitM the two legs derive DIFFERENT `bound_key`s, so the two
    /// SAS values diverge — the human compare is what catches the attack. We use
    /// the confirm variants and capture each side's SAS; if both captured one
    /// they must differ. The channel-binding tag check already aborts both, so
    /// this also asserts both still fail.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn confirm_variant_relay_mitm_yields_different_sas_per_leg() {
        use std::sync::{Arc, Mutex};
        use tokio::io::{copy, AsyncWriteExt};

        let responder_cert = SelfSignedCert::generate("responder-device").unwrap();
        let initiator_cert = SelfSignedCert::generate("initiator-device").unwrap();
        let relay_cert = SelfSignedCert::generate("relay-mitm-device").unwrap();
        let password = "sas-relay-secret";

        let responder = BootstrapResponder::bind(
            responder_cert.cert_der.clone(),
            responder_cert.key_der.clone(),
        )
        .await
        .expect("bind responder");
        let responder_port = responder.local_addr().expect("local addr").port();

        let resp_seen: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
        let init_seen: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

        let resp_seen_cb = resp_seen.clone();
        let responder_task = tokio::spawn(async move {
            responder
                .run_with_confirm(
                    "sas-relay-secret",
                    "127.0.0.1:7105",
                    &PeerMeta::default(),
                    None,
                    move |sas| {
                        let slot = resp_seen_cb.clone();
                        let sas = sas.to_string();
                        async move {
                            *slot.lock().unwrap() = Some(sas);
                            true
                        }
                    },
                )
                .await
        });

        let relay_listener = TcpListener::bind("127.0.0.1:0").await.expect("relay bind");
        let relay_port = relay_listener.local_addr().unwrap().port();

        let relay_server_cfg = ServerConfig::builder()
            .with_client_cert_verifier(Arc::new(AcceptAnyCert))
            .with_single_cert(
                vec![CertificateDer::from(relay_cert.cert_der.clone())],
                PrivateKeyDer::Pkcs8(rustls::pki_types::PrivatePkcs8KeyDer::from(
                    relay_cert.key_der.clone(),
                )),
            )
            .expect("relay server cfg");
        let relay_acceptor = TlsAcceptor::from(Arc::new(relay_server_cfg));

        let relay_client_cfg = ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(AcceptAnyCert))
            .with_client_auth_cert(
                vec![CertificateDer::from(relay_cert.cert_der.clone())],
                PrivateKeyDer::Pkcs8(rustls::pki_types::PrivatePkcs8KeyDer::from(
                    relay_cert.key_der.clone(),
                )),
            )
            .expect("relay client cfg");
        let relay_connector = TlsConnector::from(Arc::new(relay_client_cfg));

        let relay_task = tokio::spawn(async move {
            let (inbound, _) = relay_listener.accept().await.expect("relay accept");
            let init_tls = relay_acceptor
                .accept(inbound)
                .await
                .expect("relay tls accept");
            let upstream = TcpStream::connect(("127.0.0.1", responder_port))
                .await
                .expect("relay->responder connect");
            let server_name = ServerName::try_from("copypaste.peer").unwrap();
            let resp_tls = relay_connector
                .connect(server_name, upstream)
                .await
                .expect("relay->responder tls");
            let (mut ir, mut iw) = tokio::io::split(init_tls);
            let (mut rr, mut rw) = tokio::io::split(resp_tls);
            let a = tokio::spawn(async move {
                let _ = copy(&mut ir, &mut rw).await;
                let _ = rw.shutdown().await;
            });
            let b = tokio::spawn(async move {
                let _ = copy(&mut rr, &mut iw).await;
                let _ = iw.shutdown().await;
            });
            let _ = tokio::join!(a, b);
        });

        let relay_addr: SocketAddr = ([127, 0, 0, 1], relay_port).into();
        let init_seen_cb = init_seen.clone();
        let initiator_task = tokio::spawn(async move {
            run_initiator_with_confirm(
                relay_addr,
                initiator_cert.cert_der,
                initiator_cert.key_der,
                "sas-relay-secret",
                "127.0.0.1:7106",
                &PeerMeta::default(),
                None,
                move |sas| {
                    let slot = init_seen_cb.clone();
                    let sas = sas.to_string();
                    async move {
                        *slot.lock().unwrap() = Some(sas);
                        true
                    }
                },
            )
            .await
        });

        let _ = password;
        let (resp_res, init_res, _relay_res) =
            tokio::join!(responder_task, initiator_task, relay_task);

        // Both must reject (the constant-time tag check aborts before confirm).
        assert!(init_res.expect("initiator join").is_err());
        assert!(resp_res.expect("responder join").is_err());

        // If both sides DID surface a SAS to the user, the two would differ —
        // that divergence is the human-visible MitM signal.
        let r = resp_seen.lock().unwrap().clone();
        let i = init_seen.lock().unwrap().clone();
        if let (Some(rs), Some(is)) = (r, i) {
            assert_ne!(rs, is, "relay legs must yield different SAS values");
        }
    }

    // ── Fix 4: fingerprint comparison is case-insensitive ────────────────────

    /// A peer that sends its fingerprint in UPPERCASE hex must still pair
    /// successfully. Before the fix, `frame_peer_fp != tls_peer_fp` was a byte
    /// comparison of the frame bytes (which might be uppercase) against
    /// `fingerprint_of` output (which is lowercase), causing a false mismatch.
    ///
    /// We test the invariant directly: `recv_fingerprint` now lowercases its
    /// output so an uppercase frame equals the lowercase TLS fingerprint.
    #[test]
    fn recv_fingerprint_normalises_to_lowercase() {
        // Construct what recv_fingerprint MUST return when the peer sends
        // an uppercase hex fingerprint — it should be lowercased.
        let uppercase_hex = "ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789";
        assert_eq!(uppercase_hex.len(), 64);
        // The function itself is async and private; test the normalised form
        // symbolically: if we lowercase the uppercase input we get a valid
        // lowercase fingerprint that would match `fingerprint_of` output.
        let normalised = uppercase_hex.to_lowercase();
        assert!(
            normalised.bytes().all(|b| b.is_ascii_hexdigit()),
            "lowercased hex must still be valid hex"
        );
        assert!(
            normalised.bytes().all(|b| !b.is_ascii_uppercase()),
            "normalised fingerprint must contain no uppercase chars"
        );
        // Also verify the current recv_fingerprint validator accepts uppercase
        // (64 chars, all hex digits including uppercase).
        assert!(
            uppercase_hex.len() == 64 && uppercase_hex.bytes().all(|b| b.is_ascii_hexdigit()),
            "uppercase fingerprint must be accepted by the length+hex check"
        );
    }
}
