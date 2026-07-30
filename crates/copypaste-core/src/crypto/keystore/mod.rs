//! Where the device secret lives, per platform.
//!
//! Three backends — `macos.rs`, `android.rs`, `file.rs` — selected by the
//! target alone, behind one entry point, [`load_or_create_secret`].
//!
//! # The rule every backend obeys
//!
//! Port manifest 02, I-20: **only an unambiguous "no entry exists" authorises
//! minting a fresh secret.** A backend answers [`Lookup::Absent`] for that and
//! for nothing else; every other failure is an error, because a secret that
//! cannot be read is not the same as a secret that is not there, and minting
//! over an existing one orphans the SQLCipher database. v1 matched `Err(_)`,
//! minted, and produced `SQLITE_NOTADB` plus a dead daemon.
//!
//! The mint *decision* is taken here rather than in each backend, so there is
//! one copy of it instead of three — and so the F-11 guard in
//! [`load_or_create_secret`] cannot be forgotten by a backend added later.

use std::path::Path;

use zeroize::Zeroizing;

use super::{CryptoError, KEY_LEN};

/// Keystore service name, shared by the macOS Keychain item and the Android
/// credential store. Frozen (port manifest 02, I-10) — renaming it makes an
/// existing install's database unopenable. Defined unconditionally even though
/// only two of the three backends read it: moving a frozen identifier behind a
/// `cfg` is how it drifts.
#[allow(dead_code)]
const KEYSTORE_SERVICE: &str = "com.copypaste.daemon";

/// Keystore account name for the device secret. Frozen (I-10).
#[allow(dead_code)]
const KEYSTORE_ACCOUNT: &str = "device-secret-key";

/// Filename of the development-only file-backed secret store.
#[allow(dead_code)]
const SECRET_FILE_NAME: &str = "device_secret.key";

/// What a backend returns: 32 bytes, zeroized on drop (I-12).
pub(super) type DeviceSecret = Zeroizing<[u8; KEY_LEN]>;

/// A backend's answer to "is there a device secret here?".
///
/// Two cases, not three: "it might be there but I could not tell" is an `Err`,
/// never [`Lookup::Absent`] (I-20).
pub(super) enum Lookup {
    Found(DeviceSecret),
    Absent,
}

#[cfg(target_os = "macos")]
#[path = "macos.rs"]
mod backend;

#[cfg(target_os = "android")]
#[path = "android.rs"]
mod backend;

#[cfg(not(any(target_os = "macos", target_os = "android")))]
#[path = "file.rs"]
mod backend;

/// The Android backend again, compiled but *not selected*, so that a host with
/// no NDK still type-checks it. Nothing here calls it; the `dead_code` allow is
/// the point rather than an oversight.
#[cfg(all(not(target_os = "android"), feature = "android-keystore-typecheck"))]
#[path = "android.rs"]
#[allow(dead_code)]
mod android;

/// Read the device secret for the history in `data_dir`, minting one only if
/// there is unambiguously none and nothing it could orphan.
pub(super) fn load_or_create_secret(data_dir: &Path) -> Result<DeviceSecret, CryptoError> {
    match backend::load(data_dir)? {
        Lookup::Found(secret) => Ok(secret),
        // Security review F-11. `--data-dir` relocates the database, and the
        // file backend's secret travels with it; a keystore-backed secret does
        // not travel at all. So "no entry" has two meanings — first run, or we
        // looked in the wrong place — and only the first authorises a mint. A
        // history sitting in this directory settles it: minting now re-keys a
        // database that is right there, which is I-20's failure reached by a
        // different route.
        Lookup::Absent if history_present(data_dir) => Err(CryptoError::KeystoreUnavailable(
            "a history database is here but its device secret is not",
        )),
        Lookup::Absent => backend::create(data_dir),
    }
}

/// Whether `data_dir` already holds a v2 history database.
///
/// The filename comes from `copypaste_ipc` rather than a literal here, because
/// a second definition of it is a second thing to keep in step.
fn history_present(data_dir: &Path) -> bool {
    copypaste_ipc::database_path()
        .file_name()
        .is_some_and(|name| data_dir.join(name).exists())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keystore_identifiers_are_frozen() {
        // Port manifest 02, I-10.
        assert_eq!(KEYSTORE_SERVICE, "com.copypaste.daemon");
        assert_eq!(KEYSTORE_ACCOUNT, "device-secret-key");
        assert_eq!(SECRET_FILE_NAME, "device_secret.key");
    }

    /// Everything below drives a real backend, so it runs only where that
    /// backend is a file in a temp directory. On macOS these would write to the
    /// developer's login Keychain; on Android they cannot run at all.
    #[cfg(not(any(target_os = "macos", target_os = "android")))]
    mod mint_authorisation {
        use super::*;

        fn database_named_like_the_real_one(dir: &Path) -> std::path::PathBuf {
            let name = copypaste_ipc::database_path();
            dir.join(name.file_name().unwrap())
        }

        #[test]
        fn a_first_run_mints_and_a_second_run_reads_back_the_same_secret() {
            let dir = tempfile::tempdir().unwrap();
            let first = load_or_create_secret(dir.path()).unwrap();
            let second = load_or_create_secret(dir.path()).unwrap();
            assert_eq!(*first, *second);
        }

        #[test]
        fn two_data_directories_hold_two_different_secrets() {
            let a = tempfile::tempdir().unwrap();
            let b = tempfile::tempdir().unwrap();
            assert_ne!(
                *load_or_create_secret(a.path()).unwrap(),
                *load_or_create_secret(b.path()).unwrap()
            );
        }

        /// F-11: the case that used to re-key a live history.
        #[test]
        fn a_database_without_its_secret_is_refused_rather_than_re_keyed() {
            let dir = tempfile::tempdir().unwrap();
            std::fs::write(database_named_like_the_real_one(dir.path()), b"SQLCipher").unwrap();

            let err = load_or_create_secret(dir.path())
                .expect_err("minting over an existing history must be refused");
            assert!(
                matches!(err, CryptoError::KeystoreUnavailable(_)),
                "{err:?}"
            );

            assert!(
                !dir.path().join(SECRET_FILE_NAME).exists(),
                "a secret was minted anyway"
            );
        }

        /// The guard must not fire on the ordinary case: a history that *does*
        /// have its secret keeps opening.
        #[test]
        fn a_database_with_its_secret_still_opens() {
            let dir = tempfile::tempdir().unwrap();
            let minted = load_or_create_secret(dir.path()).unwrap();
            std::fs::write(database_named_like_the_real_one(dir.path()), b"SQLCipher").unwrap();

            assert_eq!(*load_or_create_secret(dir.path()).unwrap(), *minted);
        }
    }
}
