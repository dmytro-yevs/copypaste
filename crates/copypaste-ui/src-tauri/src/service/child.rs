//! The started daemon as a handle, and how it is ended.
//!
//! Separate from the supervisor because the supervisor decides *when* to stop
//! the service and this decides what stopping one costs. The trait is also the
//! seam every lifecycle test drives, so a fake child never has to be a process.

use std::process::Child;
use std::time::Duration;

pub(super) enum ChildState {
    Running,
    Exited(ChildExitCode),
}

pub(super) struct ChildExitCode {
    #[cfg(windows)]
    code: Option<u32>,
    #[cfg(not(windows))]
    code: Option<i32>,
}

impl ChildExitCode {
    pub(super) fn from_status(status: std::process::ExitStatus) -> Self {
        let code = status.code();
        #[cfg(windows)]
        let code = code.map(windows_exit_code);
        Self { code }
    }

    pub(super) fn log_value(&self) -> String {
        self.code
            .map_or_else(|| "unavailable".to_owned(), |value| value.to_string())
    }

    #[cfg(test)]
    pub(super) fn from_test_code(code: i32) -> Self {
        #[cfg(windows)]
        let code = Some(windows_exit_code(code));
        #[cfg(not(windows))]
        let code = Some(code);
        Self { code }
    }

    #[cfg(test)]
    pub(super) fn unavailable() -> Self {
        Self { code: None }
    }
}

#[cfg(any(windows, test))]
fn windows_exit_code(code: i32) -> u32 {
    u32::from_ne_bytes(code.to_ne_bytes())
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
            Some(status) => ChildState::Exited(ChildExitCode::from_status(status)),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_crash_status_keeps_all_dword_bits() {
        let signed = i32::from_ne_bytes(0xC000_0005_u32.to_ne_bytes());
        assert_eq!(windows_exit_code(signed), 0xC000_0005);
    }

    #[cfg(not(windows))]
    #[test]
    fn non_windows_exit_codes_keep_signed_semantics() {
        assert_eq!(ChildExitCode::from_test_code(-1).log_value(), "-1");
    }

    #[cfg(windows)]
    #[test]
    fn windows_exit_codes_are_logged_as_unsigned_dwords() {
        let signed = i32::from_ne_bytes(0xC000_0005_u32.to_ne_bytes());
        assert_eq!(
            ChildExitCode::from_test_code(signed).log_value(),
            "3221225477"
        );
    }
}
