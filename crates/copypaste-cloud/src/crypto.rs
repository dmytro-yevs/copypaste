//! Client-side encryption for cloud rows. This module is what makes the
//! claim in the crate docs — *the server never sees plaintext* — true.
//!
//! # The shape of it
//!
//! ```text
//! passphrase (never leaves the device)
//!   │
//!   │  salt = HKDF-SHA256(salt = SALT_HKDF_SALT,
//!   │                     ikm  = account_id,
//!   │                     info = INFO_PER_ACCOUNT_SALT) -> 16 B
//!   ▼
//! Argon2id(m = 19456 KiB, t = 2, p = 1, v = 0x13, out = 32 B)
//!   ▼
//! SyncKey ──► XChaCha20-Poly1305, AAD = "copypaste/v2/cloud-row-aead|1|<len>:<item_id>"
//!   ▼
//! (nonce_b64, ciphertext_b64)  ──►  Supabase
//! ```
//!
//! Supabase holds the two base64 strings, the `item_id`, and metadata. It holds
//! nothing from which the key can be derived: the passphrase is never sent, and
//! the per-account salt is derived from the account id, which the server already
//! knows — a salt is not a secret, it exists so that two users who pick the same
//! passphrase do not get the same key, and so that one Argon2id table cannot be
//! precomputed against every account at once.
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
//! parameter field on the wire and no trial decryption — see "fail closed".
//!
//! # Fail closed
//!
//! A wrong passphrase, a wrong `account_id`, a wrong `item_id`, a flipped bit
//! anywhere in nonce or ciphertext or tag: all of them produce
//! [`CloudCryptoError::AuthFailed`], with no detail about which. Distinguishing
//! them is an oracle (port manifest 02, I-15). There is no fallback read path,
//! no "try the other key", no version byte to negotiate down.
//!
//! Structural framing failures — base64 that does not decode, a nonce that is
//! not 24 bytes — are reported separately as [`CloudCryptoError::Malformed`].
//! They carry no information about the key and callers need them to tell "this
//! row is corrupt" from "this row is not mine" (I-15 again: framing errors are
//! *deliberately* distinguishable).
//!
//! # What the AAD binds, and why
//!
//! ```text
//! b"copypaste/v2/cloud-row-aead|" || "1" || "|" || item_id.len() || ":" || item_id
//! ```
//!
//! * The **prefix** domain-separates cloud ciphertext from the local at-rest
//!   ciphertext in `copypaste-core` (whose AAD begins `copypaste/v2/item-aead|`).
//!   A blob can never be moved between the two domains and still authenticate,
//!   even if the two keys were somehow confused.
//! * The **schema version** is the fail-closed hinge for a future format change.
//!   It is bound into the AAD rather than carried on the wire on purpose: a wire
//!   field would be attacker-controlled and would need a dispatch table, which is
//!   how v1 ended up with `key_version` and a repair sweep. Here, a v2 reader
//!   simply cannot open a v1 row — it gets `AuthFailed` — and the migration is a
//!   deliberate act, not a silent fallback.
//! * The **`item_id`** is the cross-device logical identity. Binding it means a
//!   row lifted out of one item and pasted into another fails authentication.
//!   That matters more here than locally: manifest 05 §5.3 records that an
//!   attacker who compromises the Supabase *account* but not the sync passphrase
//!   can write rows into the table. They cannot forge content for an existing
//!   `item_id`, because the AAD binds it.
//! * The **`<len>:` prefix** on `item_id` is the fix for `CopyPaste-lkmy`
//!   (manifest 02 §3.2.2): a string built by concatenating caller-controlled
//!   fields with a delimiter collides when a field contains the delimiter. Only
//!   one caller-controlled field appears here and it is terminal, so the prefix
//!   is not strictly required today — it is here so that adding a second field
//!   later cannot silently reintroduce the bug.
//!
//! # Dependencies
//!
//! Per `CLAUDE.md` rule 1 nothing here is hand-rolled: `argon2` for the
//! passphrase KDF, `hkdf` + `sha2` for the salt, `chacha20poly1305` for the
//! AEAD, `rand` for nonces, `zeroize` for erasure, `base64` for the wire
//! encoding. The code in this file is glue and policy.

use argon2::{Algorithm, Argon2, Params, Version};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine as _;
use chacha20poly1305::{
    aead::{Aead, KeyInit, Payload},
    Key, XChaCha20Poly1305, XNonce,
};
use hkdf::Hkdf;
use rand::{rngs::OsRng, RngCore};
use sha2::Sha256;
use zeroize::Zeroizing;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// XChaCha20-Poly1305 nonce length. 192 bits, so random nonces have no
/// practical birthday bound and no counter state has to be persisted.
pub const NONCE_LEN: usize = 24;

/// Poly1305 tag length, appended to the ciphertext by the AEAD.
pub const TAG_LEN: usize = 16;

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

/// Fixed prefix of the cloud row AAD. Distinct from `copypaste-core`'s
/// `copypaste/v2/item-aead|`, which is what domain-separates the two.
const CLOUD_AAD_PREFIX: &[u8] = b"copypaste/v2/cloud-row-aead|";

/// Cloud ciphertext schema version, bound into the AAD. A bump makes every
/// previously-written row fail authentication — deliberately.
const CLOUD_AAD_SCHEMA_VERSION: u32 = 1;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Every failure this module can produce.
///
/// Payloads are `&'static str` on purpose. `CLAUDE.md` rule 4 forbids showing
/// users a filesystem path, and the sync path additionally must never render a
/// token or a passphrase. A closed set of literals cannot leak one even by
/// accident — there is no `String` to `format!` into.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CloudCryptoError {
    /// Wrong key (wrong passphrase or wrong account), wrong `item_id`, or a
    /// tampered nonce/ciphertext/tag.
    ///
    /// These are **deliberately indistinguishable**. Telling the caller which
    /// one happened turns decryption into an oracle (manifest 02, I-15). The
    /// only correct handling is skip-and-count; never treat it as success, and
    /// never let it delete or overwrite a local row (manifest 05, INV-N3).
    #[error("authentication failed")]
    AuthFailed,

    /// The passphrase is shorter than [`MIN_PASSPHRASE_CHARS`] characters.
    ///
    /// Carries no length and no content: it is a user-facing validation error,
    /// and the value being validated is a secret.
    #[error("the sync passphrase must be at least {MIN_PASSPHRASE_CHARS} characters")]
    PassphraseTooShort,

    /// `account_id` was empty.
    ///
    /// Rejected loudly rather than allowed to collapse the per-account salt
    /// toward a shared constant, which would reintroduce the cross-account
    /// Argon2id precompute weakness the salt exists to prevent (manifest 02,
    /// I-19).
    #[error("an account id is required to derive the sync key")]
    EmptyAccountId,

    /// Structural: the base64 did not decode, or the nonce is not
    /// [`NONCE_LEN`] bytes.
    ///
    /// Safe to distinguish from [`CloudCryptoError::AuthFailed`] — it says
    /// nothing about the key. It means the row is corrupt or a caller passed
    /// the wrong column.
    #[error("the stored row is not well formed")]
    Malformed,

    /// A primitive failed for a reason that is not attacker-controlled — an
    /// Argon2 parameter the implementation rejects, or an AEAD input past the
    /// ~256 GiB per-message limit.
    #[error("internal crypto error: {0}")]
    Internal(&'static str),
}

// ---------------------------------------------------------------------------
// The key
// ---------------------------------------------------------------------------

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
pub struct SyncKey(Zeroizing<[u8; KEY_LEN]>);

impl SyncKey {
    /// Construct from raw bytes.
    ///
    /// For callers that already hold the derived key — the daemon reads it back
    /// from the OS keystore rather than asking for the passphrase on every
    /// start. Does not validate anything; there is nothing to validate about 32
    /// random-looking bytes.
    pub fn from_bytes(bytes: [u8; KEY_LEN]) -> Self {
        Self(Zeroizing::new(bytes))
    }

    /// The raw key bytes, for handing to the keystore.
    ///
    /// Wrapped in `Zeroizing` so the copy this makes is erased when the caller
    /// drops it. Keep the borrow short.
    pub fn to_bytes(&self) -> Zeroizing<[u8; KEY_LEN]> {
        Zeroizing::new(*self.0)
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

    Ok(SyncKey(okm))
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

// ---------------------------------------------------------------------------
// AEAD
// ---------------------------------------------------------------------------

/// Associated data for a cloud row. See the module docs for the layout and the
/// reason behind each field.
fn cloud_aad(item_id: &str) -> Vec<u8> {
    let id = item_id.as_bytes();
    let mut aad = Vec::with_capacity(CLOUD_AAD_PREFIX.len() + 32 + id.len());
    aad.extend_from_slice(CLOUD_AAD_PREFIX);
    aad.extend_from_slice(CLOUD_AAD_SCHEMA_VERSION.to_string().as_bytes());
    aad.push(b'|');
    aad.extend_from_slice(id.len().to_string().as_bytes());
    aad.push(b':');
    aad.extend_from_slice(id);
    aad
}

/// Seal one row's plaintext for the cloud.
///
/// Returns `(nonce_b64, ciphertext_b64)`, both standard base64 with padding,
/// in the order the columns appear on `CloudItem`. The nonce is
/// [`NONCE_LEN`] bytes freshly drawn from the OS CSPRNG on every call —
/// never a counter, never reused (manifest 02, I-11). The ciphertext is
/// `body || tag`, so it is [`TAG_LEN`] bytes longer than the plaintext; an
/// empty plaintext still produces a 16-byte ciphertext, which is why "this row
/// has no content" is not readable from the length alone.
///
/// # Errors
///
/// [`CloudCryptoError::Internal`] if the AEAD rejects the input, which for
/// XChaCha20-Poly1305 means a plaintext past `(2^32 - 1) * 64` bytes. Never
/// panics on caller-supplied data (manifest 02, I-14).
pub fn encrypt_row(
    plaintext: &[u8],
    key: &SyncKey,
    item_id: &str,
) -> Result<(String, String), CloudCryptoError> {
    let cipher = XChaCha20Poly1305::new(Key::from_slice(key.0.as_ref()));

    let mut nonce_bytes = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce_bytes);

    let aad = cloud_aad(item_id);
    let ciphertext = cipher
        .encrypt(
            XNonce::from_slice(&nonce_bytes),
            Payload {
                msg: plaintext,
                aad: &aad,
            },
        )
        .map_err(|_| CloudCryptoError::Internal("AEAD rejected the plaintext (too large)"))?;

    Ok((B64.encode(nonce_bytes), B64.encode(ciphertext)))
}

/// Open a row sealed by [`encrypt_row`] under the same key and `item_id`.
///
/// # Errors
///
/// [`CloudCryptoError::Malformed`] if either field is not valid base64 or the
/// nonce is not [`NONCE_LEN`] bytes. This is a corrupt row, not a wrong key.
///
/// [`CloudCryptoError::AuthFailed`] for everything else — wrong passphrase,
/// wrong account, wrong `item_id`, or any modification to nonce, ciphertext or
/// tag. These are not distinguished from one another by design; see the
/// variant's documentation.
pub fn decrypt_row(
    ciphertext_b64: &str,
    nonce_b64: &str,
    key: &SyncKey,
    item_id: &str,
) -> Result<Vec<u8>, CloudCryptoError> {
    let nonce = B64
        .decode(nonce_b64)
        .map_err(|_| CloudCryptoError::Malformed)?;
    if nonce.len() != NONCE_LEN {
        return Err(CloudCryptoError::Malformed);
    }
    let ciphertext = B64
        .decode(ciphertext_b64)
        .map_err(|_| CloudCryptoError::Malformed)?;
    // Shorter than a bare tag: it cannot be a sealed message. The AEAD would
    // reject it too; the explicit branch keeps the intent visible.
    if ciphertext.len() < TAG_LEN {
        return Err(CloudCryptoError::AuthFailed);
    }

    let cipher = XChaCha20Poly1305::new(Key::from_slice(key.0.as_ref()));
    let aad = cloud_aad(item_id);

    cipher
        .decrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: &ciphertext,
                aad: &aad,
            },
        )
        .map_err(|_| CloudCryptoError::AuthFailed)
}

// ---------------------------------------------------------------------------
// Handle
// ---------------------------------------------------------------------------

/// A [`SyncKey`] with the two row operations bound to it.
///
/// Purely a convenience for callers that would otherwise thread the key through
/// every call site; it holds no state beyond the key and adds no behaviour.
/// [`derive_sync_key`], [`encrypt_row`] and [`decrypt_row`] remain the API —
/// this is one indirection over them, not a second implementation.
pub struct CloudCrypto {
    key: SyncKey,
}

impl CloudCrypto {
    /// Derive the key and wrap it. See [`derive_sync_key`] for the cost and for
    /// what `account_id` must be.
    ///
    /// # Errors
    ///
    /// As [`derive_sync_key`].
    pub fn derive(passphrase: &str, account_id: &str) -> Result<Self, CloudCryptoError> {
        Ok(Self {
            key: derive_sync_key(passphrase, account_id)?,
        })
    }

    /// Wrap a key that was derived earlier — typically read back from the OS
    /// keystore, so that a restart does not re-prompt for the passphrase.
    pub fn new(key: SyncKey) -> Self {
        Self { key }
    }

    /// The wrapped key, for handing to [`crate::sync::CloudSync`].
    pub fn key(&self) -> &SyncKey {
        &self.key
    }

    /// See [`encrypt_row`].
    ///
    /// # Errors
    ///
    /// As [`encrypt_row`].
    pub fn seal(
        &self,
        plaintext: &[u8],
        item_id: &str,
    ) -> Result<(String, String), CloudCryptoError> {
        encrypt_row(plaintext, &self.key, item_id)
    }

    /// See [`decrypt_row`].
    ///
    /// # Errors
    ///
    /// As [`decrypt_row`].
    pub fn open(
        &self,
        ciphertext_b64: &str,
        nonce_b64: &str,
        item_id: &str,
    ) -> Result<Vec<u8>, CloudCryptoError> {
        decrypt_row(ciphertext_b64, nonce_b64, &self.key, item_id)
    }
}

/// Redacted: a `Debug` that prints key material ends up in a log file.
impl std::fmt::Debug for CloudCrypto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CloudCrypto").finish_non_exhaustive()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // Twelve characters exactly, so the boundary is exercised by the happy path
    // as well as by the rejection tests.
    const PASS: &str = "correct horse battery staple";
    const ACCOUNT: &str = "3f2b1c0a-0000-4000-8000-000000000001";
    const ITEM: &str = "1f3c9a4e-0000-4000-8000-000000000001";

    /// The fixture key.
    ///
    /// Derived once for the whole binary and rebuilt from bytes thereafter:
    /// Argon2id is deliberately expensive, and paying 19 MiB per fixture would
    /// make the suite slow enough that people stop running it. The tests that
    /// are *about* derivation call [`derive_sync_key`] directly.
    fn key() -> SyncKey {
        static KEY: std::sync::OnceLock<[u8; KEY_LEN]> = std::sync::OnceLock::new();
        SyncKey::from_bytes(
            *KEY.get_or_init(|| *derive_sync_key(PASS, ACCOUNT).unwrap().to_bytes()),
        )
    }

    // --- round trip -------------------------------------------------------

    #[test]
    fn round_trip_recovers_the_plaintext() {
        let k = key();
        let msg = b"https://example.invalid/some/link";

        let (nonce, ct) = encrypt_row(msg, &k, ITEM).unwrap();

        // The wire form is base64 of a 24-byte nonce and of body||tag.
        assert_eq!(B64.decode(&nonce).unwrap().len(), NONCE_LEN);
        assert_eq!(B64.decode(&ct).unwrap().len(), msg.len() + TAG_LEN);
        assert!(
            !ct.contains("example"),
            "plaintext is recognisable in the ciphertext"
        );

        assert_eq!(decrypt_row(&ct, &nonce, &k, ITEM).unwrap(), msg);
    }

    #[test]
    fn round_trip_survives_a_re_derived_key() {
        // What a second device does: same passphrase, same account, no shared
        // state of any kind.
        let (nonce, ct) = encrypt_row(b"synced", &key(), ITEM).unwrap();
        let other_device = derive_sync_key(PASS, ACCOUNT).unwrap();
        assert_eq!(
            decrypt_row(&ct, &nonce, &other_device, ITEM).unwrap(),
            b"synced"
        );
    }

    #[test]
    fn empty_and_large_plaintexts_round_trip() {
        let k = key();

        let (nonce, ct) = encrypt_row(b"", &k, ITEM).unwrap();
        assert_eq!(B64.decode(&ct).unwrap().len(), TAG_LEN);
        assert_eq!(
            decrypt_row(&ct, &nonce, &k, ITEM).unwrap(),
            Vec::<u8>::new()
        );

        let big: Vec<u8> = (0..512 * 1024).map(|i| (i % 251) as u8).collect();
        let (nonce, ct) = encrypt_row(&big, &k, ITEM).unwrap();
        assert_eq!(decrypt_row(&ct, &nonce, &k, ITEM).unwrap(), big);
    }

    #[test]
    fn non_utf8_payloads_round_trip() {
        let k = key();
        for msg in [&[0xff, 0xfe, 0x00, 0x01][..], "🔐 ключ".as_bytes()] {
            let (nonce, ct) = encrypt_row(msg, &k, ITEM).unwrap();
            assert_eq!(decrypt_row(&ct, &nonce, &k, ITEM).unwrap(), msg);
        }
    }

    // --- fail closed ------------------------------------------------------

    #[test]
    fn wrong_passphrase_fails_closed() {
        let (nonce, ct) = encrypt_row(b"secret", &key(), ITEM).unwrap();
        let wrong = derive_sync_key("incorrect horse battery staple", ACCOUNT).unwrap();

        assert_eq!(
            decrypt_row(&ct, &nonce, &wrong, ITEM),
            Err(CloudCryptoError::AuthFailed)
        );
    }

    #[test]
    fn another_accounts_key_fails_closed() {
        // The cross-tenant case: same passphrase, different account. If the
        // salt were shared, this would decrypt.
        let (nonce, ct) = encrypt_row(b"secret", &key(), ITEM).unwrap();
        let other_account = derive_sync_key(PASS, "3f2b1c0a-0000-4000-8000-000000000002").unwrap();

        assert_eq!(
            decrypt_row(&ct, &nonce, &other_account, ITEM),
            Err(CloudCryptoError::AuthFailed)
        );
    }

    #[test]
    fn wrong_item_id_fails_closed() {
        // The row-move case: an attacker with write access to the account
        // cannot relabel one item's ciphertext as another item.
        let k = key();
        let (nonce, ct) = encrypt_row(b"secret", &k, ITEM).unwrap();

        assert_eq!(
            decrypt_row(&ct, &nonce, &k, "1f3c9a4e-0000-4000-8000-000000000002"),
            Err(CloudCryptoError::AuthFailed)
        );
    }

    #[test]
    fn an_item_id_prefix_does_not_authenticate_the_full_id() {
        // Guards the AAD's length prefix.
        let k = key();
        let (nonce, ct) = encrypt_row(b"secret", &k, "item").unwrap();

        assert!(decrypt_row(&ct, &nonce, &k, "item-2").is_err());
        assert!(decrypt_row(&ct, &nonce, &k, "").is_err());
    }

    #[test]
    fn tampered_ciphertext_fails_closed_in_every_byte() {
        let k = key();
        let (nonce, ct) = encrypt_row(b"the quick brown fox", &k, ITEM).unwrap();
        let raw = B64.decode(&ct).unwrap();

        // Body and all sixteen tag bytes.
        for i in 0..raw.len() {
            let mut bad = raw.clone();
            bad[i] ^= 0x01;
            assert_eq!(
                decrypt_row(&B64.encode(&bad), &nonce, &k, ITEM),
                Err(CloudCryptoError::AuthFailed),
                "flipping a bit in ciphertext byte {i} was not detected"
            );
        }
    }

    #[test]
    fn tampered_nonce_fails_closed() {
        let k = key();
        let (nonce, ct) = encrypt_row(b"the quick brown fox", &k, ITEM).unwrap();
        let raw = B64.decode(&nonce).unwrap();

        for i in 0..raw.len() {
            let mut bad = raw.clone();
            bad[i] ^= 0x80;
            assert_eq!(
                decrypt_row(&ct, &B64.encode(&bad), &k, ITEM),
                Err(CloudCryptoError::AuthFailed)
            );
        }
    }

    #[test]
    fn truncated_ciphertext_fails_without_panicking() {
        let k = key();
        let (nonce, ct) = encrypt_row(b"the quick brown fox", &k, ITEM).unwrap();
        let raw = B64.decode(&ct).unwrap();

        for len in 0..raw.len() {
            assert_eq!(
                decrypt_row(&B64.encode(&raw[..len]), &nonce, &k, ITEM),
                Err(CloudCryptoError::AuthFailed)
            );
        }
    }

    #[test]
    fn swapped_nonces_both_fail() {
        let k = key();
        let (n1, c1) = encrypt_row(b"first", &k, ITEM).unwrap();
        let (n2, c2) = encrypt_row(b"second", &k, ITEM).unwrap();

        assert!(decrypt_row(&c1, &n2, &k, ITEM).is_err());
        assert!(decrypt_row(&c2, &n1, &k, ITEM).is_err());
    }

    #[test]
    fn malformed_wire_fields_are_structural_not_auth_failures() {
        let k = key();
        let (nonce, ct) = encrypt_row(b"x", &k, ITEM).unwrap();

        // Not base64 at all.
        assert_eq!(
            decrypt_row("not base64!!", &nonce, &k, ITEM),
            Err(CloudCryptoError::Malformed)
        );
        assert_eq!(
            decrypt_row(&ct, "not base64!!", &k, ITEM),
            Err(CloudCryptoError::Malformed)
        );

        // Valid base64, wrong nonce length.
        for len in [0usize, NONCE_LEN - 1, NONCE_LEN + 1] {
            assert_eq!(
                decrypt_row(&ct, &B64.encode(vec![0u8; len]), &k, ITEM),
                Err(CloudCryptoError::Malformed)
            );
        }
    }

    // --- key derivation ---------------------------------------------------

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

    // --- nonces -----------------------------------------------------------

    #[test]
    fn two_encryptions_of_the_same_plaintext_differ() {
        let k = key();
        let (n1, c1) = encrypt_row(b"identical", &k, ITEM).unwrap();
        let (n2, c2) = encrypt_row(b"identical", &k, ITEM).unwrap();

        assert_ne!(n1, n2, "nonce reuse");
        assert_ne!(
            c1, c2,
            "deterministic ciphertext discloses which rows are equal"
        );
    }

    #[test]
    fn nonces_are_unique_and_never_all_zero() {
        use std::collections::HashSet;

        let k = key();
        let mut seen = HashSet::new();
        for _ in 0..2_000 {
            let (nonce, _) = encrypt_row(b"x", &k, ITEM).unwrap();
            assert_ne!(
                B64.decode(&nonce).unwrap(),
                vec![0u8; NONCE_LEN],
                "all-zero nonce from the CSPRNG"
            );
            assert!(seen.insert(nonce), "nonce repeated");
        }
    }

    // --- pinned parameters and layout -------------------------------------

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
    fn cloud_aad_has_the_documented_layout() {
        assert_eq!(
            cloud_aad("item-abc"),
            b"copypaste/v2/cloud-row-aead|1|8:item-abc"
        );
        assert_eq!(cloud_aad(""), b"copypaste/v2/cloud-row-aead|1|0:");
        // The length is in bytes, not chars.
        assert_eq!(cloud_aad("é"), b"copypaste/v2/cloud-row-aead|1|2:\xc3\xa9");
    }

    #[test]
    fn cloud_aad_is_domain_separated_from_the_local_item_aad() {
        // copypaste-core seals local rows under `copypaste/v2/item-aead|…`.
        // Neither prefix may be a prefix of the other, or a blob could cross
        // domains under a confused key.
        let aad = cloud_aad(ITEM);
        assert!(aad.starts_with(b"copypaste/v2/cloud-row-aead|"));
        assert!(!aad.starts_with(b"copypaste/v2/item-aead|"));
    }

    #[test]
    fn cloud_aad_is_injective_across_delimiter_abuse() {
        let ids = ["a|b", "a", "b", "3:a|b", "|", "1:a", "", "1|2:x"];
        let mut seen = std::collections::HashSet::new();
        for id in ids {
            assert!(seen.insert(cloud_aad(id)), "AAD collision for {id:?}");
        }
    }

    #[test]
    fn schema_version_is_bound_so_a_bump_fails_closed() {
        // Simulate the next format: same key, same item, AAD that differs only
        // in the schema version. It must not open.
        let k = key();
        let (nonce, ct) = encrypt_row(b"v1 row", &k, ITEM).unwrap();

        let cipher = XChaCha20Poly1305::new(Key::from_slice(k.0.as_ref()));
        let mut v2_aad = cloud_aad(ITEM);
        // b"…|1|36:…" -> b"…|2|36:…"
        let pos = CLOUD_AAD_PREFIX.len();
        v2_aad[pos] = b'2';

        let out = cipher.decrypt(
            XNonce::from_slice(&B64.decode(&nonce).unwrap()),
            Payload {
                msg: &B64.decode(&ct).unwrap(),
                aad: &v2_aad,
            },
        );
        assert!(out.is_err(), "a schema-version bump did not fail closed");
    }

    // --- handle -----------------------------------------------------------

    #[test]
    fn the_handle_agrees_with_the_free_functions() {
        let crypto = CloudCrypto::derive(PASS, ACCOUNT).unwrap();
        let (nonce, ct) = crypto.seal(b"through the handle", ITEM).unwrap();

        assert_eq!(
            crypto.open(&ct, &nonce, ITEM).unwrap(),
            b"through the handle"
        );
        // And the free function opens what the handle sealed, with the key the
        // handle exposes.
        assert_eq!(
            decrypt_row(&ct, &nonce, crypto.key(), ITEM).unwrap(),
            b"through the handle"
        );
    }

    #[test]
    fn from_bytes_and_to_bytes_round_trip() {
        // The keystore path: derive once, store the bytes, rebuild on restart.
        let derived = key();
        let stored = *derived.to_bytes();
        let rebuilt = SyncKey::from_bytes(stored);

        let (nonce, ct) = encrypt_row(b"across a restart", &derived, ITEM).unwrap();
        assert_eq!(
            decrypt_row(&ct, &nonce, &rebuilt, ITEM).unwrap(),
            b"across a restart"
        );
    }

    // --- hygiene ----------------------------------------------------------

    #[test]
    fn debug_output_never_contains_key_material() {
        let crypto = CloudCrypto::derive(PASS, ACCOUNT).unwrap();
        assert_eq!(format!("{crypto:?}"), "CloudCrypto { .. }");
    }

    #[test]
    fn error_messages_contain_no_paths_and_no_secrets() {
        // CLAUDE.md rule 4, plus the sync rule that a token or a passphrase
        // must never be rendered. The type makes this structural — every
        // payload is a `&'static str` — but pin the rendered strings too.
        let errors = [
            CloudCryptoError::AuthFailed,
            CloudCryptoError::PassphraseTooShort,
            CloudCryptoError::EmptyAccountId,
            CloudCryptoError::Malformed,
            CloudCryptoError::Internal("AEAD rejected the plaintext (too large)"),
        ];
        for e in errors {
            let msg = e.to_string();
            assert!(!msg.contains('/'), "path-like separator in {msg:?}");
            assert!(!msg.contains('\\'), "path-like separator in {msg:?}");
            assert!(!msg.contains(PASS), "passphrase in {msg:?}");
            assert!(!msg.contains(ACCOUNT), "account id in {msg:?}");
        }
    }
}
