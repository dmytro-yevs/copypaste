use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::Mutex;

use copypaste_core::{ClipboardItem, Database};

use super::PUSH_RETRY_QUEUE_CAP;

/// Append `(item, payload_ct_b64)` to the retry queue, evicting the oldest
/// entry when the queue is at capacity. Bounded so a long outage cannot exhaust
/// memory.
pub(crate) fn enqueue_for_retry(
    queue: &mut VecDeque<(ClipboardItem, Option<String>)>,
    item: ClipboardItem,
    payload_ct_b64: Option<String>,
) {
    if queue.len() >= PUSH_RETRY_QUEUE_CAP {
        if let Some((dropped, _)) = queue.pop_front() {
            tracing::warn!(
                "cloud-sync retry queue at cap ({}); dropping oldest id={}",
                PUSH_RETRY_QUEUE_CAP,
                dropped.id,
            );
        }
    }
    queue.push_back((item, payload_ct_b64));
}

/// Mark a row as successfully uploaded by setting `is_synced = 1`.
///
/// Fix CLOUD-IS_SYNCED: without this, `is_synced` stayed 0 forever, causing
/// the startup backlog sweep (`WHERE is_synced = 0`) to re-upload the entire
/// history on every daemon restart. Best-effort: a failed UPDATE is logged and
/// not retried — the row will simply appear in the next backlog sweep, which is
/// harmless (the server deduplicates by primary key).
pub(super) async fn mark_item_synced(db: &Arc<Mutex<Database>>, item_id: &str) {
    let db_arc = db.clone();
    let id_owned = item_id.to_owned();
    // Run on the blocking pool — rusqlite is synchronous.
    let result = tokio::task::spawn_blocking(move || {
        let db = db_arc.blocking_lock();
        db.conn()
            .execute(
                "UPDATE clipboard_items SET is_synced = 1 WHERE item_id = ?1",
                rusqlite::params![id_owned],
            )
            .map_err(|e| e.to_string())
    })
    .await;
    match result {
        Ok(Ok(rows)) => {
            if rows == 0 {
                // Row may have been deleted between push and update — benign.
                tracing::debug!("mark_item_synced: no row updated for item_id={item_id}");
            }
        }
        Ok(Err(e)) => {
            tracing::warn!("mark_item_synced: UPDATE failed for item_id={item_id}: {e}");
        }
        Err(e) => {
            tracing::warn!("mark_item_synced: blocking task panicked for item_id={item_id}: {e}");
        }
    }
}
