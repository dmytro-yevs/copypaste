//! Development-only file backend: a `0600` file in the data directory.
//!
//! **This is not a keystore.** The secret sits in plaintext next to the
//! database it opens, protected by file permissions alone and readable by any
//! process running as that user, and by anything that backs the directory up.
//! It exists so the daemon can be built and tested on a Linux workstation.
//!
//! Android does not reach it. `cfg(target_os = "android")` selects the Android
//! Keystore backend, so on that target this file is not compiled at all —
//! rather than being a fallback that a misconfiguration could land on.

use std::fs;
use std::io::{ErrorKind, Read, Write};
use std::path::{Path, PathBuf};

use zeroize::Zeroizing;

use super::super::keys::random_secret;
use super::{CryptoError, DeviceSecret, Lookup, KEY_LEN, SECRET_FILE_NAME};

/// The secret sits in the data directory it belongs to, not in the platform
/// default: security review F-11. Resolving through `directories::ProjectDirs`
/// meant `--data-dir` moved the database and left the secret behind.
///
/// Never surfaced in an error (`CLAUDE.md` rule 4) — the path contains the
/// local username.
fn secret_path(data_dir: &Path) -> PathBuf {
    data_dir.join(SECRET_FILE_NAME)
}

pub(super) fn load(data_dir: &Path) -> Result<Lookup, CryptoError> {
    let path = secret_path(data_dir);
    match fs::File::open(&path) {
        Ok(file) => read_secret(&path, file).map(Lookup::Found),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(Lookup::Absent),
        // A permission failure, a directory in the way, an I/O error: the
        // file's state is unknown, which is not the same as absent (I-20).
        Err(_) => Err(CryptoError::KeystoreUnavailable(
            "the device secret could not be opened",
        )),
    }
}

pub(super) fn create(data_dir: &Path) -> Result<DeviceSecret, CryptoError> {
    let path = secret_path(data_dir);
    fs::create_dir_all(data_dir)
        .map_err(|_| CryptoError::KeystoreUnavailable("could not create the data directory"))?;

    let mut opts = fs::OpenOptions::new();
    opts.write(true).create_new(true);
    // 0600 is set *at create time*, not chmod'd afterwards, so the secret is
    // never momentarily group- or world-readable.
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }

    let mut file = match opts.open(&path) {
        Ok(f) => f,
        // `create_new` fails if the file exists, so two daemons racing on first
        // run cannot clobber each other's secret — the loser reads the
        // winner's. No temp-file-and-rename: rename replaces, and replacing
        // this file is exactly what orphans a database.
        Err(e) if e.kind() == ErrorKind::AlreadyExists => {
            let file = fs::File::open(&path).map_err(|_| {
                CryptoError::KeystoreUnavailable("the device secret could not be opened")
            })?;
            return read_secret(&path, file);
        }
        Err(_) => {
            return Err(CryptoError::KeystoreUnavailable(
                "could not create the device secret file",
            ))
        }
    };

    #[cfg(not(unix))]
    tracing::warn!(
        "this platform has no file-permission enforcement here; the device \
         secret is stored without restricted permissions"
    );

    let secret = Zeroizing::new(random_secret());
    let write = file
        .write_all(secret.as_ref())
        .and_then(|()| file.sync_all());
    if write.is_err() {
        // A partial file would later read back as the wrong length and fail
        // closed rather than silently minting a new key, but leaving it behind
        // would wedge every subsequent start. Remove it.
        let _ = fs::remove_file(&path);
        return Err(CryptoError::KeystoreUnavailable(
            "could not write the device secret",
        ));
    }

    tracing::warn!(
        "created a file-backed device secret. This is a development fallback, \
         not an OS keystore."
    );
    Ok(secret)
}

fn read_secret(path: &Path, mut file: fs::File) -> Result<DeviceSecret, CryptoError> {
    // Read one byte more than expected so a too-long file is rejected rather
    // than silently truncated to its first 32 bytes.
    let mut buf = Zeroizing::new(Vec::with_capacity(KEY_LEN + 1));
    file.read_to_end(&mut buf)
        .map_err(|_| CryptoError::KeystoreUnavailable("the device secret could not be read"))?;

    let secret: [u8; KEY_LEN] = buf.as_slice().try_into().map_err(|_| {
        CryptoError::KeystoreEntryUnusable("the stored device secret is the wrong length")
    })?;

    // Re-assert 0600 if something loosened it. Refusing to start is stricter
    // but locks the user out of their own history over a permission bit;
    // tightening and continuing risks no data loss.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = file.metadata() {
            let mode = meta.permissions().mode() & 0o777;
            if mode != 0o600 {
                tracing::warn!(
                    found_mode = format!("{mode:o}"),
                    "device secret file had permissive modes; tightening to 0600"
                );
                let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
            }
        }
    }

    Ok(Zeroizing::new(secret))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_minted_secret_lands_in_the_given_directory_and_not_the_platform_default() {
        // Security review F-11.
        let dir = tempfile::tempdir().unwrap();
        create(dir.path()).unwrap();
        assert!(dir.path().join(SECRET_FILE_NAME).is_file());
    }

    #[test]
    fn a_missing_directory_is_created_rather_than_refused() {
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("not").join("there").join("yet");
        let secret = create(&nested).unwrap();
        assert!(matches!(load(&nested).unwrap(), Lookup::Found(found) if *found == *secret));
    }

    #[test]
    fn an_absent_file_is_absent_and_not_an_error() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(load(dir.path()).unwrap(), Lookup::Absent));
    }

    /// I-20 seen from the file backend: a secret that is *there* but unusable
    /// must never read back as "there is none".
    #[test]
    fn a_wrong_length_secret_is_unusable_rather_than_absent() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(SECRET_FILE_NAME), [7u8; KEY_LEN - 1]).unwrap();

        // `Lookup` has no `Debug` on purpose — it holds key material — so this
        // matches rather than unwrapping.
        assert!(matches!(
            load(dir.path()),
            Err(CryptoError::KeystoreEntryUnusable(_))
        ));
    }

    #[test]
    fn a_too_long_secret_is_rejected_rather_than_truncated() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(SECRET_FILE_NAME), [7u8; KEY_LEN + 1]).unwrap();

        assert!(matches!(
            load(dir.path()),
            Err(CryptoError::KeystoreEntryUnusable(_))
        ));
    }

    /// The second caller reads the first one's secret instead of replacing it.
    #[test]
    fn creating_twice_returns_the_existing_secret() {
        let dir = tempfile::tempdir().unwrap();
        let first = create(dir.path()).unwrap();
        let second = create(dir.path()).unwrap();
        assert_eq!(*first, *second);
    }

    #[cfg(unix)]
    #[test]
    fn a_minted_secret_is_0600() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        create(dir.path()).unwrap();

        let mode = fs::metadata(dir.path().join(SECRET_FILE_NAME))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    #[cfg(unix)]
    #[test]
    fn a_loosened_secret_file_is_tightened_on_read() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(SECRET_FILE_NAME);
        create(dir.path()).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();

        load(dir.path()).unwrap();
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}
