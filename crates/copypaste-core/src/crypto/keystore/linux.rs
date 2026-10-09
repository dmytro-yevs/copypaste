//! Linux Secret Service backend.
//!
//! Release Linux stores the device secret in the user's session keyring via
//! the FreeDesktop Secret Service API. GNOME Keyring and KWallet implement the
//! same API, so the backend is independent of X11 versus Wayland and desktop
//! shell. A missing item is the sole `Lookup::Absent` case; a locked keyring,
//! unavailable session bus, malformed item, or duplicate item fails closed.

use std::collections::HashMap;
use std::env;
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};

use rustix::fs::{flock, FlockOperation};
use secret_service::blocking::SecretService;
use secret_service::EncryptionType;
use zeroize::Zeroizing;

use super::super::keys::random_secret;
use super::{CryptoError, DeviceSecret, Lookup, KEYSTORE_ACCOUNT, KEYSTORE_SERVICE, KEY_LEN};

const ATTRIBUTE_SERVICE: &str = "service";
const ATTRIBUTE_ACCOUNT: &str = "account";
const ITEM_LABEL: &str = "CopyPaste device secret";
const CONTENT_TYPE: &str = "application/octet-stream";
const CREATION_LOCK_NAME: &str = "copypaste-device-secret.lock";

/// `data_dir` is deliberately not part of the item identity. Secret Service is
/// user-session scoped, like the macOS Keychain: `--data-dir` relocates the
/// encrypted history but does not create a second device identity. The parent
/// module verifies that a relocated history is never re-keyed before calling
/// `create`.
pub(super) fn load(_data_dir: &Path) -> Result<Lookup, CryptoError> {
    let service = connect()?;
    lookup_in(&service)
}

pub(super) fn create(data_dir: &Path) -> Result<DeviceSecret, CryptoError> {
    // Secret Service's CreateItem is not an atomic "create if absent" across
    // independent application processes. Serialize the lookup/create sequence
    // in the session runtime directory, then re-read inside the lock. This is
    // coordination only; no secret ever reaches the filesystem.
    let _creation_lock = acquire_creation_lock()?;

    match load(data_dir)? {
        Lookup::Found(secret) => return Ok(secret),
        Lookup::Absent => {}
    }

    // The initial F-11 check happens before entering this backend. Check again
    // after waiting for the lock so a concurrent process cannot create a
    // database between that check and the mint.
    if super::history_present(data_dir) {
        return Err(CryptoError::KeystoreUnavailable(
            "a history database is here but its device secret is not",
        ));
    }

    let service = connect()?;
    let collection = service.get_default_collection().map_err(unavailable)?;
    if collection.is_locked().map_err(unavailable)? {
        return Err(CryptoError::KeystoreUnavailable(
            "the Linux keyring is locked",
        ));
    }

    let secret = Zeroizing::new(random_secret());
    collection
        .create_item(
            ITEM_LABEL,
            attributes(),
            secret.as_ref(),
            false,
            CONTENT_TYPE,
        )
        .map_err(unavailable)?;

    // Read back through the same exact match used by every later launch. This
    // verifies the write and preserves the only safe outcome if an unrelated
    // client created a matching item despite the lock.
    match load(data_dir)? {
        Lookup::Found(stored) => Ok(stored),
        Lookup::Absent => Err(CryptoError::KeystoreUnavailable(
            "the Linux keyring did not retain the device secret",
        )),
    }
}

fn connect() -> Result<SecretService<'static>, CryptoError> {
    SecretService::connect(EncryptionType::Dh).map_err(unavailable)
}

fn lookup_in(service: &SecretService<'_>) -> Result<Lookup, CryptoError> {
    let matches = service.search_items(attributes()).map_err(unavailable)?;
    if !matches.locked.is_empty() {
        return Err(CryptoError::KeystoreUnavailable(
            "the Linux keyring is locked",
        ));
    }

    match matches.unlocked.as_slice() {
        [] => Ok(Lookup::Absent),
        [item] => {
            let bytes = Zeroizing::new(item.get_secret().map_err(unavailable)?);
            decode_secret(&bytes).map(Lookup::Found)
        }
        _ => Err(CryptoError::KeystoreEntryUnusable(
            "more than one stored Linux device secret matches this application",
        )),
    }
}

fn attributes() -> HashMap<&'static str, &'static str> {
    HashMap::from([
        (ATTRIBUTE_SERVICE, KEYSTORE_SERVICE),
        (ATTRIBUTE_ACCOUNT, KEYSTORE_ACCOUNT),
    ])
}

fn decode_secret(bytes: &[u8]) -> Result<DeviceSecret, CryptoError> {
    let secret: [u8; KEY_LEN] = bytes.try_into().map_err(|_| {
        CryptoError::KeystoreEntryUnusable("the stored device secret is the wrong length")
    })?;
    Ok(Zeroizing::new(secret))
}

fn acquire_creation_lock() -> Result<File, CryptoError> {
    let runtime_dir = env::var_os("XDG_RUNTIME_DIR").ok_or(CryptoError::KeystoreUnavailable(
        "the Linux runtime directory is unavailable",
    ))?;
    let path = PathBuf::from(runtime_dir).join(CREATION_LOCK_NAME);
    let file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(path)
        .map_err(|_| {
            CryptoError::KeystoreUnavailable("could not coordinate Linux keyring access")
        })?;
    flock(&file, FlockOperation::LockExclusive).map_err(|_| {
        CryptoError::KeystoreUnavailable("could not coordinate Linux keyring access")
    })?;
    Ok(file)
}

fn unavailable(_: secret_service::Error) -> CryptoError {
    CryptoError::KeystoreUnavailable("the Linux keyring could not be reached")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_service_identity_is_the_frozen_service_and_account() {
        assert_eq!(
            attributes(),
            HashMap::from([
                (ATTRIBUTE_SERVICE, "com.copypaste.daemon"),
                (ATTRIBUTE_ACCOUNT, "device-secret-key"),
            ])
        );
    }

    #[test]
    fn only_an_exactly_sized_keyring_secret_is_accepted() {
        assert!(matches!(
            decode_secret(&[7; KEY_LEN - 1]),
            Err(CryptoError::KeystoreEntryUnusable(_))
        ));
        assert!(matches!(
            decode_secret(&[7; KEY_LEN + 1]),
            Err(CryptoError::KeystoreEntryUnusable(_))
        ));
        assert_eq!(*decode_secret(&[7; KEY_LEN]).unwrap(), [7; KEY_LEN]);
    }
}
