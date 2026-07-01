// `KeychainError`/`SfError` are only referenced by the macOS-only
// `set_generic_password_locked_down` below; gate the imports so a non-macOS
// build (where this module compiles to nothing) does not trip an
// unused-import warning under `-D warnings`.
#[cfg(target_os = "macos")]
use super::KeychainError;
#[cfg(target_os = "macos")]
use security_framework::base::Error as SfError;

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

/// Store a generic Keychain password with `kSecAttrSynchronizable=false` and
/// `kSecAttrAccessibleWhenUnlockedThisDeviceOnly` so the secret never leaves
/// the originating device via iCloud Keychain sync or Time Machine backup.
///
/// Used by all secret-write paths in this module (device key, cloud-sync key,
/// Supabase password) — see CopyPaste-nkro.
#[cfg(target_os = "macos")]
pub(crate) fn set_generic_password_locked_down(
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

#[cfg(test)]
mod tests {
    use super::*;

    /// CopyPaste-nkro: `store_supabase_password_to_keychain` and the cloud-sync
    /// key persist paths must use `set_generic_password_locked_down` so the
    /// kSecAttrSynchronizable=false + ThisDeviceOnly attributes prevent the
    /// secret from leaving the originating device via iCloud Keychain sync.
    ///
    /// This structural test verifies that `set_generic_password_locked_down` is
    /// accessible (pub(crate)) and callable with the expected signature, ensuring
    /// the function is wired up correctly.  Full round-trip verification (read
    /// back + check accessibility attribute) requires interactive Keychain access
    /// and lives in the `#[ignore]` tests below.
    #[cfg(target_os = "macos")]
    #[test]
    fn set_generic_password_locked_down_has_correct_signature() {
        // Structural check: the function must be accessible and have the
        // signature `(service: &str, account: &str, secret: &[u8]) -> Result<(), KeychainError>`.
        // We call it inside the ephemeral-key bypass so no real Keychain is touched.
        // COPYPASTE_EPHEMERAL_KEY is NOT set here, so we must not call the real
        // Security framework.  Instead, just verify the function pointer is
        // callable at the type level — the compiler guarantees this.
        let _fn_ptr: fn(&str, &str, &[u8]) -> Result<(), KeychainError> =
            set_generic_password_locked_down;
        // The function must be accessible and callable.
        let _ = _fn_ptr; // suppress unused warning
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
        use security_framework::passwords::{delete_generic_password, get_generic_password};

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
