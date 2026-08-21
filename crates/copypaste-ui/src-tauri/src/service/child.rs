//! The started daemon as a handle, and how it is ended.
//!
//! Separate from the supervisor because the supervisor decides *when* to stop
//! the service and this decides what stopping one costs. The trait is also the
//! seam every lifecycle test drives, so a fake child never has to be a process.

use std::process::Child;
use std::time::Duration;

pub(super) enum ChildState {
    Running,
    Exited(Option<i32>),
}

/// The slice of [`super::SHUTDOWN_BUDGET`] held back for ending the child.
///
/// `kill` can fail and `reap` is a blocking wait with no timeout of its own. If
/// the graceful attempt were allowed to spend the whole budget, the fallback
/// would begin already out of time and quit would hang on the very step that
/// exists to stop it hanging.
pub(super) const FORCED_STOP_BUDGET: Duration = Duration::from_secs(3);

pub(super) trait ChildProcess: Send {
    fn state(&mut self) -> std::io::Result<ChildState>;
    fn kill(&mut self) -> std::io::Result<()>;
    fn reap(&mut self) -> std::io::Result<()>;
}

impl ChildProcess for Child {
    fn state(&mut self) -> std::io::Result<ChildState> {
        self.try_wait().map(|status| match status {
            Some(status) => ChildState::Exited(status.code()),
            None => ChildState::Running,
        })
    }

    fn kill(&mut self) -> std::io::Result<()> {
        Child::kill(self)
    }

    fn reap(&mut self) -> std::io::Result<()> {
        self.wait().map(|_| ())
    }
}

/// Kill the child and reap it, giving up on the reap at the budget.
///
/// Reaped rather than left a zombie: the app may run for days after. But a
/// `wait` that never returns is worse than a zombie, so it runs on a thread of
/// its own and this call stops waiting.
pub(super) fn end_child(mut child: Box<dyn ChildProcess>) {
    let _ = child.kill();
    let (done, reaped) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = child.reap();
        let _ = done.send(());
    });
    if reaped.recv_timeout(FORCED_STOP_BUDGET).is_err() {
        tracing::warn!("the background service could not be reaped");
    }
}
