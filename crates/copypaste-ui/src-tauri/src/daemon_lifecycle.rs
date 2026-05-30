//! Daemon UPGRADE/RESTART lifecycle helpers exposed to the frontend.
//!
//! After an in-place upgrade the OLD daemon keeps running and holding the IPC
//! socket, so the freshly-installed UI reaches stale code (the symptom: an
//! "unknown method" error for a verb only the new daemon knows). These Tauri
//! commands let the UI:
//!
//!   * report the app's own build version (`app_version`) so it can compare it
//!     against the daemon's reported `build_version` and detect a stale daemon;
//!   * force the running daemon to the freshly-installed binary
//!     (`restart_daemon`) via `launchctl kickstart -k`, with a
//!     `bootout`+`bootstrap` fallback for a daemon that is degraded /
//!     unresponsive or whose job is not currently bootstrapped.
//!
//! The restart path talks to `launchctl` (not the daemon IPC), so it works even
//! when the daemon is wedged and cannot answer IPC.

/// LaunchAgent label, matching `packaging/macos/com.copypaste.daemon.plist`.
#[cfg(target_os = "macos")]
const LAUNCHD_LABEL: &str = "com.copypaste.daemon";

/// The app's own build version (the crate version, e.g. `"0.5.2"`).
///
/// The frontend compares this against the daemon's reported `build_version`
/// (which is `"<crate-version>+<git-sha>"`) by semver prefix: a different
/// prefix means a stale daemon survived an upgrade and should be restarted.
#[tauri::command]
pub fn app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Restart the daemon so the freshly-installed binary takes over.
///
/// On macOS this uses `launchctl kickstart -k gui/<uid>/<label>`, which
/// terminates the current job instance and starts a new one from the plist's
/// `ProgramArguments` (the on-disk binary). When the job is not currently
/// bootstrapped (e.g. a prior `bootout`), `kickstart` fails; we then fall back
/// to `bootout` (best-effort) + `bootstrap` of the user's installed plist.
///
/// Because everything goes through `launchctl`, this works even when the daemon
/// is degraded or unresponsive on the IPC socket.
///
/// Returns `Ok(())` on success or a human-readable error string the UI surfaces
/// loudly.
#[tauri::command]
pub fn restart_daemon() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        restart_daemon_macos()
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("Restarting the daemon from the app is only supported on macOS.".to_string())
    }
}

#[cfg(target_os = "macos")]
fn restart_daemon_macos() -> Result<(), String> {
    use std::process::Command;

    let uid = current_uid()?;
    let service_target = format!("gui/{uid}/{LAUNCHD_LABEL}");

    // Primary path: kickstart -k kills the running instance and relaunches it
    // from the plist's ProgramArguments (the on-disk binary).
    let kick = Command::new("/bin/launchctl")
        .args(["kickstart", "-k", &service_target])
        .output();

    match kick {
        Ok(out) if out.status.success() => {
            tracing::info!("restart_daemon: kickstarted {service_target}");
            return Ok(());
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            tracing::warn!(
                "restart_daemon: kickstart {service_target} failed ({}): {} — \
                 falling back to bootout+bootstrap",
                out.status,
                stderr.trim()
            );
        }
        Err(e) => {
            return Err(format!("failed to invoke launchctl kickstart: {e}"));
        }
    }

    // Fallback: the job may not be bootstrapped (e.g. after a prior bootout, or
    // a fresh install where the plist exists but was never loaded). Bootout
    // best-effort (ignore failure — it just means it wasn't loaded), then
    // bootstrap the installed plist.
    let domain = format!("gui/{uid}");
    let _ = Command::new("/bin/launchctl")
        .args(["bootout", &service_target])
        .output();

    let plist = installed_plist_path()
        .ok_or_else(|| "cannot resolve LaunchAgents plist path (HOME unset?)".to_string())?;
    if !plist.exists() {
        return Err(format!(
            "daemon LaunchAgent plist not found at {} — reinstall CopyPaste to restore it",
            plist.display()
        ));
    }

    // Clearing any stale `disabled` override before bootstrap mirrors the
    // installer (`scripts/launchd/install-agent.sh`): a prior disable would
    // make bootstrap fail with "Input/output error".
    let _ = Command::new("/bin/launchctl")
        .args(["enable", &service_target])
        .output();

    let boot = Command::new("/bin/launchctl")
        .args(["bootstrap", &domain, &plist.to_string_lossy()])
        .output()
        .map_err(|e| format!("failed to invoke launchctl bootstrap: {e}"))?;

    if boot.status.success() {
        tracing::info!(
            "restart_daemon: bootstrapped {service_target} from {}",
            plist.display()
        );
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&boot.stderr);
        Err(format!(
            "launchctl bootstrap failed ({}): {}",
            boot.status,
            stderr.trim()
        ))
    }
}

/// Resolve the current numeric UID via `id -u` (no extra crate dependency —
/// the restart path already shells out to `launchctl`). Matches how the
/// LaunchAgent installer and the Cask postflight derive the GUI domain.
#[cfg(target_os = "macos")]
fn current_uid() -> Result<u32, String> {
    let out = std::process::Command::new("/usr/bin/id")
        .arg("-u")
        .output()
        .map_err(|e| format!("failed to run `id -u`: {e}"))?;
    if !out.status.success() {
        return Err(format!("`id -u` exited with {}", out.status));
    }
    String::from_utf8_lossy(&out.stdout)
        .trim()
        .parse::<u32>()
        .map_err(|e| format!("could not parse uid from `id -u`: {e}"))
}

/// Path to the user's installed LaunchAgent plist
/// (`~/Library/LaunchAgents/com.copypaste.daemon.plist`).
#[cfg(target_os = "macos")]
fn installed_plist_path() -> Option<std::path::PathBuf> {
    let home = home::home_dir()?;
    Some(
        home.join("Library/LaunchAgents")
            .join(format!("{LAUNCHD_LABEL}.plist")),
    )
}
