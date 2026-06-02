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
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::Manager;

/// launchd label of the legacy LaunchAgent (kept only so we can boot out any
/// leftover instance from a prior install — we no longer install it ourselves).
const LAUNCHD_LABEL: &str = "com.copypaste.daemon";

/// How long to poll for socket release after sending SIGTERM, before giving up.
const SOCKET_RELEASE_TIMEOUT: Duration = Duration::from_secs(3);
/// Polling interval while waiting for socket release.
const SOCKET_POLL_INTERVAL: Duration = Duration::from_millis(50);
/// Maximum wait for the daemon socket to become reachable after spawn.
const SOCKET_READY_TIMEOUT: Duration = Duration::from_millis(2000);

/// Managed state holding the handle to the daemon child process the app
/// spawned (if any). `None` means the app has not (yet) started a daemon, or
/// spawning failed — in which case the degraded UI surfaces it.
#[derive(Default)]
pub struct DaemonChild(pub Mutex<Option<Child>>);

/// Last spawn error, surfaced to the frontend via [`get_daemon_error`].
/// `None` means no error (or daemon started successfully).
#[derive(Default)]
pub struct DaemonSpawnError(pub Mutex<Option<String>>);

/// Lifecycle generation counter. [`restart_daemon`] increments this (with
/// `SeqCst`) **before** killing the old child and storing a new one.
/// [`ensure_daemon_running`] snapshots the counter before its socket-ready poll
/// loop and rechecks it afterwards; if the generation changed, the child it
/// spawned has been superseded by a restart, so it skips writing a false error.
///
/// ### Why this can't deadlock
/// The startup thread never holds the `DaemonChild` mutex across the blocking
/// socket-wait loop — it releases the mutex immediately after storing the child
/// (line `*guard = Some(child)`).  The generation load/compare is a pure atomic
/// operation that cannot block or require any mutex.  `restart_daemon` only
/// holds the `DaemonChild` mutex briefly (inside `stop_tracked_child`) and
/// releases it before calling `ensure_daemon_running`, so there is no lock
/// ordering inversion.
#[derive(Default)]
pub struct DaemonLifecycleGen(pub AtomicU64);

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

/// Return `true` if the socket *file* exists (regardless of whether it answers).
fn socket_file_exists() -> bool {
    socket_path().exists()
}

/// Minimal `status` reply we care about for eviction decisions.
/// We use a raw JSON probe to avoid pulling in daemon types here.
struct StatusReply {
    pid: Option<u32>,
    build_version: Option<String>,
    degraded: bool,
}

/// Probe the daemon's `status` IPC method synchronously over the Unix socket.
/// Returns `None` if the socket is unreachable or the reply cannot be parsed.
#[cfg(unix)]
fn probe_status() -> Option<StatusReply> {
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::UnixStream;

    let mut stream = UnixStream::connect(socket_path()).ok()?;
    stream
        .set_read_timeout(Some(Duration::from_millis(500)))
        .ok()?;
    // Newline-delimited JSON-RPC used by copypaste-daemon.
    let req = r#"{"jsonrpc":"2.0","id":1,"method":"status","params":null}"#;
    writeln!(stream, "{req}").ok()?;
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;
    let v: serde_json::Value = serde_json::from_str(line.trim()).ok()?;
    let result = v.get("result")?;
    Some(StatusReply {
        pid: result.get("pid").and_then(|p| p.as_u64()).map(|p| p as u32),
        build_version: result
            .get("build_version")
            .and_then(|bv| bv.as_str())
            .map(|s| s.to_string()),
        degraded: result
            .get("degraded")
            .and_then(|d| d.as_bool())
            .unwrap_or(false),
    })
}

#[cfg(not(unix))]
fn probe_status() -> Option<StatusReply> {
    None
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

// SAFETY: getuid() is an async-signal-safe, always-succeeding POSIX function
// with no side effects. Calling it via FFI is safe.
#[cfg(target_os = "macos")]
fn current_uid_libc() -> u32 {
    extern "C" {
        fn getuid() -> u32;
    }
    unsafe { getuid() }
}

/// Best-effort boot-out of any leftover LaunchAgent so launchd cannot revive
/// the daemon behind the app's back. No-op (and no error) when nothing is
/// loaded or on non-macOS.
fn bootout_launchagent() {
    #[cfg(target_os = "macos")]
    {
        let uid = current_uid_libc();
        let target = format!("gui/{uid}/{LAUNCHD_LABEL}");
        let _ = std::process::Command::new("launchctl")
            .args(["bootout", &target])
            .output();
    }
}

/// Send SIGTERM to a specific PID. Refuses to signal pid 0 or 1 (safety guard).
#[cfg(unix)]
fn sigterm_pid(pid: u32) {
    // Never signal init (1) or the broadcast pid (0).
    if pid <= 1 {
        tracing::warn!("sigterm_pid: refusing to signal pid {pid}");
        return;
    }
    // Also refuse to signal our own process.
    if pid == std::process::id() {
        tracing::warn!("sigterm_pid: refusing to signal self");
        return;
    }
    let _ = std::process::Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .output();
}

/// Terminate any stale daemon using the exact binary name (not substring match).
/// Falls back to `pkill -x` (exact name) only when we do not have a live PID.
#[cfg(unix)]
fn terminate_stray_by_name() {
    // -x: exact name match — never a substring match like -f.
    let _ = std::process::Command::new("pkill")
        .args(["-TERM", "-x", "copypaste-daemon"])
        .output();
}

/// Wait (up to `timeout`) for the socket to be gone (released by dying process).
/// Polls every `SOCKET_POLL_INTERVAL`. Returns when the socket is gone or on timeout.
fn wait_for_socket_released(timeout: Duration) {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if !socket_file_exists() && !daemon_reachable() {
            return;
        }
        std::thread::sleep(SOCKET_POLL_INTERVAL);
    }
    tracing::warn!(
        "wait_for_socket_released: timed out after {}ms; socket may still be held",
        timeout.as_millis()
    );
}

/// Evict any pre-existing daemon so the freshly-installed binary always wins.
///
/// Strategy:
/// 1. If the socket answers `status`, use the reported PID for targeted SIGTERM
///    and also inspect version/degraded for logging.
/// 2. If the socket *file* exists but doesn't answer, treat as stale: remove
///    the file and fall back to `pkill -x` by name.
/// 3. Either way, poll until the socket is gone before returning so the new
///    daemon won't see a live socket when it tries to bind.
#[cfg(unix)]
fn evict_stray_daemon() {
    if let Some(status) = probe_status() {
        // Socket answered — targeted SIGTERM by PID.
        if let Some(pid) = status.pid {
            tracing::info!(
                "evict_stray_daemon: sending SIGTERM to daemon pid={pid} \
                 build={:?} degraded={}",
                status.build_version,
                status.degraded
            );
            sigterm_pid(pid);
        } else {
            // Status answered but no pid field — fall back to name match.
            tracing::info!("evict_stray_daemon: no pid in status reply; using pkill -x");
            terminate_stray_by_name();
        }
    } else if socket_file_exists() {
        // Socket file exists but didn't answer — stale. Clean it up and kill by name.
        tracing::info!(
            "evict_stray_daemon: socket exists but is unresponsive — \
             removing stale socket file and sending pkill -x"
        );
        let sock = socket_path();
        let _ = std::fs::remove_file(&sock);
        terminate_stray_by_name();
    } else {
        // No socket at all — nothing to evict.
        return;
    }

    // Poll until the socket is confirmed gone before the caller spawns.
    wait_for_socket_released(SOCKET_RELEASE_TIMEOUT);
}

#[cfg(not(unix))]
fn evict_stray_daemon() {
    // No-op on non-unix.
}

/// Ensure the daemon is running, starting the bundled one if needed.
///
/// Always attempts eviction: if the socket answers, we read its pid/version to
/// decide; if the socket file exists but is silent, we treat it as stale.
/// After eviction, we poll until the socket is gone, THEN spawn — eliminating
/// the fixed-sleep race where a still-live socket causes the new daemon to bail.
///
/// Returns `Ok(true)` if the app started a daemon (handle stored in managed
/// state) and it became reachable, or an `Err` describing why the app could
/// not bring it up (surfaced LOUDLY so the degraded UI shows the failure —
/// never swallowed).
///
/// ## Startup-vs-restart race
/// If [`restart_daemon`] runs concurrently during the socket-ready poll loop,
/// it increments [`DaemonLifecycleGen`] before killing the child we stored.
/// We snapshot the generation before the poll loop and recheck it on timeout:
/// if it changed, our child was superseded and the daemon is healthy under new
/// ownership — we return `Ok(true)` instead of a false error.
pub fn ensure_daemon_running(app: &tauri::AppHandle) -> Result<bool, String> {
    // 1. Snapshot the lifecycle generation at function entry — BEFORE any
    //    blocking work — so that a restart_daemon which completes entirely
    //    (bump gen → kill old → spawn new-and-reachable) during bootout or
    //    eviction is captured here.  If we took the snapshot AFTER the up-to-3s
    //    blocking evict we would see the POST-restart gen, mistake the spurious
    //    3rd daemon we're about to spawn as the "first", time out, compare
    //    gen_after==gen_before, and write a FALSE "daemon failed" error.
    //    Reusing the same Arc for both the before/after reads also eliminates
    //    the silent footgun of try_state returning None if the state is not yet
    //    registered (the debug_assert below catches that in debug builds).
    let gen_arc = app.try_state::<DaemonLifecycleGen>();
    debug_assert!(
        gen_arc.is_some(),
        "DaemonLifecycleGen must be registered in Tauri managed state before \
         ensure_daemon_running is called"
    );
    let gen_before = gen_arc
        .as_ref()
        .map(|s| s.0.load(Ordering::SeqCst))
        .unwrap_or(0);

    // 2. Reconcile the LaunchAgent first: boot out any leftover instance so it
    //    cannot fight the app-owned lifecycle.
    bootout_launchagent();

    // 3. Always attempt eviction (fixes wedged/stale daemon after upgrade).
    //    Unlike the old code that skipped eviction when daemon_reachable()=false,
    //    we handle all three cases: reachable, socket-exists-but-silent, no socket.
    evict_stray_daemon();

    // 4. Spawn the fresh bundled daemon.
    let bin = bundled_daemon_path().ok_or_else(|| {
        // Actionable message: this happens when the user dragged a new .app over a
        // running instance, leaving the bundle incomplete.
        "Installation incomplete — the background service is missing. \
         Quit CopyPaste fully (tray → Quit), delete /Applications/CopyPaste.app, \
         then reinstall from the DMG (don't drag while it's running)."
            .to_string()
    })?;

    let child = std::process::Command::new(&bin)
        // Enable P2P (mTLS identity + mDNS LAN advertising) so the app-owned
        // daemon advertises a device fingerprint and can generate pairing QRs.
        // The daemon gates P2P on COPYPASTE_P2P; without this it runs P2P-off,
        // leaving Devices->Fingerprint empty and LAN pairing impossible.
        .env("COPYPASTE_P2P", "1")
        .spawn()
        .map_err(|e| format!("failed to spawn daemon at {}: {e}", bin.display()))?;

    if let Some(state) = app.try_state::<DaemonChild>() {
        // Replace any previously-tracked child (shouldn't exist at startup, but
        // be defensive). Dropping the old handle does not kill it.
        // Use unwrap_or_else on poisoned mutex so a panic in a prior thread
        // never blocks app exit or a successful subsequent call.
        // NOTE: we release the mutex immediately — we do NOT hold it across the
        // blocking poll loop below (that would deadlock restart_daemon's
        // stop_tracked_child, which also locks DaemonChild).
        let mut guard = state.0.lock().unwrap_or_else(|e| e.into_inner());
        *guard = Some(child);
    }

    // 5. Wait for the socket to come up. Poll up to SOCKET_READY_TIMEOUT.
    let deadline = Instant::now() + SOCKET_READY_TIMEOUT;
    while Instant::now() < deadline {
        if daemon_reachable() {
            tracing::info!("app-owned daemon is up (binary: {})", bin.display());
            return Ok(true);
        }
        std::thread::sleep(SOCKET_POLL_INTERVAL);
    }

    // 6. Timed out. Check whether restart_daemon superseded us while we polled.
    //    If the generation advanced, our child was killed and replaced — the
    //    daemon is healthy under new ownership, so this is NOT an error.
    //    Reuse gen_arc captured at function entry so both reads hit the same
    //    Arc and there is no window where try_state could return None on one
    //    read but Some on the other.
    let gen_after = gen_arc
        .as_ref()
        .map(|s| s.0.load(Ordering::SeqCst))
        .unwrap_or(0);
    if gen_after != gen_before {
        tracing::info!(
            "ensure_daemon_running: startup child superseded by restart \
             (gen {gen_before} → {gen_after}); not recording a false error"
        );
        return Ok(true);
    }

    Err(format!(
        "started daemon at {} but it did not become reachable on {} within {}ms",
        bin.display(),
        socket_path().display(),
        SOCKET_READY_TIMEOUT.as_millis()
    ))
}

/// Ensure the daemon is running on a background thread. The result is stored
/// in [`DaemonSpawnError`] managed state and optionally emitted as a Tauri
/// event `"daemon-spawn-result"` so the UI can surface failures without
/// blocking the main thread (tray + window render immediately).
pub fn ensure_daemon_running_async(app: tauri::AppHandle) {
    let app_clone = app.clone();
    std::thread::spawn(move || {
        let result = ensure_daemon_running(&app_clone);
        // Store the error (or clear it on success) so `get_daemon_error` can read it.
        if let Some(state) = app_clone.try_state::<DaemonSpawnError>() {
            let mut guard = state.0.lock().unwrap_or_else(|e| e.into_inner());
            *guard = result.as_ref().err().cloned();
        }
        // Emit an event so the UI knows the daemon is (or isn't) ready.
        let payload = match &result {
            Ok(_) => serde_json::json!({ "ok": true }),
            Err(e) => serde_json::json!({ "ok": false, "error": e }),
        };
        if let Err(e) = tauri::Emitter::emit(&app_clone, "daemon-spawn-result", payload) {
            tracing::warn!("failed to emit daemon-spawn-result event: {e}");
        }
        if let Err(e) = result {
            tracing::error!("failed to start app-owned daemon: {e}");
        }
    });
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
    // Use unwrap_or_else so a poisoned mutex (from a panicked thread) never
    // blocks app exit — we take the inner value and proceed.
    let mut guard = state.0.lock().unwrap_or_else(|e| e.into_inner());
    let Some(mut child) = guard.take() else {
        // We never started a daemon — nothing of ours to reap.
        return;
    };

    let pid = child.id();
    #[cfg(unix)]
    {
        // Graceful SIGTERM via targeted kill so the daemon flushes WAL / Keychain
        // state before exiting (`Child::kill` would send SIGKILL).
        sigterm_pid(pid);
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

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

/// The app's own build version (the crate version, e.g. `"0.5.2"`).
///
/// The frontend compares this against the daemon's reported `build_version`
/// (which is `"<crate-version>+<git-sha>"`) by semver prefix: a different
/// prefix means a stale daemon survived an upgrade and should be restarted.
#[tauri::command]
pub fn app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Return the last spawn error from [`ensure_daemon_running_async`], if any.
///
/// Returns `null` when the daemon started successfully (or hasn't been
/// attempted yet). The UI should listen for the `"daemon-spawn-result"` event
/// for real-time feedback; this command is the fallback for views that load
/// after the event fires.
#[tauri::command]
pub fn get_daemon_error(app: tauri::AppHandle) -> Option<String> {
    app.try_state::<DaemonSpawnError>().and_then(|state| {
        let guard = state.0.lock().unwrap_or_else(|e| {
            tracing::error!(
                "get_daemon_error: DaemonSpawnError mutex was poisoned; \
                 recovering inner value (a prior thread panicked while holding it)"
            );
            e.into_inner()
        });
        guard.clone()
    })
}

/// Restart the daemon so the freshly-installed binary takes over.
///
/// In app-owned mode the LaunchAgent is not used, so this stops the tracked
/// child (SIGTERM + reap) then calls `ensure_daemon_running` to respawn the
/// bundled binary. This works even when the daemon is degraded or unresponsive
/// because it talks to the child handle directly, not via IPC.
///
/// Returns `Ok(())` on success or a human-readable error string the UI
/// surfaces loudly.
#[tauri::command]
pub fn restart_daemon(app: tauri::AppHandle) -> Result<(), String> {
    // Step 1: bump the lifecycle generation BEFORE killing the old child.
    // This lets a concurrent ensure_daemon_running (running in the startup
    // background thread) detect that its child was superseded and suppress
    // the false "daemon failed" error it would otherwise emit on timeout.
    if let Some(gen_state) = app.try_state::<DaemonLifecycleGen>() {
        gen_state.0.fetch_add(1, Ordering::SeqCst);
    }

    // Step 2: stop the currently tracked child gracefully.
    stop_tracked_child(&app);

    // Step 3: spawn a fresh one and wait for it to become reachable.
    let result = ensure_daemon_running(&app);
    // Mirror what ensure_daemon_running_async does: write the outcome into
    // DaemonSpawnError so get_daemon_error reflects the restart result and a
    // stale startup error does not linger after a successful restart.
    if let Some(state) = app.try_state::<DaemonSpawnError>() {
        let mut guard = state.0.lock().unwrap_or_else(|e| e.into_inner());
        *guard = result.as_ref().err().cloned();
    }
    result.map(|_| ())
}

/// Stop the child we're tracking (used by restart_daemon before respawn).
/// Mirrors stop_daemon but does NOT bootout the LaunchAgent — we are about to
/// respawn, not quit the app.
fn stop_tracked_child(app: &tauri::AppHandle) {
    let Some(state) = app.try_state::<DaemonChild>() else {
        return;
    };
    let mut guard = state.0.lock().unwrap_or_else(|e| e.into_inner());
    let Some(mut child) = guard.take() else {
        return;
    };
    let pid = child.id();
    #[cfg(unix)]
    {
        sigterm_pid(pid);
        for _ in 0..40 {
            match child.try_wait() {
                Ok(Some(_)) => {
                    tracing::info!("restart_daemon: stopped tracked child (pid {pid})");
                    return;
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(50)),
                Err(_) => break,
            }
        }
        // Force-kill if it didn't exit gracefully.
        let _ = child.kill();
        let _ = child.wait();
    }
    #[cfg(not(unix))]
    {
        let _ = child.kill();
        let _ = child.wait();
    }
    tracing::info!("restart_daemon: stopped tracked child (pid {pid})");
}
