//! Local rebuild (download side): turn cloud/relay-decrypted plaintext back
//! into a locally-storable [`ClipboardItem`] — symmetric with
//! `sync_orch::rewrap_inbound_blob`.
//!
//! Split out of the former flat `sync_common.rs` (ADR-017, CopyPaste-vp63.7)
//! — moved verbatim, no behavior change.

use copypaste_core::{
    derive_v2, is_sensitive_for_autowipe, ClipboardItem, ITEM_KEY_VERSION_CURRENT,
};

use super::envelope::{decode_cloud_file_payload, CLOUD_FILE_LEGACY_MIME, CLOUD_FILE_LEGACY_NAME};
use super::local_crypto::encrypt_v2_for_local_storage;

/// Build a local [`ClipboardItem`] from decrypted plaintext by re-encrypting
/// it with the daemon's local key (v2 HKDF path, `key_version = 2`).
// Cloud/relay items carry several independent metadata fields (timestamps, ids,
// type, key material) that do not group naturally without adding an intermediate
// struct. The function is internal-only; a struct parameter would add indirection
// without clarity benefit.
#[allow(clippy::too_many_arguments)]
pub(crate) fn build_local_item(
    id: &str,
    item_id: &str,
    content_type: &str,
    plaintext: &[u8],
    lamport_ts: i64,
    wall_time: i64,
    expires_at: Option<i64>,
    app_bundle_id: Option<String>,
    origin_device_id: String,
    local_key: &zeroize::Zeroizing<[u8; 32]>,
) -> Result<ClipboardItem, String> {
    // v0.6: image/file payloads arrive as a single sync-key-wrapped plaintext
    // (PNG / raw bytes). Re-chunk them under THIS device's LOCAL v1 seed and
    // rebuild the meta JSON so the stored row reads back through the production
    // image/file decode path — symmetric with sync_orch::rewrap_inbound_blob.
    if content_type == "image" || content_type == "file" {
        return build_local_blob_item(
            id,
            item_id,
            content_type,
            plaintext,
            lamport_ts,
            wall_time,
            expires_at,
            app_bundle_id,
            origin_device_id,
            local_key,
        );
    }
    if content_type != "text" {
        return Err(format!(
            "unsupported content_type '{content_type}' for cloud download"
        ));
    }
    let v1_key: [u8; 32] = **local_key;
    let v2_key = derive_v2(&v1_key);
    let (nonce, ciphertext) =
        encrypt_v2_for_local_storage(item_id, plaintext, &v2_key).map_err(|e| e.to_string())?;

    // Fix CLOUD-SENSITIVE: run the same auto-wipe gate as the clipboard capture
    // path (daemon handle_text) so cross-device sensitive items are flagged for
    // auto-wipe on the receiving device using the SAME confidence floor (>=0.70).
    let is_sensitive = if content_type == "text" {
        let text = std::str::from_utf8(plaintext).unwrap_or("");
        is_sensitive_for_autowipe(text)
    } else {
        false
    };

    Ok(ClipboardItem {
        id: id.into(),
        item_id: item_id.into(),
        content_type: content_type.to_owned(),
        content: Some(ciphertext),
        content_nonce: Some(nonce.to_vec()),
        blob_ref: None,
        is_sensitive,
        is_synced: true,
        lamport_ts,
        wall_time,
        expires_at,
        app_bundle_id,
        content_hash: None,
        origin_device_id,
        key_version: ITEM_KEY_VERSION_CURRENT as u8,
        pinned: false,
        // pin_order is a local-only ordering field, not carried over cloud sync.
        pin_order: None,
        // thumb is a local-only image thumbnail (schema v9); cloud download is
        // text-only here, so it never carries one.
        thumb: None,
        // Cloud-downloaded items are always live; tombstones are handled by the
        // caller before constructing a ClipboardItem.
        deleted: false,
    })
}

/// Build a local image/file [`ClipboardItem`] from decrypted plaintext by
/// re-chunking it under the daemon's LOCAL v1 seed (the chunk-encryption key,
/// keyed by a deterministically re-derived `file_id`) and rebuilding the
/// `blob_ref` meta JSON. Symmetric with `sync_orch::rewrap_inbound_blob`; the
/// stored row reads back through the production image/file decode path.
#[allow(clippy::too_many_arguments)]
fn build_local_blob_item(
    id: &str,
    item_id: &str,
    content_type: &str,
    plaintext: &[u8],
    lamport_ts: i64,
    wall_time: i64,
    expires_at: Option<i64>,
    app_bundle_id: Option<String>,
    origin_device_id: String,
    local_key: &zeroize::Zeroizing<[u8; 32]>,
) -> Result<ClipboardItem, String> {
    let ceiling = crate::sync_orch::SYNC_MAX_BLOB_BYTES;
    if plaintext.len() > ceiling {
        return Err(format!(
            "inbound blob {} bytes exceeds cloud sync ceiling {ceiling}",
            plaintext.len()
        ));
    }
    let v1_key: [u8; 32] = **local_key;

    // BUG C1: a downloaded FILE payload may carry a self-describing header
    // (version + name + mime) prepended by the sender before cloud encryption.
    // Strip it and recover the original name/MIME; a headerless (old-daemon)
    // payload decodes as raw bytes with the legacy name/MIME. We re-bind file_id
    // and the meta to the header-STRIPPED bytes so the local row reads back as
    // the true file content.
    let (file_plaintext, file_name, file_mime) = if content_type == "file" {
        decode_cloud_file_payload(plaintext)
    } else {
        // Images carry no header; keep the plaintext as-is (owned for a uniform
        // type below).
        (
            plaintext.to_vec(),
            CLOUD_FILE_LEGACY_NAME.to_string(),
            CLOUD_FILE_LEGACY_MIME.to_string(),
        )
    };

    // Re-derive file_id from the (header-stripped) plaintext so item_id/dedup
    // converge with the sender (the chunk AEAD binds file_id as AAD).
    let file_id = crate::clipboard::image_content_hash(&file_plaintext);

    let (content, blob_ref) = if content_type == "image" {
        let (meta, chunks) = copypaste_core::encode_image_with_limit(
            plaintext,
            &v1_key,
            &file_id,
            copypaste_core::MAX_IMAGE_BYTES,
            copypaste_core::config::MAX_DECODED_IMAGE_MB,
        )
        .map_err(|e| e.to_string())?;
        let blob = copypaste_core::chunks_to_blob(&chunks).map_err(|e| e.to_string())?;
        let thumb_file_id = crate::clipboard::image_thumb_file_id(&file_id);
        let meta_json = crate::clipboard::build_image_meta_json(&meta, &thumb_file_id, 0, 0);
        (blob, meta_json)
    } else {
        // BUG C1: re-chunk the header-STRIPPED bytes and restore the original
        // name/MIME recovered from the envelope (legacy fallback for headerless
        // payloads). encode_file rejects an empty filename, so the legacy "file"
        // default also guards the empty-name edge.
        let name = if file_name.is_empty() {
            CLOUD_FILE_LEGACY_NAME
        } else {
            &file_name
        };
        let mime = if file_mime.is_empty() {
            CLOUD_FILE_LEGACY_MIME
        } else {
            &file_mime
        };
        let (meta, chunks) = copypaste_core::encode_file(
            &file_plaintext,
            name,
            mime,
            &v1_key,
            &file_id,
            copypaste_core::MAX_FILE_BYTES,
        )
        .map_err(|e| e.to_string())?;
        let blob = copypaste_core::chunks_to_blob(&chunks).map_err(|e| e.to_string())?;
        let meta_json = crate::clipboard::build_file_meta_json(&meta);
        (blob, meta_json)
    };

    Ok(ClipboardItem {
        id: id.into(),
        item_id: item_id.into(),
        content_type: content_type.to_owned(),
        content: Some(content),
        // Chunks are self-framed per-chunk; there is no item-level nonce.
        content_nonce: None,
        blob_ref: Some(blob_ref),
        is_sensitive: false,
        is_synced: true,
        lamport_ts,
        wall_time,
        expires_at,
        app_bundle_id,
        content_hash: None,
        origin_device_id,
        // Chunk content is v1-keyed (local seed + file_id AAD), not the v2
        // item-AAD scheme used for text.
        key_version: 1,
        pinned: false,
        pin_order: None,
        // Thumbnail is regenerated locally on demand, never synced.
        thumb: None,
        // Cloud-downloaded items are always live; tombstones are handled before
        // this function is called.
        deleted: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use copypaste_core::{decrypt_item_by_version, V1Key, V2Key};

    /// Characterization test (CopyPaste-vp63.7 gap): `build_local_item` for
    /// text encrypts under this device's v2 key so the production read path
    /// (`decrypt_item_by_version`) recovers the original plaintext, and flags
    /// `is_sensitive` per `is_sensitive_for_autowipe`.
    #[test]
    fn build_local_item_text_round_trips_and_flags_sensitive() {
        let local_key = zeroize::Zeroizing::new([0x33u8; 32]);
        let plaintext = "AKIAIOSFODNN7EXAMPLE"; // a real AWS key => sensitive

        let item = build_local_item(
            "row-1",
            "item-1",
            "text",
            plaintext.as_bytes(),
            1,
            1_700_000_000_000,
            None,
            None,
            "device-a".to_string(),
            &local_key,
        )
        .expect("build_local_item must succeed");

        assert_eq!(item.key_version, 2);
        assert!(item.is_sensitive, "AWS key must be flagged sensitive");

        let v1_key: [u8; 32] = *local_key;
        let v2_key = derive_v2(&v1_key);
        let nonce_vec = item.content_nonce.clone().expect("nonce");
        let nonce: [u8; copypaste_core::NONCE_SIZE] = nonce_vec.try_into().expect("nonce len");
        let decrypted = decrypt_item_by_version(
            item.key_version,
            V1Key(&v1_key),
            V2Key(&v2_key),
            &item.item_id,
            &nonce,
            item.content.as_ref().expect("content"),
        )
        .expect("decrypt with production read path must succeed");
        assert_eq!(decrypted, plaintext.as_bytes());
    }

    /// Characterization test (CopyPaste-vp63.7 gap): a non-text/image/file
    /// `content_type` is rejected with a descriptive error.
    #[test]
    fn build_local_item_rejects_unsupported_content_type() {
        let local_key = zeroize::Zeroizing::new([0x33u8; 32]);
        let err = build_local_item(
            "row-1",
            "item-1",
            "carrier-pigeon",
            b"whatever",
            1,
            1,
            None,
            None,
            "device-a".to_string(),
            &local_key,
        )
        .expect_err("unsupported content_type must error");
        assert!(err.contains("unsupported content_type"), "got: {err}");
    }

    /// Characterization test (CopyPaste-vp63.7 gap): `build_local_item` for
    /// "file" decodes back through the production file path and recovers the
    /// BUG C1 file-identity header's name/mime + stripped body.
    #[test]
    fn build_local_item_file_recovers_header_stripped_body_and_identity() {
        use copypaste_core::{chunks_from_blob, decode_file};

        let local_key = zeroize::Zeroizing::new([0x44u8; 32]);
        let raw_body = b"the actual file bytes";
        let wrapped =
            super::super::envelope::encode_cloud_file_payload("notes.txt", "text/plain", raw_body);

        let item = build_local_item(
            "row-2",
            "item-2",
            "file",
            &wrapped,
            1,
            1,
            None,
            None,
            "device-a".to_string(),
            &local_key,
        )
        .expect("build_local_item (file) must succeed");

        assert_eq!(item.key_version, 1);
        let meta_json = item.blob_ref.expect("blob_ref must be set");
        // `parse_file_name_mime` lives in `sync_orch::rekey` at `pub(super)`
        // visibility (not reachable from this crate-sibling module) — parse
        // the meta JSON directly instead of duplicating that helper here.
        let v: serde_json::Value = serde_json::from_str(&meta_json).unwrap();
        let name = v.get("filename").unwrap().as_str().unwrap().to_string();
        let mime = v.get("mime").unwrap().as_str().unwrap().to_string();
        assert_eq!(name, "notes.txt");
        assert_eq!(mime, "text/plain");

        let v1_key: [u8; 32] = *local_key;
        let file_id = crate::clipboard::image_content_hash(raw_body);
        let chunks = chunks_from_blob(item.content.as_ref().expect("content")).expect("chunks");
        let decoded = decode_file(&chunks, &v1_key, &file_id).expect("decode_file");
        assert_eq!(
            decoded, raw_body,
            "file body must survive header-strip + re-chunk"
        );
    }
}
