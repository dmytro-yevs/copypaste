//! macOS Keychain backend.
//!
//! Selected by the target alone: `security-framework` is declared under
//! `[target.'cfg(target_os = "macos")'.dependencies]`, so it is already absent
//! from every other build and a cargo feature could only add a way to forget
//! it. It was one — every shipped macOS binary took the file backend, because
//! nothing passed `--features macos-keychain`.

use std::path::Path;

use zeroize::Zeroizing;

use super::super::keys::random_secret;
use super::{CryptoError, DeviceSecret, Lookup, KEYSTORE_ACCOUNT, KEYSTORE_SERVICE, KEY_LEN};

/// `errSecItemNotFound`. The *only* status that means "no entry exists" and so
/// the only one that can lead to a fresh device secret (port manifest 02,
/// I-20). Locked (-25308), auth-failed (-25293), missing entitlement (-34018),
/// timeout and I/O all leave the entry's state unknown.
const ERR_SEC_ITEM_NOT_FOUND: i32 = -25300;

/// `data_dir` is ignored, and that is correct rather than an oversight: the
/// Keychain item is scoped to the login keychain, not to a directory. `--data-dir`
/// gives an isolated *database*, keyed from the same device secret — which is
/// what I-10's frozen service/account pair requires, since scoping the account
/// name to a path would rename it.
pub(super) fn load(_data_dir: &Path) -> Result<Lookup, CryptoError> {
    match security_framework::passwords::get_generic_password(KEYSTORE_SERVICE, KEYSTORE_ACCOUNT) {
        Ok(bytes) => {
            // Zeroize the Keychain-returned buffer even on the error path
            // (I-12, audit MED #4).
            let bytes = Zeroizing::new(bytes);
            let secret: [u8; KEY_LEN] = bytes.as_slice().try_into().map_err(|_| {
                CryptoError::KeystoreEntryUnusable("the stored device secret is the wrong length")
            })?;
            Ok(Lookup::Found(Zeroizing::new(secret)))
        }
        Err(e) if e.code() == ERR_SEC_ITEM_NOT_FOUND => Ok(Lookup::Absent),
        Err(_) => Err(CryptoError::KeystoreUnavailable(
            "the keychain is locked or access was denied",
        )),
    }
}

pub(super) fn create(_data_dir: &Path) -> Result<DeviceSecret, CryptoError> {
    let secret = Zeroizing::new(random_secret());
    security_framework::passwords::set_generic_password(
        KEYSTORE_SERVICE,
        KEYSTORE_ACCOUNT,
        secret.as_ref(),
    )
    .map_err(|_| CryptoError::KeystoreUnavailable("could not write to the keychain"))?;
    Ok(secret)
}
