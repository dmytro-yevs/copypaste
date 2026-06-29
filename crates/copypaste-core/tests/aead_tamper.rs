//! AEAD tamper-detection tests for XChaCha20-Poly1305
//! (`encrypt_item_with_aad` / `decrypt_item_with_aad`).
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
//! AAD binding (v0.3 security contract): the AEAD layer authenticates an
//! Associated Data string bound to the row's `(item_id, schema_version)`.
//! The trailing AAD tests in this file pin:
//!   * `aad_swap_fails`            — ciphertext bound to row A cannot be replayed into row B
//!   * `aad_match_succeeds`        — matching AAD round-trips cleanly
//!   * `decrypt_without_aad_fails` — v0.3 explicitly drops the v0.2 empty-AAD fallback;
//!     pre-v0.3 ciphertexts no longer decrypt cleanly.

use copypaste_core::{
    build_item_aad, decrypt_item_with_aad, encrypt_item_with_aad, EncryptError, ItemId,
    AAD_SCHEMA_VERSION, NONCE_SIZE,
};

const TAG_SIZE: usize = 16;

fn key_a() -> [u8; 32] {
    [0x11u8; 32]
}

fn key_b() -> [u8; 32] {
    [0x22u8; 32]
}

/// Default AAD used by the legacy-style round-trip tests below. Each test
/// uses a unique item_id so any future cross-test bleed would be detected
/// by the AAD binding itself.
fn aad_for(item_id: &str) -> Vec<u8> {
    build_item_aad(&ItemId::from(item_id), AAD_SCHEMA_VERSION)
}

#[test]
fn decrypt_unmodified_ciphertext_succeeds() {
    let key = key_a();
    let aad = aad_for("decrypt_unmodified");
    let plaintext = b"hello clipboard";
    let (nonce, ciphertext) = encrypt_item_with_aad(plaintext, &key, &aad).expect("encrypt");
    let decrypted = decrypt_item_with_aad(&ciphertext, &nonce, &key, &aad).expect("decrypt");
    assert_eq!(decrypted, plaintext);
}

#[test]
fn bit_flip_in_ciphertext_body_returns_error() {
    let key = key_a();
    let aad = aad_for("body_flip");
    let plaintext = b"sensitive payload that is longer than the tag";
    let (nonce, mut ciphertext) = encrypt_item_with_aad(plaintext, &key, &aad).expect("encrypt");

    // Body = everything before the trailing 16-byte tag. Flip a bit in the
    // middle of the body.
    assert!(
        ciphertext.len() > TAG_SIZE,
        "ciphertext must contain body + tag"
    );
    let body_idx = (ciphertext.len() - TAG_SIZE) / 2;
    ciphertext[body_idx] ^= 0x01;

    let result = decrypt_item_with_aad(&ciphertext, &nonce, &key, &aad);
    assert!(
        matches!(result, Err(EncryptError::AuthFailed)),
        "expected AuthFailed for body bit-flip, got {:?}",
        result
    );
}

#[test]
fn bit_flip_in_nonce_returns_error() {
    let key = key_a();
    let aad = aad_for("nonce_flip");
    let plaintext = b"nonce-bound message";
    let (mut nonce, ciphertext) = encrypt_item_with_aad(plaintext, &key, &aad).expect("encrypt");

    nonce[0] ^= 0x01;

    let result = decrypt_item_with_aad(&ciphertext, &nonce, &key, &aad);
    assert!(
        matches!(result, Err(EncryptError::AuthFailed)),
        "expected AuthFailed for nonce bit-flip, got {:?}",
        result
    );
}

#[test]
fn bit_flip_in_auth_tag_returns_error() {
    let key = key_a();
    let aad = aad_for("tag_flip");
    let plaintext = b"tag-protected message";
    let (nonce, mut ciphertext) = encrypt_item_with_aad(plaintext, &key, &aad).expect("encrypt");

    // Last 16 bytes of the AEAD output are the Poly1305 tag.
    let len = ciphertext.len();
    assert!(len >= TAG_SIZE);
    ciphertext[len - 1] ^= 0x80;

    let result = decrypt_item_with_aad(&ciphertext, &nonce, &key, &aad);
    assert!(
        matches!(result, Err(EncryptError::AuthFailed)),
        "expected AuthFailed for tag bit-flip, got {:?}",
        result
    );
}

#[test]
fn truncated_ciphertext_returns_error_not_panic() {
    let key = key_a();
    let aad = aad_for("truncated");
    let plaintext = b"truncate me please";
    let (nonce, mut ciphertext) = encrypt_item_with_aad(plaintext, &key, &aad).expect("encrypt");

    // Drop the last byte — this corrupts the Poly1305 tag length.
    ciphertext.pop();

    let result = decrypt_item_with_aad(&ciphertext, &nonce, &key, &aad);
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
    let aad = aad_for("truncated_sub_tag");
    let (nonce, _ciphertext) = encrypt_item_with_aad(b"x", &key, &aad).expect("encrypt");

    let stub = vec![0u8; TAG_SIZE - 1];
    let result = decrypt_item_with_aad(&stub, &nonce, &key, &aad);
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
    let aad = aad_for("nonce_swap");
    let pt_a = b"AAAAAAAAAAAAAAAA";
    let pt_b = b"BBBBBBBBBBBBBBBB";

    let (nonce_a, ct_a) = encrypt_item_with_aad(pt_a, &key, &aad).expect("encrypt a");
    let (nonce_b, ct_b) = encrypt_item_with_aad(pt_b, &key, &aad).expect("encrypt b");

    assert_ne!(nonce_a, nonce_b, "fresh nonces must differ");

    // Cross-pair: wrong nonce must NOT yield the other plaintext.
    let cross_1 = decrypt_item_with_aad(&ct_a, &nonce_b, &key, &aad);
    let cross_2 = decrypt_item_with_aad(&ct_b, &nonce_a, &key, &aad);
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
    assert_eq!(
        decrypt_item_with_aad(&ct_a, &nonce_a, &key, &aad).unwrap(),
        pt_a
    );
    assert_eq!(
        decrypt_item_with_aad(&ct_b, &nonce_b, &key, &aad).unwrap(),
        pt_b
    );
}

#[test]
fn wrong_key_returns_error() {
    let aad = aad_for("wrong_key");
    let plaintext = b"only key A can read this";
    let (nonce, ciphertext) = encrypt_item_with_aad(plaintext, &key_a(), &aad).expect("encrypt");

    let result = decrypt_item_with_aad(&ciphertext, &nonce, &key_b(), &aad);
    assert!(
        matches!(result, Err(EncryptError::AuthFailed)),
        "expected AuthFailed for wrong key, got {:?}",
        result
    );
}

#[test]
fn empty_plaintext_encrypts_decrypts_correctly() {
    let key = key_a();
    let aad = aad_for("empty_plaintext");
    let (nonce, ciphertext) = encrypt_item_with_aad(b"", &key, &aad).expect("encrypt empty");

    // Even empty plaintext produces a 16-byte tag.
    assert_eq!(
        ciphertext.len(),
        TAG_SIZE,
        "empty plaintext ciphertext must equal tag size"
    );
    assert_eq!(nonce.len(), NONCE_SIZE);

    let decrypted = decrypt_item_with_aad(&ciphertext, &nonce, &key, &aad).expect("decrypt empty");
    assert!(decrypted.is_empty());
}

#[test]
fn every_byte_flip_in_tag_is_detected() {
    // Stronger guarantee: any single-byte flip anywhere in the tag must be
    // rejected. Iterates the full 16-byte tag region.
    let key = key_a();
    let aad = aad_for("tag_fuzz");
    let plaintext = b"per-byte tag fuzz";
    let (nonce, ciphertext) = encrypt_item_with_aad(plaintext, &key, &aad).expect("encrypt");
    let len = ciphertext.len();
    assert!(len >= TAG_SIZE);

    for offset in (len - TAG_SIZE)..len {
        let mut tampered = ciphertext.clone();
        tampered[offset] ^= 0xFF;
        let result = decrypt_item_with_aad(&tampered, &nonce, &key, &aad);
        assert!(
            matches!(result, Err(EncryptError::AuthFailed)),
            "flip at tag offset {} must fail, got {:?}",
            offset,
            result
        );
    }
}

// ---------------------------------------------------------------------------
// AAD binding (v0.3 security contract): bind ciphertext to (item_id, schema).
// ---------------------------------------------------------------------------

/// Ciphertext encrypted with AAD "A" must NOT decrypt under AAD "B" — this is
/// the substitution-attack protection. An attacker who swaps `clipboard_items.content`
/// blobs between two rows is detected by the AEAD auth tag because each row's
/// AAD is derived from its unique `item_id`.
#[test]
fn aad_swap_fails() {
    let key = key_a();
    let plaintext = b"row-A content";

    let aad_a = build_item_aad(&ItemId::from("item-A-uuid"), 3);
    let aad_b = build_item_aad(&ItemId::from("item-B-uuid"), 3);
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
    let aad_a_v2 = build_item_aad(&ItemId::from("item-A-uuid"), 2);
    let schema_swap = decrypt_item_with_aad(&ciphertext, &nonce, &key, &aad_a_v2);
    assert!(
        matches!(schema_swap, Err(EncryptError::AuthFailed)),
        "schema-version downgrade must fail with AuthFailed, got {:?}",
        schema_swap,
    );
}

/// Same AAD on both ends round-trips cleanly — the happy path for the
/// per-item binding contract.
#[test]
fn aad_match_succeeds() {
    let key = key_a();
    let plaintext = b"correctly-bound payload";
    // Pin the exported AAD_SCHEMA_VERSION constant — storage callers will use
    // this value instead of hard-coding `3`.
    let aad = build_item_aad(&ItemId::from("item-X-uuid"), AAD_SCHEMA_VERSION);

    let (nonce, ciphertext) = encrypt_item_with_aad(plaintext, &key, &aad).expect("encrypt");
    let decrypted = decrypt_item_with_aad(&ciphertext, &nonce, &key, &aad).expect("decrypt");
    assert_eq!(decrypted, plaintext);

    // build_item_aad must be deterministic — encrypting and decrypting from
    // independently-reconstructed AAD bytes must also succeed.
    let aad_again = build_item_aad(&ItemId::from("item-X-uuid"), AAD_SCHEMA_VERSION);
    let decrypted_again = decrypt_item_with_aad(&ciphertext, &nonce, &key, &aad_again)
        .expect("decrypt with rebuilt AAD");
    assert_eq!(decrypted_again, plaintext);
}

/// v0.3 breaking change: rows written by the pre-AAD v0.2 path (empty AAD) MUST
/// NO LONGER decrypt under the new strict-AAD decrypt surface. v0.2 → v0.3
/// upgrade path is: run `copypaste migrate v3` (which backfills AAD across the
/// row population) BEFORE upgrading the daemon. If the v0.2 daemon is killed
/// before the backfill completes, those rows are unreadable in v0.3 — that is
/// the one-way break we are explicitly accepting in v0.3.
#[test]
fn decrypt_without_aad_fails() {
    let key = key_a();
    let plaintext = b"row written by v0.2 with empty AAD";

    // Simulate a v0.2 ciphertext: encrypted with empty AAD.
    let empty_aad: &[u8] = &[];
    let (nonce, ciphertext) =
        encrypt_item_with_aad(plaintext, &key, empty_aad).expect("simulate v0.2 encrypt");

    // v0.3 reader: reconstructs AAD from (item_id, schema_version). The
    // legacy empty-AAD fallback is GONE, so the strict-AAD decrypt MUST fail.
    let v3_aad = build_item_aad(&ItemId::from("legacy-row-uuid"), AAD_SCHEMA_VERSION);
    let result = decrypt_item_with_aad(&ciphertext, &nonce, &key, &v3_aad);
    assert!(
        matches!(result, Err(EncryptError::AuthFailed)),
        "v0.3 must reject v0.2 empty-AAD ciphertexts (legacy fallback removed); got {:?}",
        result,
    );

    // Sanity: the same ciphertext still decrypts when the caller supplies
    // the original empty AAD — proves the failure above is specifically the
    // AAD mismatch, not key/nonce corruption.
    let sanity = decrypt_item_with_aad(&ciphertext, &nonce, &key, empty_aad);
    assert_eq!(sanity.unwrap(), plaintext);
}
