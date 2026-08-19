//! Sealing and opening one row, and the associated data that binds it in
//! place.
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

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine as _;
use chacha20poly1305::{
    aead::{Aead, KeyInit, Payload},
    Key, XChaCha20Poly1305, XNonce,
};
use rand::{rngs::OsRng, RngCore};
use zeroize::Zeroizing;

use super::error::CloudCryptoError;
use super::key::SyncKey;

/// XChaCha20-Poly1305 nonce length. 192 bits, so random nonces have no
/// practical birthday bound and no counter state has to be persisted.
pub const NONCE_LEN: usize = 24;

/// Poly1305 tag length, appended to the ciphertext by the AEAD.
pub const TAG_LEN: usize = 16;

/// Fixed prefix of the cloud row AAD. Distinct from `copypaste-core`'s
/// `copypaste/v2/item-aead|`, which is what domain-separates the two.
const CLOUD_AAD_PREFIX: &[u8] = b"copypaste/v2/cloud-row-aead|";

/// Cloud ciphertext schema version, bound into the AAD. A bump makes every
/// previously-written row fail authentication — deliberately.
const CLOUD_AAD_SCHEMA_VERSION: u32 = 1;

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
    let cipher = XChaCha20Poly1305::new(Key::from_slice(key.material()));

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
) -> Result<Zeroizing<Vec<u8>>, CloudCryptoError> {
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

    let cipher = XChaCha20Poly1305::new(Key::from_slice(key.material()));
    let aad = cloud_aad(item_id);

    cipher
        .decrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: &ciphertext,
                aad: &aad,
            },
        )
        .map(Zeroizing::new)
        .map_err(|_| CloudCryptoError::AuthFailed)
}

// Tests

#[cfg(test)]
mod tests {
    use super::super::key::derive_sync_key;
    use super::super::testkit::{key, ACCOUNT, ITEM, PASS};
    use super::*;

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

        assert_eq!(decrypt_row(&ct, &nonce, &k, ITEM).unwrap().as_slice(), msg);
    }

    #[test]
    fn round_trip_survives_a_re_derived_key() {
        // What a second device does: same passphrase, same account, no shared
        // state of any kind.
        let (nonce, ct) = encrypt_row(b"synced", &key(), ITEM).unwrap();
        let other_device = derive_sync_key(PASS, ACCOUNT).unwrap();
        assert_eq!(
            decrypt_row(&ct, &nonce, &other_device, ITEM)
                .unwrap()
                .as_slice(),
            b"synced"
        );
    }

    #[test]
    fn empty_and_large_plaintexts_round_trip() {
        let k = key();

        let (nonce, ct) = encrypt_row(b"", &k, ITEM).unwrap();
        assert_eq!(B64.decode(&ct).unwrap().len(), TAG_LEN);
        assert_eq!(decrypt_row(&ct, &nonce, &k, ITEM).unwrap().as_slice(), b"");

        let big: Vec<u8> = (0..512 * 1024).map(|i| (i % 251) as u8).collect();
        let (nonce, ct) = encrypt_row(&big, &k, ITEM).unwrap();
        assert_eq!(decrypt_row(&ct, &nonce, &k, ITEM).unwrap().as_slice(), big);
    }

    #[test]
    fn non_utf8_payloads_round_trip() {
        let k = key();
        for msg in [&[0xff, 0xfe, 0x00, 0x01][..], "🔐 ключ".as_bytes()] {
            let (nonce, ct) = encrypt_row(msg, &k, ITEM).unwrap();
            assert_eq!(decrypt_row(&ct, &nonce, &k, ITEM).unwrap().as_slice(), msg);
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

    // --- the AAD layout ---------------------------------------------------

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

        let cipher = XChaCha20Poly1305::new(Key::from_slice(k.material()));
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
}
