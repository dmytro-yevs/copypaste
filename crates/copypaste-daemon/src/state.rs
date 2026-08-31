//! What the capture loop and the IPC server share.
//!
//! One `Arc`, no optional fields, built once in `main`. Optional shared slots
//! would make readiness impossible to prove; the
//! `Option`s existed only because construction was spread across the builders.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

use copypaste_core::{Detector, Keyring, Store};
use copypaste_ipc::{DiagnosticCounters, EventData, EventKind};
use tokio::sync::{broadcast, watch, OwnedRwLockReadGuard, RwLock};

use crate::clipboard::ClipboardSource;
use crate::cloud::Cloud;
use crate::meta::Meta;
use crate::p2p::P2p;
use crate::settings::Settings;

/// Everything the daemon shares between the capture loop and the IPC server.
pub struct AppState {
    pub store: Store,
    /// `Arc` because `copypaste_core::StoreSource` holds one for as long as the
    /// peer listener runs, and the device secret is not `Clone` on purpose.
    pub keyring: Arc<Keyring>,
    /// `Arc` because the cloud upload gate holds one too. A second `Detector`
    /// would be a second ruleset (AGENTS.md rule 1) free to disagree with the
    /// one capture uses.
    pub detector: Arc<Detector>,
    /// `std::sync::Mutex`, not `tokio`'s: the guard is never held across an
    /// `.await`, so a `copy` cannot queue behind a capture tick for longer than
    /// one pasteboard access.
    clipboard: Mutex<Box<dyn ClipboardSource>>,
    pub meta: Meta,
    pub p2p: P2p,
    pub cloud: Cloud,
    pub settings: Settings,
    /// Never put in a client-visible string: it discloses the local username.
    db_path: PathBuf,
    /// `broadcast` rather than a list of senders: a subscriber that stops
    /// draining lags and is told so, instead of applying backpressure to the
    /// capture loop. A dropped event is safe — it says only *that* something
    /// changed, and the client re-reads.
    events: broadcast::Sender<EventData>,
    /// Here rather than in `main` because [`copypaste_ipc::Method::Shutdown`]
    /// has to reach it: an app that did not start this daemon cannot signal the
    /// process. `main` holds no second sender, so SIGTERM and the IPC verb take
    /// one teardown path rather than two.
    shutdown: watch::Sender<bool>,
    drain_release: watch::Sender<bool>,
    draining: AtomicBool,
    admissions: Arc<RwLock<()>>,
    backend_name: &'static str,
    ready: AtomicBool,
    capture_running: AtomicBool,
    /// Set rather than passed: decided outside construction, read by `status`,
    /// and threading it through `new` would touch every caller for a value none
    /// of them has an opinion about. Same for `ready` and `index_purged`.
    /// The clipboard's own two counters live on the port; these are the ones
    /// nothing else owns.
    sensitive_swept: AtomicU64,
    index_purged: AtomicU64,
    started_at: Instant,
}

/// How many change events are buffered per subscriber before it is told it
/// lagged. Small on purpose: the recovery for a lag is one extra re-read.
const EVENT_BUFFER: usize = 256;

impl AppState {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        store: Store,
        keyring: Arc<Keyring>,
        detector: Arc<Detector>,
        clipboard: Box<dyn ClipboardSource>,
        meta: Meta,
        p2p: P2p,
        cloud: Cloud,
        settings: Settings,
        db_path: PathBuf,
    ) -> Self {
        let backend_name = clipboard.backend_name();
        Self {
            store,
            keyring,
            detector,
            clipboard: Mutex::new(clipboard),
            meta,
            p2p,
            cloud,
            settings,
            db_path,
            events: broadcast::channel(EVENT_BUFFER).0,
            shutdown: watch::channel(false).0,
            drain_release: watch::channel(false).0,
            draining: AtomicBool::new(false),
            admissions: Arc::new(RwLock::new(())),
            backend_name,
            ready: AtomicBool::new(false),
            capture_running: AtomicBool::new(false),
            sensitive_swept: AtomicU64::new(0),
            index_purged: AtomicU64::new(0),
            started_at: Instant::now(),
        }
    }

    pub fn shutdown_rx(&self) -> watch::Receiver<bool> {
        self.shutdown.subscribe()
    }

    pub fn drain_release_rx(&self) -> watch::Receiver<bool> {
        self.drain_release.subscribe()
    }

    pub fn is_draining(&self) -> bool {
        self.draining.load(Ordering::Acquire)
    }

    pub async fn admit_request(&self) -> Option<OwnedRwLockReadGuard<()>> {
        if self.is_draining() {
            return None;
        }
        let permit = Arc::clone(&self.admissions).read_owned().await;
        (!self.is_draining()).then_some(permit)
    }

    pub async fn wait_for_admitted_requests(&self) {
        drop(self.admissions.write().await);
    }

    pub fn release_drain_listener(&self) {
        let _ = self.drain_release.send(true);
    }

    /// Begin an orderly shutdown. Idempotent, and safe from any thread.
    ///
    /// Every task finishes the unit of work it is in before observing this, so
    /// a capture already past the clipboard read still reaches the database.
    pub fn request_shutdown(&self) {
        self.draining.store(true, Ordering::Release);
        // Fails only if every receiver has been dropped, which means the tasks
        // this would have stopped are already gone.
        let _ = self.shutdown.send(true);
    }

    pub fn db_path(&self) -> &std::path::Path {
        &self.db_path
    }

    pub fn subscribe(&self) -> broadcast::Receiver<EventData> {
        self.events.subscribe()
    }

    /// History changed *on this device*. The split from
    /// [`AppState::note_remote_change`] is what keeps the two from feeding each
    /// other: both wake the watchers, but only a local change pulls the sync
    /// loops to their floor. A round that applied a peer's row would otherwise
    /// reset the cadence, provoke an empty round on the other device, and ring
    /// back.
    pub fn note_local_change(&self) {
        self.p2p.node().note_local_version(copypaste_core::now_ms());
        self.publish(EventKind::Items, false, 0);
        self.p2p.wake();
        self.cloud.wake();
    }

    /// The auto-wipe sweep deleted `count` secrets — the only local change the
    /// user did not ask for, so the count travels with it.
    pub fn note_sensitive_swept(&self, count: u32) {
        self.sensitive_swept
            .fetch_add(u64::from(count), Ordering::Relaxed);
        self.publish(EventKind::Items, false, count);
        self.p2p.wake();
        self.cloud.wake();
    }

    /// [`AppState::note_local_change`] plus the bit that separates a capture
    /// from a delete, a pin or an import: without it a client would post the
    /// "copied" notification on every one of those too. The daemon does not
    /// read `notify_on_copy` — the event says what happened, the setting says
    /// what to do about it, and the surface that owns the notification reads
    /// it.
    pub fn note_capture(&self, floor_ms: i64) {
        self.p2p.node().note_local_version(floor_ms);
        self.publish(EventKind::Items, true, 0);
        self.p2p.wake();
        self.cloud.wake();
    }

    /// History changed because a peer or the cloud delivered something.
    pub fn note_remote_change(&self) {
        self.publish(EventKind::Items, false, 0);
    }

    pub fn note_peers_changed(&self) {
        self.publish(EventKind::Peers, false, 0);
    }

    fn publish(&self, event: EventKind, captured: bool, swept: u32) {
        // Having no subscriber is the ordinary case: the CLI does not subscribe
        // and the app may not be running. `item_count` costs a `SELECT COUNT(*)`
        // over the live set, so it is not built for nobody. Safe against the
        // subscribe-then-ack ordering in `server::listener` because
        // `subscribe()` increments the count synchronously.
        if self.events.receiver_count() == 0 {
            return;
        }
        let _ = self.events.send(EventData {
            event,
            item_count: self.store.count().unwrap_or(0),
            captured,
            swept,
        });
    }

    /// Lock recovery rather than propagation: a poisoned clipboard mutex means
    /// a previous pasteboard call panicked, not that the handle is unusable.
    /// Refusing every later `copy` would be a worse outcome than retrying.
    pub fn clipboard(&self) -> MutexGuard<'_, Box<dyn ClipboardSource>> {
        self.clipboard
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn backend_name(&self) -> &'static str {
        self.backend_name
    }

    pub fn is_ready(&self) -> bool {
        self.ready.load(Ordering::Acquire)
    }

    pub fn set_ready(&self, ready: bool) {
        self.ready.store(ready, Ordering::Release);
    }

    pub fn capture_running(&self) -> bool {
        self.capture_running.load(Ordering::Acquire)
    }

    pub fn set_capture_running(&self, running: bool) {
        self.capture_running.store(running, Ordering::Release);
    }

    pub fn set_index_purged(&self, purged: u64) {
        self.index_purged.store(purged, Ordering::Release);
    }

    /// Assembled here rather than in the status handler because two of the five
    /// are behind the clipboard mutex: a caller reading them itself would take
    /// that lock a second time for the same reply.
    pub fn counters(&self) -> DiagnosticCounters {
        let (rejected_too_large, lost_intermediates) = {
            let clipboard = self.clipboard();
            (
                clipboard.rejected_too_large_count(),
                clipboard.lost_intermediates_count(),
            )
        };
        DiagnosticCounters {
            rejected_too_large,
            lost_intermediates,
            sensitive_swept: self.sensitive_swept.load(Ordering::Relaxed),
            index_purged: self.index_purged.load(Ordering::Acquire),
            uptime_secs: self.started_at.elapsed().as_secs(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::test_state;

    #[test]
    fn event_buffer_capacity_is_two_fifty_six() {
        assert_eq!(EVENT_BUFFER, 256);
    }

    #[tokio::test]
    async fn admission_rechecks_drain_after_waiting_for_a_permit() {
        let (state, _dir) = test_state("admission-race");
        let blocker = state.admissions.write().await;
        let waiter = tokio::spawn({
            let state = Arc::clone(&state);
            async move { state.admit_request().await }
        });
        tokio::task::yield_now().await;

        state.request_shutdown();
        drop(blocker);

        assert!(
            waiter
                .await
                .expect("admission task must not panic")
                .is_none(),
            "a request that waited through drain was admitted"
        );
    }
}
