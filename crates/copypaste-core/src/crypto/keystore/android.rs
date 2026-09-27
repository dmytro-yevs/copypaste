//! Android Keystore backend. **Never compiled on this host** — no NDK, and
//! `dl.google.com` is unreachable from it. Every line below is unverified;
//! ADR-0003 lists what a first device run would falsify.
//!
//! # Wrapped, not stored
//!
//! The Android Keystore holds *keys*, not arbitrary blobs, and a
//! hardware-backed key is non-exportable by construction — so "put the 32-byte
//! device secret in the Keystore" is not a thing that can be done. What is done
//! instead: an AES-GCM key lives in the Keystore under a frozen alias and never
//! leaves it, and the device secret is sealed with that key and kept as a blob
//! in app-private `SharedPreferences`. Compromise of the file yields
//! ciphertext; the key that opens it is held by keymaster.
//!
//! # No user authentication, no StrongBox
//!
//! Both are product decisions, not omissions.
//!
//! * `setUserAuthenticationRequired(false)`. A clipboard that syncs in the
//!   background must be able to write while the screen is locked. With it true,
//!   `Cipher.init` throws `UserNotAuthenticatedException` on a locked device
//!   and history becomes unreadable at exactly the moment an item arrives.
//!   The consequence to accept: a device unlocked by an attacker can decrypt.
//! * No StrongBox. `setIsStrongBoxBacked(true)` throws
//!   `StrongBoxUnavailableException` on devices without the chip, so it needs a
//!   per-device fallback, and the fallback is the TEE-backed key we already
//!   have. A secure element also has a small key budget and is markedly slower.
//!
//! Because the key is not auth-bound, biometric re-enrolment does **not**
//! invalidate it: that only applies to keys built with
//! `setInvalidatedByBiometricEnrollment`. What does destroy it is clearing the
//! app's data or uninstalling — and both remove the database in the same
//! motion, so the pair stays consistent.
//!
//! # If the key is gone anyway
//!
//! A wrapping key that no longer opens its own blob — a keymaster failure, a
//! restore that carried `SharedPreferences` across devices without the
//! Keystore, an alias replaced under us — surfaces as
//! [`CryptoError::KeystoreEntryUnusable`] and stops there. Per I-20 that is not
//! authorisation to mint: the history in app storage was encrypted under the
//! secret we can no longer read, and a fresh secret would leave the user with a
//! file that looks corrupt. The honest report is that the history is
//! unrecoverable and clearing app data starts over.

use std::collections::HashMap;
use std::path::Path;

use keyring_core::api::CredentialStoreApi;
use keyring_core::{Entry, Error as KeyringError};
use zeroize::Zeroizing;

use super::super::keys::random_secret;
use super::{CryptoError, DeviceSecret, Lookup, KEYSTORE_ACCOUNT, KEYSTORE_SERVICE, KEY_LEN};

/// The `SharedPreferences` file holding the wrapped secret, and the Keystore
/// alias of the key that wraps it — the store uses one name for both. Frozen
/// for the reason the service/account pair is (I-10): a rename abandons the
/// wrapping key and orphans the database it opens.
const STORE_NAME: &str = "copypaste-device-secret";

/// `data_dir` is ignored, and that is correct rather than an oversight.
/// `SharedPreferences` and the Keystore are scoped to the app's UID, not to a
/// directory; Android has exactly one app-private data directory and no
/// `--data-dir` to move it with. The F-11 guard in [`super`] still applies —
/// it asks whether a history is present, which is a question about this one
/// directory either way.
pub(super) fn load(_data_dir: &Path) -> Result<Lookup, CryptoError> {
    match entry()?.get_secret() {
        Ok(bytes) => {
            let bytes = Zeroizing::new(bytes);
            let secret: [u8; KEY_LEN] = bytes.as_slice().try_into().map_err(|_| {
                CryptoError::KeystoreEntryUnusable("the stored device secret is the wrong length")
            })?;
            Ok(Lookup::Found(Zeroizing::new(secret)))
        }
        // The store reports `NoEntry` only when the wrapping key was read
        // successfully and the preferences file holds nothing under our key.
        // That is the unambiguous "no entry exists" I-20 requires, and it is
        // the single arm that can lead to a mint.
        Err(KeyringError::NoEntry) => Ok(Lookup::Absent),
        Err(e) => Err(classify(e)),
    }
}

pub(super) fn create(_data_dir: &Path) -> Result<DeviceSecret, CryptoError> {
    let secret = Zeroizing::new(random_secret());
    entry()?.set_secret(secret.as_ref()).map_err(|_| {
        CryptoError::KeystoreUnavailable("could not write to the Android key store")
    })?;
    Ok(secret)
}

/// Open the store and address our one credential in it.
///
/// Opening is what generates the wrapping key on first run, and what fails if
/// the alias exists but cannot be fetched — so both of those are classified
/// here rather than being mistaken for an absent secret later.
fn entry() -> Result<Entry, CryptoError> {
    let config = HashMap::from([("name", STORE_NAME), ("filename", STORE_NAME)]);
    let store =
        android_native_keyring_store::Store::new_with_configuration(&config).map_err(classify)?;
    store
        .build(KEYSTORE_SERVICE, KEYSTORE_ACCOUNT, None)
        .map_err(classify)
}

/// I-20's classification, in the store's vocabulary.
///
/// `NoEntry` is deliberately absent: it is handled at the one call site that
/// may act on it, so a future arm added here cannot quietly authorise a mint.
fn classify(error: KeyringError) -> CryptoError {
    match error {
        // The blob is there and the wrapping key does not open it, or the
        // store's own record is not what this build wrote. Permanent.
        KeyringError::BadDataFormat(..) | KeyringError::BadStoreFormat(_) => {
            CryptoError::KeystoreEntryUnusable(
                "the Android key store can no longer read the device secret",
            )
        }
        // A JNI failure, a thrown exception, an uninitialised Android context:
        // the entry's state is unknown, and may well be readable next start.
        _ => CryptoError::KeystoreUnavailable("the Android key store could not be reached"),
    }
}
