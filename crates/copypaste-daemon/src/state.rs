//! What the capture loop and the IPC server share.
//!
//! One `Arc`, built once in `main`: the shared [`Runtime`] that owns history,
//! settings, sync and the request dispatch, plus what only a long-lived
//! daemon has — the native clipboard, the socket lifecycle and readiness.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use copypaste_core::{Keyring, Store};
use copypaste_p2p::peers::PeerStore;
use copypaste_runtime::{Meta, Runtime, RuntimeParts, Settings};
use tokio::sync::{watch, OwnedRwLockReadGuard, RwLock};

use crate::clipboard::ClipboardSource;
use crate::host::{DaemonHost, SharedClipboard};

/// Everything the daemon shares between the capture loop and the IPC server.
pub struct AppState {
    pub runtime: Arc<Runtime>,
    /// `std::sync::Mutex`, not `tokio`'s: the guard is never held across an
    /// `.await`, so a `copy` cannot queue behind a capture tick for longer than
    /// one pasteboard access.
    clipboard: SharedClipboard,
    /// Never put in a client-visible string: it discloses the local username.
    db_path: PathBuf,
    /// Here rather than in `main` because [`copypaste_ipc::Method::Shutdown`]
    /// has to reach it: SIGTERM and the IPC verb take one teardown path.
    shutdown: watch::Sender<bool>,
    drain_release: watch::Sender<bool>,
    draining: AtomicBool,
    admissions: Arc<RwLock<()>>,
    host: Arc<DaemonHost>,
    ready: AtomicBool,
    observed_clipboard_available: AtomicBool,
}

/// The daemon reads the shared runtime's history, settings and peers in place.
impl std::ops::Deref for AppState {
    type Target = Runtime;

    fn deref(&self) -> &Runtime {
        &self.runtime
    }
}

/// What `main` opened, composed into a runtime around this daemon's clipboard.
pub struct Opened {
    pub db_path: PathBuf,
    pub store: Store,
    pub keyring: Arc<Keyring>,
    pub meta: Meta,
    pub settings: Arc<Settings>,
    pub peers: PeerStore,
    pub port: u16,
    pub clipboard: Box<dyn ClipboardSource>,
}

impl AppState {
    pub fn open(opened: Opened) -> anyhow::Result<Self> {
        let clipboard_available = opened.clipboard.is_available();
        let clipboard: SharedClipboard = Arc::new(Mutex::new(opened.clipboard));
        let host = Arc::new(DaemonHost::new(
            Arc::clone(&clipboard),
            Arc::clone(&opened.settings),
        ));
        let runtime = Runtime::assemble(RuntimeParts {
            data_dir: opened
                .db_path
                .parent()
                .unwrap_or(std::path::Path::new("."))
                .to_path_buf(),
            store: opened.store,
            keyring: opened.keyring,
            meta: opened.meta,
            settings: opened.settings,
            peers: opened.peers,
            port: opened.port,
            host: Arc::clone(&host) as _,
        })?;
        Ok(Self {
            runtime: Arc::new(runtime),
            clipboard,
            db_path: opened.db_path,
            shutdown: watch::channel(false).0,
            drain_release: watch::channel(false).0,
            draining: AtomicBool::new(false),
            admissions: Arc::new(RwLock::new(())),
            host,
            ready: AtomicBool::new(false),
            observed_clipboard_available: AtomicBool::new(clipboard_available),
        })
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
        self.runtime.shutdown();
        // Fails only if every receiver has been dropped, which means the tasks
        // this would have stopped are already gone.
        let _ = self.shutdown.send(true);
    }

    pub fn db_path(&self) -> &std::path::Path {
        &self.db_path
    }

    pub fn clipboard(&self) -> MutexGuard<'_, Box<dyn ClipboardSource>> {
        crate::host::lock(&self.clipboard)
    }

    pub fn backend_name(&self) -> &'static str {
        self.host.backend_name()
    }

    /// A capture succeeded: the same copy feedback the runtime asks for.
    pub fn copy_feedback(&self) {
        copypaste_runtime::RuntimeHost::copy_feedback(self.host.as_ref());
    }

    pub fn is_ready(&self) -> bool {
        self.ready.load(Ordering::Acquire)
    }

    pub fn set_ready(&self, ready: bool) {
        self.ready.store(ready, Ordering::Release);
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::test_state;
    use copypaste_ipc::EventKind;

    #[test]
    fn capture_events_only_report_actual_transitions() {
        let (state, _dir) = test_state("capture-state-events");
        let mut events = state.subscribe_runtime_events();
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
            fn set_contents(&mut self, _: &str, _: bool) -> anyhow::Result<()> {
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
        let mut events = state.subscribe_runtime_events();
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
        assert_eq!(copypaste_runtime::EVENT_BUFFER, 256);
    }

    #[test]
    fn sync_status_revision_is_published_once_after_subscription() {
        let (state, _dir) = test_state("sync-status-events");
        state.note_sync_status_changed();
        let mut events = state.subscribe_events();
        state.note_sync_status_changed();
        let event = events.try_recv().unwrap();
        assert_eq!(event.event, EventKind::Peers);
        assert_eq!(event.sync_status.unwrap().revision, 0);
        state.note_sync_status_changed();
        assert!(events.try_recv().is_err());
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
