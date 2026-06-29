use copypaste_core::*;
use tempfile::tempdir;

/// T2: paste-back e2e test (macOS).
///
/// Requires a macOS event loop and real clipboard access — must be run manually
/// or with a mock event loop. Marked `#[ignore]` so CI skips it.
///
/// When un-ignoring:
///   1. Insert an encrypted item into the database.
///   2. Trigger the paste-back path (e.g. via IPC command or direct call).
///   3. Assert the system clipboard contains the original plaintext.
#[tokio::test]
#[ignore = "requires macOS paste-back event loop — run manually"]
async fn test_paste_back_e2e_macos() {
    // CopyPaste-2h5d: stub body — this test cannot be implemented without a
    // real (or mock) macOS NSPasteboard event loop. Un-ignore and fill in once
    // the mock event loop infrastructure lands in v0.3. Until then the test
    // body intentionally does nothing; the `#[ignore]` gate above prevents it
    // from being executed by `cargo test` (without --ignored).
}

#[test]
fn full_encrypt_store_retrieve_decrypt_flow() {
    let dir = tempdir().unwrap();
    let key = [0x00u8; 32]; // deterministic test key
    let db = Database::open(dir.path().join("test.db"), &key).unwrap();

    let alice = DeviceKeypair::generate();
    let bob = DeviceKeypair::generate();
    let enc_key = alice.derive_enc_key(&bob.public_key_bytes(), "alice-id", "bob-id");

    let plaintext = b"Secret clipboard content";

    // Build a stub item first so we have its UUID — that uuid is bound
    // into the AAD before encryption, then the encrypted payload replaces
    // the stub content. This mirrors the production flow where the row id
    // is generated, then content is encrypted with AAD = (id, schema).
    let mut item = ClipboardItem::new_text(Vec::new(), Vec::new(), 1);
    let aad = build_item_aad(&item.item_id, AAD_SCHEMA_VERSION);
    let (nonce, ciphertext) = encrypt_item_with_aad(plaintext, &enc_key, &aad).unwrap();
    item.content = Some(ciphertext);
    item.content_nonce = Some(nonce.to_vec());
    insert_item(&db, &item).unwrap();

    let pages = get_page(&db, 10, 0).unwrap();
    assert_eq!(pages.len(), 1);
    let stored = &pages[0];
    let nonce_arr: [u8; NONCE_SIZE] = stored
        .content_nonce
        .as_ref()
        .unwrap()
        .as_slice()
        .try_into()
        .unwrap();
    let stored_aad = build_item_aad(&stored.item_id, AAD_SCHEMA_VERSION);
    let decrypted = decrypt_item_with_aad(
        stored.content.as_ref().unwrap(),
        &nonce_arr,
        &enc_key,
        &stored_aad,
    )
    .unwrap();
    assert_eq!(decrypted, plaintext);
}

#[test]
fn sensitive_detection_works() {
    assert!(detect("AKIAIOSFODNN7EXAMPLE").is_some());
    assert!(detect("Hello world").is_none());
}

#[test]
fn chunked_encryption_large_item_roundtrip() {
    let key = [0x77u8; 32];
    let file_id = [0x11u8; 16];
    let data = vec![0xABu8; 200_000]; // 200 KB
    let chunks = encrypt_chunks(&data, &key, &file_id, 64 * 1024).unwrap();
    assert!(chunks.len() > 1);
    let decrypted = decrypt_chunks(&chunks, &key, &file_id).unwrap();
    assert_eq!(decrypted, data);
}
