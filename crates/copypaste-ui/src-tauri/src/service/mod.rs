use backon::{ExponentialBuilder, Retryable};
use serde::Serialize;
use std::path::Path;
#[cfg(test)]
use std::sync::Mutex;
use std::time::Duration;
use tokio::sync::Mutex as AsyncMutex;

use crate::backend::{Backend, BackendError, Result};

mod child;
pub mod diagnostics;
pub mod locate;
mod owned_child;
pub mod push;
pub(crate) mod quit;
mod spawn;
pub(crate) mod startup_diagnostics;

use child::ChildProcess;
#[cfg(test)]
use child::ChildState;
use owned_child::{BeginReap, OwnedChild, StartReservation};
#[cfg(any(test, target_os = "macos", target_os = "windows"))]
use quit::UpdateDrainPermit;
use quit::{
    ExitRequest, FailurePresentation, QuitGate, ReapReservation, ShutdownPermit, MSG_QUIT_FAILED,
};
use spawn::spawn_process;

const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

const MSG_NOT_INSTALLED: &str = "This build of CopyPaste doesn't include the background service.";
const MSG_START_FAILED: &str = "The background service could not be started.";
const MSG_NEVER_READY: &str = "The background service started but didn't finish coming up.";

const READY_TIMEOUT: Duration = Duration::from_secs(10);

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

#[cfg(test)]
impl Supervisor {
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
    async fn shutdown_injected<B: ServiceBackend>(&self, backend: &B) -> Result<()> {
        let _lifecycle = self.lifecycle.lock().await;
        self.shutdown_locked(backend).await.map(|_| ())
    }
    #[cfg(any(test, target_os = "macos", target_os = "windows"))]
    async fn install_after_update_drain_injected<B: ServiceBackend, T>(
        &self,
        backend: &B,
        install: impl FnOnce(UpdateDrainPermit) -> T,
    ) -> Result<T> {
        self.install_after_update_drain_with(backend, install).await
    }
    fn child_slot(&self) -> std::sync::MutexGuard<'_, Option<Box<dyn ChildProcess>>> {
        self.owned.test_slot()
    }
    fn take_child(&self) -> Option<Box<dyn ChildProcess>> {
        self.owned.take_for_test()
    }
    fn holds_child(&self) -> bool {
        self.owned.service_is_owned()
    }
    fn hold_next_reap_delivery(&self) -> owned_child::ReapDeliveryHold {
        self.owned.hold_next_reap_delivery()
    }
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
    owned: OwnedChild,
    quit_gate: QuitGate,
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
            owned: OwnedChild::default(),
            quit_gate: QuitGate::default(),
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
        if self.owned.is_reap_pending() {
            return ServiceState::Unhealthy;
        }
        match backend.service_status().await {
            Ok(status) => ServiceState::Running {
                matches_app: status.version == APP_VERSION,
                version: status.version,
                ours: self.owned.service_is_owned(),
            },
            Err(BackendError::Unreachable) => {
                if self.owned.service_is_owned() {
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
        if !self
            .owned
            .spawn_if_start_admitted(&self.quit_gate, spawn, &binary)?
        {
            return Ok(ServiceState::Unhealthy);
        }
        self.await_ready(backend).await?;
        let state = self.service_state_with(backend, locate).await;
        if state.is_live() {
            self.owned.clear_terminal_failure();
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
            if self.owned.service_is_owned() {
                self.reap_owned_child(None).await?;
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
        self.owned.wait_for_reap_completion().await;
        if !self.owned.service_is_owned() {
            if self.owned.terminal_failure() {
                return Err(BackendError::Internal(MSG_QUIT_FAILED.into()));
            }
            return Ok(ShutdownPermit(Some(self.quit_gate.clone())));
        }
        backend.stop_service().await?;
        let reservation = self
            .reap_owned_child(Some(ReapReservation::ordinary(self.quit_gate.clone())))
            .await?;
        reservation
            .and_then(ReapReservation::into_ordinary)
            .ok_or_else(|| BackendError::Internal(MSG_QUIT_FAILED.into()))
    }

    /// Reserve update installation before changing the service state.
    ///
    /// The permit reaches the platform installer only after the owned child
    /// has actually exited, so cancelling its async caller cannot admit a new
    /// service while a synchronous reap or installer still owns the update.
    #[cfg(any(test, target_os = "macos", target_os = "windows"))]
    async fn drain_for_update<B: ServiceBackend>(&self, backend: &B) -> Result<UpdateDrainPermit> {
        let permit =
            UpdateDrainPermit::reserve(self.quit_gate.clone()).ok_or(BackendError::NotReady)?;
        let _lifecycle = self.lifecycle.lock().await;
        self.drain_for_update_locked(backend, permit).await
    }

    #[cfg(any(test, target_os = "macos", target_os = "windows"))]
    pub(crate) async fn install_after_update_drain<B: Backend, T>(
        &self,
        backend: &B,
        install: impl FnOnce(UpdateDrainPermit) -> T,
    ) -> Result<T> {
        self.install_after_update_drain_with(backend, install).await
    }

    #[cfg(any(test, target_os = "macos", target_os = "windows"))]
    async fn install_after_update_drain_with<B: ServiceBackend, T>(
        &self,
        backend: &B,
        install: impl FnOnce(UpdateDrainPermit) -> T,
    ) -> Result<T> {
        Ok(install(self.drain_for_update(backend).await?))
    }

    #[cfg(any(test, target_os = "macos", target_os = "windows"))]
    async fn drain_for_update_locked<B: ServiceBackend>(
        &self,
        backend: &B,
        permit: UpdateDrainPermit,
    ) -> Result<UpdateDrainPermit> {
        if self.owned.terminal_failure() {
            return Err(BackendError::Internal(MSG_QUIT_FAILED.into()));
        }
        self.owned.wait_for_reap_completion().await;
        if self.owned.terminal_failure() {
            return Err(BackendError::Internal(MSG_QUIT_FAILED.into()));
        }
        let state = self.service_state(backend).await;
        if self.owned.terminal_failure() {
            return Err(BackendError::Internal(MSG_QUIT_FAILED.into()));
        }
        match state {
            ServiceState::Stopped | ServiceState::NotInstalled => Ok(permit),
            ServiceState::Running { .. } | ServiceState::Unhealthy => {
                backend.stop_service().await?;
                if self.owned.service_is_owned() {
                    let reservation = self
                        .reap_owned_child(Some(ReapReservation::Update(permit)))
                        .await?;
                    reservation
                        .and_then(ReapReservation::into_update)
                        .ok_or_else(|| BackendError::Internal(MSG_QUIT_FAILED.into()))
                } else {
                    if self.owned.terminal_failure() {
                        return Err(BackendError::Internal(MSG_QUIT_FAILED.into()));
                    }
                    self.await_stopped(backend).await?;
                    if self.owned.terminal_failure() {
                        return Err(BackendError::Internal(MSG_QUIT_FAILED.into()));
                    }
                    Ok(permit)
                }
            }
        }
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
            if self.owned.child_exited() {
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

    async fn reap_owned_child(
        &self,
        reservation: Option<ReapReservation>,
    ) -> Result<Option<ReapReservation>> {
        let received = match self.owned.begin_reap(reservation) {
            BeginReap::Started(received) => received,
            BeginReap::AlreadyExited(reservation) => {
                return self
                    .owned
                    .terminal_failure()
                    .then_some(())
                    .map_or(Ok(reservation), |_| {
                        Err(BackendError::Internal(MSG_QUIT_FAILED.into()))
                    });
            }
        };

        match received.await {
            Err(_) => Err(BackendError::Internal(MSG_QUIT_FAILED.into())),
            Ok(completion) => match completion.finish() {
                (Ok(code), reservation) if code.is_success() => Ok(reservation),
                (Ok(code), reservation) => {
                    if let Some(reservation) = reservation {
                        reservation.retain_ordinary_failure();
                    }
                    startup_diagnostics::child_exited(code);
                    Err(BackendError::Internal(MSG_QUIT_FAILED.into()))
                }
                (Err(error), reservation) => {
                    if let Some(reservation) = reservation {
                        reservation.retain_ordinary_failure();
                    }
                    tracing::warn!(%error, "the background service could not be reaped");
                    Err(BackendError::Internal(MSG_QUIT_FAILED.into()))
                }
            },
        }
    }

    pub(crate) fn request_quit(&self) -> ExitRequest {
        self.owned.request_quit(&self.quit_gate)
    }

    fn reserve_start(&self) -> Option<StartReservation> {
        self.owned.reserve_start(&self.quit_gate)
    }

    pub(crate) fn failure_presentation(&self) -> FailurePresentation {
        if !self.quit_gate.is_reserved() {
            self.quit_gate.reserve_failure();
        }
        FailurePresentation::new(self.owned.terminal_failure_handle(), self.quit_gate.clone())
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
        self.owned.forget_on_drop();
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
    struct StubbornAdopted {
        shutdown_calls: AtomicUsize,
    }

    #[derive(Default)]
    struct UnhealthyAdopted {
        stopped: AtomicBool,
        shutdown_calls: AtomicUsize,
    }

    impl ServiceBackend for UnhealthyAdopted {
        async fn service_status(&self) -> Result<StatusData> {
            if self.stopped.load(Ordering::SeqCst) {
                Err(BackendError::Unreachable)
            } else {
                Err(BackendError::Internal("controlled unhealthy status".into()))
            }
        }

        async fn stop_service(&self) -> Result<()> {
            self.shutdown_calls.fetch_add(1, Ordering::SeqCst);
            self.stopped.store(true, Ordering::SeqCst);
            Ok(())
        }
    }

    #[derive(Default)]
    struct UnhealthyStopFails {
        shutdown_calls: AtomicUsize,
    }

    impl ServiceBackend for UnhealthyStopFails {
        async fn service_status(&self) -> Result<StatusData> {
            Err(BackendError::Internal("controlled unhealthy status".into()))
        }

        async fn stop_service(&self) -> Result<()> {
            self.shutdown_calls.fetch_add(1, Ordering::SeqCst);
            Err(BackendError::Internal("controlled shutdown failure".into()))
        }
    }

    impl ServiceBackend for StubbornAdopted {
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

    struct BlockingRelease(Option<std::sync::mpsc::Sender<()>>);

    impl BlockingRelease {
        fn release(&mut self) {
            if let Some(release) = self.0.take() {
                let _ = release.send(());
            }
        }
    }

    impl Drop for BlockingRelease {
        fn drop(&mut self) {
            self.release();
        }
    }

    struct UnknownExitChild;

    struct ControlledExitChild {
        exited: Arc<AtomicBool>,
        code: Option<ChildExitCode>,
    }

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

    async fn update_drain_refuses_failed_reap(
        child: Box<dyn ChildProcess>,
        expected_quit: ExitRequest,
    ) {
        let sup = Supervisor::default();
        let backend = ControlledBackend::new(true);
        let installed = AtomicBool::new(false);
        *sup.child_slot() = Some(child);

        let error = sup
            .install_after_update_drain_injected(&backend, |permit| {
                installed.store(true, Ordering::SeqCst);
                drop(permit);
            })
            .await
            .expect_err("a failed owned reap cannot install an update");

        assert_eq!(error.to_string(), MSG_QUIT_FAILED);
        assert!(!installed.load(Ordering::SeqCst));
        assert_eq!(sup.request_quit(), expected_quit);
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
        sup.owned.set_terminal_failure_for_test();

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
        sup.owned.set_terminal_failure_for_test();

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
        sup.owned.set_terminal_failure_for_test();
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

    #[tokio::test]
    async fn update_drain_reserves_quit_until_the_installer_receives_it() {
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

        let installed = AtomicBool::new(false);
        let mut drain = Box::pin(sup.install_after_update_drain_injected(&backend, |permit| {
            installed.store(true, Ordering::SeqCst);
            permit
        }));
        assert!(matches!(futures_util::poll!(&mut drain), Poll::Pending));
        at_reap.await.expect("owned reap began");
        assert!(
            !installed.load(Ordering::SeqCst),
            "installation cannot begin before the owned child is reaped"
        );
        assert_eq!(sup.request_quit(), ExitRequest::AlreadyDraining);
        release.send(()).expect("release owned reap");

        let permit = drain.await.expect("successful owned update drain");
        assert_eq!(sup.request_quit(), ExitRequest::AlreadyDraining);
        let spawns = AtomicUsize::new(0);
        let start = |_: &Path| -> Result<Box<dyn ChildProcess>> {
            spawns.fetch_add(1, Ordering::SeqCst);
            Err(BackendError::Internal(
                "must not start during update".into(),
            ))
        };
        assert_eq!(
            sup.start_injected(&backend, &injected_binary, &start)
                .await
                .expect("reserved start is a safe no-op"),
            ServiceState::Unhealthy
        );
        assert_eq!(spawns.load(Ordering::SeqCst), 0);
        drop(permit);
        assert_eq!(sup.request_quit(), ExitRequest::Allow);
    }

    #[tokio::test]
    async fn cancelling_an_update_drain_keeps_the_reservation_until_owned_reap_finishes() {
        let sup = Supervisor::default();
        let backend = ControlledBackend::new(true);
        let (entered, at_reap) = tokio::sync::oneshot::channel();
        let (finished, reap_finished) = tokio::sync::oneshot::channel();
        let (release, wait) = std::sync::mpsc::channel();
        *sup.child_slot() = Some(Box::new(DelayedChild {
            probe: Arc::new(ChildProbe::default()),
            exit_code: 0,
            reap_error: false,
            entered: Some(entered),
            release: Some(wait),
            finished: Some(finished),
        }));

        let mut drain =
            Box::pin(sup.install_after_update_drain_injected(&backend, |permit| permit));
        assert!(matches!(futures_util::poll!(&mut drain), Poll::Pending));
        at_reap.await.expect("owned reap began");
        drop(drain);
        assert_eq!(sup.request_quit(), ExitRequest::AlreadyDraining);

        release.send(()).expect("release owned reap");
        reap_finished.await.expect("owned reap completed");
        while sup.owned.is_reap_pending_for_test() {
            tokio::task::yield_now().await;
        }
        assert_eq!(sup.request_quit(), ExitRequest::Allow);
    }

    #[tokio::test]
    async fn cancelling_after_update_reap_delivery_releases_the_reservation() {
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
        let mut update =
            Box::pin(sup.install_after_update_drain_injected(&backend, |permit| permit));

        assert!(matches!(futures_util::poll!(&mut update), Poll::Pending));
        at_reap.await.expect("owned reap began");
        release.send(()).expect("release owned reap");
        while !delivery.delivered() {
            tokio::task::yield_now().await;
        }
        drop(update);

        let spawns = AtomicUsize::new(0);
        let spawn = |_: &Path| -> Result<Box<dyn ChildProcess>> {
            spawns.fetch_add(1, Ordering::SeqCst);
            backend.set_running(true);
            Ok(Box::new(FakeChild {
                probe: Arc::new(ChildProbe::default()),
            }))
        };
        assert!(sup
            .start_injected(&backend, &injected_binary, &spawn)
            .await
            .expect("cancelled update releases start")
            .is_live());
        assert_eq!(sup.request_quit(), ExitRequest::Drain);
        assert_eq!(spawns.load(Ordering::SeqCst), 1);
        delivery.release();
    }

    #[tokio::test]
    async fn update_drain_stops_an_adopted_service_without_a_child_signal() {
        let sup = Supervisor::default();
        let backend = ControlledBackend::new(true);

        let permit = sup
            .install_after_update_drain_injected(&backend, |permit| permit)
            .await
            .expect("adopted service confirmed stopped");

        assert_eq!(backend.shutdown_calls.load(Ordering::SeqCst), 1);
        assert_eq!(sup.request_quit(), ExitRequest::AlreadyDraining);
        drop(permit);
        assert_eq!(sup.request_quit(), ExitRequest::Allow);
    }

    #[tokio::test]
    async fn cancelling_an_update_before_shutdown_ack_keeps_the_owned_child() {
        let sup = Supervisor::default();
        let backend = ControlledBackend::new(true);
        backend.shutdown_gate.arm();
        *sup.child_slot() = Some(Box::new(FakeChild {
            probe: Arc::new(ChildProbe::default()),
        }));
        let installed = AtomicBool::new(false);
        let mut update = Box::pin(sup.install_after_update_drain_injected(&backend, |permit| {
            installed.store(true, Ordering::SeqCst);
            drop(permit);
        }));

        assert!(matches!(futures_util::poll!(&mut update), Poll::Pending));
        backend.shutdown_gate.wait_until_entered().await;
        drop(update);

        assert!(!installed.load(Ordering::SeqCst));
        assert!(sup.holds_child());
        assert_eq!(sup.request_quit(), ExitRequest::Drain);
    }

    #[tokio::test]
    async fn an_owned_child_that_exits_nonzero_after_update_ack_refuses_install() {
        let sup = Supervisor::default();
        let backend = ControlledBackend::new(true);
        backend.shutdown_gate.arm();
        let exited = Arc::new(AtomicBool::new(false));
        *sup.child_slot() = Some(Box::new(ControlledExitChild {
            exited: Arc::clone(&exited),
            code: Some(ChildExitCode::from_test_code(23)),
        }));
        let installed = AtomicBool::new(false);
        let mut update = Box::pin(sup.install_after_update_drain_injected(&backend, |permit| {
            installed.store(true, Ordering::SeqCst);
            drop(permit);
        }));

        assert!(matches!(futures_util::poll!(&mut update), Poll::Pending));
        backend.shutdown_gate.wait_until_entered().await;
        exited.store(true, Ordering::SeqCst);
        backend.shutdown_gate.resume();
        let error = update
            .await
            .expect_err("nonzero exit prevents update installation");

        assert_eq!(error.to_string(), MSG_QUIT_FAILED);
        assert!(!installed.load(Ordering::SeqCst));
        assert_eq!(sup.request_quit(), ExitRequest::Failure);
    }

    #[tokio::test]
    async fn a_no_service_update_hands_a_reservation_to_the_installer() {
        let sup = Supervisor::default();
        let backend = ControlledBackend::new(false);

        let permit = sup
            .install_after_update_drain_injected(&backend, |permit| permit)
            .await
            .expect("no-service update can hand off to the installer");

        assert_eq!(sup.request_quit(), ExitRequest::AlreadyDraining);
        drop(permit);
        assert_eq!(sup.request_quit(), ExitRequest::Allow);
    }

    #[tokio::test]
    async fn a_rejected_update_reservation_cannot_clear_an_ordinary_drain() {
        let sup = Supervisor::default();
        assert_eq!(sup.quit_gate.request(true), ExitRequest::Drain);

        assert!(UpdateDrainPermit::reserve(sup.quit_gate.clone()).is_none());
        assert_eq!(sup.request_quit(), ExitRequest::AlreadyDraining);
        let backend = ControlledBackend::new(false);
        let spawns = AtomicUsize::new(0);
        let spawn = |_: &Path| -> Result<Box<dyn ChildProcess>> {
            spawns.fetch_add(1, Ordering::SeqCst);
            unreachable!("the reserved drain blocks start")
        };
        assert_eq!(
            sup.start_injected(&backend, &injected_binary, &spawn)
                .await
                .expect("reserved start is a no-op"),
            ServiceState::Unhealthy
        );
        assert_eq!(spawns.load(Ordering::SeqCst), 0);
    }

    #[tokio::test(start_paused = true)]
    async fn update_refuses_an_adopted_service_that_never_confirms_stop() {
        let sup = Supervisor::default();
        let backend = StubbornAdopted::default();
        let installed = AtomicBool::new(false);
        let mut update = Box::pin(sup.install_after_update_drain_injected(&backend, |permit| {
            installed.store(true, Ordering::SeqCst);
            drop(permit);
        }));

        assert!(matches!(futures_util::poll!(&mut update), Poll::Pending));
        tokio::time::advance(READY_TIMEOUT + Duration::from_secs(1)).await;
        let error = update
            .await
            .expect_err("unconfirmed adopted stop refuses the update");

        assert_eq!(error.to_string(), MSG_NEVER_READY);
        assert_eq!(backend.shutdown_calls.load(Ordering::SeqCst), 1);
        assert!(!installed.load(Ordering::SeqCst));
        assert_eq!(sup.request_quit(), ExitRequest::Allow);
    }

    #[tokio::test]
    async fn update_drain_refuses_nonzero_unknown_io_and_panic_without_installing() {
        update_drain_refuses_failed_reap(
            Box::new(DelayedChild {
                probe: Arc::new(ChildProbe::default()),
                exit_code: 23,
                reap_error: false,
                entered: None,
                release: None,
                finished: None,
            }),
            ExitRequest::Failure,
        )
        .await;
        update_drain_refuses_failed_reap(Box::new(UnknownExitChild), ExitRequest::Failure).await;
        update_drain_refuses_failed_reap(
            Box::new(DelayedChild {
                probe: Arc::new(ChildProbe::default()),
                exit_code: 0,
                reap_error: true,
                entered: None,
                release: None,
                finished: None,
            }),
            ExitRequest::Drain,
        )
        .await;
        update_drain_refuses_failed_reap(Box::new(PanickingChild), ExitRequest::Failure).await;
    }

    #[tokio::test]
    async fn update_drain_stops_an_unhealthy_adopted_service_before_installing() {
        let sup = Supervisor::default();
        let backend = UnhealthyAdopted::default();

        let permit = sup
            .install_after_update_drain_injected(&backend, |permit| permit)
            .await
            .expect("shutdown acknowledgement and vanished endpoint drain the update");

        assert_eq!(backend.shutdown_calls.load(Ordering::SeqCst), 1);
        assert!(backend.stopped.load(Ordering::SeqCst));
        assert_eq!(sup.request_quit(), ExitRequest::AlreadyDraining);
        drop(permit);
        assert_eq!(sup.request_quit(), ExitRequest::Allow);
    }

    #[tokio::test]
    async fn update_drain_refuses_an_unhealthy_service_when_shutdown_fails() {
        let sup = Supervisor::default();
        let backend = UnhealthyStopFails::default();
        let installed = AtomicBool::new(false);
        let error = sup
            .install_after_update_drain_injected(&backend, |permit| {
                installed.store(true, Ordering::SeqCst);
                drop(permit);
            })
            .await
            .expect_err("a failed shutdown cannot grant an update boundary");

        assert!(matches!(error, BackendError::Internal(_)));
        assert_eq!(backend.shutdown_calls.load(Ordering::SeqCst), 1);
        assert!(!installed.load(Ordering::SeqCst));
        assert_eq!(sup.request_quit(), ExitRequest::Allow);
    }

    #[tokio::test]
    async fn cancelling_a_blocking_installer_keeps_the_update_reservation() {
        let sup = Supervisor::default();
        let backend = ControlledBackend::new(false);
        let (entered, installer_entered) = tokio::sync::oneshot::channel();
        let (finished, installer_finished) = tokio::sync::oneshot::channel();
        let (release, wait) = std::sync::mpsc::channel();

        let mut operation = Box::pin(async {
            let installer = sup
                .install_after_update_drain_injected(&backend, move |permit| {
                    tokio::task::spawn_blocking(move || {
                        let permit = permit;
                        let _ = entered.send(());
                        let _ = wait.recv();
                        drop(permit);
                        let _ = finished.send(());
                    })
                })
                .await
                .expect("update drain completed");
            installer.await.expect("installer worker joined");
        });
        assert!(matches!(futures_util::poll!(&mut operation), Poll::Pending));
        installer_entered.await.expect("blocking installer began");
        drop(operation);
        assert_eq!(sup.request_quit(), ExitRequest::AlreadyDraining);

        release.send(()).expect("release installer");
        installer_finished.await.expect("installer finished");
        assert_eq!(sup.request_quit(), ExitRequest::Allow);
    }

    #[tokio::test]
    async fn an_installer_error_releases_the_update_reservation() {
        let sup = Supervisor::default();
        let backend = ControlledBackend::new(false);
        let result = sup
            .install_after_update_drain_injected(&backend, |permit| {
                drop(permit);
                Err::<(), _>("installer failed")
            })
            .await
            .expect("drain passed the installer result through");

        assert_eq!(result, Err("installer failed"));
        assert_eq!(sup.request_quit(), ExitRequest::Allow);
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

        assert!(sup.owned.is_reap_pending_for_test());
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
        while sup.owned.is_reap_pending_for_test() {
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
        assert!(!sup.owned.is_reap_pending_for_test());
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
        while sup.owned.is_reap_pending_for_test() {
            tokio::task::yield_now().await;
        }
        assert!(sup.holds_child());
        assert_eq!(sup.request_quit(), ExitRequest::Drain);
    }

    #[tokio::test]
    async fn shutdown_waits_for_a_cancelled_restart_reap_before_deciding_exit() {
        for reap_error in [false, true] {
            let sup = Supervisor::default();
            let backend = ControlledBackend::new(true);
            let (entered, at_reap) = tokio::sync::oneshot::channel();
            let (release, wait) = std::sync::mpsc::channel();
            let mut release = BlockingRelease(Some(release));
            *sup.child_slot() = Some(Box::new(DelayedChild {
                probe: Arc::new(ChildProbe::default()),
                exit_code: 0,
                reap_error,
                entered: Some(entered),
                release: Some(wait),
                finished: None,
            }));

            let no_restart = |_: &Path| -> Result<Box<dyn ChildProcess>> {
                Err(BackendError::Internal("restart must stay cancelled".into()))
            };
            let mut restart =
                Box::pin(sup.restart_injected(&backend, &injected_binary, &no_restart));
            assert!(matches!(futures_util::poll!(&mut restart), Poll::Pending));
            at_reap.await.expect("restart began owned reap");
            drop(restart);
            assert!(matches!(
                sup.request_quit(),
                ExitRequest::Drain | ExitRequest::AlreadyDraining
            ));

            let mut shutdown = Box::pin(sup.shutdown_locked(&backend));
            assert!(matches!(futures_util::poll!(&mut shutdown), Poll::Pending));
            assert_eq!(sup.request_quit(), ExitRequest::AlreadyDraining);

            release.release();
            if reap_error {
                assert!(shutdown.await.is_err());
                assert!(matches!(
                    sup.request_quit(),
                    ExitRequest::Drain | ExitRequest::AlreadyDraining
                ));
            } else {
                let permit = shutdown.await.expect("clean reap permits exit");
                assert_eq!(sup.request_quit(), ExitRequest::AlreadyDraining);
                permit.allow_exit();
                assert_eq!(sup.request_quit(), ExitRequest::Allow);
            }
        }
    }
}
