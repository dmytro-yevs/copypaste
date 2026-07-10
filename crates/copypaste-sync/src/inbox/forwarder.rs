//! Forwarder construction/supervision — bridges the ring buffer to the
//! downstream `mpsc` channel via a dedicated forwarding task.

use std::sync::Arc;

use tokio::sync::{mpsc, Mutex, Notify};
use tracing::warn;

use super::replay_guard::{ReplayGuard, REPLAY_GUARD_CAPACITY};
use super::sender::SyncInboxSender;
use super::state::InboxState;
use crate::metrics::SyncLagCounter;
use crate::protocol::WireItem;

/// Forwarder that bridges the non-blocking ring to the downstream `mpsc`.
///
/// Construct with [`SyncInboxForwarder::new`], then call [`Self::start`] to
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
    /// [`SyncInboxSender`] that the P2P connection task uses to enqueue items.
    ///
    /// The returned sender carries its own [`ReplayGuard`] scoped to this
    /// session.  If multiple tasks on the same connection need to enqueue
    /// items, the caller should wrap the sender in an `Arc<Mutex<…>>` rather
    /// than calling `start()` multiple times (each call creates an independent
    /// guard, defeating per-session dedup).
    ///
    /// The forwarding task runs until the downstream sender is closed (i.e.
    /// `sync_orch` shut down), after which it exits silently.
    pub fn start(self) -> SyncInboxSender {
        let state = Arc::clone(&self.state);
        let notify = Arc::clone(&self.notify);
        let downstream = self.downstream;

        // CopyPaste-crh3.93: supervise the forwarding task. Previously the
        // JoinHandle was dropped immediately, so a panic inside `forward_loop`
        // was swallowed by tokio's default hook — all P2P deliveries for this
        // session would stop silently while callers kept enqueuing into a
        // never-drained ring buffer. We now await the handle in a supervisor and
        // log a panic at ERROR.
        let forward = tokio::spawn(async move {
            forward_loop(state, notify, downstream).await;
        });
        tokio::spawn(async move {
            let _ = supervise_forward_task(forward).await;
        });

        SyncInboxSender {
            state: self.state,
            notify: self.notify,
            // Fresh replay guard for this session — scoped to the lifetime of
            // this sender (i.e. this mTLS connection).
            guard: ReplayGuard::new(REPLAY_GUARD_CAPACITY),
        }
    }
}

/// CopyPaste-crh3.93: await a forwarding task's `JoinHandle` and, if it ended in
/// a panic, log at ERROR rather than letting tokio's default hook swallow it.
///
/// Returns `true` iff the task panicked (used by tests). A *cancelled* task is a
/// normal shutdown path and is not logged. This turns a silent "P2P deliveries
/// stopped" failure mode into a visible operator-facing error.
pub(super) async fn supervise_forward_task(handle: tokio::task::JoinHandle<()>) -> bool {
    match handle.await {
        Ok(()) => false,
        Err(join_err) if join_err.is_panic() => {
            tracing::error!(
                error = %join_err,
                "SyncInboxForwarder forward_loop panicked — P2P deliveries for this \
                 session have stopped; the ring buffer will no longer drain"
            );
            true
        }
        Err(_cancelled) => false,
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
