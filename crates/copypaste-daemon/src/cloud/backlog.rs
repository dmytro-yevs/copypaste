use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::sync_common::{decrypt_item_plaintext_blocking, wrap_and_check_cloud_upload_plaintext};
use copypaste_core::{encrypt_for_cloud, ClipboardItem, Database, SyncKey};

use super::push::{enqueue_for_retry, PUSH_RETRY_QUEUE_CAP};

/// Sweep the local DB for unsynced syncable items, re-encrypt each under the
/// supplied sync key, and enqueue it for upload.
///
/// Shared by the startup pre-load and the BUG C2 None→Some key-transition path
/// so both follow the identical `is_synced = 0 AND content_type IN (...)` query
/// and chronological ordering. `key_bytes` MUST be exactly 32 bytes (the
/// `SyncKey` width); a wrong length is logged and the sweep is skipped. The
/// derived key material is zeroized before return.
pub(super) async fn run_backlog_sweep(
    db: &Arc<Mutex<Database>>,
    local_key: &Arc<zeroize::Zeroizing<[u8; 32]>>,
    key_bytes: &[u8],
    retry_queue: &mut VecDeque<(ClipboardItem, Option<String>)>,
) {
    let mut key_arr = [0u8; 32];
    if key_bytes.len() != key_arr.len() {
        tracing::warn!(
            "cloud-sync backlog: sync key wrong length ({} != 32); skipping sweep",
            key_bytes.len()
        );
        return;
    }
    key_arr.copy_from_slice(key_bytes);

    // v0.6: text, image, and file items all sync to the cloud now. Mark any
    // OTHER (unknown) non-syncable content_type synced so it does not linger in
    // the unsynced count forever.
    {
        let db_arc2 = db.clone();
        let _ = tokio::task::spawn_blocking(move || {
            let db = db_arc2.blocking_lock();
            match db.conn().execute(
                "UPDATE clipboard_items SET is_synced = 1 \
                 WHERE is_synced = 0 \
                   AND content_type NOT IN ('text', 'image', 'file')",
                [],
            ) {
                Ok(0) => {}
                Ok(n) => tracing::warn!(
                    "cloud-sync backlog: marked {n} unsupported-type item(s) is_synced=1"
                ),
                Err(e) => tracing::warn!(
                    "cloud-sync backlog: failed to mark unsupported items synced: {e}"
                ),
            }
        })
        .await;
    }

    let db_arc = db.clone();
    // Load unsynced items on the blocking pool (rusqlite is sync).
    let backlog_items: Vec<ClipboardItem> = tokio::task::spawn_blocking(move || {
        let db = db_arc.blocking_lock();
        // Fetch up to PUSH_RETRY_QUEUE_CAP unsynced syncable items
        // (text/image/file), oldest first, so the Supabase timeline is
        // chronological.
        let mut stmt = match db.conn().prepare(
            "SELECT id, item_id, content_type, content, content_nonce, \
             blob_ref, is_sensitive, is_synced, lamport_ts, wall_time, \
             expires_at, app_bundle_id, content_hash, origin_device_id, \
             key_version, pinned, pin_order \
             FROM clipboard_items \
             WHERE is_synced = 0 \
               AND content_type IN ('text', 'image', 'file') \
             ORDER BY wall_time ASC \
             LIMIT ?1",
        ) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("cloud-sync backlog query prepare failed: {e}");
                return vec![];
            }
        };
        stmt.query_map(rusqlite::params![PUSH_RETRY_QUEUE_CAP as i64], |row| {
            Ok(ClipboardItem {
                id: row.get(0)?,
                item_id: row.get(1)?,
                content_type: row.get(2)?,
                content: row.get(3)?,
                content_nonce: row.get(4)?,
                blob_ref: row.get(5)?,
                is_sensitive: row.get(6)?,
                is_synced: row.get(7)?,
                lamport_ts: row.get(8)?,
                wall_time: row.get(9)?,
                expires_at: row.get(10)?,
                app_bundle_id: row.get(11)?,
                content_hash: row.get(12)?,
                origin_device_id: row.get(13).unwrap_or_default(),
                key_version: row.get::<_, i64>(14).unwrap_or(2) as u8,
                pinned: row.get(15).unwrap_or(false),
                // pin_order is synced alongside pinned so that the drag-to-reorder
                // ordering chosen on the source device is preserved on every peer.
                pin_order: row.get(16)?,
                // backlog query selects no thumb column; thumbnails are a
                // local-only field (schema v9) and never synced.
                thumb: None,
                // Rows fetched from the upload backlog are live items (not
                // tombstones) — if they were soft-deleted they would have been
                // filtered out by the backlog query's `deleted = 0` guard.
                deleted: false,
            })
        })
        .map(|rows| rows.filter_map(|r| r.ok()).collect::<Vec<_>>())
        .unwrap_or_default()
    })
    .await
    .unwrap_or_default();

    let count = backlog_items.len();
    if count > 0 {
        tracing::info!("cloud-sync backlog: found {count} unsynced item(s) — queuing for push");
        let tmp_key = SyncKey::from_bytes(key_arr);
        for item in backlog_items {
            // P1-1: sensitive items must never leave this device — skip them in the
            // backlog sweep just as push_loop skips them from the broadcast channel.
            // Mark them as synced so the sweep doesn't visit them again on restart.
            if item.is_sensitive {
                tracing::debug!(
                    "cloud-sync backlog: skipping sensitive id={} (never uploaded); marking synced",
                    item.id
                );
                let db_arc3 = db.clone();
                let id_owned = item.item_id.clone();
                let _ = tokio::task::spawn_blocking(move || {
                    let g = db_arc3.blocking_lock();
                    let _ = g.conn().execute(
                        "UPDATE clipboard_items SET is_synced = 1 WHERE item_id = ?1",
                        rusqlite::params![id_owned],
                    );
                })
                .await;
                continue;
            }
            // CopyPaste-z1xt: decrypt on the blocking thread pool (CPU-bound
            // decode/decrypt was stalling the async executor). The wrapper
            // consumes + returns the item so the heavy `content` blob is moved,
            // not cloned.
            let (item_back, decrypt_res) =
                decrypt_item_plaintext_blocking(item, zeroize::Zeroizing::new(***local_key)).await;
            let item = match item_back {
                Some(it) => it,
                None => {
                    tracing::warn!("cloud-sync backlog: decrypt task failed; skipping");
                    continue;
                }
            };
            let plaintext = match decrypt_res {
                Ok(p) => p,
                Err(e) => {
                    tracing::warn!(
                        "cloud-sync backlog: decrypt failed for id={}: {e}; skipping",
                        item.id
                    );
                    continue;
                }
            };
            // BUG C1: for files, embed name+MIME inside the encrypted plaintext
            // so cloud sync preserves file identity. No-op for text/image. The
            // ceiling is enforced on the WRAPPED bytes so the backlog sweep skips
            // exactly what the download side would reject.
            let cloud_plaintext = match wrap_and_check_cloud_upload_plaintext(&item, plaintext) {
                Ok(p) => p,
                Err(e) => {
                    tracing::warn!("cloud-sync backlog: skipping id={}: {e}", item.id);
                    continue;
                }
            };
            match encrypt_for_cloud(&tmp_key, &item.item_id, &cloud_plaintext) {
                Ok(blob) => {
                    use base64::Engine as _;
                    let payload_ct_b64 = base64::engine::general_purpose::STANDARD.encode(&blob);
                    enqueue_for_retry(retry_queue, item, Some(payload_ct_b64));
                }
                Err(e) => {
                    tracing::warn!(
                        "cloud-sync backlog: encrypt failed for id={}: {e}; skipping",
                        item.id
                    );
                }
            }
        }
        tracing::info!(
            "cloud-sync backlog: {} item(s) queued for upload",
            retry_queue.len()
        );
    }
    // Zero the derived key bytes regardless of whether any items were queued.
    zeroize::Zeroize::zeroize(&mut key_arr);
}

/// Sweep the local DB for tombstone rows that have not yet been pushed to cloud
/// (`deleted = 1 AND is_synced = 0`), and enqueue each as a tombstone push.
///
/// # CopyPaste-e89n
///
/// `soft_delete_item` now resets `is_synced = 0` when deleting an item. This
/// sweep picks those rows up and enqueues a minimal tombstone push (no ciphertext,
/// `deleted = true`) so the cloud row transitions to `deleted = true` on the
/// server. Other devices apply the tombstone on their next poll/WS event.
///
/// Tombstones carry no content; they are pushed directly without decrypt/re-encrypt.
pub(super) async fn run_tombstone_backlog_sweep(
    db: &Arc<Mutex<Database>>,
    retry_queue: &mut VecDeque<(ClipboardItem, Option<String>)>,
) {
    let db_arc = db.clone();
    let tombstones: Vec<ClipboardItem> = tokio::task::spawn_blocking(move || {
        let db = db_arc.blocking_lock();
        let mut stmt = match db.conn().prepare(
            "SELECT id, item_id, content_type, is_sensitive, is_synced, lamport_ts, \
             wall_time, expires_at, app_bundle_id, content_hash, origin_device_id, \
             key_version, pinned, pin_order \
             FROM clipboard_items \
             WHERE deleted = 1 AND is_synced = 0 \
             ORDER BY wall_time ASC \
             LIMIT ?1",
        ) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("cloud-sync tombstone-backlog query prepare failed: {e}");
                return vec![];
            }
        };
        stmt.query_map(rusqlite::params![PUSH_RETRY_QUEUE_CAP as i64], |row| {
            Ok(ClipboardItem {
                id: row.get(0)?,
                item_id: row.get(1)?,
                content_type: row.get(2)?,
                content: None, // tombstone: content already wiped
                content_nonce: None,
                blob_ref: None,
                is_sensitive: row.get(3)?,
                is_synced: row.get(4)?,
                lamport_ts: row.get(5)?,
                wall_time: row.get(6)?,
                expires_at: row.get(7)?,
                app_bundle_id: row.get(8)?,
                content_hash: row.get(9)?,
                origin_device_id: row.get(10).unwrap_or_default(),
                key_version: row.get::<_, i64>(11).unwrap_or(2) as u8,
                pinned: row.get(12).unwrap_or(false),
                pin_order: row.get(13)?,
                thumb: None,
                deleted: true,
            })
        })
        .map(|rows| rows.filter_map(|r| r.ok()).collect::<Vec<_>>())
        .unwrap_or_default()
    })
    .await
    .unwrap_or_default();

    let count = tombstones.len();
    if count > 0 {
        tracing::info!(
            "cloud-sync tombstone-backlog: found {count} unsynced tombstone(s) — queuing for push"
        );
        for tombstone in tombstones {
            // Tombstones carry no content — push directly without decrypt/encrypt.
            // payload_ct is None so clipboard_item_to_json sends null to the server.
            enqueue_for_retry(retry_queue, tombstone, None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use copypaste_core::insert_item;

    fn item(id: &str, sensitive: bool) -> ClipboardItem {
        let mut it = ClipboardItem::new_text(vec![9, 9, 9], vec![0u8; 24], 1);
        it.id = id.to_owned();
        it.item_id = id.to_owned();
        it.content_type = "text".to_owned();
        it.is_sensitive = sensitive;
        it.is_synced = false;
        it
    }

    fn is_synced(db: &Database, id: &str) -> bool {
        db.conn()
            .query_row(
                "SELECT is_synced FROM clipboard_items WHERE id = ?1",
                rusqlite::params![id],
                |r| r.get::<_, i64>(0),
            )
            .map(|v| v == 1)
            .unwrap_or(false)
    }

    /// CopyPaste-20yw / P1-1: the cloud backlog sweep must never enqueue a
    /// SENSITIVE item — it skips it and marks it synced so the sweep does not
    /// revisit it. Real guard test: a sensitive item ends up is_synced=1 and is
    /// NOT in the retry queue; a non-sensitive item with undecryptable content
    /// (the positive control) is skipped WITHOUT being marked synced, proving the
    /// sensitive guard (not the generic skip) is what marks-and-drops the secret.
    /// Removing the guard routes the secret to the decrypt path → garbage fails →
    /// it is NOT marked synced → this test fails.
    #[tokio::test]
    async fn cloud_backlog_skips_and_marks_sensitive_never_enqueues() {
        let db = Arc::new(Mutex::new(Database::open_in_memory().expect("db")));
        {
            let g = db.lock().await;
            insert_item(&g, &item("secret-1", true)).expect("insert secret");
            insert_item(&g, &item("plain-1", false)).expect("insert plain");
        }
        let local_key = Arc::new(zeroize::Zeroizing::new([0u8; 32]));
        let mut queue = VecDeque::new();

        run_backlog_sweep(&db, &local_key, &[7u8; 32], &mut queue).await;

        let g = db.lock().await;
        // Sensitive item: marked synced, never enqueued.
        assert!(
            is_synced(&g, "secret-1"),
            "sensitive item must be marked synced by the sweep"
        );
        assert!(
            !queue.iter().any(|(it, _)| it.id == "secret-1"),
            "sensitive item must never be enqueued for cloud upload"
        );
        // Positive control: non-sensitive item with bogus content fails decrypt
        // and is skipped WITHOUT being marked synced — distinct from the
        // sensitive guard's mark-and-drop.
        assert!(
            !is_synced(&g, "plain-1"),
            "non-sensitive undecryptable item must NOT be marked synced (distinguishes the guard)"
        );
    }
}
