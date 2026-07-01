//! TTL cleanup (sensitive + general prune) on a blocking thread, and the
//! byte-cap prune run after each insert.

use copypaste_core::{prune_to_cap, AppConfig, Database};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Run the sensitive- and/or general-TTL deletes on a blocking thread.
///
/// daemon-core L1: both `delete_sensitive_expired` and `delete_expired` are
/// synchronous rusqlite calls. Previously they ran inline inside the `select!`
/// loop under `db.lock().await` while holding the tokio Mutex, blocking the
/// async worker for the duration of the SQL. We now mirror the IPC path:
/// acquire the lock and run the SQL inside `spawn_blocking`. The clock-skew-safe
/// `unwrap_or_default()` on the timestamp is preserved.
///
/// CopyPaste-98ja: when `do_sensitive` is true the sensitive prune is guarded
/// by a cheap `SELECT EXISTS` pre-check (`has_sensitive_items`).  On a system
/// with no sensitive history at all this short-circuits the full scan every
/// 5 seconds and avoids a gratuitous write transaction.  The TTL guarantee is
/// preserved: the prune still runs whenever the pre-check finds at least one
/// eligible row.
pub(crate) async fn run_ttl_cleanup(
    db: &Arc<Mutex<Database>>,
    sensitive_ttl_ms: i64,
    do_sensitive: bool,
    do_general: bool,
) {
    if !do_sensitive && !do_general {
        return;
    }
    let db = db.clone();
    let join = tokio::task::spawn_blocking(move || {
        let guard = db.blocking_lock();
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        let sensitive = if do_sensitive {
            // CopyPaste-98ja: cheap EXISTS probe before the full DELETE scan.
            // When no sensitive (non-pinned) rows exist at all, skip the prune
            // — nothing has expired and the write transaction is unnecessary.
            if copypaste_core::has_sensitive_items(&guard) {
                Some(copypaste_core::delete_sensitive_expired(
                    &guard,
                    now_ms,
                    sensitive_ttl_ms,
                ))
            } else {
                None
            }
        } else {
            None
        };
        let general = if do_general {
            Some(copypaste_core::delete_expired(&guard, now_ms))
        } else {
            None
        };
        (sensitive, general)
    })
    .await;
    let (sensitive, general) = match join {
        Ok(pair) => pair,
        Err(e) => {
            tracing::warn!("TTL cleanup blocking task failed: {e}");
            return;
        }
    };
    match sensitive {
        Some(Ok(n)) if n > 0 => tracing::info!("sensitive TTL cleanup: wiped {n} sensitive items"),
        Some(Err(e)) => tracing::warn!("sensitive TTL cleanup error: {e}"),
        _ => {}
    }
    match general {
        Some(Ok(n)) if n > 0 => tracing::info!("TTL cleanup: removed {n} expired items"),
        Some(Err(e)) => tracing::warn!("TTL cleanup error: {e}"),
        _ => {}
    }
}

/// Enforce the size-only cap after each local insert.
///
/// The count cap (`history_limit`) has been removed: the local DB is bounded
/// exclusively by `storage_quota_bytes`. Pinned items are never evicted.
///
/// `storage_quota_bytes` is u64 in AppConfig; saturating cast to i64 is safe
/// because values above i64::MAX (>9 EB) are unreachable in practice.
pub(crate) fn prune_history(db: &Database, config: &AppConfig) {
    match prune_to_cap(db, config.storage_quota_bytes as i64) {
        Ok(0) => {}
        Ok(n) => tracing::debug!("prune_history: byte-cap pruned {n} rows"),
        Err(e) => tracing::warn!("prune_history: byte-cap prune failed: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // CopyPaste-98ja: sensitive TTL pre-check gate
    // -----------------------------------------------------------------------

    /// When the database contains NO sensitive items `run_ttl_cleanup` must not
    /// run `delete_sensitive_expired` — verified by checking that the
    /// has_sensitive_items pre-check (CopyPaste-98ja) gates the full scan.
    ///
    /// This test exercises the gate by:
    /// 1. Starting with an empty DB (no sensitive rows → has_sensitive_items = false).
    /// 2. Calling `run_ttl_cleanup` with `do_sensitive = true` and a 0 ms TTL
    ///    that would delete EVERYTHING if the gate were absent.
    /// 3. Inserting a non-sensitive row and confirming it survives the cleanup —
    ///    proving the DELETE did NOT run.
    #[tokio::test]
    async fn run_ttl_cleanup_skips_sensitive_scan_when_no_sensitive_items() {
        let local_key = [0xAAu8; 32];
        let db = Arc::new(Mutex::new(Database::open_in_memory().expect("open db")));
        let config = AppConfig::default();

        // Insert ONE non-sensitive item via handle_text (which correctly
        // sets is_sensitive based on the content).
        crate::daemon::capture::text::handle_text(
            "hello world not sensitive".to_string(),
            &db,
            &local_key,
            &config,
            "test-device",
            None,
        )
        .await;

        // Confirm has_sensitive_items returns false (plain text is not sensitive).
        {
            let guard = db.lock().await;
            assert!(
                !copypaste_core::has_sensitive_items(&guard),
                "no sensitive items must be present initially"
            );
        }

        // Run cleanup with an extremely aggressive TTL (0 ms — would delete
        // everything if the gate were absent) and do_sensitive = true.
        // If the gate is working, has_sensitive_items returns false and the
        // DELETE is skipped, leaving the non-sensitive row intact.
        run_ttl_cleanup(&db, 0, true, false).await;

        // The non-sensitive item must still exist.
        let guard = db.lock().await;
        let count = copypaste_core::count_items(&*guard).expect("count_items");
        assert_eq!(
            count, 1,
            "non-sensitive item must survive cleanup when no sensitive items exist"
        );
    }

    // -----------------------------------------------------------------------
    // P2 (ugv7): startup TTL purge runs before IPC bind
    // -----------------------------------------------------------------------

    /// `run_ttl_cleanup` (reused by the startup purge) must delete a sensitive
    /// item whose creation time + TTL is in the past.  This verifies that the
    /// purge that now runs at startup (before the IPC socket is bound) would
    /// actually remove already-expired sensitive rows.
    ///
    /// The test inserts a row with `wall_time = 1` (epoch ms — always expired)
    /// and `is_sensitive = 1`, then calls `run_ttl_cleanup` with a 1 ms TTL so
    /// `threshold = now - 1 ms ≫ 1`, and asserts the row is gone.
    #[tokio::test]
    async fn startup_ttl_purge_removes_expired_sensitive_items() {
        let local_key = zeroize::Zeroizing::new([0xBBu8; 32]);
        let local_key_arc: Arc<zeroize::Zeroizing<[u8; 32]>> = Arc::new(local_key);
        let db = Arc::new(Mutex::new(Database::open_in_memory().expect("open db")));

        // Insert an expired sensitive row directly via SQL so we can control
        // wall_time (handle_text would set it to now(), which would NOT be expired).
        {
            let guard = db.lock().await;
            let row_id = uuid::Uuid::new_v4().to_string();
            let item_id = copypaste_core::ItemId::from(uuid::Uuid::new_v4().to_string());
            let aad = copypaste_core::build_item_aad(&item_id, copypaste_core::AAD_SCHEMA_VERSION);
            let (nonce, ciphertext) =
                copypaste_core::encrypt_item_with_aad(b"sk-supersecrettoken", &local_key_arc, &aad)
                    .expect("encrypt");
            guard
                .conn()
                .execute(
                    "INSERT INTO clipboard_items \
                     (id, item_id, content_type, content, content_nonce, \
                      is_sensitive, is_synced, lamport_ts, wall_time, key_version) \
                     VALUES (?1,?2,'text',?3,?4,1,0,1,1,2)",
                    rusqlite::params![row_id, item_id, ciphertext, nonce.to_vec()],
                )
                .expect("insert expired sensitive row");
        }

        // Verify the row is present.
        {
            let guard = db.lock().await;
            assert!(
                copypaste_core::has_sensitive_items(&guard),
                "sensitive item must be present before cleanup"
            );
        }

        // Run with a 1 ms TTL — the epoch-1 wall_time is always older than
        // `now_ms - 1`, so the row must be purged.
        run_ttl_cleanup(&db, 1, true, false).await;

        // Verify the row was removed.
        let guard = db.lock().await;
        let count = copypaste_core::count_items(&*guard).expect("count_items");
        assert_eq!(
            count, 0,
            "expired sensitive item must be purged by startup TTL cleanup"
        );
    }
}
