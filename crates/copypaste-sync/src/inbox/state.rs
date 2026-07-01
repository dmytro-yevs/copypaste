//! Shared bounded ring-buffer state between senders and the forwarding task.

use std::collections::VecDeque;

use crate::metrics::SyncLagCounter;
use crate::protocol::WireItem;

/// Shared state between [`super::SyncInboxSender`]s and the forwarding task.
///
/// `pub(super)`: reachable from `sender.rs` and `forwarder.rs` (both children
/// of `inbox`), not part of the crate's public API.
pub(super) struct InboxState {
    /// Bounded ring buffer: oldest item at front, newest at back.
    ring: VecDeque<WireItem>,
    /// Maximum number of items the ring may hold before evicting the oldest.
    capacity: usize,
    /// Metered counter for evicted (dropped-oldest) items.
    lag: SyncLagCounter,
}

impl InboxState {
    pub(super) fn new(capacity: usize, lag: SyncLagCounter) -> Self {
        debug_assert!(capacity > 0, "inbox capacity must be > 0");
        Self {
            ring: VecDeque::with_capacity(capacity),
            capacity,
            lag,
        }
    }

    /// Enqueue `item`. Returns the number of items dropped to make room (0 or 1).
    pub(super) fn push(&mut self, item: WireItem) -> u64 {
        let dropped = if self.ring.len() >= self.capacity {
            // Ring is full: evict the oldest entry to make room for the fresh item.
            self.ring.pop_front();
            1u64
        } else {
            0u64
        };
        if dropped > 0 {
            // Record the eviction in the lag counter so operators can observe it.
            self.lag.record_lagged(dropped);
        }
        self.ring.push_back(item);
        dropped
    }

    /// Dequeue the oldest item, if any.
    pub(super) fn pop(&mut self) -> Option<WireItem> {
        self.ring.pop_front()
    }
}
