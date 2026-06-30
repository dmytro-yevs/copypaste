//! File-backed device-key store — the non-prompting key path for ad-hoc /
//! unsigned macOS installs.
//!
//! ## Why this exists
//!
//! The macOS login Keychain pins a generic-password item's ACL to the
//! *designated requirement* of every trusted application in its trust list.
//! For an **ad-hoc-signed** binary that designated requirement is the binary's
//! `cdhash` (`codesign -d --requirements -` prints `cdhash H"…"`). The cdhash
//! changes on EVERY rebuild/reinstall, so after each app update the running
//! daemon no longer matches the trusted-app entry recorded when the item was
//! created — and macOS raises the interactive
//! "copypaste-daemon wants to use your confidential information" password
//! prompt. "Always Allow" only re-pins to the *current* cdhash, so the prompt
//! returns on the next update. There is no public Security-framework API that
//! creates a generic-password item with an ACL that trusts "any process this
//! user runs" without prompting under ad-hoc signing.
//!
//! CopyPaste ships ad-hoc-signed (no Developer ID certificate is guaranteed),
//! so the Keychain path is structurally unable to be prompt-free. This module
//! stores the 32-byte X25519 device secret in a `0600` file under the app
//! data dir instead. The file is created atomically (write to a temp file in
//! the same directory, `fchmod` to `0600` before any secret bytes are written,
//! then `rename`), owned by the user, and survives reinstalls — so an ad-hoc
//! rebuild reads it back with **no prompt, ever**.
//!
//! ## Threat-model tradeoff (documented, accepted)
//!
//! A `0600` file is readable by any process running as the same user — exactly
//! like a Keychain item whose ACL the user clicked "Always Allow" on, or like
//! the existing `device_id` and SQLCipher WAL files already sitting in this
//! directory. For an **unsigned / ad-hoc** app this is the standard, accepted
//! tradeoff: there is no OS-enforced code-identity to gate on, so per-app
//! Keychain isolation buys nothing it can actually enforce across updates.
//! A genuinely **Developer-ID-signed** build has a STABLE designated
//! requirement (a real `TeamIdentifier`), so its Keychain ACL survives updates
//! and is strictly better — that build prefers the Keychain
//! (see [`super::signing`]). The file store is only used when no stable code
//! identity exists to protect.

use std::io::Write;
use std::path::{Path, PathBuf};

use copypaste_core::DeviceKeypair;

use super::KeychainError;

/// Filename of the device-secret file inside the app data dir.
const KEY_FILE_NAME: &str = "device_secret.key";

/// Filename of the cloud-sync passphrase-derived key inside the app data dir.
const CLOUD_SYNC_FILE_NAME: &str = "cloud_sync.key";

/// Filename of the **v2 per-account-salt** cloud-sync key (CopyPaste-jdq5).
///
/// Sibling of [`CLOUD_SYNC_FILE_NAME`]; holds the 32-byte v2 key derived via
/// `derive_sync_key_for_account(passphrase, account_id)`. Persisted separately so
/// a restart can restore BOTH the v1 (relay + read-fallback) and v2 (cloud-write
/// + preferred-read) keys for dual-key read dispatch without the passphrase.
#[cfg_attr(
    not(feature = "cloud-sync"),
    allow(
        dead_code,
        reason = "only used by the cloud-sync v2 persist/restore path"
    )
)]
const CLOUD_SYNC_V2_FILE_NAME: &str = "cloud_sync_v2.key";

/// Resolve the app data dir for key files, honouring `COPYPASTE_KEY_FILE_PATH`
/// for tests. When the override is set it points at a *file*, and its parent
/// directory is used as the data dir so sibling key files (cloud-sync) land
/// next to it in the same temp dir.
fn data_dir_for_keys() -> Result<PathBuf, KeychainError> {
    if let Some(p) = std::env::var_os("COPYPASTE_KEY_FILE_PATH") {
        let pb = PathBuf::from(p);
        return Ok(pb
            .parent()
            .map(|d| d.to_path_buf())
            .unwrap_or_else(|| PathBuf::from(".")));
    }
    crate::paths::try_app_support_dir().map_err(|e| {
        KeychainError::Io(std::io::Error::other(format!(
            "could not resolve app support dir for device key file: {e}"
        )))
    })
}

/// Test/override hook: when `COPYPASTE_KEY_FILE_PATH` is set, the file store
/// reads and writes the device secret at that exact path instead of the
/// default `app_support_dir()/device_secret.key`. Mirrors the
/// `COPYPASTE_DEVICE_ID_PATH` convention used by the device-id store so unit
/// tests can point at a per-test temp file without touching `$HOME`.
fn key_file_path() -> Result<PathBuf, KeychainError> {
    if let Some(p) = std::env::var_os("COPYPASTE_KEY_FILE_PATH") {
        return Ok(PathBuf::from(p));
    }
    Ok(data_dir_for_keys()?.join(KEY_FILE_NAME))
}

/// Path to the cloud-sync key file (sibling of the device-key file).
fn cloud_sync_file_path() -> Result<PathBuf, KeychainError> {
    Ok(data_dir_for_keys()?.join(CLOUD_SYNC_FILE_NAME))
}

/// Path to the v2 per-account-salt cloud-sync key file (CopyPaste-jdq5).
#[cfg_attr(
    not(feature = "cloud-sync"),
    allow(
        dead_code,
        reason = "only used by the cloud-sync v2 persist/restore path"
    )
)]
fn cloud_sync_v2_file_path() -> Result<PathBuf, KeychainError> {
    Ok(data_dir_for_keys()?.join(CLOUD_SYNC_V2_FILE_NAME))
}

/// Persist the 32-byte cloud-sync (passphrase-derived) key to a `0600` file.
///
/// Used instead of the Keychain on ad-hoc / unsigned installs so that setting
/// a sync passphrase does not raise the login-password prompt. Same atomic
/// 0600 write as the device key; same documented threat-model tradeoff.
pub fn store_cloud_sync_key(secret: &[u8; 32]) -> Result<(), KeychainError> {
    let path = cloud_sync_file_path()?;
    write_secret_atomic_to(&path, KEY_FILE_NAME, secret)
}

/// Load the persisted cloud-sync key, or `None` if no passphrase was ever set.
///
/// Returns the raw array (copied out of the internal `Zeroizing` wrapper) so
/// callers that require `[u8; 32]` by value work without change. The `Zeroizing`
/// wrapper is dropped here, wiping the intermediate copy from the stack.
pub fn load_cloud_sync_key() -> Result<Option<[u8; 32]>, KeychainError> {
    Ok(read_secret(&cloud_sync_file_path()?)?.map(|z| *z))
}

/// Persist the 32-byte **v2 per-account-salt** cloud-sync key to a `0600` file
/// (CopyPaste-jdq5). Mirrors [`store_cloud_sync_key`] exactly but writes the
/// sibling `CLOUD_SYNC_V2_FILE_NAME` so the v1 and v2 keys coexist.
#[cfg(feature = "cloud-sync")]
pub fn store_cloud_sync_key_v2(secret: &[u8; 32]) -> Result<(), KeychainError> {
    let path = cloud_sync_v2_file_path()?;
    write_secret_atomic_to(&path, CLOUD_SYNC_V2_FILE_NAME, secret)
}

/// Load the persisted v2 per-account-salt cloud-sync key, or `None` if no v2 key
/// was ever derived (e.g. the passphrase was set before a Supabase account was
/// available). Mirrors [`load_cloud_sync_key`].
#[cfg(feature = "cloud-sync")]
pub fn load_cloud_sync_key_v2() -> Result<Option<[u8; 32]>, KeychainError> {
    Ok(read_secret(&cloud_sync_v2_file_path()?)?.map(|z| *z))
}

/// Delete the persisted v2 cloud-sync key file (used when the v2 key is cleared,
/// e.g. on sign-out). Missing file is not an error. Mirrors [`delete_stored`].
#[cfg(feature = "cloud-sync")]
pub fn delete_cloud_sync_key_v2() -> Result<(), KeychainError> {
    let path = cloud_sync_v2_file_path()?;
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(KeychainError::Io(e)),
    }
}

/// Load the device keypair from the `0600` key file, creating + persisting a
/// fresh one on first run. Never touches the Keychain, so it can never raise a
/// password prompt — this is the path used on ad-hoc / unsigned installs.
///
/// ## Upgrade migration (v0.5.1 → v0.5.2+)
///
/// v0.5.1 and earlier had NO file store: the local SQLCipher device key was
/// stored in the macOS Keychain under `(SERVICE, ACCOUNT)`
/// (`com.copypaste.daemon` / `device-secret-key`). v0.5.2 introduced this file
/// store and routes ad-hoc / unsigned builds here. On the FIRST launch after
/// that upgrade the key file is absent — but the real DB key is still sitting in
/// the Keychain. If we just generated a fresh key we would orphan the existing
/// SQLCipher database (`file is not a database` → DEGRADED). So when the file is
/// absent we first attempt to ADOPT the legacy Keychain key (a plain read — no
/// ACL rotation, no prompt-inducing write) and persist it into the file store.
/// Only when neither the file NOR a legacy Keychain key exists do we generate a
/// brand-new key. An existing key file is NEVER overwritten.
pub fn load_or_create() -> Result<DeviceKeypair, KeychainError> {
    let path = key_file_path()?;
    match read_secret(&path)? {
        Some(secret) => {
            let kp = DeviceKeypair::from_secret_bytes(&secret)?;
            tracing::debug!(path = %path.display(), "loaded device key from file store");
            Ok(kp)
        }
        None => {
            // No file yet. Before minting a fresh key, try to adopt a legacy
            // Keychain-stored key from a pre-file-store build (v0.5.1 and
            // earlier) so we don't orphan an existing encrypted DB.
            if let Some(secret) = read_legacy_keychain_key() {
                let kp = DeviceKeypair::from_secret_bytes(&secret)?;
                write_secret_atomic_to(&path, KEY_FILE_NAME, &secret)?;
                tracing::info!(
                    path = %path.display(),
                    "migrated legacy Keychain device key (com.copypaste.daemon / \
                     device-secret-key) into the 0600 file store — preserves the \
                     existing encrypted database across the v0.5.1→v0.5.2 upgrade"
                );
                return Ok(kp);
            }
            let kp = DeviceKeypair::generate();
            let secret = kp.secret_key_bytes_zeroizing();
            write_secret_atomic_to(&path, KEY_FILE_NAME, &secret)?;
            tracing::info!(
                path = %path.display(),
                "generated new device key in file store (ad-hoc / unsigned install — \
                 non-prompting 0600 file; see keychain::file_store docs)"
            );
            Ok(kp)
        }
    }
}

/// Best-effort read of the legacy local DB key from the macOS Keychain.
///
/// Pre-v0.5.2 builds stored the X25519 device secret in the login Keychain under
/// [`super::SERVICE`] / [`super::ACCOUNT`]. This is a plain `get_generic_password`
/// read — it performs NO ACL rotation and NO write, so on the benign
/// install-moved case (ACL still trusts the binary, or the user grants the
/// prompt once) it returns the legacy key; otherwise it returns `None` and the
/// caller mints a fresh key (the database, if any, then surfaces as a genuine
/// `db_key_mismatch` upstream rather than being clobbered).
///
/// Returns `None` (never an error) on every failure path: the dev/test
/// ephemeral bypass, a missing item, a denied/locked Keychain, or a
/// wrong-length blob. Migration is opportunistic — a failure here must never
/// abort the file-store load.
///
/// The returned array is wrapped in [`zeroize::Zeroizing`] so the 32 key bytes
/// are scrubbed from the stack when the caller drops the value — consistent with
/// every other key-material path in this crate (SEC-2 / CopyPaste-44rq.54).
#[cfg(target_os = "macos")]
fn read_legacy_keychain_key() -> Option<zeroize::Zeroizing<[u8; 32]>> {
    // Honour the central dev/test bypass: never touch the real Keychain (and
    // never prompt) when ephemeral keys are in force.
    if super::keychain_bypassed() {
        return None;
    }
    match security_framework::passwords::get_generic_password(super::SERVICE, super::ACCOUNT) {
        Ok(bytes) => {
            let bytes = zeroize::Zeroizing::new(bytes);
            if bytes.len() != 32 {
                tracing::warn!(
                    len = bytes.len(),
                    "legacy Keychain device key has unexpected length; ignoring for migration"
                );
                return None;
            }
            // Wrap in Zeroizing so the 32 secret bytes are wiped on drop
            // (SEC-2 / CopyPaste-44rq.54: was plain [u8;32], now Zeroizing).
            let mut arr = zeroize::Zeroizing::new([0u8; 32]);
            arr.copy_from_slice(&bytes);
            Some(arr)
        }
        Err(e) => {
            tracing::debug!(
                error = %e,
                "no adoptable legacy Keychain device key (item absent or access denied); \
                 will not migrate"
            );
            None
        }
    }
}

/// Non-macOS: there is no Keychain to migrate from.
#[cfg(not(target_os = "macos"))]
fn read_legacy_keychain_key() -> Option<zeroize::Zeroizing<[u8; 32]>> {
    None
}

/// Delete the persisted key file (factory reset / test cleanup). Missing file
/// is not an error.
pub fn delete_stored() -> Result<(), KeychainError> {
    let path = key_file_path()?;
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(KeychainError::Io(e)),
    }
}

/// Read and validate the 32-byte secret from `path`. Returns `Ok(None)` when
/// the file does not exist (first run). A wrong-length file is a hard error so
/// a corrupt/truncated key is never silently treated as "absent" (which would
/// generate a fresh key and orphan the existing encrypted DB).
///
/// The returned array is wrapped in [`zeroize::Zeroizing`] so the secret bytes
/// are wiped from the stack when the caller drops the value (security MED #6).
fn read_secret(path: &Path) -> Result<Option<zeroize::Zeroizing<[u8; 32]>>, KeychainError> {
    let bytes = match std::fs::read(path) {
        Ok(b) => zeroize::Zeroizing::new(b),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(KeychainError::Io(e)),
    };
    if bytes.len() != 32 {
        return Err(KeychainError::InvalidLength(bytes.len()));
    }
    let mut arr = zeroize::Zeroizing::new([0u8; 32]);
    arr.copy_from_slice(&bytes);
    Ok(Some(arr))
}

/// Atomically persist `secret` to `path` with `0600` permissions.
///
/// Steps (so a reader never observes a partial or world-readable file):
/// 1. ensure the parent directory exists,
/// 2. create a temp file in the SAME directory (so `rename` is atomic — same
///    filesystem),
/// 3. set the temp file mode to `0600` BEFORE writing any secret bytes,
/// 4. write + flush + sync the secret,
/// 5. `rename` over the destination.
fn write_secret_atomic_to(
    path: &Path,
    base_name: &str,
    secret: &[u8; 32],
) -> Result<(), KeychainError> {
    let parent = path.parent().ok_or_else(|| {
        KeychainError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "key file path has no parent directory",
        ))
    })?;
    std::fs::create_dir_all(parent).map_err(KeychainError::Io)?;

    // Unique temp name in the same dir. PID + nanos is enough — only this
    // daemon writes here, and `rename` is the atomic commit point.
    let tmp = parent.join(format!(
        ".{base_name}.tmp.{}.{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));

    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        // Create with 0600 from the outset so the secret is never momentarily
        // group/other-readable between create and chmod.
        opts.mode(0o600);
    }

    let write_result = (|| -> std::io::Result<()> {
        let mut f = opts.open(&tmp)?;
        // Defence-in-depth: explicitly re-assert 0600 in case a restrictive
        // parent umask or a non-honouring filesystem ignored the create mode.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            f.set_permissions(std::fs::Permissions::from_mode(0o600))?;
        }
        f.write_all(secret)?;
        f.flush()?;
        f.sync_all()?;
        Ok(())
    })();

    if let Err(e) = write_result {
        let _ = std::fs::remove_file(&tmp);
        return Err(KeychainError::Io(e));
    }

    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(KeychainError::Io(e));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// RAII guard pointing `COPYPASTE_KEY_FILE_PATH` at a per-test file under a
    /// `tempfile::TempDir`. Serialised via the shared `TEST_ENV_LOCK` so the
    /// env mutation cannot race other env-touching daemon tests.
    ///
    /// It also forces `COPYPASTE_EPHEMERAL_KEY` ON for the duration of the test
    /// so the new legacy-Keychain migration read in `load_or_create` is bypassed
    /// (`read_legacy_keychain_key` honours that env). This keeps every file-store
    /// test fully hermetic — no real Keychain item is ever read and no macOS
    /// login-password prompt can be raised on a developer machine that happens to
    /// hold a real `(SERVICE, ACCOUNT)` entry.
    struct KeyFileEnv {
        _dir: tempfile::TempDir,
        path: PathBuf,
        original: Option<std::ffi::OsString>,
        original_ephemeral: Option<std::ffi::OsString>,
        _guard: std::sync::MutexGuard<'static, ()>,
    }

    impl KeyFileEnv {
        fn new() -> Self {
            let guard = crate::TEST_ENV_LOCK
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            let dir = tempfile::tempdir().expect("tempdir");
            let path = dir.path().join("device_secret.key");
            let original = std::env::var_os("COPYPASTE_KEY_FILE_PATH");
            let original_ephemeral = std::env::var_os("COPYPASTE_EPHEMERAL_KEY");
            // SAFETY: serialised via TEST_ENV_LOCK.
            unsafe {
                std::env::set_var("COPYPASTE_KEY_FILE_PATH", &path);
                std::env::set_var("COPYPASTE_EPHEMERAL_KEY", "1");
            }
            Self {
                _dir: dir,
                path,
                original,
                original_ephemeral,
                _guard: guard,
            }
        }
    }

    impl Drop for KeyFileEnv {
        fn drop(&mut self) {
            // SAFETY: restoring under TEST_ENV_LOCK.
            unsafe {
                match self.original.take() {
                    Some(v) => std::env::set_var("COPYPASTE_KEY_FILE_PATH", v),
                    None => std::env::remove_var("COPYPASTE_KEY_FILE_PATH"),
                }
                match self.original_ephemeral.take() {
                    Some(v) => std::env::set_var("COPYPASTE_EPHEMERAL_KEY", v),
                    None => std::env::remove_var("COPYPASTE_EPHEMERAL_KEY"),
                }
            }
        }
    }

    #[test]
    fn create_then_load_round_trips_same_secret() {
        let env = KeyFileEnv::new();
        assert!(!env.path.exists(), "fresh tempdir should have no key file");

        let kp1 = load_or_create().expect("first load creates key");
        assert!(env.path.exists(), "key file must be persisted on create");

        let kp2 = load_or_create().expect("second load reads existing key");
        assert_eq!(
            kp1.public_key_bytes(),
            kp2.public_key_bytes(),
            "reload must return the SAME device key (no prompt, no regeneration)"
        );
        assert_eq!(
            kp1.local_enc_key().as_ref(),
            kp2.local_enc_key().as_ref(),
            "SQLCipher key derived from the reloaded device key must be identical"
        );
    }

    #[cfg(unix)]
    #[test]
    fn key_file_is_mode_0600() {
        use std::os::unix::fs::PermissionsExt;
        let env = KeyFileEnv::new();
        load_or_create().expect("create key");
        let mode = std::fs::metadata(&env.path)
            .expect("stat")
            .permissions()
            .mode();
        assert_eq!(
            mode & 0o777,
            0o600,
            "device key file must be owner-only readable/writable (0600), got {:o}",
            mode & 0o777
        );
    }

    #[test]
    fn delete_then_reload_generates_new_key() {
        let env = KeyFileEnv::new();
        let kp1 = load_or_create().expect("create");
        delete_stored().expect("delete");
        assert!(!env.path.exists(), "delete must remove the file");
        let kp2 = load_or_create().expect("recreate");
        assert_ne!(
            kp1.public_key_bytes(),
            kp2.public_key_bytes(),
            "after delete a fresh key must be generated"
        );
    }

    #[test]
    fn delete_missing_file_is_ok() {
        let _env = KeyFileEnv::new();
        // No file created yet — delete must be a benign no-op.
        delete_stored().expect("deleting a missing key file is not an error");
    }

    #[test]
    fn corrupt_length_file_is_hard_error_not_silent_regen() {
        let env = KeyFileEnv::new();
        std::fs::write(&env.path, b"too short").expect("write corrupt file");
        match load_or_create() {
            Ok(_) => panic!("wrong-length key file must error, not silently regenerate"),
            Err(KeychainError::InvalidLength(9)) => {}
            Err(other) => panic!("expected InvalidLength(9), got {other:?}"),
        }
    }

    /// Upgrade-bug regression (v0.5.1 → v0.5.2): an EXISTING device-key file must
    /// NEVER be clobbered — not by a re-load, and not by the new legacy-Keychain
    /// migration path. This is the no-clobber invariant: once a key file exists,
    /// `load_or_create` reads it verbatim and the on-disk bytes are byte-for-byte
    /// unchanged across reloads, so the SQLCipher key that encrypted the DB is
    /// preserved. (Migration only runs when the file is ABSENT.)
    #[test]
    fn existing_key_file_is_never_clobbered_on_reload() {
        let env = KeyFileEnv::new();

        // Seed a known 32-byte key file directly (simulates an already-migrated
        // or previously-created install).
        let original_secret = [0x5Au8; 32];
        write_secret_atomic_to(&env.path, KEY_FILE_NAME, &original_secret).expect("seed key file");
        let on_disk_before = std::fs::read(&env.path).expect("read seeded key");
        assert_eq!(on_disk_before.len(), 32, "seeded key must be 32 bytes");

        // Two reloads must both return the SAME key and must NOT rewrite the file.
        let kp1 = load_or_create().expect("reload existing key #1");
        let kp2 = load_or_create().expect("reload existing key #2");
        assert_eq!(
            kp1.public_key_bytes(),
            kp2.public_key_bytes(),
            "reloads must return the identical device key"
        );

        let on_disk_after = std::fs::read(&env.path).expect("read key after reloads");
        assert_eq!(
            on_disk_before, on_disk_after,
            "an existing device-key file must be byte-for-byte unchanged after \
             reload — it must NEVER be clobbered (that would orphan the encrypted DB)"
        );
        // And the bytes must be exactly what we seeded.
        assert_eq!(
            &on_disk_after[..],
            &original_secret[..],
            "the persisted key must still equal the originally-seeded secret"
        );
    }

    /// CopyPaste-jdq5: the v2 per-account cloud key persists to its OWN sibling
    /// file and round-trips independently of the v1 cloud key — proving a restart
    /// can restore BOTH slots for dual-key read dispatch. Also verifies the v2
    /// file is absent until written (so a daemon that never derived v2 simply
    /// loads `None` and stays on v1) and that delete removes it.
    #[cfg(feature = "cloud-sync")]
    #[test]
    fn cloud_sync_key_v2_round_trips_and_is_independent_of_v1() {
        let _env = KeyFileEnv::new();
        // Nothing persisted yet.
        assert_eq!(
            load_cloud_sync_key_v2().expect("load v2 (absent)"),
            None,
            "no v2 key until one is derived"
        );

        let v1_secret = [0x11u8; 32];
        let v2_secret = [0x22u8; 32];
        store_cloud_sync_key(&v1_secret).expect("store v1");
        store_cloud_sync_key_v2(&v2_secret).expect("store v2");

        // Each slot reloads its OWN bytes — they never alias.
        assert_eq!(
            load_cloud_sync_key().expect("load v1"),
            Some(v1_secret),
            "v1 slot must reload the v1 bytes"
        );
        assert_eq!(
            load_cloud_sync_key_v2().expect("load v2"),
            Some(v2_secret),
            "v2 slot must reload the v2 bytes (restart-surviving dual-key read)"
        );

        // Deleting v2 must NOT disturb v1.
        delete_cloud_sync_key_v2().expect("delete v2");
        assert_eq!(
            load_cloud_sync_key_v2().expect("load v2 after delete"),
            None,
            "v2 must be gone after delete"
        );
        assert_eq!(
            load_cloud_sync_key().expect("load v1 after v2 delete"),
            Some(v1_secret),
            "deleting v2 must leave the v1 key intact"
        );
        // Deleting a missing v2 file is a benign no-op.
        delete_cloud_sync_key_v2().expect("delete missing v2 is ok");
    }

    /// When the key file is ABSENT and no legacy Keychain key is adoptable (the
    /// ephemeral bypass guarantees the migration read is skipped here), a fresh
    /// key is minted and persisted exactly once. The hermetic stand-in for "no
    /// legacy key" is `COPYPASTE_EPHEMERAL_KEY=1`, which `KeyFileEnv` sets.
    #[test]
    fn absent_file_with_no_legacy_key_mints_fresh_and_persists() {
        let env = KeyFileEnv::new();
        assert!(!env.path.exists(), "fresh tempdir should have no key file");

        let kp = load_or_create().expect("mint fresh key when nothing to migrate");
        assert!(
            env.path.exists(),
            "fresh key must be persisted to the file store"
        );

        // Re-load must return the SAME minted key from the file (no second mint).
        let kp_again = load_or_create().expect("reload minted key");
        assert_eq!(
            kp.public_key_bytes(),
            kp_again.public_key_bytes(),
            "the minted key must be the one persisted and reloaded"
        );
    }
}
