//! UniFFI-exported version + ABI compatibility check.
//!
//! Kotlin (or any other UniFFI consumer) calls these functions on app startup
//! to verify it is talking to a compatible build of the Rust core. Bump
//! [`UNIFFI_ABI_VERSION`] whenever the UDL surface or any data contract
//! between Rust and Kotlin breaks in a non-backwards-compatible way.

/// Current UniFFI ABI version exposed to Kotlin.
///
/// Increment this constant whenever the UDL (or any serialized data shape
/// crossing the FFI boundary) changes in a way that is **not** backwards
/// compatible with previously generated Kotlin bindings.
///
/// **v0.3 (ABI 2):** `encrypt_text` / `decrypt_text` gained a leading
/// `item_id: String` parameter for AEAD AAD binding (commit 1c55e57 dropped
/// the legacy empty-AAD fallback). Kotlin generated against ABI 1 will fail
/// `check_compatibility` and must be regenerated.
///
/// **v0.3 (ABI 3):** `CopypasteError` gained a `Panicked { message }`
/// variant (THREAT-MODEL OI-7). UniFFI-exported functions now wrap their
/// bodies with `panic_boundary::catch_result`, so Rust panics that
/// previously aborted the JVM are now reported as
/// `CopypasteError::Panicked` instead. Kotlin generated against ABI 2 is
/// missing the new error variant and must be regenerated.
///
/// **ABI 4 (cloud sync):** Added three cloud-sync FFI functions:
/// `derive_cloud_sync_key`, `cloud_encrypt`, `cloud_decrypt`. These expose
/// the Argon2id-derived SyncKey and XChaCha20-Poly1305 AEAD (schema v5)
/// used by the macOS daemon, enabling end-to-end Supabase sync from Android.
/// Kotlin generated against ABI 3 lacks these symbols and must be regenerated.
///
/// **ABI 5 (QR pairing):** Added `build_pairing_qr` / `parse_pairing_qr` plus
/// the `PairingQrPayload` / `ScannedPairing` records. These expose the
/// `copypaste-core` QR pairing payload (a transport for the existing PAKE
/// material — no new crypto). Kotlin generated against ABI 4 lacks these
/// symbols and must be regenerated.
///
/// **ABI 6 (stable item_id):** The `LocalItem` and `SyncedItem` records each
/// gained a `item_id: String` field carrying the STABLE cross-device identity
/// (minted once at capture, reused on every push/sync) so the daemon keys
/// merge/dedup/LWW on it instead of treating each re-sync as a new item. This
/// changes both records' serialized FFI layout, so Kotlin generated against
/// ABI 5 reads them with the wrong shape and must be regenerated.
///
/// **ABI 7 (file identity on the wire — task #21b):** `SyncedItem` gained two
/// optional fields: `file_name: String?` and `mime: String?`. These are
/// populated for `content_type == "file"` items so the receiver knows the
/// original filename and MIME type without having to parse the at-rest
/// `blob_ref` meta JSON. Kotlin generated against ABI 6 reads `SyncedItem`
/// with the wrong shape (missing two fields) and must be regenerated.
///
/// **ABI 8 (Android→macOS file send):** `LocalItem` gained two optional
/// fields: `file_name: String?` and `mime: String?`. These are set by the
/// Kotlin capture path for `content_type == "file"` items and forwarded
/// verbatim onto the outbound `WireItem` so the macOS daemon's
/// `rewrap_inbound_blob` can reconstruct the original filename and MIME type.
/// Kotlin generated against ABI 7 constructs `LocalItem` without these fields
/// and must be regenerated.
///
/// **ABI 9 (Android settings-SSOT + device-management parity):** A batch of
/// related FFI additions, all breaking the binding surface:
///   * `Config` dictionary + `default_config()` + `clamp_config(Config)` — the
///     canonical user-tunable config mirrored from `copypaste_core::AppConfig`
///     so Android seeds defaults and enforces the SAME floors/ceilings as the
///     macOS daemon (triage B2/B6/B7) instead of hand-mirroring with divergent
///     defaults. Both functions are pure (no I/O).
///   * `revoke_device_audit(db_path, key, fingerprint, name) -> u64` — records
///     a peer revocation in the SQLCipher `revoked_devices` audit table (via
///     `copypaste_core::revoke_device`); feature-gated stub off-live.
///   * `list_revoked_fingerprints(db_path, key) -> [string]` and
///     `list_revoked_peers(db_path, key) -> [RevokedPeer]` — read the audit
///     table newest-first to drive the dialer fast-skip and the audit UI.
///   * `sync_with_peer` gained TWO trailing params: `revoked_fingerprints:
///     [string]` (the load-bearing transport-layer denylist — a revoked peer's
///     dial is refused at the trust layer before any socket opens) and
///     `device_id: string` (stable origin identity, folding in the queued
///     origin_device_id fixwave).
///
/// Kotlin generated against ABI 8 is missing all of the above (and constructs
/// the old `sync_with_peer` arity) and must be regenerated.
///
/// **ABI 10 (QR fully provisions all sync):** Added the `SyncProvisioning`
/// dictionary and threaded it through the QR pairing FFI so scanning the
/// pairing QR on a new device also sets up Supabase + relay (not just P2P),
/// transmitted over the ALREADY-AUTHENTICATED mTLS+PAKE bootstrap tunnel —
/// never in the QR image. Concretely:
///   * New `SyncProvisioning` dictionary `{ supabase_url?, supabase_anon_key?,
///     relay_url?, derived_sync_key? }` — mirrors
///     `copypaste_p2p::bootstrap::SyncProvisioning`. `derived_sync_key` is the
///     32-byte DERIVED cloud sync key (NOT the passphrase) and is secret.
///   * `bootstrap_pair_initiator` gained a trailing optional param
///     `local_provisioning: SyncProvisioning?` (the setup THIS device offers;
///     Android scanning a configured PC passes `null`).
///   * `BootstrapResult` gained a trailing optional field `peer_provisioning:
///     SyncProvisioning?` carrying what the peer advertised, for Kotlin to
///     persist later.
///
/// Kotlin generated against ABI 9 lacks `SyncProvisioning`, constructs the old
/// `bootstrap_pair_initiator` arity, and reads `BootstrapResult` with the wrong
/// shape — it must be regenerated.
///
/// **ABI 11 (inbound P2P listener — so macOS can INITIATE to Android):** Added
/// the persistent inbound mTLS accept loop at parity with the macOS daemon's
/// `accept_loop`, exposed as four new FFI functions plus two new dictionaries:
///   * `start_p2p_listener(listen_port, cert_der, key_der, allowed_fingerprints,
///     revoked_fingerprints, session_keys, local_items, device_id)
///     -> P2pListenerHandle` — binds `0.0.0.0:port` (0 = OS-assigned), registers
///     a listener in a process-global registry, spawns its accept loop on the
///     shared runtime, and returns immediately with the handle + actual port.
///   * `poll_p2p_listener(listener_id) -> [SyncedItem]` — atomically drains the
///     items decrypted from inbound frames since the last poll.
///   * `update_p2p_listener_peers(listener_id, allowed, revoked, session_keys)` —
///     live roster/denylist/session-key refresh without restarting.
///   * `stop_p2p_listener(listener_id)` — cancel + deregister (idempotent).
///   * New dictionary `PeerSessionKey { fingerprint, session_key }` — a peer's
///     32-byte PAKE session key keyed by its pinned cert fingerprint (per-peer
///     decryption, never a global key).
///   * New dictionary `P2pListenerHandle { listener_id, actual_port }`.
///
/// Kotlin generated against ABI 10 lacks all four functions and both
/// dictionaries and must be regenerated.
///
/// **ABI 12 (LAN discovery + SAS pairing — Android parity):** Added the
/// discovery + Short-Authentication-String pairing surface (the Android analog
/// of the macOS daemon's discovery-pairing path), as a POLLED state machine
/// (UniFFI cannot pass an async Rust callback). Eight new FFI functions plus two
/// new dictionaries, and one new field on the existing `Config` dictionary:
///   * `start_discovery(device_id, device_name, sync_port, bport, cert_der,
///     key_der)` — advertise over mDNS with the v2 `bport` TXT key, browse for
///     peers, AND bind a standing `BootstrapResponder` on `bport` (Responder
///     role) so macOS can INITIATE pairing to this device. Idempotent.
///   * `stop_discovery()` — tear down discovery + responder + initiator tasks.
///   * `list_discovered(paired_fingerprints) -> [DiscoveredPeer]` — snapshot the
///     LAN peers, flagging which are already paired.
///   * `pair_with_discovered(device_id, cert_der, key_der, sync_addr,
///     local_provisioning)` — resolve the peer's bport + IPv4-first address and
///     SPAWN the bootstrap initiator (Initiator role) on the shared runtime.
///   * `pair_get_sas() -> PairStatus` — poll the pairing machine; the peer_*
///     outputs (incl. the 32-byte `session_key`) appear only on `confirmed`.
///   * `pair_confirm_sas(accept)` — deliver the user's SAS decision.
///   * `pair_abort()` / `pair_reset()` — abort / reset the machine.
///   * New dictionary `DiscoveredPeer { device_id, device_name, ip_addrs, port,
///     bport?, paired }`.
///   * New dictionary `PairStatus { state, sas?, role?, peer_fingerprint?,
///     peer_sync_addr?, session_key?, peer_provisioning? }`.
///   * New `Config` field `sequence<string> excluded_app_bundle_ids` (maps to
///     `AppConfig::excluded_app_bundle_ids`) so the Android settings UI can
///     render the excluded-apps list — folded in here to avoid a later ABI bump.
///
/// Kotlin generated against ABI 11 lacks all eight functions, both dictionaries,
/// and the new `Config` field, and must be regenerated.
///
/// **ABI 13 (relay-as-database producer — R3b):** Added two FFI functions that
/// expose the shared-account relay inbox derivation so Android co-registers,
/// subscribes to, and pushes to the SAME relay inbox the macOS daemon uses
/// (instead of its wrong per-install device id):
///   * `relay_inbox_id(sync_key) -> string` — the deterministic inbox
///     `device_id` (canonical UUID) from `copypaste_core::derive_relay_inbox_id`.
///   * `relay_public_key_b64(sync_key) -> string` — STANDARD base64 of
///     `copypaste_core::derive_relay_public_key`, the registration public key.
///
/// Both throw `InvalidKeyLength` if the key is not 32 bytes. They wrap the core
/// functions directly (no Kotlin-side HKDF) for a guaranteed byte-match with the
/// daemon. Kotlin generated against ABI 12 lacks both symbols and must be
/// regenerated.
///
/// **ABI 14 (PeerMeta send+receive + P2P drop counters — v0.6.1 HB-1/HB-7):**
/// A batch of additive FFI changes, all breaking the binding surface so Kotlin
/// must be regenerated:
///   * HB-1a — Android now SENDS its own device metadata. The three pairing
///     functions gained five trailing optional params `device_name?,
///     device_model?, os_version?, app_version?, local_ip?` used to build a real
///     `PeerMeta` instead of `PeerMeta::default()`: `bootstrap_pair_initiator`,
///     `start_discovery` (threaded into the standing responder loop), and
///     `pair_with_discovered`. (`public_ip` stays `None` for now.)
///   * HB-1b — Android now RECEIVES the peer's metadata. `BootstrapResult` and
///     the discovery `PairStatus` each gained five trailing optional fields
///     `peer_model?, peer_os?, peer_app_version?, peer_local_ip?,
///     peer_public_ip?`, populated from the `BootstrapPairing.peer_*` fields the
///     p2p crate already carries. Kotlin persists them so Wave 3 renders device
///     cards at parity with macOS.
///   * HB-7a — `P2pSyncResult` gained three per-reason drop counters
///     `items_skipped_decrypt_fail`, `items_skipped_unknown_type`,
///     `items_skipped_missing_blob`, incremented at the previously-silent
///     `continue` sites in the receive loop so "received N stored 0" surfaces
///     WHY items dropped instead of vanishing.
///
/// (AB-6a — the `is_sensitive` threshold parity change to a >= 0.70 confidence
/// gate — is a pure behaviour fix with no signature change, so it does not by
/// itself force a bump; it ships in this ABI for coherence.)
///
/// Kotlin generated against ABI 13 constructs the old pairing-fn arities and
/// reads `BootstrapResult` / `PairStatus` / `P2pSyncResult` with the wrong shape
/// — it must be regenerated.
pub const UNIFFI_ABI_VERSION: u32 = 14;

/// Returns the semantic version of the Rust `copypaste-android` crate
/// (the `version` field from `Cargo.toml`).
pub fn core_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Returns the ABI version the Rust core currently speaks.
///
/// Kotlin compares this against the ABI version baked into its generated
/// bindings; a mismatch means the two were built from incompatible sources.
pub fn uniffi_abi_version() -> u32 {
    UNIFFI_ABI_VERSION
}

/// Reasons a Kotlin/Rust ABI handshake can fail.
#[derive(Debug, thiserror::Error)]
pub enum VersionError {
    #[error("UniFFI ABI mismatch: rust={rust_abi} kotlin={kotlin_abi}")]
    Incompatible { rust_abi: u32, kotlin_abi: u32 },
}

/// Verifies that the Kotlin caller's ABI version matches the Rust core's.
///
/// Returns `Ok(())` on a match, or
/// [`VersionError::Incompatible`] (carrying both versions) on a mismatch.
pub fn check_compatibility(kotlin_abi_version: u32) -> Result<(), VersionError> {
    if kotlin_abi_version == UNIFFI_ABI_VERSION {
        Ok(())
    } else {
        Err(VersionError::Incompatible {
            rust_abi: UNIFFI_ABI_VERSION,
            kotlin_abi: kotlin_abi_version,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_version_is_non_empty() {
        let v = core_version();
        assert!(
            !v.is_empty(),
            "CARGO_PKG_VERSION must resolve at compile time"
        );
        // Sanity check that it looks semver-ish (contains at least one dot).
        assert!(v.contains('.'), "expected semver-style version, got {v}");
    }

    #[test]
    fn uniffi_abi_version_matches_const() {
        assert_eq!(uniffi_abi_version(), UNIFFI_ABI_VERSION);
    }

    #[test]
    fn check_compatibility_accepts_match_and_rejects_mismatch() {
        // Matching version — must succeed.
        check_compatibility(UNIFFI_ABI_VERSION).expect("matching ABI must be Ok");

        // Mismatched version — must return Incompatible carrying both sides.
        let bad = UNIFFI_ABI_VERSION.wrapping_add(1);
        let err = check_compatibility(bad).expect_err("mismatched ABI must error");
        match err {
            VersionError::Incompatible {
                rust_abi,
                kotlin_abi,
            } => {
                assert_eq!(rust_abi, UNIFFI_ABI_VERSION);
                assert_eq!(kotlin_abi, bad);
            }
        }
    }
}
