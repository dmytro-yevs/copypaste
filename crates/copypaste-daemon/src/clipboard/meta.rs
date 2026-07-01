//! Content identity + meta serialization helpers shared crate-wide via
//! `crate::clipboard::{image_content_hash, image_thumb_file_id,
//! build_image_meta_json, build_file_meta_json}`.
//!
//! Consumed by `sync_common.rs`, `sync_orch/rekey.rs`, `sync_orch/mod.rs`,
//! `ipc/pasteboard.rs`, `ipc/handlers_items.rs` and their tests — keep the
//! `crate::clipboard::` paths stable (re-exported from `clipboard/mod.rs`).

use sha2::{Digest, Sha256};

/// SHA-256 based content hash for image deduplication. Returns the first
/// 16 bytes of `SHA-256(raw)`, giving a 128-bit collision-resistant
/// fingerprint. Replaces the prior `DefaultHasher XOR nanos` scheme which
/// was non-deterministic and trivially collidable (security LOW #19).
pub fn image_content_hash(raw: &[u8]) -> [u8; 16] {
    let digest = Sha256::digest(raw);
    let mut out = [0u8; 16];
    out.copy_from_slice(&digest[..16]);
    out
}

/// Derive the thumbnail's `file_id` deterministically from the full image's
/// `file_id`. The thumbnail is encrypted with the SAME content key but a
/// DISTINCT `file_id` so its AEAD AAD is isolated from the full image's
/// (see `image::encode_image_full`). Domain-separating the hash (a `"thumb"`
/// prefix) guarantees the two ids never collide while staying deterministic,
/// so identical images still dedup and a reader can recompute / parse the id.
pub fn image_thumb_file_id(file_id: &[u8; 16]) -> [u8; 16] {
    let mut hasher = Sha256::new();
    hasher.update(b"copypaste-thumb-v1");
    hasher.update(file_id);
    let digest = hasher.finalize();
    let mut out = [0u8; 16];
    out.copy_from_slice(&digest[..16]);
    out
}

/// Build the image `blob_ref` meta JSON for an image item.
///
/// Keeps the original `width`/`height`/`original_size`/`chunk_count`/`file_id`
/// keys (consumed by `ipc::parse_image_file_id` and the full-res decode path)
/// and ADDITIVELY records the thumbnail's `thumb_file_id` (as a byte array, the
/// same shape as `file_id`) plus `thumb_w`/`thumb_h`. The core reader ignores
/// unknown keys, so this stays forward-/backward-compatible.
pub fn build_image_meta_json(
    meta: &copypaste_core::ImageMeta,
    thumb_file_id: &[u8; 16],
    thumb_w: u32,
    thumb_h: u32,
) -> String {
    format!(
        r#"{{"width":{},"height":{},"original_size":{},"chunk_count":{},"file_id":{:?},"thumb_file_id":{:?},"thumb_w":{},"thumb_h":{}}}"#,
        meta.width,
        meta.height,
        meta.original_size,
        meta.chunk_count,
        meta.file_id,
        thumb_file_id,
        thumb_w,
        thumb_h
    )
}

/// Build the file `blob_ref` meta JSON for a file item.
///
/// Carries the same `file_id` key the image meta uses (so the shared
/// `ipc::parse_image_file_id` parser recovers it for both content types) plus
/// the file-specific `filename`/`mime`/`original_size`/`chunk_count`. The core
/// reader ignores unknown keys, so this stays forward-/backward-compatible.
///
/// `filename` and `mime` are JSON-string-escaped via `serde_json` so arbitrary
/// names round-trip safely.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub fn build_file_meta_json(meta: &copypaste_core::FileMeta) -> String {
    // serde_json::to_string on a &str produces a correctly-escaped JSON string
    // literal (including the surrounding quotes); infallible for plain strings,
    // so the unwrap_or keeps us total without panicking.
    let filename = serde_json::to_string(&meta.filename).unwrap_or_else(|_| "\"\"".to_string());
    let mime = serde_json::to_string(&meta.mime).unwrap_or_else(|_| "\"\"".to_string());
    format!(
        r#"{{"filename":{},"mime":{},"original_size":{},"chunk_count":{},"file_id":{:?}}}"#,
        filename, mime, meta.original_size, meta.chunk_count, meta.file_id
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// security LOW #19 — image_dedup_uses_sha256.
    /// `image_content_hash` must be deterministic across calls and equal
    /// to the first 16 bytes of SHA-256(input). Different inputs must
    /// produce different hashes.
    #[test]
    fn image_dedup_uses_sha256() {
        let a = b"\x89PNG\r\n\x1a\n some image bytes";
        let b = b"\x89PNG\r\n\x1a\n some image bytes";
        let c = b"\x89PNG\r\n\x1a\n DIFFERENT bytes";

        let ha = image_content_hash(a);
        let hb = image_content_hash(b);
        let hc = image_content_hash(c);

        // Deterministic.
        assert_eq!(ha, hb);
        // Distinct inputs → distinct hashes (with overwhelming probability).
        assert_ne!(ha, hc);

        // Equals first 16 bytes of SHA-256.
        let expected = Sha256::digest(a);
        assert_eq!(&ha[..], &expected[..16]);
        assert_eq!(ha.len(), 16);
    }
}
