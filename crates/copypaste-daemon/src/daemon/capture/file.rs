//! File capture ingest: chunk + encrypt raw file bytes verbatim (no
//! decode/re-encode, unlike images) and store.

use copypaste_core::{
    chunks_to_blob, encode_file, insert_item_with_fts, AppConfig, ClipboardItem, Database,
};
use std::sync::Arc;
use tokio::sync::Mutex;

use super::cleanup::prune_history;

/// Encrypt and store a freshly-captured file for at-rest storage.
///
/// Mirrors [`super::image::handle_image`] but uses [`copypaste_core::encode_file`]
/// (no decode/re-encode — the raw bytes are chunked verbatim). The `file_id` is
/// derived from SHA-256(raw_bytes)[..16] so identical files dedup across
/// devices. The `item_id` is set to `uuid::Uuid::from_bytes(file_id)` for the
/// same reason (cross-device CRDT identity, mirrors the image path).
///
/// The meta JSON produced by [`crate::clipboard::build_file_meta_json`] uses
/// the keys `filename`, `mime`, `original_size`, `chunk_count`, `file_id` —
/// identical to the keys expected by `ipc::parse_file_meta` and
/// `sync_orch::build_file_meta_json`.
pub(crate) async fn handle_file(
    raw_bytes: Vec<u8>,
    filename: String,
    mime: String,
    db: &Arc<Mutex<Database>>,
    local_key: &[u8; 32],
    config: &AppConfig,
    local_device_id: &str,
    // mtf5 (PG-22): bundle ID of the frontmost app at capture time.
    source_bundle_id: Option<String>,
) -> Option<ClipboardItem> {
    let db = db.clone();
    let config = config.clone();
    let local_key = *local_key;
    let local_device_id = local_device_id.to_string();
    // mtf5 (PG-22): pre-compute before move into blocking closure.
    let app_is_sensitive_file = source_bundle_id
        .as_deref()
        .map(copypaste_core::is_sensitive_app)
        .unwrap_or(false);
    let join = tokio::task::spawn_blocking(move || {
        // Content-hash file_id: deterministic so identical files dedup.
        let file_id = crate::clipboard::image_content_hash(&raw_bytes);

        let max_file_bytes = usize::try_from(config.max_file_size_bytes).unwrap_or(usize::MAX);

        let v2_key = copypaste_core::derive_v2(&local_key);
        match encode_file(
            &raw_bytes,
            &filename,
            &mime,
            &v2_key,
            &file_id,
            max_file_bytes,
        ) {
            Ok((meta, chunks)) => {
                let blob = match chunks_to_blob(&chunks) {
                    Ok(b) => b,
                    Err(e) => {
                        tracing::error!(error = %e, "handle_file: chunks_to_blob failed");
                        return None;
                    }
                };
                let meta_json = crate::clipboard::build_file_meta_json(&meta);
                let mut item = ClipboardItem::new_file(blob, meta_json, 0);
                // CopyPaste-ojhe: stamp the unified lamport value space at
                // capture (`next_lamport_ts(0, wall_time) == wall_time`) instead
                // of a hardcoded 0, so a fresh capture is time-ordered under
                // lamport-first LWW. `new_file` set `wall_time = now` already.
                item.lamport_ts = copypaste_core::next_lamport_ts(0, item.wall_time);
                // Stable cross-device identity: derive item_id from the
                // content-hash file_id (same pattern as handle_image).
                item.item_id = uuid::Uuid::from_bytes(file_id).to_string().into();
                item.origin_device_id = local_device_id;
                // mtf5 (PG-22): mark sensitive when source app is a password manager.
                item.is_sensitive = app_is_sensitive_file;
                item.app_bundle_id = source_bundle_id;
                tracing::debug!(
                    "file encoded: {} chunks, original_size={}",
                    meta.chunk_count,
                    meta.original_size
                );

                let db_guard = db.blocking_lock();
                // Files have no searchable text body; pass "" to skip FTS.
                match insert_item_with_fts(&db_guard, &item, "") {
                    Ok(stored_id) => {
                        if stored_id != item.id {
                            tracing::debug!(
                                requested = %item.id,
                                existing = %stored_id,
                                "file item deduped against existing row"
                            );
                        } else {
                            tracing::info!(id = %item.id, "stored file item id={}", item.id);
                        }
                        prune_history(&db_guard, &config);
                        Some(item)
                    }
                    Err(e) => {
                        tracing::warn!("failed to store file item: {e}");
                        None
                    }
                }
            }
            Err(e) => {
                tracing::warn!("file encode failed (skipping): {e}");
                None
            }
        }
    })
    .await;
    match join {
        Ok(item) => item,
        Err(e) => {
            tracing::warn!("handle_file blocking task failed: {e}");
            None
        }
    }
}
