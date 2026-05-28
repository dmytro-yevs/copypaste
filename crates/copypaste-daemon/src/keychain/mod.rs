use copypaste_core::DeviceKeypair;
use sha2::{Digest, Sha256};
use thiserror::Error;

#[cfg(target_os = "macos")]
use security_framework::base::Error as SfError;
#[cfg(target_os = "macos")]
use security_framework::passwords::{delete_generic_password, get_generic_password};

#[cfg(target_os = "macos")]
pub mod acl;

pub(crate) const SERVICE: &str = "com.copypaste.daemon";
pub(crate) const ACCOUNT: &str = "device-secret-key";

/// Compute the canonical device fingerprint from a raw public key.
///
/// Format: first 16 bytes of `SHA-256(public_key)` rendered as
/// lowercase hex pairs separated by `:` (e.g. `aa:bb:cc:...`).
/// This is the user-visible identifier shown during pairing — keep it short
/// enough for humans to compare on two screens.
pub fn own_fingerprint(public_key: &[u8]) -> String {
    let digest = Sha256::digest(public_key);
    digest[..16]
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<Vec<_>>()
        .join(":")
}

#[derive(Debug, Error)]
pub enum KeychainError {
    #[error("Key is wrong length: expected 32 bytes, got {0}")]
    InvalidLength(usize),
    #[cfg(target_os = "macos")]
    #[error("Keychain error: {0}")]
    Keychain(#[from] SfError),
    #[cfg(not(target_os = "macos"))]
    #[error("Keychain not supported on this platform")]
    Unsupported,
    #[error("Core key error: {0}")]
    Key(#[from] copypaste_core::KeyError),
    // ── v0.3 ACL surface (macOS only) ──────────────────────────────────────
    #[cfg(target_os = "macos")]
    #[error("Keychain ACL trust list is empty — refusing to create unrestricted entry")]
    AclEmpty,
    #[cfg(target_os = "macos")]
    #[error("Cannot encode binary path for Keychain ACL (interior NUL byte)")]
    AclPathEncoding,
    #[cfg(target_os = "macos")]
    #[error("Security.framework call {op} failed with OSStatus {code}")]
    OsStatus { op: &'static str, code: i32 },
    #[cfg(target_os = "macos")]
    #[error("Filesystem I/O: {0}")]
    Io(#[from] std::io::Error),
}

/// Load device keypair from Keychain, or generate and store a new one.
///
/// On v0.3 macOS builds, freshly created entries are written with an ACL
/// pinned to the three CopyPaste binaries (see [`acl::store_with_acl`]).
/// Pre-existing v0.2 entries without an ACL are upgraded by
/// [`acl::rotate_acl_to_current_install`], which is called once at daemon
/// startup separately from this function so that the rotation latency does
/// not block per-component reads.
///
/// Beta-merge audit HIGH #2: also opportunistically re-stores entries
/// written by older builds with the locked-down `ThisDeviceOnly` +
/// `Synchronizable=false` attributes so the secret never leaves the device
/// via iCloud Keychain sync — see `migrate_legacy_accessibility_if_needed`.
pub fn load_or_create() -> Result<DeviceKeypair, KeychainError> {
    #[cfg(target_os = "macos")]
    {
        match get_generic_password(SERVICE, ACCOUNT) {
            Ok(bytes) => {
                // Audit MED #4: wrap the keychain-returned Vec in Zeroizing
                // so the heap buffer is scrubbed when this scope exits, and
                // use a checked conversion instead of `bytes.try_into().unwrap()`.
                let bytes = zeroize::Zeroizing::new(bytes);
                if bytes.len() != 32 {
                    return Err(KeychainError::InvalidLength(bytes.len()));
                }
                let arr: [u8; 32] = (&**bytes)
                    .try_into()
                    .map_err(|_| KeychainError::InvalidLength(bytes.len()))?;
                // Audit HIGH #2 migration: re-store with the locked-down
                // accessibility so legacy items written by pre-fix builds
                // stop syncing to iCloud Keychain on the next run. Failure
                // here is logged but not fatal — the keypair load itself
                // succeeded, and we retry on every cold start.
                //
                // Fix C: a `SecAccessControl` with `ThisDeviceOnly`
                // accessibility requires the `keychain-access-groups`
                // entitlement, which an ad-hoc-signed binary CANNOT carry
                // (see `set_generic_password_locked_down`). On those builds
                // `SecItemAdd`/`SecItemUpdate` returns
                // `errSecMissingEntitlement` (-34018, "A required entitlement
                // isn't present"). We treat that one error code as an EXPECTED
                // degraded state and log it at debug — not warn — so the daemon
                // does not spam an error on every cold start it can never fix.
                // The keypair is still fully usable; only the
                // iCloud-sync-suppression hardening is skipped.
                match migrate_legacy_accessibility_if_needed(&arr) {
                    Ok(()) => {}
                    Err(e) if is_missing_entitlement(&e) => {
                        tracing::debug!(
                            "device key ThisDeviceOnly accessibility hardening \
                             skipped: required keychain entitlement is absent \
                             (expected on ad-hoc-signed builds). The key is \
                             usable; iCloud-sync suppression needs a \
                             Developer-ID-signed build with keychain-access-groups."
                        );
                    }
                    Err(e) => {
                        tracing::warn!(
                            error = %e,
                            "could not migrate device key to ThisDeviceOnly accessibility; will retry on next launch"
                        );
                    }
                }
                Ok(DeviceKeypair::from_secret_bytes(&arr)?)
            }
            Err(_) => {
                let kp = DeviceKeypair::generate();
                // Beta-merge audit MED #3 + #4: pull the secret via the
                // zeroizing accessor so the buffer handed to the Keychain
                // syscall is scrubbed when this function returns.
                let secret = kp.secret_key_bytes_zeroizing();
                // v0.3: create with ACL pinned to the current install.
                let trusted = acl::trusted_binary_paths()?;
                acl::store_with_acl(&secret, &trusted)?;
                let fp = own_fingerprint(&kp.public_key_bytes());
                // Log only the short prefix to keep full fingerprint out of info logs.
                tracing::info!(
                    acl_apps = trusted.len(),
                    "Generated new device keypair with ACL; fingerprint_prefix={}",
                    &fp[..23]
                );
                tracing::debug!("full device fingerprint={}", fp);
                Ok(kp)
            }
        }
    }
    #[cfg(not(target_os = "macos"))]
    Err(KeychainError::Unsupported)
}

/// Delete the stored keypair — used for testing and factory reset.
#[cfg(target_os = "macos")]
pub fn delete_stored() -> Result<(), KeychainError> {
    delete_generic_password(SERVICE, ACCOUNT).map_err(KeychainError::from)
}

// ── HIGH #2: hardened `SecItemAdd` wrapper ─────────────────────────────────────
//
// `security_framework::passwords::set_generic_password` does NOT let you
// specify accessibility, so the item lands with the default
// `kSecAttrAccessibleWhenUnlocked` — which on macOS makes the item
// eligible for iCloud Keychain sync AND for inclusion in a Time Machine
// backup of the system keychain. Both violate the threat model: the X25519
// device secret must never leave the originating device.
//
// We bypass `passwords::set_generic_password` by building the
// `SecItemAdd` query manually with:
//   * `kSecAttrAccessControl` = SecAccessControl built with
//     `ProtectionMode::AccessibleWhenUnlockedThisDeviceOnly` (the only
//     protection that suppresses iCloud sync and Time Machine inclusion).
//   * `kSecAttrSynchronizable` = false (defence-in-depth — duplicate of
//     the `ThisDeviceOnly` accessibility flag, but explicit).
//
// On duplicate (item already exists), we fall back to `SecItemUpdate`
// with the same access-control attribute so an existing legacy entry is
// re-written with the locked-down ACL.

#[cfg(target_os = "macos")]
fn set_generic_password_locked_down(
    service: &str,
    account: &str,
    secret: &[u8],
) -> Result<(), KeychainError> {
    use core_foundation::base::{CFType, TCFType};
    use core_foundation::boolean::CFBoolean;
    use core_foundation::data::CFData;
    use core_foundation::dictionary::CFDictionary;
    use core_foundation::string::CFString;
    use security_framework::access_control::{ProtectionMode, SecAccessControl};
    use security_framework_sys::base::errSecDuplicateItem;
    use security_framework_sys::item::{
        kSecAttrAccessControl, kSecAttrAccount, kSecAttrService, kSecAttrSynchronizable, kSecClass,
        kSecClassGenericPassword, kSecValueData,
    };
    use security_framework_sys::keychain_item::{SecItemAdd, SecItemUpdate};

    // Build the access-control descriptor: WhenUnlockedThisDeviceOnly +
    // no extra constraints (no biometry / passcode prompt — this is a
    // service item, not a user-presence secret).
    let acl = SecAccessControl::create_with_protection(
        Some(ProtectionMode::AccessibleWhenUnlockedThisDeviceOnly),
        0,
    )
    .map_err(KeychainError::from)?;

    // Common attributes shared between add and update queries.
    let class_key: CFString = unsafe { CFString::wrap_under_get_rule(kSecClass) };
    let class_val: CFType =
        unsafe { CFString::wrap_under_get_rule(kSecClassGenericPassword).into_CFType() };
    let service_key: CFString = unsafe { CFString::wrap_under_get_rule(kSecAttrService) };
    let service_val: CFType = CFString::from(service).into_CFType();
    let account_key: CFString = unsafe { CFString::wrap_under_get_rule(kSecAttrAccount) };
    let account_val: CFType = CFString::from(account).into_CFType();

    let value_key: CFString = unsafe { CFString::wrap_under_get_rule(kSecValueData) };
    let value_val: CFType = CFData::from_buffer(secret).into_CFType();
    let acl_key: CFString = unsafe { CFString::wrap_under_get_rule(kSecAttrAccessControl) };
    let acl_val: CFType = acl.into_CFType();
    let sync_key: CFString = unsafe { CFString::wrap_under_get_rule(kSecAttrSynchronizable) };
    let sync_val: CFType = CFBoolean::false_value().into_CFType();

    // Add query: identity + value + ACL + synchronizable=false.
    let add_pairs: Vec<(CFString, CFType)> = vec![
        (class_key.clone(), class_val.clone()),
        (service_key.clone(), service_val.clone()),
        (account_key.clone(), account_val.clone()),
        (value_key.clone(), value_val.clone()),
        (acl_key.clone(), acl_val.clone()),
        (sync_key.clone(), sync_val.clone()),
    ];
    let add_params = CFDictionary::from_CFType_pairs(&add_pairs);
    let mut ret: core_foundation_sys::base::CFTypeRef = std::ptr::null();
    let status = unsafe { SecItemAdd(add_params.as_concrete_TypeRef(), &mut ret) };

    if status == 0 {
        return Ok(());
    }
    if status != errSecDuplicateItem {
        return Err(KeychainError::from(SfError::from_code(status)));
    }

    // Item already exists — update value + ACL + synchronizable so legacy
    // items get re-written with the locked-down accessibility.
    let lookup_pairs: Vec<(CFString, CFType)> = vec![
        (class_key, class_val),
        (service_key, service_val),
        (account_key, account_val),
    ];
    let update_pairs: Vec<(CFString, CFType)> = vec![
        (value_key, value_val),
        (acl_key, acl_val),
        (sync_key, sync_val),
    ];
    let lookup = CFDictionary::from_CFType_pairs(&lookup_pairs);
    let update = CFDictionary::from_CFType_pairs(&update_pairs);
    let status =
        unsafe { SecItemUpdate(lookup.as_concrete_TypeRef(), update.as_concrete_TypeRef()) };
    if status == 0 {
        Ok(())
    } else {
        Err(KeychainError::from(SfError::from_code(status)))
    }
}

/// macOS `errSecMissingEntitlement` ("A required entitlement isn't present").
///
/// Not exported by `security-framework-sys`, so we pin the literal from
/// `<Security/SecBase.h>`. Returned by `SecItemAdd`/`SecItemUpdate` when a
/// `SecAccessControl` requiring `ThisDeviceOnly` accessibility is used by a
/// binary lacking the `keychain-access-groups` entitlement (i.e. any ad-hoc
/// signed build — ad-hoc signatures cannot carry that entitlement).
#[cfg(target_os = "macos")]
const ERR_SEC_MISSING_ENTITLEMENT: i32 = -34018;

/// True iff `e` is the keychain `errSecMissingEntitlement` failure. Used to
/// downgrade the `ThisDeviceOnly` migration failure from a per-launch WARN to
/// a one-line DEBUG on builds that can never carry the entitlement.
#[cfg(target_os = "macos")]
fn is_missing_entitlement(e: &KeychainError) -> bool {
    matches!(e, KeychainError::Keychain(sf) if sf.code() == ERR_SEC_MISSING_ENTITLEMENT)
}

/// Re-write the existing device-key entry under the locked-down ACL.
///
/// Called from `load_or_create`'s read path so any item written by a
/// pre-fix build (default `kSecAttrAccessibleWhenUnlocked`, iCloud-sync
/// eligible) is migrated on the next launch. We always do the rewrite —
/// the API has no read-side accessor for the current accessibility, and
/// `SecItemUpdate` with an identical ACL is a no-op cost-wise (no user
/// prompt, single round-trip).
#[cfg(target_os = "macos")]
fn migrate_legacy_accessibility_if_needed(secret: &[u8; 32]) -> Result<(), KeychainError> {
    set_generic_password_locked_down(SERVICE, ACCOUNT, secret)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fix C: the `errSecMissingEntitlement` classifier must recognise
    /// OSStatus -34018 (and only that code) so the daemon downgrades the
    /// ThisDeviceOnly migration failure to a quiet DEBUG on ad-hoc builds.
    #[cfg(target_os = "macos")]
    #[test]
    fn is_missing_entitlement_matches_only_minus_34018() {
        let missing = KeychainError::Keychain(SfError::from_code(ERR_SEC_MISSING_ENTITLEMENT));
        assert!(is_missing_entitlement(&missing));

        // A different keychain error must NOT be classified as missing-entitlement.
        let other = KeychainError::Keychain(SfError::from_code(-25300)); // errSecItemNotFound
        assert!(!is_missing_entitlement(&other));

        // A non-keychain variant must not match either.
        assert!(!is_missing_entitlement(&KeychainError::InvalidLength(7)));
    }

    #[test]
    fn own_fingerprint_is_sha256_prefix() {
        let pk = [0u8; 32];
        let fp = own_fingerprint(&pk);
        // SHA-256 of 32 zero bytes is known: 66687aadf862bd776c8fc18b8e9f8e20...
        assert!(fp.starts_with("66:68:7a:ad:f8:62:bd:77:6c:8f:c1:8b:8e:9f:8e:20"));
        assert_eq!(fp.matches(':').count(), 15); // 16 bytes = 15 colons
    }

    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "requires interactive Keychain access; run manually with `cargo test -- --ignored`"]
    #[allow(deprecated)]
    fn load_or_create_returns_keypair() {
        let _ = delete_stored();
        let kp = load_or_create().expect("should create keypair");
        assert_eq!(kp.secret_key_bytes().len(), 32);
        delete_stored().unwrap();
    }

    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "requires interactive Keychain access; run manually with `cargo test -- --ignored`"]
    #[allow(deprecated)]
    fn load_or_create_is_idempotent() {
        let _ = delete_stored();
        let kp1 = load_or_create().unwrap();
        let kp2 = load_or_create().unwrap();
        assert_eq!(kp1.secret_key_bytes(), kp2.secret_key_bytes());
        delete_stored().unwrap();
    }

    /// Audit HIGH #2: structural test — verify the new accessibility-aware
    /// setter rejects nothing on the happy path and is callable. Full
    /// round-trip verification (read back + check accessibility attribute)
    /// requires interactive Keychain access and lives in
    /// `tests/keychain_macos.rs` with `#[ignore]`.
    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "requires interactive Keychain access; run manually with `cargo test -- --ignored`"]
    fn set_generic_password_locked_down_round_trips() {
        let service = "com.copypaste.daemon.test.locked_down";
        let account = "test-account";
        let secret = [0xABu8; 32];
        // Cleanup any leftover from a previous failed run.
        let _ = delete_generic_password(service, account);
        set_generic_password_locked_down(service, account, &secret)
            .expect("locked-down add should succeed");
        // Second call must hit the SecItemUpdate path (errSecDuplicateItem).
        set_generic_password_locked_down(service, account, &secret)
            .expect("locked-down add on duplicate should fall back to update");
        let readback = get_generic_password(service, account).expect("readback");
        assert_eq!(readback, &secret[..]);
        delete_generic_password(service, account).expect("cleanup");
    }
}
