use copypaste_core::*;
use tempfile::tempdir;

#[test]
fn full_encrypt_store_retrieve_decrypt_flow() {
    let dir = tempdir().unwrap();
    let db = Database::open(dir.path().join("test.db")).unwrap();

    let alice = DeviceKeypair::generate();
    let bob = DeviceKeypair::generate();
    let enc_key = alice.derive_enc_key(&bob.public_key_bytes(), "alice-id", "bob-id");

    let plaintext = b"Secret clipboard content";
    let (nonce, ciphertext) = encrypt_item(plaintext, &enc_key);

    let item = ClipboardItem::new_text(ciphertext.clone(), nonce.to_vec(), 1);
    insert_item(&db, &item).unwrap();

    let pages = get_page(&db, 10, 0).unwrap();
    assert_eq!(pages.len(), 1);
    let stored = &pages[0];
    let nonce_arr: [u8; NONCE_SIZE] = stored.content_nonce.as_ref().unwrap()
        .as_slice().try_into().unwrap();
    let decrypted = decrypt_item(stored.content.as_ref().unwrap(), &nonce_arr, &enc_key).unwrap();
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
    let chunks = encrypt_chunks(&data, &key, &file_id, 64 * 1024);
    assert!(chunks.len() > 1);
    let decrypted = decrypt_chunks(&chunks, &key, &file_id).unwrap();
    assert_eq!(decrypted, data);
}
