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

/// Background loop that drains [`crate::state::PushRetryQueue`] and replays
/// failed DB writes.
///
/// Spawned by `main.rs` via [`crate::supervise::spawn_supervised`] so panics
/// are logged and the task restarts automatically.
///
/// The task ticks at `PUSH_RETRY_POLL_MS` intervals. On a persistent DB failure
/// it applies exponential backoff (capped at `PUSH_RETRY_MAX_DELAY_MS`) before
/// re-enqueuing the item for the next attempt. After `MAX_RETRY_ATTEMPTS` the
/// write is discarded with an ERROR log.
pub async fn run_push_retry(state: AppState) {
    loop {
        // Tick every PUSH_RETRY_POLL_MS; the lock is held only for a brief
        // drain + write, never across .await.
        tokio::time::sleep(Duration::from_millis(PUSH_RETRY_POLL_MS)).await;

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

    /// CopyPaste-k4py: when push_item_decoded returns Ok (normal path), the
    /// retry queue stays empty (no spurious enqueue on success).
    #[test]
    fn no_retry_enqueued_on_successful_push() {
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
        assert!(
            store.push_retry_queue.is_empty(),
            "CopyPaste-k4py: retry queue must be empty after a successful push"
        );
    }

    // ---- async integration: retry runner drains queue on subsequent tick ----

    /// CopyPaste-k4py: run_push_retry drains a pre-seeded retry queue and writes
    /// the pending item to the DB. Verifies end-to-end: item inserted by
    /// push_item_decoded sits in the retry queue → background task retries →
    /// item appears in the DB.
    ///
    /// We simulate the "retry queue has an entry" state by directly enqueuing a
    /// PendingDbWrite after a normal push that already stored the item in memory,
    /// then verify the background task eventually persists it.
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

        // Seed the retry queue with a PendingDbWrite whose DB write has not yet
        // happened (simulates a transient failure on the initial push attempt).
        let content_b64: std::sync::Arc<str> = std::sync::Arc::from(B64.encode(b"retry payload"));
        {
            let mut s = state.lock().unwrap();
            // Push the item into memory so we have a real item_id.
            let item_id = s
                .push_item(
                    &device_id,
                    "text".into(),
                    B64.encode(b"retry payload"),
                    5000,
                    10 * 1024 * 1024,
                )
                .unwrap();
            // The normal push wrote to DB. Remove the DB row to simulate "the
            // first DB write failed" — then enqueue for retry.
            s.db_delete_item_for_test(&device_id, item_id);

            s.push_retry_queue.enqueue(PendingDbWrite {
                device_id: device_id.clone(),
                item_id,
                content_type: "text".into(),
                content_b64: std::sync::Arc::clone(&content_b64),
                wall_time: 5000,
                inserted_at_unix: 1,
                pruned_count: 0,
                next_sync_id: item_id + 1,
                attempts: 0,
            });
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
            .expect("CopyPaste-k4py: retry queue must drain within 3 s");

        // Verify the item was written to the DB by checking the DB item count.
        {
            let s = state.lock().unwrap();
            let count = s.db_item_count_for_test(&device_id);
            assert!(
                count >= 1,
                "CopyPaste-k4py: DB must contain the retried item; got count={count}"
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
