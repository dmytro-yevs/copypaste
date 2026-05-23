//! AEAD tamper-detection tests for XChaCha20-Poly1305 (`encrypt_item` / `decrypt_item`).
//!
//! Verifies the authenticated-encryption guarantees of the wrapper in
//! `copypaste-core::crypto::encrypt`:
//!
//! * Unmodified ciphertext round-trips.
//! * Any single-bit flip in the ciphertext body, in the nonce, or in the
//!   16-byte Poly1305 tag (last `TAG_SIZE` bytes of the returned `Vec<u8>`)
//!   makes decryption fail with `EncryptError::AuthFailed` rather than
//!   returning corrupted plaintext.
//! * Truncated ciphertext and wrong-key attempts return an error without
//!   panicking.
//! * Empty plaintext is a valid input that round-trips cleanly.
//! * Cross-decrypting two independently-encrypted messages does NOT yield the
//!   other plaintext — the random per-message nonce binds the ciphertext to
//!   that nonce.
//!
//! AAD observation: the current `encrypt_item` / `decrypt_item` API does NOT
//! accept associated data, so an `aad_mismatch_returns_error` test is not
//! applicable. If AAD support is added later (e.g. binding ciphertext to an
//! item id), a parallel test should be appended here.

use copypaste_core::{decrypt_item, encrypt_item, EncryptError, NONCE_SIZE};

const TAG_SIZE: usize = 16;

fn key_a() -> [u8; 32] {
    [0x11u8; 32]
}

fn key_b() -> [u8; 32] {
    [0x22u8; 32]
}

#[test]
fn decrypt_unmodified_ciphertext_succeeds() {
    let key = key_a();
    let plaintext = b"hello clipboard";
    let (nonce, ciphertext) = encrypt_item(plaintext, &key).expect("encrypt");
    let decrypted = decrypt_item(&ciphertext, &nonce, &key).expect("decrypt");
    assert_eq!(decrypted, plaintext);
}

#[test]
fn bit_flip_in_ciphertext_body_returns_error() {
    let key = key_a();
    let plaintext = b"sensitive payload that is longer than the tag";
    let (nonce, mut ciphertext) = encrypt_item(plaintext, &key).expect("encrypt");

    // Body = everything before the trailing 16-byte tag. Flip a bit in the
    // middle of the body.
    assert!(ciphertext.len() > TAG_SIZE, "ciphertext must contain body + tag");
    let body_idx = (ciphertext.len() - TAG_SIZE) / 2;
    ciphertext[body_idx] ^= 0x01;

    let result = decrypt_item(&ciphertext, &nonce, &key);
    assert!(matches!(result, Err(EncryptError::AuthFailed)),
        "expected AuthFailed for body bit-flip, got {:?}", result);
}

#[test]
fn bit_flip_in_nonce_returns_error() {
    let key = key_a();
    let plaintext = b"nonce-bound message";
    let (mut nonce, ciphertext) = encrypt_item(plaintext, &key).expect("encrypt");

    nonce[0] ^= 0x01;

    let result = decrypt_item(&ciphertext, &nonce, &key);
    assert!(matches!(result, Err(EncryptError::AuthFailed)),
        "expected AuthFailed for nonce bit-flip, got {:?}", result);
}

#[test]
fn bit_flip_in_auth_tag_returns_error() {
    let key = key_a();
    let plaintext = b"tag-protected message";
    let (nonce, mut ciphertext) = encrypt_item(plaintext, &key).expect("encrypt");

    // Last 16 bytes of the AEAD output are the Poly1305 tag.
    let len = ciphertext.len();
    assert!(len >= TAG_SIZE);
    ciphertext[len - 1] ^= 0x80;

    let result = decrypt_item(&ciphertext, &nonce, &key);
    assert!(matches!(result, Err(EncryptError::AuthFailed)),
        "expected AuthFailed for tag bit-flip, got {:?}", result);
}

#[test]
fn truncated_ciphertext_returns_error_not_panic() {
    let key = key_a();
    let plaintext = b"truncate me please";
    let (nonce, mut ciphertext) = encrypt_item(plaintext, &key).expect("encrypt");

    // Drop the last byte — this corrupts the Poly1305 tag length.
    ciphertext.pop();

    let result = decrypt_item(&ciphertext, &nonce, &key);
    assert!(matches!(result, Err(EncryptError::AuthFailed)),
        "truncated ciphertext must return Err, not panic; got {:?}", result);
}

#[test]
fn truncated_below_tag_size_returns_error_not_panic() {
    // Even more aggressive: trim ciphertext down to less than TAG_SIZE bytes.
    // The AEAD layer must reject this cleanly without panicking.
    let key = key_a();
    let (nonce, _ciphertext) = encrypt_item(b"x", &key).expect("encrypt");

    let stub = vec![0u8; TAG_SIZE - 1];
    let result = decrypt_item(&stub, &nonce, &key);
    assert!(matches!(result, Err(EncryptError::AuthFailed)),
        "sub-tag-size ciphertext must return Err; got {:?}", result);
}

#[test]
fn swapped_two_ciphertexts_same_key_decrypts_to_other_plaintext() {
    // Sanity that the random nonce binds each ciphertext: decrypting
    // ciphertext_a with nonce_b (or vice versa) must FAIL — neither
    // ciphertext "decrypts to the other plaintext" under the wrong nonce.
    // This pins the property that nonce-pairing is enforced.
    let key = key_a();
    let pt_a = b"AAAAAAAAAAAAAAAA";
    let pt_b = b"BBBBBBBBBBBBBBBB";

    let (nonce_a, ct_a) = encrypt_item(pt_a, &key).expect("encrypt a");
    let (nonce_b, ct_b) = encrypt_item(pt_b, &key).expect("encrypt b");

    assert_ne!(nonce_a, nonce_b, "fresh nonces must differ");

    // Cross-pair: wrong nonce must NOT yield the other plaintext.
    let cross_1 = decrypt_item(&ct_a, &nonce_b, &key);
    let cross_2 = decrypt_item(&ct_b, &nonce_a, &key);
    assert!(matches!(cross_1, Err(EncryptError::AuthFailed)),
        "ct_a + nonce_b must fail; got {:?}", cross_1);
    assert!(matches!(cross_2, Err(EncryptError::AuthFailed)),
        "ct_b + nonce_a must fail; got {:?}", cross_2);

    // Correct pairing still works.
    assert_eq!(decrypt_item(&ct_a, &nonce_a, &key).unwrap(), pt_a);
    assert_eq!(decrypt_item(&ct_b, &nonce_b, &key).unwrap(), pt_b);
}

#[test]
fn wrong_key_returns_error() {
    let plaintext = b"only key A can read this";
    let (nonce, ciphertext) = encrypt_item(plaintext, &key_a()).expect("encrypt");

    let result = decrypt_item(&ciphertext, &nonce, &key_b());
    assert!(matches!(result, Err(EncryptError::AuthFailed)),
        "expected AuthFailed for wrong key, got {:?}", result);
}

#[test]
fn empty_plaintext_encrypts_decrypts_correctly() {
    let key = key_a();
    let (nonce, ciphertext) = encrypt_item(b"", &key).expect("encrypt empty");

    // Even empty plaintext produces a 16-byte tag.
    assert_eq!(ciphertext.len(), TAG_SIZE,
        "empty plaintext ciphertext must equal tag size");
    assert_eq!(nonce.len(), NONCE_SIZE);

    let decrypted = decrypt_item(&ciphertext, &nonce, &key).expect("decrypt empty");
    assert!(decrypted.is_empty());
}

#[test]
fn every_byte_flip_in_tag_is_detected() {
    // Stronger guarantee: any single-byte flip anywhere in the tag must be
    // rejected. Iterates the full 16-byte tag region.
    let key = key_a();
    let plaintext = b"per-byte tag fuzz";
    let (nonce, ciphertext) = encrypt_item(plaintext, &key).expect("encrypt");
    let len = ciphertext.len();
    assert!(len >= TAG_SIZE);

    for offset in (len - TAG_SIZE)..len {
        let mut tampered = ciphertext.clone();
        tampered[offset] ^= 0xFF;
        let result = decrypt_item(&tampered, &nonce, &key);
        assert!(matches!(result, Err(EncryptError::AuthFailed)),
            "flip at tag offset {} must fail, got {:?}", offset, result);
    }
}
