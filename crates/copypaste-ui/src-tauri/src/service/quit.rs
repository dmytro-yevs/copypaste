use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Arc;
use std::sync::Mutex;

use super::{child::ChildExitCode, Supervisor};
use tauri::{AppHandle, Manager as _, Runtime, WebviewWindow};
#[cfg(not(target_os = "android"))]
use tauri::{WebviewUrl, WebviewWindowBuilder};

pub(super) const MSG_QUIT_FAILED: &str = "CopyPaste could not safely stop the background service.";

pub(super) struct ReapCompletion {
    pub(super) result: Option<std::io::Result<ChildExitCode>>,
    pub(super) reservation: Option<ReapReservation>,
}
impl ReapCompletion {
    pub(super) fn finish(mut self) -> (std::io::Result<ChildExitCode>, Option<ReapReservation>) {
        (
            self.result.take().expect("completion has one result"),
            self.reservation.take(),
        )
    }
}
impl Drop for ReapCompletion {
    fn drop(&mut self) {
        drop(self.reservation.take());
    }
}

pub(super) enum ReapReservation {
    Ordinary(ShutdownPermit),
    #[cfg(any(test, target_os = "macos", target_os = "windows"))]
    Update(UpdateDrainPermit),
}

impl ReapReservation {
    pub(super) fn ordinary(gate: QuitGate) -> Self {
        Self::Ordinary(ShutdownPermit(Some(gate)))
    }

    #[cfg(any(test, target_os = "macos", target_os = "windows"))]
    pub(super) fn into_update(self) -> Option<UpdateDrainPermit> {
        match self {
            Self::Update(permit) => Some(permit),
            Self::Ordinary(_) => None,
        }
    }

    pub(super) fn into_ordinary(self) -> Option<ShutdownPermit> {
        match self {
            Self::Ordinary(permit) => Some(permit),
            #[cfg(any(test, target_os = "macos", target_os = "windows"))]
            Self::Update(_) => None,
        }
    }

    pub(super) fn retain_ordinary_failure(self) {
        #[cfg(any(test, target_os = "macos", target_os = "windows"))]
        let mut permit = match self {
            Self::Ordinary(permit) => permit,
            Self::Update(_) => return,
        };
        #[cfg(not(any(test, target_os = "macos", target_os = "windows")))]
        let Self::Ordinary(mut permit) = self;
        let _ = permit.0.take();
    }
}

#[cfg(any(test, target_os = "macos", target_os = "windows"))]
pub(crate) struct UpdateDrainPermit(Option<QuitGate>);

#[cfg(any(test, target_os = "macos", target_os = "windows"))]
impl UpdateDrainPermit {
    pub(super) fn reserve(gate: QuitGate) -> Option<Self> {
        if gate.reserve() {
            Some(Self(Some(gate)))
        } else {
            None
        }
    }
}

#[cfg(any(test, target_os = "macos", target_os = "windows"))]
impl Drop for UpdateDrainPermit {
    fn drop(&mut self) {
        if let Some(gate) = self.0.take() {
            gate.failed();
        }
    }
}

pub(crate) struct ShutdownPermit(pub(super) Option<QuitGate>);
impl ShutdownPermit {
    pub(crate) fn allow_exit(mut self) {
        if let Some(gate) = self.0.take() {
            gate.allow_exit();
        }
    }
}
impl Drop for ShutdownPermit {
    fn drop(&mut self) {
        if let Some(gate) = self.0.take() {
            gate.failed();
        }
    }
}

const QUIT_WINDOW: &str = "quit-failure";

#[derive(Default)]
pub struct QuitFailureStore {
    next_id: std::sync::atomic::AtomicU64,
    pending: Mutex<Option<PendingFailure>>,
}

struct PendingFailure {
    id: u64,
    presentation: FailurePresentation,
}

#[derive(Debug, Clone, serde::Serialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export_to = "ipc.ts"))]
pub struct QuitFailureView {
    id: u64,
    message: String,
}

impl QuitFailureStore {
    fn publish(&self, mut presentation: FailurePresentation) {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst).wrapping_add(1);
        if let Ok(mut pending) = self.pending.lock() {
            // A second failure cannot replace an unacknowledged first one.
            if pending.is_none() {
                *pending = Some(PendingFailure { id, presentation });
            } else {
                presentation.disarm_duplicate();
            }
        }
    }

    fn read(&self) -> Option<QuitFailureView> {
        self.pending
            .lock()
            .ok()?
            .as_ref()
            .map(|failure| QuitFailureView {
                id: failure.id,
                message: MSG_QUIT_FAILED.to_owned(),
            })
    }

    fn acknowledge(&self, id: u64) -> bool {
        let Ok(mut pending) = self.pending.lock() else {
            return false;
        };
        if pending.as_ref().is_none_or(|failure| failure.id != id) {
            return false;
        }
        let Some(mut failure) = pending.take() else {
            return false;
        };
        failure.presentation.ack();
        true
    }
}

pub(crate) fn show_failure<R: Runtime>(app: &AppHandle<R>, presentation: FailurePresentation) {
    app.state::<QuitFailureStore>().publish(presentation);
    #[cfg(not(target_os = "android"))]
    ensure_failure_window(app);
    #[cfg(target_os = "android")]
    {
        use tauri_plugin_dialog::DialogExt as _;
        let handle = app.clone();
        app.dialog()
            .message(MSG_QUIT_FAILED)
            .title("CopyPaste")
            .show(move |_| {
                if let Some(failure) = handle.state::<QuitFailureStore>().read() {
                    handle.state::<QuitFailureStore>().acknowledge(failure.id);
                }
            });
    }
}

#[cfg(not(target_os = "android"))]
pub(crate) fn ensure_failure_window<R: Runtime>(app: &AppHandle<R>) {
    if app.state::<QuitFailureStore>().read().is_none() {
        return;
    }
    {
        let window = match app.get_webview_window(QUIT_WINDOW) {
            Some(window) => Some(window),
            None => WebviewWindowBuilder::new(
                app,
                QUIT_WINDOW,
                WebviewUrl::App("index.html?surface=quit-failure".into()),
            )
            .title("CopyPaste")
            .inner_size(440.0, 260.0)
            .resizable(false)
            .visible(false)
            .content_protected(true)
            .on_navigation(|url| {
                url.path() == "/index.html" && url.query() == Some("surface=quit-failure")
            })
            .build()
            .ok(),
        };
        if let Some(window) = window {
            // This window contains only fixed, non-sensitive copy. A capture
            // protection failure must not strand the quit acknowledgment.
            if let Err(error) = window.set_content_protected(true) {
                tracing::warn!(%error, "quit recovery capture protection unavailable");
            }
            let _ = window.show();
            let _ = window.set_focus();
        }
    }
}

pub(crate) fn read_for_window(
    window: &WebviewWindow,
    store: &QuitFailureStore,
) -> Option<QuitFailureView> {
    (window.label() == QUIT_WINDOW)
        .then(|| store.read())
        .flatten()
}

pub(crate) fn ack_for_window(window: &WebviewWindow, store: &QuitFailureStore, id: u64) -> bool {
    if window.label() != QUIT_WINDOW || !store.acknowledge(id) {
        return false;
    }
    let _ = window.destroy();
    true
}

pub(crate) fn finish_failure(supervisor: &Supervisor, show: impl FnOnce(FailurePresentation)) {
    show(supervisor.failure_presentation());
}

pub(crate) struct FailurePresentation {
    terminal_failure: Arc<AtomicBool>,
    gate: QuitGate,
    acknowledged: bool,
}

impl FailurePresentation {
    pub(super) fn new(terminal_failure: Arc<AtomicBool>, gate: QuitGate) -> Self {
        Self {
            terminal_failure,
            gate,
            acknowledged: false,
        }
    }

    pub(crate) fn ack(&mut self) {
        self.terminal_failure.store(false, Ordering::SeqCst);
        self.gate.failed();
        self.acknowledged = true;
    }

    fn disarm_duplicate(&mut self) {
        // A previously published presentation still owns this same gate.
        // Dropping a duplicate must not reset it before the first is read.
        self.acknowledged = true;
    }
}

impl Drop for FailurePresentation {
    fn drop(&mut self) {
        if !self.acknowledged {
            self.gate.failed();
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExitRequest {
    Allow,
    Drain,
    AlreadyDraining,
    Failure,
}

const IDLE: u8 = 0;
const DRAINING: u8 = 1;
const ALLOW_EXIT: u8 = 2;

/// Serializes ordinary app quit without making a second exit request recursive.
#[derive(Clone)]
pub(super) struct QuitGate(std::sync::Arc<AtomicU8>);

impl Default for QuitGate {
    fn default() -> Self {
        Self(std::sync::Arc::new(AtomicU8::new(IDLE)))
    }
}

impl QuitGate {
    pub(super) fn request(&self, owns_or_is_draining: bool) -> ExitRequest {
        match self.0.load(Ordering::SeqCst) {
            ALLOW_EXIT => ExitRequest::Allow,
            DRAINING => ExitRequest::AlreadyDraining,
            IDLE if owns_or_is_draining => {
                if self
                    .0
                    .compare_exchange(IDLE, DRAINING, Ordering::SeqCst, Ordering::SeqCst)
                    .is_ok()
                {
                    ExitRequest::Drain
                } else {
                    self.request(owns_or_is_draining)
                }
            }
            IDLE => {
                self.0.store(ALLOW_EXIT, Ordering::SeqCst);
                ExitRequest::Allow
            }
            _ => unreachable!("quit gate has a known state"),
        }
    }

    pub(super) fn failed(&self) {
        self.0.store(IDLE, Ordering::SeqCst);
    }

    pub(super) fn allow_exit(&self) {
        self.0.store(ALLOW_EXIT, Ordering::SeqCst);
    }

    pub(super) fn is_reserved(&self) -> bool {
        self.0.load(Ordering::SeqCst) != IDLE
    }

    fn reserve(&self) -> bool {
        self.0
            .compare_exchange(IDLE, DRAINING, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }

    pub(super) fn reserve_failure(&self) {
        debug_assert!(self.reserve());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_requests_start_one_drain_and_final_exit_does_not_recurse() {
        let gate = QuitGate::default();

        assert_eq!(gate.request(true), ExitRequest::Drain);
        assert_eq!(gate.request(true), ExitRequest::AlreadyDraining);
        gate.allow_exit();
        assert_eq!(gate.request(true), ExitRequest::Allow);
        assert_eq!(gate.request(true), ExitRequest::Allow);
    }

    #[test]
    fn failed_drain_can_be_requested_again() {
        let gate = QuitGate::default();
        assert_eq!(gate.request(true), ExitRequest::Drain);
        gate.failed();
        assert_eq!(gate.request(true), ExitRequest::Drain);
    }

    #[test]
    fn duplicate_failure_does_not_release_first_ack_gate() {
        let gate = QuitGate::default();
        let terminal = Arc::new(AtomicBool::new(true));
        assert_eq!(gate.request(true), ExitRequest::Drain);
        let store = QuitFailureStore::default();
        store.publish(FailurePresentation::new(terminal.clone(), gate.clone()));
        let first = store.read().expect("first failure").id;
        store.publish(FailurePresentation::new(terminal.clone(), gate.clone()));
        assert_eq!(store.read().expect("still first").id, first);
        assert_eq!(gate.request(true), ExitRequest::AlreadyDraining);
        assert!(!store.acknowledge(first.wrapping_add(1)));
        assert_eq!(gate.request(true), ExitRequest::AlreadyDraining);
        assert!(store.acknowledge(first));
        assert!(!terminal.load(Ordering::SeqCst));
        assert_eq!(gate.request(false), ExitRequest::Allow);
    }
}
