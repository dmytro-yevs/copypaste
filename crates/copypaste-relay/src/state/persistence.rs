//! Push-retry queue, pending DB write types, and retry-helper impl (CopyPaste-k4py).
//!
//! [`PushRetryQueue`] is a bounded `VecDeque` stored inside [`super::RelayStore`].
//! When a `push_item_decoded` DB write fails transiently, the write parameters
//! are serialised into a [`PendingDbWrite`] and enqueued here instead of
//! returning `Err`. The background `crate::retry::run_push_retry` task drains
//! the queue, replaying each write with exponential backoff.

use std::collections::VecDeque;

use crate::error::RelayError;

use super::PUSH_RETRY_QUEUE_CAP;

/// One failed DB write that needs to be replayed by the retry background task.
///
/// Captures everything required to re-execute the three SQLite calls that
/// `push_item_decoded` makes after inserting the item into the in-memory inbox:
///   1. `Db::insert_item` — insert the new row.
///   2. `Db::delete_oldest_items` — prune the oldest `pruned_count` rows if the
///      inbox cap was hit (0 = no pruning needed).
///   3. `Db::set_next_sync_id` — advance the per-device counter.
///
/// Fields are read by `retry::run_push_retry` (called from `main.rs`).
/// `#[path]`-include test binaries that compile `state.rs` without `retry.rs`
/// see the struct as unreachable — `#[allow(dead_code)]` mirrors the pattern
/// already used for `DeviceRecord::public_key_b64` and `DeviceRecord::tier`.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct PendingDbWrite {
    pub device_id: String,
    pub item_id: i64,
    pub content_type: String,
    /// `Arc<str>` — shared with the in-memory inbox entry so the retry queue
    /// holds a refcount rather than a full payload copy.
    pub content_b64: std::sync::Arc<str>,
    pub wall_time: u64,
    pub inserted_at_unix: u64,
    /// How many oldest items were pruned from the inbox cap. 0 = no pruning.
    pub pruned_count: usize,
    /// The next per-device sync id to persist after this write.
    pub next_sync_id: i64,
    /// How many times this write has been attempted (incremented before each
    /// retry; 0 on the first enqueue from `push_item_decoded`).
    pub attempts: u32,
}

/// Bounded queue of pending DB writes for the relay push retry path.
///
/// Stored as a field of [`super::RelayStore`] so the existing
/// `std::sync::Mutex<RelayStore>` serialises all access — no second lock needed.
/// Drained by the background `crate::retry::run_push_retry` task.
#[derive(Debug)]
pub struct PushRetryQueue {
    inner: VecDeque<PendingDbWrite>,
}

impl PushRetryQueue {
    /// Create an empty queue.
    pub fn new() -> Self {
        Self {
            inner: VecDeque::new(),
        }
    }

    /// Push a write onto the queue.  Returns `false` (and logs WARN) if the
    /// queue is already at [`PUSH_RETRY_QUEUE_CAP`] — the write is discarded.
    pub fn enqueue(&mut self, write: PendingDbWrite) -> bool {
        if self.inner.len() >= PUSH_RETRY_QUEUE_CAP {
            tracing::warn!(
                device_id = %write.device_id,
                item_id = write.item_id,
                queue_len = self.inner.len(),
                "CopyPaste-k4py: push retry queue full; discarding item write (durability lost)"
            );
            return false;
        }
        self.inner.push_back(write);
        true
    }

    /// Drain up to `n` entries from the front of the queue into `out`.
    // Called from `retry::run_push_retry` (via `main.rs`). `#[path]`-include
    // test binaries that compile `state.rs` without `retry.rs` never call this.
    #[allow(dead_code)]
    pub fn drain_front(&mut self, n: usize, out: &mut Vec<PendingDbWrite>) {
        let take = n.min(self.inner.len());
        out.extend(self.inner.drain(..take));
    }

    /// Return the number of pending writes in the queue.
    // Used in `#[cfg(test)]` only; the production retry task calls `is_empty`.
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Return `true` if the queue is empty.
    // Called from `retry::run_push_retry`. `#[path]`-include test binaries that
    // compile `state.rs` without `retry.rs` see this as unused; allow mirrors
    // the pattern for `PendingDbWrite` and `DeviceRecord` fields above.
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

// ---------------------------------------------------------------------------
// RelayStore: retry helpers (CopyPaste-k4py)
// ---------------------------------------------------------------------------

impl super::RelayStore {
    // -----------------------------------------------------------------------
    // Retry helpers (CopyPaste-k4py)
    // -----------------------------------------------------------------------

    /// Attempt to insert an inbox item into the DB (called by the retry task).
    ///
    /// Uses `OR IGNORE` semantics: if the row already exists (e.g. from a
    /// previous partially-successful retry), the call is silently a no-op
    /// rather than an error. Ciphertext is never logged.
    // Called from `retry::run_push_retry` (via `main.rs`). `#[path]`-include
    // test binaries that compile `state.rs` without `retry.rs` never call this.
    #[allow(dead_code)]
    pub fn db_insert_item_retry(&self, write: &PendingDbWrite) -> Result<(), RelayError> {
        self.db
            .insert_item(
                &write.device_id,
                write.item_id,
                &write.content_type,
                &write.content_b64,
                write.wall_time,
                write.inserted_at_unix,
            )
            .or_else(|e| {
                // A UNIQUE constraint violation means the row already exists —
                // treat as success (idempotent retry). Any other error bubbles.
                if matches!(
                    e,
                    rusqlite::Error::SqliteFailure(
                        rusqlite::ffi::Error {
                            code: rusqlite::ErrorCode::ConstraintViolation,
                            ..
                        },
                        _,
                    )
                ) {
                    Ok(())
                } else {
                    Err(RelayError::from(e))
                }
            })
    }

    /// Attempt to prune the oldest `count` items from the DB for a device
    /// (called by the retry task after `db_insert_item_retry` succeeds).
    ///
    /// A "no rows deleted" outcome is treated as success — the rows may have
    /// already been pruned by a TTL sweep or a previous successful partial retry.
    // Called from `retry::run_push_retry`. See `db_insert_item_retry` for the
    // dead_code rationale.
    #[allow(dead_code)]
    pub fn db_delete_oldest_retry(&self, device_id: &str, count: usize) -> Result<(), RelayError> {
        self.db
            .delete_oldest_items(device_id, count)
            .map_err(RelayError::from)
    }

    /// Advance the per-device sync-id counter in the DB
    /// (called by the retry task after the item write succeeds).
    ///
    /// Uses `MAX(current, next_sync_id)` semantics (via
    /// [`crate::db::Db::set_next_sync_id_at_least`]) so an out-of-order retry
    /// cannot roll the counter BACK: the primary `set_next_sync_id` call in
    /// `push_item_decoded` always runs in insertion order (under the store mutex);
    /// a retry that arrives late after a later item already advanced the counter
    /// must not clobber the higher value.
    // Called from `retry::run_push_retry`. See `db_insert_item_retry` for the
    // dead_code rationale.
    #[allow(dead_code)]
    pub fn db_set_next_sync_id_retry(
        &self,
        device_id: &str,
        next_sync_id: i64,
    ) -> Result<(), RelayError> {
        self.db
            .set_next_sync_id_at_least(device_id, next_sync_id)
            .map_err(RelayError::from)
    }

    /// Test-only helper: delete an item row from the DB by `(device_id, item_id)`.
    ///
    /// Used by `retry.rs` tests to simulate "the initial DB write failed" by
    /// deleting the row that `push_item` just wrote, then seeding the retry queue.
    /// Only called from `#[cfg(test)]` code in `retry.rs`; path-include test
    /// binaries that compile `state.rs` without `retry.rs` never invoke it.
    #[cfg(test)]
    #[allow(dead_code)]
    pub fn db_delete_item_for_test(&self, device_id: &str, item_id: i64) {
        self.db
            .delete_item(device_id, item_id)
            .expect("test helper: db_delete_item_for_test failed");
    }

    /// Test-only helper: count inbox rows in the DB for a device.
    /// Only called from `#[cfg(test)]` code in `retry.rs`; path-include test
    /// binaries that compile `state.rs` without `retry.rs` never invoke it.
    #[cfg(test)]
    #[allow(dead_code)]
    pub fn db_item_count_for_test(&self, device_id: &str) -> i64 {
        self.db
            .item_count(device_id)
            .expect("test helper: db_item_count_for_test failed")
    }

    /// Test-only helper: synchronously drain and execute all pending deferred DB
    /// writes from the retry queue (CopyPaste-crh3.70).
    ///
    /// In production, deferred `insert_item` / `delete_oldest_items` writes are
    /// processed by the `run_push_retry` background task (woken immediately via
    /// `db_write_notify`). Tests that need synchronous DB durability — e.g.
    /// persistence tests that drop the store and reopen the same file — must call
    /// this helper to flush the queue before "restarting."
    ///
    /// Mirrors the per-item logic in `retry::run_push_retry` but runs on the
    /// current thread without spawning a task or sleeping.
    #[cfg(any(test, feature = "quota-tiers"))]
    #[allow(dead_code)]
    pub fn flush_pending_db_writes_for_test(&mut self) {
        use super::PUSH_RETRY_QUEUE_CAP;
        let mut batch = Vec::new();
        self.push_retry_queue
            .drain_front(PUSH_RETRY_QUEUE_CAP, &mut batch);
        for write in batch {
            // Ignore individual errors: test helper, not retry logic.
            let _ = self.db_insert_item_retry(&write);
            if write.pruned_count > 0 {
                let _ = self.db_delete_oldest_retry(&write.device_id, write.pruned_count);
            }
            let _ = self.db_set_next_sync_id_retry(&write.device_id, write.next_sync_id);
        }
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use crate::state::test_helpers::*;
    use crate::state::RelayStore;

    /// CopyPaste-hf40: the `next_sync_id` counter (relay watermark) must be
    /// rehydrated from the database on startup. Simulated by:
    ///   1. Create store A (on-disk SQLite via `new_persistent`).
    ///   2. Push N items → counter advances to N+1.
    ///   3. Create store B reloading from the same on-disk DB → must seed from N+1.
    ///   4. Push one more item in store B → must get server id N+1 (not 1).
    #[test]
    fn next_sync_id_watermark_is_seeded_from_db_on_restart() {
        // Create a temp file path for the DB.
        let dir = std::env::temp_dir().join(format!(
            "copypaste-relay-hf40-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("relay.db").to_str().unwrap().to_string();

        // Store A: register device, push 3 items → last pushed id must be 3.
        // Note: `push_item_decoded` writes `next_sync_id` synchronously via
        // `db.set_next_sync_id` (even though the item payload write is deferred),
        // so the watermark is guaranteed in DB when the store drops.
        let last_id_a = {
            let mut store = RelayStore::new_persistent(3600, 500, &db_path).unwrap();
            store
                .register_device(device_a_id(), "A".into(), valid_key_b64(), valid_pop_b64())
                .unwrap();
            push_text(&mut store, &device_a_id(), 10); // id=1
            push_text(&mut store, &device_a_id(), 20); // id=2
            push_text(&mut store, &device_a_id(), 30) // id=3
        };
        assert_eq!(last_id_a, 3, "first store: last pushed id must be 3");

        // Store B: open the same on-disk DB and reload via `new_persistent`.
        // The next push must continue from id=4, NOT restart from 1.
        let first_id_b = {
            let mut store = RelayStore::new_persistent(3600, 500, &db_path).unwrap();
            push_text(&mut store, &device_a_id(), 40) // must be id=4
        };
        assert_eq!(
            first_id_b,
            4,
            "CopyPaste-hf40: after restart the first new push must get id={} (continuation), \
             not 1 (restart from scratch); got {}",
            last_id_a + 1,
            first_id_b
        );

        // Clean up.
        std::fs::remove_dir_all(&dir).ok();
    }
}
