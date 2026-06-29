//! Pasteboard / image / file helper functions for paste-back and thumbnail
//! backfill.
//!
//! Extracted from `ipc.rs` for organisation — behaviour unchanged.
//! All public items are re-exported from `ipc/mod.rs`.

use anyhow::Context as _; // CopyPaste-crh3.90
use copypaste_core::{
    chunks_from_blob, decode_image, derive_v2, encode_thumbnail_from_png, set_thumb, FileMeta,
};

/// Internal error type for the paste-back path so the dispatcher can
/// distinguish authentication / decryption failures (which deserve a
/// dedicated error code so a tampered row is surfaced to the caller) from
/// generic write failures.
#[derive(Debug)]
#[allow(dead_code)]
pub(crate) enum PasteboardError {
    DecryptFailed(String),
    Other(String),
}

impl PasteboardError {
    pub(crate) fn decrypt(msg: impl Into<String>) -> Self {
        Self::DecryptFailed(msg.into())
    }
    pub(crate) fn other(msg: impl Into<String>) -> Self {
        Self::Other(msg.into())
    }
}

/// Parse the `file_id` field out of the JSON metadata embedded in an
/// image item's `blob_ref`. The metadata shape is produced by
/// `daemon::handle_image` (`{"width":...,"file_id":[u8; 16]}` — Rust
/// `{:?}` debug formatting of the byte array).
///
/// Lives here as `pub(crate)` (not behind `#[cfg(macos)]`) so the daemon's
/// image round-trip tests can drive the exact same read-path parser on any
/// host. Only the macOS `write_to_pasteboard` path calls it at runtime, hence
/// the dead-code allowance on non-macOS builds.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(crate) fn parse_image_file_id(meta_json: &str) -> Result<[u8; 16], String> {
    parse_meta_id_array(meta_json, "file_id")
}

/// Parse the thumbnail's distinct `thumb_file_id` (a 16-byte array) out of the
/// image `blob_ref` meta JSON. Mirrors [`parse_image_file_id`]; the thumbnail
/// is encrypted with the SAME content key but this SEPARATE id as AEAD AAD
/// (written additively by `clipboard::build_image_meta_json`). Backs the
/// `get_item_thumbnail` IPC verb.
pub(crate) fn parse_image_thumb_file_id(meta_json: &str) -> Result<[u8; 16], String> {
    parse_meta_id_array(meta_json, "thumb_file_id")
}

/// Parse the recorded `(thumb_w, thumb_h)` pixel dimensions out of an image
/// `blob_ref` meta JSON. Returns `(0, 0)` when either field is absent — legacy
/// rows written before the thumb-dim fields existed have no dims to compare,
/// so the caller treats `(0, 0)` as "unknown / do not regenerate on size".
///
/// Used to decide whether a *stored* thumbnail was encoded under an older,
/// larger [`copypaste_core::THUMBNAIL_MAX_DIM`] cap and must be regenerated
/// (HB-10). See [`copypaste_core::thumb_dims_exceed_cap`].
pub(crate) fn parse_image_thumb_dims(meta_json: &str) -> (u32, u32) {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(meta_json) else {
        return (0, 0);
    };
    let w = v
        .get("thumb_w")
        .and_then(|x| x.as_u64())
        .and_then(|n| u32::try_from(n).ok())
        .unwrap_or(0);
    let h = v
        .get("thumb_h")
        .and_then(|x| x.as_u64())
        .and_then(|n| u32::try_from(n).ok())
        .unwrap_or(0);
    (w, h)
}

/// Parse a named 16-byte array (e.g. `"file_id"` / `"thumb_file_id"`) out of an
/// image `blob_ref` meta JSON. Shared by [`parse_image_file_id`] and
/// [`parse_image_thumb_file_id`].
fn parse_meta_id_array(meta_json: &str, key: &str) -> Result<[u8; 16], String> {
    let value: serde_json::Value =
        serde_json::from_str(meta_json).map_err(|e| format!("image meta_json parse error: {e}"))?;
    let arr = value
        .get(key)
        .and_then(|v| v.as_array())
        .ok_or_else(|| format!("image meta_json missing '{key}' array"))?;
    if arr.len() != 16 {
        return Err(format!(
            "image meta_json '{key}' has wrong length: expected 16, got {}",
            arr.len()
        ));
    }
    let mut out = [0u8; 16];
    for (i, v) in arr.iter().enumerate() {
        out[i] = v
            .as_u64()
            .and_then(|n| u8::try_from(n).ok())
            .ok_or_else(|| format!("image meta_json '{key}[{i}]' not a u8"))?;
    }
    Ok(out)
}

/// Phase 4 lazy-backfill helper: generate and persist an encrypted thumbnail
/// for a legacy image item whose `thumb` column is NULL.
///
/// # Pipeline
/// 1. `chunks_from_blob(content)` + `decode_image(…)` → full-res PNG bytes.
/// 2. `encode_thumbnail_from_png(…)` → encrypted thumbnail blob + dimensions.
/// 3. `set_thumb(db, id, Some(&blob))` — write the blob to the DB (crash-safe:
///    a failed write just means we regenerate on the next display).
/// 4. Update `blob_ref` with `thumb_file_id` / `thumb_w` / `thumb_h` so the
///    normal decode path (`parse_image_thumb_file_id`) can find the AAD key.
///
/// Returns `(thumb_blob, updated_meta_json)` on success. The caller must
/// replace its in-scope `meta_json` with the returned `updated_meta_json` so
/// the subsequent `decode_thumbnail` call uses the correct `thumb_file_id`.
///
/// # Errors
/// Any step failure is returned as an `anyhow::Error`; the caller logs it and
/// falls back to the `{ "thumbnail": null }` sentinel — the request never
/// errors out.
pub(crate) fn lazy_backfill_thumbnail(
    db: &copypaste_core::Database,
    item_id: &str,
    content: &[u8],
    meta_json: &str,
    local_key: &[u8; 32],
    key_version: u8,
) -> Result<(Vec<u8>, String), anyhow::Error> {
    use copypaste_core::THUMBNAIL_MAX_DIM;
    let v2_key_backfill = derive_v2(local_key);
    let decode_key: &[u8; 32] = if key_version == 1 {
        local_key
    } else {
        &v2_key_backfill
    };

    // 1. Decrypt the full-resolution content to PNG bytes.
    let file_id = parse_image_file_id(meta_json)
        .map_err(|e| anyhow::anyhow!("backfill: file_id parse error: {e}"))?;
    let chunks = chunks_from_blob(content).context("backfill: chunks_from_blob failed")?;
    let png_bytes =
        decode_image(&chunks, decode_key, &file_id).context("backfill: decode_image failed")?;

    // 2. Derive the distinct thumb_file_id and encode the thumbnail.
    //    `image_thumb_file_id` is deterministic (SHA-256 domain-separated), so
    //    the same id is always derived for the same full-image file_id.
    let thumb_file_id = crate::clipboard::image_thumb_file_id(&file_id);
    let (thumb_blob, thumb_w, thumb_h) =
        encode_thumbnail_from_png(&png_bytes, decode_key, &thumb_file_id, THUMBNAIL_MAX_DIM)
            .context("backfill: encode_thumbnail_from_png failed")?;

    // 3. Persist the thumbnail blob.  A write failure is non-fatal: the item
    //    will just be regenerated on the next `get_item_thumbnail` call.
    if let Err(e) = set_thumb(db, item_id, Some(&thumb_blob)) {
        tracing::warn!(
            item_id = %item_id,
            err = %e,
            "backfill: set_thumb write failed (will regenerate next time)"
        );
    }

    // 4. Build the updated meta_json with the additive thumb fields and
    //    persist it.  Parse the existing meta to get the full-image fields so
    //    we can reconstruct the JSON in the canonical shape expected by
    //    `parse_image_file_id` and `get_item_image`.
    let updated_meta = build_updated_meta_json(meta_json, &thumb_file_id, thumb_w, thumb_h)
        .map_err(|e| anyhow::anyhow!("backfill: meta_json update failed: {e}"))?;
    if let Err(e) = db.conn().execute(
        "UPDATE clipboard_items SET blob_ref = ?1 WHERE id = ?2",
        rusqlite::params![updated_meta, item_id],
    ) {
        tracing::warn!(
            item_id = %item_id,
            err = %e,
            "backfill: blob_ref update failed (will regenerate next time)"
        );
    }

    Ok((thumb_blob, updated_meta))
}

/// Rebuild the image `blob_ref` meta JSON by injecting `thumb_file_id`,
/// `thumb_w`, and `thumb_h` into an existing legacy meta JSON that lacks them.
///
/// Preserves all existing keys (width, height, original_size, chunk_count,
/// file_id) and appends the three new thumbnail keys — identical in shape to
/// [`crate::clipboard::build_image_meta_json`].
///
/// Returns `Err` if the input JSON cannot be parsed or is missing required
/// fields.
fn build_updated_meta_json(
    meta_json: &str,
    thumb_file_id: &[u8; 16],
    thumb_w: u32,
    thumb_h: u32,
) -> Result<String, String> {
    let v: serde_json::Value =
        serde_json::from_str(meta_json).map_err(|e| format!("meta_json parse error: {e}"))?;

    // Pull out required fields; missing fields → error so the caller can
    // surface the backfill failure rather than writing a broken meta_json.
    let width = v
        .get("width")
        .and_then(|x| x.as_u64())
        .ok_or("meta_json missing 'width'")?;
    let height = v
        .get("height")
        .and_then(|x| x.as_u64())
        .ok_or("meta_json missing 'height'")?;
    let original_size = v
        .get("original_size")
        .and_then(|x| x.as_u64())
        .ok_or("meta_json missing 'original_size'")?;
    let chunk_count = v
        .get("chunk_count")
        .and_then(|x| x.as_u64())
        .ok_or("meta_json missing 'chunk_count'")?;
    let file_id = parse_meta_id_array(meta_json, "file_id")
        .map_err(|e| format!("meta_json missing 'file_id': {e}"))?;

    // Produce the same canonical shape as `clipboard::build_image_meta_json`.
    Ok(format!(
        r#"{{"width":{width},"height":{height},"original_size":{original_size},"chunk_count":{chunk_count},"file_id":{file_id:?},"thumb_file_id":{thumb_file_id:?},"thumb_w":{thumb_w},"thumb_h":{thumb_h}}}"#
    ))
}

/// Parse all file metadata fields out of the `blob_ref` JSON stored in a
/// `content_type == "file"` item. The JSON is produced by
/// [`crate::clipboard::build_file_meta_json`] and has the shape:
/// `{"filename":"...","mime":"...","original_size":N,"chunk_count":N,"file_id":[u8;16]}`.
///
/// Returns a [`copypaste_core::FileMeta`] so the caller can pass `file_id` to
/// `decode_file` and surface `filename`/`mime` over IPC. `pub(crate)` so it
/// is reachable from the inline tests (`parse_file_meta_round_trips_build_file_meta_json`).
pub(crate) fn parse_file_meta(meta_json: &str) -> Result<FileMeta, String> {
    let v: serde_json::Value =
        serde_json::from_str(meta_json).map_err(|e| format!("file meta_json parse error: {e}"))?;

    let filename = v
        .get("filename")
        .and_then(|s| s.as_str())
        .ok_or("file meta_json missing 'filename'")?
        .to_string();
    let mime = v
        .get("mime")
        .and_then(|s| s.as_str())
        .ok_or("file meta_json missing 'mime'")?
        .to_string();
    let original_size = v
        .get("original_size")
        .and_then(|n| n.as_u64())
        .ok_or("file meta_json missing or invalid 'original_size'")?;
    let chunk_count = v
        .get("chunk_count")
        .and_then(|n| n.as_u64())
        .and_then(|n| u32::try_from(n).ok())
        .ok_or("file meta_json missing or invalid 'chunk_count'")?;
    // file_id is stored as a JSON array of u8 values (same shape as
    // image meta's file_id, so parse_meta_id_array can be reused).
    let file_id = parse_meta_id_array(meta_json, "file_id")?;

    Ok(FileMeta {
        filename,
        mime,
        original_size,
        chunk_count,
        file_id,
    })
}

/// Map the daemon's internal `content_type` string to a macOS UTI suitable for
/// `setData:forType:`.
///
/// CopyPaste-c4q2.10: the mapping table is now owned by `copypaste-ipc`
/// ([`copypaste_ipc::map_content_type_to_uti`]) so it is a single, tested source
/// of truth shared with any client that needs a UTI. This is a thin re-export so
/// existing in-crate callers (`use super::map_content_type_to_uti`) keep working.
/// Gated to macOS to match the prior definition (the only caller is the macOS
/// paste-back path); the shared `copypaste-ipc` function itself is unconditional.
#[cfg(target_os = "macos")]
pub(crate) use copypaste_ipc::map_content_type_to_uti;

// ---------------------------------------------------------------------------
// File copy-back helpers
// ---------------------------------------------------------------------------

/// Returns the directory used to stage decrypted files for paste-back.
///
/// Path: `<cache_dir>/paste-files`  (e.g. `~/Library/Caches/CopyPaste/paste-files` on macOS).
///
/// The directory is created lazily in `write_file_to_paste_cache`; callers that
/// only need the path (e.g. [`prune_old_paste_files`]) do not require it to exist.
pub(crate) fn paste_file_cache_dir() -> std::path::PathBuf {
    crate::paths::cache_dir().join("paste-files")
}

/// Remove files in `dir` whose last-modified time is older than `PASTE_FILE_MAX_AGE_SECS`.
///
/// Called on every file copy-back so the staging directory does not grow
/// unbounded.  We do NOT delete immediately after paste because the receiving
/// app may read the file URL asynchronously (e.g. Finder copy).
///
/// Errors on individual entries are logged at DEBUG level and skipped; the
/// prune is best-effort and must never block the paste path.
pub(crate) fn prune_old_paste_files(dir: &std::path::Path) {
    /// Files older than this are eligible for deletion.
    const PASTE_FILE_MAX_AGE_SECS: u64 = 10 * 60; // 10 minutes

    let entries = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return, // nothing to prune
        Err(e) => {
            tracing::debug!("paste-files prune: read_dir({dir:?}) failed: {e}");
            return;
        }
    };
    let now = std::time::SystemTime::now();
    for entry in entries.flatten() {
        let path = entry.path();
        let mtime = match entry.metadata().and_then(|m| m.modified()) {
            Ok(t) => t,
            Err(e) => {
                tracing::debug!("paste-files prune: metadata({path:?}) failed: {e}");
                continue;
            }
        };
        let age = now.duration_since(mtime).unwrap_or_default();
        if age.as_secs() >= PASTE_FILE_MAX_AGE_SECS {
            if let Err(e) = std::fs::remove_file(&path) {
                tracing::debug!("paste-files prune: remove({path:?}) failed: {e}");
            }
        }
    }
}
