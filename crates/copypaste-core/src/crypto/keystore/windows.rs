//! Windows device-secret backend: DPAPI, sealed blob in the data directory.
//!
//! **DPAPI rather than the Credential Manager**, on the axis those two differ
//! on: what a process already running as the user can read. Neither asks
//! anyone's permission, so both are readable in-session by design. But
//! `CredEnumerate` hands every generic credential *and its blob* to any caller
//! in the session, and `credwiz` exports the same set to a file — so a
//! credential dumper takes the whole history without ever having heard of this
//! program. A DPAPI blob has no directory to walk.

//! Not a boundary, and not to be described as one: an attacker running as the
//! user can read the blob and unseal it once they know to look, and the entropy
//! is public in this repository. The difference bought is between a sweep and a
//! targeted tool.
#![allow(unsafe_code)]

use std::fs;
use std::io::{ErrorKind, Read, Write};
use std::path::{Path, PathBuf};
use std::ptr;

use windows_sys::Win32::Foundation::{GetLastError, LocalFree};
use windows_sys::Win32::Security::Cryptography::{
    CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
};
use zeroize::{Zeroize, Zeroizing};

use super::super::keys::random_secret;
use super::{CryptoError, DeviceSecret, Lookup, DPAPI_ENTROPY, KEY_LEN, SECRET_BLOB_NAME};

/// `ERROR_INVALID_DATA`, DPAPI's answer when this blob will not decrypt under
/// this user: tampered, sealed under different entropy, or carried in from
/// another account. The one failure that is permanent, so the one that becomes
/// [`CryptoError::KeystoreEntryUnusable`].
const ERROR_INVALID_DATA: u32 = 13;

/// What is read off disk before DPAPI is asked about it. A sealed 32-byte
/// secret is a few hundred bytes; past this it is not our blob, and pulling it
/// into memory to discover that is the part worth refusing.
const MAX_SEALED_LEN: usize = 4096;

/// The blob sits in the data directory it opens, not in a user-scoped store:
/// security review F-11, and what makes `--data-dir` a separate install rather
/// than two databases sharing one secret. Never surfaced in an error
/// (`AGENTS.md` rule 4) — the path contains the local username.
fn blob_path(data_dir: &Path) -> PathBuf {
    data_dir.join(SECRET_BLOB_NAME)
}

pub(super) fn load(data_dir: &Path) -> Result<Lookup, CryptoError> {
    let path = blob_path(data_dir);
    match fs::File::open(&path) {
        Ok(file) => unseal(&read_sealed(file)?).map(Lookup::Found),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(Lookup::Absent),
        // A sharing violation, a directory in the way, an I/O error: the blob's
        // state is unknown, which is not the same as absent (I-20).
        Err(_) => Err(CryptoError::KeystoreUnavailable(
            "the device secret could not be opened",
        )),
    }
}

pub(super) fn create(data_dir: &Path) -> Result<DeviceSecret, CryptoError> {
    fs::create_dir_all(data_dir)
        .map_err(|_| CryptoError::KeystoreUnavailable("could not create the data directory"))?;

    let secret = Zeroizing::new(random_secret());
    let sealed = seal(&secret)?;
    let path = blob_path(data_dir);

    let mut file = match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
    {
        Ok(file) => file,
        // `create_new` fails if the blob is there, so two daemons racing on
        // first run cannot clobber each other — the loser reads the winner's.
        // No temp-file-and-rename: rename replaces, and replacing this blob is
        // exactly what orphans a database.
        Err(e) if e.kind() == ErrorKind::AlreadyExists => {
            let file = fs::File::open(&path).map_err(|_| {
                CryptoError::KeystoreUnavailable("the device secret could not be opened")
            })?;
            return unseal(&read_sealed(file)?);
        }
        Err(_) => {
            return Err(CryptoError::KeystoreUnavailable(
                "could not create the device secret file",
            ));
        }
    };

    let written = file.write_all(&sealed).and_then(|()| file.sync_all());
    if written.is_err() {
        // Dropped first: Windows will not unlink a file this process still
        // holds open. Leaving a half-written blob behind would wedge every
        // later start, and it would read as unusable rather than as absent.
        drop(file);
        let _ = fs::remove_file(&path);
        return Err(CryptoError::KeystoreUnavailable(
            "could not write the device secret",
        ));
    }
    Ok(secret)
}

fn read_sealed(file: fs::File) -> Result<Vec<u8>, CryptoError> {
    // One byte past the bound, so an oversized file is rejected rather than
    // truncated into something DPAPI would refuse for a less clear reason.
    let mut sealed = Vec::with_capacity(MAX_SEALED_LEN + 1);
    file.take(MAX_SEALED_LEN as u64 + 1)
        .read_to_end(&mut sealed)
        .map_err(|_| CryptoError::KeystoreUnavailable("the device secret could not be read"))?;
    if sealed.len() > MAX_SEALED_LEN {
        return Err(CryptoError::KeystoreEntryUnusable(
            "the stored device secret is not a sealed blob",
        ));
    }
    Ok(sealed)
}

fn unseal(sealed: &[u8]) -> Result<DeviceSecret, CryptoError> {
    let plain = unprotect(sealed)?;
    let secret: [u8; KEY_LEN] = plain.as_slice().try_into().map_err(|_| {
        CryptoError::KeystoreEntryUnusable("the stored device secret is the wrong length")
    })?;
    Ok(Zeroizing::new(secret))
}

/// Borrow `bytes` in the shape DPAPI takes. `pbData` is `*mut` in the binding
/// and read-only in the API for every input blob; nothing here writes through
/// it.
fn input(bytes: &[u8]) -> CRYPT_INTEGER_BLOB {
    CRYPT_INTEGER_BLOB {
        cbData: u32::try_from(bytes.len()).unwrap_or(u32::MAX),
        pbData: bytes.as_ptr().cast_mut(),
    }
}

fn seal(secret: &[u8; KEY_LEN]) -> Result<Vec<u8>, CryptoError> {
    let plaintext = input(secret);
    let entropy = input(DPAPI_ENTROPY);
    let mut out = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: ptr::null_mut(),
    };

    // SAFETY: both input blobs borrow live slices for the whole call, `out` is
    // a writable blob, and every optional pointer is null.
    //
    // `CRYPTPROTECT_UI_FORBIDDEN` stops a daemon with no window being handed a
    // dialog nobody can answer. `CRYPTPROTECT_LOCAL_MACHINE` is deliberately
    // absent: it would make the blob readable by every account on the machine.
    let ok = unsafe {
        CryptProtectData(
            &plaintext,
            ptr::null(),
            &entropy,
            ptr::null(),
            ptr::null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut out,
        )
    };
    if ok == 0 || out.pbData.is_null() {
        return Err(CryptoError::KeystoreUnavailable(
            "the device secret could not be sealed",
        ));
    }

    // SAFETY: DPAPI wrote `cbData` bytes at `pbData` and owns them until
    // `LocalFree`; the borrow ends before that call.
    let sealed = unsafe { std::slice::from_raw_parts(out.pbData, out.cbData as usize) }.to_vec();
    unsafe { LocalFree(out.pbData.cast()) };
    Ok(sealed)
}

fn unprotect(sealed: &[u8]) -> Result<Zeroizing<Vec<u8>>, CryptoError> {
    let ciphertext = input(sealed);
    let entropy = input(DPAPI_ENTROPY);
    let mut out = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: ptr::null_mut(),
    };

    // SAFETY: as in `seal`. The description out-parameter is null, so DPAPI
    // allocates no string for this call to free.
    let ok = unsafe {
        CryptUnprotectData(
            &ciphertext,
            ptr::null_mut(),
            &entropy,
            ptr::null(),
            ptr::null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut out,
        )
    };
    if ok == 0 {
        // SAFETY: read on this thread immediately after the failed call.
        return Err(classify(unsafe { GetLastError() }));
    }
    if out.pbData.is_null() {
        return Err(CryptoError::KeystoreEntryUnusable(
            "the stored device secret unsealed to nothing",
        ));
    }

    // SAFETY: as in `seal`, and mutable because the buffer is wiped below.
    let plain = unsafe { std::slice::from_raw_parts_mut(out.pbData, out.cbData as usize) };
    let secret = Zeroizing::new(plain.to_vec());
    // DPAPI's own buffer is the device secret in the clear and nothing drops
    // it for us (I-12); freeing it unwiped leaves 32 bytes of key material in
    // the process heap for every start of the daemon.
    plain.zeroize();
    unsafe { LocalFree(out.pbData.cast()) };
    Ok(secret)
}

/// I-20's classification, in DPAPI's vocabulary.
fn classify(code: u32) -> CryptoError {
    if code == ERROR_INVALID_DATA {
        CryptoError::KeystoreEntryUnusable("the stored device secret does not decrypt here")
    } else {
        // A master key that is not loaded — a service account with no profile,
        // a password reset performed without the old one — may well unseal on
        // the next start. Calling that permanent tells a user their history is
        // gone and invites them to delete it, which is rule 4's worst outcome
        // reached by a message rather than by code.
        CryptoError::KeystoreUnavailable("the device secret could not be unsealed")
    }
}

// Tests — against real DPAPI. No arming flag and no cleanup, unlike the macOS
// Keychain set: DPAPI writes nothing outside the directory it is handed.
//
// Every assertion names *which backend answered*. A test satisfied by bytes
// coming back is equally satisfied by the plaintext file this replaces, which
// is the failure that shipped on macOS for a year.

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::process::Command;

    use sha2::{Digest, Sha256};

    use super::super::{load_or_create_secret, SECRET_FILE_NAME};
    use super::*;

    /// Set by the parent half of [`a_sealed_secret_survives_a_process_restart`]
    /// so the child knows it is the child.
    const CHILD_DIR: &str = "COPYPASTE_DPAPI_CHILD_DIR";
    const CHILD_DIGEST: &str = "COPYPASTE_DPAPI_CHILD_DIGEST";

    fn blob(dir: &Path) -> Vec<u8> {
        fs::read(dir.join(SECRET_BLOB_NAME)).expect("the sealed blob must be on disk")
    }

    fn digest(secret: &[u8]) -> String {
        hex::encode(Sha256::digest(secret))
    }

    fn like_the_real_database(dir: &Path) -> PathBuf {
        let name = copypaste_ipc::database_path();
        dir.join(name.file_name().expect("the database path has a filename"))
    }

    /// The whole point: a first run seals the secret, a second run reads that
    /// same secret back, and what is on disk is not the secret.
    #[test]
    fn the_blob_holds_the_secret_sealed_and_no_plaintext_file_is_written() {
        let dir = tempfile::tempdir().unwrap();

        let minted = load_or_create_secret(dir.path()).expect("a first run must mint a secret");

        // Which backend answered. `file.rs` would have left the 32 bytes
        // themselves in `device_secret.key`.
        assert!(
            !dir.path().join(SECRET_FILE_NAME).exists(),
            "the file backend answered: {SECRET_FILE_NAME} was written"
        );
        let sealed = blob(dir.path());
        assert!(
            !sealed
                .windows(KEY_LEN)
                .any(|window| window == minted.as_slice()),
            "the secret is in the blob in the clear"
        );
        assert!(sealed.len() > KEY_LEN, "the blob is not a DPAPI envelope");

        let reopened = load_or_create_secret(dir.path()).expect("a second run must read it back");
        assert_eq!(*reopened, *minted, "a second run minted a different secret");
    }

    /// The Android nightly's property — the secret outlives the process that
    /// minted it — proved the only way it can be, in a second process. An
    /// in-process round trip would pass against a backend that cached in a
    /// `static` and never wrote anything.
    #[test]
    fn a_sealed_secret_survives_a_process_restart() {
        if let Some(dir) = std::env::var_os(CHILD_DIR) {
            let dir = PathBuf::from(dir);
            let read_back = load_or_create_secret(&dir).expect("the child must read the secret");
            // A digest crosses the process boundary, not the secret: the
            // environment of a test child is not a place to put key material.
            assert_eq!(
                digest(read_back.as_slice()),
                std::env::var(CHILD_DIGEST).expect("the parent sets the expected digest"),
                "the child read a different secret than the parent minted"
            );
            fs::write(dir.join("child-ran"), b"1").unwrap();
            return;
        }

        let dir = tempfile::tempdir().unwrap();
        let minted = load_or_create_secret(dir.path()).unwrap();

        let name = child_test_name();
        let status = Command::new(std::env::current_exe().expect("current test binary"))
            .args(["--exact", name.as_str(), "--nocapture"])
            .env(CHILD_DIR, dir.path())
            .env(CHILD_DIGEST, digest(minted.as_slice()))
            .status()
            .expect("run the child test process");

        assert!(status.success(), "the child process failed");
        assert!(
            dir.path().join("child-ran").exists(),
            "the child exited 0 without running the test — the filter matched nothing"
        );
    }

    /// Derived rather than written out: this module is mounted as
    /// `keystore::backend`, so the literal name would be wrong in a way that
    /// still passes — the child would filter to zero tests and exit 0.
    fn child_test_name() -> String {
        let (_crate_name, path) = module_path!()
            .split_once("::")
            .expect("a module path starts with the crate name");
        format!("{path}::a_sealed_secret_survives_a_process_restart")
    }

    /// I-20. Nothing on disk is the only thing that authorises a mint, and it
    /// must not read as a failure.
    #[test]
    fn an_absent_blob_is_absent_and_not_an_error() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(load(dir.path()).unwrap(), Lookup::Absent));
    }

    /// Rule 4, fail closed. A flipped bit in the envelope must authenticate
    /// as a failure, never as a shorter or different secret.
    #[test]
    fn a_tampered_blob_fails_closed_and_is_never_replaced() {
        let dir = tempfile::tempdir().unwrap();
        load_or_create_secret(dir.path()).unwrap();
        let mut sealed = blob(dir.path());
        let last = sealed.len() - 1;
        sealed[last] ^= 0x01;
        fs::write(dir.path().join(SECRET_BLOB_NAME), &sealed).unwrap();

        // Destructured rather than `expect_err`: the `Ok` this must not take
        // holds the device secret, and `expect_err` would print it.
        let Err(err) = load(dir.path()) else {
            panic!("a tampered blob must not unseal");
        };
        assert!(
            matches!(err, CryptoError::KeystoreEntryUnusable(_)),
            "expected KeystoreEntryUnusable, got {err:?}"
        );
        assert!(
            load_or_create_secret(dir.path()).is_err(),
            "an unusable blob must not be minted over"
        );
        assert_eq!(blob(dir.path()), sealed, "the stored blob was overwritten");
    }

    /// The entropy is what a credential sweep would have to know. A blob
    /// sealed without it is somebody else's blob, and must fail closed rather
    /// than be adopted.
    #[test]
    fn a_blob_sealed_under_other_entropy_does_not_unseal() {
        let dir = tempfile::tempdir().unwrap();
        let foreign = seal_with(&[3u8; KEY_LEN], b"not-copypaste");
        fs::write(dir.path().join(SECRET_BLOB_NAME), &foreign).unwrap();

        let Err(err) = load(dir.path()) else {
            panic!("a blob sealed under other entropy must not unseal");
        };
        assert!(
            matches!(err, CryptoError::KeystoreEntryUnusable(_)),
            "expected KeystoreEntryUnusable, got {err:?}"
        );
    }

    /// I-20 from the other side: a blob that unseals to the wrong number of
    /// bytes is *there* and unusable, never absent.
    #[test]
    fn a_wrong_length_secret_is_unusable_rather_than_absent() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join(SECRET_BLOB_NAME),
            seal_with(b"short", DPAPI_ENTROPY),
        )
        .unwrap();

        assert!(matches!(
            load(dir.path()),
            Err(CryptoError::KeystoreEntryUnusable(_))
        ));
    }

    #[test]
    fn an_oversized_file_is_refused_before_dpapi_sees_it() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join(SECRET_BLOB_NAME),
            vec![0u8; MAX_SEALED_LEN + 1],
        )
        .unwrap();

        assert!(matches!(
            load(dir.path()),
            Err(CryptoError::KeystoreEntryUnusable(_))
        ));
    }

    /// Security review F-11, on the backend that ships on this platform. A
    /// history beside no blob means the wrong directory, not a first run.
    #[test]
    fn a_history_without_its_secret_is_refused_rather_than_re_keyed() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(like_the_real_database(dir.path()), b"SQLCipher").unwrap();

        let Err(err) = load_or_create_secret(dir.path()) else {
            panic!("minting beside an existing history must be refused");
        };
        assert!(
            matches!(err, CryptoError::KeystoreUnavailable(_)),
            "expected KeystoreUnavailable, got {err:?}"
        );
        assert!(
            !dir.path().join(SECRET_BLOB_NAME).exists(),
            "a secret was sealed anyway"
        );
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

    #[test]
    fn creating_twice_returns_the_existing_secret() {
        let dir = tempfile::tempdir().unwrap();
        let first = create(dir.path()).unwrap();
        let second = create(dir.path()).unwrap();
        assert_eq!(*first, *second);
    }

    /// Two seals of the same bytes differ, so a blob does not double as a
    /// stable identifier of the secret it holds.
    #[test]
    fn sealing_is_not_deterministic() {
        let mut seen = HashSet::new();
        for _ in 0..3 {
            assert!(
                seen.insert(seal_with(&[5u8; KEY_LEN], DPAPI_ENTROPY)),
                "DPAPI produced the same envelope twice"
            );
        }
    }

    /// `seal` with the entropy as a parameter, so a test can produce a blob
    /// this backend must refuse.
    fn seal_with(plaintext: &[u8], entropy: &[u8]) -> Vec<u8> {
        let plaintext = input(plaintext);
        let entropy = input(entropy);
        let mut out = CRYPT_INTEGER_BLOB {
            cbData: 0,
            pbData: ptr::null_mut(),
        };
        // SAFETY: as in `seal`.
        let ok = unsafe {
            CryptProtectData(
                &plaintext,
                ptr::null(),
                &entropy,
                ptr::null(),
                ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut out,
            )
        };
        assert!(ok != 0 && !out.pbData.is_null(), "CryptProtectData failed");
        // SAFETY: as in `seal`.
        let sealed =
            unsafe { std::slice::from_raw_parts(out.pbData, out.cbData as usize) }.to_vec();
        unsafe { LocalFree(out.pbData.cast()) };
        sealed
    }
}
