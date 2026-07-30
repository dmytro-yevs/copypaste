//! The device secret and the two keys derived from it.
//!
//! One extract, two expands, no dispatch. The keystore that holds the secret is
//! [`super::keystore`]; the envelope that consumes [`ItemKey`] is
//! [`super::aead`].

use std::sync::OnceLock;

use hkdf::Hkdf;
use rand::{rngs::OsRng, RngCore};
use sha2::Sha256;
use subtle::ConstantTimeEq;
use zeroize::Zeroizing;

use super::CryptoError;

/// Length of the device secret and of every key derived from it.
pub const KEY_LEN: usize = 32;

/// HKDF extract salt. One salt for the whole v2 tree — domain separation
/// between the derived keys comes from the `info` strings below, which is what
/// HKDF's `info` parameter is for. Using distinct salts *as well* would be
/// belt-and-braces with no additional property.
const HKDF_SALT: &[u8] = b"copypaste/v2/device-secret/hkdf-salt";

/// HKDF `info` for the SQLCipher database key.
const INFO_DB_KEY: &[u8] = b"copypaste/v2/sqlcipher-db-key";

/// HKDF `info` for the per-item content AEAD key.
const INFO_ITEM_KEY: &[u8] = b"copypaste/v2/item-content-key";

/// Dev/test bypass. When set to any value, no keystore is touched at all and a
/// fresh random secret is minted for the process lifetime. Read exactly once
/// (see [`ephemeral_requested`]) so that mutating the environment of a running
/// process cannot flip an already-keyed daemon into ephemeral mode
/// (port manifest 02, I-23).
const ENV_EPHEMERAL: &str = "COPYPASTE_EPHEMERAL_KEY";

/// The device secret plus the keys derived from it.
///
/// Construct with [`Keyring::load_or_create`] in production or
/// [`Keyring::from_secret`] in tests. The secret is zeroized when the `Keyring`
/// is dropped.
pub struct Keyring {
    secret: Zeroizing<[u8; KEY_LEN]>,
}

impl Keyring {
    /// Load the device secret from the OS keystore, creating one on first run.
    ///
    /// * **macOS** — a Keychain generic-password item under service
    ///   `com.copypaste.daemon`, account `device-secret-key`. Requires the
    ///   `macos-keychain` cargo feature (see the note above the `allow` in
    ///   [`crate::crypto`]).
    /// * **Every other platform** — a `0600` file named `device_secret.key`
    ///   under the platform data directory.
    ///
    /// **The file backend is for development only.** It is not a keystore: the
    /// secret sits in the user's home directory in plaintext, protected by
    /// nothing but Unix file permissions, and is readable by any process
    /// running as that user and by anything that backs the directory up. It
    /// exists so the daemon can be built and tested on a Linux workstation.
    /// Android must use the Android Keystore before shipping — a
    /// platform-`cfg`'d backend added here, alongside the macOS one.
    ///
    /// Setting `COPYPASTE_EPHEMERAL_KEY` short-circuits both backends before
    /// any keystore call and mints a throwaway secret. Data written under it is
    /// unrecoverable after the process exits, which is the point.
    ///
    /// # Errors
    ///
    /// [`CryptoError::KeystoreUnavailable`] when the keystore's state could not
    /// be determined. That is *not* the same as "no secret exists yet": a first
    /// run creates a secret and returns `Ok`. See the variant's docs.
    pub fn load_or_create() -> Result<Self, CryptoError> {
        if ephemeral_requested() {
            tracing::warn!(
                "{ENV_EPHEMERAL} is set: using a throwaway device secret. \
                 Anything written now becomes unreadable when this process exits."
            );
            return Ok(Self::from_secret(&random_secret()));
        }
        let secret = super::keystore::load_or_create_secret()?;
        Ok(Self { secret })
    }

    /// Deterministic construction from a known secret.
    ///
    /// For tests and for callers that obtained the secret some other way. Does
    /// not touch any keystore.
    pub fn from_secret(secret: &[u8; KEY_LEN]) -> Self {
        Self {
            secret: Zeroizing::new(*secret),
        }
    }

    /// Key for SQLCipher, as raw bytes.
    ///
    /// The caller formats the PRAGMA — this module does not build SQL. Note
    /// that SQLCipher wants `PRAGMA key = "x'<64 lowercase hex chars>'"`
    /// applied *before any other statement*, and that both the hex string and
    /// this array should be wrapped in `zeroize::Zeroizing` at the call site;
    /// the signature is fixed by the shared API contract, so it cannot do that
    /// for you.
    pub fn db_key(&self) -> [u8; KEY_LEN] {
        derive(&self.secret, INFO_DB_KEY)
    }

    /// Key for item content AEAD.
    pub fn item_key(&self) -> ItemKey {
        ItemKey(Zeroizing::new(derive(&self.secret, INFO_ITEM_KEY)))
    }
}

/// Redacted: a `Debug` that prints key bytes ends up in a log file.
impl std::fmt::Debug for Keyring {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Keyring").finish_non_exhaustive()
    }
}

/// The per-item content key. Zeroized on drop; never printed.
///
/// The field is `pub(super)` rather than private only so [`super::aead`] can
/// reach the raw bytes; it is not reachable from outside this module.
pub struct ItemKey(pub(super) Zeroizing<[u8; KEY_LEN]>);

impl ItemKey {
    /// Constant-time equality.
    ///
    /// `==` on key bytes short-circuits on the first differing byte and leaks
    /// a prefix-match length by timing (port manifest 02, I-13). This is the
    /// only equality this type has.
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

/// Read the dev bypass exactly once per process.
fn ephemeral_requested() -> bool {
    static EPHEMERAL: OnceLock<bool> = OnceLock::new();
    *EPHEMERAL.get_or_init(|| std::env::var_os(ENV_EPHEMERAL).is_some())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::super::test_support::{key_a, ITEM, SECRET_A, SECRET_B};
    use super::super::{decrypt, encrypt};
    use super::*;

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
    fn db_key_and_item_key_are_domain_separated() {
        // Same IKM, same salt, different `info`. If these ever collide, a
        // SQLCipher header disclosure would also disclose the item key.
        let ring = Keyring::from_secret(&SECRET_A);
        let item = ring.item_key();
        assert_ne!(ring.db_key(), *item.0.as_ref());
    }

    #[test]
    fn derived_keys_are_not_the_secret_itself() {
        // v1's "the seed is the item key" quirk (port manifest 02, §3.2.3) was
        // the single most easily-broken thing in that design. v2 derives both
        // keys; neither is the stored secret.
        let ring = Keyring::from_secret(&SECRET_A);
        assert_ne!(ring.db_key(), SECRET_A);
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
            for byte in ring.db_key() {
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
