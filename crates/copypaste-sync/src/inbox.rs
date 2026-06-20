/// Non-blocking inbox forwarder for the P2P sync pipeline.
///
/// # Problem (CopyPaste-bxsa)
///
/// The daemon wires P2P accept/connector tasks to `sync_orch` via an
/// `mpsc::channel::<WireItem>(64)` (`sync_incoming_tx` / `sync_incoming_rx`).
/// Every per-peer read task calls `incoming_tx.send(wire).await` when a frame
/// arrives from the peer. If `sync_orch` is slow to consume from
/// `sync_incoming_rx` — e.g. blocked on a DB write — all per-peer tasks park
/// on that `.await`. Back-pressure propagates: TCP receive-buffers fill,
/// mTLS accept() stalls on the OS socket queue, and new connections cannot
/// be established.
///
/// # Fix
///
/// [`SyncInboxForwarder`] decouples the two sides with:
///
/// 1. **A bounded ring buffer** (capacity `N`) protected by a `Mutex` +
///    `Notify`. P2P tasks call [`SyncInboxSender::try_enqueue`] which is
///    lock-take + push + notify — never blocks on the downstream consumer.
///
/// 2. **Drop-oldest policy when full**: if the ring already holds `N` items,
///    the *oldest* entry is evicted (not the new one) so the consumer always
///    receives the most recent data. Evictions are counted in a
///    [`crate::metrics::SyncLagCounter`] so operators can observe them.
///
/// 3. **A dedicated forwarding task** spawned by
///    [`SyncInboxForwarder::start`] that reads from the ring (`.await` on
///    `Notify`) and forwards each item to the real downstream
///    `mpsc::Sender<WireItem>` via `.send().await`. When the downstream
///    channel closes the task exits cleanly.
///
/// P2P tasks now hold a [`SyncInboxSender`] instead of the raw downstream
/// sender. The forwarding task is the only place that blocks on the downstream;
/// it is spawned once and is isolated from the accept loop.
use std::collections::VecDeque;
use std::sync::Arc;

use tokio::sync::{mpsc, Mutex, Notify};
use tracing::warn;

use crate::metrics::SyncLagCounter;
use crate::protocol::WireItem;

/// Shared state between [`SyncInboxSender`]s and the forwarding task.
struct InboxState {
    /// Bounded ring buffer: oldest item at front, newest at back.
    ring: VecDeque<WireItem>,
    /// Maximum number of items the ring may hold before evicting the oldest.
    capacity: usize,
    /// Metered counter for evicted (dropped-oldest) items.
    lag: SyncLagCounter,
}

impl InboxState {
    fn new(capacity: usize, lag: SyncLagCounter) -> Self {
        debug_assert!(capacity > 0, "inbox capacity must be > 0");
        Self {
            ring: VecDeque::with_capacity(capacity),
            capacity,
            lag,
        }
    }

    /// Enqueue `item`. Returns the number of items dropped to make room (0 or 1).
    fn push(&mut self, item: WireItem) -> u64 {
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
    fn pop(&mut self) -> Option<WireItem> {
        self.ring.pop_front()
    }
}

/// Non-blocking sender handle.
///
/// Each P2P connection task should hold one clone; all clones share the same
/// underlying ring buffer via `Arc`. Enqueue operations take a `Mutex` for a
/// very short critical section (no I/O) and wake the forwarding task via
/// `Notify`.
///
/// This is intentionally `Clone` so you can distribute one per spawned task.
#[derive(Clone)]
pub struct SyncInboxSender {
    state: Arc<Mutex<InboxState>>,
    notify: Arc<Notify>,
}

impl SyncInboxSender {
    /// Enqueue `item` without blocking.
    ///
    /// If the ring is full the **oldest** queued item is evicted (drop-oldest)
    /// and the eviction is recorded in the `SyncLagCounter`. Returns the
    /// number of items dropped (0 or 1).
    ///
    /// This call takes a `Mutex` for a short critical section (no I/O inside)
    /// and notifies the forwarding task. It **never** blocks on the downstream
    /// consumer.
    pub async fn try_enqueue(&self, item: WireItem) -> u64 {
        let dropped = {
            let mut state = self.state.lock().await;
            state.push(item)
        };
        self.notify.notify_one();
        dropped
    }
}

/// Forwarder that bridges the non-blocking ring to the downstream `mpsc`.
///
/// Construct with [`SyncInboxForwarder::new`], then call [`start`] to
/// spawn the forwarding task and obtain a [`SyncInboxSender`] to hand to
/// P2P tasks.
pub struct SyncInboxForwarder {
    state: Arc<Mutex<InboxState>>,
    notify: Arc<Notify>,
    downstream: mpsc::Sender<WireItem>,
}

impl SyncInboxForwarder {
    /// Create a new forwarder.
    ///
    /// * `capacity` — maximum items the ring buffer may hold before dropping
    ///   the oldest. Must be ≥ 1. Values ≥ 64 are recommended in production.
    /// * `downstream` — the real `mpsc::Sender<WireItem>` that `sync_orch`
    ///   reads from.
    /// * `lag` — shared counter incremented for each evicted item.
    pub fn new(capacity: usize, downstream: mpsc::Sender<WireItem>, lag: SyncLagCounter) -> Self {
        let capacity = capacity.max(1); // guard against 0
        let state = Arc::new(Mutex::new(InboxState::new(capacity, lag)));
        let notify = Arc::new(Notify::new());
        Self {
            state,
            notify,
            downstream,
        }
    }

    /// Spawn the forwarding task (on the current `tokio` runtime) and return a
    /// [`SyncInboxSender`] that P2P tasks can clone and share.
    ///
    /// The forwarding task runs until the downstream sender is closed (i.e.
    /// `sync_orch` shut down), after which it exits silently.
    pub fn start(self) -> SyncInboxSender {
        let state = Arc::clone(&self.state);
        let notify = Arc::clone(&self.notify);
        let downstream = self.downstream;

        tokio::spawn(async move {
            forward_loop(state, notify, downstream).await;
        });

        SyncInboxSender {
            state: self.state,
            notify: self.notify,
        }
    }
}

/// Forwarding task body: drain the ring buffer into the downstream sender.
///
/// Waits on `notify` when the ring is empty, then forwards as many items as
/// are available in one lock-hold before waiting again. Exits when the
/// downstream `send().await` returns `Err` (receiver dropped / shutdown).
async fn forward_loop(
    state: Arc<Mutex<InboxState>>,
    notify: Arc<Notify>,
    downstream: mpsc::Sender<WireItem>,
) {
    loop {
        // Wait until at least one item is enqueued.
        notify.notified().await;

        // Drain the ring while items are available.
        loop {
            let item = {
                let mut st = state.lock().await;
                st.pop()
            };
            match item {
                None => break, // ring emptied — wait for the next notify
                Some(wire) => {
                    if downstream.send(wire).await.is_err() {
                        // Downstream receiver closed (sync_orch shut down).
                        // Exit the forwarding task cleanly.
                        warn!("sync inbox: downstream channel closed — forwarding task exiting");
                        return;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wire_item(id: &str, lamport: i64) -> WireItem {
        WireItem {
            id: id.to_string(),
            item_id: format!("{id}-item"),
            content_type: "text".to_string(),
            content: Some(vec![0xAA]),
            content_nonce: Some(vec![0u8; 24]),
            blob_ref: None,
            is_sensitive: false,
            lamport_ts: lamport,
            wall_time: 1_700_000_000_000 + lamport,
            expires_at: None,
            app_bundle_id: None,
            origin_device_id: format!("dev-{id}"),
            key_version: 2,
            file_name: None,
            mime: None,
            deleted: false,
            pinned: false,
            pin_order: None,
        }
    }

    // -------------------------------------------------------------------------
    // CopyPaste-bxsa: enqueue MUST be non-blocking even when downstream is full
    // -------------------------------------------------------------------------

    /// Enqueueing to a full downstream channel must not block the caller.
    ///
    /// This is the primary regression guard for CopyPaste-bxsa: the P2P accept
    /// loop must be able to call try_enqueue and return immediately regardless
    /// of the downstream consumer's state. We simulate a slow / stalled
    /// downstream by creating an mpsc with capacity 1 and never reading from it,
    /// then verify that enqueue completes instantly.
    #[tokio::test]
    async fn enqueue_is_nonblocking_when_downstream_stalled() {
        // Downstream channel — we intentionally do NOT read from it so it fills.
        let (tx, _rx) = mpsc::channel::<WireItem>(1);
        let lag = SyncLagCounter::new();
        let forwarder = SyncInboxForwarder::new(4, tx, lag.clone());
        let sender = forwarder.start();

        // Enqueue more items than the downstream can absorb without blocking.
        // If try_enqueue ever blocked on the downstream this would deadlock.
        for i in 0..10i64 {
            let dropped = sender.try_enqueue(wire_item(&format!("item-{i}"), i)).await;
            // No assertion on dropped here — just must not deadlock.
            let _ = dropped;
        }

        // Reached here without deadlock — non-blocking contract upheld.
    }

    /// When the ring is full, the OLDEST item is evicted (not the newest).
    ///
    /// Verifies the drop-oldest policy: if we enqueue items [A, B, C] into a
    /// ring of capacity 2 (so C causes A to be dropped), the consumer should
    /// receive B then C — A must be gone.
    #[tokio::test]
    async fn drop_oldest_policy_when_ring_full() {
        // Large-capacity downstream so the forwarder can send without blocking.
        let (tx, mut rx) = mpsc::channel::<WireItem>(64);
        let lag = SyncLagCounter::new();
        // Ring capacity = 2: a third enqueue must evict the oldest.
        let forwarder = SyncInboxForwarder::new(2, tx, lag.clone());
        let sender = forwarder.start();

        let a = wire_item("A", 1);
        let b = wire_item("B", 2);
        let c = wire_item("C", 3); // causes A to be evicted

        // Enqueue all three — the ring can only hold 2.
        let dropped_a = sender.try_enqueue(a).await; // ring=[A]     dropped=0
        let dropped_b = sender.try_enqueue(b).await; // ring=[A,B]   dropped=0
        let dropped_c = sender.try_enqueue(c).await; // ring=[B,C]   dropped=1 (A evicted)

        assert_eq!(dropped_a, 0, "first two enqueues must not drop");
        assert_eq!(dropped_b, 0, "first two enqueues must not drop");
        assert_eq!(dropped_c, 1, "third enqueue into full ring must drop 1 (oldest)");

        // Lag counter must reflect the single eviction.
        assert_eq!(lag.total_dropped(), 1, "lag counter must record 1 eviction");

        // Let the forwarder drain; collect received items.
        // Give the forward task a moment to drain.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let mut received = Vec::new();
        while let Ok(item) = rx.try_recv() {
            received.push(item.id);
        }

        // Must receive B and C; A must have been evicted.
        assert!(
            !received.contains(&"A".to_string()),
            "evicted item A must not reach the downstream; got: {received:?}"
        );
        assert!(
            received.contains(&"B".to_string()),
            "B must be forwarded; got: {received:?}"
        );
        assert!(
            received.contains(&"C".to_string()),
            "C must be forwarded; got: {received:?}"
        );
    }

    /// Items enqueued into a non-full ring must all reach the downstream.
    #[tokio::test]
    async fn items_forwarded_to_downstream() {
        let (tx, mut rx) = mpsc::channel::<WireItem>(64);
        let lag = SyncLagCounter::new();
        let forwarder = SyncInboxForwarder::new(16, tx, lag);
        let sender = forwarder.start();

        for i in 0..5i64 {
            sender.try_enqueue(wire_item(&format!("item-{i}"), i)).await;
        }

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let mut ids = Vec::new();
        while let Ok(item) = rx.try_recv() {
            ids.push(item.id);
        }
        assert_eq!(ids.len(), 5, "all 5 items must reach downstream; got {ids:?}");
    }

    /// Forwarder exits cleanly when the downstream channel is closed (sync_orch
    /// shut down). Enqueues into the closed forwarder must not panic.
    #[tokio::test]
    async fn forwarder_exits_cleanly_when_downstream_closed() {
        let (tx, rx) = mpsc::channel::<WireItem>(8);
        let lag = SyncLagCounter::new();
        let forwarder = SyncInboxForwarder::new(8, tx, lag);
        let sender = forwarder.start();

        // Drop the receiver to simulate sync_orch shutdown.
        drop(rx);

        // Enqueue a few items — should not panic.
        for i in 0..3i64 {
            sender.try_enqueue(wire_item(&format!("item-{i}"), i)).await;
        }
        // Brief pause so the forward task can observe the closed channel.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        // No assertion — we just verify we reached here without panicking.
    }

    /// Multiple senders (simulating multiple P2P connections) can enqueue
    /// concurrently without races.
    #[tokio::test]
    async fn concurrent_senders_do_not_race() {
        let (tx, mut rx) = mpsc::channel::<WireItem>(256);
        let lag = SyncLagCounter::new();
        // Large ring so nothing is dropped.
        let forwarder = SyncInboxForwarder::new(256, tx, lag.clone());
        let sender = forwarder.start();

        // Spawn 8 tasks, each enqueuing 10 items.
        let mut handles = Vec::new();
        for task in 0u64..8 {
            let s = sender.clone();
            handles.push(tokio::spawn(async move {
                for i in 0i64..10 {
                    s.try_enqueue(wire_item(&format!("t{task}-{i}"), i)).await;
                }
            }));
        }
        for h in handles {
            h.await.expect("task must not panic");
        }

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let mut count = 0usize;
        while rx.try_recv().is_ok() {
            count += 1;
        }

        assert_eq!(count, 80, "all 80 items (8 × 10) must be forwarded; got {count}");
        assert_eq!(lag.total_dropped(), 0, "no items should be dropped with sufficient capacity");
    }

    /// The lag counter accumulates across multiple evictions in a single sender.
    #[tokio::test]
    async fn lag_counter_accumulates_across_evictions() {
        // Downstream channel that we intentionally never read from.
        let (tx, _rx) = mpsc::channel::<WireItem>(1);
        let lag = SyncLagCounter::new();
        // Very small ring: capacity 2.
        let forwarder = SyncInboxForwarder::new(2, tx, lag.clone());
        let sender = forwarder.start();

        // Enqueue 5 items into a ring of capacity 2 → 3 evictions (items 1, 2, 3).
        for i in 0..5i64 {
            sender.try_enqueue(wire_item(&format!("item-{i}"), i)).await;
        }

        // The forwarding task may try to forward some items, but the downstream
        // rx is held open with cap 1. Give it a moment to attempt forwarding.
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;

        // At least 3 evictions must have been recorded (may be more if the
        // downstream sent one item then stalled).
        let dropped = lag.total_dropped();
        assert!(
            dropped >= 3,
            "expected ≥ 3 evictions for 5 items into capacity-2 ring, got {dropped}"
        );
    }
}
