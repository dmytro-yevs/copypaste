use super::*;
use sha2::{Digest, Sha256};

// ── shared fixtures ──────────────────────────────────────────────────────

const ACCOUNT_A: &str = "proj_abc|00000000-0000-0000-0000-0000000000aa";
const ACCOUNT_B: &str = "proj_abc|00000000-0000-0000-0000-0000000000bb";
const PASS: &str = "correct horse battery staple";

fn make_key(passphrase: &str) -> SyncKey {
    derive_sync_key(passphrase, ACCOUNT_A).expect("derive_sync_key must succeed")
}

// ── golden-byte: PER_ACCOUNT_SALT_IKM ────────────────────────────────────

/// PER_ACCOUNT_SALT_IKM must equal SHA-256(b"copypaste/cloud-sync-key/per-account-salt-ikm").
/// Changing the constant is a hard-fork of all cloud ciphertexts; this test
/// makes that a deliberate, visible act.
#[test]
fn per_account_salt_ikm_is_sha256_of_canonical_input() {
    let expected = Sha256::digest(b"copypaste/cloud-sync-key/per-account-salt-ikm");
    assert_eq!(
        PER_ACCOUNT_SALT_IKM.as_ref(),
        expected.as_slice(),
        "PER_ACCOUNT_SALT_IKM must equal SHA-256(b\"copypaste/cloud-sync-key/per-account-salt-ikm\")"
    );
}

// ── derive_sync_key ──────────────────────────────────────────────────────

/// Same passphrase + same account must yield the same key on every call
/// (cross-device agreement depends on this property).
#[test]
fn same_account_same_passphrase_is_deterministic() {
    let k1 = derive_sync_key(PASS, ACCOUNT_A).expect("derive 1");
    let k2 = derive_sync_key(PASS, ACCOUNT_A).expect("derive 2");
    assert_eq!(
        k1.as_bytes(),
        k2.as_bytes(),
        "same account + passphrase must be deterministic across devices"
    );
}

/// Different passphrases (same account) must produce different keys.
#[test]
fn different_passphrases_produce_different_keys() {
    let k1 = derive_sync_key("passphrase-alpha-xx", ACCOUNT_A).expect("derive 1");
    let k2 = derive_sync_key("passphrase-beta-xxx", ACCOUNT_A).expect("derive 2");
    assert_ne!(k1.as_bytes(), k2.as_bytes());
}

/// Two DIFFERENT account ids with the SAME passphrase must derive DIFFERENT
/// keys — this is the property that defeats cross-user precompute.
#[test]
fn different_accounts_same_passphrase_derive_different_keys() {
    let key_a = derive_sync_key(PASS, ACCOUNT_A).expect("derive A");
    let key_b = derive_sync_key(PASS, ACCOUNT_B).expect("derive B");
    assert_ne!(
        key_a.as_bytes(),
        key_b.as_bytes(),
        "different accounts must not share a key (cross-user precompute would survive)"
    );
    // And a blob from account A must NOT decrypt under account B's key.
    let blob = encrypt_for_cloud(&key_a, "x", b"secret").expect("encrypt");
    assert!(
        matches!(
            decrypt_from_cloud(&key_b, "x", &blob),
            Err(SyncKeyError::DecryptFailed)
        ),
        "account B's key must not decrypt account A's blob"
    );
}

/// An empty account id is rejected with a clear, dedicated error — never a
/// silently-degenerate key.
#[test]
fn empty_account_id_is_rejected() {
    assert!(matches!(
        derive_sync_key(PASS, ""),
        Err(SyncKeyError::EmptyAccountId)
    ));
}

// ── round-trip ───────────────────────────────────────────────────────────

/// Encrypt then decrypt with the SAME key and item_id must return the
/// original plaintext.
#[test]
fn cloud_roundtrip_same_key_and_item_id() {
    let key = make_key(PASS);
    let item_id = "item-cloud-001";
    let plaintext = b"hello from the cloud";

    let blob = encrypt_for_cloud(&key, item_id, plaintext).unwrap();
    let recovered = decrypt_from_cloud(&key, item_id, &blob).unwrap();
    assert_eq!(recovered, plaintext);
}

/// Empty plaintext must also round-trip correctly.
#[test]
fn cloud_roundtrip_empty_plaintext() {
    let key = make_key(PASS);
    let blob = encrypt_for_cloud(&key, "item-empty", b"").unwrap();
    let recovered = decrypt_from_cloud(&key, "item-empty", &blob).unwrap();
    assert_eq!(recovered, b"");
}

// ── wrong passphrase → decrypt fails ────────────────────────────────────

/// Decrypting with a key derived from a different passphrase must fail.
#[test]
fn wrong_passphrase_decrypt_fails() {
    let key_enc = make_key("correct-passphrase-a");
    let key_dec = make_key("wrong-passphrase-bbb");
    let blob = encrypt_for_cloud(&key_enc, "item-x", b"secret data").unwrap();
    let result = decrypt_from_cloud(&key_dec, "item-x", &blob);
    assert!(
        matches!(result, Err(SyncKeyError::DecryptFailed)),
        "wrong passphrase must produce DecryptFailed, got {result:?}"
    );
}

// ── tampered ciphertext fails ────────────────────────────────────────────

/// Flipping a bit in the ciphertext body must cause auth-tag failure.
#[test]
fn tampered_ciphertext_fails() {
    let key = make_key(PASS);
    let mut blob = encrypt_for_cloud(&key, "item-tamper", b"important data").unwrap();
    // Flip a byte in the ciphertext portion (after the 24-byte nonce).
    blob[NONCE_SIZE] ^= 0xFF;
    let result = decrypt_from_cloud(&key, "item-tamper", &blob);
    assert!(matches!(result, Err(SyncKeyError::DecryptFailed)));
}

// ── wrong item_id AAD fails ──────────────────────────────────────────────

/// Decrypting with a different item_id must fail (AAD mismatch).
#[test]
fn wrong_item_id_aad_fails() {
    let key = make_key(PASS);
    let blob = encrypt_for_cloud(&key, "item-correct", b"payload").unwrap();
    let result = decrypt_from_cloud(&key, "item-wrong", &blob);
    assert!(
        matches!(result, Err(SyncKeyError::DecryptFailed)),
        "wrong item_id in AAD must produce DecryptFailed"
    );
}

// ── nonce uniqueness ─────────────────────────────────────────────────────

/// Two encryptions of the same plaintext with the same key must produce
/// different nonces (and therefore different blobs).
#[test]
fn nonce_unique_across_two_encrypts() {
    let key = make_key(PASS);
    let item_id = "item-nonce";
    let plaintext = b"same plaintext";

    let blob1 = encrypt_for_cloud(&key, item_id, plaintext).unwrap();
    let blob2 = encrypt_for_cloud(&key, item_id, plaintext).unwrap();

    // Nonce is the first NONCE_SIZE bytes of the blob.
    assert_ne!(
        &blob1[..NONCE_SIZE],
        &blob2[..NONCE_SIZE],
        "two encrypts must use different nonces"
    );
    // The full blobs must differ too (nonces are embedded in the output).
    assert_ne!(blob1, blob2);
}

// ── blob format ──────────────────────────────────────────────────────────

/// Cloud blob must start with exactly NONCE_SIZE bytes followed by
/// ciphertext+tag (plaintext.len() + 16).
#[test]
fn blob_format_nonce_then_ciphertext_plus_tag() {
    let key = make_key(PASS);
    let plaintext = b"format check";
    let blob = encrypt_for_cloud(&key, "item-fmt", plaintext).unwrap();
    // blob length must be nonce(24) + plaintext(N) + tag(16)
    assert_eq!(blob.len(), NONCE_SIZE + plaintext.len() + 16);
}

// ── blob too short ───────────────────────────────────────────────────────

/// A blob shorter than NONCE_SIZE must return BlobTooShort, not panic.
#[test]
fn blob_too_short_returns_error_not_panic() {
    let key = make_key(PASS);
    let short_blob = [0u8; 10];
    let result = decrypt_from_cloud(&key, "item-short", &short_blob);
    assert!(
        matches!(result, Err(SyncKeyError::BlobTooShort(10))),
        "expected BlobTooShort(10), got {result:?}"
    );
}

// ── cloud domain separation from local ───────────────────────────────────

/// The cloud AAD schema version must be strictly greater than any local
/// schema version (3 and 4) so cloud and local ciphertexts cannot collide.
#[test]
fn cloud_aad_schema_version_is_5() {
    assert_eq!(CLOUD_AAD_SCHEMA_VERSION, 5);
}

/// Cloud AAD bytes match the expected format.
#[test]
fn build_cloud_aad_format() {
    let aad = build_cloud_aad("item-abc");
    assert_eq!(aad, b"item-abc|5");
}

// ── parameter constants ──────────────────────────────────────────────────

#[test]
fn argon2_params_are_expected_values() {
    assert_eq!(ARGON2_M_COST_KIB, 19_456);
    assert_eq!(ARGON2_T_COST, 2);
    assert_eq!(ARGON2_P_COST, 1);
}

// ── SyncKey::from_bytes round-trip ───────────────────────────────────────

/// A blob encrypted with a `SyncKey` produced by `derive_sync_key` must
/// decrypt successfully using a `SyncKey` reconstructed from the same raw
/// bytes via `from_bytes`. This is the code path used by the cloud download
/// worker which snapshots the key bytes before crossing a `spawn_blocking`
/// boundary.
#[test]
fn from_bytes_decrypts_blob_encrypted_by_derive_sync_key() {
    let item_id = "round-trip-item-001";
    let plaintext = b"clipboard content for cloud sync round-trip";

    // Derive a key from the passphrase + account and encrypt.
    let original_key = derive_sync_key(PASS, ACCOUNT_A).expect("derive must succeed");
    // Wrap in Zeroizing so the stack copy is scrubbed when it goes out of
    // scope, closing the window where raw key bytes sit on the stack unguarded.
    let key_bytes = zeroize::Zeroizing::new(*original_key.as_bytes());
    let blob = encrypt_for_cloud(&original_key, item_id, plaintext).expect("encrypt must succeed");

    // Reconstruct a SyncKey from the raw bytes (simulates the spawn_blocking
    // snapshot path) and verify the blob decrypts to the original plaintext.
    let reconstructed_key = SyncKey::from_bytes(*key_bytes);
    let decrypted = decrypt_from_cloud(&reconstructed_key, item_id, &blob)
        .expect("decrypt with from_bytes key must succeed");

    assert_eq!(
        decrypted, plaintext,
        "decrypted plaintext must match the original"
    );
}

/// `from_bytes` with the wrong key bytes must produce `DecryptFailed`, not
/// a panic or incorrect plaintext.
#[test]
fn from_bytes_wrong_key_returns_decrypt_failed() {
    let key = derive_sync_key(PASS, ACCOUNT_A).expect("derive must succeed");
    let blob = encrypt_for_cloud(&key, "item-fb-wrong", b"secret").expect("encrypt must succeed");

    // Construct a key from all-zero bytes — should not decrypt correctly.
    let wrong_key = SyncKey::from_bytes([0u8; 32]);
    let result = decrypt_from_cloud(&wrong_key, "item-fb-wrong", &blob);
    assert!(
        matches!(result, Err(SyncKeyError::DecryptFailed)),
        "wrong key via from_bytes must return DecryptFailed, got {result:?}"
    );
}

// ── passphrase length enforcement ────────────────────────────────────────

/// Passphrases shorter than MIN_PASSPHRASE_LEN must be rejected with
/// `PassphraseTooShort` before Argon2id even runs. Includes an 11-char case
/// to pin the floor (MIN_PASSPHRASE_LEN == 12).
#[test]
fn short_passphrase_returns_passphrase_too_short() {
    for short in &["", "a", "1234567", "12345678901"] {
        assert!(
            short.chars().count() < MIN_PASSPHRASE_LEN,
            "test fixture {short:?} must be shorter than the enforced minimum"
        );
        let result = derive_sync_key(short, ACCOUNT_A);
        assert!(
            matches!(result, Err(SyncKeyError::PassphraseTooShort(_))),
            "passphrase {:?} (len {}) must produce PassphraseTooShort",
            short,
            short.chars().count(),
        );
    }
}

/// The enforced minimum is 12 characters.
#[test]
fn min_passphrase_len_is_twelve() {
    assert_eq!(MIN_PASSPHRASE_LEN, 12);
}

/// A passphrase of exactly MIN_PASSPHRASE_LEN characters must succeed, and
/// one character shorter must be rejected.
#[test]
fn passphrase_at_min_length_succeeds() {
    // "123456789012" is exactly 12 chars — must not return PassphraseTooShort.
    assert!(
        derive_sync_key("123456789012", ACCOUNT_A).is_ok(),
        "passphrase of exactly {MIN_PASSPHRASE_LEN} chars must succeed"
    );
    // 11 chars — one short of the floor — must be rejected.
    assert!(
        matches!(
            derive_sync_key("12345678901", ACCOUNT_A),
            Err(SyncKeyError::PassphraseTooShort(11))
        ),
        "passphrase one char below the floor must be rejected"
    );
}

/// The PassphraseTooShort error carries the actual char count.
#[test]
fn passphrase_too_short_error_contains_length() {
    match derive_sync_key("abc", ACCOUNT_A) {
        Err(SyncKeyError::PassphraseTooShort(n)) => assert_eq!(n, 3),
        // Note: `SyncKey` has no `Debug`, so don't format the matched value.
        _ => panic!("expected PassphraseTooShort(3), got a different Err variant or Ok"),
    }
}

// ── C-P0-4: sync-key rotation framing ────────────────────────────────────

/// Rotating the sync key is the ONLY real cloud/relay device revocation: a
/// blob encrypted under key A (held by a now-revoked device) must FAIL to
/// decrypt under the rotated key B. This proves the revoked device can no
/// longer read items produced AFTER the rotation, even though it still holds
/// key A.
#[test]
fn rotated_key_cannot_decrypt_pre_rotation_blob() {
    let item_id = "rotation-item-001";
    let plaintext = b"secret produced after rotation";

    // Key A = the pre-rotation key the revoked device still holds.
    let key_a = derive_sync_key("old-shared-passphrase", ACCOUNT_A).expect("derive A");
    // Key B = the rotated key. A different passphrase yields different bytes.
    let key_b = derive_sync_key("new-rotated-passphrase", ACCOUNT_A).expect("derive B");
    assert_ne!(
        key_a.as_bytes(),
        key_b.as_bytes(),
        "rotation must produce a distinct key"
    );

    // A NEW cloud item is encrypted under the rotated key B.
    let blob_b = encrypt_for_cloud(&key_b, item_id, plaintext).expect("encrypt under B");

    // The revoked device, holding only key A, must NOT be able to decrypt it.
    let result = decrypt_from_cloud(&key_a, item_id, &blob_b);
    assert!(
        matches!(result, Err(SyncKeyError::DecryptFailed)),
        "pre-rotation key A must not decrypt a post-rotation blob, got {result:?}"
    );

    // Sanity: the rotated key B still decrypts its own blob.
    let ok = decrypt_from_cloud(&key_b, item_id, &blob_b).expect("B decrypts its own blob");
    assert_eq!(ok, plaintext);
}

/// `SyncKey::ct_eq_bytes` is the helper the daemon's provisioning-apply path
/// uses to distinguish a routine re-provision (identical key → no-op) from a
/// rotation re-provision (differing key → replace). Verify it matches the
/// raw-byte equality without leaking via `==`.
#[test]
fn ct_eq_bytes_matches_byte_equality() {
    let key = derive_sync_key("ct-eq-passphrase", ACCOUNT_A).expect("derive must succeed");
    let same = *key.as_bytes();
    let mut different = *key.as_bytes();
    different[0] ^= 0xFF;

    assert!(key.ct_eq_bytes(&same), "identical bytes must compare equal");
    assert!(
        !key.ct_eq_bytes(&different),
        "differing bytes must compare unequal"
    );
}

// ── per-account salt internals ───────────────────────────────────────────

/// The per-account salt itself must be deterministic and account-dependent,
/// and must never collapse to the bare IKM constant.
#[test]
fn per_account_salt_is_deterministic_and_unique() {
    let sa1 = derive_per_account_salt(ACCOUNT_A);
    let sa2 = derive_per_account_salt(ACCOUNT_A);
    let sb = derive_per_account_salt(ACCOUNT_B);
    assert_eq!(sa1, sa2, "same account id must yield the same salt");
    assert_ne!(sa1, sb, "different account ids must yield different salts");
    // The per-account salt must not collapse to the raw IKM.
    assert_ne!(&sa1, PER_ACCOUNT_SALT_IKM.as_ref());
}
