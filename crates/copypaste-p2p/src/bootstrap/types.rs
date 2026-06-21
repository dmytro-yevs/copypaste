//! Public types for the bootstrap wire protocol.
//!
//! [`PeerMeta`], [`SyncProvisioning`], and [`BootstrapPairing`] are exchanged
//! over the authenticated bootstrap TLS channel during PAKE pairing.

use crate::pake::SessionKey;
use crate::transport::DeviceFingerprint;

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
///
/// `ZeroizeOnDrop` ensures the `derived_sync_key` bytes are overwritten in
/// memory when ANY `SyncProvisioning` value is dropped — including the transient
/// clone created inside `exchange_peer_meta` and any copies on the receive path.
/// This closes the CopyPaste-34u2 window where the secret key could persist in
/// freed heap pages until overwritten by a future allocation.
#[derive(
    Clone, Default, PartialEq, serde::Serialize, serde::Deserialize, zeroize::ZeroizeOnDrop,
)]
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
