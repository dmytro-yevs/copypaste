//! Replay-attack protection (CopyPaste-4cyh).
//!
//! [`SyncInboxSender::try_enqueue`](super::SyncInboxSender::try_enqueue) can
//! pass each incoming [`WireItem`](crate::protocol::WireItem) through a
//! per-sender [`ReplayGuard`] before enqueuing it, but the daemon's actual
//! P2P pump does not currently route through `SyncInboxSender`: it
//! constructs its own `ReplayGuard` inline, per-connection, in
//! `copypaste-daemon`'s `p2p::framed_pump::run_peer_connection_framed`
//! (CopyPaste-sreb) — see the `inbox` module doc for why. The guard maintains
//! a bounded set of
//! `(item_id, lamport_ts)` pairs seen in this session.  An exact duplicate —
//! same item_id AND same lamport_ts — is silently dropped; only the first
//! delivery is enqueued.
//!
//! Crucially, a WireItem carrying the **same item_id but a strictly higher
//! lamport_ts** is NOT treated as a replay: it is a legitimate CRDT update
//! (the remote peer applied a newer write to the same logical item) and is
//! admitted normally so LWW merge can apply it.
//!
//! The guard is bounded to [`REPLAY_GUARD_CAPACITY`] entries; when the set
//! fills the oldest entries are evicted in insertion order (LRU-ish) to
//! prevent unbounded memory growth during long-running sessions.  Because the
//! guard is per-sender (i.e. per-P2P-connection) it does not need to be
//! shared between multiple P2P tasks — each mTLS connection carries its own
//! [`SyncInboxSender`](super::SyncInboxSender) and therefore its own guard.
//!
//! Note: daemon-level cross-session dedup (across re-pairings) is a
//! separate concern handled by the UNIQUE constraint on `item_id` in
//! SQLite and the LWW merge in `sync_orch`.  The guard here closes only
//! the within-session replay window (CopyPaste-4cyh form 2 / form 3 partial).
use std::collections::{HashMap, VecDeque};

/// Maximum number of `(item_id, lamport_ts)` pairs the [`ReplayGuard`] retains
/// per session before evicting the oldest.
///
/// 4096 covers any realistic burst of distinct items in a single P2P sync
/// session while keeping per-connection heap usage well under 1 MiB (each
/// entry is ~40–80 bytes for a UUID string key + i64 value).
pub const REPLAY_GUARD_CAPACITY: usize = 4096;

/// Per-session replay-attack guard (CopyPaste-4cyh).
///
/// Tracks `(item_id, lamport_ts)` pairs received on one P2P connection.
/// Call [`ReplayGuard::is_replay`] before enqueuing an item; the method
/// records the pair on first sight and returns `true` for every subsequent
/// delivery of the identical pair (replay), `false` for new or updated items.
///
/// A pair with the **same item_id but a higher lamport_ts** is admitted
/// (returns `false`) — it is a valid CRDT update, not a replay.
pub struct ReplayGuard {
    /// Maps item_id → set of lamport_ts values seen for that item.
    /// Using a `Vec<i64>` per item_id instead of HashSet<(String,i64)> lets us
    /// store multiple lamport timestamps for the same item cheaply without a
    /// nested HashMap or a second string allocation.
    seen: HashMap<String, Vec<i64>>,
    /// Insertion order for eviction: (item_id, lamport_ts) in arrival order.
    order: VecDeque<(String, i64)>,
    /// Maximum number of (item_id, lamport_ts) pairs to retain.
    capacity: usize,
}

impl ReplayGuard {
    /// Create a new guard with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            seen: HashMap::new(),
            order: VecDeque::with_capacity(capacity.min(1024)),
            capacity: capacity.max(1),
        }
    }

    /// Return `true` if this `(item_id, lamport_ts)` pair has already been
    /// seen in this session (i.e. it is a replay and should be dropped).
    /// Return `false` if the pair is new — records it and admits the item.
    ///
    /// A pair with the **same item_id but a different (higher) lamport_ts** is
    /// always new — it represents a legitimate CRDT update from the peer and
    /// must NOT be rejected.
    pub fn is_replay(&mut self, item_id: &str, lamport_ts: i64) -> bool {
        // Fast path: check whether this exact (item_id, lamport_ts) was seen.
        if let Some(tss) = self.seen.get(item_id) {
            if tss.contains(&lamport_ts) {
                // Exact duplicate — this is a replay.
                return true;
            }
        }

        // New pair — record it, evicting the oldest entry if at capacity.
        if self.order.len() >= self.capacity {
            if let Some((old_id, old_ts)) = self.order.pop_front() {
                // Remove the evicted lamport_ts from the seen map.
                if let Some(tss) = self.seen.get_mut(&old_id) {
                    tss.retain(|&t| t != old_ts);
                    if tss.is_empty() {
                        self.seen.remove(&old_id);
                    }
                }
            }
        }

        self.seen
            .entry(item_id.to_owned())
            .or_default()
            .push(lamport_ts);
        self.order.push_back((item_id.to_owned(), lamport_ts));

        false
    }
}

/// The replay guard itself: unit tests for `ReplayGuard::is_replay`.
#[cfg(test)]
mod tests {
    use super::{ReplayGuard, REPLAY_GUARD_CAPACITY};

    #[test]
    fn first_delivery_not_a_replay() {
        let mut g = ReplayGuard::new(REPLAY_GUARD_CAPACITY);
        assert!(
            !g.is_replay("item-a", 1),
            "first delivery must not be flagged as replay"
        );
    }

    #[test]
    fn second_delivery_same_pair_is_replay() {
        let mut g = ReplayGuard::new(REPLAY_GUARD_CAPACITY);
        g.is_replay("item-a", 1); // record first delivery
        assert!(
            g.is_replay("item-a", 1),
            "exact duplicate (item_id + lamport_ts) must be flagged as replay"
        );
    }

    #[test]
    fn same_item_higher_lamport_not_replay() {
        let mut g = ReplayGuard::new(REPLAY_GUARD_CAPACITY);
        g.is_replay("item-a", 10); // first version
        assert!(
            !g.is_replay("item-a", 20),
            "higher lamport_ts on same item_id must NOT be a replay (it is a CRDT update)"
        );
    }

    #[test]
    fn different_item_not_replay() {
        let mut g = ReplayGuard::new(REPLAY_GUARD_CAPACITY);
        g.is_replay("item-a", 10);
        assert!(
            !g.is_replay("item-b", 10),
            "a different item_id with the same lamport_ts must not be flagged as replay"
        );
    }

    #[test]
    fn eviction_allows_re_recording_old_pair() {
        // Capacity 2: inserting 3 pairs evicts the first.
        let mut g = ReplayGuard::new(2);
        g.is_replay("item-a", 1); // slot 1 — order=[a1]
        g.is_replay("item-b", 2); // slot 2 — order=[a1,b2]
        g.is_replay("item-c", 3); // slot 3 — evicts (a1) → order=[b2,c3]

        // ("item-c", 3) is still in the guard — must be flagged as replay.
        assert!(
            g.is_replay("item-c", 3),
            "most-recent pair must still be flagged as replay"
        );

        // After eviction, ("item-a", 1) is no longer tracked — re-inserting it
        // is treated as a new delivery, not a replay.
        // NOTE: re-inserting (a,1) triggers another eviction of (b2) since the
        // guard is at capacity again.
        assert!(
            !g.is_replay("item-a", 1),
            "evicted pair must be re-admitted as a new delivery"
        );

        // Verify that ("item-c", 3) is still tracked after the re-admission.
        // order after the re-admission call: (c3) was at front and (a1) was pushed;
        // the eviction that occurred when re-admitting (a1) evicted (b2) — but
        // the previous is_replay("item-c", 3) call already recorded (c3) a second
        // time. Verify only what is deterministic: (a1) is now tracked.
        assert!(
            g.is_replay("item-a", 1),
            "re-admitted pair must be tracked after re-recording"
        );
    }
}
