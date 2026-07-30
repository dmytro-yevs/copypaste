//! The item envelope: XChaCha20-Poly1305 with the item id bound into the AAD.
//!
//! One seal path and one open path. Every failure that could be attacker
//! influenced collapses into [`CryptoError::AuthFailed`], so there is no oracle
//! to query (port manifest 02, I-15).

use chacha20poly1305::{
    aead::{Aead, KeyInit, Payload},
    Key, XChaCha20Poly1305, XNonce,
};
use rand::{rngs::OsRng, RngCore};

use super::{CryptoError, ItemKey};

/// XChaCha20-Poly1305 nonce length. 192 bits is the whole reason for choosing
/// XChaCha over ChaCha: random nonces have no practical birthday bound, so no
/// counter state has to be persisted anywhere (ADR-001).
pub const NONCE_LEN: usize = 24;

/// Poly1305 authentication tag length, appended to the ciphertext by the AEAD.
pub const TAG_LEN: usize = 16;

/// Fixed prefix of the item AEAD's associated data.
const AAD_PREFIX: &[u8] = b"copypaste/v2/item-aead|";

/// Associated data for the item AEAD: `copypaste/v2/item-aead|<len>:<item_id>`.
///
/// `item_id` is the cross-device logical item id, not a storage row id. Binding
/// it means a ciphertext copied into a different row fails to authenticate,
/// which is the entire reason per-item AEAD exists on top of an already
/// encrypted SQLCipher file.
///
/// The `<len>:` prefix is not decoration. Port manifest 02 §3.2.2 records
/// **CopyPaste-lkmy**: an info string concatenating two caller-controlled ids
/// with `|` collided when an id contained `|`, deriving identical keys for
/// different inputs. One terminal field does not need it today; it is here so
/// that adding a second later cannot silently reintroduce the bug.
fn item_aad(item_id: &str) -> Vec<u8> {
    let id = item_id.as_bytes();
    let mut aad = Vec::with_capacity(AAD_PREFIX.len() + 24 + id.len());
    aad.extend_from_slice(AAD_PREFIX);
    aad.extend_from_slice(id.len().to_string().as_bytes());
    aad.push(b':');
    aad.extend_from_slice(id);
    aad
}

/// Seal `plaintext` under `key`, binding `item_id` into the AAD.
///
/// Returns `(nonce, ciphertext)`. The nonce is [`NONCE_LEN`] bytes, freshly
/// drawn from the OS CSPRNG every call; the ciphertext is
/// `body || poly1305_tag`, so an empty plaintext yields a 16-byte ciphertext,
/// not an empty one. Both are opaque; store them as written.
///
/// # Errors
///
/// [`CryptoError::Internal`] if the AEAD rejects the input, which for
/// XChaCha20-Poly1305 means a plaintext past `(2^32 - 1) * 64` bytes. Never
/// panics on caller input (port manifest 02, I-14).
pub fn encrypt(
    plaintext: &[u8],
    key: &ItemKey,
    item_id: &str,
) -> Result<(Vec<u8>, Vec<u8>), CryptoError> {
    let cipher = XChaCha20Poly1305::new(Key::from_slice(key.0.as_ref()));

    let mut nonce_bytes = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = XNonce::from_slice(&nonce_bytes);

    let aad = item_aad(item_id);
    let ciphertext = cipher
        .encrypt(
            nonce,
            Payload {
                msg: plaintext,
                aad: &aad,
            },
        )
        .map_err(|_| CryptoError::Internal("AEAD rejected the plaintext (too large)"))?;

    Ok((nonce_bytes.to_vec(), ciphertext))
}

/// Open a ciphertext produced by [`encrypt`] with the same key and `item_id`.
///
/// # Errors
///
/// [`CryptoError::InvalidNonce`] if `nonce` is not [`NONCE_LEN`] bytes.
///
/// [`CryptoError::AuthFailed`] for everything else — wrong key, wrong
/// `item_id`, or a modified nonce, ciphertext or tag. Not distinguished from
/// each other by design; see the variant.
pub fn decrypt(
    ciphertext: &[u8],
    nonce: &[u8],
    key: &ItemKey,
    item_id: &str,
) -> Result<Vec<u8>, CryptoError> {
    if nonce.len() != NONCE_LEN {
        return Err(CryptoError::InvalidNonce);
    }
    // Shorter than a bare tag: cannot be a valid sealed message. The AEAD
    // returns an error here too; checking first keeps the branch explicit.
    if ciphertext.len() < TAG_LEN {
        return Err(CryptoError::AuthFailed);
    }

    let cipher = XChaCha20Poly1305::new(Key::from_slice(key.0.as_ref()));
    let aad = item_aad(item_id);

    cipher
        .decrypt(
            XNonce::from_slice(nonce),
            Payload {
                msg: ciphertext,
                aad: &aad,
            },
        )
        .map_err(|_| CryptoError::AuthFailed)
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{key_a, ITEM, SECRET_A, SECRET_B};
    use super::super::Keyring;
    use super::*;


    #[test]
    fn round_trip_recovers_the_plaintext() {
        let key = key_a();
        let msg = b"ssh-rsa AAAAB3NzaC1yc2E... not really a key";

        let (nonce, ct) = encrypt(msg, &key, ITEM).unwrap();
        assert_eq!(nonce.len(), NONCE_LEN);
        assert_eq!(ct.len(), msg.len() + TAG_LEN);
        assert_ne!(
            &ct[..msg.len()],
            &msg[..],
            "plaintext appears in ciphertext"
        );

        let out = decrypt(&ct, &nonce, &key, ITEM).unwrap();
        assert_eq!(out, msg);
    }

    #[test]
    fn round_trip_survives_a_rebuilt_key() {
        // A restarted daemon re-derives the key from the same device secret.
        let (nonce, ct) = encrypt(b"persisted", &key_a(), ITEM).unwrap();
        let reloaded = Keyring::from_secret(&SECRET_A).item_key();
        assert_eq!(decrypt(&ct, &nonce, &reloaded, ITEM).unwrap(), b"persisted");
    }


    #[test]
    fn wrong_key_fails() {
        let (nonce, ct) = encrypt(b"secret", &key_a(), ITEM).unwrap();
        let other = Keyring::from_secret(&SECRET_B).item_key();

        assert!(matches!(
            decrypt(&ct, &nonce, &other, ITEM),
            Err(CryptoError::AuthFailed)
        ));
    }

    #[test]
    fn wrong_item_id_in_aad_fails() {
        // The row-swap case: same key, same bytes, different row.
        let key = key_a();
        let (nonce, ct) = encrypt(b"secret", &key, ITEM).unwrap();

        assert!(matches!(
            decrypt(&ct, &nonce, &key, "1f3c9a4e-0000-4000-8000-000000000002"),
            Err(CryptoError::AuthFailed)
        ));
    }

    #[test]
    fn item_id_prefix_is_not_accepted_for_the_full_id() {
        // Guards the AAD's length prefix: "item" must not authenticate a
        // ciphertext bound to "item-2", however the framing is concatenated.
        let key = key_a();
        let (nonce, ct) = encrypt(b"secret", &key, "item").unwrap();

        assert!(decrypt(&ct, &nonce, &key, "item-2").is_err());
        assert!(decrypt(&ct, &nonce, &key, "").is_err());
    }

    #[test]
    fn empty_item_id_is_bound_too() {
        let key = key_a();
        let (nonce, ct) = encrypt(b"secret", &key, "").unwrap();
        assert_eq!(decrypt(&ct, &nonce, &key, "").unwrap(), b"secret");
        assert!(decrypt(&ct, &nonce, &key, ITEM).is_err());
    }

    #[test]
    fn tampered_ciphertext_fails() {
        let key = key_a();
        let (nonce, ct) = encrypt(b"the quick brown fox", &key, ITEM).unwrap();

        // Every byte matters: body and all 16 tag bytes.
        for i in 0..ct.len() {
            let mut bad = ct.clone();
            bad[i] ^= 0x01;
            assert!(
                matches!(
                    decrypt(&bad, &nonce, &key, ITEM),
                    Err(CryptoError::AuthFailed)
                ),
                "flipping a bit in ciphertext byte {i} was not detected"
            );
        }
    }

    #[test]
    fn tampered_nonce_fails() {
        let key = key_a();
        let (nonce, ct) = encrypt(b"the quick brown fox", &key, ITEM).unwrap();

        for i in 0..nonce.len() {
            let mut bad = nonce.clone();
            bad[i] ^= 0x80;
            assert!(matches!(
                decrypt(&ct, &bad, &key, ITEM),
                Err(CryptoError::AuthFailed)
            ));
        }
    }

    #[test]
    fn truncated_ciphertext_fails_without_panicking() {
        let key = key_a();
        let (nonce, ct) = encrypt(b"the quick brown fox", &key, ITEM).unwrap();

        for len in 0..ct.len() {
            assert!(matches!(
                decrypt(&ct[..len], &nonce, &key, ITEM),
                Err(CryptoError::AuthFailed)
            ));
        }
    }

    #[test]
    fn swapped_nonces_both_fail() {
        let key = key_a();
        let (n1, c1) = encrypt(b"first", &key, ITEM).unwrap();
        let (n2, c2) = encrypt(b"second", &key, ITEM).unwrap();

        assert!(decrypt(&c1, &n2, &key, ITEM).is_err());
        assert!(decrypt(&c2, &n1, &key, ITEM).is_err());
    }

    #[test]
    fn wrong_nonce_length_is_a_structural_error() {
        let key = key_a();
        let (nonce, ct) = encrypt(b"x", &key, ITEM).unwrap();

        assert!(matches!(
            decrypt(&ct, &nonce[..NONCE_LEN - 1], &key, ITEM),
            Err(CryptoError::InvalidNonce)
        ));
        assert!(matches!(
            decrypt(&ct, &[], &key, ITEM),
            Err(CryptoError::InvalidNonce)
        ));

        let mut long = nonce.clone();
        long.push(0);
        assert!(matches!(
            decrypt(&ct, &long, &key, ITEM),
            Err(CryptoError::InvalidNonce)
        ));
    }


    #[test]
    fn empty_plaintext_round_trips() {
        let key = key_a();
        let (nonce, ct) = encrypt(b"", &key, ITEM).unwrap();

        // An empty plaintext still produces a tag, so the ciphertext is not
        // empty and "no content" is not distinguishable by length alone.
        assert_eq!(ct.len(), TAG_LEN);
        assert_eq!(decrypt(&ct, &nonce, &key, ITEM).unwrap(), Vec::<u8>::new());
    }

    #[test]
    fn large_plaintext_round_trips() {
        // 4 MiB: larger than any realistic clipboard text item and past every
        // internal buffer size in the AEAD.
        let big: Vec<u8> = (0..4 * 1024 * 1024).map(|i| (i % 251) as u8).collect();
        let key = key_a();

        let (nonce, ct) = encrypt(&big, &key, ITEM).unwrap();
        assert_eq!(ct.len(), big.len() + TAG_LEN);
        assert_eq!(decrypt(&ct, &nonce, &key, ITEM).unwrap(), big);
    }

    #[test]
    fn non_utf8_and_unicode_payloads_round_trip() {
        let key = key_a();
        for msg in [&[0xff, 0xfe, 0x00, 0x01][..], "🔐 ключ".as_bytes()] {
            let (nonce, ct) = encrypt(msg, &key, ITEM).unwrap();
            assert_eq!(decrypt(&ct, &nonce, &key, ITEM).unwrap(), msg);
        }
    }


    #[test]
    fn two_encryptions_of_the_same_plaintext_use_different_nonces() {
        let key = key_a();
        let (n1, c1) = encrypt(b"identical", &key, ITEM).unwrap();
        let (n2, c2) = encrypt(b"identical", &key, ITEM).unwrap();

        assert_ne!(n1, n2, "nonce reuse");
        assert_ne!(c1, c2, "deterministic ciphertext leaks equality of items");
    }

    #[test]
    fn nonces_are_unique_and_never_all_zero() {
        use std::collections::HashSet;

        let key = key_a();
        let mut seen = HashSet::new();
        for _ in 0..2_000 {
            let (nonce, _) = encrypt(b"x", &key, ITEM).unwrap();
            assert_ne!(
                nonce,
                vec![0u8; NONCE_LEN],
                "all-zero nonce from the CSPRNG"
            );
            assert!(seen.insert(nonce), "nonce repeated");
        }
    }


    #[test]
    fn item_aad_has_the_documented_layout() {
        assert_eq!(item_aad("item-abc"), b"copypaste/v2/item-aead|8:item-abc");
        assert_eq!(item_aad(""), b"copypaste/v2/item-aead|0:");
        // Length is in bytes, not chars.
        assert_eq!(item_aad("é"), b"copypaste/v2/item-aead|2:\xc3\xa9");
    }

    #[test]
    fn item_aad_is_injective_across_delimiter_abuse() {
        // CopyPaste-lkmy in miniature: ids containing the delimiter must not
        // collide with each other.
        let ids = ["a|b", "a", "b", "3:a|b", "|", "1:a", ""];
        let mut seen = std::collections::HashSet::new();
        for id in ids {
            assert!(seen.insert(item_aad(id)), "AAD collision for {id:?}");
        }
    }
}
