//! The key: where it comes from, what it costs, and why those numbers.
//!
//! # Argon2id parameters (port manifest 02, I-17 and §4.2)
//!
//! `m = 19456 KiB (19 MiB)`, `t = 2`, `p = 1`, `Version::V0x13`, 32-byte output.
//!
//! These are the OWASP "second choice" interactive-login parameters, which the
//! v1 manifest already pinned as a **floor, not a suggestion**, and they are
//! still current. The reasoning behind each, restated so a future change is a
//! decision rather than a drift:
//!
//! * **`m = 19 MiB`** is the memory cost, and memory is what makes an ASIC or
//!   GPU attack expensive. Raising it raises the attacker's cost linearly and
//!   ours too — on every device that derives the key, including a phone. 19 MiB
//!   is the largest value that is comfortable on the low end for a flow the user
//!   performs rarely (once per device, at sign-in).
//! * **`t = 2`** is the OWASP-paired iteration count at 19 MiB. It buys mixing
//!   without doubling memory.
//! * **`p = 1`** is deliberate and is *not* a performance oversight. Both devices
//!   must derive a byte-identical key, so the parameter set has to be
//!   reproducible on a single-threaded or WASM target. Parallelism would also not
//!   help much here: it does not change the memory an attacker must commit.
//!
//! Raising any of these is a key change: every existing account's ciphertext
//! becomes unreadable, exactly as a passphrase change would. There is no
//! parameter field on the wire and no trial decryption — see "fail closed" in
//! the module docs.

use argon2::{Algorithm, Argon2, Params, Version};
use hkdf::Hkdf;
use sha2::Sha256;
use zeroize::Zeroizing;

use super::error::CloudCryptoError;

/// Length of a [`SyncKey`].
pub const KEY_LEN: usize = 32;

/// Argon2id memory cost in KiB. See the module docs.
pub const ARGON2_M_COST_KIB: u32 = 19_456;

/// Argon2id time cost (iterations). See the module docs.
pub const ARGON2_T_COST: u32 = 2;

/// Argon2id parallelism. Pinned at 1 for cross-platform reproducibility.
pub const ARGON2_P_COST: u32 = 1;

/// Minimum passphrase length, counted in `chars()` rather than bytes so that a
/// twelve-character passphrase in a non-Latin script is not rejected for being
/// "too short" when it is in fact longer than a Latin one (manifest 02, I-18).
///
/// Checked *before* Argon2id runs, so a too-short passphrase costs no memory
/// and the user gets an immediate answer.
pub const MIN_PASSPHRASE_CHARS: usize = 12;

/// Argon2id salt length. 16 bytes is the recommended size; the salt is derived,
/// not stored, so there is no wire cost to it.
const ARGON2_SALT_LEN: usize = 16;

/// HKDF extract salt for the per-account Argon2id salt derivation.
const SALT_HKDF_SALT: &[u8] = b"copypaste/v2/cloud-sync-key/salt-hkdf";

/// HKDF `info` for the per-account Argon2id salt.
const INFO_PER_ACCOUNT_SALT: &[u8] = b"copypaste/v2/cloud-sync-key/per-account-argon2id-salt";

/// A 32-byte key derived from the user's sync passphrase and account id.
///
/// Zeroized on drop. Deliberately has no `Debug`, `Display`, `Clone` or `Copy`
/// (manifest 02, I-12): a `Debug` ends up in a log file, and a `Clone` makes it
/// impossible to reason about how many copies are in memory.
///
/// It also has no `PartialEq`. There is no comparison to get wrong, and none is
/// needed — nothing in v2 rotates or re-provisions this key in place, so the
/// constant-time-comparison hazard of manifest 02 I-13 does not arise. If a
/// rotation flow is added later, add a `subtle::ConstantTimeEq`-backed
/// `ct_eq` and no plain `==`.
pub struct SyncKey {
    material: Zeroizing<[u8; KEY_LEN]>,
    /// The row-signing subkey, derived once here rather than per row.
    ///
    /// It is a pure function of `material`, which is resident for the life of
    /// the key anyway, so holding it adds no secret that was not already
    /// reachable — and [`super::sign`] runs on every row of every push and
    /// every pull, where one HKDF-Expand each way is the whole of the work.
    row_signing: Zeroizing<[u8; KEY_LEN]>,
}

impl SyncKey {
    /// Construct from raw bytes.
    ///
    /// For callers that already hold the derived key — the daemon reads it back
    /// from the OS keystore rather than asking for the passphrase on every
    /// start. Does not validate anything; there is nothing to validate about 32
    /// random-looking bytes.
    pub fn from_bytes(bytes: [u8; KEY_LEN]) -> Self {
        Self::new(Zeroizing::new(bytes))
    }

    fn new(material: Zeroizing<[u8; KEY_LEN]>) -> Self {
        let row_signing = super::sign::derive_row_signing_key(material.as_ref());
        Self {
            material,
            row_signing,
        }
    }

    /// The raw key bytes, for handing to the keystore.
    ///
    /// Wrapped in `Zeroizing` so the copy this makes is erased when the caller
    /// drops it. Keep the borrow short.
    pub fn to_bytes(&self) -> Zeroizing<[u8; KEY_LEN]> {
        Zeroizing::new(*self.material)
    }

    /// The key material, for the AEAD in [`super::row`].
    ///
    /// Module-private and a borrow rather than a copy: [`SyncKey::to_bytes`]
    /// exists for the keystore and hands out an owned copy, which is the wrong
    /// shape for something called once per row.
    pub(super) fn material(&self) -> &[u8] {
        self.material.as_ref()
    }

    /// The row-signing subkey. Derived once, at construction; see the field.
    pub(super) fn row_signing_key(&self) -> &[u8] {
        self.row_signing.as_ref()
    }
}

/// Derive the sync key from a passphrase and an account id.
///
/// The passphrase never leaves the device and is never stored. The salt is
/// derived from `account_id`, so two users who choose the same passphrase get
/// different keys, and an attacker who wants to precompute must do it per
/// account rather than once for everybody.
///
/// `account_id` must be byte-identical on every device of the account or they
/// will not agree on a key (manifest 02, I-8). Use one canonical form — the
/// GoTrue `user_id` is the obvious choice, since [`crate::auth::Session`]
/// already carries it and it is stable for the life of the account.
///
/// This is deliberately slow: ~19 MiB and a few tens of milliseconds. Call it
/// once, at sign-in, and keep the [`SyncKey`]; do not call it per row. On an
/// async runtime, run it inside `spawn_blocking` — it is CPU-bound and will
/// otherwise stall the reactor.
///
/// # Errors
///
/// [`CloudCryptoError::PassphraseTooShort`] if the passphrase is shorter than
/// [`MIN_PASSPHRASE_CHARS`] characters; checked before any work is done.
///
/// [`CloudCryptoError::EmptyAccountId`] if `account_id` is empty.
///
/// [`CloudCryptoError::Internal`] if Argon2 rejects the pinned parameters,
/// which cannot happen for the constants above and is therefore a build
/// problem, not a runtime one.
pub fn derive_sync_key(passphrase: &str, account_id: &str) -> Result<SyncKey, CloudCryptoError> {
    if passphrase.chars().count() < MIN_PASSPHRASE_CHARS {
        return Err(CloudCryptoError::PassphraseTooShort);
    }
    if account_id.is_empty() {
        return Err(CloudCryptoError::EmptyAccountId);
    }

    let salt = per_account_salt(account_id);

    let params = Params::new(
        ARGON2_M_COST_KIB,
        ARGON2_T_COST,
        ARGON2_P_COST,
        Some(KEY_LEN),
    )
    .map_err(|_| CloudCryptoError::Internal("Argon2 rejected the pinned parameters"))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    let mut okm = Zeroizing::new([0u8; KEY_LEN]);
    argon2
        .hash_password_into(passphrase.as_bytes(), salt.as_ref(), okm.as_mut())
        .map_err(|_| CloudCryptoError::Internal("Argon2 derivation failed"))?;

    Ok(SyncKey::new(okm))
}

/// HKDF-SHA256 over the account id. Not a secret and not meant to be — a salt's
/// job is uniqueness, not confidentiality.
///
/// `account_id` is the whole IKM rather than being concatenated into an `info`
/// string, so there is no delimiter for a hostile account id to abuse
/// (`CopyPaste-lkmy` again); two distinct account ids are two distinct IKMs.
fn per_account_salt(account_id: &str) -> Zeroizing<[u8; ARGON2_SALT_LEN]> {
    let hk = Hkdf::<Sha256>::new(Some(SALT_HKDF_SALT), account_id.as_bytes());
    let mut salt = Zeroizing::new([0u8; ARGON2_SALT_LEN]);
    // Infallible for any length <= 255 * 32 bytes, and this is 16.
    hk.expand(INFO_PER_ACCOUNT_SALT, salt.as_mut())
        .expect("HKDF expand of 16 bytes cannot fail");
    salt
}

// Tests

#[cfg(test)]
mod tests {
    use super::super::row::{decrypt_row, encrypt_row, NONCE_LEN, TAG_LEN};
    use super::super::testkit::{key, ACCOUNT, ITEM, PASS};
    use super::*;

    #[test]
    fn derivation_is_deterministic_for_a_fixed_passphrase_and_account() {
        let a = derive_sync_key(PASS, ACCOUNT).unwrap();
        let b = derive_sync_key(PASS, ACCOUNT).unwrap();
        assert_eq!(a.to_bytes().as_ref(), b.to_bytes().as_ref());
    }

    #[test]
    fn derivation_diverges_when_either_input_changes() {
        let base = derive_sync_key(PASS, ACCOUNT).unwrap();
        let other_pass = derive_sync_key("a different passphrase entirely", ACCOUNT).unwrap();
        let other_account = derive_sync_key(PASS, "3f2b1c0a-0000-4000-8000-000000000002").unwrap();

        assert_ne!(base.to_bytes().as_ref(), other_pass.to_bytes().as_ref());
        assert_ne!(base.to_bytes().as_ref(), other_account.to_bytes().as_ref());
        assert_ne!(
            other_pass.to_bytes().as_ref(),
            other_account.to_bytes().as_ref()
        );
    }

    #[test]
    fn a_one_character_account_difference_changes_the_key() {
        // The salt must actually depend on the whole account id, not on a
        // prefix or a hash bucket of it.
        let a = derive_sync_key(PASS, "acct-a").unwrap();
        let b = derive_sync_key(PASS, "acct-b").unwrap();
        assert_ne!(a.to_bytes().as_ref(), b.to_bytes().as_ref());
    }

    #[test]
    fn per_account_salt_is_deterministic_and_unique() {
        assert_eq!(
            per_account_salt("acct-a").as_ref(),
            per_account_salt("acct-a").as_ref()
        );
        assert_ne!(
            per_account_salt("acct-a").as_ref(),
            per_account_salt("acct-b").as_ref()
        );
        // And it never collapses to the account id itself or to zero.
        assert_ne!(per_account_salt("acct-a").as_ref(), &[0u8; ARGON2_SALT_LEN]);
    }

    /// `SyncKey` has no `Debug` on purpose, so `unwrap_err` is unavailable —
    /// which is the point. Compare the error without ever formatting the key.
    fn rejection(result: Result<SyncKey, CloudCryptoError>) -> CloudCryptoError {
        match result {
            Err(e) => e,
            Ok(_) => panic!("expected a rejection, got a key"),
        }
    }

    #[test]
    fn short_passphrases_are_rejected_before_any_work() {
        // Eleven characters, counted in chars.
        assert_eq!(
            rejection(derive_sync_key("elevenchar!", ACCOUNT)),
            CloudCryptoError::PassphraseTooShort
        );
        assert_eq!(
            rejection(derive_sync_key("", ACCOUNT)),
            CloudCryptoError::PassphraseTooShort
        );

        // Exactly twelve succeeds, and the count is in chars, not bytes: this
        // one is 12 chars and 24 bytes.
        assert!(derive_sync_key("паролькудачи", ACCOUNT).is_ok());
        assert!(derive_sync_key("123456789012", ACCOUNT).is_ok());
    }

    #[test]
    fn an_empty_account_id_is_rejected() {
        assert_eq!(
            rejection(derive_sync_key(PASS, "")),
            CloudCryptoError::EmptyAccountId
        );
    }

    #[test]
    fn passphrase_length_is_checked_before_the_account_id() {
        // Both invalid: the user should be told about the thing they typed,
        // and it should cost no Argon2 memory to find out.
        assert_eq!(
            rejection(derive_sync_key("short", "")),
            CloudCryptoError::PassphraseTooShort
        );
    }

    #[test]
    fn argon2_parameters_are_the_documented_ones() {
        // These are a floor (manifest 02, I-17). Changing any of them makes
        // every existing account's ciphertext unreadable, so a change should be
        // a visible edit of both the constant and this test.
        assert_eq!(ARGON2_M_COST_KIB, 19_456);
        assert_eq!(ARGON2_T_COST, 2);
        assert_eq!(ARGON2_P_COST, 1);
        assert_eq!(MIN_PASSPHRASE_CHARS, 12);
        assert_eq!(KEY_LEN, 32);
        assert_eq!(NONCE_LEN, 24);
        assert_eq!(TAG_LEN, 16);
        // The parameter set must be constructible, or every derivation is an
        // Internal error at runtime rather than a compile failure.
        assert!(Params::new(
            ARGON2_M_COST_KIB,
            ARGON2_T_COST,
            ARGON2_P_COST,
            Some(KEY_LEN)
        )
        .is_ok());
    }

    #[test]
    fn from_bytes_and_to_bytes_round_trip() {
        // The keystore path: derive once, store the bytes, rebuild on restart.
        let derived = key();
        let stored = *derived.to_bytes();
        let rebuilt = SyncKey::from_bytes(stored);

        let (nonce, ct) = encrypt_row(b"across a restart", &derived, ITEM).unwrap();
        assert_eq!(
            decrypt_row(&ct, &nonce, &rebuilt, ITEM).unwrap().as_slice(),
            b"across a restart"
        );
    }
}
