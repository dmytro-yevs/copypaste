//! Round-trip tests for the key_version-dispatched `encrypt_text` / `decrypt_text`
//! Android FFI functions (CopyPaste-4i2).
//!
//! Verifies:
//!   1. key_version=1 uses AAD `"{item_id}|3"` — matches the daemon's v1 path.
//!   2. key_version=2 uses AAD `"{item_id}|4|2"` — matches the daemon's v2 path
//!      (`build_item_aad_v2(item_id, AAD_SCHEMA_VERSION_V4, 2)`).
//!   3. Cross-version mismatches are rejected (v1-encrypted ≠ v2-decrypted).
//!   4. The daemon-equivalent v2 path: encrypting directly with
//!      `copypaste_core::build_item_aad_v2` and then decrypting via the FFI
//!      `decrypt_text(key_version=2)` round-trips (proves daemon→Android compat).
//!
//! Run with:
//!   cargo test -p copypaste-android --test aad_key_version_roundtrip

use copypaste_android::{decrypt_text, encrypt_text};
use copypaste_core::{
    build_item_aad, build_item_aad_v2, decrypt_item_with_aad, encrypt_item_with_aad,
    AAD_SCHEMA_VERSION, AAD_SCHEMA_VERSION_V4, NONCE_SIZE,
};

/// Deterministic 32-byte test key (not secret — tests only).
fn test_key() -> Vec<u8> {
    (0u8..32u8)
        .map(|i| i.wrapping_mul(11).wrapping_add(7))
        .collect()
}

fn test_key_arr() -> [u8; 32] {
    test_key().try_into().unwrap()
}

// ── key_version=1 round-trip ──────────────────────────────────────────────────

#[test]
fn encrypt_text_v1_roundtrips_with_decrypt_text_v1() {
    let key = test_key();
    let item_id = "test-item-v1-roundtrip";
    let plaintext = b"hello from key_version=1";

    let blob = encrypt_text(item_id.to_string(), plaintext, &key, 1)
        .expect("encrypt_text(key_version=1) must succeed");

    let recovered = decrypt_text(item_id.to_string(), &blob.ciphertext, &blob.nonce, &key, 1)
        .expect("decrypt_text(key_version=1) must round-trip");

    assert_eq!(
        recovered, plaintext,
        "key_version=1 round-trip failed: plaintext did not match"
    );
}

// ── key_version=2 round-trip ──────────────────────────────────────────────────

#[test]
fn encrypt_text_v2_roundtrips_with_decrypt_text_v2() {
    let key = test_key();
    let item_id = "test-item-v2-roundtrip";
    let plaintext = b"hello from key_version=2";

    let blob = encrypt_text(item_id.to_string(), plaintext, &key, 2)
        .expect("encrypt_text(key_version=2) must succeed");

    let recovered = decrypt_text(item_id.to_string(), &blob.ciphertext, &blob.nonce, &key, 2)
        .expect("decrypt_text(key_version=2) must round-trip");

    assert_eq!(
        recovered, plaintext,
        "key_version=2 round-trip failed: plaintext did not match"
    );
}

// ── Cross-version rejection ───────────────────────────────────────────────────

#[test]
fn v1_encrypted_rejected_by_v2_decrypt() {
    let key = test_key();
    let item_id = "cross-version-v1-enc-v2-dec";
    let plaintext = b"must not decrypt with wrong version";

    let blob = encrypt_text(item_id.to_string(), plaintext, &key, 1)
        .expect("encrypt_text(key_version=1) must succeed");

    // Decrypting a v1 ciphertext with v2 AAD must fail (auth-tag mismatch).
    let result = decrypt_text(item_id.to_string(), &blob.ciphertext, &blob.nonce, &key, 2);
    assert!(
        result.is_err(),
        "Expected DecryptionFailed when decrypting v1 ciphertext with key_version=2, got Ok"
    );
}

#[test]
fn v2_encrypted_rejected_by_v1_decrypt() {
    let key = test_key();
    let item_id = "cross-version-v2-enc-v1-dec";
    let plaintext = b"must not decrypt with wrong version";

    let blob = encrypt_text(item_id.to_string(), plaintext, &key, 2)
        .expect("encrypt_text(key_version=2) must succeed");

    // Decrypting a v2 ciphertext with v1 AAD must fail (auth-tag mismatch).
    let result = decrypt_text(item_id.to_string(), &blob.ciphertext, &blob.nonce, &key, 1);
    assert!(
        result.is_err(),
        "Expected DecryptionFailed when decrypting v2 ciphertext with key_version=1, got Ok"
    );
}

// ── Daemon→Android compat: daemon writes v2; FFI decrypt_text(key_version=2) reads it ──

#[test]
fn daemon_v2_ciphertext_decrypts_via_ffi_decrypt_text_v2() {
    // Simulate exactly what the daemon does: encrypt with build_item_aad_v2(item_id, 4, 2).
    // Then prove the Android FFI decrypt_text(key_version=2) recovers the plaintext.
    let key = test_key_arr();
    let item_id = "daemon-written-v2-item";
    let plaintext = b"daemon wrote this with key_version=2";

    // Daemon-side encryption path (mirrors decrypt_item_by_version arm for v=2).
    let daemon_aad = build_item_aad_v2(&item_id.into(), AAD_SCHEMA_VERSION_V4, 2);
    let (nonce, ciphertext) = encrypt_item_with_aad(plaintext, &key, &daemon_aad)
        .expect("daemon-side encrypt must succeed");

    // Android FFI decrypt path.
    let recovered = decrypt_text(
        item_id.to_string(),
        &ciphertext,
        &nonce,
        &key,
        2, // key_version=2 → must choose build_item_aad_v2 AAD
    )
    .expect(
        "decrypt_text(key_version=2) must recover daemon-written v2 ciphertext — \
         THIS IS THE BUG BEING FIXED (CopyPaste-4i2)",
    );

    assert_eq!(
        recovered, plaintext,
        "Android FFI failed to decrypt daemon-written key_version=2 ciphertext"
    );
}

#[test]
fn daemon_v1_ciphertext_decrypts_via_ffi_decrypt_text_v1() {
    // Daemon v1 path: build_item_aad(item_id, AAD_SCHEMA_VERSION=3).
    let key = test_key_arr();
    let item_id = "daemon-written-v1-item";
    let plaintext = b"daemon wrote this with key_version=1 (legacy)";

    let daemon_aad = build_item_aad(&item_id.into(), AAD_SCHEMA_VERSION);
    let (nonce, ciphertext) = encrypt_item_with_aad(plaintext, &key, &daemon_aad)
        .expect("daemon v1 encrypt must succeed");

    let recovered = decrypt_text(
        item_id.to_string(),
        &ciphertext,
        &nonce,
        &key,
        1, // key_version=1 → must choose build_item_aad
    )
    .expect("decrypt_text(key_version=1) must recover daemon-written v1 ciphertext");

    assert_eq!(
        recovered, plaintext,
        "Android FFI failed to decrypt daemon-written key_version=1 ciphertext"
    );
}

// ── AAD content verification (ensures the correct AAD strings are used) ───────

#[test]
fn v1_aad_content_is_item_id_pipe_3() {
    // Encrypt with v1, then try to decrypt with manually-constructed v1 AAD.
    // If the AAD string is wrong, this test catches the mismatch.
    let key = test_key_arr();
    let item_id = "aad-content-check-v1";
    let plaintext = b"aad content check v1";

    let blob = encrypt_text(item_id.to_string(), plaintext, &key, 1)
        .expect("encrypt_text v1 must succeed");
    let nonce: [u8; NONCE_SIZE] = blob.nonce.clone().try_into().unwrap();

    // Construct the expected v1 AAD explicitly.
    let expected_aad = format!("{item_id}|3").into_bytes();

    let recovered = decrypt_item_with_aad(&blob.ciphertext, &nonce, &key, &expected_aad)
        .expect("direct decrypt with v1 AAD must succeed — confirms FFI uses \"{item_id}|3\"");
    assert_eq!(recovered, plaintext);
}

#[test]
fn v2_aad_content_is_item_id_pipe_4_pipe_2() {
    // Encrypt with v2, then try to decrypt with manually-constructed v2 AAD.
    let key = test_key_arr();
    let item_id = "aad-content-check-v2";
    let plaintext = b"aad content check v2";

    let blob = encrypt_text(item_id.to_string(), plaintext, &key, 2)
        .expect("encrypt_text v2 must succeed");
    let nonce: [u8; NONCE_SIZE] = blob.nonce.clone().try_into().unwrap();

    // Construct the expected v2 AAD explicitly.
    let expected_aad = format!("{item_id}|4|2").into_bytes();

    let recovered = decrypt_item_with_aad(&blob.ciphertext, &nonce, &key, &expected_aad)
        .expect("direct decrypt with v2 AAD must succeed — confirms FFI uses \"{item_id}|4|2\"");
    assert_eq!(recovered, plaintext);
}
