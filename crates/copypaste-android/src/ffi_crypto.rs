//! Local XChaCha20-Poly1305 encrypt/decrypt FFI exports.
//!
//! Covers: `encrypt_text`, `decrypt_text`, `decrypt_text_batch`, and all
//! supporting types (`EncryptedBlob`, `EncryptedItem`, `DecryptedItem`,
//! `DecryptBatchResult`). These are the per-item crypto primitives used by
//! Kotlin for at-rest encryption of clipboard content in the local SQLite store.

use copypaste_core::{
    build_item_aad, build_item_aad_v2, decrypt_item_with_aad, encrypt_item_with_aad, ItemId,
    AAD_SCHEMA_VERSION, AAD_SCHEMA_VERSION_V4, NONCE_SIZE,
};
use zeroize::Zeroizing;

use crate::{panic_boundary, CopypasteError};

pub struct EncryptedBlob {
    pub nonce: Vec<u8>,
    pub ciphertext: Vec<u8>,
}

/// Encrypt `bytes` with `key` (XChaCha20-Poly1305), binding `item_id` and
/// `key_version` into the AEAD AAD.
///
/// | `key_version` | AAD format                           |
/// |---------------|--------------------------------------|
/// | 1             | `build_item_aad(item_id, 3)`         |
/// | 2             | `build_item_aad_v2(item_id, 4, 2)`   |
/// | other         | `Err(EncryptionFailed)`              |
///
/// Kotlin callers MUST persist `key_version` alongside the ciphertext and pass
/// it back to `decrypt_text` verbatim — a mismatch will fail decryption.
/// New items should always use `key_version = 2` (matches the daemon's
/// `ITEM_KEY_VERSION_CURRENT`). Legacy stored items encrypted with v1 must
/// continue to round-trip with `key_version = 1`.
pub fn encrypt_text(
    item_id: String,
    bytes: &[u8],
    key: &[u8],
    key_version: u8,
) -> Result<EncryptedBlob, CopypasteError> {
    panic_boundary::catch_result(|| {
        let key_arr: Zeroizing<[u8; 32]> = Zeroizing::new(
            key.try_into()
                .map_err(|_| CopypasteError::InvalidKeyLength)?,
        );
        // Mirror the dispatch table in decrypt_item_by_version (copypaste-core).
        let aad = match key_version {
            1 => build_item_aad(&ItemId::from(item_id.as_str()), AAD_SCHEMA_VERSION),
            2 => build_item_aad_v2(&ItemId::from(item_id.as_str()), AAD_SCHEMA_VERSION_V4, u32::from(key_version)),
            _ => return Err(CopypasteError::EncryptionFailed),
        };
        let (nonce, ciphertext) = encrypt_item_with_aad(bytes, &key_arr, &aad)
            .map_err(|_| CopypasteError::EncryptionFailed)?;
        Ok(EncryptedBlob {
            nonce: nonce.to_vec(),
            ciphertext,
        })
    })
}

/// Decrypt `ciphertext` encrypted by `encrypt_text`, dispatching on
/// `key_version` to select the correct AAD format.
///
/// | `key_version` | AAD format                           |
/// |---------------|--------------------------------------|
/// | 1             | `build_item_aad(item_id, 3)`         |
/// | 2             | `build_item_aad_v2(item_id, 4, 2)`   |
/// | other         | `Err(DecryptionFailed)`              |
///
/// `item_id` and `key_version` MUST match the values used during
/// `encrypt_text` — a mismatch will cause an AEAD auth-tag failure.
pub fn decrypt_text(
    item_id: String,
    ciphertext: &[u8],
    nonce: &[u8],
    key: &[u8],
    key_version: u8,
) -> Result<Vec<u8>, CopypasteError> {
    panic_boundary::catch_result(|| {
        let key_arr: Zeroizing<[u8; 32]> = Zeroizing::new(
            key.try_into()
                .map_err(|_| CopypasteError::InvalidKeyLength)?,
        );
        let nonce_arr: [u8; NONCE_SIZE] =
            nonce
                .try_into()
                .map_err(|_| CopypasteError::DecryptionFailed {
                    reason: "wrong nonce length".into(),
                })?;
        // Mirror the dispatch table in decrypt_item_by_version (copypaste-core).
        let aad = match key_version {
            1 => build_item_aad(&ItemId::from(item_id.as_str()), AAD_SCHEMA_VERSION),
            2 => build_item_aad_v2(&ItemId::from(item_id.as_str()), AAD_SCHEMA_VERSION_V4, u32::from(key_version)),
            v => {
                return Err(CopypasteError::DecryptionFailed {
                    reason: format!("unknown key_version: {v}"),
                })
            }
        };
        decrypt_item_with_aad(ciphertext, &nonce_arr, &key_arr, &aad).map_err(|e| {
            CopypasteError::DecryptionFailed {
                reason: e.to_string(),
            }
        })
    })
}

/// One encrypted local clipboard item handed to [`decrypt_text_batch`].
///
/// Mirrors the at-rest columns Kotlin reads from its local SQLite store:
/// the stable `item_id` (bound into the AEAD AAD), the `ciphertext` + `nonce`
/// blobs, and the `key_version` (1 or 2) that selects the AAD/key format.
#[derive(Debug)]
pub struct EncryptedItem {
    pub item_id: String,
    pub ciphertext: Vec<u8>,
    pub nonce: Vec<u8>,
    pub key_version: u8,
}

/// One successfully-decrypted item returned by [`decrypt_text_batch`], carrying
/// its `item_id` back so Kotlin can re-associate the plaintext with its row.
#[derive(Debug)]
pub struct DecryptedItem {
    pub item_id: String,
    pub plaintext: Vec<u8>,
}

/// Outcome of [`decrypt_text_batch`]: the decryptable items plus an aggregate
/// count of the rows skipped because they could not be decrypted.
#[derive(Debug)]
pub struct DecryptBatchResult {
    /// Items whose AEAD auth tag verified and decrypted cleanly.
    pub items: Vec<DecryptedItem>,
    /// Number of input items skipped because they failed to decrypt (wrong /
    /// rotated key, format drift, an unsupported `key_version`, or a malformed
    /// nonce). Kotlin logs this ONCE in aggregate instead of one error per row.
    pub skipped: u32,
}

/// Decrypt a batch of local clipboard items at startup/list time, **degrading
/// gracefully** when individual items cannot be decrypted (CopyPaste-00zz).
///
/// # Why this exists
///
/// Kotlin's startup load previously called [`decrypt_text`] once per row, and
/// every undecryptable legacy item (encrypted under a now-rotated key/format)
/// threw `DecryptionFailed`. After a key rotation / re-pair this fired hundreds
/// of times on a single launch (~629 observed) — flooding logcat and degrading
/// UX even though those rows are simply dead legacy ciphertext.
///
/// This batch entry point decrypts every item in one FFI call: each item that
/// fails AEAD verification (or carries an unsupported `key_version` / malformed
/// nonce) is **skipped, not thrown**, and counted in
/// [`DecryptBatchResult::skipped`]. Kotlin surfaces a single aggregate line
/// ("skipped N undecryptable legacy items") and renders only the decryptable
/// items, instead of catching one exception per row.
///
/// # Security
///
/// Graceful means *skip*, never *bypass*. A failed auth tag is never accepted
/// as plaintext — the item is dropped from `items`. The AAD binding of
/// `(item_id, schema_version, key_version)` is preserved verbatim (each item's
/// AAD is rebuilt here exactly as [`decrypt_text`] does), so this path cannot be
/// used to swap or replay ciphertext across items. `key` is zeroized on drop via
/// [`Zeroizing`].
///
/// Errors: `InvalidKeyLength` if `key` is not exactly 32 bytes. Per-item
/// decryption failures do NOT error — they are skipped and counted.
pub fn decrypt_text_batch(
    items: Vec<EncryptedItem>,
    key: &[u8],
) -> Result<DecryptBatchResult, CopypasteError> {
    panic_boundary::catch_result(|| {
        let key_arr: Zeroizing<[u8; 32]> = Zeroizing::new(
            key.try_into()
                .map_err(|_| CopypasteError::InvalidKeyLength)?,
        );
        let mut decrypted = Vec::with_capacity(items.len());
        let mut skipped: u32 = 0;
        for item in &items {
            match try_decrypt_one(item, &key_arr) {
                Some(plaintext) => decrypted.push(DecryptedItem {
                    item_id: item.item_id.clone(),
                    plaintext,
                }),
                // Skip-and-count: a wrong/rotated key, format drift, malformed
                // nonce, or unsupported key_version is NOT surfaced as an error.
                None => skipped = skipped.saturating_add(1),
            }
        }
        Ok(DecryptBatchResult {
            items: decrypted,
            skipped,
        })
    })
}

/// Attempt to decrypt a single [`EncryptedItem`], returning `None` (rather than
/// erroring) on any failure so [`decrypt_text_batch`] can skip-and-count.
///
/// Rebuilds the AAD from the item's own `item_id` + `key_version` exactly as
/// [`decrypt_text`] does, keeping the AAD binding intact. The only `Some` path
/// is a fully-verified AEAD decrypt — a failed auth tag yields `None`, never
/// accepted plaintext.
pub(crate) fn try_decrypt_one(item: &EncryptedItem, key: &[u8; 32]) -> Option<Vec<u8>> {
    let nonce: [u8; NONCE_SIZE] = item.nonce.as_slice().try_into().ok()?;
    let aad = match item.key_version {
        1 => build_item_aad(&ItemId::from(item.item_id.as_str()), AAD_SCHEMA_VERSION),
        2 => build_item_aad_v2(
            &ItemId::from(item.item_id.as_str()),
            AAD_SCHEMA_VERSION_V4,
            u32::from(item.key_version),
        ),
        // Unknown key_version: undecryptable by definition — skip.
        _ => return None,
    };
    decrypt_item_with_aad(&item.ciphertext, &nonce, key, &aad).ok()
}
