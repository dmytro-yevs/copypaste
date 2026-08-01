//! macOS Keychain backend.
//!
//! Selected by the target alone: `security-framework` is declared under
//! `[target.'cfg(target_os = "macos")'.dependencies]`, so it is already absent
//! from every other build and a cargo feature could only add a way to forget
//! it. It was one — every shipped macOS binary took the file backend, because
//! nothing passed `--features macos-keychain`.

use std::path::Path;

use security_framework::base::Error as SecurityFrameworkError;
use security_framework::passwords::{generic_password, PasswordOptions};
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
    let options = PasswordOptions::new_generic_password(KEYSTORE_SERVICE, KEYSTORE_ACCOUNT);
    match generic_password(options) {
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
    store_device_secret(&secret)
        .map_err(|_| CryptoError::KeystoreUnavailable("could not write to the keychain"))?;
    Ok(secret)
}

/// CopyPaste-nkro: add and update both apply the attributes that keep the
/// device secret off iCloud Keychain and out of device backups.
fn store_device_secret(secret: &[u8; KEY_LEN]) -> Result<(), SecurityFrameworkError> {
    copypaste_macos_keychain::set_generic_password_locked_down(
        KEYSTORE_SERVICE,
        KEYSTORE_ACCOUNT,
        secret,
    )
}

// ---------------------------------------------------------------------------
// Tests — against the real Keychain
// ---------------------------------------------------------------------------
//
// Every assertion names *which backend answered*: a test satisfied by a value
// coming back is satisfied by the `0600` file this backend exists to replace,
// which is what shipped unnoticed for a year.
//
// Two guards, because the item under test is the real device secret and
// deleting it leaves an installed history unopenable. `#[ignore]` keeps them
// out of `cargo test`; `disposable_keychain` refuses a login keychain even
// when they are asked for by name.

#[cfg(test)]
mod tests {
    use std::process::Command;
    use std::sync::{Mutex, MutexGuard};

    use security_framework::passwords::{
        delete_generic_password, generic_password, set_generic_password, PasswordOptions,
    };

    use super::super::{load_or_create_secret, SECRET_FILE_NAME};
    use super::*;

    /// Set only where the Keychain is disposable. CI creates a throwaway
    /// keychain, makes it the default and sets this; a developer's Mac has
    /// neither.
    const ENV_ARMED: &str = "COPYPASTE_KEYCHAIN_TEST";

    static KEYCHAIN: Mutex<()> = Mutex::new(());

    fn serialised() -> MutexGuard<'static, ()> {
        KEYCHAIN.lock().unwrap_or_else(|held| held.into_inner())
    }

    /// Whether it is safe to write to this machine's default keychain.
    ///
    /// Prints its reason rather than passing quietly: a skipped test that says
    /// nothing is indistinguishable from one that never ran.
    fn disposable_keychain() -> bool {
        if std::env::var_os(ENV_ARMED).is_none() {
            eprintln!(
                "SKIPPED: {ENV_ARMED} is unset. These tests create and delete the real \
                 device-secret Keychain item, which would leave any CopyPaste install on \
                 this machine unable to open its history."
            );
            return false;
        }

        let default = Command::new("security")
            .arg("default-keychain")
            .output()
            .expect("the security tool must be present on macOS");
        let default = String::from_utf8_lossy(&default.stdout).into_owned();
        assert!(
            !default.contains("login.keychain"),
            "{ENV_ARMED} is set but the default keychain is the login keychain. \
             Refusing: this would destroy a real device secret."
        );
        true
    }

    /// No entry, whatever was there before.
    fn clear_entry() {
        match delete_generic_password(KEYSTORE_SERVICE, KEYSTORE_ACCOUNT) {
            Ok(()) => {}
            Err(e) if e.code() == ERR_SEC_ITEM_NOT_FOUND => {}
            Err(e) => panic!("could not clear the test Keychain entry: {}", e.code()),
        }
    }

    fn stored_secret() -> Result<Vec<u8>, SecurityFrameworkError> {
        generic_password(PasswordOptions::new_generic_password(
            KEYSTORE_SERVICE,
            KEYSTORE_ACCOUNT,
        ))
    }

    fn assert_device_only_attributes() {
        let attributes = copypaste_macos_keychain::generic_password_security_attributes(
            KEYSTORE_SERVICE,
            KEYSTORE_ACCOUNT,
        )
        .expect("the device secret security attributes must be readable");
        assert!(
            attributes.when_unlocked_this_device_only,
            "accessibility is not WhenUnlockedThisDeviceOnly"
        );
        assert!(
            !attributes.synchronizable,
            "the device secret is synchronizable"
        );
    }

    fn like_the_real_database(dir: &Path) -> std::path::PathBuf {
        let name = copypaste_ipc::database_path();
        dir.join(name.file_name().expect("the database path has a filename"))
    }

    /// The whole point: a first run puts the secret in the Keychain, a second
    /// run reads that same secret back, and no file is written anywhere.
    #[test]
    #[ignore = "writes to the real macOS Keychain"]
    fn the_keychain_holds_the_secret_and_no_file_is_written() {
        let _lock = serialised();
        if !disposable_keychain() {
            return;
        }
        clear_entry();
        let dir = tempfile::tempdir().unwrap();

        let minted = load_or_create_secret(dir.path()).expect("a first run must mint a secret");

        // Which backend answered. The file backend would have left its 32 bytes
        // in the data directory; the Keychain leaves it empty and answers under
        // the frozen service/account pair (manifest 02, I-10).
        assert!(
            !dir.path().join(SECRET_FILE_NAME).exists(),
            "the file backend answered: {SECRET_FILE_NAME} was written"
        );
        assert_eq!(
            std::fs::read_dir(dir.path()).unwrap().count(),
            0,
            "the data directory should be untouched by a Keychain-backed secret"
        );
        let stored =
            stored_secret().expect("the secret must be readable straight out of the Keychain");
        assert_eq!(stored.as_slice(), minted.as_slice());
        assert_device_only_attributes();

        let reopened = load_or_create_secret(dir.path()).expect("a second run must read it back");
        assert_eq!(*reopened, *minted, "a second run minted a different secret");

        clear_entry();
    }

    /// CopyPaste-nkro: the duplicate path must upgrade a legacy item rather
    /// than changing only its bytes and retaining the default accessibility.
    #[test]
    #[ignore = "writes to the real macOS Keychain"]
    fn a_duplicate_write_updates_data_and_security_attributes() {
        let _lock = serialised();
        if !disposable_keychain() {
            return;
        }
        clear_entry();
        set_generic_password(KEYSTORE_SERVICE, KEYSTORE_ACCOUNT, &[3; KEY_LEN]).unwrap();

        let replacement = [7; KEY_LEN];
        store_device_secret(&replacement).expect("the legacy item must update in place");

        assert_eq!(stored_secret().unwrap(), replacement);
        assert_device_only_attributes();

        clear_entry();
    }

    /// Manifest 02, I-20. `errSecItemNotFound` is the only status that
    /// authorises minting, and this is the only place the constant can be
    /// checked against what the framework actually returns. Get it wrong and
    /// every fresh Mac reports the Keychain as unavailable and never starts.
    #[test]
    #[ignore = "writes to the real macOS Keychain"]
    fn an_absent_entry_reads_as_absent_rather_than_as_a_failure() {
        let _lock = serialised();
        if !disposable_keychain() {
            return;
        }
        clear_entry();
        let dir = tempfile::tempdir().unwrap();

        match load(dir.path()) {
            Ok(Lookup::Absent) => {}
            Ok(Lookup::Found(_)) => panic!("an entry was found after being deleted"),
            Err(e) => panic!(
                "an absent entry must not be an error — ERR_SEC_ITEM_NOT_FOUND \
                 ({ERR_SEC_ITEM_NOT_FOUND}) does not match what the framework returns: {e:?}"
            ),
        }
    }

    /// I-20 from the other side: an entry that is there but unusable must fail
    /// closed. Minting over it would re-key a live history.
    #[test]
    #[ignore = "writes to the real macOS Keychain"]
    fn a_wrong_length_entry_is_unusable_and_is_never_replaced() {
        let _lock = serialised();
        if !disposable_keychain() {
            return;
        }
        clear_entry();
        let dir = tempfile::tempdir().unwrap();
        set_generic_password(KEYSTORE_SERVICE, KEYSTORE_ACCOUNT, b"short").unwrap();

        // `expect_err` would need `Lookup: Debug`, and `Lookup::Found` holds
        // the device secret — deriving it puts 32 bytes of key material one
        // `{:?}` away from a log.
        let Err(err) = load(dir.path()) else {
            panic!("a five-byte secret is not usable");
        };
        assert!(
            matches!(err, CryptoError::KeystoreEntryUnusable(_)),
            "expected KeystoreEntryUnusable, got {err:?}"
        );
        assert!(
            load_or_create_secret(dir.path()).is_err(),
            "an unusable entry must not be minted over"
        );
        assert_eq!(
            stored_secret().unwrap(),
            b"short".to_vec(),
            "the stored entry was overwritten"
        );

        clear_entry();
    }

    /// Security review F-11, on the backend it was written for. The file
    /// backend's copy of this test runs on Linux; this one is the case that
    /// actually ships — a Keychain secret does not travel with `--data-dir`,
    /// so "no entry" beside an existing history means the wrong place, not a
    /// first run.
    #[test]
    #[ignore = "writes to the real macOS Keychain"]
    fn a_history_without_its_secret_is_refused_rather_than_re_keyed() {
        let _lock = serialised();
        if !disposable_keychain() {
            return;
        }
        clear_entry();
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(like_the_real_database(dir.path()), b"SQLCipher").unwrap();

        // Destructured rather than `expect_err` for the reason above: the `Ok`
        // this must not take is a freshly minted device secret, and
        // `expect_err` prints it.
        let Err(err) = load_or_create_secret(dir.path()) else {
            panic!("minting beside an existing history must be refused");
        };
        assert!(
            matches!(err, CryptoError::KeystoreUnavailable(_)),
            "expected KeystoreUnavailable, got {err:?}"
        );
        assert!(
            stored_secret().is_err(),
            "a secret was minted into the Keychain anyway"
        );
    }
}
