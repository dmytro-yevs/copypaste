//! Non-blocking producer handle for the P2P sync inbox.

use std::sync::Arc;

use tokio::sync::{Mutex, Notify};
use tracing::debug;

use super::replay_guard::ReplayGuard;
use super::state::InboxState;
use crate::protocol::WireItem;

/// Non-blocking sender handle.
///
/// Each P2P connection task should hold one clone; all clones share the same
/// underlying ring buffer via `Arc`. Enqueue operations take a `Mutex` for a
/// very short critical section (no I/O) and wake the forwarding task via
/// `Notify`.
///
/// # Replay guard (CopyPaste-4cyh)
///
/// Each `SyncInboxSender` owns a private [`ReplayGuard`] that is NOT shared
/// with other senders.  The guard is intentionally per-sender (per P2P
/// connection) so that the guard lifetime is bounded to a single mTLS
/// session: when the connection is dropped the guard is dropped too, which is
/// the right scope — a new session (new mTLS handshake, new session key) gets
/// a fresh guard.
///
/// **Do not make `SyncInboxSender` `Clone`** if the replay guard needs to be
/// exclusive to the connection.  If multiple tasks on the same connection need
/// to enqueue items, wrap the sender in an `Arc<Mutex<SyncInboxSender>>` or
/// use a single task with a channel.  The `Clone` impl is removed to enforce
/// this: a cloned sender would share the ring-buffer `Arc` but carry an
/// independent guard, allowing the same item to slip past the guard once per
/// clone — defeating the dedup.
pub struct SyncInboxSender {
    /// `pub(super)`: constructed directly by `forwarder::SyncInboxForwarder::start`
    /// (a sibling module under `inbox`); not part of the crate's public API.
    pub(super) state: Arc<Mutex<InboxState>>,
    pub(super) notify: Arc<Notify>,
    /// Per-connection replay guard — not shared between senders.
    pub(super) guard: ReplayGuard,
}

impl SyncInboxSender {
    /// Enqueue `item` without blocking, after passing it through the replay guard.
    ///
    /// If the item's `(item_id, lamport_ts)` pair has already been seen in
    /// this session it is silently dropped (returns `0` dropped from ring,
    /// replay logged at `debug` level).  A new `lamport_ts` for the same
    /// `item_id` is admitted normally — it is a legitimate CRDT update.
    ///
    /// If the ring is full the **oldest** queued item is evicted (drop-oldest)
    /// and the eviction is recorded in the `SyncLagCounter`. Returns the
    /// number of ring-eviction drops (0 or 1); replay drops are not counted
    /// here as they are not a ring-capacity event.
    ///
    /// This call takes a `Mutex` for a short critical section (no I/O inside)
    /// and notifies the forwarding task. It **never** blocks on the downstream
    /// consumer.
    pub async fn try_enqueue(&mut self, item: WireItem) -> u64 {
        // Replay check — must happen before acquiring the ring lock.
        // The guard is not shared, so no lock is needed here.
        if self.guard.is_replay(&item.item_id, item.lamport_ts) {
            debug!(
                item_id = %item.item_id,
                lamport_ts = item.lamport_ts,
                "sync inbox: dropping replayed WireItem (CopyPaste-4cyh)"
            );
            return 0;
        }

        let dropped = {
            let mut state = self.state.lock().await;
            state.push(item)
        };
        self.notify.notify_one();
        dropped
    }
}
