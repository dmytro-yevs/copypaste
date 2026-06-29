//! Eviction: TTL pruning, inactive-device cleanup, and orphan-map reclamation.
//!
//! [`super::RelayStore::prune_expired`] is called by the background TTL evictor
//! (`store.rs`); [`super::RelayStore::cleanup_inactive_devices`] is called by the
//! same task to reclaim device records whose inbox has been empty too long.

use std::cmp::Reverse;
use std::sync::atomic::Ordering;

impl super::RelayStore {
    // -----------------------------------------------------------------------
    // Cleanup
    // -----------------------------------------------------------------------

    /// Remove device records (and their inbox + id-counter map entries) for
    /// devices that registered at least `inactive_threshold_secs` ago AND have
    /// an empty inbox. Wired into the background evictor (see `store.rs`) so the
    /// `devices` map and the per-device counter map are actually reclaimed
    /// (H1) — previously this was never called, so both grew without bound.
    ///
    /// Returns the number of device records removed.
    pub fn cleanup_inactive_devices(&mut self, inactive_threshold_secs: u64) -> usize {
        let inactive_ids: Vec<String> = self
            .devices
            .iter()
            .filter(|(id, record)| {
                // Evict on `last_seen`, not `registered_at`. A device that was
                // registered long ago but has actively pulled recently should NOT be
                // evicted — `registered_at` never advances after creation, so using it
                // would lock out any active receiver whose inbox happens to be empty.
                let old_enough = record.last_seen.elapsed().as_secs() >= inactive_threshold_secs;
                if !old_enough {
                    return false;
                }
                let inbox = self.sync_items.get(*id);
                let has_items = inbox.is_some_and(|items| !items.is_empty());
                !has_items
            })
            .map(|(id, _)| id.clone())
            .collect();

        let count = inactive_ids.len();
        for id in &inactive_ids {
            self.devices.remove(id);
            self.sync_items.remove(id);
            self.next_sync_id_per_device.remove(id);
            // Drop the SSE wake channel too (issue #26): the device record is
            // gone, so any open subscription would already have failed auth on
            // re-read; dropping the `Sender` here closes its receivers and keeps
            // the notifier map bounded by the live device set.
            self.sync_notifiers.remove(id);
            // R1b write-through: delete the device row; the FK `ON DELETE
            // CASCADE` reclaims its tokens + inbox atomically. This runs on the
            // background evictor, not a request, and the signature is infallible,
            // so a persistence failure is logged and skipped rather than
            // aborting the sweep — the in-memory removal already happened, and
            // the row will be re-reclaimed on the next sweep / next restart's
            // rehydrate (a stale row only re-loads a device with an empty inbox,
            // which the very next sweep evicts again).
            if let Err(e) = self.db.delete_device(id) {
                tracing::warn!(device_id = %id, error = %e, "relay: failed to delete inactive device from store");
            }
        }
        count
    }

    // -----------------------------------------------------------------------
    // Devices listing
    // -----------------------------------------------------------------------

    // Called by `list_devices_handler` in `routes/mod.rs` (`GET /devices`) in
    // the production binary. `#[path]`-include test binaries that compile
    // state.rs without routes/mod.rs see this as dead; allow suppresses the
    // lint in those test compilations.
    #[allow(dead_code)]
    pub fn list_devices(&self) -> Vec<String> {
        let mut records: Vec<&super::device::DeviceRecord> = self.devices.values().collect();
        records.sort_by_key(|r| Reverse(r.registered_at));
        records
            .into_iter()
            .take(100)
            .map(|r| r.device_id.clone())
            .collect()
    }

    // -----------------------------------------------------------------------
    // TTL eviction (see ADR-009)
    // -----------------------------------------------------------------------

    /// Drop sync items whose `inserted_at_unix + ttl_secs <= now_unix`.
    ///
    /// `now_unix` is supplied by the caller so unit tests can advance a
    /// virtual clock (`tokio::time::advance`) without touching the real
    /// system clock.
    ///
    /// Returns the number of items evicted (across all device inboxes).
    ///
    /// Inboxes belonging to a still-registered device are kept even when they
    /// drain to empty (the device keeps its registration). However, *orphaned*
    /// map entries — a `sync_items` inbox or a `next_sync_id_per_device`
    /// counter whose `device_id` is no longer in `devices` — are reclaimed
    /// here regardless of contents (H2): once the device record is gone the
    /// inbox is unreachable (reads require a live `verify_token`), so retaining
    /// it would just leak dead data. Without this, any inbox/counter that
    /// outlives its device would grow unboundedly; pruning keeps both maps
    /// bounded by the live device set.
    pub fn prune_expired(&mut self, now_unix: u64, ttl_secs: u64) -> usize {
        // Reclaim orphaned map entries regardless of TTL — these are pure
        // memory leaks unrelated to item age (H2). Bind `devices` to a local
        // shared borrow so the `retain` closures don't conflict with the
        // mutable borrow of the map being retained.
        // An inbox / counter whose device record is gone is unreachable (every
        // read path requires a live `verify_token`), so reclaim it regardless
        // of whether it still holds items — keeping it would leak dead data.
        let devices = &self.devices;
        self.sync_items
            .retain(|device_id, _| devices.contains_key(device_id));
        self.next_sync_id_per_device
            .retain(|device_id, _| devices.contains_key(device_id));
        // Reclaim SSE wake channels orphaned by a gone device record (issue #26).
        self.sync_notifiers
            .retain(|device_id, _| devices.contains_key(device_id));

        if ttl_secs == 0 {
            return 0;
        }
        let cutoff = now_unix.saturating_sub(ttl_secs);
        let mut evicted = 0usize;
        for inbox in self.sync_items.values_mut() {
            let before = inbox.len();
            inbox.retain(|item| item.inserted_at_unix > cutoff);
            evicted += before - inbox.len();
        }
        if evicted > 0 {
            self.evictions_total
                .fetch_add(evicted as u64, Ordering::Relaxed);
        }
        // R1b write-through: mirror the TTL eviction in SQL. The in-memory
        // sweep keeps items with `inserted_at_unix > cutoff`, i.e. evicts
        // `inserted_at_unix <= cutoff` — `Db::prune_expired(cutoff)` deletes
        // exactly that set, so memory and disk stay consistent. Runs on the
        // background evictor with an infallible signature, so a persistence
        // failure is logged and skipped; the same rows are deleted on the next
        // tick / re-evicted after a restart's rehydrate.
        if let Err(e) = self.db.prune_expired(cutoff) {
            tracing::warn!(error = %e, "relay: failed to prune expired items from store");
        }
        evicted
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use crate::state::test_helpers::*;

    #[test]
    fn cleanup_removes_old_inactive_devices() {
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
        let removed = store.cleanup_inactive_devices(0);
        assert_eq!(removed, 2);
        assert!(store.devices.is_empty());
        assert!(store.sync_items.is_empty());
    }

    #[test]
    fn cleanup_keeps_recently_registered_devices() {
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
        let removed = store.cleanup_inactive_devices(u64::MAX);
        assert_eq!(removed, 0);
        assert!(store.devices.contains_key(&device_a_id()));
        assert!(store.devices.contains_key(&device_b_id()));
    }

    #[test]
    fn cleanup_keeps_devices_with_items() {
        let mut store = make_store();
        store
            .register_device(
                device_a_id(),
                "Device A".into(),
                valid_key_b64(),
                valid_pop_b64(),
            )
            .unwrap();
        push_text(&mut store, &device_a_id(), 1000);
        let removed = store.cleanup_inactive_devices(0);
        assert_eq!(removed, 0, "device with items must not be removed");
    }

    /// `prune_expired` must reclaim `next_sync_id_per_device` counters and empty
    /// `sync_items` inboxes whose device record no longer exists, so those maps
    /// stay bounded by the live device set instead of leaking forever.
    #[test]
    fn prune_expired_reclaims_orphaned_maps() {
        let mut store = make_store();
        store
            .register_device(device_a_id(), "A".into(), valid_key_b64(), valid_pop_b64())
            .unwrap();
        push_text(&mut store, &device_a_id(), 1000);
        // Counter + inbox now exist for device A.
        assert!(store.next_sync_id_per_device.contains_key(&device_a_id()));
        assert!(store.sync_items.contains_key(&device_a_id()));

        // Forcibly drop *only* the device record, simulating a record removed
        // by some path that left the side maps behind.
        store.devices.remove(&device_a_id());

        let now = 1_000_000u64;
        store.prune_expired(now, 60);

        assert!(
            !store.next_sync_id_per_device.contains_key(&device_a_id()),
            "orphaned id-counter entry must be reclaimed"
        );
        assert!(
            !store.sync_items.contains_key(&device_a_id()),
            "orphaned inbox must be reclaimed"
        );
    }

    /// Empty inboxes belonging to a *still-registered* device must be kept (the
    /// device retains its registration regardless of inbox activity).
    #[test]
    fn prune_expired_keeps_empty_inbox_of_registered_device() {
        let mut store = make_store();
        store
            .register_device(device_a_id(), "A".into(), valid_key_b64(), valid_pop_b64())
            .unwrap();
        // Empty inbox, device still registered.
        store.prune_expired(u64::MAX, 1);
        assert!(store.sync_items.contains_key(&device_a_id()));
    }
}
