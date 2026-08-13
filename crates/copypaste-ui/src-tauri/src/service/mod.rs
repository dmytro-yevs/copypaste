//! Starting and stopping the background service (ADR-0004).
//!
//! The app owns the daemon's lifetime on macOS: opening the app starts it,
//! quitting stops it, and no launchd agent is installed. The ADR carries the
//! argument and the rejected alternatives; what is here is the mechanism and
//! the two rules that are easy to lose in a refactor.
//!
//! **Only a daemon this process started is ever stopped.** [`Supervisor::stop`]
//! acts on a child handle, never on a pid discovered from the socket. An
//! adopted daemon — one already running when the app launched — is used and
//! reported, never killed.
//!
//! **No path reaches a user.** The binary's location is never interpolated into
//! a message; every failure here is a fixed sentence (AGENTS.md rule 4).
//!
//! # Android
//!
//! Compiled, and inert by construction rather than by `cfg`. The embedded
//! backend's `status` always succeeds, so [`Supervisor::state`] answers
//! `Running` and [`Supervisor::start`] returns without looking for a binary.
//! That keeps `crate::commands` free of `cfg`, which is what ADR-0002 asks for.

use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

use backon::{ExponentialBuilder, Retryable};
use serde::Serialize;
use tokio::sync::Mutex as AsyncMutex;
use tokio::time::timeout;

use crate::backend::{Backend, BackendError, Result};

mod child;
pub mod diagnostics;
pub mod locate;
pub mod push;
mod spawn;

use child::{end_child, ChildProcess, FORCED_STOP_BUDGET};
use spawn::spawn_process;

/// The version this app expects the daemon to report. Both come from the
/// workspace version, so a mismatch means two different installs, not a
/// packaging slip.
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

const MSG_NOT_INSTALLED: &str = "This build of CopyPaste doesn't include the background service.";
const MSG_START_FAILED: &str = "The background service could not be started.";
const MSG_NEVER_READY: &str = "The background service started but didn't finish coming up.";

/// How long to wait for a freshly started daemon to answer `status`.
///
/// It opens SQLCipher and derives a key before it binds, so the first answer is
/// not instant. Ten seconds is long enough for a cold start on a slow disk and
/// short enough that a daemon which is never going to answer is reported rather
/// than waited on.
const READY_TIMEOUT: Duration = Duration::from_secs(10);

/// The whole of a graceful quit, from asking the daemon to stop to seeing it
/// gone.
///
/// The two steps inside it are bounded individually, but quit is the caller and
/// the number it needs is how long the *app* can take to close — two bounded
/// steps in series is still their sum. Past this the child is stopped outright,
/// which is the bounded fallback ADR-0004 accepts: WAL makes a killed daemon
/// recoverable, and a user waiting on a window that will not close does not
/// know that anything is being waited for.
const SHUTDOWN_BUDGET: Duration = Duration::from_secs(15);

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
    child: Mutex<Option<Box<dyn ChildProcess>>>,
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
            child: Mutex::new(None),
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
        match backend.service_status().await {
            Ok(status) => ServiceState::Running {
                matches_app: status.version == APP_VERSION,
                version: status.version,
                ours: self.holds_child(),
            },
            Err(BackendError::Unreachable) => {
                if locate().is_some() {
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
        let state = self.service_state_with(backend, locate).await;
        match state {
            ServiceState::NotInstalled => return Err(BackendError::Unsupported(MSG_NOT_INSTALLED)),
            ServiceState::Stopped => {}
            live => return Ok(live),
        }

        self.stop();
        let binary = locate().ok_or(BackendError::Unsupported(MSG_NOT_INSTALLED))?;
        self.spawn(&binary, spawn)?;
        if let Err(error) = self.await_ready(backend).await {
            self.stop();
            return Err(error);
        }
        Ok(self.service_state_with(backend, locate).await)
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
            self.await_stopped(backend).await?;
            self.reap_child().await;
        }
        self.start_locked(backend, locate, spawn).await
    }

    /// Gracefully stop the daemon when this app process owns it.
    pub async fn shutdown<B: Backend>(&self, backend: &B) {
        let _lifecycle = self.lifecycle.lock().await;
        self.shutdown_locked(backend).await;
    }

    async fn shutdown_locked<B: ServiceBackend>(&self, backend: &B) {
        if !self.holds_child() {
            return;
        }
        let graceful = async {
            backend.stop_service().await.is_ok() && self.await_stopped(backend).await.is_ok()
        };
        if matches!(
            timeout(SHUTDOWN_BUDGET - FORCED_STOP_BUDGET, graceful).await,
            Ok(true)
        ) {
            self.reap_child().await;
            return;
        }
        self.stop_within_budget().await;
    }

    /// Kill and reap the child on a worker, and stop waiting at the budget.
    ///
    /// Both calls are the OS's business and neither has a timeout of its own,
    /// so neither may be the last thing between a user and a window that
    /// closes. Giving up leaves at worst a zombie — one slot in the process
    /// table — and on Windows the job handle went with `kill`, so the kernel
    /// ends the child regardless.
    async fn stop_within_budget(&self) {
        let Some(mut child) = self.take_child() else {
            return;
        };
        let ended = tokio::task::spawn_blocking(move || {
            let _ = child.kill();
            let _ = child.reap();
        });
        if timeout(FORCED_STOP_BUDGET, ended).await.is_err() {
            tracing::warn!("the background service did not stop within the quit budget");
        }
    }

    /// Stop the daemon this process started. Safe to call when there is none.
    ///
    /// `kill` is `SIGKILL` — `std::process::Child` sends nothing else. WAL mode
    /// makes that recoverable and the daemon clears its own stale socket on the
    /// next bind; ADR-0004 records it as a cost rather than a preference.
    fn stop(&self) {
        if let Some(child) = self.take_child() {
            end_child(child);
        }
    }

    fn spawn<S>(&self, binary: &Path, spawn: &S) -> Result<()>
    where
        S: Fn(&Path) -> Result<Box<dyn ChildProcess>>,
    {
        let child = spawn(binary)?;
        if let Some(previous) = self.child_slot().replace(child) {
            end_child(previous);
        }
        Ok(())
    }

    /// Wait for the daemon to answer, or for it to die trying.
    ///
    /// `backon` rather than a sleep loop: the workspace already carries one
    /// retry implementation and v1 grew six (AGENTS.md rule 1).
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
        match slot.as_mut().map(|child| child.reap_if_exited()) {
            Some(Ok(false)) => true,
            Some(Ok(true)) => {
                *slot = None;
                false
            }
            Some(Err(_)) => true,
            None => false,
        }
    }

    fn child_exited(&self) -> bool {
        let mut slot = self.child_slot();
        let exited = matches!(
            slot.as_mut().map(|child| child.reap_if_exited()),
            Some(Ok(true))
        );
        if exited {
            *slot = None;
        }
        exited
    }

    async fn reap_child(&self) {
        let Some(mut child) = self.take_child() else {
            return;
        };
        let reaped = tokio::task::spawn_blocking(move || child.reap());
        // Bounded even here. A daemon that answered "stopping" and then never
        // exited would otherwise hold the quit open on the one path that
        // believed it had gone quietly.
        if timeout(FORCED_STOP_BUDGET, reaped).await.is_err() {
            tracing::warn!("the background service did not finish exiting");
        }
    }

    fn take_child(&self) -> Option<Box<dyn ChildProcess>> {
        self.child_slot().take()
    }

    fn child_slot(&self) -> std::sync::MutexGuard<'_, Option<Box<dyn ChildProcess>>> {
        self.child
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
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
    async fn shutdown_injected<B: ServiceBackend>(&self, backend: &B) {
        let _lifecycle = self.lifecycle.lock().await;
        self.shutdown_locked(backend).await;
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
    /// Belt to the `RunEvent::Exit` hook's braces. A panic on the main thread
    /// unwinds past that hook, and an orphaned daemon is the state ADR-0004
    /// exists to prevent.
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
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
        fn reap_if_exited(&mut self) -> std::io::Result<bool> {
            let exited = self.probe.exited.load(Ordering::SeqCst);
            if exited {
                assert_eq!(self.probe.reaps.fetch_add(1, Ordering::SeqCst), 0);
            }
            Ok(exited)
        }

        fn kill(&mut self) -> std::io::Result<()> {
            self.probe.kills.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        fn reap(&mut self) -> std::io::Result<()> {
            assert_eq!(self.probe.reaps.fetch_add(1, Ordering::SeqCst), 0);
            Ok(())
        }
    }

    /// A child that refuses to be terminated and then blocks in `wait`, which
    /// is what an unkillable process looks like from this side.
    struct StuckChild {
        probe: Arc<ChildProbe>,
        release: Mutex<Option<std::sync::mpsc::Receiver<()>>>,
    }

    impl ChildProcess for StuckChild {
        fn reap_if_exited(&mut self) -> std::io::Result<bool> {
            Ok(false)
        }

        fn kill(&mut self) -> std::io::Result<()> {
            self.probe.kills.fetch_add(1, Ordering::SeqCst);
            Err(std::io::Error::other("terminate refused"))
        }

        fn reap(&mut self) -> std::io::Result<()> {
            let blocked = self.release.lock().unwrap().take().expect("one reap");
            let _ = blocked.recv();
            self.probe.reaps.fetch_add(1, Ordering::SeqCst);
            Ok(())
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
        sup.stop();
        assert_eq!(child.reaps.load(Ordering::SeqCst), 1);
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

        let shutdown = sup.shutdown_injected(&backend);
        tokio::pin!(shutdown);
        assert!(matches!(futures_util::poll!(&mut shutdown), Poll::Pending));
        backend.shutdown_gate.resume();

        let (restart, ()) = tokio::join!(restart, shutdown);
        assert!(restart.unwrap().is_live());
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

        sup.shutdown_injected(&backend).await;

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

    #[test]
    fn stopping_with_no_child_is_a_no_op() {
        Supervisor::default().stop();
    }

    /// Every sentence this module can show a user, checked in one place.
    #[test]
    fn no_message_names_a_path() {
        for message in [MSG_NOT_INSTALLED, MSG_START_FAILED, MSG_NEVER_READY] {
            assert!(!message.contains('/'), "{message}");
            assert!(message.ends_with('.'), "{message}");
            // "daemon" is a developer word; the user-facing name is
            // "background service" (bdac.34/36).
            assert!(!message.contains("daemon"), "{message}");
        }
    }

    /// Quit must end, whatever the daemon is doing.
    ///
    /// The gate here holds `stop_service` open forever, which is what a daemon
    /// wedged mid-shutdown looks like from this side. Before the budget the
    /// app's `RunEvent::Exit` hook waited on it and the window never closed;
    /// now the wait ends and the child is stopped outright.
    #[tokio::test(start_paused = true)]
    async fn a_daemon_that_will_not_stop_still_lets_the_app_quit() {
        let sup = Supervisor::default();
        let backend = ControlledBackend::new(true);
        let child = Arc::new(ChildProbe::default());
        *sup.child_slot() = Some(Box::new(FakeChild {
            probe: Arc::clone(&child),
        }));
        // Armed and never resumed: the handler is entered and does not return.
        backend.shutdown_gate.arm();

        sup.shutdown_injected(&backend).await;

        assert_eq!(
            child.kills.load(Ordering::SeqCst),
            1,
            "quit waited on the daemon instead of falling back"
        );
        assert_eq!(child.reaps.load(Ordering::SeqCst), 1);
        assert!(sup.take_child().is_none());
    }

    /// The kill/reap fallback is inside the budget, not after it.
    ///
    /// The gate holds `stop_service` open forever and the child refuses both
    /// `kill` and a prompt `reap`, which is the shape of a wedged daemon on a
    /// loaded machine. Before this, `stop()` ran outside the timeout: quit
    /// spent the whole budget being polite and then blocked in `wait` with no
    /// bound at all, so the window never closed.
    #[tokio::test(start_paused = true)]
    async fn a_child_that_will_not_die_still_ends_the_quit_inside_the_budget() {
        let sup = Supervisor::default();
        let backend = ControlledBackend::new(true);
        let child = Arc::new(ChildProbe::default());
        let (release, blocked) = std::sync::mpsc::channel::<()>();
        // Released from outside the runtime, so the blocked reap ends whatever
        // this test does — a `wait` left blocked forever would hang the
        // runtime's own shutdown rather than reporting a failure.
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(500));
            let _ = release.send(());
        });
        *sup.child_slot() = Some(Box::new(StuckChild {
            probe: Arc::clone(&child),
            release: Mutex::new(Some(blocked)),
        }));
        backend.shutdown_gate.arm();

        let started = tokio::time::Instant::now();
        sup.shutdown_injected(&backend).await;
        let elapsed = started.elapsed();

        assert_eq!(
            child.kills.load(Ordering::SeqCst),
            1,
            "kill was never tried"
        );
        assert!(
            elapsed <= SHUTDOWN_BUDGET,
            "quit took {elapsed:?}, over its {SHUTDOWN_BUDGET:?} budget"
        );
        assert!(sup.take_child().is_none(), "the child was handed back");
    }

    /// The fallback is a fallback, not the path. A daemon that stops when asked
    /// is never killed, because a killed daemon costs a WAL recovery on the
    /// next start.
    #[tokio::test(start_paused = true)]
    async fn a_daemon_that_stops_when_asked_is_not_killed() {
        let sup = Supervisor::default();
        let backend = ControlledBackend::new(true);
        let child = Arc::new(ChildProbe::default());
        *sup.child_slot() = Some(Box::new(FakeChild {
            probe: Arc::clone(&child),
        }));

        sup.shutdown_injected(&backend).await;

        assert_eq!(child.kills.load(Ordering::SeqCst), 0);
        assert_eq!(child.reaps.load(Ordering::SeqCst), 1);
    }
}
