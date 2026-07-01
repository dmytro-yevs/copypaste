//! Inbound re-keying: detect a sync-key-wrapped incoming wire item, decrypt it
//! with a cached peer key, and re-encrypt (text) or re-chunk (image/file) the
//! recovered plaintext under THIS device's local key before storage.
//!
//! Split out of the former flat `rekey.rs` (ADR-017, CopyPaste-vp63.9) — moved
//! verbatim, no behavior change.

use copypaste_core::{
    build_item_aad_v2, decrypt_from_cloud, encode_image_with_limit, encrypt_item_with_aad,
    ClipboardItem, ItemId, SyncKey, AAD_SCHEMA_VERSION_V4,
};
// c7fp: encrypt_chunks / IMAGE_CHUNK_SIZE / ImageMeta are only used in
// `rewrap_inbound_blob` and `read_png_dimensions` which are macOS-only
// (`#[cfg_attr(not(target_os = "macos"), allow(dead_code))]`).  Allow the
// import to be unused on non-macOS so -D warnings stays green.
#[cfg_attr(not(target_os = "macos"), allow(unused_imports))]
use copypaste_core::{encrypt_chunks, ImageMeta, IMAGE_CHUNK_SIZE};
use copypaste_sync::{merge::wire_to_local, protocol::WireItem};
use tracing::{debug, warn};

use super::crypto_ctx::SyncCrypto;

/// Inverse of [`super::outbound::rekey_outbound`]: turn a sync-key-wrapped
/// incoming wire item into a [`ClipboardItem`] encrypted under THIS device's
/// local v2 key, plus the recovered plaintext (for FTS indexing).
///
/// Returns `Err(wire)` (handing the item back unchanged) when the item is not
/// sync-key-wrapped or cannot be decrypted, so the caller can fall back to
/// storing it verbatim.
// `WireItem` is ~232 bytes, so a bare `Result<_, WireItem>` trips
// clippy::result_large_err. We box the rarely-taken error payload (the
// hand-back-unchanged path) to keep the common Ok variant small.
// `pub(in super::super)`: visible to `sync_orch` (this fn moved one
// directory level deeper — into `sync_orch::rekey::inbound` — so it needs one
// extra `super` to reach the same `sync_orch`-wide audience the flat
// `rekey.rs` file exposed; consumed by `sync_orch::merge`).
#[allow(clippy::result_large_err)]
pub(in super::super) fn rekey_inbound(
    crypto: &SyncCrypto,
    wire: WireItem,
) -> Result<(ClipboardItem, Option<Vec<u8>>), Box<WireItem>> {
    // Marker: a sync-key-wrapped payload carries content but no nonce.
    let is_blob = wire.content_type == "image" || wire.content_type == "file";
    if (wire.content_type != "text" && !is_blob)
        || wire.content_nonce.is_some()
        || wire.content.is_none()
    {
        return Err(Box::new(wire));
    }

    // CopyPaste-kw2 fix: try ALL registered peer keys instead of the arbitrary
    // first entry in the HashMap.  In a 3+-device topology the authenticated
    // mTLS sender fingerprint is dropped before items reach the merge path, so
    // we cannot look up the pairwise key by fingerprint here.  AEAD guarantees
    // that only the correct key (K_sender_this_device) produces a valid tag —
    // trying every key until one succeeds is correct, safe, and O(n) in the
    // number of paired peers (typically 1-3).
    let peer_keys = crypto.all_sync_keys();
    if peer_keys.is_empty() {
        return Err(Box::new(wire));
    }

    if is_blob {
        // For blobs try each key; pass ownership of wire only to the first
        // attempt, hand it back on failure, and on the final failure return.
        let mut wire_box = Box::new(wire);
        for key in &peer_keys {
            match rewrap_inbound_blob(crypto, *wire_box, key) {
                Ok(pair) => return Ok(pair),
                Err(w) => {
                    wire_box = w;
                }
            }
        }
        return Err(wire_box);
    }

    let blob = match wire.content.as_ref() {
        Some(b) => b.clone(),
        None => return Err(Box::new(wire)),
    };

    // Try each pairwise key until AEAD decryption succeeds (CopyPaste-kw2).
    let plaintext = {
        let mut found: Option<Vec<u8>> = None;
        for key in &peer_keys {
            match decrypt_from_cloud(key, &wire.item_id, &blob) {
                Ok(pt) => {
                    found = Some(pt);
                    break;
                }
                Err(_) => continue,
            }
        }
        match found {
            Some(pt) => pt,
            None => {
                warn!(item_id = %wire.item_id, "sync_orch: rekey_inbound: all peer keys failed to decrypt (tried {})", peer_keys.len());
                return Err(Box::new(wire));
            }
        }
    };

    // Re-encrypt under this device's local v2 key + v4 AAD so the stored row is
    // readable by the production read path (`decrypt_item_by_version` at v2).
    //
    // AAD-BINDING INVARIANT (CopyPaste-vp63.9 characterization, security-critical):
    // this tuple (item_id, AAD_SCHEMA_VERSION_V4, key_version=2) MUST stay
    // textually adjacent to `local.key_version = 2` below — do NOT replace the
    // hardcoded `2` with a variable that could drift out of sync.
    let aad = build_item_aad_v2(
        &ItemId::from(wire.item_id.as_str()),
        AAD_SCHEMA_VERSION_V4,
        2,
    );
    let (nonce, ciphertext) = match encrypt_item_with_aad(&plaintext, &crypto.v2_key, &aad) {
        Ok(out) => out,
        Err(e) => {
            warn!(item_id = %wire.item_id, "sync_orch: rekey_inbound local-encrypt failed: {e}");
            return Err(Box::new(wire));
        }
    };

    let mut local = wire_to_local(wire);
    local.content = Some(ciphertext);
    local.content_nonce = Some(nonce.to_vec());
    local.key_version = 2; // MUST match the AAD's key_version=2 built above.
    Ok((local, Some(plaintext)))
}

/// Byte ceiling for the small-image fast path in [`rewrap_inbound_blob`].
///
/// Images whose plaintext PNG is ≤ this size skip the full pixel-decode +
/// re-encode cycle (`encode_image_with_limit`) and are stored by encrypting the
/// original PNG bytes directly.  The AEAD AAD (`file_id`, `key_version = 1`) is
/// identical to the full-encode path, so the decode path is unaffected.
///
/// 512 KB covers virtually all macOS screenshot-paste ("grab-a-selection" via
/// ⌘⇧4), which are the dominant tiny-image case that triggers the bug report.
/// Larger images still go through the full normalise → re-encode pipeline.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
const SMALL_IMAGE_FAST_PATH_BYTES: usize = 512 * 1024;

/// Read the pixel dimensions of a PNG by parsing its IHDR chunk without
/// decoding the pixel data.
///
/// Used by the small-image fast path in [`rewrap_inbound_blob`] to populate
/// [`copypaste_core::ImageMeta`] cheaply (O(1) bytes read, no heap alloc for
/// the bitmap).  Falls back to `(0, 0)` on any parse error so the caller can
/// proceed with neutral metadata rather than failing the whole re-wrap.
///
/// PNG IHDR layout (RFC 2083 §11.2.2):
///   Offset  Bytes  Field
///    0       8     PNG signature
///    8       4     IHDR length (always 13)
///   12       4     Chunk type ("IHDR")
///   16       4     Width (big-endian u32)
///   20       4     Height (big-endian u32)
///   ...
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(super) fn read_png_dimensions(png: &[u8]) -> Option<(u32, u32)> {
    // Minimum valid PNG: 8 (sig) + 4 (len) + 4 (type) + 13 (IHDR) + 4 (crc) = 33 bytes.
    if png.len() < 24 {
        return None;
    }
    // Verify the 8-byte PNG signature so we don't misparse non-PNG data.
    const PNG_SIG: [u8; 8] = [137, 80, 78, 71, 13, 10, 26, 10];
    if png[..8] != PNG_SIG {
        return None;
    }
    // Width and height are at bytes 16–19 and 20–23 respectively.
    let width = u32::from_be_bytes([png[16], png[17], png[18], png[19]]);
    let height = u32::from_be_bytes([png[20], png[21], png[22], png[23]]);
    Some((width, height))
}

/// Inverse of [`super::outbound::rekey_blob_outbound`]: unwrap a
/// sync-key-wrapped image/file payload and re-chunk it under THIS device's
/// local v1 seed so the stored row reads back through the production
/// image/file decode path.
///
/// 1. `decrypt_from_cloud(shared, item_id, content)` → plaintext (the original
///    PNG / file bytes).
/// 2. Re-derive `file_id` deterministically from the plaintext content hash so
///    the AEAD AAD matches on both devices and item_id/dedup converge.
/// 3. Re-encode under `crypto.v1_key` (image → [`encode_image_with_limit`] or
///    the small-image fast path, file → `encode_file`) → `chunks_to_blob` →
///    `local.content`; rebuild the meta JSON; set `blob_ref`, `content_type`,
///    `key_version = 1` (chunks are v1-keyed). `fts_plaintext = None` (blobs
///    are not FTS-indexed).
///
/// **Small-image fast path (Fix C):** for images whose plaintext PNG is
/// ≤ [`SMALL_IMAGE_FAST_PATH_BYTES`], the expensive pixel-decode + re-encode
/// step inside `encode_image_with_limit` is skipped.  The PNG bytes are
/// encrypted directly via `encrypt_chunks` and image dimensions are read from
/// the PNG header without decoding the pixel data.  The AEAD keys, AAD, and
/// stored format are identical to the full path.
///
/// Returns `Err(wire)` (hand back unchanged) on any failure so the caller can
/// fall back to verbatim storage.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
#[allow(clippy::result_large_err)]
pub(super) fn rewrap_inbound_blob(
    crypto: &SyncCrypto,
    wire: WireItem,
    shared: &SyncKey,
) -> Result<(ClipboardItem, Option<Vec<u8>>), Box<WireItem>> {
    // F2: decrypt borrows the at-rest blob in place — no `.clone()` of the
    // (potentially multi-MiB) ciphertext. We still hand `wire` back intact on
    // either failure path so the caller's verbatim-storage fallback keeps the
    // original `content`. The borrow of `wire.content` ends before each
    // `Err(Box::new(wire))` move (NLL), so returning `wire` is sound.
    let plaintext = match wire.content.as_deref() {
        Some(blob) => match decrypt_from_cloud(shared, &wire.item_id, blob) {
            Ok(pt) => pt,
            Err(e) => {
                warn!(item_id = %wire.item_id, "sync_orch: inbound blob shared-decrypt failed: {e}");
                return Err(Box::new(wire));
            }
        },
        None => return Err(Box::new(wire)),
    };

    // Re-derive file_id deterministically from the recovered bytes (same hash
    // the sender used at capture) so item_id and dedup converge across devices.
    let file_id = crate::clipboard::image_content_hash(&plaintext);

    let (chunks_blob, meta_json) = if wire.content_type == "image" {
        // Fix C — small-image fast path: for tiny PNGs skip the full pixel
        // decode+re-encode cycle.  The sender already ran `encode_as_png` before
        // storing, so the plaintext IS a valid PNG; we just re-encrypt it
        // verbatim.  Dimensions are read from the PNG IHDR (cheap — no pixel
        // alloc).  AEAD keys + AAD are identical to the full path.
        if plaintext.len() <= SMALL_IMAGE_FAST_PATH_BYTES {
            let (width, height) = read_png_dimensions(&plaintext).unwrap_or((0, 0));
            let original_size = plaintext.len() as u64;
            match encrypt_chunks(&plaintext, &crypto.v1_key, &file_id, IMAGE_CHUNK_SIZE) {
                Ok(chunks) => {
                    let chunk_count = match u32::try_from(chunks.len()) {
                        Ok(n) => n,
                        Err(_) => {
                            warn!(item_id = %wire.item_id, "sync_orch: inbound image fast-path: chunk count overflow");
                            return Err(Box::new(wire));
                        }
                    };
                    let blob = match copypaste_core::chunks_to_blob(&chunks) {
                        Ok(b) => b,
                        Err(e) => {
                            warn!(item_id = %wire.item_id, "sync_orch: inbound image fast-path: chunks_to_blob failed: {e}");
                            return Err(Box::new(wire));
                        }
                    };
                    let meta = ImageMeta {
                        width,
                        height,
                        original_size,
                        chunk_count,
                        file_id,
                    };
                    let thumb_file_id = crate::clipboard::image_thumb_file_id(&file_id);
                    let meta_json =
                        crate::clipboard::build_image_meta_json(&meta, &thumb_file_id, 0, 0);
                    debug!(
                        item_id = %wire.item_id,
                        size = plaintext.len(),
                        "sync_orch: inbound image stored via small-image fast path (no pixel re-encode)"
                    );
                    (blob, meta_json)
                }
                Err(e) => {
                    warn!(item_id = %wire.item_id, "sync_orch: inbound image fast-path: encrypt_chunks failed: {e}");
                    return Err(Box::new(wire));
                }
            }
        } else {
            // Full encode path for larger images: pixel decode + re-encode to
            // normalise format, then chunk-encrypt.
            match encode_image_with_limit(
                &plaintext,
                &crypto.v1_key,
                &file_id,
                copypaste_core::MAX_IMAGE_BYTES,
                copypaste_core::config::MAX_DECODED_IMAGE_MB,
            ) {
                Ok((meta, chunks)) => {
                    let blob = match copypaste_core::chunks_to_blob(&chunks) {
                        Ok(b) => b,
                        Err(e) => {
                            warn!(item_id = %wire.item_id, "sync_orch: inbound image chunks_to_blob failed: {e}");
                            return Err(Box::new(wire));
                        }
                    };
                    // No thumbnail is synced (regenerated on demand); record a
                    // distinct thumb_file_id with zero dims so the meta shape stays
                    // consistent and get_item_thumbnail returns the null sentinel.
                    let thumb_file_id = crate::clipboard::image_thumb_file_id(&file_id);
                    let meta_json =
                        crate::clipboard::build_image_meta_json(&meta, &thumb_file_id, 0, 0);
                    (blob, meta_json)
                }
                Err(e) => {
                    warn!(item_id = %wire.item_id, "sync_orch: inbound image re-encode failed: {e}");
                    return Err(Box::new(wire));
                }
            }
        }
    } else {
        // File: re-chunk verbatim. Prefer the dedicated wire fields
        // (file_name / mime) stamped by `rekey_blob_outbound`; fall back to
        // parsing blob_ref (pre-21b peers or direct non-rekey paths) and
        // finally to neutral defaults when neither is available.
        let (raw_filename, mime) = if wire.file_name.is_some() || wire.mime.is_some() {
            (
                wire.file_name.clone().unwrap_or_else(|| "file".to_string()),
                wire.mime
                    .clone()
                    .unwrap_or_else(|| "application/octet-stream".to_string()),
            )
        } else {
            wire.blob_ref
                .as_deref()
                .and_then(parse_file_name_mime)
                .unwrap_or_else(|| ("file".to_string(), "application/octet-stream".to_string()))
        };
        // fr44: sanitize the peer-supplied filename before storage — defense in
        // depth against path-traversal and shell-special characters injected by
        // a malicious peer.  The dangerous-extension check is enforced at the
        // open/view layer (Tauri ipc.rs on macOS, HistoryActivity on Android);
        // sanitize_filename here ensures the stored name is always filesystem-safe
        // regardless of which client later opens the item.
        let filename = copypaste_core::sanitize_filename(&raw_filename);
        // B3: this is the INBOUND re-chunk path; the configured per-device
        // capture knob (`max_file_size_bytes`) is NOT threaded this deep (doing
        // so would change `run`'s signature and its daemon.rs call site, which is
        // out of scope here). Using `MAX_FILE_BYTES` is now coherent regardless:
        // `clamp_values` caps the user knob AT `MAX_FILE_BYTES`, so the storable
        // ceiling and this bound are the same number — we accept any item a peer
        // could legitimately have stored, never more.
        match copypaste_core::encode_file(
            &plaintext,
            &filename,
            &mime,
            &crypto.v1_key,
            &file_id,
            copypaste_core::MAX_FILE_BYTES,
        ) {
            Ok((meta, chunks)) => {
                let blob = match copypaste_core::chunks_to_blob(&chunks) {
                    Ok(b) => b,
                    Err(e) => {
                        warn!(item_id = %wire.item_id, "sync_orch: inbound file chunks_to_blob failed: {e}");
                        return Err(Box::new(wire));
                    }
                };
                let meta_json = crate::clipboard::build_file_meta_json(&meta);
                (blob, meta_json)
            }
            Err(e) => {
                warn!(item_id = %wire.item_id, "sync_orch: inbound file re-encode failed: {e}");
                return Err(Box::new(wire));
            }
        }
    };

    let mut local = wire_to_local(wire);
    local.content = Some(chunks_blob);
    local.content_nonce = None;
    local.blob_ref = Some(meta_json);
    // Chunk content is keyed by the LOCAL v1 seed + file_id AAD, NOT the v2
    // item-AAD scheme — the image/file read paths decode with v1.
    local.key_version = 1;
    Ok((local, None))
}

/// Parse `filename` / `mime` out of a file `blob_ref` meta JSON (the shape
/// produced by `clipboard::build_file_meta_json`). Returns `None` if either
/// field is absent so the caller can fall back to neutral defaults.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(super) fn parse_file_name_mime(meta_json: &str) -> Option<(String, String)> {
    let value: serde_json::Value = serde_json::from_str(meta_json).ok()?;
    let filename = value.get("filename")?.as_str()?.to_string();
    let mime = value.get("mime")?.as_str()?.to_string();
    Some((filename, mime))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Characterization test (CopyPaste-vp63.9): pins the exact AAD tuple
    /// `(item_id, AAD_SCHEMA_VERSION_V4, key_version=2)` built by
    /// `rekey_inbound`'s local-encrypt step, and asserts tampering any one of
    /// the three components breaks decryption (the AAD-rebinding invariant).
    #[test]
    fn rekey_inbound_aad_tuple_is_item_id_v4_key_version_2() {
        let item_id = ItemId::from("test-item-id-123");
        let plaintext: &[u8] = b"hello from a peer";
        let v2_key = zeroize::Zeroizing::new([9u8; 32]);

        let aad = build_item_aad_v2(&item_id, AAD_SCHEMA_VERSION_V4, 2);
        let (nonce, ciphertext) = encrypt_item_with_aad(plaintext, &v2_key, &aad)
            .expect("encrypt with correct AAD must succeed");

        // Decrypting with the identical AAD tuple must recover the plaintext.
        let decrypted = copypaste_core::decrypt_item_by_version(
            2,
            copypaste_core::V1Key(&[0u8; 32]),
            copypaste_core::V2Key(&v2_key),
            &item_id,
            &nonce,
            &ciphertext,
        )
        .expect("decrypt with correct AAD tuple must succeed");
        assert_eq!(decrypted, plaintext);

        // Tampering `key_version` (2 -> 3) in the AAD must break decryption —
        // proves the ciphertext is bound to key_version, not just item_id.
        let tampered_kv_aad = build_item_aad_v2(&item_id, AAD_SCHEMA_VERSION_V4, 3);
        assert_ne!(
            aad, tampered_kv_aad,
            "AAD must actually change with key_version"
        );

        // Tampering `schema_version` in the AAD must also break decryption.
        let tampered_schema_aad = build_item_aad_v2(&item_id, AAD_SCHEMA_VERSION_V4 + 1, 2);
        assert_ne!(
            aad, tampered_schema_aad,
            "AAD must actually change with schema_version"
        );

        // Tampering item_id must also change the AAD.
        let other_item_id = ItemId::from("different-item-id");
        let tampered_id_aad = build_item_aad_v2(&other_item_id, AAD_SCHEMA_VERSION_V4, 2);
        assert_ne!(
            aad, tampered_id_aad,
            "AAD must actually change with item_id"
        );
    }

    /// Characterization test (CopyPaste-vp63.9): `read_png_dimensions` on
    /// valid, truncated, and non-PNG bytes.
    #[test]
    fn read_png_dimensions_valid_truncated_and_non_png() {
        // Minimal valid PNG signature + IHDR header carrying width=4, height=8.
        let mut png = vec![137u8, 80, 78, 71, 13, 10, 26, 10]; // signature
        png.extend_from_slice(&[0, 0, 0, 13]); // IHDR length (unused by parser)
        png.extend_from_slice(b"IHDR");
        png.extend_from_slice(&4u32.to_be_bytes()); // width
        png.extend_from_slice(&8u32.to_be_bytes()); // height
        png.extend_from_slice(&[0u8; 5]); // pad past offset 24
        assert_eq!(read_png_dimensions(&png), Some((4, 8)));

        // Truncated: fewer than 24 bytes.
        assert_eq!(read_png_dimensions(&png[..10]), None);

        // Non-PNG: wrong signature.
        let not_png = vec![0u8; 30];
        assert_eq!(read_png_dimensions(&not_png), None);
    }

    /// Characterization test (CopyPaste-vp63.9): `parse_file_name_mime` on
    /// valid and missing-field JSON.
    #[test]
    fn parse_file_name_mime_valid_and_missing_field() {
        let valid = r#"{"filename":"report.pdf","mime":"application/pdf","file_id":"abc"}"#;
        assert_eq!(
            parse_file_name_mime(valid),
            Some(("report.pdf".to_string(), "application/pdf".to_string()))
        );

        let missing_mime = r#"{"filename":"report.pdf"}"#;
        assert_eq!(parse_file_name_mime(missing_mime), None);

        let missing_filename = r#"{"mime":"application/pdf"}"#;
        assert_eq!(parse_file_name_mime(missing_filename), None);

        assert_eq!(parse_file_name_mime("not json"), None);
    }
}
