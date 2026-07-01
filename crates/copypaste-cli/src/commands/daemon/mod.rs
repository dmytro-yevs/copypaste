//! `copypaste daemon` — manage the background daemon process.
//!
//! Platform support:
//!   - macOS:   `launchctl bootstrap gui/<uid> <plist>` / `launchctl bootout gui/<uid>/<label>`
//!   - Linux:   `systemctl --user` (FROZEN — wiring documented, returns clear error)
//!   - Windows: `sc.exe` (FUTURE — returns clear error)
//!
//! All shell-outs are wrapped through `CommandRunner` so unit tests can assert the
//! constructed argv without actually invoking `launchctl` on the host.
//!
//! ## Idempotency (beta hotfix)
//!
//! `start` and `install` are idempotent — re-running them when the daemon is
//! already loaded prints a friendly "already running" notice and exits 0 instead
//! of returning `launchctl bootstrap` error 5 ("Input/output error" / "already
//! loaded"). Similarly `stop` is a no-op when the daemon is not loaded.
//!
//! We also refuse to run `start` as root: `~/Library/LaunchAgents/` is a
//! per-user domain (`gui/<UID>`), so running with `sudo` (UID 0) tries to bootstrap
//! into `gui/0` where the plist isn't registered → launchctl error 125
//! ("Domain does not support specified action"). The fix is to run without sudo.
//!
//! ## Module layout
//!
//! - [`runner`]   — `CommandRunner` / `FsOps` traits + production impls.
//! - [`platform`] — macOS launchd functions; `unsupported_platform` for others.

mod platform;
mod runner;

// Re-export the testability traits so the test module can implement them for
// mock types without having to qualify the path.
pub(crate) use runner::{CommandRunner, FsOps};

use anyhow::Result;

/// Public subcommand entry point. Dispatches to platform-specific logic via the
/// default `SystemRunner` (which actually shells out).
pub fn run(action: DaemonAction) -> Result<()> {
    let mut runner = runner::SystemRunner;
    let mut fs = runner::SystemFs;
    dispatch(action, &mut runner, &mut fs)
}

/// User-selected daemon action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DaemonAction {
    Start,
    Stop,
    Restart,
    Install,
    Uninstall,
}

/// Internal dispatcher — generic over `CommandRunner` + `FsOps` for testability.
pub(crate) fn dispatch<R: CommandRunner, F: FsOps>(
    action: DaemonAction,
    runner: &mut R,
    fs: &mut F,
) -> Result<()> {
    if !cfg!(target_os = "macos") {
        return platform::unsupported_platform();
    }

    match action {
        DaemonAction::Start => platform::macos_start(runner, fs),
        DaemonAction::Stop => platform::macos_stop(runner),
        DaemonAction::Restart => {
            // bootout is allowed to fail (daemon may not be loaded)
            let _ = platform::macos_stop(runner);
            platform::macos_start(runner, fs)
        }
        DaemonAction::Install => platform::macos_install(runner, fs),
        DaemonAction::Uninstall => platform::macos_uninstall(runner, fs),
    }
}

// --------------------------------------------------------------------------------------------
// Tests
// --------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests;
