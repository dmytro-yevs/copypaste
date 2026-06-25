//! QR-based and direct bootstrap pairing FFI exports.
//!
//! Covers: `PairingQrPayload`, `ScannedPairing`, `DeviceCert`, `BootstrapResult`,
//! `SyncProvisioning`, `build_pairing_qr`, `parse_pairing_qr`,
//! `generate_device_cert`, `bootstrap_pair_initiator`, and private helpers
//! `build_android_peer_meta`, `bootstrap_result_from_pairing`,
//! `confirmed_pairing_from`.

use std::sync::OnceLock;

use crate::{pairing, panic_boundary, CopypasteError};

// ── QR device pairing ───────────────────────────────────────────────────────
//
// The QR code is purely a transport for the existing PAKE pairing material.
// `pake_password` is the base64url rendering of the single-use token; it is fed
// into the existing password-authenticated pairing flow in place of the
// manually-typed code, preserving every property of that handshake.

/// FFI result of [`build_pairing_qr`].
pub struct PairingQrPayload {
    pub qr: String,
    pub pake_password: String,
}

/// FFI result of [`parse_pairing_qr`].
pub struct ScannedPairing {
    pub fingerprint: String,
    pub device_id: String,
    pub device_name: String,
    pub addr_hint: String,
    pub pake_password: String,
}

/// Build a QR pairing payload (display side). Generates a fresh single-use
/// token internally and returns both the encoded QR string and the PAKE
/// password derived from that token.
pub fn build_pairing_qr(
    fingerprint: String,
    device_id: String,
    device_name: String,
    addr_hint: String,
) -> Result<PairingQrPayload, CopypasteError> {
    panic_boundary::catch_result(|| {
        let payload =
            copypaste_core::PairingPayload::new(fingerprint, device_id, device_name, addr_hint)
                // P2pError is semantically correct here: QR payload generation is
                // pairing infrastructure (token generation / encoding), not a
                // decryption step.  DecryptionFailed was a copy-paste mistake from
                // parse_pairing_qr (the scan side) and is misleading to Kotlin
                // callers trying to distinguish pairing vs. crypto failures.
                .map_err(|e| CopypasteError::P2pError {
                    reason: e.to_string(),
                })?;
        let pake_password = payload.token.to_pake_password();
        let qr = payload.encode();
        Ok(PairingQrPayload { qr, pake_password })
    })
}

/// Parse a scanned QR payload (scan side). Returns the peer pairing material,
/// including the PAKE password to drive the initiator handshake.
///
/// A malformed or unsupported-version payload yields
/// [`CopypasteError::DecryptionFailed`] (reused as the generic parse error so
/// no new FFI error variant / ABI break is needed).
pub fn parse_pairing_qr(payload: String) -> Result<ScannedPairing, CopypasteError> {
    panic_boundary::catch_result(|| {
        let parsed = copypaste_core::PairingPayload::decode(&payload).map_err(|e| {
            CopypasteError::DecryptionFailed {
                reason: e.to_string(),
            }
        })?;
        let pake_password = parsed.token.to_pake_password();
        Ok(ScannedPairing {
            fingerprint: parsed.fingerprint,
            device_id: parsed.device_id,
            device_name: parsed.device_name,
            addr_hint: parsed.addr_hint,
            pake_password,
        })
    })
}

// ---------------------------------------------------------------------------
// P2P pairing FFI — drive the EXISTING copypaste-p2p stack from Android.
//
// Android does NOT reimplement P2P. These wrappers expose the same mTLS cert
// generation and bootstrap PAKE pairing the macOS daemon uses, so the
// fingerprints Android generates/pins are bit-for-bit what the desktop side
// expects. The synchronous UniFFI surface blocks on a long-lived multi-thread
// tokio runtime (the bootstrap handshake drives concurrent TLS read/write).
// ---------------------------------------------------------------------------

/// Process-wide tokio runtime backing the blocking P2P FFI wrappers.
///
/// A single multi-thread runtime is created lazily on first pairing call and
/// reused for the life of the process. Multi-thread is required: the bootstrap
/// handshake interleaves framed TLS reads and writes that would deadlock on a
/// current-thread runtime under `block_on`.
///
/// `OnceLock` only lets us store a fully-initialised value, so we store a
/// `Result` (via an `Option`) to propagate build failures to callers instead
/// of panicking across the FFI boundary. The `Option` is always `Some` after
/// the first call; `None` is unreachable in practice but handled for
/// soundness.
pub(crate) static RUNTIME: OnceLock<Result<tokio::runtime::Runtime, String>> = OnceLock::new();

/// Return a reference to the shared multi-thread runtime, or an error if it
/// could not be built. Never panics — callers surface the error as
/// `CopypasteError::P2pError` so the JVM is not killed.
pub(crate) fn runtime() -> Result<&'static tokio::runtime::Runtime, CopypasteError> {
    RUNTIME
        .get_or_init(|| {
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .map_err(|e| format!("failed to build tokio runtime for P2P FFI: {e}"))
        })
        .as_ref()
        .map_err(|e| CopypasteError::P2pError { reason: e.clone() })
}

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

/// HB-1b (ABI 14): map a completed `BootstrapPairing` into the FFI
/// [`BootstrapResult`], carrying the PEER's `peer_*` metadata through so Kotlin
/// can persist + render it. Shared by the QR-initiator path (the discovery paths
/// build a [`pairing::ConfirmedPairing`] instead).
pub fn bootstrap_result_from_pairing(
    pairing: copypaste_p2p::bootstrap::BootstrapPairing,
) -> BootstrapResult {
    BootstrapResult {
        peer_fingerprint: pairing.peer_fingerprint,
        peer_sync_addr: pairing.peer_sync_addr,
        session_key: pairing.session_key.as_bytes().to_vec(),
        peer_provisioning: pairing.peer_provisioning.map(Into::into),
        peer_model: pairing.peer_model,
        peer_os: pairing.peer_os,
        peer_app_version: pairing.peer_app_version,
        peer_local_ip: pairing.peer_local_ip,
        peer_public_ip: pairing.peer_public_ip,
        peer_device_id: pairing.peer_device_id,
    }
}

/// HB-1b (ABI 14): map a completed `BootstrapPairing` into the discovery-path
/// [`pairing::ConfirmedPairing`], carrying the PEER's `peer_*` metadata through
/// so the polled [`pairing::PairStatus`] surfaces it to Kotlin on `confirmed`.
/// Shared by both discovery paths (standing responder + `pair_with_discovered`).
pub fn confirmed_pairing_from(
    p: copypaste_p2p::bootstrap::BootstrapPairing,
) -> pairing::ConfirmedPairing {
    pairing::ConfirmedPairing {
        peer_fingerprint: p.peer_fingerprint,
        peer_sync_addr: p.peer_sync_addr,
        session_key: p.session_key.as_bytes().to_vec(),
        peer_provisioning: p.peer_provisioning.map(Into::into),
        peer_model: p.peer_model,
        peer_os: p.peer_os,
        peer_app_version: p.peer_app_version,
        peer_local_ip: p.peer_local_ip,
        peer_public_ip: p.peer_public_ip,
        peer_device_id: p.peer_device_id,
    }
}

// ── Discovery + SAS pairing (ABI 12 — Android parity for LAN discovery) ──────
//
// The Android analog of the macOS daemon's discovery-pairing path. Drives the
// SAME `copypaste_p2p` discovery (mDNS browse/advertise) + bootstrap PAKE stack
// the desktop uses, wired to a POLLED state machine (UniFFI cannot pass an async
// Rust callback). Kotlin starts discovery once, polls `list_discovered`, calls
// `pair_with_discovered` to initiate, polls `pair_get_sas` for the SAS, then
// confirms/aborts. The standing responder bound on `bport` makes the Android
// device pairable FROM macOS. See `pairing.rs` for the full security contract.

/// Start LAN discovery + the standing SAS-pairing responder. Idempotent: a
/// second call tears down and replaces the previous discovery/responder tasks
/// (restart-in-place after a roster / port change).
///
/// Advertises this device over mDNS with the v2 `bport` TXT key (so macOS peers
/// can dial it for SAS pairing) and browses for peers. ALSO binds a standing
/// `BootstrapResponder` on `bport` that accepts inbound discovery-pair
/// connections and runs `run_with_confirm` wired to the SAME coordinator with
/// the `Responder` role — this is what makes Android pairable FROM macOS.
///
/// `cert_der`/`key_der` are this device's mTLS identity (`generate_device_cert`);
/// `sync_port` is the P2P sync-listener port advertised in mDNS; `bport` is the
/// fixed TCP port the standing bootstrap responder binds (advertised so
/// initiators know where to dial). `key_der` is secret — the caller must zero
/// the ByteArray after the call and never log it.
///
/// Errors: [`CopypasteError::P2pError`] if the discovery registration, mDNS
/// daemon, or the standing responder bind fails.
#[allow(clippy::too_many_arguments)] // FFI contract: identity + ports + names.
pub fn start_discovery(
    device_id: String,
    device_name: String,
    sync_port: u16,
    bport: u16,
    cert_der: &[u8],
    key_der: &[u8],
    // HB-1a (ABI 14) / PG-28 (ABI 18): THIS device's own metadata, threaded
    // into the standing responder loop so a macOS-INITIATED discovery pair
    // records real Android info. `device_name` is already a param; the standing
    // responder reuses it for `PeerMeta.device_name`.
    device_model: Option<String>,
    os_version: Option<String>,
    app_version: Option<String>,
    local_ip: Option<String>,
    // ABI 18 (PG-28): STUN-derived WAN address. Kotlin collects it via
    // `StunUtils.queryPublicIp` before starting discovery. `None` when
    // `collect_public_ip` is false or STUN failed.
    public_ip: Option<String>,
) -> Result<(), CopypasteError> {
    panic_boundary::catch_result(|| {
        let rt = runtime()?;
        let cert_der = cert_der.to_vec();
        let key_der = key_der.to_vec();
        // Assemble the responder's PeerMeta once; reuse `device_name` (already a
        // param) for the friendly name field.
        let own_meta = build_android_peer_meta(
            Some(device_name.clone()),
            device_model,
            os_version,
            app_version,
            local_ip,
            public_ip,
        );

        // Build + register the discovery service (advertise with bport so we are
        // a v2 peer macOS can pair with) and start its browse task.
        let discovery = std::sync::Arc::new(copypaste_p2p::discovery::DiscoveryService::new());
        discovery
            .register_with_bport(sync_port, device_id.clone(), device_name.clone(), bport)
            .map_err(|e| pairing::p2p_err(format!("discovery register failed: {e}")))?;
        let discovery_for_start = std::sync::Arc::clone(&discovery);
        let browse_task = rt.spawn(async move {
            // `start` returns a JoinHandle for the internal browse loop; await it
            // so this task lives as long as discovery is running. A start error
            // just ends the task (discovery simply yields no peers).
            if let Ok(handle) = discovery_for_start.start().await {
                let _ = handle.await;
            }
        });

        // Spawn the standing bootstrap responder: re-bind `bport`, accept ONE
        // inbound discovery-pair connection per iteration, run `run_with_confirm`
        // wired to the SAME coordinator with the Responder role.
        let responder_task = rt.spawn(standing_responder_loop(bport, cert_der, key_der, own_meta));

        pairing::global().install(discovery, browse_task, responder_task);
        Ok(())
    })
}

/// The standing-responder accept loop (Responder role). Re-binds `bport` for
/// each inbound pairing attempt and runs the confirm-gated responder handshake
/// wired to the global coordinator. Never logs key/SAS bytes.
async fn standing_responder_loop(
    bport: u16,
    cert_der: Vec<u8>,
    key_der: Vec<u8>,
    // HB-1a (ABI 14): this device's own metadata, advertised to the macOS
    // initiator on every accepted pairing (was `PeerMeta::default()`).
    own_meta: copypaste_p2p::bootstrap::PeerMeta,
) {
    use copypaste_p2p::bootstrap::BootstrapResponder;

    let coordinator = std::sync::Arc::clone(&pairing::global().coordinator);
    loop {
        // Bind the fixed bport afresh each iteration. A *listening* socket that
        // is dropped (not connected) never enters TIME_WAIT, so re-binding the
        // same port succeeds immediately (mirrors the macOS standing responder).
        let responder =
            match BootstrapResponder::bind_on(bport, cert_der.clone(), key_der.clone()).await {
                Ok(r) => r,
                Err(_e) => {
                    // Bind failed (port busy / transient). Back off briefly and retry
                    // so a momentary conflict does not permanently disable inbound
                    // pairing. Never log the error verbatim (no secrets, but keep it
                    // quiet — this loop is hot).
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    continue;
                }
            };

        // Only accept an inbound pairing when idle (single active pairing). If a
        // pairing is already in flight, drop this responder and loop; the next
        // bind happens once the previous one finishes.
        if !coordinator.try_begin(pairing::PairingRole::Responder) {
            drop(responder);
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            continue;
        }

        let confirm_coord = std::sync::Arc::clone(&coordinator);
        // The discovery path uses a FIXED well-known PAKE password
        // (`DISCOVERY_PAIRING_PASSWORD`): opaque-ke is asymmetric, so both ends
        // must register the IDENTICAL password or the handshake fails at frame 7
        // before any SAS is derived. Authentication is ENTIRELY via the SAS
        // compare (see `pairing::DISCOVERY_PAIRING_PASSWORD` docs + plan
        // §"SAS design rationale"). The responder advertises no sync_addr here
        // (Android learns the peer's address from the inbound frames / discovery).
        let result = responder
            .run_with_confirm(
                pairing::DISCOVERY_PAIRING_PASSWORD,
                "",
                // HB-1a: advertise this Android device's real metadata.
                &own_meta,
                None,
                move |sas: &str, _peer_fp: &str| {
                    let coord = std::sync::Arc::clone(&confirm_coord);
                    let sas = sas.to_string();
                    async move {
                        // Park on the user's decision, bounded by the SAS window.
                        let rx = coord.enter_awaiting_sas(sas, pairing::PairingRole::Responder);
                        match tokio::time::timeout(pairing::SAS_CONFIRM_TIMEOUT, rx).await {
                            Ok(Ok(accept)) => accept,
                            // Timeout or sender dropped (abort) → reject.
                            _ => false,
                        }
                    }
                },
            )
            .await;

        match result {
            Ok(p) => {
                coordinator.finish(pairing::PairingState::Confirmed(confirmed_pairing_from(p)))
            }
            Err(_e) => {
                // A confirm-rejected SAS, a timeout, an abort, or a network/PAKE
                // failure all land here. Only move out of an active state — if
                // `pair_abort` already set Aborted, leave it. Keys drop/zeroize
                // (nothing persisted). Distinguish timeout vs reject is not
                // observable from the Err alone, so report Aborted unless the
                // coordinator already recorded a terminal state.
                if coordinator.snapshot().is_active() {
                    coordinator.finish(pairing::PairingState::Aborted);
                }
            }
        }
    }
}

/// Stop LAN discovery + the standing responder. Idempotent. Aborts the browse,
/// responder, and any in-flight initiator task and drops the discovery service
/// (releasing the mDNS socket). Any in-flight confirmation is aborted.
pub fn stop_discovery() -> Result<(), CopypasteError> {
    panic_boundary::catch_result(|| {
        pairing::global().stop();
        Ok(())
    })
}

/// Snapshot the peers currently discovered on the LAN. Despite its legacy name
/// (frozen for ABI 14), `paired_fingerprints` now carries the caller's set of
/// already-paired IP HOSTS (a peer's `local_ip` / sync-address host) — NOT cert
/// fingerprints.
///
/// HB-4: the mDNS `device_id` is a random UUID, not a cert fingerprint, so the
/// old fingerprint-compare against `device_id` NEVER matched and paired devices
/// kept showing "Pair". We now mark a peer `paired` when ANY of its resolved
/// `ip_addrs` is in the caller-supplied set. Returns an empty list when
/// discovery is not running.
pub fn list_discovered(
    paired_fingerprints: Vec<String>,
) -> Result<Vec<pairing::DiscoveredPeer>, CopypasteError> {
    panic_boundary::catch_result(|| {
        let Some(discovery) = pairing::global().discovery() else {
            return Ok(Vec::new());
        };
        // Param name is frozen at ABI 14; the values are paired IP hosts.
        let paired_ips: std::collections::HashSet<String> = paired_fingerprints
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect();
        let peers = discovery
            .peers()
            .into_iter()
            .map(|p| {
                let is_paired = p
                    .ip_addrs
                    .iter()
                    .any(|ip| paired_ips.contains(&ip.to_string()));
                pairing::DiscoveredPeer::from_peer_info(p, is_paired)
            })
            .collect();
        Ok(peers)
    })
}

/// Begin pairing (Initiator role) with a discovered peer. Resolves the peer's
/// `bport` + IPv4-first address from discovery, claims the coordinator, and
/// SPAWNS the bootstrap initiator on the shared runtime (does NOT block). Kotlin
/// then polls `pair_get_sas` for the SAS and calls `pair_confirm_sas`.
///
/// `cert_der`/`key_der` are this device's mTLS identity; `sync_addr` is this
/// device's own P2P sync-listener `host:port` (sent in-band); `local_provisioning`
/// is the OPTIONAL sync-account setup this device offers (typically `null` on
/// Android). Errors: [`CopypasteError::P2pError`] if the peer is unknown, lacks a
/// `bport` (v1 peer), advertises no address, or a pairing is already in flight.
#[allow(clippy::too_many_arguments)] // FFI contract: peer id + identity + 5 meta fields.
pub fn pair_with_discovered(
    device_id: String,
    cert_der: &[u8],
    key_der: &[u8],
    sync_addr: String,
    local_provisioning: Option<SyncProvisioning>,
    // HB-1a (ABI 14) / PG-28 (ABI 18): THIS device's own metadata, advertised
    // to the discovered peer during the initiator handshake.
    device_name: Option<String>,
    device_model: Option<String>,
    os_version: Option<String>,
    app_version: Option<String>,
    local_ip: Option<String>,
    // ABI 18 (PG-28): STUN-derived WAN address. Kotlin collects it via
    // `StunUtils.queryPublicIp` before calling this function. `None` when
    // `collect_public_ip` is false or STUN failed.
    public_ip: Option<String>,
) -> Result<(), CopypasteError> {
    panic_boundary::catch_result(|| {
        let rt = runtime()?;
        let global = pairing::global();

        let Some(discovery) = global.discovery() else {
            return Err(pairing::p2p_err("discovery is not running"));
        };
        let peer = discovery
            .resolve_peer(&device_id)
            .ok_or_else(|| pairing::p2p_err(format!("peer {device_id} not found in discovery")))?;
        if peer.bport.is_none() {
            return Err(pairing::p2p_err(
                "peer is a v1 build (no bport) and cannot SAS-pair",
            ));
        }
        let addr = pairing::ipv4_first_addr(&peer)
            .ok_or_else(|| pairing::p2p_err("peer advertised no routable address"))?;

        // Claim the machine (single active pairing). The standing responder uses
        // the same coordinator, so this also refuses while an inbound pairing is
        // in flight.
        if !global
            .coordinator
            .try_begin(pairing::PairingRole::Initiator)
        {
            return Err(pairing::p2p_err("a pairing is already in flight"));
        }

        let coordinator = std::sync::Arc::clone(&global.coordinator);
        let cert_der = cert_der.to_vec();
        let key_der = key_der.to_vec();
        let provisioning = local_provisioning.map(Into::into);
        // ABI 18: build PeerMeta including the STUN-derived public_ip.
        let own_meta = build_android_peer_meta(
            device_name,
            device_model,
            os_version,
            app_version,
            local_ip,
            public_ip,
        );

        let task = rt.spawn(async move {
            use copypaste_p2p::bootstrap::run_initiator_with_confirm;
            let confirm_coord = std::sync::Arc::clone(&coordinator);
            let result = run_initiator_with_confirm(
                addr,
                cert_der,
                key_der,
                // Discovery path: fixed well-known PAKE password; the SAS is the
                // real authenticator (see `pairing::DISCOVERY_PAIRING_PASSWORD`).
                pairing::DISCOVERY_PAIRING_PASSWORD,
                &sync_addr,
                // HB-1a: advertise this Android device's real metadata.
                &own_meta,
                provisioning,
                move |sas: &str, _peer_fp: &str| {
                    let coord = std::sync::Arc::clone(&confirm_coord);
                    let sas = sas.to_string();
                    async move {
                        let rx = coord.enter_awaiting_sas(sas, pairing::PairingRole::Initiator);
                        match tokio::time::timeout(pairing::SAS_CONFIRM_TIMEOUT, rx).await {
                            Ok(Ok(accept)) => accept,
                            _ => false,
                        }
                    }
                },
            )
            .await;

            match result {
                Ok(p) => {
                    coordinator.finish(pairing::PairingState::Confirmed(confirmed_pairing_from(p)))
                }
                Err(_e) => {
                    // Reject/timeout/abort/network failure: keys drop/zeroize,
                    // nothing persisted. Only move out of an active state so an
                    // explicit `pair_abort` (Aborted) is not clobbered.
                    if coordinator.snapshot().is_active() {
                        coordinator.finish(pairing::PairingState::Aborted);
                    }
                }
            }
        });
        global.set_initiator_task(task);
        Ok(())
    })
}

/// Poll the current pairing status. While active, `sas` + `role` are populated;
/// the `peer_*` outputs (incl. the 32-byte `session_key`) are populated ONLY
/// when `state == "confirmed"`. Kotlin persists those then calls `pair_reset`.
/// The `session_key` is secret — zero the ByteArray after KEK-wrapping it.
pub fn pair_get_sas() -> Result<pairing::PairStatus, CopypasteError> {
    panic_boundary::catch_result(|| {
        let state = pairing::global().coordinator.snapshot();
        Ok(pairing::PairStatus::from_state(&state))
    })
}

/// Deliver the local user's accept(`true`)/reject(`false`) SAS decision into the
/// in-flight handshake. A reject drops/zeroizes the session key (nothing
/// persisted). No-op (returns Ok) when no pairing is awaiting confirmation.
pub fn pair_confirm_sas(accept: bool) -> Result<(), CopypasteError> {
    panic_boundary::catch_result(|| {
        pairing::global().coordinator.deliver_decision(accept);
        Ok(())
    })
}

/// Abort the in-flight pairing: cancel the initiator task, drop the confirmation
/// channel (the handshake's confirm await resolves to a rejection → keys
/// drop/zeroize), and move the machine to `aborted`. Idempotent.
pub fn pair_abort() -> Result<(), CopypasteError> {
    panic_boundary::catch_result(|| {
        let global = pairing::global();
        global.abort_initiator();
        global.coordinator.abort();
        Ok(())
    })
}

/// Reset the pairing machine to `idle` (call after observing a terminal state so
/// a fresh pairing may begin). Also aborts any lingering initiator task.
pub fn pair_reset() -> Result<(), CopypasteError> {
    panic_boundary::catch_result(|| {
        let global = pairing::global();
        global.abort_initiator();
        global.coordinator.reset();
        Ok(())
    })
}
