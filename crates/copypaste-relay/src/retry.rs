//! Background retry runner for transient DB write failures on relay push
//! (CopyPaste-k4py).
//!
//! ## Problem
//!
//! When the relay accepts a `POST /devices/:id/items` push, it writes the item
//! into the in-memory inbox (immediately visible to pollers and SSE streams) and
//! then writes through to SQLite for durability. Before this fix, a transient
//! SQLite error (WAL checkpoint race, disk hiccup, brief lock contention) on the
//! write-through caused `push_item_decoded` to propagate `Err` all the way to the
//! HTTP handler, which returned 500 to the pushing daemon. The item was already
//! in memory — delivery to live pollers was unaffected — but it was NEVER written
//! to the durable store and would be lost on process restart: a silent, permanent
//! data loss on a transient fault.
//!
//! The cloud sync path (`copypaste-supabase`) already has a retry queue. This
//! module brings equivalent resilience to the relay push path.
//!
//! ## Solution
//!
//! [`crate::state::PushRetryQueue`] is a bounded `VecDeque` that lives inside
//! [`crate::state::RelayStore`]. When `push_item_decoded`'s DB write fails:
//!
//! 1. The item is already in the in-memory inbox — it stays there (the push
//!    succeeded from the client's perspective).
//! 2. The failed write is serialised into a [`crate::state::PendingDbWrite`] and
//!    pushed onto the queue (if space allows; if the queue is full the write is
//!    logged and dropped — the item remains in memory, but durability is lost,
//!    same as the old behaviour).
//! 3. `push_item_decoded` returns `Ok(id)` so the HTTP handler returns 201.
//! 4. The background [`run_push_retry`] task (driven by
//!    [`crate::supervise::spawn_supervised`] from `main.rs`) periodically drains
//!    the queue, retrying each write with capped exponential backoff. Successful
//!    retries are removed; persistent failures accumulate retry counts (capped at
//!    [`MAX_RETRY_ATTEMPTS`]) before being discarded with an ERROR log.
//!
//! ## Bounded memory
//!
//! [`crate::state::PUSH_RETRY_QUEUE_CAP`] is the maximum number of pending writes.
//! Each entry holds one `Arc<str>` (the ciphertext, shared with the in-memory
//! inbox entry — no extra copy) plus a small amount of metadata. The queue is
//! drained continuously; items that exceed the cap are logged and discarded rather
//! than buffered unboundedly.
//!
//! ## Locking model
//!
//! The retry task holds `Arc<Mutex<RelayStore>>` and acquires the store lock
//! briefly on each drain tick — identical to the TTL evictor. No `.await` is
//! held across the lock (the crate-wide `deny(clippy::await_holding_lock)` lint
//! enforces this). The retry queue itself is a field of `RelayStore`, so the
//! same lock that serialises all other store mutations also serialises queue
//! access — no additional lock needed.

use std::sync::Arc;
use std::time::Duration;

use crate::state::{AppState, PendingDbWrite};

// ---------------------------------------------------------------------------
// Configuration constants
// ---------------------------------------------------------------------------

/// Maximum number of retry attempts before a write is discarded with an ERROR
/// log. With `PUSH_RETRY_BASE_DELAY_MS` doubling on each attempt and capped at
/// `PUSH_RETRY_MAX_DELAY_MS`, this keeps the item in the queue for at most a
/// few minutes before giving up.
pub const MAX_RETRY_ATTEMPTS: u32 = 8;

/// Base retry interval. Each successive attempt doubles this up to
/// `PUSH_RETRY_MAX_DELAY_MS`. On the first retry (attempt 1) the task sleeps
/// ~500 ms; by attempt 5 the sleep is ~8 s.
const PUSH_RETRY_BASE_DELAY_MS: u64 = 500;

/// Upper bound on the inter-retry sleep — capped at 64 s.
const PUSH_RETRY_MAX_DELAY_MS: u64 = 64_000;

/// How often the retry background task ticks when the queue is non-empty (and
/// no backoff is in effect).
const PUSH_RETRY_POLL_MS: u64 = 500;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Compute the inter-attempt backoff for `attempt` (1-based: the first retry
/// is attempt 1). Doubles from `PUSH_RETRY_BASE_DELAY_MS`, capped at
/// `PUSH_RETRY_MAX_DELAY_MS`.
fn backoff_ms(attempt: u32) -> u64 {
    let shift = attempt.saturating_sub(1);
    // 2^shift × base, capped at max. Use checked_shl to avoid a panic on large
    // shift values, falling back to u64::MAX (which the .min clamps to max).
    let multiplier = 1u64.checked_shl(shift).unwrap_or(u64::MAX);
    PUSH_RETRY_BASE_DELAY_MS
        .saturating_mul(multiplier)
        .min(PUSH_RETRY_MAX_DELAY_MS)
}

// ---------------------------------------------------------------------------
// Background retry runner
// ---------------------------------------------------------------------------

/// Background loop that drains [`crate::state::PushRetryQueue`] and writes
/// all deferred DB operations (CopyPaste-crh3.70 / CopyPaste-k4py).
///
/// Spawned by `main.rs` via [`crate::supervise::spawn_supervised`] so panics
/// are logged and the task restarts automatically.
///
/// The task wakes immediately when `push_item_decoded` enqueues a write via
/// `store.db_write_notify.notify_one()`, or at most `PUSH_RETRY_POLL_MS` later
/// as a safety-net fallback (catches any missed notifications and handles
/// persistent write failures needing backoff). Each item's three DB calls
/// (insert + optional delete_oldest + set_next_sync_id) run under a single brief
/// lock acquisition; the lock is released between items so concurrent push
/// requests are not stalled for more than one item's worth of SQLite I/O at a
/// time. On a persistent DB failure the item is re-enqueued with exponential
/// backoff; after `MAX_RETRY_ATTEMPTS` it is discarded with an ERROR log.
pub async fn run_push_retry(state: AppState) {
    // Clone the Arc<Notify> once at start-up and hold it outside the lock so
    // we can .await it without keeping the store mutex locked.
    let notify: Arc<tokio::sync::Notify> = {
        let store = state.lock().unwrap_or_else(|e| e.into_inner());
        Arc::clone(&store.db_write_notify)
    };

    loop {
        // Wake immediately on a push enqueue (notify_one from push_item_decoded)
        // or fall back to the polling interval — whichever fires first.
        tokio::select! {
            _ = notify.notified() => {}
            _ = tokio::time::sleep(Duration::from_millis(PUSH_RETRY_POLL_MS)) => {}
        }

        // Drain up to 16 pending writes per tick so a burst doesn't hold the
        // lock for many SQLite calls.
        let mut batch: Vec<PendingDbWrite> = Vec::new();
        {
            let mut store = state.lock().unwrap_or_else(|e| e.into_inner());
            if store.push_retry_queue.is_empty() {
                continue;
            }
            store.push_retry_queue.drain_front(16, &mut batch);
        }

        // Retry each write. We re-acquire the lock once per write so other
        // requests are not blocked for the full batch duration.
        for mut write in batch {
            write.attempts += 1;

            let result = {
                let store = state.lock().unwrap_or_else(|e| e.into_inner());
                // Attempt the three DB calls that push_item_decoded makes.
                let r1 = store.db_insert_item_retry(&write);
                let r2 = if r1.is_ok() && write.pruned_count > 0 {
                    store.db_delete_oldest_retry(&write.device_id, write.pruned_count)
                } else {
                    Ok(())
                };
                let r3 = if r1.is_ok() {
                    store.db_set_next_sync_id_retry(&write.device_id, write.next_sync_id)
                } else {
                    Ok(())
                };
                r1.and(r2).and(r3)
            };

            match result {
                Ok(()) => {
                    tracing::debug!(
                        device_id = %write.device_id,
                        item_id = write.item_id,
                        attempts = write.attempts,
                        "CopyPaste-k4py: relay push DB retry succeeded"
                    );
                }
                Err(e) => {
                    if write.attempts >= MAX_RETRY_ATTEMPTS {
                        tracing::error!(
                            device_id = %write.device_id,
                            item_id = write.item_id,
                            attempts = write.attempts,
                            error = %e,
                            "CopyPaste-k4py: relay push DB retry exhausted; item not durable"
                        );
                        // Discard — in-memory item is still available to pollers
                        // for the process lifetime, but it won't survive restart.
                    } else {
                        let delay = backoff_ms(write.attempts);
                        tracing::warn!(
                            device_id = %write.device_id,
                            item_id = write.item_id,
                            attempt = write.attempts,
                            retry_in_ms = delay,
                            error = %e,
                            "CopyPaste-k4py: relay push DB write failed; will retry"
                        );
                        // Sleep the backoff BEFORE re-enqueuing so that the next
                        // tick doesn't immediately retry (no `.await` while lock
                        // is held — the lock is not held here).
                        tokio::time::sleep(Duration::from_millis(delay)).await;
                        let mut store = state.lock().unwrap_or_else(|e| e.into_inner());
                        store.push_retry_queue.enqueue(write);
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{AppState, PushRetryQueue, RelayStore, PUSH_RETRY_QUEUE_CAP};

    // ---- PushRetryQueue unit tests -----------------------------------------

    /// CopyPaste-k4py: a fresh queue is empty.
    #[test]
    fn push_retry_queue_starts_empty() {
        let q = PushRetryQueue::new();
        assert!(q.is_empty());
        assert_eq!(q.len(), 0);
    }

    /// CopyPaste-k4py: enqueue accepts up to PUSH_RETRY_QUEUE_CAP writes.
    #[test]
    fn push_retry_queue_accepts_up_to_cap() {
        let mut q = PushRetryQueue::new();
        for i in 0..PUSH_RETRY_QUEUE_CAP {
            let accepted = q.enqueue(make_write(i as i64));
            assert!(accepted, "write {i} must be accepted");
        }
        assert_eq!(q.len(), PUSH_RETRY_QUEUE_CAP);
    }

    /// CopyPaste-k4py: the (cap+1)th write is rejected when the queue is full.
    #[test]
    fn push_retry_queue_rejects_when_full() {
        let mut q = PushRetryQueue::new();
        for i in 0..PUSH_RETRY_QUEUE_CAP {
            q.enqueue(make_write(i as i64));
        }
        let accepted = q.enqueue(make_write(9999));
        assert!(!accepted, "overflow write must be rejected");
        assert_eq!(
            q.len(),
            PUSH_RETRY_QUEUE_CAP,
            "queue length must not exceed cap"
        );
    }

    /// CopyPaste-k4py: drain_front delivers writes in FIFO order.
    #[test]
    fn push_retry_queue_drain_is_fifo() {
        let mut q = PushRetryQueue::new();
        q.enqueue(make_write(1));
        q.enqueue(make_write(2));
        q.enqueue(make_write(3));

        let mut out = Vec::new();
        q.drain_front(2, &mut out);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].item_id, 1, "first drained must be oldest");
        assert_eq!(out[1].item_id, 2);
        assert_eq!(q.len(), 1, "one write must remain");
    }

    /// CopyPaste-k4py: drain_front does not panic when n > queue length.
    #[test]
    fn push_retry_queue_drain_with_excess_n_is_safe() {
        let mut q = PushRetryQueue::new();
        q.enqueue(make_write(1));
        let mut out = Vec::new();
        q.drain_front(100, &mut out);
        assert_eq!(out.len(), 1);
        assert!(q.is_empty());
    }

    // ---- backoff_ms unit tests ---------------------------------------------

    /// CopyPaste-k4py: backoff for attempt 1 must equal the base delay.
    #[test]
    fn backoff_ms_attempt_1_equals_base_delay() {
        assert_eq!(backoff_ms(1), PUSH_RETRY_BASE_DELAY_MS);
    }

    /// CopyPaste-k4py: backoff doubles on each successive attempt.
    #[test]
    fn backoff_ms_doubles_each_attempt() {
        let b1 = backoff_ms(1);
        let b2 = backoff_ms(2);
        let b3 = backoff_ms(3);
        assert_eq!(b2, b1 * 2);
        assert_eq!(b3, b1 * 4);
    }

    /// CopyPaste-k4py: backoff is capped at PUSH_RETRY_MAX_DELAY_MS.
    #[test]
    fn backoff_ms_is_capped() {
        // After enough doublings (≥8 attempts) we must hit the cap.
        assert_eq!(backoff_ms(20), PUSH_RETRY_MAX_DELAY_MS);
    }

    // ---- integration-style test: push_item_decoded enqueues on DB failure ---

    /// CopyPaste-crh3.70: every push enqueues exactly one deferred DB write so
    /// SQLite I/O is never performed while the store mutex is held by a push
    /// request. Previously (CopyPaste-k4py) the queue was only populated on
    /// failure; now it is the primary write path.
    #[test]
    fn push_enqueues_deferred_db_write() {
        use base64::engine::general_purpose::STANDARD as B64;
        use base64::Engine;

        let mut store = make_store_for_test();
        store
            .register_device(
                "aaaa0000-0000-0000-0000-000000000001".to_string(),
                "Dev".into(),
                B64.encode([0u8; 32]),
                B64.encode([0xDE_u8; 32]),
            )
            .unwrap();

        let id = store
            .push_item(
                "aaaa0000-0000-0000-0000-000000000001",
                "text".into(),
                B64.encode(b"hello"),
                1000,
                10 * 1024 * 1024,
            )
            .unwrap();

        assert!(id >= 1, "push must return a positive id");
        assert_eq!(
            store.push_retry_queue.len(),
            1,
            "CopyPaste-crh3.70: push must enqueue exactly one deferred DB write; queue len should be 1"
        );
    }

    // ---- async integration: retry runner drains queue on subsequent tick ----

    /// CopyPaste-crh3.70: run_push_retry drains the deferred write queue and
    /// persists each item to the DB. Verifies the end-to-end path:
    /// push_item_decoded enqueues a deferred write → the retry task is woken
    /// immediately via db_write_notify → the item appears in the DB.
    ///
    /// Since crh3.70 makes push_item_decoded always enqueue (no immediate DB
    /// write), after push the item is in the in-memory inbox and in the retry
    /// queue, but NOT yet in the DB. The retry task must write it.
    #[tokio::test]
    async fn retry_runner_drains_queue_and_writes_to_db() {
        use base64::engine::general_purpose::STANDARD as B64;
        use base64::Engine;
        use std::sync::{Arc, Mutex};

        let store = make_store_for_test();
        let state: AppState = Arc::new(Mutex::new(store));

        // Register a device so push_item_decoded can find it.
        let device_id = "bbbb0000-0000-0000-0000-000000000002".to_string();
        {
            let mut s = state.lock().unwrap();
            s.register_device(
                device_id.clone(),
                "Retry Dev".into(),
                B64.encode([1u8; 32]),
                B64.encode([0xDE_u8; 32]),
            )
            .unwrap();
        }

        // Push an item — crh3.70: insert_item is deferred to the retry task.
        // The item lands in the in-memory inbox and the retry queue;
        // set_next_sync_id runs synchronously (fast metadata UPDATE).
        {
            let mut s = state.lock().unwrap();
            s.push_item(
                &device_id,
                "text".into(),
                B64.encode(b"retry payload"),
                5000,
                10 * 1024 * 1024,
            )
            .unwrap();
            assert_eq!(
                s.push_retry_queue.len(),
                1,
                "push must enqueue exactly one deferred insert_item write"
            );
            // insert_item has NOT been called yet — ciphertext is not in the DB.
            assert_eq!(
                s.db_item_count_for_test(&device_id),
                0,
                "CopyPaste-crh3.70: insert_item must be deferred — DB item count must be 0 before retry task runs"
            );
        }

        // Spawn the retry background task.
        let state_clone = state.clone();
        let handle = tokio::spawn(run_push_retry(state_clone));

        // Wait up to 3 seconds for the queue to drain and the DB write to land.
        let deadline = tokio::time::timeout(std::time::Duration::from_secs(3), async {
            loop {
                {
                    let s = state.lock().unwrap();
                    if s.push_retry_queue.is_empty() {
                        break;
                    }
                }
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
        });
        deadline
            .await
            .expect("CopyPaste-crh3.70: deferred write queue must drain within 3 s");

        // Verify the item was written to the DB by the retry task.
        {
            let s = state.lock().unwrap();
            let count = s.db_item_count_for_test(&device_id);
            assert!(
                count >= 1,
                "CopyPaste-crh3.70: DB must contain the pushed item after retry task runs; got count={count}"
            );
        }

        handle.abort();
        let _ = handle.await;
    }

    // ---- helpers -----------------------------------------------------------

    fn make_write(item_id: i64) -> PendingDbWrite {
        PendingDbWrite {
            device_id: "test-device".into(),
            item_id,
            content_type: "text".into(),
            content_b64: std::sync::Arc::from("dGVzdA=="),
            wall_time: 1000,
            inserted_at_unix: 1,
            pruned_count: 0,
            next_sync_id: item_id + 1,
            attempts: 0,
        }
    }

    fn make_store_for_test() -> RelayStore {
        RelayStore::new(3600)
    }
}
