//! mTLS cert generation + QR-initiator bootstrap PAKE pairing FFI:
//! `DeviceCert`, `BootstrapResult`, `SyncProvisioning`, `generate_device_cert`,
//! `bootstrap_pair_initiator`, and the mapping helpers shared across every
//! Android pairing path (`build_android_peer_meta`,
//! `bootstrap_result_from_pairing`, `confirmed_pairing_from`).
//!
//! Android does NOT reimplement P2P. These wrappers expose the same mTLS cert
//! generation and bootstrap PAKE pairing the macOS daemon uses, so the
//! fingerprints Android generates/pins are bit-for-bit what the desktop side
//! expects. The synchronous UniFFI surface blocks on the shared long-lived
//! multi-thread tokio runtime ([`super::runtime`]) — the bootstrap handshake
//! drives concurrent TLS read/write.

use crate::{pairing, panic_boundary, CopypasteError};

use super::runtime::runtime;

/// FFI result of [`generate_device_cert`]: a fresh self-signed mTLS identity.
///
/// `fingerprint` is `hex(SHA-256(cert_der))` — the SAME value the macOS side
/// pins. Kotlin must persist `cert_der` + `key_der` securely (key_der is
/// secret) and advertise `fingerprint` / `device_id` in the pairing QR.
///
/// # SECURITY NOTE — `key_der` crosses the FFI boundary unzeroized.
/// UniFFI copies it into a Kotlin `ByteArray`. The Kotlin layer MUST zero that
/// array and any copies after use (store in AndroidKeystore; never log/persist
/// the raw bytes). This is a load-bearing contract: failing to do so leaves
/// private key material on the JVM heap until GC.
pub struct DeviceCert {
    pub device_id: String,
    pub fingerprint: String,
    pub cert_der: Vec<u8>,
    pub key_der: Vec<u8>,
}

/// FFI result of [`bootstrap_pair_initiator`]: the outcome of one PAKE pairing.
///
/// `peer_fingerprint` is the responder's pinned cert fingerprint; `session_key`
/// is the 32-byte PAKE+channel-bound key both ends derived.
///
/// # SECURITY NOTE — `session_key` crosses the FFI boundary unzeroized.
/// UniFFI copies it into a Kotlin `ByteArray`. The Kotlin layer MUST zero that
/// array after deriving the content sync key from it — it is a load-bearing
/// contract that must not be skipped, otherwise raw PAKE key material lingers
/// on the JVM heap until GC.
#[derive(Debug)]
pub struct BootstrapResult {
    pub peer_fingerprint: String,
    pub peer_sync_addr: String,
    pub session_key: Vec<u8>,
    /// Sync-account provisioning the PEER advertised over the authenticated
    /// bootstrap tunnel ("QR fully provisions all sync"). `None` when the peer
    /// advertised nothing or is a legacy build. Kotlin persists these later
    /// (Supabase URL/anon key + the derived cloud sync key) so scanning a
    /// configured PC also sets up cloud sync, not just P2P. See
    /// [`SyncProvisioning`].
    pub peer_provisioning: Option<SyncProvisioning>,
    /// HB-1b (ABI 14): the PEER's device metadata, learned in-band over the
    /// authenticated bootstrap tunnel and sourced from `BootstrapPairing.peer_*`.
    /// All `None` when the peer is a legacy build or advertised nothing. Kotlin
    /// persists these on the `PairedPeer` so Wave 3 renders a device card at
    /// parity with macOS. `peer_public_ip` is informational metadata only — never
    /// used for authentication or trust decisions.
    pub peer_model: Option<String>,
    pub peer_os: Option<String>,
    pub peer_app_version: Option<String>,
    pub peer_local_ip: Option<String>,
    pub peer_public_ip: Option<String>,
    /// ABI 17 (CopyPaste-3k6m): the PEER's stable device UUID (from its
    /// `generate_device_cert` / `PeerMeta.device_id`), learned in-band over the
    /// authenticated bootstrap tunnel. `None` for legacy peers that do not
    /// advertise this field. Kotlin persists it as `PairedPeer.peerDeviceId` so
    /// `OriginDeviceFilter` can resolve clipboard item names by UUID instead of
    /// falling back to the TLS cert fingerprint.
    pub peer_device_id: Option<String>,
    /// ABI 19 (CopyPaste-gldr): the PEER's non-secret Supabase/cloud account
    /// id, learned in-band over the authenticated bootstrap tunnel (sourced
    /// from `BootstrapPairing.peer_supabase_account_id`). `None` for legacy
    /// peers or peers with no cloud account configured. Kotlin persists it on
    /// the `PairedPeer` so cross-account pairing mismatches can be detected,
    /// at parity with the macOS daemon (CopyPaste-yw2k).
    pub peer_supabase_account_id: Option<String>,
}

/// FFI mirror of [`copypaste_p2p::bootstrap::SyncProvisioning`].
///
/// Carries the sync-account setup exchanged in-band over the authenticated
/// bootstrap tunnel. The URLs and anon key are non-secret; `derived_sync_key`
/// is the 32-byte DERIVED cloud sync key (NOT the passphrase) and is secret.
///
/// # SECURITY NOTE — `derived_sync_key` crosses the FFI boundary unzeroized.
/// UniFFI copies it into a Kotlin `ByteArray`. The Kotlin layer MUST zero that
/// array after persisting the key (store in AndroidKeystore; never log it) —
/// a load-bearing contract, otherwise raw key material lingers on the JVM heap.
#[derive(Clone)]
pub struct SyncProvisioning {
    pub supabase_url: Option<String>,
    pub supabase_anon_key: Option<String>,
    pub relay_url: Option<String>,
    pub derived_sync_key: Option<Vec<u8>>,
}

impl std::fmt::Debug for SyncProvisioning {
    /// NEVER prints the secret `derived_sync_key` bytes — only a redacted length
    /// marker. The URLs/anon key are non-secret and shown verbatim. Required so
    /// `BootstrapResult`'s `#[derive(Debug)]` does not leak the key.
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

impl From<copypaste_p2p::bootstrap::SyncProvisioning> for SyncProvisioning {
    fn from(p: copypaste_p2p::bootstrap::SyncProvisioning) -> Self {
        // `SyncProvisioning` implements `Drop` (zeroing); clone the fields so we
        // do not attempt to move out of a type that implements `Drop`.
        SyncProvisioning {
            supabase_url: p.supabase_url.clone(),
            supabase_anon_key: p.supabase_anon_key.clone(),
            relay_url: p.relay_url.clone(),
            derived_sync_key: p.derived_sync_key.clone(),
        }
    }
}

impl From<SyncProvisioning> for copypaste_p2p::bootstrap::SyncProvisioning {
    fn from(p: SyncProvisioning) -> Self {
        copypaste_p2p::bootstrap::SyncProvisioning {
            supabase_url: p.supabase_url,
            supabase_anon_key: p.supabase_anon_key,
            relay_url: p.relay_url,
            derived_sync_key: p.derived_sync_key,
        }
    }
}

/// Generate a fresh self-signed ECDSA P-256 mTLS certificate for this device,
/// reusing `copypaste_p2p::SelfSignedCert` (the exact mechanism the daemon and
/// P2P transport use). A random `device_id` (UUID) is generated and used as the
/// cert CN; the returned `fingerprint` is `fingerprint_of(cert_der)`.
///
/// Errors: [`CopypasteError::P2pError`] if rcgen certificate generation fails.
pub fn generate_device_cert() -> Result<DeviceCert, CopypasteError> {
    panic_boundary::catch_result(|| {
        let device_id = uuid::Uuid::new_v4().to_string();
        let cert = copypaste_p2p::SelfSignedCert::generate(&device_id).map_err(|e| {
            CopypasteError::P2pError {
                reason: e.to_string(),
            }
        })?;
        let fingerprint = copypaste_p2p::fingerprint_of(&cert.cert_der);
        Ok(DeviceCert {
            device_id,
            fingerprint,
            cert_der: cert.cert_der,
            key_der: cert.key_der,
        })
    })
}

/// Run the initiator side of bootstrap PAKE pairing against a responder at
/// `addr_hint` (a `host:port` string), driving `copypaste_p2p::bootstrap::
/// run_initiator` on the shared runtime.
///
/// `cert_der`/`key_der` are this device's mTLS identity (from
/// [`generate_device_cert`]). `pake_password` is the PAKE password derived from
/// the scanned QR token. `sync_addr` is this device's own P2P sync-listener
/// `host:port`, sent in-band so the peer can persist it.
///
/// Errors: [`CopypasteError::P2pError`] for a malformed `addr_hint`, or any
/// `TransportError` (TLS / socket / framing / PAKE failure, wrong password, or
/// a channel-binding MitM abort).
#[allow(clippy::too_many_arguments)] // FFI contract: identity + addr + 5 meta fields.
pub fn bootstrap_pair_initiator(
    addr_hint: String,
    cert_der: &[u8],
    key_der: &[u8],
    pake_password: String,
    sync_addr: String,
    // "QR fully provisions all sync": optional provisioning THIS device sends to
    // the responder. An Android device scanning a configured PC passes `None`
    // (it has nothing to offer yet); the received provisioning comes back in the
    // result's `peer_provisioning`.
    local_provisioning: Option<SyncProvisioning>,
    // HB-1a (ABI 14) / PG-28 (ABI 18): THIS device's own metadata, gathered in
    // Kotlin (`Build.MODEL`, "Android <release>", BuildConfig.VERSION_NAME,
    // device name, LAN IP, and now the STUN WAN IP) and sent in-band so the
    // peer's device card shows real Android info including the public address.
    device_name: Option<String>,
    device_model: Option<String>,
    os_version: Option<String>,
    app_version: Option<String>,
    local_ip: Option<String>,
    // ABI 18 (PG-28): STUN-derived WAN address. Kotlin collects it via
    // `StunUtils.queryPublicIp` before calling this function and passes the
    // result here. `None` when `collect_public_ip` is false or STUN failed.
    public_ip: Option<String>,
) -> Result<BootstrapResult, CopypasteError> {
    panic_boundary::catch_result(|| {
        let addr: std::net::SocketAddr =
            addr_hint
                .parse()
                .map_err(|e: std::net::AddrParseError| CopypasteError::P2pError {
                    reason: format!("invalid addr_hint '{addr_hint}': {e}"),
                })?;

        let pairing = runtime()?
            .block_on(copypaste_p2p::bootstrap::run_initiator(
                addr,
                cert_der.to_vec(),
                key_der.to_vec(),
                &pake_password,
                &sync_addr,
                // ABI 18: build PeerMeta with the STUN public_ip so the macOS
                // peer can use it for NAT traversal / external candidate selection.
                &build_android_peer_meta(
                    device_name,
                    device_model,
                    os_version,
                    app_version,
                    local_ip,
                    public_ip,
                ),
                local_provisioning.map(Into::into),
            ))
            .map_err(|e| CopypasteError::P2pError {
                reason: e.to_string(),
            })?;

        Ok(bootstrap_result_from_pairing(pairing))
    })
}

/// HB-1a (ABI 14) / PG-28 (ABI 18): assemble a
/// `copypaste_p2p::bootstrap::PeerMeta` from the optional device-metadata
/// fields Kotlin gathers and passes across the FFI. Used by every Android
/// pairing path (initiator, discovery initiator, standing responder) so the
/// peer always sees real Android device info instead of `PeerMeta::default()`.
///
/// `public_ip` is the STUN-derived WAN address collected via
/// `resolve_stun_public_ip()` (or Kotlin's `StunUtils.queryPublicIp`).
/// Passing `None` is valid when the user has opted out of `collect_public_ip`
/// or STUN failed — the peer will simply see no public address.
pub fn build_android_peer_meta(
    device_name: Option<String>,
    device_model: Option<String>,
    os_version: Option<String>,
    app_version: Option<String>,
    local_ip: Option<String>,
    public_ip: Option<String>,
) -> copypaste_p2p::bootstrap::PeerMeta {
    copypaste_p2p::bootstrap::PeerMeta {
        model: device_model,
        os_version,
        app_version,
        local_ip,
        device_name,
        // ABI 18 (PG-28): public_ip is now threaded from Kotlin (collected via
        // StunUtils before calling pairWithDiscovered / bootstrap_pair_initiator).
        public_ip,
        device_id: None,
        // Android does not yet compute supabase_account_id locally; the field is
        // additive optional — None is safe and back-compat with all peers.
        supabase_account_id: None,
    }
}

/// HB-1b (ABI 14) / ABI 19 (CopyPaste-gldr): map a completed
/// `BootstrapPairing` into the FFI [`BootstrapResult`], carrying the PEER's
/// `peer_*` metadata (including `peer_supabase_account_id`) through so Kotlin
/// can persist + render it. Shared by the QR-initiator path (the discovery paths
/// build a [`pairing::ConfirmedPairing`] instead).
pub fn bootstrap_result_from_pairing(
    pairing: copypaste_p2p::bootstrap::BootstrapPairing,
) -> BootstrapResult {
    BootstrapResult {
        peer_fingerprint: pairing.peer_fingerprint.into_string(),
        peer_sync_addr: pairing.peer_sync_addr,
        session_key: pairing.session_key.as_bytes().to_vec(),
        peer_provisioning: pairing.peer_provisioning.map(Into::into),
        peer_model: pairing.peer_model,
        peer_os: pairing.peer_os,
        peer_app_version: pairing.peer_app_version,
        peer_local_ip: pairing.peer_local_ip,
        peer_public_ip: pairing.peer_public_ip,
        peer_device_id: pairing.peer_device_id,
        peer_supabase_account_id: pairing.peer_supabase_account_id,
    }
}

/// HB-1b (ABI 14) / ABI 19 (CopyPaste-gldr): map a completed
/// `BootstrapPairing` into the discovery-path [`pairing::ConfirmedPairing`],
/// carrying the PEER's `peer_*` metadata (including
/// `peer_supabase_account_id`) through so the polled [`pairing::PairStatus`]
/// surfaces it to Kotlin on `confirmed`. Shared by both discovery paths
/// (standing responder + `pair_with_discovered`).
pub fn confirmed_pairing_from(
    p: copypaste_p2p::bootstrap::BootstrapPairing,
) -> pairing::ConfirmedPairing {
    pairing::ConfirmedPairing {
        peer_fingerprint: p.peer_fingerprint.into_string(),
        peer_sync_addr: p.peer_sync_addr,
        session_key: p.session_key.as_bytes().to_vec(),
        peer_provisioning: p.peer_provisioning.map(Into::into),
        peer_model: p.peer_model,
        peer_os: p.peer_os,
        peer_app_version: p.peer_app_version,
        peer_local_ip: p.peer_local_ip,
        peer_public_ip: p.peer_public_ip,
        peer_device_id: p.peer_device_id,
        peer_supabase_account_id: p.peer_supabase_account_id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use copypaste_p2p::bootstrap::BootstrapPairing;
    use copypaste_p2p::pake::SessionKey;
    use copypaste_p2p::transport::DeviceFingerprint;

    /// A completed `BootstrapPairing` carrying every optional `peer_*` field,
    /// used by both mapper characterization tests below.
    fn sample_bootstrap_pairing() -> BootstrapPairing {
        BootstrapPairing {
            peer_fingerprint: DeviceFingerprint("deadbeef".to_string()),
            peer_sync_addr: "10.0.0.2:51515".to_string(),
            session_key: SessionKey([0x42u8; 32]),
            sas: "123456".to_string(),
            peer_model: Some("MacBook Air".to_string()),
            peer_os: Some("macOS 15.5".to_string()),
            peer_app_version: Some("0.6.1".to_string()),
            peer_local_ip: Some("10.0.0.2".to_string()),
            peer_device_name: Some("Alice's MacBook".to_string()),
            peer_public_ip: Some("203.0.113.7".to_string()),
            peer_device_id: Some("device-uuid-abc123".to_string()),
            peer_provisioning: None,
            peer_supabase_account_id: Some("supabase-acct-xyz789".to_string()),
        }
    }

    /// Characterization test (ADR-017 split safety net): the manual `Debug`
    /// impl on `SyncProvisioning` must NEVER print the raw `derived_sync_key`
    /// bytes — only a redacted `<N bytes redacted>` marker — while the
    /// non-secret fields print verbatim. This is a security control, not just
    /// a formatting nicety: `BootstrapResult`'s `#[derive(Debug)]` relies on
    /// it to avoid leaking key material into logs.
    #[test]
    fn sync_provisioning_debug_redacts_derived_key() {
        let prov = SyncProvisioning {
            supabase_url: Some("https://xxxx.supabase.co".to_string()),
            supabase_anon_key: Some("anon-key".to_string()),
            relay_url: Some("https://relay.example.com".to_string()),
            derived_sync_key: Some(vec![0xABu8; 32]),
        };
        let debug = format!("{prov:?}");
        assert!(
            !debug.contains("171"), // 0xAB == 171 decimal; would appear if raw bytes leaked.
            "Debug output must never contain the raw derived_sync_key bytes: {debug}"
        );
        assert!(
            debug.contains("32 bytes redacted"),
            "Debug output must show the redacted-length marker: {debug}"
        );
        assert!(
            debug.contains("https://xxxx.supabase.co"),
            "non-secret fields must still print verbatim: {debug}"
        );
    }

    /// Characterization test: converting the p2p `SyncProvisioning` into the
    /// FFI `SyncProvisioning` and back must preserve every field exactly (a
    /// lossless mirror type, not a lossy compression of the wire type).
    #[test]
    fn sync_provisioning_roundtrips_p2p() {
        let original = copypaste_p2p::bootstrap::SyncProvisioning {
            supabase_url: Some("https://xxxx.supabase.co".to_string()),
            supabase_anon_key: Some("anon-key".to_string()),
            relay_url: Some("https://relay.example.com".to_string()),
            derived_sync_key: Some(vec![0x11u8; 32]),
        };

        let ffi: SyncProvisioning = original.clone().into();
        let back: copypaste_p2p::bootstrap::SyncProvisioning = ffi.into();

        assert_eq!(
            back, original,
            "SyncProvisioning must round-trip losslessly"
        );
    }

    /// Characterization test: `build_android_peer_meta` must map every input
    /// field into the matching `PeerMeta` field, and the two fields Android
    /// never sets locally (`device_id`, `supabase_account_id`) must always be
    /// `None` regardless of the other inputs.
    #[test]
    fn build_android_peer_meta_maps_all_fields() {
        let meta = build_android_peer_meta(
            Some("Alice's Pixel".to_string()),
            Some("Pixel 8".to_string()),
            Some("Android 15".to_string()),
            Some("2.0.0".to_string()),
            Some("192.168.1.5".to_string()),
            Some("203.0.113.42".to_string()),
        );

        assert_eq!(meta.device_name.as_deref(), Some("Alice's Pixel"));
        assert_eq!(meta.model.as_deref(), Some("Pixel 8"));
        assert_eq!(meta.os_version.as_deref(), Some("Android 15"));
        assert_eq!(meta.app_version.as_deref(), Some("2.0.0"));
        assert_eq!(meta.local_ip.as_deref(), Some("192.168.1.5"));
        assert_eq!(meta.public_ip.as_deref(), Some("203.0.113.42"));
        assert_eq!(
            meta.device_id, None,
            "Android never sets device_id via build_android_peer_meta"
        );
        assert_eq!(
            meta.supabase_account_id, None,
            "Android never sets supabase_account_id via build_android_peer_meta"
        );
    }

    /// Characterization test: `bootstrap_result_from_pairing` must carry every
    /// `peer_*` metadata field (plus fingerprint/sync-addr/session key) from
    /// the completed `BootstrapPairing` through to the FFI `BootstrapResult`.
    #[test]
    fn bootstrap_result_from_pairing_carries_peer_metadata() {
        let pairing = sample_bootstrap_pairing();
        let result = bootstrap_result_from_pairing(pairing);

        assert_eq!(result.peer_fingerprint, "deadbeef");
        assert_eq!(result.peer_sync_addr, "10.0.0.2:51515");
        assert_eq!(result.session_key, vec![0x42u8; 32]);
        assert_eq!(result.peer_model.as_deref(), Some("MacBook Air"));
        assert_eq!(result.peer_os.as_deref(), Some("macOS 15.5"));
        assert_eq!(result.peer_app_version.as_deref(), Some("0.6.1"));
        assert_eq!(result.peer_local_ip.as_deref(), Some("10.0.0.2"));
        assert_eq!(result.peer_public_ip.as_deref(), Some("203.0.113.7"));
        assert_eq!(result.peer_device_id.as_deref(), Some("device-uuid-abc123"));
        assert_eq!(
            result.peer_supabase_account_id.as_deref(),
            Some("supabase-acct-xyz789")
        );
        assert!(result.peer_provisioning.is_none());
    }

    /// Characterization test: `confirmed_pairing_from` must carry the SAME
    /// `peer_*` metadata through to the discovery-path `ConfirmedPairing` as
    /// `bootstrap_result_from_pairing` does for the QR path.
    #[test]
    fn confirmed_pairing_from_carries_peer_metadata() {
        let pairing = sample_bootstrap_pairing();
        let confirmed = confirmed_pairing_from(pairing);

        assert_eq!(confirmed.peer_fingerprint, "deadbeef");
        assert_eq!(confirmed.peer_sync_addr, "10.0.0.2:51515");
        assert_eq!(confirmed.session_key, vec![0x42u8; 32]);
        assert_eq!(confirmed.peer_model.as_deref(), Some("MacBook Air"));
        assert_eq!(confirmed.peer_os.as_deref(), Some("macOS 15.5"));
        assert_eq!(confirmed.peer_app_version.as_deref(), Some("0.6.1"));
        assert_eq!(confirmed.peer_local_ip.as_deref(), Some("10.0.0.2"));
        assert_eq!(confirmed.peer_public_ip.as_deref(), Some("203.0.113.7"));
        assert_eq!(
            confirmed.peer_device_id.as_deref(),
            Some("device-uuid-abc123")
        );
        assert_eq!(
            confirmed.peer_supabase_account_id.as_deref(),
            Some("supabase-acct-xyz789")
        );
        assert!(confirmed.peer_provisioning.is_none());
    }
}
