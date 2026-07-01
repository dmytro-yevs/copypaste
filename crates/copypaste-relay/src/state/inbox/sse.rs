//! Per-device SSE wake-channel management (issue #26).
//!
//! Each `broadcast::Sender<()>` is a *wake* signal: when an item is pushed
//! into a device's inbox we `send(())` on that device's channel, waking every
//! open `GET /devices/:id/subscribe` SSE stream so it re-reads the inbox from
//! its own cursor and flushes the new item(s). The relay never sends item
//! *data* over this channel — only a wake tick — so a lagged receiver simply
//! re-reads the inbox (no data loss).

use tokio::sync::broadcast;

use super::types::SYNC_NOTIFY_CHANNEL_CAP;

impl super::super::RelayStore {
    // -----------------------------------------------------------------------
    // SSE push notifications (issue #26)
    // -----------------------------------------------------------------------

    /// Subscribe to `device_id`'s SSE wake channel, creating it lazily on the
    /// first subscribe. Returns a fresh `broadcast::Receiver<()>`; each open SSE
    /// stream holds its own receiver. The wake channel is a signal-only
    /// primitive (see [`super::super::RelayStore::sync_notifiers`]) — the SSE handler re-reads the
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
    pub(super) fn notify_subscribers(&self, device_id: &str) {
        if let Some(tx) = self.sync_notifiers.get(device_id) {
            // Ignore the receiver count / send error: zero receivers is normal
            // (no device is currently subscribed) and not an error condition.
            let _ = tx.send(());
        }
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use crate::state::test_helpers::*;

    // ---- CopyPaste-0y04: SSE per-device connection cap ----------------------

    /// `notifier_receiver_count` must reflect the number of live SSE receivers.
    #[test]
    fn sse_receiver_count_tracks_live_subscriptions() {
        let mut store = make_store();
        store
            .register_device(device_a_id(), "A".into(), valid_key_b64(), valid_pop_b64())
            .unwrap();

        assert_eq!(store.notifier_receiver_count(&device_a_id()), 0);

        let rx1 = store.subscribe_notifier(&device_a_id());
        assert_eq!(store.notifier_receiver_count(&device_a_id()), 1);

        let rx2 = store.subscribe_notifier(&device_a_id());
        assert_eq!(store.notifier_receiver_count(&device_a_id()), 2);

        drop(rx1);
        assert_eq!(store.notifier_receiver_count(&device_a_id()), 1);

        drop(rx2);
        assert_eq!(store.notifier_receiver_count(&device_a_id()), 0);
    }

    /// Calling `notifier_receiver_count` on an unknown device returns 0 (no panic).
    #[test]
    fn sse_receiver_count_returns_zero_for_unknown_device() {
        let store = make_store();
        assert_eq!(store.notifier_receiver_count(&device_a_id()), 0);
    }
}
