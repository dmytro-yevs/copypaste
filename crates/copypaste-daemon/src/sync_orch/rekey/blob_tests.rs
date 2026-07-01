//! Image/file blob rekey round-trip tests, relocated out of the former flat
//! `sync_orch/mod.rs` test module (ADR-017, CopyPaste-vp63.3). Text round-trip
//! + per-peer key isolation live in `tests.rs`.

use super::inbound::parse_file_name_mime;
use super::*;

/// v0.6 image sync: an image stored at rest under device A's local v1 seed
/// (the chunk-encryption key handle, exactly as `handle_image` uses) must,
/// after `rekey_outbound` (image arm — reassembles plaintext, re-wraps under
/// the shared key) → `rekey_inbound` (re-chunks under device B's local key),
/// decode back to the ORIGINAL PNG bytes on B, with a re-derived
/// file_id/item_id that converges with A's (deterministic dedup).
#[test]
fn image_rekey_round_trip_decodes_back_to_original_png_on_peer() {
    use base64::Engine as _;
    use copypaste_core::{chunks_from_blob, chunks_to_blob, decode_image, encode_image_with_limit};
    use copypaste_sync::protocol::WireItem;
    use tempfile::tempdir;

    let seed_a = [0x11u8; 32];
    let seed_b = [0x22u8; 32];
    let shared = [0x33u8; 32];
    let shared_b64 = base64::engine::general_purpose::STANDARD.encode(shared);

    // A real tiny PNG (2x2 RGB). Built via the image crate so encode/decode
    // is exercised end-to-end.
    let png = {
        use image::ImageEncoder;
        let mut buf = Vec::new();
        let raw = vec![0xFFu8; 2 * 2 * 3];
        image::codecs::png::PngEncoder::new(&mut buf)
            .write_image(&raw, 2, 2, image::ExtendedColorType::Rgb8)
            .expect("encode test png");
        buf
    };

    // SENDER A: store the image under A's LOCAL v1 seed (handle_image path).
    let file_id_a = crate::clipboard::image_content_hash(&png);
    let (_meta_a, chunks_a) =
        encode_image_with_limit(&png, &seed_a, &file_id_a, 0, 256).expect("A encode image");
    let blob_a = chunks_to_blob(&chunks_a).expect("A blob");
    let item_id = uuid::Uuid::from_bytes(file_id_a).to_string();
    let meta_json_a = format!(r#"{{"file_id":{file_id_a:?}}}"#);

    let mut wire = WireItem {
        deleted: false,
        pinned: false,
        pin_order: None,
        id: "img-row".to_string(),
        item_id: item_id.clone(),
        content_type: "image".to_string(),
        content: Some(blob_a),
        content_nonce: Some(vec![0u8; 24]),
        blob_ref: Some(meta_json_a),
        is_sensitive: false,
        lamport_ts: 9,
        wall_time: 1_700_000_000_000,
        expires_at: None,
        app_bundle_id: None,
        origin_device_id: "device-A".to_string(),
        key_version: 1,
        file_name: None,
        mime: None,
    };

    let dir_a = tempdir().unwrap();
    let peers_a = dir_a.path().join("peers.json");
    std::fs::write(
        &peers_a,
        format!(
            r#"[{{"fingerprint":"bb:bb","added_at":1,"address":"127.0.0.1:9","sync_key_b64":"{shared_b64}"}}]"#
        ),
    )
    .unwrap();
    let crypto_a = SyncCrypto::new(seed_a, peers_a);

    // OUTBOUND image arm: reassemble + re-wrap under shared key.
    assert_eq!(
        rekey_outbound(&crypto_a, &mut wire),
        RekeyOutcome::Rewrapped
    );
    assert!(wire.content_nonce.is_none(), "wrapped blob clears nonce");
    assert!(wire.blob_ref.is_none(), "blob_ref dropped on the wire");
    assert_eq!(wire.content_type, "image");

    // RECEIVER B: different local seed, same shared key.
    let dir_b = tempdir().unwrap();
    let peers_b = dir_b.path().join("peers.json");
    std::fs::write(
        &peers_b,
        format!(
            r#"[{{"fingerprint":"aa:aa","added_at":1,"address":"127.0.0.1:9","sync_key_b64":"{shared_b64}"}}]"#
        ),
    )
    .unwrap();
    let crypto_b = SyncCrypto::new(seed_b, peers_b);

    let (stored, fts) = rekey_inbound(&crypto_b, wire).expect("B must unwrap the synced image");
    assert!(fts.is_none(), "images carry no FTS plaintext");
    assert_eq!(stored.content_type, "image");
    assert_eq!(
        stored.item_id, item_id,
        "item_id must converge across devices"
    );

    // B decodes its re-chunked blob under B's local seed → original PNG.
    let meta_b = stored.blob_ref.expect("B meta json");
    let file_id_b = crate::ipc::parse_image_file_id(&meta_b).expect("parse B file_id");
    assert_eq!(file_id_b, file_id_a, "file_id must converge");
    let b_chunks =
        chunks_from_blob(&stored.content.expect("B content")).expect("B chunks_from_blob");
    let recovered = decode_image(&b_chunks, &seed_b, &file_id_b).expect("B decode image");
    assert_eq!(recovered, png, "B recovers A's exact PNG bytes");
}

/// v0.6 file sync: same as the image round-trip but for an arbitrary file
/// blob (`content_type = "file"`), chunked verbatim (no decode/re-encode).
#[test]
fn file_rekey_round_trip_decodes_back_to_original_bytes_on_peer() {
    use base64::Engine as _;
    use copypaste_core::{chunks_from_blob, chunks_to_blob, decode_file, encode_file};
    use copypaste_sync::protocol::WireItem;
    use tempfile::tempdir;

    let seed_a = [0x44u8; 32];
    let seed_b = [0x55u8; 32];
    let shared = [0x66u8; 32];
    let shared_b64 = base64::engine::general_purpose::STANDARD.encode(shared);

    let raw = b"arbitrary file bytes \x00\x01\x02 over P2P sync".to_vec();

    let file_id_a = crate::clipboard::image_content_hash(&raw);
    let (meta_a, chunks_a) = encode_file(
        &raw,
        "notes.bin",
        "application/octet-stream",
        &seed_a,
        &file_id_a,
        0,
    )
    .expect("A encode file");
    let blob_a = chunks_to_blob(&chunks_a).expect("A blob");
    let item_id = uuid::Uuid::from_bytes(file_id_a).to_string();
    let meta_json_a = crate::clipboard::build_file_meta_json(&meta_a);

    let mut wire = WireItem {
        deleted: false,
        pinned: false,
        pin_order: None,
        id: "file-row".to_string(),
        item_id: item_id.clone(),
        content_type: "file".to_string(),
        content: Some(blob_a),
        content_nonce: Some(vec![0u8; 24]),
        blob_ref: Some(meta_json_a),
        is_sensitive: false,
        lamport_ts: 11,
        wall_time: 1_700_000_000_000,
        expires_at: None,
        app_bundle_id: None,
        origin_device_id: "device-A".to_string(),
        key_version: 1,
        file_name: None,
        mime: None,
    };

    let dir_a = tempdir().unwrap();
    let peers_a = dir_a.path().join("peers.json");
    std::fs::write(
        &peers_a,
        format!(
            r#"[{{"fingerprint":"bb:bb","added_at":1,"address":"127.0.0.1:9","sync_key_b64":"{shared_b64}"}}]"#
        ),
    )
    .unwrap();
    let crypto_a = SyncCrypto::new(seed_a, peers_a);

    assert_eq!(
        rekey_outbound(&crypto_a, &mut wire),
        RekeyOutcome::Rewrapped
    );
    assert!(wire.content_nonce.is_none());
    assert!(
        wire.blob_ref.is_none(),
        "blob_ref must be cleared on the wire"
    );
    assert_eq!(wire.content_type, "file");
    // #21b: filename + mime must be stamped on the wire before blob_ref is cleared.
    assert_eq!(
        wire.file_name.as_deref(),
        Some("notes.bin"),
        "rekey_blob_outbound must stamp file_name onto the wire"
    );
    assert_eq!(
        wire.mime.as_deref(),
        Some("application/octet-stream"),
        "rekey_blob_outbound must stamp mime onto the wire"
    );

    let dir_b = tempdir().unwrap();
    let peers_b = dir_b.path().join("peers.json");
    std::fs::write(
        &peers_b,
        format!(
            r#"[{{"fingerprint":"aa:aa","added_at":1,"address":"127.0.0.1:9","sync_key_b64":"{shared_b64}"}}]"#
        ),
    )
    .unwrap();
    let crypto_b = SyncCrypto::new(seed_b, peers_b);

    let (stored, fts) = rekey_inbound(&crypto_b, wire).expect("B must unwrap the synced file");
    assert!(fts.is_none());
    assert_eq!(stored.content_type, "file");
    assert_eq!(stored.item_id, item_id, "item_id converges");

    let meta_b = stored.blob_ref.expect("B meta json");
    // #21b: verify that filename + mime survive the full rekey round-trip
    // (outbound stamps wire fields; inbound reads them to rebuild file meta).
    let (recovered_name, recovered_mime) =
        parse_file_name_mime(&meta_b).expect("B meta must carry filename+mime");
    assert_eq!(
        recovered_name, "notes.bin",
        "filename must survive outbound→wire→inbound"
    );
    assert_eq!(
        recovered_mime, "application/octet-stream",
        "mime must survive outbound→wire→inbound"
    );
    let file_id_b = crate::ipc::parse_image_file_id(&meta_b).expect("parse B file_id");
    assert_eq!(file_id_b, file_id_a);
    let b_chunks =
        chunks_from_blob(&stored.content.expect("B content")).expect("B chunks_from_blob");
    let recovered = decode_file(&b_chunks, &seed_b, &file_id_b).expect("B decode file");
    assert_eq!(recovered, raw, "B recovers A's exact file bytes");
}
