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
//! AAD binding (beta security hardening): the AEAD layer now also exposes
//! `encrypt_item_with_aad` / `decrypt_item_with_aad`, which authenticate an
//! Associated Data string bound to the row's `(item_id, schema_version)`.
//! The three trailing tests in this file pin:
//!   * `aad_swap_fails`        — ciphertext bound to row A cannot be replayed into row B
//!   * `aad_match_succeeds`    — matching AAD round-trips cleanly
//!   * `legacy_empty_aad_fallback` — pre-AAD ciphertext still decrypts (v0.2→v0.3 bridge)

use copypaste_core::{
    build_item_aad, decrypt_item, decrypt_item_with_aad, encrypt_item, encrypt_item_with_aad,
    EncryptError, AAD_SCHEMA_VERSION, NONCE_SIZE,
};

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
    assert!(
        ciphertext.len() > TAG_SIZE,
        "ciphertext must contain body + tag"
    );
    let body_idx = (ciphertext.len() - TAG_SIZE) / 2;
    ciphertext[body_idx] ^= 0x01;

    let result = decrypt_item(&ciphertext, &nonce, &key);
    assert!(
        matches!(result, Err(EncryptError::AuthFailed)),
        "expected AuthFailed for body bit-flip, got {:?}",
        result
    );
}

#[test]
fn bit_flip_in_nonce_returns_error() {
    let key = key_a();
    let plaintext = b"nonce-bound message";
    let (mut nonce, ciphertext) = encrypt_item(plaintext, &key).expect("encrypt");

    nonce[0] ^= 0x01;

    let result = decrypt_item(&ciphertext, &nonce, &key);
    assert!(
        matches!(result, Err(EncryptError::AuthFailed)),
        "expected AuthFailed for nonce bit-flip, got {:?}",
        result
    );
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
    assert!(
        matches!(result, Err(EncryptError::AuthFailed)),
        "expected AuthFailed for tag bit-flip, got {:?}",
        result
    );
}

#[test]
fn truncated_ciphertext_returns_error_not_panic() {
    let key = key_a();
    let plaintext = b"truncate me please";
    let (nonce, mut ciphertext) = encrypt_item(plaintext, &key).expect("encrypt");

    // Drop the last byte — this corrupts the Poly1305 tag length.
    ciphertext.pop();

    let result = decrypt_item(&ciphertext, &nonce, &key);
    assert!(
        matches!(result, Err(EncryptError::AuthFailed)),
        "truncated ciphertext must return Err, not panic; got {:?}",
        result
    );
}

#[test]
fn truncated_below_tag_size_returns_error_not_panic() {
    // Even more aggressive: trim ciphertext down to less than TAG_SIZE bytes.
    // The AEAD layer must reject this cleanly without panicking.
    let key = key_a();
    let (nonce, _ciphertext) = encrypt_item(b"x", &key).expect("encrypt");

    let stub = vec![0u8; TAG_SIZE - 1];
    let result = decrypt_item(&stub, &nonce, &key);
    assert!(
        matches!(result, Err(EncryptError::AuthFailed)),
        "sub-tag-size ciphertext must return Err; got {:?}",
        result
    );
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
    assert!(
        matches!(cross_1, Err(EncryptError::AuthFailed)),
        "ct_a + nonce_b must fail; got {:?}",
        cross_1
    );
    assert!(
        matches!(cross_2, Err(EncryptError::AuthFailed)),
        "ct_b + nonce_a must fail; got {:?}",
        cross_2
    );

    // Correct pairing still works.
    assert_eq!(decrypt_item(&ct_a, &nonce_a, &key).unwrap(), pt_a);
    assert_eq!(decrypt_item(&ct_b, &nonce_b, &key).unwrap(), pt_b);
}

#[test]
fn wrong_key_returns_error() {
    let plaintext = b"only key A can read this";
    let (nonce, ciphertext) = encrypt_item(plaintext, &key_a()).expect("encrypt");

    let result = decrypt_item(&ciphertext, &nonce, &key_b());
    assert!(
        matches!(result, Err(EncryptError::AuthFailed)),
        "expected AuthFailed for wrong key, got {:?}",
        result
    );
}

#[test]
fn empty_plaintext_encrypts_decrypts_correctly() {
    let key = key_a();
    let (nonce, ciphertext) = encrypt_item(b"", &key).expect("encrypt empty");

    // Even empty plaintext produces a 16-byte tag.
    assert_eq!(
        ciphertext.len(),
        TAG_SIZE,
        "empty plaintext ciphertext must equal tag size"
    );
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
        assert!(
            matches!(result, Err(EncryptError::AuthFailed)),
            "flip at tag offset {} must fail, got {:?}",
            offset,
            result
        );
    }
}

// ---------------------------------------------------------------------------
// AAD binding (beta security hardening): bind ciphertext to (item_id, schema).
// ---------------------------------------------------------------------------

/// Ciphertext encrypted with AAD "A" must NOT decrypt under AAD "B" — this is
/// the substitution-attack protection. An attacker who swaps `clipboard_items.content`
/// blobs between two rows is detected by the AEAD auth tag because each row's
/// AAD is derived from its unique `item_id`.
#[test]
fn aad_swap_fails() {
    let key = key_a();
    let plaintext = b"row-A content";

    let aad_a = build_item_aad("item-A-uuid", 3);
    let aad_b = build_item_aad("item-B-uuid", 3);
    assert_ne!(aad_a, aad_b, "distinct item_ids must produce distinct AAD");

    let (nonce, ciphertext) = encrypt_item_with_aad(plaintext, &key, &aad_a).expect("encrypt");

    // Simulate the attacker copying row-A's ciphertext+nonce into row-B.
    // Row-B's decrypt path reconstructs AAD from row-B's own (item_id, schema)
    // — which does not match — so decryption MUST fail.
    let result = decrypt_item_with_aad(&ciphertext, &nonce, &key, &aad_b);
    assert!(
        matches!(result, Err(EncryptError::AuthFailed)),
        "AAD substitution must fail with AuthFailed, got {:?}",
        result,
    );

    // Schema-version mismatch (same item_id, different version) must also fail —
    // pins the second half of the binding pair.
    let aad_a_v2 = build_item_aad("item-A-uuid", 2);
    let schema_swap = decrypt_item_with_aad(&ciphertext, &nonce, &key, &aad_a_v2);
    assert!(
        matches!(schema_swap, Err(EncryptError::AuthFailed)),
        "schema-version downgrade must fail with AuthFailed, got {:?}",
        schema_swap,
    );
}

/// Same AAD on both ends round-trips cleanly — the happy path for the new
/// per-item binding contract.
#[test]
fn aad_match_succeeds() {
    let key = key_a();
    let plaintext = b"correctly-bound payload";
    // Pin the exported AAD_SCHEMA_VERSION constant — storage callers will use
    // this value instead of hard-coding `3`.
    let aad = build_item_aad("item-X-uuid", AAD_SCHEMA_VERSION);

    let (nonce, ciphertext) = encrypt_item_with_aad(plaintext, &key, &aad).expect("encrypt");
    let decrypted = decrypt_item_with_aad(&ciphertext, &nonce, &key, &aad).expect("decrypt");
    assert_eq!(decrypted, plaintext);

    // build_item_aad must be deterministic — encrypting and decrypting from
    // independently-reconstructed AAD bytes must also succeed.
    let aad_again = build_item_aad("item-X-uuid", AAD_SCHEMA_VERSION);
    let decrypted_again = decrypt_item_with_aad(&ciphertext, &nonce, &key, &aad_again)
        .expect("decrypt with rebuilt AAD");
    assert_eq!(decrypted_again, plaintext);
}

/// Legacy rows written by the pre-AAD `encrypt_item` path (empty AAD) MUST
/// still decrypt when read back through the new `decrypt_item_with_aad`
/// surface — the v0.2→v0.3 bridge. The fallback only triggers when the
/// strict-AAD attempt fails AND the supplied AAD is non-empty.
#[test]
fn legacy_empty_aad_fallback() {
    let key = key_a();
    let plaintext = b"row written before AAD landed";

    // Pre-AAD path: legacy producer.
    let (nonce, ciphertext) = encrypt_item(plaintext, &key).expect("legacy encrypt");

    // Post-AAD reader: tries new AAD, falls back to empty AAD on failure.
    let aad = build_item_aad("legacy-row-uuid", 3);
    let decrypted =
        decrypt_item_with_aad(&ciphertext, &nonce, &key, &aad).expect("legacy fallback decrypt");
    assert_eq!(decrypted, plaintext);

    // Also pin the inverse: a NEW ciphertext (encrypted with real AAD) must
    // NOT silently decrypt under a *different* item's AAD just because the
    // fallback exists. The fallback retries with empty AAD, which still
    // does not match the real AAD, so the call must still fail.
    let real_aad = build_item_aad("real-item", 3);
    let other_aad = build_item_aad("attacker-item", 3);
    let (n2, ct2) = encrypt_item_with_aad(b"protected", &key, &real_aad).expect("encrypt");
    let result = decrypt_item_with_aad(&ct2, &n2, &key, &other_aad);
    assert!(
        matches!(result, Err(EncryptError::AuthFailed)),
        "fallback path must not weaken AAD enforcement; got {:?}",
        result,
    );
}
