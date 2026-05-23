//! In-app updater using Homebrew Cask (v0.3 — no Sparkle, see ADR-012).
//!
//! Distribution policy for CopyPaste is Homebrew-only. Rather than embedding a
//! third-party update framework (Sparkle), this module reuses the user's
//! already-installed `brew` toolchain to:
//!
//! 1. Periodically (default: every 24h) run
//!    `brew outdated --cask copypaste --json=v2` and parse the result.
//! 2. Surface an [`UpdateStatus`] the UI layer can show as a banner /
//!    tray-menu badge.
//! 3. Apply the upgrade on user confirmation via
//!    `brew upgrade --cask copypaste` followed by `launchctl bootout
//!    gui/$UID/com.copypaste.daemon` so launchd respawns the daemon from
//!    the new install.
//!
//! All shell invocations go through the [`CommandRunner`] trait so the unit
//! tests can drive deterministic mock output without touching the real
//! `brew` binary.
//!
//! ### Failure modes (non-panicking)
//!
//! * `brew` not on `PATH`  → [`UpdateStatus::BrewNotInstalled`]
//! * `brew outdated` exits non-zero → [`UpdateStatus::CheckFailed`]
//! * JSON parse fails → [`UpdateStatus::CheckFailed`]
//! * Empty `casks` array → [`UpdateStatus::UpToDate`]

use std::io;
use std::process::{Command, Output};
use std::time::Duration;

/// Default interval between background update checks (24 hours).
pub const CHECK_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

/// Cask name as published in the Homebrew tap (see Brewfile / cask formula).
pub const CASK_NAME: &str = "copypaste";

/// The launchd label for the CopyPaste daemon. Used by [`apply_update`] to
/// boot the running daemon out so launchd respawns it from the newly
/// upgraded bundle.
pub const DAEMON_LAUNCHD_LABEL: &str = "com.copypaste.daemon";

/// Version pair surfaced when an update is available.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateInfo {
    pub current_version: String,
    pub latest_version: String,
}

/// Result of a single update probe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateStatus {
    /// Installed cask matches the tap's published version.
    UpToDate,
    /// A newer version is published.
    UpdateAvailable(UpdateInfo),
    /// `brew` is not available on `PATH`. The user likely installed the
    /// daemon by some other means (cargo install, manual unzip, …); the
    /// in-app updater silently degrades to a no-op.
    BrewNotInstalled,
    /// The probe itself failed (network, JSON parse, non-zero exit, …).
    /// The UI should log this and try again next interval.
    CheckFailed(String),
}

/// Thin shim around [`std::process::Command`] so tests can inject a stub.
pub trait CommandRunner: Send + Sync {
    fn run(&self, cmd: &str, args: &[&str]) -> io::Result<Output>;
}

/// Production [`CommandRunner`] that shells out via [`std::process::Command`].
pub struct SystemRunner;

impl CommandRunner for SystemRunner {
    fn run(&self, cmd: &str, args: &[&str]) -> io::Result<Output> {
        Command::new(cmd).args(args).output()
    }
}

/// Probe Homebrew for an outstanding upgrade to the `copypaste` cask.
///
/// Returns [`UpdateStatus::BrewNotInstalled`] if `brew` is missing — the
/// caller should treat that as "auto-update unavailable", not as a hard
/// error.
pub fn check_for_update(runner: &dyn CommandRunner) -> UpdateStatus {
    match runner.run("brew", &["outdated", "--cask", CASK_NAME, "--json=v2"]) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => UpdateStatus::BrewNotInstalled,
        Err(e) => UpdateStatus::CheckFailed(format!("spawn failed: {e}")),
        Ok(output) => {
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                // `brew outdated` returns 1 when nothing is outdated in some
                // versions, but with `--json` it should always be 0. Treat
                // empty JSON below as authoritative; only bubble up real
                // errors here.
                if stderr.contains("No such file or directory") {
                    return UpdateStatus::BrewNotInstalled;
                }
                return UpdateStatus::CheckFailed(format!(
                    "brew outdated exit={:?}: {}",
                    output.status.code(),
                    stderr.trim()
                ));
            }
            let stdout = String::from_utf8_lossy(&output.stdout);
            parse_outdated_json(&stdout)
        }
    }
}

/// Parse the `--json=v2` payload from `brew outdated --cask copypaste`.
///
/// Shape:
/// ```json
/// {
///   "formulae": [],
///   "casks": [
///     { "name": "copypaste",
///       "installed_versions": ["0.3.0-beta.1"],
///       "current_version": "0.3.0" }
///   ]
/// }
/// ```
///
/// When `casks` is empty the cask is up to date.
fn parse_outdated_json(stdout: &str) -> UpdateStatus {
    let value: serde_json::Value = match serde_json::from_str(stdout.trim()) {
        Ok(v) => v,
        Err(e) => return UpdateStatus::CheckFailed(format!("json parse: {e}")),
    };

    let casks = match value.get("casks").and_then(|c| c.as_array()) {
        Some(arr) => arr,
        // Some legacy `brew outdated --json=v1` payloads are a bare array.
        None => match value.as_array() {
            Some(arr) => arr,
            None => return UpdateStatus::CheckFailed("missing `casks` field".into()),
        },
    };

    if casks.is_empty() {
        return UpdateStatus::UpToDate;
    }

    let cask = &casks[0];
    let current = cask
        .get("installed_versions")
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())
        .and_then(|v| v.as_str())
        .or_else(|| cask.get("installed_version").and_then(|v| v.as_str()))
        .unwrap_or("unknown")
        .to_string();
    let latest = cask
        .get("current_version")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    // Defensive: if brew reports a row but the strings are identical, treat
    // as up-to-date rather than nagging the user with a no-op upgrade.
    if current == latest {
        return UpdateStatus::UpToDate;
    }

    UpdateStatus::UpdateAvailable(UpdateInfo {
        current_version: current,
        latest_version: latest,
    })
}

/// Apply the update by running `brew upgrade --cask copypaste`, then
/// asking launchd to boot out the running daemon so it respawns from the
/// new bundle (launchd's `RunAtLoad=true` plist handles the relaunch).
///
/// Returns the upgraded version string on success, or a human-readable
/// error otherwise.
pub fn apply_update(runner: &dyn CommandRunner) -> Result<(), String> {
    // Step 1 — upgrade the cask.
    let upgrade = runner
        .run("brew", &["upgrade", "--cask", CASK_NAME])
        .map_err(|e| {
            if e.kind() == io::ErrorKind::NotFound {
                "brew not found on PATH".to_string()
            } else {
                format!("spawn failed: {e}")
            }
        })?;
    if !upgrade.status.success() {
        return Err(format!(
            "brew upgrade failed (exit={:?}): {}",
            upgrade.status.code(),
            String::from_utf8_lossy(&upgrade.stderr).trim()
        ));
    }

    // Step 2 — restart the daemon so the new binary is loaded.
    // We deliberately ignore the bootout exit status: if the daemon was
    // already gone, that's still a success from the user's POV.
    let uid = nix_uid();
    let target = format!("gui/{uid}/{DAEMON_LAUNCHD_LABEL}");
    let _ = runner.run("launchctl", &["bootout", &target]);

    Ok(())
}

/// Fetch the effective UID via `id -u`. We avoid the `nix` / `libc` crates
/// here to keep the dependency footprint of the UI crate minimal — this
/// only runs on macOS and at human-pace (~1 click per upgrade).
fn nix_uid() -> String {
    Command::new("id")
        .arg("-u")
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "501".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::process::ExitStatusExt;
    use std::process::ExitStatus;
    use std::sync::Mutex;

    /// Test double: replays canned [`Output`]s in FIFO order and records
    /// the (cmd, args) tuples it was invoked with.
    struct MockRunner {
        responses: Mutex<Vec<io::Result<Output>>>,
        calls: Mutex<Vec<(String, Vec<String>)>>,
    }

    impl MockRunner {
        fn new(responses: Vec<io::Result<Output>>) -> Self {
            Self {
                responses: Mutex::new(responses),
                calls: Mutex::new(Vec::new()),
            }
        }

        fn calls(&self) -> Vec<(String, Vec<String>)> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl CommandRunner for MockRunner {
        fn run(&self, cmd: &str, args: &[&str]) -> io::Result<Output> {
            self.calls
                .lock()
                .unwrap()
                .push((cmd.to_string(), args.iter().map(|s| s.to_string()).collect()));
            let mut r = self.responses.lock().unwrap();
            if r.is_empty() {
                return Err(io::Error::other("no more responses"));
            }
            r.remove(0)
        }
    }

    fn ok_output(stdout: &str) -> io::Result<Output> {
        Ok(Output {
            status: ExitStatus::from_raw(0),
            stdout: stdout.as_bytes().to_vec(),
            stderr: Vec::new(),
        })
    }

    fn err_output(code: i32, stderr: &str) -> io::Result<Output> {
        // ExitStatus::from_raw expects wait(2) status: shift the exit code
        // left by 8 so WEXITSTATUS recovers it correctly.
        Ok(Output {
            status: ExitStatus::from_raw(code << 8),
            stdout: Vec::new(),
            stderr: stderr.as_bytes().to_vec(),
        })
    }

    #[test]
    fn parses_brew_outdated_json_returns_update_available() {
        let json = r#"{
            "formulae": [],
            "casks": [
              {
                "name": "copypaste",
                "installed_versions": ["0.3.0-beta.1"],
                "current_version": "0.3.0"
              }
            ]
        }"#;
        let runner = MockRunner::new(vec![ok_output(json)]);
        let status = check_for_update(&runner);
        assert_eq!(
            status,
            UpdateStatus::UpdateAvailable(UpdateInfo {
                current_version: "0.3.0-beta.1".to_string(),
                latest_version: "0.3.0".to_string(),
            })
        );
        // Verify exact brew invocation.
        let calls = runner.calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "brew");
        assert_eq!(calls[0].1, vec!["outdated", "--cask", "copypaste", "--json=v2"]);
    }

    #[test]
    fn parses_brew_outdated_empty_returns_up_to_date() {
        let json = r#"{ "formulae": [], "casks": [] }"#;
        let runner = MockRunner::new(vec![ok_output(json)]);
        assert_eq!(check_for_update(&runner), UpdateStatus::UpToDate);
    }

    #[test]
    fn brew_command_not_found_returns_brew_not_installed() {
        let runner = MockRunner::new(vec![Err(io::Error::new(
            io::ErrorKind::NotFound,
            "no such file",
        ))]);
        assert_eq!(check_for_update(&runner), UpdateStatus::BrewNotInstalled);
    }

    #[test]
    fn nonzero_exit_returns_check_failed() {
        let runner = MockRunner::new(vec![err_output(1, "Error: Unknown cask: copypaste")]);
        match check_for_update(&runner) {
            UpdateStatus::CheckFailed(msg) => {
                assert!(msg.contains("brew outdated"), "msg = {msg}");
                assert!(msg.contains("Unknown cask"), "msg = {msg}");
            }
            other => panic!("expected CheckFailed, got {other:?}"),
        }
    }

    #[test]
    fn malformed_json_returns_check_failed() {
        let runner = MockRunner::new(vec![ok_output("not json {{{")]);
        match check_for_update(&runner) {
            UpdateStatus::CheckFailed(msg) => assert!(msg.contains("json parse"), "msg = {msg}"),
            other => panic!("expected CheckFailed, got {other:?}"),
        }
    }

    #[test]
    fn version_comparison_handles_semver_with_prerelease() {
        // installed = pre-release, latest = stable release → update.
        let json = r#"{ "casks": [
            { "name": "copypaste",
              "installed_versions": ["0.3.0-beta.4"],
              "current_version": "0.3.0" }
        ] }"#;
        let runner = MockRunner::new(vec![ok_output(json)]);
        assert!(matches!(
            check_for_update(&runner),
            UpdateStatus::UpdateAvailable(_)
        ));

        // Identical strings → up to date (defensive: brew shouldn't list
        // these, but if it does we must not nag).
        let json2 = r#"{ "casks": [
            { "name": "copypaste",
              "installed_versions": ["0.3.0"],
              "current_version": "0.3.0" }
        ] }"#;
        let runner2 = MockRunner::new(vec![ok_output(json2)]);
        assert_eq!(check_for_update(&runner2), UpdateStatus::UpToDate);
    }

    #[test]
    fn apply_update_invokes_brew_upgrade_then_bootout() {
        let runner = MockRunner::new(vec![
            ok_output(""), // brew upgrade
            ok_output(""), // launchctl bootout
        ]);
        apply_update(&runner).expect("apply_update succeeds");

        let calls = runner.calls();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].0, "brew");
        assert_eq!(calls[0].1, vec!["upgrade", "--cask", "copypaste"]);
        assert_eq!(calls[1].0, "launchctl");
        // launchctl args = ["bootout", "gui/<uid>/com.copypaste.daemon"]
        assert_eq!(calls[1].1.len(), 2);
        assert_eq!(calls[1].1[0], "bootout");
        assert!(
            calls[1].1[1].starts_with("gui/")
                && calls[1].1[1].ends_with("/com.copypaste.daemon"),
            "unexpected bootout target: {}",
            calls[1].1[1]
        );
    }

    #[test]
    fn apply_update_propagates_brew_failure() {
        let runner = MockRunner::new(vec![err_output(1, "Error: Cask 'copypaste' is not installed")]);
        let err = apply_update(&runner).expect_err("should fail");
        assert!(err.contains("brew upgrade failed"), "err = {err}");
    }

    #[test]
    fn apply_update_reports_brew_missing() {
        let runner = MockRunner::new(vec![Err(io::Error::new(
            io::ErrorKind::NotFound,
            "no such file",
        ))]);
        let err = apply_update(&runner).expect_err("should fail");
        assert_eq!(err, "brew not found on PATH");
    }
}
