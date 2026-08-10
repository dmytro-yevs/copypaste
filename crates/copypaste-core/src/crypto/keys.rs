//! The device secret and the two keys derived from it.
//!
//! One extract, two expands, no dispatch. The keystore that holds the secret is
//! [`super::keystore`]; the envelope that consumes [`ItemKey`] is
//! [`super::aead`].

use std::path::Path;
#[cfg(feature = "dev-ephemeral-key")]
use std::sync::OnceLock;
#[cfg(any(target_os = "macos", test))]
use std::{
    sync::mpsc::{self, RecvTimeoutError},
    time::Duration,
};

use hkdf::Hkdf;
use rand::{rngs::OsRng, RngCore};
use sha2::Sha256;
use subtle::ConstantTimeEq;
use zeroize::Zeroizing;

use super::CryptoError;

/// Length of the device secret and of every key derived from it.
pub const KEY_LEN: usize = 32;

/// HKDF extract salt. One salt for the whole v2 tree; domain separation comes
/// from the `info` strings below, which is what `info` is for.
const HKDF_SALT: &[u8] = b"copypaste/v2/device-secret/hkdf-salt";

/// HKDF `info` for the SQLCipher database key.
const INFO_DB_KEY: &[u8] = b"copypaste/v2/sqlcipher-db-key";

/// HKDF `info` for the per-item content AEAD key.
const INFO_ITEM_KEY: &[u8] = b"copypaste/v2/item-content-key";

/// Development bypass, compiled out unless `dev-ephemeral-key` is enabled.
#[cfg(feature = "dev-ephemeral-key")]
const ENV_EPHEMERAL: &str = "COPYPASTE_EPHEMERAL_KEY";

/// Upper bound on a macOS Keychain load during startup (port manifest 02,
/// I-22). The previous production implementation used the same deadline.
#[cfg(any(target_os = "macos", test))]
const KEYSTORE_LOAD_TIMEOUT: Duration = Duration::from_secs(8);

/// The device secret plus the keys derived from it. The secret is zeroized on
/// drop.
pub struct Keyring {
    secret: Zeroizing<[u8; KEY_LEN]>,
}

impl Keyring {
    /// Load the device secret that opens the history in `data_dir`, creating
    /// one on first run.
    ///
    /// * **macOS** — a Keychain generic-password item under service
    ///   `com.copypaste.daemon`, account `device-secret-key`. Selected by the
    ///   target, not by a cargo feature: a feature is a way to ship without it,
    ///   and that is what happened.
    /// * **Android** — a 32-byte secret sealed with an AES-GCM key held in the
    ///   Android Keystore, kept as a blob in app-private storage.
    /// * **Windows** — the same shape: sealed with DPAPI under the user's
    ///   login, kept as `device_secret.dpapi` in the data directory. Not the
    ///   Credential Manager, which any process in the session can enumerate.
    /// * **Everywhere else** — a `0600` file named `device_secret.key`, for
    ///   development only. See `keystore::file` for why it is not a keystore.
    ///
    /// `data_dir` is the directory holding the history database, so the secret
    /// stays with the data it opens when `--data-dir` moves it (security review
    /// F-11). The Keychain and the Android Keystore are app- or user-scoped and
    /// ignore it; the argument is still theirs to receive, because the guard
    /// that refuses to mint over an existing history needs it everywhere.
    ///
    /// With the development-only `dev-ephemeral-key` feature,
    /// `COPYPASTE_EPHEMERAL_KEY` short-circuits every backend and mints a
    /// throwaway secret; data written under it is unrecoverable after exit.
    ///
    /// # Errors
    ///
    /// [`CryptoError::KeystoreUnavailable`] when the keystore's state could not
    /// be determined — *not* the same as "no secret exists yet", which creates
    /// one and returns `Ok`. [`CryptoError::KeystoreEntryUnusable`] when a
    /// secret is there and cannot be read, which no retry will fix.
    pub fn load_or_create(data_dir: &Path) -> Result<Self, CryptoError> {
        #[cfg(feature = "dev-ephemeral-key")]
        if ephemeral_requested() {
            tracing::warn!(
                "{ENV_EPHEMERAL} is set: using a throwaway device secret. \
                 Anything written now becomes unreadable when this process exits."
            );
            return Ok(Self::from_secret(&random_secret()));
        }
        #[cfg(target_os = "macos")]
        let secret = {
            let lookup_dir = data_dir.to_path_buf();
            let lookup = load_with_timeout(KEYSTORE_LOAD_TIMEOUT, move || {
                super::keystore::lookup_secret(&lookup_dir)
            })?;
            super::keystore::finish_load_or_create_secret(data_dir, lookup)?
        };
        #[cfg(not(target_os = "macos"))]
        let secret = super::keystore::load_or_create_secret(data_dir)?;
        Ok(Self { secret })
    }

    /// Deterministic construction from a known secret; touches no keystore.
    pub fn from_secret(secret: &[u8; KEY_LEN]) -> Self {
        Self {
            secret: Zeroizing::new(*secret),
        }
    }

    /// Key for SQLCipher. The caller formats the PRAGMA:
    /// SQLCipher wants `PRAGMA key = "x'<64 lowercase hex chars>'"` applied
    /// *before any other statement*, and both the hex string and this array
    /// and the returned copy is zeroized when the caller is done with it.
    pub fn db_key(&self) -> Zeroizing<[u8; KEY_LEN]> {
        Zeroizing::new(derive(&self.secret, INFO_DB_KEY))
    }

    /// Key for item content AEAD.
    pub fn item_key(&self) -> ItemKey {
        ItemKey(Zeroizing::new(derive(&self.secret, INFO_ITEM_KEY)))
    }
}

/// Run a potentially prompting Keychain load without allowing it to hold
/// startup indefinitely.
#[cfg(any(target_os = "macos", test))]
fn load_with_timeout<T, F>(timeout: Duration, load: F) -> Result<T, CryptoError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, CryptoError> + Send + 'static,
{
    let (sender, receiver) = mpsc::sync_channel(1);
    let worker = std::thread::Builder::new()
        .name("keystore-load".into())
        .spawn(move || {
            let _ = sender.send(load());
        })
        .map_err(|_| CryptoError::KeystoreUnavailable("the keychain read could not be started"))?;

    match receiver.recv_timeout(timeout) {
        Ok(result) => {
            let _ = worker.join();
            result
        }
        Err(RecvTimeoutError::Timeout) => {
            // Security-framework exposes no cancellation. Dropping the handle
            // detaches the worker until the prompt resolves or the process
            // exits; it owns no database handle, and a late secret is zeroized.
            drop(worker);
            Err(CryptoError::KeystoreUnavailable(
                "the keychain read timed out",
            ))
        }
        Err(RecvTimeoutError::Disconnected) => {
            let _ = worker.join();
            Err(CryptoError::KeystoreUnavailable(
                "the keychain read ended unexpectedly",
            ))
        }
    }
}

/// Redacted: a `Debug` that prints key bytes ends up in a log file.
impl std::fmt::Debug for Keyring {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Keyring").finish_non_exhaustive()
    }
}

/// The per-item content key. Zeroized on drop; never printed. The field is
/// `pub(super)` only so [`super::aead`] can reach the raw bytes.
pub struct ItemKey(pub(super) Zeroizing<[u8; KEY_LEN]>);

impl ItemKey {
    /// Constant-time equality, and the only equality this type has: `==` on
    /// key bytes short-circuits on the first differing byte and leaks a
    /// prefix-match length by timing (port manifest 02, I-13).
    pub fn ct_eq(&self, other: &Self) -> bool {
        self.0.as_ref().ct_eq(other.0.as_ref()).into()
    }
}

/// Deliberately routed through [`ItemKey::ct_eq`] so no caller can reach a
/// short-circuiting comparison of key bytes.
impl PartialEq for ItemKey {
    fn eq(&self, other: &Self) -> bool {
        self.ct_eq(other)
    }
}

impl Eq for ItemKey {}

/// Redacted, same reason as [`Keyring`]'s.
impl std::fmt::Debug for ItemKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ItemKey").finish_non_exhaustive()
    }
}

/// HKDF-SHA256 over the device secret. One extract, one expand per `info`.
fn derive(secret: &[u8; KEY_LEN], info: &[u8]) -> [u8; KEY_LEN] {
    let hk = Hkdf::<Sha256>::new(Some(HKDF_SALT), secret);
    let mut okm = [0u8; KEY_LEN];
    // Infallible for any length <= 255 * 32 bytes, and KEY_LEN is 32. The
    // `expect` documents that; it is not reachable from caller input.
    hk.expand(info, &mut okm)
        .expect("HKDF expand of 32 bytes cannot fail");
    okm
}

/// 32 bytes from the OS CSPRNG.
pub(super) fn random_secret() -> [u8; KEY_LEN] {
    let mut secret = [0u8; KEY_LEN];
    OsRng.fill_bytes(&mut secret);
    secret
}

/// Read the development bypass exactly once per process (I-23).
#[cfg(feature = "dev-ephemeral-key")]
fn ephemeral_requested() -> bool {
    static EPHEMERAL: OnceLock<bool> = OnceLock::new();
    *EPHEMERAL.get_or_init(|| std::env::var_os(ENV_EPHEMERAL).is_some())
}

/// Shipped builds do not consult the process environment for key selection.
#[cfg(all(not(feature = "dev-ephemeral-key"), test))]
const fn ephemeral_requested() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use std::process::Command;
    use std::sync::mpsc;

    use super::super::test_support::{key_a, ITEM, SECRET_A, SECRET_B};
    use super::super::{decrypt, encrypt};
    use super::*;

    /// For the one test whose subject *is* the deadline. Short so it does not
    /// stall the suite.
    const TEST_TIMEOUT: Duration = Duration::from_millis(20);

    /// For the three below, whose subject is what a completed load returns.
    /// 20 ms is a thread spawn plus a channel hop, and on a Windows runner with
    /// 300 tests in flight that budget is genuinely marginal — those tests were
    /// timing out and reporting it as the wrong outcome.
    const AMPLE_TIMEOUT: Duration = Duration::from_secs(10);

    const FEATURE_CHILD_MODE: &str = "COPYPASTE_EPHEMERAL_KEY_FEATURE_TEST_CHILD";

    fn run_feature_child(test: &str, mode: &str, ephemeral_env: bool) {
        let (_, module) = module_path!().split_once("::").unwrap();
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args(["--exact", &format!("{module}::{test}"), "--nocapture"])
            .env(FEATURE_CHILD_MODE, mode);
        if ephemeral_env {
            command.env("COPYPASTE_EPHEMERAL_KEY", "1");
        } else {
            command.env_remove("COPYPASTE_EPHEMERAL_KEY");
        }
        assert!(command.status().unwrap().success());
    }

    #[cfg(not(feature = "dev-ephemeral-key"))]
    #[test]
    fn release_feature_set_ignores_ephemeral_environment() {
        if std::env::var_os(FEATURE_CHILD_MODE).is_none() {
            run_feature_child(
                "release_feature_set_ignores_ephemeral_environment",
                "release",
                true,
            );
            return;
        }

        assert!(std::env::var_os("COPYPASTE_EPHEMERAL_KEY").is_some());
        assert!(!ephemeral_requested());

        #[cfg(not(any(target_os = "macos", target_os = "android")))]
        {
            let dir = tempfile::tempdir().unwrap();
            let first = Keyring::load_or_create(dir.path()).unwrap();
            #[cfg(target_os = "windows")]
            assert!(dir.path().join("device_secret.dpapi").exists());
            #[cfg(not(target_os = "windows"))]
            assert!(dir.path().join("device_secret.key").exists());
            let reopened = Keyring::load_or_create(dir.path()).unwrap();
            assert_eq!(first.db_key(), reopened.db_key());
        }
    }

    #[cfg(feature = "dev-ephemeral-key")]
    #[test]
    fn development_feature_short_circuits_and_caches_its_choice() {
        let Some(mode) = std::env::var_os(FEATURE_CHILD_MODE) else {
            run_feature_child(
                "development_feature_short_circuits_and_caches_its_choice",
                "enabled",
                true,
            );
            run_feature_child(
                "development_feature_short_circuits_and_caches_its_choice",
                "disabled",
                false,
            );
            return;
        };

        if mode == "enabled" {
            assert!(ephemeral_requested());
            std::env::remove_var(ENV_EPHEMERAL);
            assert!(ephemeral_requested());

            let dir = tempfile::tempdir().unwrap();
            let database = copypaste_ipc::database_path();
            std::fs::write(dir.path().join(database.file_name().unwrap()), b"existing").unwrap();
            Keyring::load_or_create(dir.path()).expect("the bypass must precede the backend");
            assert!(!dir.path().join("device_secret.key").exists());
            assert!(!dir.path().join("device_secret.dpapi").exists());
        } else {
            assert_eq!(mode, "disabled");
            assert!(!ephemeral_requested());
            std::env::set_var(ENV_EPHEMERAL, "1");
            assert!(!ephemeral_requested());
        }
    }

    #[test]
    fn bounded_load_returns_a_successful_secret() {
        let secret = load_with_timeout(AMPLE_TIMEOUT, || Ok(Zeroizing::new(SECRET_A))).unwrap();

        assert_eq!(*secret, SECRET_A);
    }

    #[test]
    fn bounded_load_preserves_a_backend_error() {
        let result: Result<Zeroizing<[u8; KEY_LEN]>, CryptoError> =
            load_with_timeout(AMPLE_TIMEOUT, || {
                Err(CryptoError::KeystoreUnavailable(
                    "the keychain is locked or access was denied",
                ))
            });

        assert!(matches!(
            result,
            Err(CryptoError::KeystoreUnavailable(
                "the keychain is locked or access was denied"
            ))
        ));
    }

    #[test]
    fn bounded_load_preserves_a_wrong_length_error() {
        let result: Result<Zeroizing<[u8; KEY_LEN]>, CryptoError> =
            load_with_timeout(AMPLE_TIMEOUT, || {
                Err(CryptoError::KeystoreEntryUnusable(
                    "the stored device secret is the wrong length",
                ))
            });

        assert!(matches!(result, Err(CryptoError::KeystoreEntryUnusable(_))));
    }

    #[test]
    fn bounded_load_rejects_an_operation_past_the_deadline() {
        let (release, blocked) = mpsc::channel();
        let (finished, worker_finished) = mpsc::channel();
        let result = load_with_timeout(TEST_TIMEOUT, move || {
            blocked.recv().unwrap();
            let result = Ok(Zeroizing::new(SECRET_A));
            finished.send(()).unwrap();
            result
        });

        assert!(matches!(
            result,
            Err(CryptoError::KeystoreUnavailable(
                "the keychain read timed out"
            ))
        ));

        release.send(()).unwrap();
        worker_finished.recv().unwrap();
    }

    #[test]
    fn production_keychain_deadline_is_eight_seconds() {
        assert_eq!(KEYSTORE_LOAD_TIMEOUT, Duration::from_secs(8));
    }

    #[test]
    fn key_derivation_is_deterministic_for_a_fixed_secret() {
        let a = Keyring::from_secret(&SECRET_A);
        let b = Keyring::from_secret(&SECRET_A);

        assert_eq!(a.db_key(), b.db_key());
        // Compared in constant time via ItemKey's PartialEq.
        assert!(a.item_key() == b.item_key());

        // And it is stable across separately built rings, which is what a
        // daemon restart does.
        let (nonce, ct) = encrypt(b"stable", &a.item_key(), ITEM).unwrap();
        assert_eq!(
            decrypt(&ct, &nonce, &b.item_key(), ITEM).unwrap(),
            b"stable"
        );
    }

    #[test]
    fn different_secrets_derive_different_keys() {
        let a = Keyring::from_secret(&SECRET_A);
        let b = Keyring::from_secret(&SECRET_B);

        assert_ne!(a.db_key(), b.db_key());
        assert!(a.item_key() != b.item_key());
    }

    #[test]
    fn database_keys_zeroize_on_drop() {
        fn assert_zeroize_on_drop<T: zeroize::ZeroizeOnDrop>() {}
        assert_zeroize_on_drop::<Zeroizing<[u8; KEY_LEN]>>();
    }

    #[test]
    fn db_key_and_item_key_are_domain_separated() {
        // Same IKM, same salt, different `info`. If these ever collide, a
        // SQLCipher header disclosure would also disclose the item key.
        let ring = Keyring::from_secret(&SECRET_A);
        let item = ring.item_key();
        assert_ne!(*ring.db_key(), *item.0.as_ref());
    }

    #[test]
    fn derived_keys_are_not_the_secret_itself() {
        // v1's "the seed is the item key" quirk (port manifest 02, §3.2.3) was
        // the single most easily-broken thing in that design. v2 derives both
        // keys; neither is the stored secret.
        let ring = Keyring::from_secret(&SECRET_A);
        assert_ne!(*ring.db_key(), SECRET_A);
        assert_ne!(*ring.item_key().0.as_ref(), SECRET_A);
    }

    #[test]
    fn hkdf_info_strings_are_the_documented_ones() {
        // Pins the derivation labels. Changing either of these silently
        // re-keys every install, so a change should be a visible, deliberate
        // edit of both the constant and this test.
        assert_eq!(INFO_DB_KEY, b"copypaste/v2/sqlcipher-db-key");
        assert_eq!(INFO_ITEM_KEY, b"copypaste/v2/item-content-key");
        assert_eq!(HKDF_SALT, b"copypaste/v2/device-secret/hkdf-salt");
        assert_ne!(INFO_DB_KEY, INFO_ITEM_KEY);
    }

    #[test]
    fn debug_output_never_contains_key_material() {
        let ring = Keyring::from_secret(&SECRET_A);
        let item = ring.item_key();

        for rendered in [format!("{ring:?}"), format!("{item:?}")] {
            assert!(
                !rendered.contains('7'),
                "secret byte value leaked: {rendered}"
            );
            for byte in ring.db_key().iter() {
                assert!(!rendered.contains(&byte.to_string()));
            }
        }
        assert_eq!(format!("{ring:?}"), "Keyring { .. }");
        assert_eq!(format!("{item:?}"), "ItemKey { .. }");
    }

    #[test]
    fn ct_eq_agrees_with_byte_equality() {
        let a = key_a();
        let a2 = Keyring::from_secret(&SECRET_A).item_key();
        let b = Keyring::from_secret(&SECRET_B).item_key();

        assert!(a.ct_eq(&a2));
        assert!(!a.ct_eq(&b));

        // Also for a one-bit difference, where a short-circuiting compare
        // would be indistinguishable in result but not in timing.
        let mut near = SECRET_A;
        near[KEY_LEN - 1] ^= 0x01;
        assert!(!a.ct_eq(&Keyring::from_secret(&near).item_key()));
    }
}
