use backon::{ExponentialBuilder, Retryable};
use serde::Serialize;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::Mutex as AsyncMutex;

use crate::backend::{Backend, BackendError, Result};

mod child;
pub mod diagnostics;
pub mod locate;
pub mod push;
pub(crate) mod quit;
mod spawn;
pub(crate) mod startup_diagnostics;

use child::{ChildProcess, ChildState};
use quit::{
    ExitRequest, FailurePresentation, QuitGate, ReapCompletion, ShutdownPermit, MSG_QUIT_FAILED,
};
use spawn::spawn_process;

const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

const MSG_NOT_INSTALLED: &str = "This build of CopyPaste doesn't include the background service.";
const MSG_START_FAILED: &str = "The background service could not be started.";
const MSG_NEVER_READY: &str = "The background service started but didn't finish coming up.";

const READY_TIMEOUT: Duration = Duration::from_secs(10);

#[cfg(test)]
struct ReapDeliveryBarrier {
    delivered: Arc<AtomicBool>,
    release: std::sync::mpsc::Receiver<()>,
    finished: Arc<AtomicBool>,
}

#[cfg(test)]
struct ReapDeliveryHold {
    delivered: Arc<AtomicBool>,
    release: Option<std::sync::mpsc::Sender<()>>,
    finished: Arc<AtomicBool>,
}

struct StartReservation {
    child: Arc<Mutex<Option<Box<dyn ChildProcess>>>>,
    pending: Arc<AtomicBool>,
}

#[cfg(test)]
struct ChildTransferBarrier {
    entered: std::sync::mpsc::Sender<()>,
    release: std::sync::mpsc::Receiver<()>,
}

#[cfg(test)]
struct ChildTransferHold {
    entered: std::sync::mpsc::Receiver<()>,
    release: Option<std::sync::mpsc::Sender<()>>,
}

#[cfg(test)]
impl ChildTransferHold {
    fn release(&mut self) {
        if let Some(release) = self.release.take() {
            let _ = release.send(());
        }
    }
}

#[cfg(test)]
impl Drop for ChildTransferHold {
    fn drop(&mut self) {
        self.release();
    }
}

impl Drop for StartReservation {
    fn drop(&mut self) {
        let _slot = self
            .child
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        self.pending.store(false, Ordering::SeqCst);
    }
}

#[cfg(test)]
impl ReapDeliveryHold {
    fn delivered(&self) -> bool {
        self.delivered.load(Ordering::SeqCst)
    }

    fn finished(&self) -> bool {
        self.finished.load(Ordering::SeqCst)
    }

    fn release(&mut self) {
        if let Some(release) = self.release.take() {
            let _ = release.send(());
        }
    }
}

#[cfg(test)]
impl Drop for ReapDeliveryHold {
    fn drop(&mut self) {
        self.release();
    }
}

/// What the background service is doing, as the UI needs to see it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export_to = "ipc.ts"))]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ServiceState {
    /// Answering on the socket.
    Running {
        version: String,
        /// `false` is the upgrade case: an older daemon still holding the
        /// socket after the app was replaced (parity finding 2).
        matches_app: bool,
        /// This app process started it, so it can also stop it.
        ours: bool,
    },
    /// Answering, but with something this build cannot read. Still a live
    /// process, so it is not a thing to start a second one alongside.
    Unhealthy,
    /// Nothing on the socket, and there is a binary to start.
    Stopped,
    /// Nothing on the socket, and nothing to start.
    NotInstalled,
}

impl ServiceState {
    /// True when there is a live daemon, whatever shape it is in.
    pub fn is_live(&self) -> bool {
        matches!(self, Self::Running { .. } | Self::Unhealthy)
    }
}

/// Owns the daemon child process for the life of this app process.
pub struct Supervisor {
    lifecycle: AsyncMutex<()>,
    child: Arc<Mutex<Option<Box<dyn ChildProcess>>>>,
    child_pending: Arc<AtomicBool>,
    start_pending: Arc<AtomicBool>,
    terminal_failure: Arc<AtomicBool>,
    quit_gate: QuitGate,
    #[cfg(test)]
    reap_delivery_barrier: Mutex<Option<ReapDeliveryBarrier>>,
    #[cfg(test)]
    child_transfer_barrier: Mutex<Option<ChildTransferBarrier>>,
}

impl std::fmt::Debug for Supervisor {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Supervisor").finish_non_exhaustive()
    }
}

impl Default for Supervisor {
    fn default() -> Self {
        Self {
            lifecycle: AsyncMutex::new(()),
            child: Arc::new(Mutex::new(None)),
            child_pending: Arc::new(AtomicBool::new(false)),
            start_pending: Arc::new(AtomicBool::new(false)),
            terminal_failure: Arc::new(AtomicBool::new(false)),
            quit_gate: QuitGate::default(),
            #[cfg(test)]
            reap_delivery_barrier: Mutex::new(None),
            #[cfg(test)]
            child_transfer_barrier: Mutex::new(None),
        }
    }
}

impl Supervisor {
    /// What the service is doing right now.
    ///
    /// `Unreachable` is the only error that means "not running" — the daemon
    /// answers `status` before it is ready, so anything else came from a live
    /// process (manifest 04: `status` is exempt from the readiness gate).
    pub async fn state<B: Backend>(&self, backend: &B) -> ServiceState {
        self.service_state(backend).await
    }

    async fn service_state<B: ServiceBackend>(&self, backend: &B) -> ServiceState {
        self.service_state_with(backend, &locate::daemon_binary)
            .await
    }

    async fn service_state_with<B, L>(&self, backend: &B, locate: &L) -> ServiceState
    where
        B: ServiceBackend,
        L: Fn() -> Option<std::path::PathBuf>,
    {
        if self.child_pending.load(Ordering::SeqCst) {
            return ServiceState::Unhealthy;
        }
        match backend.service_status().await {
            Ok(status) => ServiceState::Running {
                matches_app: status.version == APP_VERSION,
                version: status.version,
                ours: self.holds_child(),
            },
            Err(BackendError::Unreachable) => {
                if self.holds_child() {
                    ServiceState::Unhealthy
                } else if locate().is_some() {
                    ServiceState::Stopped
                } else {
                    ServiceState::NotInstalled
                }
            }
            Err(_) => ServiceState::Unhealthy,
        }
    }

    /// Start the service if it is not already running.
    ///
    /// Idempotent: a second call while one is live is a no-op that reports the
    /// live state. That matters because the button that calls this is on a
    /// screen a user can double-click.
    pub async fn start<B: Backend>(&self, backend: &B) -> Result<ServiceState> {
        let _lifecycle = self.lifecycle.lock().await;
        self.start_locked(backend, &locate::daemon_binary, &spawn_process)
            .await
    }

    async fn start_locked<B, L, S>(
        &self,
        backend: &B,
        locate: &L,
        spawn: &S,
    ) -> Result<ServiceState>
    where
        B: ServiceBackend,
        L: Fn() -> Option<std::path::PathBuf>,
        S: Fn(&Path) -> Result<Box<dyn ChildProcess>>,
    {
        let Some(_start) = self.reserve_start() else {
            return Ok(ServiceState::Unhealthy);
        };
        let state = self.service_state_with(backend, locate).await;
        startup_diagnostics::initial_state(&state);
        match state {
            ServiceState::NotInstalled => {
                startup_diagnostics::branch(startup_diagnostics::StartBranch::RejectMissing);
                return Err(BackendError::Unsupported(MSG_NOT_INSTALLED));
            }
            ServiceState::Stopped => {
                startup_diagnostics::branch(startup_diagnostics::StartBranch::Spawn);
            }
            live @ ServiceState::Running { .. } => {
                startup_diagnostics::branch(startup_diagnostics::StartBranch::UseRunning);
                return Ok(live);
            }
            live @ ServiceState::Unhealthy => {
                startup_diagnostics::branch(startup_diagnostics::StartBranch::ReturnUnhealthy);
                return Ok(live);
            }
        }

        let binary = locate().ok_or(BackendError::Unsupported(MSG_NOT_INSTALLED))?;
        if !self.spawn(&binary, spawn)? {
            return Ok(ServiceState::Unhealthy);
        }
        if let Err(error) = self.await_ready(backend).await {
            return Err(error);
        }
        let state = self.service_state_with(backend, locate).await;
        if state.is_live() {
            self.terminal_failure.store(false, Ordering::SeqCst);
        }
        Ok(state)
    }

    /// Stop the live daemon, then start the bundled version.
    ///
    /// The IPC request is safe for an adopted daemon: socket access already
    /// grants every destructive history operation, and this avoids signalling
    /// a pid the app does not own (ADR-0004).
    pub async fn restart<B: Backend>(&self, backend: &B) -> Result<ServiceState> {
        let _lifecycle = self.lifecycle.lock().await;
        self.restart_locked(backend, &locate::daemon_binary, &spawn_process)
            .await
    }

    async fn restart_locked<B, L, S>(
        &self,
        backend: &B,
        locate: &L,
        spawn: &S,
    ) -> Result<ServiceState>
    where
        B: ServiceBackend,
        L: Fn() -> Option<std::path::PathBuf>,
        S: Fn(&Path) -> Result<Box<dyn ChildProcess>>,
    {
        let state = self.service_state_with(backend, locate).await;
        if state.is_live() {
            backend.stop_service().await?;
            if self.holds_child() {
                self.reap_owned_child(false).await?;
            } else {
                self.await_stopped(backend).await?;
            }
        }
        self.start_locked(backend, locate, spawn).await
    }

    /// Gracefully stop the daemon this process owns without discarding its
    /// authoritative child handle before the process has actually exited.
    pub(crate) async fn shutdown<B: Backend>(&self, backend: &B) -> Result<ShutdownPermit> {
        let _lifecycle = self.lifecycle.lock().await;
        self.shutdown_locked(backend).await
    }

    async fn shutdown_locked<B: ServiceBackend>(&self, backend: &B) -> Result<ShutdownPermit> {
        if !self.holds_child() {
            if self.terminal_failure.load(Ordering::SeqCst) {
                return Err(BackendError::Internal(MSG_QUIT_FAILED.into()));
            }
            return Ok(ShutdownPermit(Some(self.quit_gate.clone())));
        }
        backend.stop_service().await?;
        self.reap_owned_child(true).await.map(ShutdownPermit)
    }

    fn spawn<S>(&self, binary: &Path, spawn: &S) -> Result<bool>
    where
        S: Fn(&Path) -> Result<Box<dyn ChildProcess>>,
    {
        let mut slot = self.child_slot();
        if self.quit_gate.is_reserved() {
            return Ok(false);
        }
        let child = spawn(binary)?;
        debug_assert!(slot.is_none(), "a live child cannot be replaced");
        *slot = Some(child);
        Ok(true)
    }

    /// Wait for the daemon to answer, or for it to die trying.
    ///
    /// `backon` rather than a sleep loop: the workspace has one retry owner
    /// (AGENTS.md rule 1).
    async fn await_ready<B: ServiceBackend>(&self, backend: &B) -> Result<()> {
        let policy = ExponentialBuilder::new()
            .with_min_delay(Duration::from_millis(50))
            .with_max_delay(Duration::from_millis(400))
            .without_max_times()
            .with_total_delay(Some(READY_TIMEOUT));

        let probe = || async {
            // Checked before the probe, so a daemon that exited on startup —
            // a locked database, a refused port — is reported now instead of
            // after the full timeout.
            if self.child_exited() {
                return Err(BackendError::Internal(MSG_START_FAILED.into()));
            }
            backend.service_status().await.map(|_| ())
        };

        probe
            .retry(policy)
            // A daemon that has already exited will not start answering.
            .when(|err| matches!(err, BackendError::Unreachable))
            .await
            .map_err(|err| match err {
                BackendError::Unreachable => BackendError::Internal(MSG_NEVER_READY.into()),
                other => other,
            })
    }

    async fn await_stopped<B: ServiceBackend>(&self, backend: &B) -> Result<()> {
        let policy = ExponentialBuilder::new()
            .with_min_delay(Duration::from_millis(25))
            .with_max_delay(Duration::from_millis(200))
            .without_max_times()
            .with_total_delay(Some(READY_TIMEOUT));
        (|| async {
            match backend.service_status().await {
                Err(BackendError::Unreachable) => Ok(()),
                _ => Err(BackendError::Internal(MSG_NEVER_READY.into())),
            }
        })
        .retry(policy)
        .await
    }

    /// Whether we hold a child that is still alive, reaping it if it is not.
    fn holds_child(&self) -> bool {
        let mut slot = self.child_slot();
        match slot.as_mut().map(|child| child.state()) {
            Some(Ok(ChildState::Running)) => true,
            Some(Ok(ChildState::Exited(code))) => {
                let failed = !code.is_success();
                startup_diagnostics::child_exited(code);
                if failed {
                    self.terminal_failure.store(true, Ordering::SeqCst);
                }
                *slot = None;
                self.child_pending.store(false, Ordering::SeqCst);
                false
            }
            Some(Err(_)) => true,
            None => false,
        }
    }

    fn child_exited(&self) -> bool {
        let mut slot = self.child_slot();
        if let Some(Ok(ChildState::Exited(code))) = slot.as_mut().map(|child| child.state()) {
            let failed = !code.is_success();
            startup_diagnostics::child_exited(code);
            if failed {
                self.terminal_failure.store(true, Ordering::SeqCst);
            }
            *slot = None;
            self.child_pending.store(false, Ordering::SeqCst);
            true
        } else {
            false
        }
    }

    async fn reap_owned_child(&self, ordinary_quit: bool) -> Result<Option<QuitGate>> {
        let Some(child) = self.take_child() else {
            return Ok(None);
        };
        let slot = Arc::clone(&self.child);
        let pending = Arc::clone(&self.child_pending);
        let terminal_failure = Arc::clone(&self.terminal_failure);
        let quit_gate = self.quit_gate.clone();
        #[cfg(test)]
        let delivery_barrier = self
            .reap_delivery_barrier
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take();
        let (sent, received) = tokio::sync::oneshot::channel();
        tokio::task::spawn_blocking(move || {
            let mut child = child;
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| child.reap()));
            let result = match result {
                Ok(Ok(code)) if code.is_success() => {
                    terminal_failure.store(false, Ordering::SeqCst);
                    Ok(code)
                }
                Ok(Ok(code)) => {
                    terminal_failure.store(true, Ordering::SeqCst);
                    Ok(code)
                }
                Ok(Err(error)) => {
                    let mut restored = slot.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                    if restored.is_none() {
                        *restored = Some(child);
                    } else {
                        std::mem::forget(child);
                    }
                    Err(error)
                }
                Err(_) => {
                    let mut restored = slot.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                    if restored.is_none() {
                        *restored = Some(child);
                    } else {
                        std::mem::forget(child);
                    }
                    terminal_failure.store(true, Ordering::SeqCst);
                    Err(std::io::Error::other("reap panicked"))
                }
            };
            pending.store(false, Ordering::SeqCst);
            let completion = ReapCompletion {
                result: Some(result),
                ordinary_quit: ordinary_quit.then_some(quit_gate),
            };
            if sent.send(completion).is_ok() {
                #[cfg(test)]
                if let Some(barrier) = delivery_barrier {
                    barrier.delivered.store(true, Ordering::SeqCst);
                    let _ = barrier.release.recv();
                    barrier.finished.store(true, Ordering::SeqCst);
                }
            }
        });

        match received.await {
            Err(_) => Err(BackendError::Internal(MSG_QUIT_FAILED.into())),
            Ok(completion) => match completion.finish() {
                (Ok(code), gate) if code.is_success() => Ok(gate),
                (Ok(code), gate) => {
                    drop(gate);
                    startup_diagnostics::child_exited(code);
                    Err(BackendError::Internal(MSG_QUIT_FAILED.into()))
                }
                (Err(error), gate) => {
                    drop(gate);
                    tracing::warn!(%error, "the background service could not be reaped");
                    Err(BackendError::Internal(MSG_QUIT_FAILED.into()))
                }
            },
        }
    }

    fn take_child(&self) -> Option<Box<dyn ChildProcess>> {
        let mut slot = self.child_slot();
        let child = slot.take();
        self.child_pending.store(child.is_some(), Ordering::SeqCst);
        #[cfg(test)]
        if let Some(barrier) = self
            .child_transfer_barrier
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take()
        {
            let _ = barrier.entered.send(());
            let _ = barrier.release.recv();
        }
        child
    }

    pub(crate) fn request_quit(&self) -> ExitRequest {
        let mut slot = self.child_slot();
        let pending =
            self.child_pending.load(Ordering::SeqCst) || self.start_pending.load(Ordering::SeqCst);
        let owns = match slot.as_mut().map(|child| child.state()) {
            Some(Ok(ChildState::Running)) => true,
            Some(Ok(ChildState::Exited(code))) => {
                let failed = !code.is_success();
                startup_diagnostics::child_exited(code);
                if failed {
                    self.terminal_failure.store(true, Ordering::SeqCst);
                }
                *slot = None;
                self.child_pending.store(false, Ordering::SeqCst);
                false
            }
            Some(Err(_)) => true,
            None => false,
        };
        if self.quit_gate.is_reserved() {
            return self.quit_gate.request(pending || owns);
        }
        if self.terminal_failure.load(Ordering::SeqCst) {
            self.quit_gate.reserve_failure();
            return ExitRequest::Failure;
        }
        self.quit_gate.request(pending || owns)
    }

    fn reserve_start(&self) -> Option<StartReservation> {
        let _slot = self.child_slot();
        if self.quit_gate.is_reserved() || self.child_pending.load(Ordering::SeqCst) {
            return None;
        }
        self.start_pending.store(true, Ordering::SeqCst);
        Some(StartReservation {
            child: Arc::clone(&self.child),
            pending: Arc::clone(&self.start_pending),
        })
    }

    pub(crate) fn failure_presentation(&self) -> FailurePresentation {
        if !self.quit_gate.is_reserved() {
            self.quit_gate.reserve_failure();
        }
        FailurePresentation::new(Arc::clone(&self.terminal_failure), self.quit_gate.clone())
    }

    fn child_slot(&self) -> std::sync::MutexGuard<'_, Option<Box<dyn ChildProcess>>> {
        self.child
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    #[cfg(test)]
    fn hold_next_reap_delivery(&self) -> ReapDeliveryHold {
        let delivered = Arc::new(AtomicBool::new(false));
        let finished = Arc::new(AtomicBool::new(false));
        let (release, receiver) = std::sync::mpsc::channel();
        *self
            .reap_delivery_barrier
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(ReapDeliveryBarrier {
            delivered: Arc::clone(&delivered),
            release: receiver,
            finished: Arc::clone(&finished),
        });
        ReapDeliveryHold {
            delivered,
            release: Some(release),
            finished,
        }
    }

    #[cfg(test)]
    fn hold_next_child_transfer(&self) -> ChildTransferHold {
        let (entered, entered_at_transfer) = std::sync::mpsc::channel();
        let (release, wait_for_release) = std::sync::mpsc::channel();
        *self
            .child_transfer_barrier
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(ChildTransferBarrier {
            entered,
            release: wait_for_release,
        });
        ChildTransferHold {
            entered: entered_at_transfer,
            release: Some(release),
        }
    }

    #[cfg(test)]
    async fn start_injected<B, L, S>(
        &self,
        backend: &B,
        locate: &L,
        spawn: &S,
    ) -> Result<ServiceState>
    where
        B: ServiceBackend,
        L: Fn() -> Option<std::path::PathBuf>,
        S: Fn(&Path) -> Result<Box<dyn ChildProcess>>,
    {
        let _lifecycle = self.lifecycle.lock().await;
        self.start_locked(backend, locate, spawn).await
    }

    #[cfg(test)]
    async fn restart_injected<B, L, S>(
        &self,
        backend: &B,
        locate: &L,
        spawn: &S,
    ) -> Result<ServiceState>
    where
        B: ServiceBackend,
        L: Fn() -> Option<std::path::PathBuf>,
        S: Fn(&Path) -> Result<Box<dyn ChildProcess>>,
    {
        let _lifecycle = self.lifecycle.lock().await;
        self.restart_locked(backend, locate, spawn).await
    }

    #[cfg(test)]
    async fn shutdown_injected<B: ServiceBackend>(&self, backend: &B) -> Result<()> {
        let _lifecycle = self.lifecycle.lock().await;
        self.shutdown_locked(backend).await.map(|_| ())
    }
}

#[allow(async_fn_in_trait)]
trait ServiceBackend: Send + Sync {
    async fn service_status(&self) -> Result<copypaste_ipc::StatusData>;
    async fn stop_service(&self) -> Result<()>;
}

impl<B: Backend> ServiceBackend for B {
    async fn service_status(&self) -> Result<copypaste_ipc::StatusData> {
        self.status().await
    }

    async fn stop_service(&self) -> Result<()> {
        self.shutdown().await
    }
}

impl Drop for Supervisor {
    fn drop(&mut self) {
        let mut slot = self
            .child
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(child) = slot.take() {
            std::mem::forget(child);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::child::ChildExitCode;
    use super::*;
    use crate::backend::testing::FakeBackend;
    use copypaste_ipc::{DiagnosticCounters, StatusData, PROTOCOL_VERSION};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::task::Poll;
    use tokio::sync::Semaphore;

    struct OneShotGate {
        armed: AtomicBool,
        entered: Semaphore,
        release: Semaphore,
    }

    impl OneShotGate {
        fn new() -> Self {
            Self {
                armed: AtomicBool::new(false),
                entered: Semaphore::new(0),
                release: Semaphore::new(0),
            }
        }

        fn arm(&self) {
            self.armed.store(true, Ordering::SeqCst);
        }

        async fn pause_once(&self) {
            if self.armed.swap(false, Ordering::SeqCst) {
                self.entered.add_permits(1);
                self.release.acquire().await.unwrap().forget();
            }
        }

        async fn wait_until_entered(&self) {
            self.entered.acquire().await.unwrap().forget();
        }

        fn resume(&self) {
            self.release.add_permits(1);
        }
    }

    struct ControlledBackend {
        running: AtomicBool,
        status_gate: OneShotGate,
        shutdown_gate: OneShotGate,
        shutdown_calls: AtomicUsize,
    }

    impl ControlledBackend {
        fn new(running: bool) -> Self {
            Self {
                running: AtomicBool::new(running),
                status_gate: OneShotGate::new(),
                shutdown_gate: OneShotGate::new(),
                shutdown_calls: AtomicUsize::new(0),
            }
        }

        fn set_running(&self, running: bool) {
            self.running.store(running, Ordering::SeqCst);
        }
    }

    impl ServiceBackend for ControlledBackend {
        async fn service_status(&self) -> Result<StatusData> {
            self.status_gate.pause_once().await;
            if !self.running.load(Ordering::SeqCst) {
                return Err(BackendError::Unreachable);
            }
            Ok(StatusData {
                device_name: "Test device".into(),
                version: APP_VERSION.into(),
                protocol_version: PROTOCOL_VERSION,
                listen_addr: None,
                device_details: None,
                item_count: 0,
                capture_running: true,
                clipboard_backend: "fake".into(),
                private_mode: false,
                private_mode_epoch: 0,
                counters: DiagnosticCounters::default(),
                settings_health: None,
            })
        }

        async fn stop_service(&self) -> Result<()> {
            self.shutdown_calls.fetch_add(1, Ordering::SeqCst);
            self.shutdown_gate.pause_once().await;
            self.set_running(false);
            Ok(())
        }
    }

    struct AdoptedStaysLive {
        shutdown_calls: AtomicUsize,
    }

    impl ServiceBackend for AdoptedStaysLive {
        async fn service_status(&self) -> Result<StatusData> {
            Ok(StatusData {
                device_name: "Test device".into(),
                version: APP_VERSION.into(),
                protocol_version: PROTOCOL_VERSION,
                listen_addr: None,
                device_details: None,
                item_count: 0,
                capture_running: true,
                clipboard_backend: "fake".into(),
                private_mode: false,
                private_mode_epoch: 0,
                counters: DiagnosticCounters::default(),
                settings_health: None,
            })
        }

        async fn stop_service(&self) -> Result<()> {
            self.shutdown_calls.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    #[derive(Default)]
    struct ChildProbe {
        exited: AtomicBool,
        kills: AtomicUsize,
        reaps: AtomicUsize,
    }

    struct FakeChild {
        probe: Arc<ChildProbe>,
    }

    impl ChildProcess for FakeChild {
        fn state(&mut self) -> std::io::Result<ChildState> {
            let exited = self.probe.exited.load(Ordering::SeqCst);
            if exited {
                assert_eq!(self.probe.reaps.fetch_add(1, Ordering::SeqCst), 0);
                return Ok(ChildState::Exited(ChildExitCode::from_test_code(23)));
            }
            Ok(ChildState::Running)
        }

        fn reap(&mut self) -> std::io::Result<ChildExitCode> {
            assert_eq!(self.probe.reaps.fetch_add(1, Ordering::SeqCst), 0);
            Ok(ChildExitCode::from_test_code(0))
        }
    }

    struct DelayedChild {
        probe: Arc<ChildProbe>,
        exit_code: i32,
        reap_error: bool,
        entered: Option<tokio::sync::oneshot::Sender<()>>,
        release: Option<std::sync::mpsc::Receiver<()>>,
        finished: Option<tokio::sync::oneshot::Sender<()>>,
    }

    struct UnknownExitChild;

    struct ControlledExitChild {
        exited: Arc<AtomicBool>,
        code: Option<ChildExitCode>,
    }

    struct UnknownExitedChild;
    struct PanickingChild;

    impl ChildProcess for UnknownExitChild {
        fn state(&mut self) -> std::io::Result<ChildState> {
            Ok(ChildState::Running)
        }

        fn reap(&mut self) -> std::io::Result<ChildExitCode> {
            Ok(ChildExitCode::unavailable())
        }
    }

    impl ChildProcess for ControlledExitChild {
        fn state(&mut self) -> std::io::Result<ChildState> {
            if self.exited.load(Ordering::SeqCst) {
                return Ok(ChildState::Exited(
                    self.code
                        .take()
                        .expect("the supervisor observes this exit once"),
                ));
            }
            Ok(ChildState::Running)
        }

        fn reap(&mut self) -> std::io::Result<ChildExitCode> {
            unreachable!("an exited child is never reaped again")
        }
    }

    impl ChildProcess for UnknownExitedChild {
        fn state(&mut self) -> std::io::Result<ChildState> {
            Ok(ChildState::Exited(ChildExitCode::unavailable()))
        }

        fn reap(&mut self) -> std::io::Result<ChildExitCode> {
            Ok(ChildExitCode::unavailable())
        }
    }

    impl ChildProcess for PanickingChild {
        fn state(&mut self) -> std::io::Result<ChildState> {
            Ok(ChildState::Running)
        }
        fn reap(&mut self) -> std::io::Result<ChildExitCode> {
            panic!("controlled reap panic")
        }
    }

    impl ChildProcess for DelayedChild {
        fn state(&mut self) -> std::io::Result<ChildState> {
            Ok(ChildState::Running)
        }

        fn reap(&mut self) -> std::io::Result<ChildExitCode> {
            if let Some(entered) = self.entered.take() {
                let _ = entered.send(());
            }
            if let Some(release) = self.release.take() {
                let _ = release.recv();
            }
            self.probe.reaps.fetch_add(1, Ordering::SeqCst);
            let result = if self.reap_error {
                Err(std::io::Error::other("reap failed"))
            } else {
                Ok(ChildExitCode::from_test_code(self.exit_code))
            };
            if let Some(finished) = self.finished.take() {
                let _ = finished.send(());
            }
            result
        }
    }

    fn injected_binary() -> Option<PathBuf> {
        Some(PathBuf::from("injected-daemon"))
    }

    #[tokio::test]
    async fn an_unreachable_daemon_with_no_binary_is_not_installed() {
        let sup = Supervisor::default();
        let backend = FakeBackend::unreachable();
        // `locate` finds nothing beside the test binary, which is the point:
        // a test runner is not a bundle.
        assert_eq!(sup.state(&backend).await, ServiceState::NotInstalled);
    }

    #[tokio::test]
    async fn starting_without_a_binary_refuses_with_a_sentence_and_no_path() {
        let sup = Supervisor::default();
        let err = sup.start(&FakeBackend::unreachable()).await.unwrap_err();
        assert!(matches!(err, BackendError::Unsupported(_)), "{err:?}");
        let shown = err.to_string();
        assert!(!shown.contains('/'), "{shown}");
        assert!(shown.ends_with('.'), "{shown}");
    }

    /// The version comparison is the upgrade detector (parity finding 2): an
    /// orphan from the previous install answers, and answers with its own
    /// version.
    #[tokio::test]
    async fn a_daemon_on_another_version_is_running_but_does_not_match() {
        let sup = Supervisor::default();
        let state = sup.state(&FakeBackend::running("0.9.9")).await;
        assert_eq!(
            state,
            ServiceState::Running {
                version: "0.9.9".into(),
                matches_app: false,
                ours: false,
            }
        );
        assert!(state.is_live());
    }

    #[tokio::test]
    async fn a_daemon_on_our_version_matches_whoever_started_it() {
        let sup = Supervisor::default();
        let state = sup.state(&FakeBackend::running(APP_VERSION)).await;
        assert!(
            matches!(
                state,
                ServiceState::Running {
                    matches_app: true,
                    ours: false,
                    ..
                }
            ),
            "{state:?}"
        );
    }

    /// Idempotence, and the reason for it: the start button is clickable twice.
    #[tokio::test]
    async fn starting_while_it_is_already_running_does_not_spawn_a_second_one() {
        let sup = Supervisor::default();
        let backend = FakeBackend::running(APP_VERSION);
        let state = sup.start(&backend).await.unwrap();
        assert!(state.is_live());
        assert!(sup.take_child().is_none(), "a second daemon was spawned");
    }

    #[tokio::test]
    async fn concurrent_starts_spawn_one_child() {
        let sup = Supervisor::default();
        let backend = ControlledBackend::new(false);
        backend.status_gate.arm();
        let spawn_count = AtomicUsize::new(0);
        let child = Arc::new(ChildProbe::default());
        let spawn = |_: &Path| -> Result<Box<dyn ChildProcess>> {
            spawn_count.fetch_add(1, Ordering::SeqCst);
            backend.set_running(true);
            Ok(Box::new(FakeChild {
                probe: Arc::clone(&child),
            }))
        };

        let first = sup.start_injected(&backend, &injected_binary, &spawn);
        tokio::pin!(first);
        tokio::select! {
            () = backend.status_gate.wait_until_entered() => {}
            result = &mut first => panic!("start passed the gate: {result:?}"),
        }

        let second = sup.start_injected(&backend, &injected_binary, &spawn);
        tokio::pin!(second);
        assert!(matches!(futures_util::poll!(&mut second), Poll::Pending));
        backend.status_gate.resume();

        let (first, second) = tokio::join!(first, second);
        assert!(first.unwrap().is_live());
        assert!(second.unwrap().is_live());
        assert_eq!(spawn_count.load(Ordering::SeqCst), 1);
        assert_eq!(child.reaps.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn quit_reserves_a_start_admitted_before_its_status_probe() {
        let sup = Supervisor::default();
        let backend = ControlledBackend::new(false);
        let spawns = AtomicUsize::new(0);
        backend.status_gate.arm();
        let spawn = |_: &Path| -> Result<Box<dyn ChildProcess>> {
            spawns.fetch_add(1, Ordering::SeqCst);
            Ok(Box::new(FakeChild {
                probe: Arc::new(ChildProbe::default()),
            }))
        };

        let start = sup.start_injected(&backend, &injected_binary, &spawn);
        tokio::pin!(start);
        assert!(matches!(futures_util::poll!(&mut start), Poll::Pending));
        backend.status_gate.wait_until_entered().await;

        assert_eq!(sup.request_quit(), ExitRequest::Drain);
        backend.status_gate.resume();
        assert_eq!(start.await.unwrap(), ServiceState::Unhealthy);
        assert_eq!(spawns.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn quit_waits_for_a_child_transfer_and_then_drains_never_allows() {
        let sup = Supervisor::default();
        *sup.child_slot() = Some(Box::new(FakeChild {
            probe: Arc::new(ChildProbe::default()),
        }));
        let mut transfer_hold = sup.hold_next_child_transfer();

        std::thread::scope(|scope| {
            let transfer_supervisor = &sup;
            let transfer = scope.spawn(move || transfer_supervisor.take_child());
            transfer_hold.entered.recv().expect("child was removed");
            let (sent, result) = std::sync::mpsc::channel();
            let quit_supervisor = &sup;
            scope.spawn(move || {
                sent.send(quit_supervisor.request_quit())
                    .expect("report quit")
            });

            assert!(
                matches!(result.try_recv(), Err(std::sync::mpsc::TryRecvError::Empty)),
                "quit observed a half-transferred child"
            );
            transfer_hold.release();
            assert_eq!(result.recv().expect("quit completed"), ExitRequest::Drain);
            drop(transfer.join().expect("transfer completed"));
        });
    }

    #[tokio::test]
    async fn a_child_that_exits_during_start_is_reaped_once() {
        let sup = Supervisor::default();
        let backend = ControlledBackend::new(false);
        let child = Arc::new(ChildProbe::default());
        child.exited.store(true, Ordering::SeqCst);
        let spawn = |_: &Path| -> Result<Box<dyn ChildProcess>> {
            Ok(Box::new(FakeChild {
                probe: Arc::clone(&child),
            }))
        };

        let error = sup
            .start_injected(&backend, &injected_binary, &spawn)
            .await
            .unwrap_err();

        assert_eq!(error.to_string(), MSG_START_FAILED);
        assert_eq!(child.kills.load(Ordering::SeqCst), 0);
        assert_eq!(child.reaps.load(Ordering::SeqCst), 1);
        assert!(sup.take_child().is_none());
    }

    #[tokio::test]
    async fn restart_and_shutdown_are_one_ordered_sequence() {
        let sup = Supervisor::default();
        let backend = ControlledBackend::new(true);
        let old_child = Arc::new(ChildProbe::default());
        *sup.child_slot() = Some(Box::new(FakeChild {
            probe: Arc::clone(&old_child),
        }));
        backend.shutdown_gate.arm();
        let new_children = Mutex::new(Vec::new());
        let spawn = |_: &Path| -> Result<Box<dyn ChildProcess>> {
            backend.set_running(true);
            let probe = Arc::new(ChildProbe::default());
            new_children.lock().unwrap().push(Arc::clone(&probe));
            Ok(Box::new(FakeChild { probe }))
        };

        let restart = sup.restart_injected(&backend, &injected_binary, &spawn);
        tokio::pin!(restart);
        tokio::select! {
            () = backend.shutdown_gate.wait_until_entered() => {}
            result = &mut restart => panic!("restart passed the gate: {result:?}"),
        }

        let mut shutdown = Box::pin(sup.shutdown_injected(&backend));
        assert!(matches!(futures_util::poll!(&mut shutdown), Poll::Pending));
        backend.shutdown_gate.resume();

        let (restart, shutdown) = tokio::join!(restart, shutdown);
        assert!(restart.unwrap().is_live());
        assert!(shutdown.is_ok());
        assert!(!backend.running.load(Ordering::SeqCst));
        assert_eq!(backend.shutdown_calls.load(Ordering::SeqCst), 2);
        assert_eq!(old_child.kills.load(Ordering::SeqCst), 0);
        assert_eq!(old_child.reaps.load(Ordering::SeqCst), 1);
        let children = new_children.lock().unwrap();
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].kills.load(Ordering::SeqCst), 0);
        assert_eq!(children[0].reaps.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn shutdown_leaves_an_adopted_daemon_running() {
        let sup = Supervisor::default();
        let backend = ControlledBackend::new(true);

        assert!(sup.shutdown_injected(&backend).await.is_ok());

        assert!(backend.running.load(Ordering::SeqCst));
        assert_eq!(backend.shutdown_calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn restarting_an_adopted_daemon_shuts_it_down_before_starting() {
        let sup = Supervisor::default();
        let err = sup
            .restart(&FakeBackend::running("0.9.9"))
            .await
            .unwrap_err();
        assert!(matches!(err, BackendError::Unsupported(MSG_NOT_INSTALLED)));
    }

    #[tokio::test]
    async fn a_daemon_answering_badly_is_unhealthy_rather_than_stopped() {
        let sup = Supervisor::default();
        assert_eq!(
            sup.state(&FakeBackend::failing()).await,
            ServiceState::Unhealthy
        );
    }

    /// Every sentence this module can show a user, checked in one place.
    #[test]
    fn no_message_names_a_path() {
        for message in [
            MSG_NOT_INSTALLED,
            MSG_START_FAILED,
            MSG_NEVER_READY,
            MSG_QUIT_FAILED,
        ] {
            assert!(!message.contains('/'), "{message}");
            assert!(message.ends_with('.'), "{message}");
            // "daemon" is a developer word; the user-facing name is
            // "background service" (bdac.34/36).
            assert!(!message.contains("daemon"), "{message}");
        }
    }

    #[test]
    fn a_shown_terminal_failure_allows_the_next_no_child_quit() {
        let sup = Supervisor::default();
        let shown = AtomicBool::new(false);
        sup.terminal_failure.store(true, Ordering::SeqCst);

        quit::finish_failure(&sup, |mut presentation| {
            shown.store(true, Ordering::SeqCst);
            presentation.ack();
        });

        assert!(shown.load(Ordering::SeqCst));
        assert_eq!(sup.request_quit(), ExitRequest::Allow);
    }

    #[tokio::test]
    async fn scheduling_failure_presentation_keeps_the_drain_reserved_until_acknowledged() {
        let sup = Supervisor::default();
        let scheduled = AtomicBool::new(false);
        sup.terminal_failure.store(true, Ordering::SeqCst);

        let held = Mutex::new(None);
        quit::finish_failure(&sup, |presentation| {
            scheduled.store(true, Ordering::SeqCst);
            *held.lock().unwrap() = Some(presentation);
        });

        assert!(scheduled.load(Ordering::SeqCst));
        assert_eq!(sup.request_quit(), ExitRequest::AlreadyDraining);
        let backend = ControlledBackend::new(false);
        let spawns = AtomicUsize::new(0);
        let spawn = |_: &Path| -> Result<Box<dyn ChildProcess>> {
            spawns.fetch_add(1, Ordering::SeqCst);
            Err(BackendError::Internal(
                "must not spawn while presenting".into(),
            ))
        };
        assert_eq!(
            sup.start_injected(&backend, &injected_binary, &spawn)
                .await
                .unwrap(),
            ServiceState::Unhealthy
        );
        assert_eq!(spawns.load(Ordering::SeqCst), 0);
        held.lock().unwrap().take().unwrap().ack();
        assert_eq!(sup.request_quit(), ExitRequest::Allow);
    }

    #[test]
    fn dropping_an_unacknowledged_presentation_keeps_the_failure_reportable() {
        let sup = Supervisor::default();
        sup.terminal_failure.store(true, Ordering::SeqCst);
        let held = Mutex::new(None);
        quit::finish_failure(&sup, |presentation| {
            *held.lock().unwrap() = Some(presentation)
        });

        drop(held.lock().unwrap().take());

        assert_eq!(sup.request_quit(), ExitRequest::Failure);
    }

    #[tokio::test]
    async fn an_acknowledged_owned_shutdown_reaps_without_killing() {
        let sup = Supervisor::default();
        let backend = ControlledBackend::new(true);
        let child = Arc::new(ChildProbe::default());
        *sup.child_slot() = Some(Box::new(FakeChild {
            probe: Arc::clone(&child),
        }));

        assert!(sup.shutdown_injected(&backend).await.is_ok());

        assert_eq!(child.kills.load(Ordering::SeqCst), 0);
        assert_eq!(child.reaps.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn dropping_the_supervisor_does_not_issue_a_child_kill() {
        let child = Arc::new(ChildProbe::default());
        let sup = Supervisor::default();
        *sup.child_slot() = Some(Box::new(FakeChild {
            probe: Arc::clone(&child),
        }));

        drop(sup);

        assert_eq!(child.kills.load(Ordering::SeqCst), 0);
    }

    #[tokio::test(start_paused = true)]
    async fn acknowledged_drain_waits_past_the_old_budget_without_killing() {
        let sup = Supervisor::default();
        let backend = ControlledBackend::new(true);
        let child = Arc::new(ChildProbe::default());
        let (entered, entered_at_reap) = tokio::sync::oneshot::channel();
        let (release, wait_for_release) = std::sync::mpsc::channel();
        *sup.child_slot() = Some(Box::new(DelayedChild {
            probe: Arc::clone(&child),
            exit_code: 0,
            reap_error: false,
            entered: Some(entered),
            release: Some(wait_for_release),
            finished: None,
        }));
        assert_eq!(sup.request_quit(), ExitRequest::Drain);

        let shutdown = sup.shutdown_injected(&backend);
        tokio::pin!(shutdown);
        assert!(matches!(futures_util::poll!(&mut shutdown), Poll::Pending));
        entered_at_reap.await.expect("reap worker entered");
        let (heartbeat, reactor_heartbeat) = tokio::sync::oneshot::channel();
        tokio::spawn(async move {
            let _ = heartbeat.send(());
        });
        reactor_heartbeat
            .await
            .expect("reactor remained responsive");
        tokio::time::advance(Duration::from_secs(16)).await;
        assert!(matches!(futures_util::poll!(&mut shutdown), Poll::Pending));
        assert_eq!(child.kills.load(Ordering::SeqCst), 0);
        assert_eq!(sup.request_quit(), ExitRequest::AlreadyDraining);

        release.send(()).expect("release reap worker");
        assert!(shutdown.await.is_ok());
        assert_eq!(child.reaps.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn reap_error_restores_owned_child_and_keeps_quit_refused() {
        let sup = Supervisor::default();
        let backend = ControlledBackend::new(true);
        let child = Arc::new(ChildProbe::default());
        *sup.child_slot() = Some(Box::new(DelayedChild {
            probe: Arc::clone(&child),
            exit_code: 0,
            reap_error: true,
            entered: None,
            release: None,
            finished: None,
        }));

        let error = sup.shutdown_injected(&backend).await.unwrap_err();
        assert_eq!(error.to_string(), MSG_QUIT_FAILED);
        assert!(sup.holds_child());
        assert_eq!(sup.request_quit(), ExitRequest::Drain);
        assert_eq!(child.kills.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn cancelling_an_owned_reap_restores_its_child_without_killing() {
        let sup = Supervisor::default();
        let backend = ControlledBackend::new(true);
        let child = Arc::new(ChildProbe::default());
        let (entered, entered_at_reap) = tokio::sync::oneshot::channel();
        let (finished, completed_reap) = tokio::sync::oneshot::channel();
        let (release, wait_for_release) = std::sync::mpsc::channel();
        *sup.child_slot() = Some(Box::new(DelayedChild {
            probe: Arc::clone(&child),
            exit_code: 0,
            reap_error: false,
            entered: Some(entered),
            release: Some(wait_for_release),
            finished: Some(finished),
        }));

        let mut shutdown = Box::pin(sup.shutdown_injected(&backend));
        assert!(matches!(futures_util::poll!(&mut shutdown), Poll::Pending));
        entered_at_reap.await.expect("reap worker entered");
        drop(shutdown);

        assert!(sup.child_pending.load(Ordering::SeqCst));
        assert_eq!(child.kills.load(Ordering::SeqCst), 0);
        release.send(()).expect("release reap worker");
        completed_reap.await.expect("reap worker completed");
        let spawned = Arc::new(ChildProbe::default());
        let spawn = |_: &Path| -> Result<Box<dyn ChildProcess>> {
            backend.set_running(true);
            Ok(Box::new(FakeChild {
                probe: Arc::clone(&spawned),
            }))
        };
        assert!(sup
            .start_injected(&backend, &injected_binary, &spawn)
            .await
            .unwrap()
            .is_live());
        assert_eq!(sup.request_quit(), ExitRequest::Drain);
    }

    #[tokio::test]
    async fn cancelling_after_successful_reap_delivery_releases_the_quit_reservation() {
        let sup = Supervisor::default();
        let backend = ControlledBackend::new(true);
        let (entered, at_reap) = tokio::sync::oneshot::channel();
        let (release, wait) = std::sync::mpsc::channel();
        *sup.child_slot() = Some(Box::new(DelayedChild {
            probe: Arc::new(ChildProbe::default()),
            exit_code: 0,
            reap_error: false,
            entered: Some(entered),
            release: Some(wait),
            finished: None,
        }));
        let mut delivery = sup.hold_next_reap_delivery();

        assert_eq!(sup.request_quit(), ExitRequest::Drain);
        let mut shutdown = Box::pin(sup.shutdown_injected(&backend));
        assert!(matches!(futures_util::poll!(&mut shutdown), Poll::Pending));
        at_reap.await.expect("reap worker entered");
        release.send(()).expect("release reap worker");
        while !delivery.delivered() {
            tokio::task::yield_now().await;
        }

        drop(shutdown);
        let spawned = Arc::new(ChildProbe::default());
        let spawn = |_: &Path| -> Result<Box<dyn ChildProcess>> {
            backend.set_running(true);
            Ok(Box::new(FakeChild {
                probe: Arc::clone(&spawned),
            }))
        };
        assert!(sup
            .start_injected(&backend, &injected_binary, &spawn)
            .await
            .unwrap()
            .is_live());
        assert_eq!(sup.request_quit(), ExitRequest::Drain);

        delivery.release();
        while !delivery.finished() {
            tokio::task::yield_now().await;
        }
    }

    #[tokio::test]
    async fn nonzero_owned_exit_is_a_quit_failure_not_a_clean_drain() {
        let sup = Supervisor::default();
        let backend = ControlledBackend::new(true);
        *sup.child_slot() = Some(Box::new(DelayedChild {
            probe: Arc::new(ChildProbe::default()),
            exit_code: 23,
            reap_error: false,
            entered: None,
            release: None,
            finished: None,
        }));

        let error = sup.shutdown_injected(&backend).await.unwrap_err();
        assert_eq!(error.to_string(), MSG_QUIT_FAILED);
        assert!(sup.take_child().is_none());
    }

    #[tokio::test]
    async fn unknown_owned_exit_is_a_quit_failure_not_a_clean_drain() {
        let sup = Supervisor::default();
        let backend = ControlledBackend::new(true);
        *sup.child_slot() = Some(Box::new(UnknownExitChild));

        let error = sup.shutdown_injected(&backend).await.unwrap_err();
        assert_eq!(error.to_string(), MSG_QUIT_FAILED);
    }

    #[tokio::test]
    async fn exit_before_quit_continuation_preserves_the_success_permit() {
        let sup = Supervisor::default();
        let backend = ControlledBackend::new(true);
        let exited = Arc::new(AtomicBool::new(false));
        *sup.child_slot() = Some(Box::new(ControlledExitChild {
            exited: Arc::clone(&exited),
            code: Some(ChildExitCode::from_test_code(0)),
        }));

        assert_eq!(sup.request_quit(), ExitRequest::Drain);
        exited.store(true, Ordering::SeqCst);
        let permit = sup.shutdown_locked(&backend).await.unwrap();
        assert_eq!(backend.shutdown_calls.load(Ordering::SeqCst), 0);
        permit.allow_exit();
        assert_eq!(sup.request_quit(), ExitRequest::Allow);
    }

    #[tokio::test]
    async fn exit_before_quit_continuation_latches_nonzero_and_unknown_failures() {
        let nonzero = Supervisor::default();
        let nonzero_backend = ControlledBackend::new(true);
        let nonzero_exited = Arc::new(AtomicBool::new(false));
        *nonzero.child_slot() = Some(Box::new(ControlledExitChild {
            exited: Arc::clone(&nonzero_exited),
            code: Some(ChildExitCode::from_test_code(23)),
        }));

        assert_eq!(nonzero.request_quit(), ExitRequest::Drain);
        nonzero_exited.store(true, Ordering::SeqCst);
        let error = match nonzero.shutdown_locked(&nonzero_backend).await {
            Err(error) => error,
            Ok(_) => panic!("a nonzero exit cannot grant final exit"),
        };
        assert_eq!(error.to_string(), MSG_QUIT_FAILED);
        assert_eq!(nonzero.request_quit(), ExitRequest::AlreadyDraining);
        quit::finish_failure(&nonzero, |mut presentation| presentation.ack());
        assert_eq!(nonzero.request_quit(), ExitRequest::Allow);

        let unknown = Supervisor::default();
        let unknown_backend = ControlledBackend::new(true);
        let unknown_exited = Arc::new(AtomicBool::new(false));
        *unknown.child_slot() = Some(Box::new(ControlledExitChild {
            exited: Arc::clone(&unknown_exited),
            code: Some(ChildExitCode::unavailable()),
        }));

        assert_eq!(unknown.request_quit(), ExitRequest::Drain);
        unknown_exited.store(true, Ordering::SeqCst);
        let error = match unknown.shutdown_locked(&unknown_backend).await {
            Err(error) => error,
            Ok(_) => panic!("an unknown exit cannot grant final exit"),
        };
        assert_eq!(error.to_string(), MSG_QUIT_FAILED);
        assert_eq!(unknown.request_quit(), ExitRequest::AlreadyDraining);
        quit::finish_failure(&unknown, |mut presentation| presentation.ack());
        assert_eq!(unknown.request_quit(), ExitRequest::Allow);
    }

    #[test]
    fn already_exited_owned_children_refuse_quit_until_the_failure_is_surfaced() {
        let nonzero = Supervisor::default();
        let probe = Arc::new(ChildProbe::default());
        probe.exited.store(true, Ordering::SeqCst);
        *nonzero.child_slot() = Some(Box::new(FakeChild { probe }));
        assert_eq!(nonzero.request_quit(), ExitRequest::Failure);

        let unknown = Supervisor::default();
        *unknown.child_slot() = Some(Box::new(UnknownExitedChild));
        assert_eq!(unknown.request_quit(), ExitRequest::Failure);
    }

    #[tokio::test(start_paused = true)]
    async fn an_unready_owned_start_is_not_killed_or_replaced() {
        let sup = Supervisor::default();
        let backend = ControlledBackend::new(false);
        let child = Arc::new(ChildProbe::default());
        let spawns = AtomicUsize::new(0);
        let spawn = |_: &Path| -> Result<Box<dyn ChildProcess>> {
            spawns.fetch_add(1, Ordering::SeqCst);
            Ok(Box::new(FakeChild {
                probe: Arc::clone(&child),
            }))
        };
        let start = sup.start_injected(&backend, &injected_binary, &spawn);
        tokio::pin!(start);
        assert!(matches!(futures_util::poll!(&mut start), Poll::Pending));
        tokio::time::advance(Duration::from_secs(11)).await;
        assert!(start.await.is_err());
        assert_eq!(child.kills.load(Ordering::SeqCst), 0);
        assert_eq!(spawns.load(Ordering::SeqCst), 1);
        assert_eq!(
            sup.start_injected(&backend, &injected_binary, &spawn)
                .await
                .unwrap(),
            ServiceState::Unhealthy
        );
        assert_eq!(spawns.load(Ordering::SeqCst), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn adopted_restart_refuses_without_kill_or_spawn_when_it_stays_live() {
        let sup = Supervisor::default();
        let backend = AdoptedStaysLive {
            shutdown_calls: AtomicUsize::new(0),
        };
        let spawns = AtomicUsize::new(0);
        let spawn = |_: &Path| -> Result<Box<dyn ChildProcess>> {
            spawns.fetch_add(1, Ordering::SeqCst);
            Err(BackendError::Internal("spawn must not run".into()))
        };
        let restart = sup.restart_injected(&backend, &injected_binary, &spawn);
        tokio::pin!(restart);
        assert!(matches!(futures_util::poll!(&mut restart), Poll::Pending));
        tokio::time::advance(READY_TIMEOUT).await;
        tokio::task::yield_now().await;
        assert!(restart.await.is_err());
        assert_eq!(backend.shutdown_calls.load(Ordering::SeqCst), 1);
        assert_eq!(spawns.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn a_quit_drain_refuses_a_queued_start_before_final_exit() {
        let sup = Supervisor::default();
        let backend = ControlledBackend::new(false);
        let spawns = AtomicUsize::new(0);
        let spawn = |_: &Path| -> Result<Box<dyn ChildProcess>> {
            spawns.fetch_add(1, Ordering::SeqCst);
            Err(BackendError::Internal("spawn must not run".into()))
        };
        assert_eq!(sup.quit_gate.request(true), ExitRequest::Drain);

        assert_eq!(
            sup.start_injected(&backend, &injected_binary, &spawn)
                .await
                .unwrap(),
            ServiceState::Unhealthy
        );
        assert_eq!(spawns.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn a_no_child_quit_reservation_refuses_a_concurrent_start() {
        let sup = Supervisor::default();
        let backend = ControlledBackend::new(false);
        let spawns = AtomicUsize::new(0);
        let spawn = |_: &Path| -> Result<Box<dyn ChildProcess>> {
            spawns.fetch_add(1, Ordering::SeqCst);
            Err(BackendError::Internal("must not spawn".into()))
        };
        assert_eq!(sup.request_quit(), ExitRequest::Allow);
        assert_eq!(
            sup.start_injected(&backend, &injected_binary, &spawn)
                .await
                .unwrap(),
            ServiceState::Unhealthy
        );
        assert_eq!(spawns.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn cancelled_quit_nonzero_completion_latches_the_next_quit_failure() {
        let sup = Supervisor::default();
        let backend = ControlledBackend::new(true);
        let (entered, at_reap) = tokio::sync::oneshot::channel();
        let (release, wait) = std::sync::mpsc::channel();
        let (finished, done) = tokio::sync::oneshot::channel();
        *sup.child_slot() = Some(Box::new(DelayedChild {
            probe: Arc::new(ChildProbe::default()),
            exit_code: 23,
            reap_error: false,
            entered: Some(entered),
            release: Some(wait),
            finished: Some(finished),
        }));
        let mut quit = Box::pin(sup.shutdown_injected(&backend));
        assert!(matches!(futures_util::poll!(&mut quit), Poll::Pending));
        at_reap.await.unwrap();
        drop(quit);
        release.send(()).unwrap();
        done.await.unwrap();
        assert_eq!(sup.request_quit(), ExitRequest::Failure);
    }

    #[tokio::test]
    async fn cancelled_quit_io_completion_restores_the_owned_child_for_a_fresh_quit() {
        let sup = Supervisor::default();
        let backend = ControlledBackend::new(true);
        let (entered, at_reap) = tokio::sync::oneshot::channel();
        let (release, wait) = std::sync::mpsc::channel();
        let (finished, done) = tokio::sync::oneshot::channel();
        *sup.child_slot() = Some(Box::new(DelayedChild {
            probe: Arc::new(ChildProbe::default()),
            exit_code: 0,
            reap_error: true,
            entered: Some(entered),
            release: Some(wait),
            finished: Some(finished),
        }));
        let mut quit = Box::pin(sup.shutdown_injected(&backend));
        assert!(matches!(futures_util::poll!(&mut quit), Poll::Pending));
        at_reap.await.unwrap();
        drop(quit);
        release.send(()).unwrap();
        done.await.unwrap();
        assert!(sup.holds_child());
        assert_eq!(sup.request_quit(), ExitRequest::Drain);
    }

    #[tokio::test]
    async fn a_held_quit_io_error_keeps_its_drain_until_failure_is_surfaced() {
        let sup = Supervisor::default();
        let backend = ControlledBackend::new(true);
        let (entered, at_reap) = tokio::sync::oneshot::channel();
        let (release, wait) = std::sync::mpsc::channel();
        let (finished, done) = tokio::sync::oneshot::channel();
        *sup.child_slot() = Some(Box::new(DelayedChild {
            probe: Arc::new(ChildProbe::default()),
            exit_code: 0,
            reap_error: true,
            entered: Some(entered),
            release: Some(wait),
            finished: Some(finished),
        }));

        assert_eq!(sup.request_quit(), ExitRequest::Drain);
        let mut first = Box::pin(sup.shutdown_injected(&backend));
        assert!(matches!(futures_util::poll!(&mut first), Poll::Pending));
        at_reap.await.unwrap();
        release.send(()).unwrap();
        done.await.unwrap();
        while sup.child_pending.load(Ordering::SeqCst) {
            tokio::task::yield_now().await;
        }

        assert_eq!(sup.request_quit(), ExitRequest::AlreadyDraining);
        assert!(first.await.is_err());
        let shown = AtomicBool::new(false);
        quit::finish_failure(&sup, |mut presentation| {
            shown.store(true, Ordering::SeqCst);
            presentation.ack();
        });
        assert!(shown.load(Ordering::SeqCst));
        assert_eq!(sup.request_quit(), ExitRequest::Drain);
    }

    #[tokio::test]
    async fn panicking_reaper_restores_ownership_and_latches_a_failure() {
        let sup = Supervisor::default();
        let backend = ControlledBackend::new(true);
        *sup.child_slot() = Some(Box::new(PanickingChild));
        assert!(sup.shutdown_injected(&backend).await.is_err());
        assert!(!sup.child_pending.load(Ordering::SeqCst));
        assert!(sup.holds_child());
        assert_eq!(sup.request_quit(), ExitRequest::Failure);
    }

    #[tokio::test]
    async fn cancelled_restart_blocks_start_until_an_io_error_restores_the_old_child() {
        let sup = Supervisor::default();
        let backend = ControlledBackend::new(true);
        let (entered, at_reap) = tokio::sync::oneshot::channel();
        let (release, wait) = std::sync::mpsc::channel();
        let (finished, done) = tokio::sync::oneshot::channel();
        *sup.child_slot() = Some(Box::new(DelayedChild {
            probe: Arc::new(ChildProbe::default()),
            exit_code: 0,
            reap_error: true,
            entered: Some(entered),
            release: Some(wait),
            finished: Some(finished),
        }));
        let spawn_count = AtomicUsize::new(0);
        let spawn = |_: &Path| -> Result<Box<dyn ChildProcess>> {
            spawn_count.fetch_add(1, Ordering::SeqCst);
            Err(BackendError::Internal("must not spawn".into()))
        };
        let mut restart = Box::pin(sup.restart_injected(&backend, &injected_binary, &spawn));
        assert!(matches!(futures_util::poll!(&mut restart), Poll::Pending));
        at_reap.await.unwrap();
        drop(restart);
        assert_eq!(
            sup.start_injected(&backend, &injected_binary, &spawn)
                .await
                .unwrap(),
            ServiceState::Unhealthy
        );
        assert_eq!(spawn_count.load(Ordering::SeqCst), 0);
        release.send(()).unwrap();
        done.await.unwrap();
        assert!(sup.holds_child());
        assert_eq!(sup.request_quit(), ExitRequest::Drain);
    }
}
