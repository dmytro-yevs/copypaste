//! Sync-item inbox: types, SSE notifications, push/pull/delete.
//!
//! [`SyncItem`] is the encrypted envelope stored per-device.  The per-device
//! [`tokio::sync::broadcast`] channel carries wake ticks for open SSE streams.
//! Push/pull/delete and SSE management are implemented on [`super::RelayStore`].

use std::sync::atomic::Ordering;

use tokio::sync::broadcast;

use crate::error::RelayError;
use crate::models::PullItem;

use super::{effective_history_cap, MAX_PULL_BYTES_BUDGET};

/// A single encrypted item in the wall-clock push/pull sync protocol.
pub struct SyncItem {
    /// Auto-incremented integer ID (unique per device inbox, ascending).
    pub id: i64,
    pub content_type: String,
    /// Opaque base64 ciphertext. `Arc<str>` (CopyPaste-ux2i) so `pull_items`
    /// clones a refcount under the global store mutex instead of memcpy-ing the
    /// full payload; the cloned `Arc` is handed straight to the `PullItem`.
    pub content_b64: std::sync::Arc<str>,
    /// Sender wall-clock time (Unix epoch milliseconds).
    pub wall_time: u64,
    /// Server-side wall-clock time at insert (Unix epoch seconds). Used for
    /// TTL eviction independent of (untrusted) sender `wall_time`. Read by
    /// `prune_expired` (in this module) and the background evictor in `store.rs`.
    pub inserted_at_unix: u64,
}

/// Capacity of each per-device SSE wake channel. A small ring buffer is
/// sufficient because the payload is a contentless wake tick: if a burst of
/// pushes overflows it, the receiver observes `RecvError::Lagged` and simply
/// re-reads the inbox from its cursor, picking up every missed item. Sized to
/// absorb a modest burst without forcing a lag-driven full re-read on every push.
// Used by `subscribe_notifier`, which is called from the production SSE route
// (`routes/items.rs`). `#[path]`-include test binaries that compile state.rs
// without the routes module do not exercise this path; those test crates
// suppress dead_code at the crate level (see individual test file headers).
pub(super) const SYNC_NOTIFY_CHANNEL_CAP: usize = 64;

// ---------------------------------------------------------------------------
// RelayStore: inbox management + SSE
// ---------------------------------------------------------------------------

impl super::RelayStore {
    // -----------------------------------------------------------------------
    // SSE push notifications (issue #26)
    // -----------------------------------------------------------------------

    /// Subscribe to `device_id`'s SSE wake channel, creating it lazily on the
    /// first subscribe. Returns a fresh `broadcast::Receiver<()>`; each open SSE
    /// stream holds its own receiver. The wake channel is a signal-only
    /// primitive (see [`super::RelayStore::sync_notifiers`]) — the SSE handler re-reads the
    /// inbox from its cursor on every wake, so a missed/lagged tick can never
    /// drop an item.
    // Called from the production SSE `subscribe` route (`routes/items.rs`).
    // Previously marked `#[allow(dead_code)]` for `#[path]`-include test
    // binaries; those binaries now suppress the lint at the crate level.
    pub fn subscribe_notifier(&mut self, device_id: &str) -> broadcast::Receiver<()> {
        match self.sync_notifiers.get(device_id) {
            Some(tx) => tx.subscribe(),
            None => {
                let (tx, rx) = broadcast::channel(SYNC_NOTIFY_CHANNEL_CAP);
                self.sync_notifiers.insert(device_id.to_string(), tx);
                rx
            }
        }
    }

    /// Number of live SSE wake-channel receivers currently held for
    /// `device_id` (0 if no channel exists yet). Each open `subscribe` stream's
    /// producer task owns exactly one receiver, so this is the count of live
    /// SSE producer tasks for the device — used to assert that a producer tears
    /// down (drops its `rx`) on client disconnect (SSE leak regression test).
    // Called from `tests/sse_subscribe.rs` to verify SSE producer lifecycle.
    // Not called from the production binary or the lib unit-test build; allow
    // suppresses the dead_code lint in those compilation units.
    #[allow(dead_code)]
    pub fn notifier_receiver_count(&self, device_id: &str) -> usize {
        self.sync_notifiers
            .get(device_id)
            .map_or(0, broadcast::Sender::receiver_count)
    }

    /// Fire a wake tick on `device_id`'s SSE channel, if any stream is
    /// subscribed. A no-op when there are no subscribers (the lazily-created
    /// `Sender` is retained for the device's lifetime so re-subscribes reuse it).
    /// `send` returning `Err` means there are currently zero live receivers,
    /// which is benign — the next subscriber backfills from its cursor.
    fn notify_subscribers(&self, device_id: &str) {
        if let Some(tx) = self.sync_notifiers.get(device_id) {
            // Ignore the receiver count / send error: zero receivers is normal
            // (no device is currently subscribed) and not an error condition.
            let _ = tx.send(());
        }
    }

    // -----------------------------------------------------------------------
    // Push / Pull (wall-clock sync protocol)
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

    /// Return up to `limit` items in `device_id`'s sync inbox strictly after the
    /// `(since, since_id)` composite cursor, ordered ascending.
    ///
    /// # Contract
    ///
    /// This method returns [`RelayError::DeviceNotFound`] for an unknown
    /// `device_id`. In production every call-site goes through
    /// [`Self::verify_token`] first, which already collapses missing-device to
    /// [`RelayError::Unauthorized`]. Callers that skip `verify_token` will
    /// observe a `DeviceNotFound` rather than `Unauthorized` — that is
    /// intentional: `pull_items` is a pure data accessor with no security
    /// semantics of its own. **Always call `verify_token` before `pull_items`**
    /// on any authenticated route.
    ///
    /// Pagination is driven by a strictly-monotonic `(wall_time, id)` tuple
    /// rather than bare `wall_time` (relay H-1 / audit finding G). `wall_time`
    /// is a sender-supplied millisecond timestamp, so ties are possible; a
    /// `wall_time`-only cursor with a strict `>` floor would skip every item
    /// sharing a boundary timestamp when a page boundary fell mid-run, silently
    /// dropping items. The per-device `id` is unique and ascending, so the tuple
    /// `(wall_time, id)` is a total order with no ties: items qualify iff
    /// `(item.wall_time, item.id) > (since, since_id)`.
    ///
    /// `since_id` is optional for backward compatibility: when `None` the cursor
    /// degrades to the historical `wall_time`-only floor (`wall_time > since`),
    /// matching pre-cursor clients. New clients paginate by feeding back the
    /// last returned `(wall_time, id)` as `(since, since_id)`.
    ///
    /// The inbox is kept sorted by `wall_time` on insert (see `push_item`),
    /// and within an equal `wall_time` run `id` is ascending too (ids are issued
    /// monotonically and ties preserve insertion order), so the inbox is sorted
    /// by the full `(wall_time, id)` tuple. This binary-searches for the first
    /// item past the cursor and clones only the (at most `limit`) items it
    /// returns — it never clones+sorts the whole inbox under the global mutex
    /// (M4). A `limit` of `0` is treated as "no items" rather than "unbounded";
    /// callers wanting the whole window pass a large explicit cap.
    pub fn pull_items(
        &self,
        device_id: &str,
        since: u64,
        since_id: Option<i64>,
        limit: usize,
    ) -> Result<Vec<PullItem>, RelayError> {
        let inbox = self
            .sync_items
            .get(device_id)
            .ok_or(RelayError::DeviceNotFound)?;

        // First index strictly past the cursor. The inbox is sorted ascending by
        // `(wall_time, id)`, so everything from `start` onward qualifies (no full
        // scan/sort). With `since_id` we advance past every item up to and
        // including the cursor tuple; without it we fall back to the legacy
        // `wall_time`-only floor (`wall_time <= since`).
        let start = match since_id {
            Some(since_id) => {
                inbox.partition_point(|item| (item.wall_time, item.id) <= (since, since_id))
            }
            None => inbox.partition_point(|item| item.wall_time <= since),
        };

        // Collect at most `limit` items but also enforce a byte-budget cap
        // (MAX_PULL_BYTES_BUDGET) on the total content_b64 bytes cloned under
        // the global mutex. Without this an authenticated caller with
        // limit=MAX_PULL_LIMIT items × up to 10 MiB each could force ~5 GiB
        // of cloning while holding the lock, stalling all other requests (DoS).
        let mut budget_remaining = MAX_PULL_BYTES_BUDGET;
        let mut result = Vec::new();
        for item in inbox[start..].iter().take(limit) {
            let item_bytes = item.content_b64.len();
            if item_bytes > budget_remaining {
                break;
            }
            budget_remaining -= item_bytes;
            result.push(PullItem {
                id: item.id,
                content_type: item.content_type.clone(),
                // CopyPaste-ux2i: refcount bump, not a full-payload memcpy.
                content_b64: std::sync::Arc::clone(&item.content_b64),
                wall_time: item.wall_time,
            });
        }

        Ok(result)
    }

    // -----------------------------------------------------------------------
    // Delete
    // -----------------------------------------------------------------------

    /// Remove item `item_id` from `device_id`'s inbox (matched by id as string).
    pub fn delete_item(&mut self, device_id: &str, item_id: &str) -> Result<(), RelayError> {
        let parsed_id: i64 = item_id
            .parse()
            .map_err(|_| RelayError::BadRequest("item_id must be an integer".to_string()))?;

        let inbox = self
            .sync_items
            .get_mut(device_id)
            .ok_or(RelayError::DeviceNotFound)?;

        let before = inbox.len();
        inbox.retain(|item| item.id != parsed_id);
        if inbox.len() == before {
            return Err(RelayError::ItemNotFound);
        }
        // R1b write-through: remove the same row from the durable store. The
        // in-memory removal already succeeded (we only reach here when the item
        // existed), so propagate any persistence failure as 500.
        self.db.delete_item(device_id, parsed_id)?;
        Ok(())
    }
}
