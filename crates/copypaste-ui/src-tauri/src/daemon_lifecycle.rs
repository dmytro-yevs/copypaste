//! App-owned daemon lifecycle (product-owner decision, 2026-05-30).
//!
//! The desktop app OWNS the `copypaste-daemon` process:
//!
//! * **On app launch** ([`ensure_daemon_running`]) — the app starts the
//!   *bundled* daemon (`Contents/MacOS/copypaste-daemon`, sibling of the
//!   running UI binary) as a tracked child process. Any *old* daemon (e.g. a
//!   stale one left over after an in-place upgrade, or one started by a legacy
//!   LaunchAgent) is stopped FIRST so the freshly-installed binary always wins.
//!   This also fixes the stale-daemon-after-upgrade problem: a new app launch
//!   replaces whatever daemon was previously running.
//!
//! * **On app quit** ([`stop_daemon`]) — the daemon the app started is sent
//!   `SIGTERM` (graceful flush) and any leftover LaunchAgent is booted out so
//!   launchd cannot resurrect it behind the app's back. Closing just the WINDOW
//!   while the tray stays alive does NOT trigger this — only a full app exit
//!   (`RunEvent::Exit`, reached via the tray "Quit" item's `app.exit(0)` or
//!   process termination) does. A main-window close is intercepted and hides
//!   the window instead (see `lib.rs::setup_main_window`).
//!
//! ## LaunchAgent decision
//!
//! Because the app now owns start/stop, an always-on LaunchAgent
//! (`RunAtLoad` + `KeepAlive`) would CONFLICT with "daemon dies when the app
//! quits" — launchd would relaunch it. We therefore choose **option (i): the
//! app fully manages the daemon as a child process and does NOT rely on the
//! LaunchAgent.** To stay robust against a leftover loaded agent from a prior
//! install, the app proactively `launchctl bootout`s the label on both startup
//! (before spawning the fresh daemon) and on quit. See
//! `docs/adr/ADR-014-app-owned-daemon-lifecycle.md`.

use std::path::PathBuf;
use std::process::Child;
use std::sync::Mutex;
use std::time::Duration;

use tauri::Manager;

/// launchd label of the legacy LaunchAgent (kept only so we can boot out any
/// leftover instance from a prior install — we no longer install it ourselves).
const LAUNCHD_LABEL: &str = "com.copypaste.daemon";

/// Managed state holding the handle to the daemon child process the app
/// spawned (if any). `None` means the app has not (yet) started a daemon, or
/// spawning failed — in which case the degraded UI surfaces it.
#[derive(Default)]
pub struct DaemonChild(pub Mutex<Option<Child>>);

/// Resolve the daemon socket path. Mirrors
/// `copypaste-daemon::paths::socket_path` and `ipc::socket_path` so the probe
/// hits the same socket the IPC layer talks to.
fn socket_path() -> PathBuf {
    if let Ok(p) = std::env::var("COPYPASTE_SOCKET") {
        return PathBuf::from(p);
    }
    let Some(home) = home::home_dir() else {
        return PathBuf::from("/nonexistent/copypaste/daemon.sock");
    };
    #[cfg(target_os = "macos")]
    {
        home.join("Library/Application Support/CopyPaste/daemon.sock")
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
            return PathBuf::from(xdg).join("copypaste/daemon.sock");
        }
        home.join(".local/share/copypaste/daemon.sock")
    }
    #[cfg(not(unix))]
    {
        home.join("daemon.sock")
    }
}

/// Return `true` if the daemon is reachable on its IPC socket right now.
#[cfg(unix)]
fn daemon_reachable() -> bool {
    use std::os::unix::net::UnixStream;
    UnixStream::connect(socket_path()).is_ok()
}

#[cfg(not(unix))]
fn daemon_reachable() -> bool {
    // Non-unix desktop builds are not a shipping target; treat as reachable so
    // we never spawn a child we cannot manage.
    true
}

/// Resolve the bundled daemon binary path: a sibling of the currently-running
/// UI executable (`…/Contents/MacOS/copypaste-daemon`).
fn bundled_daemon_path() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    #[cfg(windows)]
    let name = "copypaste-daemon.exe";
    #[cfg(not(windows))]
    let name = "copypaste-daemon";
    let candidate = dir.join(name);
    candidate.exists().then_some(candidate)
}

// Minimal `getuid` shim so we don't pull in the `libc` crate for one call.
#[cfg(target_os = "macos")]
extern "C" {
    #[link_name = "getuid"]
    fn libc_getuid() -> u32;
}

/// Best-effort boot-out of any leftover LaunchAgent so launchd cannot revive
/// the daemon behind the app's back. No-op (and no error) when nothing is
/// loaded or on non-macOS.
fn bootout_launchagent() {
    #[cfg(target_os = "macos")]
    {
        // SAFETY: getuid is always safe and never fails.
        let uid = unsafe { libc_getuid() };
        let target = format!("gui/{uid}/{LAUNCHD_LABEL}");
        let _ = std::process::Command::new("launchctl")
            .args(["bootout", &target])
            .output();
    }
}

/// Best-effort `SIGTERM` to any *other* `copypaste-daemon` process (an old one
/// left over after an upgrade or started by a legacy agent). Uses `pkill` so we
/// don't need a PID file. Runs only on unix.
fn terminate_stray_daemons() {
    #[cfg(unix)]
    {
        // -TERM: graceful; the daemon flushes on SIGTERM. Match the binary name
        // so we don't hit unrelated processes.
        let _ = std::process::Command::new("pkill")
            .args(["-TERM", "-f", "copypaste-daemon"])
            .output();
    }
}

/// Ensure the daemon is running, starting the bundled one if needed.
///
/// Returns `Ok(true)` if the app started a daemon (handle stored in managed
/// state) and it became reachable, or an `Err` describing why the app could
/// not bring it up (surfaced LOUDLY so the degraded UI shows the failure —
/// never swallowed).
pub fn ensure_daemon_running(app: &tauri::AppHandle) -> Result<bool, String> {
    // 1. Reconcile the LaunchAgent first: boot out any leftover instance so it
    //    cannot fight the app-owned lifecycle.
    bootout_launchagent();

    // 2. Stop any pre-existing daemon so the freshly-installed binary always
    //    wins (fixes stale-daemon-after-upgrade). We do this even if the socket
    //    currently answers — the answering process may be the OLD binary.
    if daemon_reachable() {
        terminate_stray_daemons();
        // Give the old daemon a beat to release the socket before we rebind.
        std::thread::sleep(Duration::from_millis(250));
    }

    // 3. Spawn the fresh bundled daemon.
    let bin = bundled_daemon_path().ok_or_else(|| {
        "could not locate bundled copypaste-daemon next to the app executable".to_string()
    })?;

    let child = std::process::Command::new(&bin)
        .spawn()
        .map_err(|e| format!("failed to spawn daemon at {}: {e}", bin.display()))?;

    if let Some(state) = app.try_state::<DaemonChild>() {
        // Replace any previously-tracked child (shouldn't exist at startup, but
        // be defensive). Dropping the old handle does not kill it.
        let mut guard = state.0.lock().expect("DaemonChild mutex poisoned");
        *guard = Some(child);
    }

    // 4. Wait briefly for the socket to come up so the first UI queries
    //    succeed. If it never comes up, surface a LOUD error.
    for _ in 0..40 {
        if daemon_reachable() {
            tracing::info!("app-owned daemon is up (binary: {})", bin.display());
            return Ok(true);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    Err(format!(
        "started daemon at {} but it did not become reachable on {}",
        bin.display(),
        socket_path().display()
    ))
}

/// Stop the daemon the app started (called on full app exit).
///
/// Sends `SIGTERM` to the tracked child for a graceful flush, then boots out
/// any leftover LaunchAgent so launchd cannot resurrect it. Idempotent and
/// best-effort: a failure here must not block app exit.
pub fn stop_daemon(app: &tauri::AppHandle) {
    // Boot out the LaunchAgent FIRST so launchd won't relaunch the daemon the
    // instant we SIGTERM it.
    bootout_launchagent();

    let Some(state) = app.try_state::<DaemonChild>() else {
        return;
    };
    let mut guard = state.0.lock().expect("DaemonChild mutex poisoned");
    let Some(mut child) = guard.take() else {
        // We never started a daemon — nothing of ours to reap.
        return;
    };

    let pid = child.id();
    #[cfg(unix)]
    {
        // Graceful SIGTERM via `kill` so the daemon flushes WAL / Keychain
        // state before exiting (`Child::kill` would send SIGKILL).
        let _ = std::process::Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .output();
        // Reap so we don't leave a zombie; give it a moment, then force-kill.
        for _ in 0..40 {
            match child.try_wait() {
                Ok(Some(_)) => {
                    tracing::info!("app-owned daemon (pid {pid}) stopped on app exit");
                    return;
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(50)),
                Err(_) => break,
            }
        }
        let _ = child.kill();
        let _ = child.wait();
    }
    #[cfg(not(unix))]
    {
        let _ = child.kill();
        let _ = child.wait();
    }
    tracing::info!("app-owned daemon (pid {pid}) stopped on app exit");
}
