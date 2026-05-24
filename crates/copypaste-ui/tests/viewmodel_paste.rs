// tests/viewmodel_paste.rs — ViewModel unit tests for paste-back / AAD selection.
//
// Regression guard for the 06b8f84 bug:
//   "ipc: fix paste decrypt always used v1 AAD regardless of key_version"
//
// These tests call `decrypt_item_by_version` from `copypaste-core` directly —
// the same function the daemon's IPC paste handler uses — to verify that
// key_version=1 selects v1 AAD and key_version=2 selects v2 AAD. A
// regression (reverting to always-v1) would cause the v2 decrypt to fail
// with an auth-tag mismatch error.
//
// No Slint runtime required.

use copypaste_core::{
    build_item_aad, build_item_aad_v2, decrypt_item_by_version, encrypt_item_with_aad,
    EncryptError, AAD_SCHEMA_VERSION, AAD_SCHEMA_VERSION_V4, NONCE_SIZE,
};

// ── AAD key_version dispatch (regression for 06b8f84) ──────────────────────

#[test]
fn paste_key_version_1_uses_v1_aad() {
    // Simulate encrypting an item as the daemon does for key_version=1:
    // AAD = build_item_aad(item_id, AAD_SCHEMA_VERSION) i.e. "id|3"
    let v1_key = [0x01u8; 32];
    let v2_key = [0x02u8; 32];
    let item_id = "clipboard-item-aabbccdd";

    let aad = build_item_aad(item_id, AAD_SCHEMA_VERSION);
    let (nonce, ciphertext) =
        encrypt_item_with_aad(b"plaintext for v1 item", &v1_key, &aad).unwrap();

    // Decrypt via the version-dispatching function (what the IPC handler calls).
    let plaintext =
        decrypt_item_by_version(1, &v1_key, &v2_key, item_id, &nonce, &ciphertext).unwrap();

    assert_eq!(
        plaintext, b"plaintext for v1 item",
        "key_version=1 must decrypt using v1 AAD (regression: 06b8f84)"
    );
}

#[test]
fn paste_key_version_2_uses_v2_aad() {
    // Simulate encrypting an item as the daemon does for key_version=2:
    // AAD = build_item_aad_v2(item_id, AAD_SCHEMA_VERSION_V4, 2) i.e. "id|4|2"
    let v1_key = [0x03u8; 32];
    let v2_key = [0x04u8; 32];
    let item_id = "clipboard-item-11223344";

    let aad = build_item_aad_v2(item_id, AAD_SCHEMA_VERSION_V4, 2);
    let (nonce, ciphertext) =
        encrypt_item_with_aad(b"plaintext for v2 item", &v2_key, &aad).unwrap();

    let plaintext =
        decrypt_item_by_version(2, &v1_key, &v2_key, item_id, &nonce, &ciphertext).unwrap();

    assert_eq!(
        plaintext, b"plaintext for v2 item",
        "key_version=2 must decrypt using v2 AAD (regression: 06b8f84)"
    );
}

#[test]
fn paste_v1_item_with_wrong_key_fails_auth() {
    // Using the wrong key for v1 must return an auth error, not garbage plaintext.
    let correct_key = [0x10u8; 32];
    let wrong_key = [0x11u8; 32];
    let item_id = "item-wrong-key-test";

    let aad = build_item_aad(item_id, AAD_SCHEMA_VERSION);
    let (nonce, ciphertext) = encrypt_item_with_aad(b"secret content", &correct_key, &aad).unwrap();

    let result = decrypt_item_by_version(
        1,
        &wrong_key, // wrong v1 key
        &[0x00u8; 32],
        item_id,
        &nonce,
        &ciphertext,
    );

    assert!(
        result.is_err(),
        "wrong key for v1 item must produce an error, not garbage plaintext"
    );
}

#[test]
fn paste_v2_item_with_v1_key_fails_auth() {
    // A v2-encrypted item decrypted with the v1 key (the pre-fix bug) must fail.
    let v1_key = [0x20u8; 32];
    let v2_key = [0x21u8; 32];
    let item_id = "item-v2-aad-mismatch";

    // Encrypt with v2 key + v2 AAD.
    let aad_v2 = build_item_aad_v2(item_id, AAD_SCHEMA_VERSION_V4, 2);
    let (nonce, ciphertext) = encrypt_item_with_aad(b"v2 secret", &v2_key, &aad_v2).unwrap();

    // Attempt to decrypt using key_version=1 (the old buggy code path): must fail.
    let result = decrypt_item_by_version(1, &v1_key, &v2_key, item_id, &nonce, &ciphertext);
    assert!(
        result.is_err(),
        "decrypting a v2 item as key_version=1 must fail — v1 key + v1 AAD cannot decrypt v2 ciphertext"
    );
}

#[test]
fn paste_v1_item_with_v2_path_fails_auth() {
    // Symmetric guard: v1 ciphertext must not decrypt via the v2 path.
    let v1_key = [0x30u8; 32];
    let v2_key = [0x31u8; 32];
    let item_id = "item-v1-aad-mismatch";

    let aad_v1 = build_item_aad(item_id, AAD_SCHEMA_VERSION);
    let (nonce, ciphertext) = encrypt_item_with_aad(b"v1 content", &v1_key, &aad_v1).unwrap();

    // Attempt to decrypt as key_version=2 (mismatched AAD): must fail.
    let result = decrypt_item_by_version(2, &v1_key, &v2_key, item_id, &nonce, &ciphertext);
    assert!(
        result.is_err(),
        "decrypting a v1 item as key_version=2 must fail — v2 AAD cannot authenticate v1 ciphertext"
    );
}

#[test]
fn paste_unknown_key_version_returns_error_not_panic() {
    // A row with a future or corrupt key_version must return an error, never panic.
    let nonce = [0u8; NONCE_SIZE];
    let result = decrypt_item_by_version(
        255, // unknown version
        &[0u8; 32],
        &[0u8; 32],
        "item-unknown-ver",
        &nonce,
        &[],
    );

    match result {
        Err(EncryptError::UnknownKeyVersion(255)) => {} // expected
        Err(_other) => {} // any other error is also acceptable (no panic)
        Ok(_) => panic!("unknown key_version must not succeed — got Ok"),
    }
}

// ── Decrypt failure surfaces as error, not panic ────────────────────────────

#[test]
fn decrypt_with_tampered_ciphertext_returns_error() {
    let v1_key = [0x40u8; 32];
    let v2_key = [0x41u8; 32];
    let item_id = "tampered-item";

    let aad = build_item_aad(item_id, AAD_SCHEMA_VERSION);
    let (nonce, mut ciphertext) =
        encrypt_item_with_aad(b"authentic content", &v1_key, &aad).unwrap();

    // Corrupt one byte to simulate a tampered ciphertext.
    if !ciphertext.is_empty() {
        ciphertext[0] ^= 0xFF;
    }

    let result = decrypt_item_by_version(1, &v1_key, &v2_key, item_id, &nonce, &ciphertext);
    assert!(
        result.is_err(),
        "tampered ciphertext must return Err, not produce garbage plaintext"
    );
}

#[test]
fn decrypt_with_wrong_item_id_returns_error() {
    // AAD binds the item_id — using a different item_id must fail authentication.
    let v1_key = [0x50u8; 32];
    let v2_key = [0x51u8; 32];
    let real_id = "item-real-id";
    let wrong_id = "item-different-id";

    let aad = build_item_aad(real_id, AAD_SCHEMA_VERSION);
    let (nonce, ciphertext) = encrypt_item_with_aad(b"bound content", &v1_key, &aad).unwrap();

    // Decrypt with the wrong item_id — AAD mismatch must return error.
    let result = decrypt_item_by_version(1, &v1_key, &v2_key, wrong_id, &nonce, &ciphertext);
    assert!(
        result.is_err(),
        "wrong item_id in AAD must cause auth failure, not return plaintext"
    );
}

// ── AAD format contracts ────────────────────────────────────────────────────

#[test]
fn aad_v1_format_is_item_id_pipe_schema_version() {
    let aad = build_item_aad("my-item-id", AAD_SCHEMA_VERSION);
    let aad_str = std::str::from_utf8(&aad).expect("AAD must be valid UTF-8");
    assert_eq!(
        aad_str,
        format!("my-item-id|{AAD_SCHEMA_VERSION}"),
        "v1 AAD format must be 'item_id|schema_version'"
    );
}

#[test]
fn aad_v2_format_is_item_id_pipe_schema_v4_pipe_key_version() {
    let aad = build_item_aad_v2("my-item-id", AAD_SCHEMA_VERSION_V4, 2);
    let aad_str = std::str::from_utf8(&aad).expect("AAD v2 must be valid UTF-8");
    assert_eq!(
        aad_str,
        format!("my-item-id|{AAD_SCHEMA_VERSION_V4}|2"),
        "v2 AAD format must be 'item_id|schema_version_v4|key_version'"
    );
}

#[test]
fn aad_v1_and_v2_are_distinct_for_same_item_id() {
    let item_id = "same-item-id";
    let aad_v1 = build_item_aad(item_id, AAD_SCHEMA_VERSION);
    let aad_v2 = build_item_aad_v2(item_id, AAD_SCHEMA_VERSION_V4, 2);
    assert_ne!(
        aad_v1, aad_v2,
        "v1 and v2 AADs for the same item_id must be distinct (otherwise substitution attacks are possible)"
    );
}
