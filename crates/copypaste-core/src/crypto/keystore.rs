//! Where the device secret lives, per platform.
//!
//! Exactly one entry point, [`load_or_create_secret`], selected at compile time.
//! The rule both backends obey is port manifest 02, I-20: **only an unambiguous
//! "no entry exists" authorises minting a fresh secret.** Any other failure is
//! [`CryptoError::KeystoreUnavailable`], because a secret that cannot be read is
//! not the same as a secret that is not there, and minting over an existing one
//! orphans the SQLCipher database.

use zeroize::Zeroizing;

use super::{CryptoError, KEY_LEN};

/// macOS Keychain service name. Frozen (port manifest 02, I-10) — renaming it
/// makes an existing install's database unopenable. Defined unconditionally
/// even though only the macOS backend reads it: moving a frozen identifier
/// behind a `cfg` is how it drifts.
#[allow(dead_code)]
const KEYCHAIN_SERVICE: &str = "com.copypaste.daemon";

/// macOS Keychain account name for the device secret. Frozen (I-10).
#[allow(dead_code)]
const KEYCHAIN_ACCOUNT: &str = "device-secret-key";

/// Filename of the development-only file-backed secret store.
///
/// Dead under the keychain backend, as the two above are dead under this one —
/// only one `mod backend` compiles.
#[allow(dead_code)]
const SECRET_FILE_NAME: &str = "device_secret.key";

/// What a backend returns: 32 bytes, zeroized on drop (I-12).
pub(super) type DeviceSecret = Zeroizing<[u8; KEY_LEN]>;

pub(super) use backend::load_or_create_secret;

/// macOS Keychain backend, gated on the `macos-keychain` cargo feature: this
/// crate's `Cargo.toml` still needs `security-framework` as an optional
/// dependency and the feature that enables it. Until then macOS falls through
/// to the file backend, which is a development posture, not a shipping one.
#[cfg(all(target_os = "macos", feature = "macos-keychain"))]
mod backend {
    use super::super::keys::random_secret;
    use super::{
        CryptoError, DeviceSecret, Zeroizing, KEYCHAIN_ACCOUNT, KEYCHAIN_SERVICE, KEY_LEN,
    };

    /// `errSecItemNotFound`. The *only* status that authorises minting a fresh
    /// device secret (port manifest 02, I-20). v1 matched `Err(_)` here, minted
    /// an ephemeral key against an existing database, and produced
    /// `SQLITE_NOTADB` plus a dead daemon.
    const ERR_SEC_ITEM_NOT_FOUND: i32 = -25300;

    pub(in crate::crypto) fn load_or_create_secret() -> Result<DeviceSecret, CryptoError> {
        match security_framework::passwords::get_generic_password(
            KEYCHAIN_SERVICE,
            KEYCHAIN_ACCOUNT,
        ) {
            Ok(bytes) => {
                // Zeroize the Keychain-returned buffer even on the error path
                // (I-12, audit MED #4).
                let bytes = Zeroizing::new(bytes);
                let secret: [u8; KEY_LEN] = bytes.as_slice().try_into().map_err(|_| {
                    CryptoError::KeystoreUnavailable("stored secret has wrong length")
                })?;
                Ok(Zeroizing::new(secret))
            }
            Err(e) if e.code() == ERR_SEC_ITEM_NOT_FOUND => {
                let secret = Zeroizing::new(random_secret());
                security_framework::passwords::set_generic_password(
                    KEYCHAIN_SERVICE,
                    KEYCHAIN_ACCOUNT,
                    secret.as_ref(),
                )
                .map_err(|_| CryptoError::KeystoreUnavailable("could not write to the keychain"))?;
                Ok(secret)
            }
            // Locked (-25308), auth failed (-25293), missing entitlement
            // (-34018), timeout, I/O: the entry's state is UNKNOWN. Report it
            // and leave the encrypted database alone.
            Err(_) => Err(CryptoError::KeystoreUnavailable(
                "the keychain is locked or access was denied",
            )),
        }
    }
}

/// Development-only file backend: a `0600` file under the platform data dir.
///
/// See [`crate::crypto::Keyring::load_or_create`] for why this is not a
/// keystore.
#[cfg(not(all(target_os = "macos", feature = "macos-keychain")))]
mod backend {
    use std::fs;
    use std::io::{ErrorKind, Read, Write};
    use std::path::PathBuf;

    use super::super::keys::random_secret;
    use super::{CryptoError, DeviceSecret, Zeroizing, KEY_LEN, SECRET_FILE_NAME};

    /// Resolve the secret's path. Never surfaced in an error (`CLAUDE.md`
    /// rule 4) — the data dir contains the local username.
    fn secret_path() -> Result<PathBuf, CryptoError> {
        let dirs = directories::ProjectDirs::from("com", "copypaste", "copypaste").ok_or(
            CryptoError::KeystoreUnavailable("no platform data directory is available"),
        )?;
        Ok(dirs.data_dir().join(SECRET_FILE_NAME))
    }

    pub(in crate::crypto) fn load_or_create_secret() -> Result<DeviceSecret, CryptoError> {
        let path = secret_path()?;

        // `create_new` fails if the file exists, so two daemons racing on
        // first run cannot clobber each other's secret — the loser reads the
        // winner's. No temp-file-and-rename: rename replaces, and replacing
        // this file is exactly what orphans a database.
        match create_secret_file(&path) {
            Ok(secret) => return Ok(secret),
            Err(CreateOutcome::Exists) => {}
            Err(CreateOutcome::Failed(e)) => return Err(e),
        }

        read_secret_file(&path)
    }

    enum CreateOutcome {
        Exists,
        Failed(CryptoError),
    }

    fn create_secret_file(path: &std::path::Path) -> Result<DeviceSecret, CreateOutcome> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|_| {
                CreateOutcome::Failed(CryptoError::KeystoreUnavailable(
                    "could not create the data directory",
                ))
            })?;
        }

        let mut opts = fs::OpenOptions::new();
        opts.write(true).create_new(true);
        // 0600 is set *at create time*, not chmod'd afterwards, so the secret
        // is never momentarily group- or world-readable.
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }

        let mut file = match opts.open(path) {
            Ok(f) => f,
            Err(e) if e.kind() == ErrorKind::AlreadyExists => return Err(CreateOutcome::Exists),
            Err(_) => {
                return Err(CreateOutcome::Failed(CryptoError::KeystoreUnavailable(
                    "could not create the device secret file",
                )))
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
            // A partial file would later read back as the wrong length and
            // fail closed rather than silently minting a new key, but leaving
            // it behind would wedge every subsequent start. Remove it.
            let _ = fs::remove_file(path);
            return Err(CreateOutcome::Failed(CryptoError::KeystoreUnavailable(
                "could not write the device secret",
            )));
        }

        tracing::warn!(
            "created a file-backed device secret. This is a development \
             fallback, not an OS keystore."
        );
        Ok(secret)
    }

    fn read_secret_file(path: &std::path::Path) -> Result<DeviceSecret, CryptoError> {
        let mut file = fs::File::open(path)
            .map_err(|_| CryptoError::KeystoreUnavailable("could not open the device secret"))?;

        // Read one byte more than expected so a too-long file is rejected
        // rather than silently truncated to its first 32 bytes.
        let mut buf = Zeroizing::new(Vec::with_capacity(KEY_LEN + 1));
        file.read_to_end(&mut buf)
            .map_err(|_| CryptoError::KeystoreUnavailable("could not read the device secret"))?;

        let secret: [u8; KEY_LEN] = buf
            .as_slice()
            .try_into()
            .map_err(|_| CryptoError::KeystoreUnavailable("stored secret has wrong length"))?;

        // Re-assert 0600 if something loosened it. Refusing to start is
        // stricter but locks the user out of their own history over a
        // permission bit; tightening and continuing risks no data loss.
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keychain_identifiers_are_frozen() {
        // Port manifest 02, I-10.
        assert_eq!(KEYCHAIN_SERVICE, "com.copypaste.daemon");
        assert_eq!(KEYCHAIN_ACCOUNT, "device-secret-key");
    }
}
