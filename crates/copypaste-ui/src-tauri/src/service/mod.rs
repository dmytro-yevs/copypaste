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
//! a message; every failure here is a fixed sentence (CLAUDE.md rule 4).
//!
//! # Android
//!
//! Compiled, and inert by construction rather than by `cfg`. The embedded
//! backend's `status` always succeeds, so [`Supervisor::state`] answers
//! `Running` and [`Supervisor::start`] returns without looking for a binary.
//! That keeps `crate::commands` free of `cfg`, which is what ADR-0002 asks for.

use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;

use backon::{ExponentialBuilder, Retryable};
use serde::Serialize;

use crate::backend::{Backend, BackendError, Result};

pub mod locate;
pub mod push;

/// The version this app expects the daemon to report. Both come from the
/// workspace version, so a mismatch means two different installs, not a
/// packaging slip.
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

const MSG_NOT_INSTALLED: &str = "This build of CopyPaste doesn't include the background service.";
const MSG_START_FAILED: &str = "The background service could not be started.";
const MSG_NEVER_READY: &str = "The background service started but didn't finish coming up.";
const MSG_NOT_OURS: &str =
    "Another copy of the background service is already running. Quit it, then try again.";

/// How long to wait for a freshly started daemon to answer `status`.
///
/// It opens SQLCipher and derives a key before it binds, so the first answer is
/// not instant. Ten seconds is long enough for a cold start on a slow disk and
/// short enough that a daemon which is never going to answer is reported rather
/// than waited on.
const READY_TIMEOUT: Duration = Duration::from_secs(10);

/// What the background service is doing, as the UI needs to see it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
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
#[derive(Debug, Default)]
pub struct Supervisor {
    child: Mutex<Option<Child>>,
}

impl Supervisor {
    /// What the service is doing right now.
    ///
    /// `Unreachable` is the only error that means "not running" — the daemon
    /// answers `status` before it is ready, so anything else came from a live
    /// process (manifest 04: `status` is exempt from the readiness gate).
    pub async fn state<B: Backend>(&self, backend: &B) -> ServiceState {
        match backend.status().await {
            Ok(status) => ServiceState::Running {
                matches_app: status.version == APP_VERSION,
                version: status.version,
                ours: self.holds_child(),
            },
            Err(BackendError::Unreachable) => {
                if locate::daemon_binary().is_some() {
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
        let state = self.state(backend).await;
        match state {
            ServiceState::NotInstalled => return Err(BackendError::Unsupported(MSG_NOT_INSTALLED)),
            ServiceState::Stopped => {}
            live => return Ok(live),
        }

        let binary = locate::daemon_binary().ok_or(BackendError::Unsupported(MSG_NOT_INSTALLED))?;
        self.spawn(&binary)?;
        self.await_ready(backend).await?;
        Ok(self.state(backend).await)
    }

    /// Stop what we started, then start again.
    ///
    /// Refuses when the live daemon is not ours: see ADR-0004 for why the app
    /// does not kill processes it did not start, and for the `Method::Shutdown`
    /// that will close the gap.
    pub async fn restart<B: Backend>(&self, backend: &B) -> Result<ServiceState> {
        let state = self.state(backend).await;
        if state.is_live() && !self.holds_child() {
            return Err(BackendError::Unsupported(MSG_NOT_OURS));
        }
        self.stop();
        self.start(backend).await
    }

    /// Stop the daemon this process started. Safe to call when there is none.
    ///
    /// `kill` is `SIGKILL` — `std::process::Child` sends nothing else. WAL mode
    /// makes that recoverable and the daemon clears its own stale socket on the
    /// next bind; ADR-0004 records it as a cost rather than a preference.
    pub fn stop(&self) {
        let Some(mut child) = self.take_child() else {
            return;
        };
        let _ = child.kill();
        // Reaped rather than left a zombie: the app may run for days after.
        let _ = child.wait();
    }

    fn spawn(&self, binary: &Path) -> Result<()> {
        let child = Command::new(binary)
            .arg("--foreground")
            // No inherited stdio. The daemon writes to `tracing`, and a child
            // holding the app's descriptors keeps them open past a crash.
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            // The `io::Error` names the binary's path, which is exactly the
            // disclosure rule 4 is about, so it is dropped rather than wrapped.
            .map_err(|_| BackendError::Internal(MSG_START_FAILED.into()))?;

        if let Ok(mut slot) = self.child.lock() {
            // Anything already here has been replaced by `restart`, which
            // stopped it first; dropping the handle would leak a zombie.
            if let Some(mut previous) = slot.replace(child) {
                let _ = previous.kill();
                let _ = previous.wait();
            }
        }
        Ok(())
    }

    /// Wait for the daemon to answer, or for it to die trying.
    ///
    /// `backon` rather than a sleep loop: the workspace already carries one
    /// retry implementation and v1 grew six (CLAUDE.md rule 1).
    async fn await_ready<B: Backend>(&self, backend: &B) -> Result<()> {
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
            backend.status().await.map(|_| ())
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

    /// Whether we hold a child that is still alive, reaping it if it is not.
    fn holds_child(&self) -> bool {
        let Ok(mut slot) = self.child.lock() else {
            return false;
        };
        match slot.as_mut().map(Child::try_wait) {
            Some(Ok(None)) => true,
            // Exited, or unknowable. Either way the handle is spent.
            Some(_) => {
                *slot = None;
                false
            }
            None => false,
        }
    }

    fn child_exited(&self) -> bool {
        let Ok(mut slot) = self.child.lock() else {
            return false;
        };
        matches!(slot.as_mut().map(Child::try_wait), Some(Ok(Some(_))))
    }

    fn take_child(&self) -> Option<Child> {
        self.child.lock().ok()?.take()
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

    /// ADR-0004's asymmetry. A live daemon we did not start is not ours to
    /// restart, and the refusal has to say so rather than silently doing
    /// nothing.
    #[tokio::test]
    async fn restarting_a_daemon_we_did_not_start_refuses() {
        let sup = Supervisor::default();
        let err = sup
            .restart(&FakeBackend::running("0.9.9"))
            .await
            .unwrap_err();
        assert!(
            matches!(err, BackendError::Unsupported(MSG_NOT_OURS)),
            "{err:?}"
        );
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
        for message in [
            MSG_NOT_INSTALLED,
            MSG_START_FAILED,
            MSG_NEVER_READY,
            MSG_NOT_OURS,
        ] {
            assert!(!message.contains('/'), "{message}");
            assert!(message.ends_with('.'), "{message}");
            // "daemon" is a developer word; the user-facing name is
            // "background service" (bdac.34/36).
            assert!(!message.contains("daemon"), "{message}");
        }
    }
}
