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
pub const UNIFFI_ABI_VERSION: u32 = 6;

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
