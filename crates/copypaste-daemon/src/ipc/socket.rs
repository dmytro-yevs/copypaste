//! Unix socket lifecycle: bind, stale-daemon eviction, and connection probing.
//!
//! Extracted from `ipc.rs` for organisation — behaviour unchanged.
//! All items are re-exported from `ipc/mod.rs`.

use super::BUILD_VERSION;
use anyhow::Context as _; // CopyPaste-crh3.90
use tokio::net::UnixListener;

/// Probe whether a Unix-domain socket at `socket_path` has a *live* listener.
///
/// A stale socket file (left behind by a daemon that crashed or was killed
/// without a clean shutdown) still exists on disk but no process is accepting
/// connections on it: `connect()` then fails with `ECONNREFUSED`. A socket
/// owned by a running daemon accepts the connection. We connect and
/// immediately drop the stream — this is a zero-byte probe the daemon's accept
/// loop tolerates (it spawns a handler that reads EOF and exits).
///
/// Returns `false` when the path does not exist, is not a socket, or the
/// connect is refused (stale). Returns `true` only when a live listener
/// actually accepts the connection.
pub(crate) fn is_socket_live(socket_path: &std::path::Path) -> bool {
    if !socket_path.exists() {
        return false;
    }
    std::os::unix::net::UnixStream::connect(socket_path).is_ok()
}

/// What the synchronous `status` probe learned about the daemon currently
/// listening on the socket.
#[derive(Debug, Default)]
pub(crate) struct ProbedDaemon {
    /// The peer's `build_version` (`<crate-version>+<git-sha>`), if it reported
    /// one. A pre-takeover daemon (older build) will not include this field.
    pub(crate) build_version: Option<String>,
    /// The peer's OS process id, if reported. Used to SIGTERM a stale
    /// predecessor that does not cooperate via IPC.
    pub(crate) pid: Option<u32>,
    /// True when the peer reported `"degraded": true` in its `status` response.
    /// A same-version daemon that is degraded (e.g. keychain-locked / DB
    /// unavailable) should be replaced by a healthy same-version daemon — the
    /// usual "same version = healthy, do not steal" rule does not apply.
    degraded: bool,
}

/// Synchronously connect to a live socket and ask `status`, returning the
/// peer's `build_version` + `pid` if it answered. Best-effort: any IO/parse
/// failure yields `None` (treated as "unknown / probably stale").
///
/// This is the blocking, pre-bind sibling of the async `status` dispatch — it
/// runs in the new daemon's startup path *before* the tokio runtime owns the
/// socket, so it deliberately uses `std::os::unix::net` with short timeouts.
pub(crate) fn probe_listening_daemon(socket_path: &std::path::Path) -> Option<ProbedDaemon> {
    use std::io::{BufRead, BufReader, Write};
    use std::time::Duration;

    // Short timeout for the takeover-probe handshake: must be fast enough that
    // startup is not delayed if the old daemon is unresponsive, but long enough
    // to complete a loopback JSON-RPC round-trip on a loaded machine.
    const PROBE_TIMEOUT: Duration = Duration::from_secs(3);

    let stream = std::os::unix::net::UnixStream::connect(socket_path).ok()?;
    let _ = stream.set_read_timeout(Some(PROBE_TIMEOUT));
    let _ = stream.set_write_timeout(Some(PROBE_TIMEOUT));

    let mut req = serde_json::to_string(
        &serde_json::json!({"id":"takeover-probe","method":"status","params":{}}),
    )
    .ok()?;
    req.push('\n');
    (&stream).write_all(req.as_bytes()).ok()?;

    let mut reader = BufReader::new(&stream);
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;
    let v: serde_json::Value = serde_json::from_str(line.trim()).ok()?;
    let data = &v["data"];
    Some(ProbedDaemon {
        build_version: data["build_version"].as_str().map(str::to_owned),
        pid: data["pid"].as_u64().and_then(|p| u32::try_from(p).ok()),
        // A peer that does not emit `degraded` is assumed healthy (false).
        degraded: data["degraded"].as_bool().unwrap_or(false),
    })
}

/// Attempt to evict a stale predecessor daemon and free its socket.
///
/// Sends `SIGTERM` to `pid` (a clean shutdown — launchd's `KeepAlive` only
/// respawns on a *crash*, so a SIGTERM exit will not race us back onto the
/// socket) and polls until the socket stops answering or a short deadline
/// elapses. Returns `true` once the socket is free.
///
/// CopyPaste-dl1e TOCTOU / pid-recycle guard:
///
/// The `pid` comes from a prior IPC `status` response. Between when we read it
/// and when we call `kill(2)` the OS may have reaped the original daemon and
/// assigned the same numeric PID to an unrelated process. Without validation we
/// could signal any arbitrary process.
///
/// Defence layers (fail-safe: if we cannot confirm identity, we do NOT signal):
/// 1. Never signal pid 0 (whole process group), 1 (init), or ourselves.
/// 2. Validate that the target exe path contains "copypaste" — if it does not,
///    the PID has been recycled and we abort rather than SIGTERM a stranger.
///    Validated via `/proc/<pid>/exe` (Linux) or `proc_pidpath` (macOS).
/// 3. After sending SIGTERM we verify the socket *actually* freed (re-probe) —
///    if a recycled pid (different process, same number) was signalled but still
///    held the socket, we surface failure rather than unlinking a live socket.
#[cfg(unix)]
fn evict_stale_daemon(socket_path: &std::path::Path, pid: u32) -> bool {
    use std::time::{Duration, Instant};

    // Guard: never signal pid=0 (whole process group), pid=1 (init), or
    // ourselves. Any of these would be a dangerous misfire from a recycled pid.
    if pid == 0 || pid == 1 || pid == std::process::id() {
        tracing::warn!(
            "evict_stale_daemon: refusing to signal dangerous pid {pid} \
             (0=process-group, 1=init, self={self_pid})",
            self_pid = std::process::id()
        );
        return false;
    }

    // CopyPaste-dl1e: validate the process exe before signalling.
    // `pid_exe_is_copypaste` resolves the exe path for `pid` and checks it
    // contains "copypaste". This catches the most common PID-recycle scenario:
    // a completely unrelated process (e.g. a user app) that happened to get
    // the same numeric PID after our predecessor exited.
    //
    // Fail-safe: if the exe cannot be determined (e.g. the process exited
    // between our probe and this check, or we lack permissions), we do NOT
    // signal — a false negative (missing the eviction) is far safer than a
    // false positive (killing an unrelated process).
    match pid_exe_is_copypaste(pid) {
        Some(true) => {
            // Confirmed copypaste daemon — safe to proceed.
        }
        Some(false) => {
            // PID has been recycled by a non-copypaste process. Do NOT signal.
            tracing::warn!(
                "evict_stale_daemon: pid {pid} exe does not match copypaste; \
                 PID may have been recycled — refusing to signal (CopyPaste-dl1e)"
            );
            return false;
        }
        None => {
            // Could not determine the exe (process may have already exited,
            // or we lack permissions to inspect it). Fail safe: don't signal.
            tracing::warn!(
                "evict_stale_daemon: could not verify exe for pid {pid}; \
                 failing safe by not signalling (CopyPaste-dl1e)"
            );
            return false;
        }
    }

    // SAFETY: `kill(2)` with SIGTERM is a thin libc wrapper; the only effect is
    // delivering a signal to `pid`. We have already excluded 0, 1, and self
    // above and confirmed the exe belongs to copypaste, so this is safe.
    let rc = unsafe { libc::kill(pid as libc::pid_t, libc::SIGTERM) };
    if rc != 0 {
        let err = std::io::Error::last_os_error();
        // ESRCH = no such process: the predecessor already exited; treat the
        // socket as ours to reclaim. Any other error (e.g. EPERM) means we
        // could not signal it — give up so we don't unlink a live socket.
        if err.raw_os_error() != Some(libc::ESRCH) {
            tracing::warn!("failed to SIGTERM stale daemon pid {pid}: {err}");
            return false;
        }
        // ESRCH: predecessor already gone — check whether the socket freed.
    } else {
        tracing::warn!(
            "sent SIGTERM to stale daemon pid {pid}; waiting for it to release the socket"
        );
    }

    // Poll until the socket stops answering (the peer shut down and closed its
    // fd) or the deadline expires. We re-probe the socket rather than just
    // checking for the file, because a pid-recycled process (different process,
    // same numeric pid) could have received SIGTERM and exited while the
    // *original* stale daemon still holds the socket — we only declare success
    // when the socket itself is no longer live.
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if !is_socket_live(socket_path) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    // Final re-probe: success only when the socket is confirmed free.
    !is_socket_live(socket_path)
}

/// Resolve the executable path for `pid` and check whether it looks like a
/// CopyPaste daemon binary (path contains "copypaste", case-insensitive).
///
/// Returns:
/// - `Some(true)`  — exe resolved and matches "copypaste"
/// - `Some(false)` — exe resolved but does NOT match — PID may be recycled
/// - `None`        — exe could not be determined (process gone, permission denied)
///
/// CopyPaste-dl1e: called by `evict_stale_daemon` before signalling to prevent
/// the PID-recycle TOCTOU where a non-copypaste process inherits the stale PID.
#[cfg(unix)]
pub(crate) fn pid_exe_is_copypaste(pid: u32) -> Option<bool> {
    let exe = pid_exe_path(pid)?;
    let exe_lower = exe.to_string_lossy().to_lowercase();
    Some(exe_lower.contains("copypaste"))
}

/// Return the exe path for `pid` using a platform-specific mechanism.
///
/// - **Linux**: reads the `/proc/<pid>/exe` symlink.
/// - **macOS**: calls `proc_pidpath(2)` via `libc`.
/// - Other platforms: falls back to `None` (fail-safe: caller will not signal).
#[cfg(unix)]
pub(crate) fn pid_exe_path(pid: u32) -> Option<std::path::PathBuf> {
    #[cfg(target_os = "linux")]
    {
        // /proc/<pid>/exe is a symlink to the actual executable. readlink
        // requires no special privileges for processes owned by the same user.
        std::fs::read_link(format!("/proc/{pid}/exe")).ok()
    }

    #[cfg(target_os = "macos")]
    {
        // proc_pidpath fills a buffer with the null-terminated exe path.
        // PROC_PIDPATHINFO_MAXSIZE is 4096 on all Apple platforms.
        const MAXSIZE: usize = 4096;
        let mut buf = vec![0u8; MAXSIZE];
        // SAFETY: buf is MAXSIZE bytes; proc_pidpath writes at most MAXSIZE bytes
        // including a null terminator. Returns number of bytes written (>0) or ≤0
        // on error (permission denied, process gone). The pointer cast to *mut
        // c_void is valid for a byte buffer. We hold `buf` alive for the duration.
        let ret = unsafe {
            libc::proc_pidpath(
                pid as libc::c_int,
                buf.as_mut_ptr() as *mut libc::c_void,
                MAXSIZE as u32,
            )
        };
        if ret <= 0 {
            return None;
        }
        // Trim to the written length (ret bytes, no null terminator needed).
        buf.truncate(ret as usize);
        // Remove trailing null bytes if proc_pidpath included them.
        while buf.last() == Some(&0) {
            buf.pop();
        }
        Some(std::path::PathBuf::from(std::ffi::OsString::from(
            String::from_utf8_lossy(&buf).into_owned(),
        )))
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        // Unknown platform: fail safe — caller will not signal the pid.
        let _ = pid;
        None
    }
}

/// Bind a [`UnixListener`] at `socket_path`, self-healing a stale socket file
/// and evicting a stale *predecessor* daemon left over from an upgrade.
///
/// macOS / Linux refuse to `bind()` over an existing socket path
/// (`EADDRINUSE`), so a socket file left behind by a previous daemon would
/// otherwise permanently block startup — the exact "process alive but IPC
/// socket not reachable" symptom seen after a v0.3.4 → v0.4.0 upgrade where an
/// old daemon died without cleaning up.
///
/// ## Atomicity (CopyPaste-ah1m)
///
/// The old implementation had a TOCTOU race between the connect-probe and
/// the remove→bind steps: two concurrently starting daemons (e.g. a launchd
/// restart race) could both conclude "socket is stale" and then both try to
/// `remove_file` + `bind`, leaving one of them holding a listener that is
/// immediately overwritten by the other.
///
/// Fix: before the probe→remove→bind sequence we acquire an **exclusive
/// `flock(2)`** on an adjacent lockfile (`<socket_path>.lock`).  Because
/// `flock` is process-wide and the fd is held until this function returns,
/// at most one concurrent starter can be inside the critical section at any
/// moment.  The second starter blocks on `flock` and then re-probes an
/// already-bound socket, correctly detecting the healthy peer.
///
/// The lockfile is never deleted (only created).  Because it is distinct from
/// the socket file itself, a SIGKILL/OOM that leaves the socket behind also
/// leaves the lockfile behind — both are cleaned up correctly on the next
/// healthy start (the lock is released implicitly when the fd closes on crash).
///
/// Policy (newest binary wins on upgrade):
///   * No file present → bind directly.
///   * File present, NO live listener → stale file; remove it and bind.
///   * File present, live listener that reports the SAME `build_version` as us
///     → a healthy same-version daemon already owns the socket; do NOT steal it
///     (that would needlessly orphan a running peer) — return an error so the
///     caller logs and exits cleanly.
///   * File present, live listener that reports a DIFFERENT `build_version`, or
///     no version at all (an older build predating this takeover logic) → a
///     STALE predecessor still serving old code after an upgrade. Evict it
///     (SIGTERM its reported pid, wait for the socket to free), then remove the
///     socket file and bind. This is what lets the freshly-installed binary
///     take over without a manual `kill`.
pub(crate) fn bind_with_stale_cleanup(
    socket_path: &std::path::Path,
) -> anyhow::Result<UnixListener> {
    // CopyPaste-ah1m: serialize the probe→remove→bind sequence with an
    // exclusive flock on an adjacent lockfile so two concurrently-starting
    // daemons cannot both conclude "socket is stale" and race on bind.
    let lock_path = {
        let mut p = socket_path.as_os_str().to_owned();
        p.push(".lock");
        std::path::PathBuf::from(p)
    };
    let lock_file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&lock_path)
        .map_err(|e| {
            anyhow::anyhow!(
                "could not open socket lockfile {}: {e} — check permissions on {}",
                lock_path.display(),
                lock_path
                    .parent()
                    .unwrap_or(std::path::Path::new("."))
                    .display()
            )
        })?;
    // Blocking exclusive flock: second concurrent starter waits here until
    // the first has either succeeded (socket now live → probe will see it) or
    // failed (socket still absent → second starter may proceed).
    {
        use std::os::unix::io::AsRawFd;
        let fd = lock_file.as_raw_fd();
        // SAFETY: fd is a valid open file descriptor owned by lock_file.
        let rc = unsafe { libc::flock(fd, libc::LOCK_EX) };
        if rc != 0 {
            let e = std::io::Error::last_os_error();
            anyhow::bail!(
                "flock on socket lockfile {} failed: {e}",
                lock_path.display()
            );
        }
    }
    // Lock is held; _lock_file keeps the fd open (and flock live) until we
    // return. Drop order: listener is returned, lock released after.
    let _lock_file = lock_file;

    if socket_path.exists() {
        if is_socket_live(socket_path) {
            let probed = probe_listening_daemon(socket_path).unwrap_or_default();
            // Decide whether to evict or refuse.
            //
            // Same version AND not degraded → healthy peer; do not steal.
            if probed.build_version.as_deref() == Some(BUILD_VERSION) && !probed.degraded {
                anyhow::bail!(
                    "another daemon (build {BUILD_VERSION}) is already listening on {} — \
                     refusing to steal the socket from a healthy same-version peer",
                    socket_path.display()
                );
            }
            // All other cases (different version, no version, or same version
            // but degraded) → attempt to evict so a healthy daemon can take over.
            let evict_reason = if probed.build_version.as_deref() == Some(BUILD_VERSION) {
                // Same version, but degraded.
                format!("same-version daemon (build {BUILD_VERSION}) is DEGRADED")
            } else {
                let reported = probed.build_version.as_deref().unwrap_or("<none>");
                format!("stale daemon (build {reported}); this build is {BUILD_VERSION}")
            };
            tracing::warn!(
                "{evict_reason} holds {}; evicting so the healthy instance can take over.",
                socket_path.display()
            );
            match probed.pid {
                Some(pid) if evict_stale_daemon(socket_path, pid) => {
                    tracing::info!("evicted daemon pid {pid} — socket released");
                }
                Some(pid) => {
                    anyhow::bail!(
                        "could not evict daemon pid {pid} holding {} ({evict_reason}) — \
                         use the app's \"Restart daemon\" control or \
                         `launchctl kickstart -k gui/$UID/com.copypaste.daemon`",
                        socket_path.display()
                    );
                }
                None => {
                    // Old build reported no pid: we cannot signal it.
                    // Surface a clear, actionable error rather than
                    // unlinking a socket a live process still owns.
                    anyhow::bail!(
                        "daemon ({evict_reason}, no pid reported) holds {} and \
                         cannot be evicted automatically — use the app's \"Restart daemon\" \
                         control or `launchctl kickstart -k gui/$UID/com.copypaste.daemon`",
                        socket_path.display()
                    );
                }
            }
        }
        tracing::warn!(
            "removing stale IPC socket at {} (no live listener answered)",
            socket_path.display()
        );
        // Best-effort: if removal races with another process recreating it,
        // the subsequent bind error is the authoritative signal.
        let _ = std::fs::remove_file(socket_path);
    }
    let listener = UnixListener::bind(socket_path)?;

    // CopyPaste-c4q2.26: tighten the socket inode to 0600 via `fchmod` on the
    // bound fd rather than a process-wide `umask(0o177)`.
    //
    // The previous fix (CopyPaste-6exk) wrapped the bind in `umask(0o177)` to
    // make the kernel create the socket at 0600. But `umask` is a per-PROCESS
    // property: during the (brief) window it was set, ANY concurrent startup
    // thread — `spawn_blocking` tasks writing `device_id`, `peers.json`,
    // `config.json` — would have its newly-created files clamped to `& ~0o177`
    // (i.e. 0600) too, regardless of their intended mode. Startup is largely
    // sequential today, so it rarely bit, but it was not structurally
    // guaranteed and is a latent correctness bug.
    //
    // A path `chmod` targets ONLY this socket's inode and has no global side
    // effect. (`fchmod(2)` on an AF_UNIX socket fd returns `EINVAL` on macOS,
    // so an fd-based chmod is not portable here — the path chmod is.) The parent
    // directory is already restricted to 0700 by `IpcServer::bind`, so the
    // socket is unreachable by other users even during the sub-millisecond
    // window between `bind` and this `chmod`; `IpcServer::bind` re-asserts 0600
    // on the path as defence-in-depth.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(socket_path, std::fs::Permissions::from_mode(0o600))
            .context("chmod(socket, 0600) failed")?;
    }

    Ok(listener)
}
