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
pub const UNIFFI_ABI_VERSION: u32 = 1;

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
