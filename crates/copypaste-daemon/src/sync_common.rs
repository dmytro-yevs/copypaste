//! Shared sync pipeline helpers used by BOTH the Supabase cloud path
//! ([`crate::cloud`]) and the relay-as-database path ([`crate::relay`]).
//!
//! These functions are the platform-independent crypto + storage glue:
//!
//! - **Upload side:** [`decrypt_item_plaintext`] (local ciphertext → plaintext)
//!   and [`wrap_and_check_cloud_upload_plaintext`] (prepend the file-identity
//!   header + enforce the sync size ceiling). The caller then runs
//!   `encrypt_for_cloud(sync_key, item_id, wrapped)` to produce the SAME opaque
//!   blob for either transport.
//! - **Download side:** [`build_local_item`] / [`build_local_blob_item`]
//!   (decrypted plaintext → a locally-re-encrypted [`ClipboardItem`]) and
//!   [`replace_cloud_item_by_item_id`] (atomic LWW in-place replace).
//! - [`decode_payload_ct`] decodes a PostgREST `bytea` (`\x<hex>`) or bare
//!   base64 ciphertext field.
//!
//! Extracted from `cloud.rs` so the relay client can reuse the byte-for-byte
//! identical envelope without pulling in `copypaste-supabase`. The module is
//! gated on `any(cloud-sync, relay-sync)`; `cloud.rs` re-imports every symbol
//! via `use crate::sync_common::*;` so its call sites and tests are unchanged.
//!
//! # Security
//! Never logs plaintext, key bytes, or ciphertext.

use copypaste_core::{
    build_item_aad_v2, decrypt_item_by_version, derive_v2, encrypt_item_with_aad,
    is_sensitive_for_autowipe, ClipboardItem, Database, AAD_SCHEMA_VERSION_V4,
    ITEM_KEY_VERSION_CURRENT,
};

// ── Cloud file-identity envelope (BUG C1) ──────────────────────────────────────
//
// Cloud / relay sync re-wraps a file's raw bytes under the sync key, but the
// wire schema carries only `content_type` — NOT the file's name/MIME. To
// preserve file identity end-to-end WITHOUT a schema change, we prepend a small
// self-describing header to the file bytes *before* `encrypt_for_cloud`, so
// name+MIME live INSIDE the encrypted plaintext (the relay/cloud only ever sees
// opaque ciphertext).
//
// Wire format (all multi-byte integers big-endian):
//   [1 byte  version = CLOUD_FILE_HEADER_VERSION]
//   [2 bytes name_len][name_len bytes UTF-8 file name]
//   [2 bytes mime_len][mime_len bytes UTF-8 MIME type]
//   [file bytes ...]
//
// Back-compat: a file uploaded by an OLD daemon has no header. On download we
// validate the version byte and both length fields against the buffer; if any
// check fails we treat the ENTIRE plaintext as raw file bytes with the legacy
// name="file" / mime="application/octet-stream" (the pre-fix behaviour).

/// Per-request HTTP timeout shared by all sync paths (cloud push/poll and
/// relay push/poll). 30 s is generous for single-row REST operations while
/// still bounding worst-case latency to a recoverable window. Without a
/// timeout, reqwest's default is infinite — one unresponsive endpoint would
/// block the whole sync loop permanently.
pub(crate) const SYNC_HTTP_TIMEOUT: std::time::Duration =
    std::time::Duration::from_secs(30);

/// Version byte for the cloud file-identity header. Bump only with a matching
/// decoder branch.
pub(crate) const CLOUD_FILE_HEADER_VERSION: u8 = 1;

/// Legacy fallback file name for headerless (old-daemon) file payloads.
pub(crate) const CLOUD_FILE_LEGACY_NAME: &str = "file";

/// Legacy fallback MIME for headerless (old-daemon) file payloads.
pub(crate) const CLOUD_FILE_LEGACY_MIME: &str = "application/octet-stream";

/// Decrypt a locally-stored [`ClipboardItem`]'s `content` field to plaintext
/// using the daemon's local key and the item's `key_version`.
///
/// Returns the raw plaintext bytes on success, or an error string for logging.
/// Never logs the plaintext or the key.
pub(crate) fn decrypt_item_plaintext(
    item: &ClipboardItem,
    local_key: &zeroize::Zeroizing<[u8; 32]>,
) -> Result<Vec<u8>, String> {
    // v0.6: image/file items store a multi-chunk blob encrypted under the LOCAL
    // v1 seed with `file_id` AAD (NOT the per-item v2 AAD). Reassemble them into
    // plaintext here so the cloud upload path re-wraps the SAME plaintext under
    // the sync key (identical wire contract to the P2P re-key path), then
    // enforce the sync ceiling so an oversized blob is rejected, not corrupted.
    if item.content_type == "image" || item.content_type == "file" {
        let meta_json = item
            .blob_ref
            .as_deref()
            .ok_or("blob item has no blob_ref")?;
        let file_id = crate::ipc::parse_image_file_id(meta_json)?;
        let content = item.content.as_deref().ok_or("blob item has no content")?;
        let chunks = copypaste_core::chunks_from_blob(content).map_err(|e| e.to_string())?;
        let v1_key: [u8; 32] = **local_key;
        let plaintext = if item.content_type == "image" {
            copypaste_core::decode_image(&chunks, &v1_key, &file_id).map_err(|e| e.to_string())?
        } else {
            copypaste_core::decode_file(&chunks, &v1_key, &file_id).map_err(|e| e.to_string())?
        };
        // NOTE: the cloud sync ceiling is enforced on the WRAPPED plaintext (after
        // `wrap_cloud_upload_plaintext` prepends the file name/MIME header), NOT on
        // this raw plaintext. The DOWNLOAD side (`build_local_blob_item`) checks the
        // same header-INCLUSIVE buffer, so checking the wrapped quantity keeps upload
        // and download symmetric — see `wrap_and_check_cloud_upload_plaintext`.
        return Ok(plaintext);
    }
    let content = item.content.as_deref().ok_or("item has no content")?;
    let nonce_vec = item
        .content_nonce
        .as_deref()
        .ok_or("item has no content_nonce")?;
    let nonce: &[u8; 24] = nonce_vec
        .try_into()
        .map_err(|_| format!("content_nonce wrong length: {}", nonce_vec.len()))?;
    let v1_key: [u8; 32] = **local_key;
    let v2_key = derive_v2(&v1_key);
    decrypt_item_by_version(
        item.key_version,
        &v1_key,
        &v2_key,
        &item.item_id,
        nonce,
        content,
    )
    .map_err(|e| e.to_string())
}

/// Prepend the cloud file-identity header to `file_bytes`.
///
/// `name`/`mime` longer than `u16::MAX` bytes are truncated on a UTF-8 char
/// boundary — these come from a captured file path / sniffed MIME and are in
/// practice far shorter, so the cap only guards the 2-byte length field.
pub(crate) fn encode_cloud_file_payload(name: &str, mime: &str, file_bytes: &[u8]) -> Vec<u8> {
    let name_b = truncate_utf8(name, u16::MAX as usize).as_bytes();
    let mime_b = truncate_utf8(mime, u16::MAX as usize).as_bytes();
    let mut out = Vec::with_capacity(1 + 2 + name_b.len() + 2 + mime_b.len() + file_bytes.len());
    out.push(CLOUD_FILE_HEADER_VERSION);
    // Lengths fit u16 by construction (truncate_utf8 bounds them).
    out.extend_from_slice(&(name_b.len() as u16).to_be_bytes());
    out.extend_from_slice(name_b);
    out.extend_from_slice(&(mime_b.len() as u16).to_be_bytes());
    out.extend_from_slice(mime_b);
    out.extend_from_slice(file_bytes);
    out
}

/// Truncate `s` to at most `max` bytes on a UTF-8 char boundary.
fn truncate_utf8(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// Parse a cloud file payload into `(file_bytes, name, mime)`.
///
/// Returns the embedded name/MIME when a valid header is present; otherwise
/// (old-daemon payload, or any malformed/overrunning header) treats the WHOLE
/// buffer as raw file bytes with the legacy name/MIME — never panics.
pub(crate) fn decode_cloud_file_payload(payload: &[u8]) -> (Vec<u8>, String, String) {
    let legacy = || {
        (
            payload.to_vec(),
            CLOUD_FILE_LEGACY_NAME.to_string(),
            CLOUD_FILE_LEGACY_MIME.to_string(),
        )
    };
    // Smallest valid header: version + 2 zero-len fields = 5 bytes.
    if payload.len() < 5 || payload[0] != CLOUD_FILE_HEADER_VERSION {
        return legacy();
    }
    let mut pos = 1usize;
    let read_field = |buf: &[u8], pos: &mut usize| -> Option<String> {
        if *pos + 2 > buf.len() {
            return None;
        }
        let len = u16::from_be_bytes([buf[*pos], buf[*pos + 1]]) as usize;
        *pos += 2;
        if *pos + len > buf.len() {
            return None;
        }
        let s = std::str::from_utf8(&buf[*pos..*pos + len])
            .ok()?
            .to_string();
        *pos += len;
        Some(s)
    };
    let name = match read_field(payload, &mut pos) {
        Some(s) => s,
        None => return legacy(),
    };
    let mime = match read_field(payload, &mut pos) {
        Some(s) => s,
        None => return legacy(),
    };
    (payload[pos..].to_vec(), name, mime)
}

/// Read a file item's `(file_name, mime)` from its local `blob_ref` meta JSON.
///
/// Mirrors the source the P2P / IPC paths use (`parse_file_meta`). Falls back to
/// the legacy name/MIME if the meta is missing or unparseable so a malformed row
/// still uploads (just without identity) rather than being dropped.
fn file_identity_from_item(item: &ClipboardItem) -> (String, String) {
    match item.blob_ref.as_deref() {
        Some(meta_json) => match crate::ipc::parse_file_meta(meta_json) {
            Ok(meta) => (meta.filename, meta.mime),
            Err(e) => {
                tracing::warn!(
                    "sync: file id={} blob_ref meta unparseable ({e}); \
                     uploading with legacy name/mime",
                    item.id
                );
                (
                    CLOUD_FILE_LEGACY_NAME.to_string(),
                    CLOUD_FILE_LEGACY_MIME.to_string(),
                )
            }
        },
        None => (
            CLOUD_FILE_LEGACY_NAME.to_string(),
            CLOUD_FILE_LEGACY_MIME.to_string(),
        ),
    }
}

/// Wrap a decrypted plaintext for cloud upload.
///
/// For `content_type == "file"` this prepends the [`encode_cloud_file_payload`]
/// header (name+MIME read from the item's local `blob_ref`). For every other
/// type the plaintext is returned unchanged.
pub(crate) fn wrap_cloud_upload_plaintext(item: &ClipboardItem, plaintext: Vec<u8>) -> Vec<u8> {
    if item.content_type == "file" {
        let (name, mime) = file_identity_from_item(item);
        encode_cloud_file_payload(&name, &mime, &plaintext)
    } else {
        plaintext
    }
}

/// Wrap a decrypted plaintext for cloud upload and enforce the sync ceiling on
/// the WRAPPED bytes (the exact bytes that get encrypted and shipped).
///
/// Returns `Err` (caller logs a `warn!` and skips the item) when the wrapped
/// payload exceeds the ceiling — never panics, never silently drops.
pub(crate) fn wrap_and_check_cloud_upload_plaintext(
    item: &ClipboardItem,
    plaintext: Vec<u8>,
) -> Result<Vec<u8>, String> {
    let wrapped = wrap_cloud_upload_plaintext(item, plaintext);
    let ceiling = crate::sync_orch::SYNC_MAX_BLOB_BYTES;
    if wrapped.len() > ceiling {
        return Err(format!(
            "wrapped blob {} bytes exceeds cloud sync ceiling {ceiling}",
            wrapped.len()
        ));
    }
    Ok(wrapped)
}

/// Decode a `payload_ct` value into the raw ciphertext blob (nonce||ciphertext).
///
/// PostgREST renders `bytea` in hex output form (`\x<hex>`); we also accept a
/// bare base64 string (the relay envelope's `ct_b64`, and pre-fix Supabase rows).
pub(crate) fn decode_payload_ct(payload_ct: &str) -> Result<Vec<u8>, String> {
    use base64::Engine as _;
    if let Some(hexpart) = payload_ct.strip_prefix("\\x") {
        return hex::decode(hexpart).map_err(|e| format!("hex decode: {e}"));
    }
    base64::engine::general_purpose::STANDARD
        .decode(payload_ct)
        .map_err(|e| format!("base64 decode: {e}"))
}

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
    // ITEM_KEY_VERSION_CURRENT is i64 (storage convention); build_item_aad_v2
    // takes u32 and ClipboardItem.key_version is u8 — cast explicitly.
    // Value is 2 (v2 HKDF key), which fits both u32 and u8.
    let aad = build_item_aad_v2(
        item_id,
        AAD_SCHEMA_VERSION_V4,
        ITEM_KEY_VERSION_CURRENT as u32,
    );
    let (nonce, ciphertext) =
        encrypt_item_with_aad(plaintext, &v2_key, &aad).map_err(|e| e.to_string())?;

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
        id: id.to_owned(),
        item_id: item_id.to_owned(),
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
        id: id.to_owned(),
        item_id: item_id.to_owned(),
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

/// Atomically replace a cloud/relay-downloaded clipboard row by its cross-device
/// `item_id`, preserving the row's primary key (`item.id`) so FTS / copy_item /
/// pins keep pointing at the same row.
///
/// Runs DELETE-by-item_id + INSERT inside one `unchecked_transaction` so a
/// failed insert rolls back the delete and the prior row survives.
pub(crate) fn replace_cloud_item_by_item_id(
    db: &Database,
    item: &ClipboardItem,
) -> anyhow::Result<()> {
    use rusqlite::params;
    let tx = db.conn().unchecked_transaction()?;
    tx.execute(
        "DELETE FROM clipboard_items WHERE item_id = ?1",
        params![item.item_id],
    )?;
    tx.execute(
        "INSERT INTO clipboard_items
         (id, item_id, content_type, content, content_nonce, blob_ref,
          is_sensitive, is_synced, lamport_ts, wall_time, expires_at, app_bundle_id,
          content_hash, origin_device_id, key_version, pinned, pin_order)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17)",
        params![
            item.id,
            item.item_id,
            item.content_type,
            item.content,
            item.content_nonce,
            item.blob_ref,
            item.is_sensitive as i64,
            item.is_synced as i64,
            item.lamport_ts,
            item.wall_time,
            item.expires_at,
            item.app_bundle_id,
            item.content_hash,
            item.origin_device_id,
            // Use the item's own key_version rather than the current constant
            // so cloud-synced items retain the key generation they were
            // encrypted with. ITEM_KEY_VERSION_CURRENT would silently stamp
            // v2 on v1-keyed chunks, poisoning future migration dispatches.
            item.key_version as i64,
            item.pinned as i64,
            item.pin_order,
        ],
    )?;
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_envelope_roundtrip() {
        let name = "report.pdf";
        let mime = "application/pdf";
        let file_bytes = b"%PDF-1.7 fake body".to_vec();
        let wrapped = encode_cloud_file_payload(name, mime, &file_bytes);
        assert_eq!(wrapped[0], CLOUD_FILE_HEADER_VERSION);
        let (rb, rn, rm) = decode_cloud_file_payload(&wrapped);
        assert_eq!(rb, file_bytes);
        assert_eq!(rn, name);
        assert_eq!(rm, mime);
    }

    #[test]
    fn file_envelope_empty_fields() {
        let file_bytes = b"raw".to_vec();
        let wrapped = encode_cloud_file_payload("", "", &file_bytes);
        let (rb, rn, rm) = decode_cloud_file_payload(&wrapped);
        assert_eq!(rb, file_bytes);
        assert_eq!(rn, "");
        assert_eq!(rm, "");
    }

    #[test]
    fn headerless_payload_falls_back_to_legacy() {
        let raw = b"not a header at all, just bytes".to_vec();
        let (bytes, name, mime) = decode_cloud_file_payload(&raw);
        assert_eq!(bytes, raw);
        assert_eq!(name, CLOUD_FILE_LEGACY_NAME);
        assert_eq!(mime, CLOUD_FILE_LEGACY_MIME);
    }

    #[test]
    fn malformed_header_falls_back_to_legacy() {
        // version byte present but name_len overruns the buffer.
        let malformed = vec![CLOUD_FILE_HEADER_VERSION, 0xFF, 0xFF, 0x00];
        let (bytes, name, mime) = decode_cloud_file_payload(&malformed);
        assert_eq!(bytes, malformed);
        assert_eq!(name, CLOUD_FILE_LEGACY_NAME);
        assert_eq!(mime, CLOUD_FILE_LEGACY_MIME);
    }

    #[test]
    fn decode_payload_ct_accepts_hex_and_base64() {
        use base64::Engine as _;
        let blob = vec![0xde, 0xad, 0xbe, 0xef];
        // PostgREST hex form
        let hexform = format!("\\x{}", hex::encode(&blob));
        assert_eq!(decode_payload_ct(&hexform).unwrap(), blob);
        // bare base64 form (relay envelope ct_b64)
        let b64 = base64::engine::general_purpose::STANDARD.encode(&blob);
        assert_eq!(decode_payload_ct(&b64).unwrap(), blob);
    }

    /// LWW fix: replace_cloud_item_by_item_id must store the item's own
    /// key_version, not the hardcoded ITEM_KEY_VERSION_CURRENT constant.
    /// A v1-keyed chunk item replaced via cloud LWW must survive as v1 so
    /// future migration dispatch can identify and re-encrypt it correctly.
    #[test]
    fn replace_cloud_item_preserves_key_version() {
        use copypaste_core::{get_item_by_item_id, insert_item, Database};

        let db = Database::open_in_memory().expect("in-memory DB");

        // Seed a v2 item that the remote will overwrite via LWW.
        let seed = ClipboardItem {
            id: "local-row-id".to_string(),
            item_id: "shared-item-id".to_string(),
            content_type: "text".to_string(),
            content: Some(b"old ciphertext".to_vec()),
            content_nonce: Some(vec![0u8; 24]),
            blob_ref: None,
            is_sensitive: false,
            is_synced: true,
            lamport_ts: 1,
            wall_time: 1_700_000_000_000,
            expires_at: None,
            app_bundle_id: None,
            content_hash: None,
            origin_device_id: "local-device".to_string(),
            key_version: 2,
            pinned: false,
            pin_order: None,
            thumb: None,
            deleted: false,
        };
        insert_item(&db, &seed).expect("insert seed");

        // Build a replacement that is v1-keyed (chunk from an older peer).
        let replacement = ClipboardItem {
            id: "local-row-id".to_string(),
            item_id: "shared-item-id".to_string(),
            content_type: "file".to_string(),
            content: None,
            content_nonce: None,
            blob_ref: Some("blob-abc".to_string()),
            is_sensitive: false,
            is_synced: true,
            lamport_ts: 2,
            wall_time: 1_700_000_001_000,
            expires_at: None,
            app_bundle_id: None,
            content_hash: None,
            origin_device_id: "remote-device".to_string(),
            key_version: 1, // <-- must survive the LWW replace
            pinned: false,
            pin_order: None,
            thumb: None,
            deleted: false,
        };

        replace_cloud_item_by_item_id(&db, &replacement).expect("replace");

        let stored = get_item_by_item_id(&db, "shared-item-id")
            .expect("query ok")
            .expect("row exists");

        assert_eq!(
            stored.key_version, 1,
            "replace_cloud_item_by_item_id must persist item.key_version, not ITEM_KEY_VERSION_CURRENT"
        );
    }
}
