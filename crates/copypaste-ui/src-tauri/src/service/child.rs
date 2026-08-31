//! The started daemon as a handle, and how it is ended.
//!
//! Separate from the supervisor because the supervisor decides *when* to stop
//! the service and this decides what stopping one costs. The trait is also the
//! seam every lifecycle test drives, so a fake child never has to be a process.

use std::process::Child;

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

    pub(super) fn is_success(&self) -> bool {
        self.code == Some(0)
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

pub(super) trait ChildProcess: Send {
    fn state(&mut self) -> std::io::Result<ChildState>;
    fn reap(&mut self) -> std::io::Result<ChildExitCode>;
}

impl ChildProcess for Child {
    fn state(&mut self) -> std::io::Result<ChildState> {
        self.try_wait().map(|status| match status {
            Some(status) => ChildState::Exited(ChildExitCode::from_status(status)),
            None => ChildState::Running,
        })
    }

    fn reap(&mut self) -> std::io::Result<ChildExitCode> {
        self.wait().map(ChildExitCode::from_status)
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
