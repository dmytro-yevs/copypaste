//! Sync-item inbox: types, SSE notifications, push/pull/delete.
//!
//! [`SyncItem`] is the encrypted envelope stored per-device.  The per-device
//! [`tokio::sync::broadcast`] channel carries wake ticks for open SSE streams.
//! Push/pull/delete and SSE management are implemented on [`super::RelayStore`].

use std::sync::atomic::Ordering;

use crate::error::RelayError;

use super::quota::effective_history_cap;

mod delete;
mod pull;
mod sse;
mod types;

pub use types::SyncItem;

// ---------------------------------------------------------------------------
// RelayStore: push
// ---------------------------------------------------------------------------

impl super::RelayStore {
    // -----------------------------------------------------------------------
    // Push (wall-clock sync protocol)
    // -----------------------------------------------------------------------

    /// Store an encrypted item in `device_id`'s sync inbox.
    ///
    /// Validates that the decoded `content_b64` does not exceed `max_item_bytes`.
    /// Prunes the oldest item when the inbox exceeds `MAX_PUSH_ITEMS_PER_DEVICE`.
    /// Returns the auto-assigned integer ID.
    //
    // The HTTP `push` handler calls `push_item_decoded` directly (decodes
    // payload before locking the store), so this self-decoding wrapper has no
    // production caller. Used only by tests. When `quota-tiers` is enabled
    // (e.g. --all-features) it is included but has no non-test caller —
    // allow suppresses dead_code.
    #[cfg(any(test, feature = "quota-tiers"))]
    #[allow(dead_code)] // intentional: test helper, no production caller
    pub fn push_item(
        &mut self,
        device_id: &str,
        content_type: String,
        content_b64: String,
        wall_time: u64,
        max_item_bytes: usize,
    ) -> Result<i64, RelayError> {
        use base64::engine::general_purpose::STANDARD as B64;
        use base64::Engine;
        // Decode here so callers that haven't already measured the payload
        // (tests, non-HTTP callers) keep working unchanged, then delegate to
        // the length-aware path. The HTTP `push` handler instead decodes once
        // *before* taking the store mutex and calls `push_item_decoded`
        // directly, so the large base64 decode never runs under the lock (perf).
        let decoded_len = B64
            .decode(&content_b64)
            .map_err(|_| RelayError::BadRequest("content_b64 must be valid base64".to_string()))?
            .len();
        self.push_item_decoded(
            device_id,
            content_type,
            content_b64,
            decoded_len,
            wall_time,
            max_item_bytes,
        )
    }

    /// Store an encrypted item whose decoded length is already known.
    ///
    /// Identical to `push_item` except the caller passes the
    /// pre-computed `decoded_len` (the number of decoded ciphertext bytes) so
    /// the base64 payload is **not** decoded again while the store mutex is
    /// held. `content_b64` is still validated for membership/quotas; it is the
    /// caller's responsibility to ensure `decoded_len` matches `content_b64`.
    pub fn push_item_decoded(
        &mut self,
        device_id: &str,
        content_type: String,
        content_b64: String,
        decoded_len: usize,
        wall_time: u64,
        max_item_bytes: usize,
    ) -> Result<i64, RelayError> {
        let tier = match self.devices.get(device_id) {
            Some(record) => record.tier,
            None => return Err(RelayError::DeviceNotFound),
        };

        if !matches!(content_type.as_str(), "text" | "image" | "file") {
            return Err(RelayError::BadRequest(
                "content_type must be 'text', 'image', or 'file'".to_string(),
            ));
        }

        if decoded_len > max_item_bytes {
            return Err(RelayError::PayloadTooLarge);
        }

        // Per-device counter, seeded from the inbox on first push so a
        // server restart cannot re-issue an id another item in the same
        // device's inbox already holds (security HIGH #3).
        let counter = self
            .next_sync_id_per_device
            .entry(device_id.to_string())
            .or_insert_with(|| {
                self.sync_items
                    .get(device_id)
                    .and_then(|inbox| inbox.iter().map(|i| i.id).max())
                    .map(|m| m.saturating_add(1))
                    .unwrap_or(1)
                    .max(1)
            });
        let id = *counter;
        // `checked_add` so an id-counter overflow returns a server error
        // instead of an unchecked-arithmetic panic (security HIGH #3).
        let next_counter = counter.checked_add(1).ok_or_else(|| {
            tracing::warn!(device_id, "sync id counter overflow");
            RelayError::Internal("sync id counter exhausted".into())
        })?;
        *counter = next_counter;
        // Copy out the new counter value so the `&mut` borrow of
        // `next_sync_id_per_device` ends here, before we mutably borrow
        // `sync_items` (the inbox) below. `next_counter` is a plain `i64`.

        // Fail closed on clock error: a stored inserted_at=0 would be treated as
        // epoch and pruned immediately by prune_expired (cutoff = now - ttl > 0),
        // silently losing every pushed item. Mirror verify_token: clock errors
        // return Internal rather than storing a bogus timestamp.
        let inserted_at_unix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| {
                tracing::error!(
                    "host clock is before UNIX_EPOCH; refusing to store item with inserted_at=0"
                );
                RelayError::Internal("server clock error; cannot record item insertion time".into())
            })?
            .as_secs();

        // CopyPaste-ux2i: take ownership of the base64 ciphertext as Arc<str>
        // ONCE (consumes the incoming String, no extra copy). The same Arc is
        // shared into the in-memory inbox and borrowed for the durable write,
        // and later cloned (refcount only) by `pull_items`.
        let content_b64: std::sync::Arc<str> = std::sync::Arc::from(content_b64);
        let inbox = self.sync_items.entry(device_id.to_string()).or_default();
        let item = SyncItem {
            id,
            content_type: content_type.clone(),
            content_b64: std::sync::Arc::clone(&content_b64),
            wall_time,
            inserted_at_unix,
        };
        // Keep the inbox sorted ascending by `wall_time` *on insert* (M4) so
        // `pull_items` can binary-search + slice instead of cloning and sorting
        // the whole inbox under the global mutex on every pull. The common case
        // is a monotonically increasing `wall_time`, which appends at the end
        // (O(1) amortised); out-of-order pushes use a binary-search insert.
        // Ties keep insertion order via `partition_point` (insert after equal
        // `wall_time`), preserving the prior stable-sort behaviour.
        let pos = inbox.partition_point(|existing| existing.wall_time <= wall_time);
        inbox.insert(pos, item);

        // History quota: cap the inbox at the tighter of:
        //   1. the operator-configured `max_items_per_device` (from
        //      `RelayConfig` / `RELAY_MAX_ITEMS_PER_DEVICE`), which is the
        //      live ceiling sourced from config — previously this field was
        //      dead and the compile-time `MAX_PUSH_ITEMS_PER_DEVICE` was
        //      always used instead, ignoring the env var entirely;
        //   2. the tier-aware effective limit (`effective_history_cap`),
        //      which is itself the tighter of the absolute hard cap and the
        //      tier's `max_history_items`.
        // Enforced as a silent prune of the oldest items (the fan-out sender
        // cannot know which recipient inboxes are full — see relay v2 quotas
        // plan).
        //
        // CopyPaste-1uqb: prune by server-assigned `id` (ascending = earliest
        // arrival), NOT by client-supplied `wall_time`. The inbox is sorted by
        // `wall_time` for the pull cursor, so an intra-account attacker can
        // forge a low `wall_time` to make their malicious item sort near the
        // front and escape eviction while displacing legitimate items.
        // Server-side `id` is assigned monotonically by the relay and is never
        // client-controlled, so pruning by the smallest `id` values removes
        // the truly earliest-arriving items regardless of what the sender set
        // as `wall_time`.
        let cap = effective_history_cap(tier).min(self.max_items_per_device);
        let pruned = if inbox.len() > cap {
            let n = inbox.len() - cap;
            // Collect the n smallest ids (earliest server-assigned arrivals)
            // to prune. The inbox is wall_time-sorted, not id-sorted, so a
            // linear scan for the minimum-id entries is required. O(n*cap)
            // but n and cap are both small (n is usually 1, cap ≤ 500).
            let mut ids_to_prune: Vec<i64> = inbox.iter().map(|it| it.id).collect();
            ids_to_prune.sort_unstable();
            ids_to_prune.truncate(n);
            let prune_set: std::collections::HashSet<i64> = ids_to_prune.into_iter().collect();
            inbox.retain(|it| !prune_set.contains(&it.id));
            n
        } else {
            0
        };
        // End the mutable borrow of `inbox` before touching `self.db` (disjoint
        // fields, but keep the sequence explicit).

        // CopyPaste-crh3.70: move the large SQLite payload write (`insert_item`
        // + `delete_oldest_items`) OUTSIDE the store-mutex critical section.
        //
        // Previously all three DB calls (insert + optional delete_oldest +
        // set_next_sync_id) ran synchronously while the `std::sync::Mutex`
        // was held, blocking the OS thread (and every concurrent Axum worker
        // waiting for the lock) for the duration of the disk I/O. With a 10 MiB
        // ciphertext, `insert_item` alone can take tens of milliseconds —
        // serialising all concurrent pushes behind a single write.
        //
        // Fix: `insert_item` and `delete_oldest_items` are enqueued to the
        // retry task (fast: a VecDeque push under the mutex + an atomic notify).
        // `set_next_sync_id` is kept synchronous because it is a tiny metadata
        // UPDATE (no payload) and is needed to keep the on-disk ID watermark
        // up-to-date across restarts (tested by `next_sync_id_watermark_is_seeded_from_db_on_restart`).
        // If that synchronous write fails (transient SQLite hiccup), the
        // `next_sync_id` field in `PendingDbWrite` lets the retry task
        // fix it (via MAX semantics — see `db_set_next_sync_id_retry`).
        //
        // The item is already in the in-memory inbox and will be delivered to
        // pollers and SSE streams regardless of the deferred write outcome.
        // Durability for the ciphertext arrives within µs (immediate retry-task
        // wake via `db_write_notify`).

        // Synchronous, fast: advances the ID watermark in DB (< 1 µs typically).
        if let Err(db_err) = self.db.set_next_sync_id(device_id, next_counter) {
            tracing::warn!(
                device_id,
                item_id = id,
                error = %db_err,
                "CopyPaste-crh3.70: sync_id DB write failed; retry task will repair it"
            );
            // Non-fatal: retry task re-sets via MAX semantics (see PendingDbWrite.next_sync_id).
        }

        // Deferred (potentially large): enqueue insert_item + delete_oldest.
        // The retry task processes this outside the store mutex and wakes
        // immediately via `db_write_notify`.
        use super::persistence::PendingDbWrite;
        let accepted = self.push_retry_queue.enqueue(PendingDbWrite {
            device_id: device_id.to_string(),
            item_id: id,
            content_type: content_type.clone(),
            content_b64: std::sync::Arc::clone(&content_b64),
            wall_time,
            inserted_at_unix,
            pruned_count: pruned,
            next_sync_id: next_counter,
            attempts: 0,
        });
        if !accepted {
            tracing::warn!(
                device_id,
                item_id = id,
                "CopyPaste-crh3.70: DB-write queue full; item not durable (in-memory only)"
            );
        }
        // Wake the retry task immediately — non-blocking signal.
        self.db_write_notify.notify_one();

        // Increment Prometheus counter — items_total tracks all accepted
        // pushes regardless of later eviction (counter semantics).
        self.items_total.fetch_add(1, Ordering::Relaxed);

        // SSE push (issue #26): wake any open `GET /devices/:id/subscribe`
        // stream for this RECIPIENT device now that the inbox write has
        // committed (the item is in `self.sync_items[device_id]` above). The
        // woken stream re-reads the inbox from its cursor and flushes the new
        // item. Fired here (still under the store mutex, after the write) so a
        // subscriber can never be woken before the item is visible.
        self.notify_subscribers(device_id);

        Ok(id)
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use base64::engine::general_purpose::STANDARD as B64;
    use base64::Engine;

    use crate::error::RelayError;
    use crate::quota::Tier;
    use crate::state::test_helpers::*;
    use crate::state::RelayStore;

    use crate::state::quota::{effective_history_cap, history_cap_for_limit};
    use crate::state::MAX_PUSH_ITEMS_PER_DEVICE;

    #[test]
    fn push_returns_ascending_ids() {
        let mut store = make_store();
        store
            .register_device(
                device_a_id(),
                "Device A".into(),
                valid_key_b64(),
                valid_pop_b64(),
            )
            .unwrap();
        let id1 = push_text(&mut store, &device_a_id(), 1000);
        let id2 = push_text(&mut store, &device_a_id(), 2000);
        assert!(id2 > id1);
    }

    #[test]
    fn push_rejects_unknown_content_type() {
        let mut store = make_store();
        store
            .register_device(
                device_a_id(),
                "Device A".into(),
                valid_key_b64(),
                valid_pop_b64(),
            )
            .unwrap();
        let err = store
            .push_item(
                &device_a_id(),
                "video".to_string(),
                B64.encode(b"x"),
                1000,
                10 * 1024 * 1024,
            )
            .unwrap_err();
        assert!(matches!(err, RelayError::BadRequest(_)));
    }

    #[test]
    fn push_rejects_invalid_base64() {
        let mut store = make_store();
        store
            .register_device(
                device_a_id(),
                "Device A".into(),
                valid_key_b64(),
                valid_pop_b64(),
            )
            .unwrap();
        let err = store
            .push_item(
                &device_a_id(),
                "text".to_string(),
                "!!!not-base64!!!".to_string(),
                1000,
                10 * 1024 * 1024,
            )
            .unwrap_err();
        assert!(matches!(err, RelayError::BadRequest(_)));
    }

    #[test]
    fn push_rejects_oversized_payload() {
        let mut store = make_store();
        store
            .register_device(
                device_a_id(),
                "Device A".into(),
                valid_key_b64(),
                valid_pop_b64(),
            )
            .unwrap();
        let big = B64.encode(b"hello world");
        let err = store
            .push_item(&device_a_id(), "text".to_string(), big, 1000, 10)
            .unwrap_err();
        assert!(matches!(err, RelayError::PayloadTooLarge));
    }

    #[test]
    fn push_quota_prunes_oldest_item() {
        let mut store = make_store();
        store
            .register_device(
                device_a_id(),
                "Device A".into(),
                valid_key_b64(),
                valid_pop_b64(),
            )
            .unwrap();
        for t in 1u64..=(MAX_PUSH_ITEMS_PER_DEVICE as u64 + 1) {
            push_text(&mut store, &device_a_id(), t);
        }
        let items = store
            .pull_items(&device_a_id(), 0, None, usize::MAX)
            .unwrap();
        assert_eq!(items.len(), MAX_PUSH_ITEMS_PER_DEVICE);
        let min_wt = items.iter().map(|i| i.wall_time).min().unwrap();
        assert_eq!(min_wt, 2, "oldest item must be evicted");
    }

    #[test]
    fn stats_counts_correctly() {
        let mut store = make_store();
        store
            .register_device(
                device_a_id(),
                "Device A".into(),
                valid_key_b64(),
                valid_pop_b64(),
            )
            .unwrap();
        store
            .register_device(
                device_b_id(),
                "Device B".into(),
                unique_key(1),
                valid_pop_b64(),
            )
            .unwrap();
        let (devices, items) = store.stats();
        assert_eq!(devices, 2);
        assert_eq!(items, 0);
        push_text(&mut store, &device_a_id(), 1000);
        let (_, items) = store.stats();
        assert_eq!(items, 1);
    }

    /// The history quota is enforced inside `push_item` keyed by the device's
    /// tier. A push is never rejected with an error: instead the inbox is capped
    /// at the effective limit by pruning the oldest items.
    #[test]
    fn free_tier_inbox_never_exceeds_history_cap() {
        let mut store = make_store();
        store
            .register_device_with_tier(
                device_a_id(),
                "Device A".into(),
                valid_key_b64(),
                valid_pop_b64(),
                Tier::Free,
            )
            .unwrap();

        // The effective cap is min(500, 1000) = 500 for Free tier.
        let effective_cap =
            MAX_PUSH_ITEMS_PER_DEVICE.min(Tier::Free.max_history_items().unwrap_or(usize::MAX));

        for t in 1u64..=(effective_cap as u64 + 50) {
            push_text(&mut store, &device_a_id(), t);
        }

        let items = store
            .pull_items(&device_a_id(), 0, None, usize::MAX)
            .unwrap();
        assert!(
            items.len() <= effective_cap,
            "inbox must never exceed the effective history cap ({effective_cap}), got {}",
            items.len()
        );
    }

    /// History-quota enforcement must consult the device's tier: a Pro device
    /// (unlimited history) is bounded only by the absolute hard cap, never by a
    /// tier history limit.
    #[test]
    fn pro_tier_history_is_bounded_only_by_hard_cap() {
        let mut store = make_store();
        store
            .register_device_with_tier(
                device_a_id(),
                "Device A".into(),
                valid_key_b64(),
                valid_pop_b64(),
                Tier::Pro,
            )
            .unwrap();

        for t in 1u64..=(MAX_PUSH_ITEMS_PER_DEVICE as u64 + 50) {
            push_text(&mut store, &device_a_id(), t);
        }

        let items = store
            .pull_items(&device_a_id(), 0, None, usize::MAX)
            .unwrap();
        // Pro tier has no history limit, so only the absolute hard cap applies.
        assert_eq!(items.len(), MAX_PUSH_ITEMS_PER_DEVICE);
    }

    /// The effective per-inbox history cap is the tighter of the absolute hard
    /// cap and the tier's `max_history_items`.
    #[test]
    fn effective_history_cap_is_tier_aware() {
        assert_eq!(effective_history_cap(Tier::Free), MAX_PUSH_ITEMS_PER_DEVICE);
        assert_eq!(effective_history_cap(Tier::Pro), MAX_PUSH_ITEMS_PER_DEVICE);
        // A tier limit tighter than the hard cap must win.
        let tight_tier_limit = 10usize;
        assert!(tight_tier_limit < MAX_PUSH_ITEMS_PER_DEVICE);
        assert_eq!(history_cap_for_limit(Some(tight_tier_limit)), 10);
        // Unlimited tier history (`None`) clamps to the hard cap.
        assert_eq!(history_cap_for_limit(None), MAX_PUSH_ITEMS_PER_DEVICE);
    }

    /// CopyPaste-1uqb: When the inbox overflows its cap, the items evicted must
    /// be chosen by server-assigned `id` (smallest = earliest arrival), not by
    /// client-supplied `wall_time`.
    #[test]
    fn inbox_overflow_evicts_by_server_id_not_client_wall_time() {
        let mut store = RelayStore::new_with_cap(3600, 2 /* cap = 2 items */);
        store
            .register_device(
                device_a_id(),
                "Device A".into(),
                valid_key_b64(),
                valid_pop_b64(),
            )
            .unwrap();

        let id_first = store
            .push_item(
                &device_a_id(),
                "text".to_string(),
                B64.encode(b"first"),
                1000,
                10 * 1024 * 1024,
            )
            .unwrap();

        let id_attacker = store
            .push_item(
                &device_a_id(),
                "text".to_string(),
                B64.encode(b"attacker"),
                1, // deliberately old wall_time
                10 * 1024 * 1024,
            )
            .unwrap();

        let _id_third = store
            .push_item(
                &device_a_id(),
                "text".to_string(),
                B64.encode(b"third"),
                2000,
                10 * 1024 * 1024,
            )
            .unwrap();

        let remaining: Vec<i64> = store
            .pull_items(&device_a_id(), 0, None, usize::MAX)
            .unwrap()
            .into_iter()
            .map(|it| it.id)
            .collect();

        assert_eq!(remaining.len(), 2, "inbox must be at cap after overflow");
        assert!(
            !remaining.contains(&id_first),
            "CopyPaste-1uqb: the earliest-arrived item (id={id_first}) must be evicted"
        );
        assert!(
            remaining.contains(&id_attacker),
            "attacker item (id={id_attacker}) must survive — it arrived AFTER id_first"
        );
        assert!(
            remaining.contains(&_id_third),
            "the third item must survive"
        );
    }

    /// The config `max_items_per_device` must govern the inbox cap.
    #[test]
    fn max_items_per_device_config_governs_cap() {
        const CUSTOM_CAP: usize = 5;
        let mut store = RelayStore::new_with_cap(3600, CUSTOM_CAP);
        store
            .register_device(
                device_a_id(),
                "Device A".into(),
                valid_key_b64(),
                valid_pop_b64(),
            )
            .unwrap();
        for t in 1u64..=(CUSTOM_CAP as u64 + 3) {
            push_text(&mut store, &device_a_id(), t);
        }
        let items = store
            .pull_items(&device_a_id(), 0, None, usize::MAX)
            .unwrap();
        assert_eq!(
            items.len(),
            CUSTOM_CAP,
            "inbox must be capped at the config-supplied max_items_per_device ({CUSTOM_CAP})"
        );
    }
}
