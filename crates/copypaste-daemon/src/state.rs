//! What the capture loop and the IPC server share.
//!
//! One `Arc`, no optional fields, built once in `main`. Optional shared slots
//! would make readiness impossible to prove; the
//! `Option`s existed only because construction was spread across the builders.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

use copypaste_core::{Keyring, Store};
use copypaste_ipc::{DiagnosticCounters, EventData, EventKind};
use tokio::sync::{broadcast, watch, OwnedRwLockReadGuard, RwLock};

use crate::clipboard::ClipboardSource;
use crate::meta::Meta;
use crate::p2p::P2p;
use crate::settings::Settings;

/// Everything the daemon shares between the capture loop and the IPC server.
pub struct AppState {
    pub file_downloads: copypaste_core::file_download::FileDownloads,
    pub modules: Arc<copypaste_modules::ModuleHost>,
    pub store: Store,
    pub spoilers: copypaste_core::spoiler::Spoilers,
    pub instant_clipboard: copypaste_core::sync::InstantClipboard,
    /// `Arc` because `copypaste_core::StoreSource` holds one for as long as the
    /// peer listener runs, and the device secret is not `Clone` on purpose.
    pub keyring: Arc<Keyring>,
    /// `std::sync::Mutex`, not `tokio`'s: the guard is never held across an
    /// `.await`, so a `copy` cannot queue behind a capture tick for longer than
    /// one pasteboard access.
    clipboard: Mutex<Box<dyn ClipboardSource>>,
    pub meta: Meta,
    pub p2p: P2p,
    pub settings: Settings,
    /// Never put in a client-visible string: it discloses the local username.
    db_path: PathBuf,
    /// `broadcast` rather than a list of senders: a subscriber that stops
    /// draining lags and is told so, instead of applying backpressure to the
    /// capture loop. A dropped event is safe — it says only *that* something
    /// changed, and the client re-reads.
    events: broadcast::Sender<EventData>,
    runtime_events: broadcast::Sender<copypaste_ipc::RuntimeEventData>,
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
    observed_clipboard_available: AtomicBool,
    capture_revision: AtomicU64,
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
        clipboard: Box<dyn ClipboardSource>,
        meta: Meta,
        p2p: P2p,
        settings: Settings,
        db_path: PathBuf,
    ) -> Self {
        let backend_name = clipboard.backend_name();
        let clipboard_available = clipboard.is_available();
        let events = broadcast::channel(EVENT_BUFFER).0;
        let modules = Arc::new(copypaste_modules::ModuleHost::new(
            db_path.parent().unwrap_or(std::path::Path::new(".")),
        ));
        let changed_events = events.clone();
        let changed_store = store.clone();
        modules.bind_search(
            store.clone(),
            Arc::new(move || {
                let _ = changed_events.send(EventData {
                    sync_status: None,
                    event: EventKind::Items,
                    item_count: changed_store.count().unwrap_or(0),
                    captured: false,
                    captured_item_id: None,
                });
            }),
        );
        Self {
            file_downloads: Default::default(),
            modules,
            store,
            instant_clipboard: Default::default(),
            spoilers: Default::default(),
            keyring,
            clipboard: Mutex::new(clipboard),
            meta,
            p2p,
            settings,
            db_path,
            events,
            runtime_events: broadcast::channel(EVENT_BUFFER).0,
            shutdown: watch::channel(false).0,
            drain_release: watch::channel(false).0,
            draining: AtomicBool::new(false),
            admissions: Arc::new(RwLock::new(())),
            backend_name,
            ready: AtomicBool::new(false),
            capture_running: AtomicBool::new(false),
            observed_clipboard_available: AtomicBool::new(clipboard_available),
            capture_revision: AtomicU64::new(0),
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
        self.file_downloads.cancel_all();
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

    pub fn subscribe_runtime(&self) -> broadcast::Receiver<copypaste_ipc::RuntimeEventData> {
        self.runtime_events.subscribe()
    }

    pub fn runtime_event_sender(&self) -> broadcast::Sender<copypaste_ipc::RuntimeEventData> {
        self.runtime_events.clone()
    }

    /// History changed *on this device*. The split from
    /// [`AppState::note_remote_change`] is what keeps the two from feeding each
    /// other: both wake the watchers, but only a local change pulls the sync
    /// loops to their floor. A round that applied a peer's row would otherwise
    /// reset the cadence, provoke an empty round on the other device, and ring
    /// back.
    pub fn note_local_change(&self) {
        self.p2p.node().note_local_version(copypaste_core::now_ms());
        self.publish(EventKind::Items, None);
        self.p2p.wake();
        self.modules.note_version(copypaste_core::now_ms());
    }

    /// [`AppState::note_local_change`] plus the bit that separates a capture
    /// from a delete, a pin or an import: without it a client would post the
    /// "copied" notification on every one of those too. The daemon does not
    /// read `notify_on_copy` — the event says what happened, the setting says
    /// what to do about it, and the surface that owns the notification reads
    /// it.
    pub fn note_capture(&self, floor_ms: i64, item_id: &str) {
        self.instant_clipboard.note_local(&self.store, floor_ms);
        self.p2p.node().note_local_version(floor_ms);
        self.publish(EventKind::Items, Some(item_id));
        self.p2p.wake();
        self.modules.note_version(copypaste_core::now_ms());
    }

    /// History changed because a peer or the cloud delivered something.
    pub fn note_remote_change(&self) {
        self.publish(EventKind::Items, None);
    }

    /// A received item reached the system clipboard. Reuse copy feedback without
    /// treating the receive as local activity or waking outbound sync.
    pub fn note_received_copy(&self, item_id: &str) {
        self.publish(EventKind::Items, Some(item_id));
        crate::notify::on_capture(self, true);
    }

    pub fn note_sync_status_changed(&self) {
        let _ = self.events.send(EventData {
            sync_status: Some(
                self.p2p
                    .node()
                    .sync_status(self.settings.get().sync_enabled),
            ),
            event: EventKind::Peers,
            item_count: self.store.count().unwrap_or(0),
            captured: false,
            captured_item_id: None,
        });
    }

    pub fn note_peers_changed(&self) {
        self.publish(EventKind::Peers, None);
    }

    fn publish(&self, event: EventKind, captured_item_id: Option<&str>) {
        if event == EventKind::Items {
            self.modules.history_changed();
        }
        // Having no subscriber is the ordinary case: the CLI does not subscribe
        // and the app may not be running. `item_count` costs a `SELECT COUNT(*)`
        // over the live set, so it is not built for nobody. Safe against the
        // subscribe-then-ack ordering in `server::listener` because
        // `subscribe()` increments the count synchronously.
        if self.events.receiver_count() == 0 {
            return;
        }
        let _ = self.events.send(EventData {
            sync_status: None,
            event,
            item_count: self.store.count().unwrap_or(0),
            captured: captured_item_id.is_some(),
            captured_item_id: captured_item_id.map(str::to_owned),
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

    pub fn clipboard_available(&self) -> bool {
        self.clipboard().is_available()
    }

    pub fn set_capture_running(&self, running: bool) {
        if self.capture_running.swap(running, Ordering::AcqRel) != running {
            self.note_capture_state_changed();
        }
    }

    pub fn capture_revision(&self) -> u64 {
        self.capture_revision.load(Ordering::Acquire)
    }

    pub fn observe_clipboard_availability(&self, available: bool) {
        if self
            .observed_clipboard_available
            .swap(available, Ordering::AcqRel)
            != available
        {
            self.note_capture_state_changed();
        }
    }

    /// Publish only when subscribed; status includes no clipboard payload.
    pub fn note_capture_state_changed(&self) {
        self.capture_revision.fetch_add(1, Ordering::AcqRel);
        if self.runtime_events.receiver_count() == 0 {
            return;
        }
        if let Some(copypaste_ipc::ResponseData::Status(status)) =
            crate::server::items::status(self, 0).data
        {
            let _ = self.runtime_events.send(copypaste_ipc::RuntimeEventData {
                file_operation: None,
                change: None,
                progress: None,
                capture: Some(status),
            });
        }
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
            uptime_secs: self.started_at.elapsed().as_secs(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::test_state;

    #[test]
    fn capture_events_only_report_actual_transitions() {
        let (state, _dir) = test_state("capture-state-events");
        let mut events = state.subscribe_runtime();
        state.set_capture_running(true);
        let first = events.try_recv().unwrap().capture.unwrap();
        assert!(first.capture_running);
        assert_eq!(first.capture_revision, 1);
        state.set_capture_running(true);
        assert!(events.try_recv().is_err());
        state.set_capture_running(false);
        let second = events.try_recv().unwrap().capture.unwrap();
        assert!(!second.capture_running);
        assert_eq!(second.capture_revision, 2);
    }

    #[test]
    fn clipboard_availability_changes_are_published_once() {
        use crate::clipboard::{Capture, CapturePolicy, ClipboardSource};
        use std::sync::atomic::AtomicBool;
        struct Source(Arc<AtomicBool>);
        impl ClipboardSource for Source {
            fn poll(&mut self) -> Option<Capture> {
                None
            }
            fn poll_with_policy(&mut self, _: CapturePolicy<'_>) -> Option<Capture> {
                None
            }
            fn set_contents(&mut self, _: &str) -> anyhow::Result<()> {
                Ok(())
            }
            fn backend_name(&self) -> &'static str {
                "availability-test"
            }
            fn is_available(&self) -> bool {
                self.0.load(Ordering::Acquire)
            }
        }
        let available = Arc::new(AtomicBool::new(true));
        let (state, _dir) = crate::testutil::test_state_with_clipboard(
            "capture-availability",
            Box::new(Source(Arc::clone(&available))),
        );
        let mut events = state.subscribe_runtime();
        available.store(false, Ordering::Release);
        state.observe_clipboard_availability(false);
        let status = events.try_recv().unwrap().capture.unwrap();
        assert!(!status.clipboard_available);
        assert_eq!(status.capture_revision, 1);
        state.observe_clipboard_availability(false);
        assert!(events.try_recv().is_err());
    }

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
