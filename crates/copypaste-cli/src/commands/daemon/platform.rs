//! macOS (launchd) implementation for the `copypaste daemon` subcommand.
//!
//! All functions are gated by the caller's `cfg!(target_os = "macos")` check
//! in `super::dispatch`; the only exception is `unsupported_platform`, which
//! is called explicitly on non-macOS targets.
//!
//! Platform support summary:
//!   - macOS:   `launchctl bootstrap gui/<uid> <plist>` / `launchctl bootout gui/<uid>/<label>`
//!   - Linux:   `systemctl --user` (FROZEN — wiring documented, returns clear error)
//!   - Windows: `sc.exe` (FUTURE — returns clear error)

use anyhow::{anyhow, bail, Result};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use super::runner::{CommandRunner, FsOps};

/// Launchd label — must match `<key>Label</key>` in the plist.
pub(super) const LAUNCHD_LABEL: &str = "com.copypaste.daemon";
/// Source plist shipped in the repo (used by `install`).
const PACKAGED_PLIST_RELATIVE: &str = "packaging/macos/com.copypaste.daemon.plist";
/// Per-user plist installation directory (macOS LaunchAgents).
const USER_LAUNCH_AGENTS_DIR: &str = "Library/LaunchAgents";

// ---------------------------------------------------------------------------
// Unsupported-platform fallback (called on non-macOS)
// ---------------------------------------------------------------------------

/// Returns a clear, actionable error for non-macOS platforms.
///
/// `pub(crate)` so that tests in `super` can call it directly to assert the
/// message text without going through `dispatch`.
pub(crate) fn unsupported_platform() -> Result<()> {
    if cfg!(target_os = "linux") {
        bail!(
            "linux daemon management is not yet wired. \
             Manual: copy packaging/linux/copypaste-daemon.service to \
             ~/.config/systemd/user/ and run: \
             systemctl --user daemon-reload && systemctl --user enable --now copypaste-daemon"
        );
    }
    if cfg!(target_os = "windows") {
        bail!(
            "windows daemon management is not yet implemented. \
             Future: use sc.exe create CopyPasteDaemon binPath= \"<path>\""
        );
    }
    bail!("unsupported platform for `copypaste daemon`")
}

// ---------------------------------------------------------------------------
// macOS implementation
// ---------------------------------------------------------------------------

fn macos_uid<R: CommandRunner>(runner: &mut R) -> Result<u32> {
    let out = runner.run("id", &["-u".into()])?;
    if !out.success {
        bail!("`id -u` failed: {}", out.stderr.trim());
    }
    out.stdout
        .trim()
        .parse::<u32>()
        .map_err(|e| anyhow!("could not parse uid from `id -u`: {e}"))
}

/// `launchctl print gui/<uid>/<label>` exits 0 iff the service is loaded.
/// Returns Ok(true) when loaded, Ok(false) otherwise. Network / IO errors
/// from spawning launchctl bubble up as Err.
fn is_loaded<R: CommandRunner>(runner: &mut R, uid: u32) -> Result<bool> {
    let target = format!("gui/{uid}/{LAUNCHD_LABEL}");
    let out = runner.run("launchctl", &["print".into(), OsString::from(&target)])?;
    Ok(out.success)
}

fn user_plist_path<F: FsOps>(fs: &F) -> Result<PathBuf> {
    let home = fs
        .home_dir()
        .ok_or_else(|| anyhow!("could not determine $HOME"))?;
    Ok(home
        .join(USER_LAUNCH_AGENTS_DIR)
        .join(format!("{LAUNCHD_LABEL}.plist")))
}

/// Discover the source plist that should be copied into
/// `~/Library/LaunchAgents/`. Search order, returning the FIRST hit:
///
///   1. `<exe>/../../Resources/com.copypaste.daemon.plist` — packaged install,
///      i.e. `/Applications/CopyPaste.app/Contents/MacOS/copypaste` →
///      `/Applications/CopyPaste.app/Contents/Resources/...`.
///   2. `<exe>/../../../packaging/macos/com.copypaste.daemon.plist` — dev path
///      when running `target/release/copypaste` from a checkout
///      (`target/release/copypaste` → `<repo>/packaging/macos/...`).
///   3. `<cwd>/packaging/macos/com.copypaste.daemon.plist` — repo-relative
///      fallback for `cargo run -- daemon install` style invocations.
///
/// If none exist, return a clear error listing all candidates so the user
/// can pinpoint the actual path issue rather than chasing "file not found".
fn packaged_plist_path<F: FsOps>(fs: &F) -> Result<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();

    // Candidate 1 — inside the .app bundle (production install).
    if let Ok(exe) = fs.current_exe() {
        if let Some(bundle_resources) = exe
            .parent() // .../Contents/MacOS
            .and_then(Path::parent)
        // .../Contents
        {
            candidates.push(
                bundle_resources
                    .join("Resources")
                    .join("com.copypaste.daemon.plist"),
            );
        }

        // Candidate 2 — dev path: target/release/copypaste → repo/packaging/macos/...
        if let Some(repo_root) = exe
            .parent() // target/release
            .and_then(Path::parent) // target
            .and_then(Path::parent)
        // repo root
        {
            candidates.push(repo_root.join(PACKAGED_PLIST_RELATIVE));
        }
    }

    // Candidate 3 — repo-relative fallback (CWD heuristic).
    let cwd = fs.current_dir()?;
    candidates.push(cwd.join(PACKAGED_PLIST_RELATIVE));

    for cand in &candidates {
        if fs.exists(cand) {
            return Ok(cand.clone());
        }
    }

    let pretty = candidates
        .iter()
        .map(|p| format!("  - {}", p.display()))
        .collect::<Vec<_>>()
        .join("\n");
    bail!(
        "packaged plist not found. Looked in:\n{pretty}\n\
         If you're running from a checkout, run from the repo root or build the .app bundle."
    )
}

/// Translate raw `launchctl` failure text into actionable advice.
///
/// Launchctl prints things like `Bootstrap failed: 5: Input/output error` —
/// useless for non-launchd-experts. We recognise the common codes and replace
/// the message; otherwise we pass through the original + a diagnostic hint.
///
/// Reference for codes (see `man launchctl`, `<sys/errno.h>`, and the launchd
/// source `launch.h`):
///   - 5   = EIO ("Input/output error") — bootstrap genuinely failed. Most
///     common cause on macOS user agents: label is on the *disabled* list
///     (`launchctl disable gui/<UID>/<label>`), so `bootstrap` refuses to
///     load it. Fix: `launchctl enable gui/<UID>/<label>` first.
///   - 36  = ENOENT-ish in launchctl ("Could not find service") — service is
///     not loaded in the target domain.
///   - 37  = ALREADY_BOOTSTRAPPED — the canonical "already running" code. Treat
///     as success.
///   - 125 = "Domain does not support specified action" — wrong domain (almost
///     always: ran with `sudo`, so domain became `gui/0`).
///
/// `pub(crate)` so that tests in the parent module can assert message text.
pub(crate) fn friendly_launchctl_error(uid: u32, op: &str, stderr: &str) -> String {
    let s = stderr.trim();
    // Error 37 = ALREADY_BOOTSTRAPPED — the correct "already loaded" code.
    if s.contains(": 37:")
        || s.contains("service already loaded")
        || s.contains("already bootstrapped")
    {
        return "daemon already running (launchctl error 37, ALREADY_BOOTSTRAPPED). \
                Run `copypaste daemon restart` if you want to reload it."
            .to_string();
    }
    // Error 5 = EIO / "Input/output error" — bootstrap genuinely failed.
    // After we started calling `launchctl enable` before every `bootstrap`,
    // this should be rare; when it does happen it usually means the service
    // is still on the disabled list or the plist is structurally invalid.
    if s.contains(": 5:") || s.contains("Input/output error") {
        return format!(
            "launchctl {op} failed (error 5, Input/output error). \
             The service may still be on launchd's disabled list. \
             Try: `launchctl enable gui/{uid}/{LAUNCHD_LABEL}` then retry. \
             If that fails, run `launchctl print-disabled gui/{uid}` to confirm."
        );
    }
    // Error 36 = "Could not find service" — service not loaded in target domain.
    if s.contains(": 36:") || s.contains("Could not find service") {
        return "daemon not running (launchctl error 36).".to_string();
    }
    // Error 125 = "Domain does not support specified action" — wrong domain
    // (typically: ran with sudo so domain is gui/0 instead of gui/<your-uid>).
    if s.contains(": 125:") || s.contains("Domain does not support") {
        return "wrong launchd domain (error 125) — are you running with sudo? Don't. \
                LaunchAgents live in your user domain (gui/<UID>), not root."
            .to_string();
    }
    format!(
        "launchctl {op} failed: {s}. \
         Try `launchctl print gui/{uid}/{LAUNCHD_LABEL}` to diagnose."
    )
}

/// Ensure the launchd label is on the *enabled* list for the user's GUI
/// domain. `launchctl enable` is idempotent: if the label is already enabled
/// it returns 0; if it's on the disabled list (because of a prior
/// `launchctl bootout` or `launchctl disable`), this moves it back to enabled
/// so the subsequent `bootstrap` won't fail with error 5.
///
/// We deliberately ignore the exit code — `enable` can fail for harmless
/// reasons (e.g. the label has never been seen by launchd yet) and any real
/// problem will surface when `bootstrap` runs.
fn enable_launchd_label<R: CommandRunner>(runner: &mut R, uid: u32) -> Result<()> {
    let target = format!("gui/{uid}/{LAUNCHD_LABEL}");
    let _ = runner.run("launchctl", &["enable".into(), OsString::from(&target)])?;
    Ok(())
}

pub(super) fn macos_start<R: CommandRunner, F: FsOps>(runner: &mut R, fs: &mut F) -> Result<()> {
    let plist = user_plist_path(fs)?;
    if !fs.exists(&plist) {
        bail!(
            "plist not installed at {}. Run `copypaste daemon install` first.",
            plist.display()
        );
    }
    let uid = macos_uid(runner)?;

    // Refuse to run as root — LaunchAgents are per-user.
    if uid == 0 {
        bail!(
            "daemon must run as your user, not root. \
             Re-run `copypaste daemon start` WITHOUT sudo. \
             The Launch Agent lives in ~/Library/LaunchAgents/ which is a per-user \
             domain (gui/<UID>); running with sudo tries to bootstrap into gui/0 \
             where the plist is not registered."
        );
    }

    // Idempotency: bail out cleanly if already loaded.
    if is_loaded(runner, uid)? {
        eprintln!("daemon already running (label: {LAUNCHD_LABEL}, domain: gui/{uid}). No-op.");
        return Ok(());
    }

    // CRITICAL: re-enable the label before bootstrap. A previous `launchctl
    // bootout` (or `launchctl disable`) leaves the label on launchd's
    // *disabled* list; the subsequent `bootstrap` would then fail with
    // "Bootstrap failed: 5: Input/output error". `enable` is idempotent so
    // it's safe to call unconditionally on every start.
    enable_launchd_label(runner, uid)?;

    let domain = format!("gui/{uid}");
    let out = runner.run(
        "launchctl",
        &[
            "bootstrap".into(),
            OsString::from(&domain),
            plist.clone().into_os_string(),
        ],
    )?;
    if !out.success {
        bail!(
            "{}",
            friendly_launchctl_error(uid, "bootstrap", &out.stderr)
        );
    }
    eprintln!("daemon started (label: {LAUNCHD_LABEL}, domain: {domain})");
    Ok(())
}

pub(super) fn macos_stop<R: CommandRunner>(runner: &mut R) -> Result<()> {
    let uid = macos_uid(runner)?;

    // Idempotency: nothing to do if not loaded.
    if !is_loaded(runner, uid)? {
        eprintln!("daemon not running (label: {LAUNCHD_LABEL}, domain: gui/{uid}). No-op.");
        return Ok(());
    }

    let target = format!("gui/{uid}/{LAUNCHD_LABEL}");
    let out = runner.run("launchctl", &["bootout".into(), OsString::from(&target)])?;
    if !out.success {
        bail!("{}", friendly_launchctl_error(uid, "bootout", &out.stderr));
    }
    eprintln!("daemon stopped (target: {target})");
    Ok(())
}

pub(super) fn macos_install<R: CommandRunner, F: FsOps>(runner: &mut R, fs: &mut F) -> Result<()> {
    // Note: do NOT resolve the source plist here — `packaged_plist_path` bails
    // when no candidate exists. If the dst already has the plist, we don't
    // need the src at all, so resolve lazily below.
    let dst = user_plist_path(fs)?;
    let uid = macos_uid(runner)?;

    // Ensure the logs directory exists. launchd does NOT mkdir intermediate
    // parents for `StandardOutPath` / `StandardErrorPath`; a missing
    // `~/Library/Logs/CopyPaste/` causes the daemon process to fail at spawn
    // time (logd refuses to open the file) and the daemon never starts.
    // Best-effort: ignore errors (e.g. permission issues will resurface
    // when launchd tries to write its first log line, with a clearer
    // message).
    if let Some(home) = fs.home_dir() {
        let logs_dir = home.join("Library/Logs/CopyPaste");
        let _ = fs.create_dir_all(&logs_dir);
    }

    // Refuse to install as root — same reasoning as start.
    if uid == 0 {
        bail!(
            "daemon must be installed as your user, not root. \
             Re-run `copypaste daemon install` WITHOUT sudo. \
             ~/Library/LaunchAgents/ is per-user; running with sudo would install \
             into root's home and bootstrap into gui/0, which is not what you want."
        );
    }

    let plist_present = fs.exists(&dst);
    let loaded = is_loaded(runner, uid)?;

    // Fully idempotent: plist already in place AND loaded → no-op success.
    if plist_present && loaded {
        eprintln!(
            "daemon already installed and running ({}). No-op.",
            dst.display()
        );
        return Ok(());
    }

    // Need the source if we have to copy.
    if !plist_present {
        // Resolve src lazily — only when we actually need to copy. This way
        // an existing-but-not-loaded install can still run `install` without
        // requiring the packaged plist to be discoverable.
        let src = packaged_plist_path(fs)?;
        if let Some(parent) = dst.parent() {
            fs.create_dir_all(parent)?;
        }
        // Substitute `/Users/USERNAME` placeholder in the bundled plist with the
        // real `$HOME` — otherwise launchd tries to write logs to a path that
        // doesn't exist and the daemon fails silently. This mirrors what
        // `scripts/launchd/install-agent.sh` does via `sed`.
        let raw = fs.read_to_string(&src)?;
        let home = fs
            .home_dir()
            .ok_or_else(|| anyhow!("could not determine $HOME"))?;
        let rendered = raw.replace("/Users/USERNAME", &home.display().to_string());
        fs.write(&dst, &rendered)?;
        eprintln!("installed plist to {}", dst.display());
    } else {
        // Even when the plist is already present, double-check we didn't ship a
        // pre-substituted version with the placeholder still in place (older
        // installs left this artefact). Re-render in-place if so.
        if let Ok(existing) = fs.read_to_string(&dst) {
            if existing.contains("/Users/USERNAME") {
                let home = fs
                    .home_dir()
                    .ok_or_else(|| anyhow!("could not determine $HOME"))?;
                let rendered = existing.replace("/Users/USERNAME", &home.display().to_string());
                fs.write(&dst, &rendered)?;
                eprintln!(
                    "re-rendered stale plist at {} (USERNAME placeholder)",
                    dst.display()
                );
            } else {
                eprintln!("plist already present at {} (skipping copy)", dst.display());
            }
        } else {
            eprintln!("plist already present at {} (skipping copy)", dst.display());
        }
    }

    // Plist is on disk now; if not yet loaded, bootstrap it.
    // (macos_start handles its own idempotency check too, but we've just confirmed it.)
    macos_start(runner, fs)
}

pub(super) fn macos_uninstall<R: CommandRunner, F: FsOps>(
    runner: &mut R,
    fs: &mut F,
) -> Result<()> {
    // Order matters here:
    //   1. bootout — best-effort. Failures are ignored: the daemon may
    //      already be unloaded, in which case launchctl returns error 36.
    //   2. unlink the IPC socket — leftover socket files cause the next
    //      `start` to error out with EADDRINUSE on `bind`. The daemon
    //      itself does `remove_file` before binding (see
    //      `copypaste-daemon/src/ipc.rs`), but if the daemon never bound
    //      cleanly the file may persist. ENOENT here is fine — just means
    //      there was nothing to remove.
    //   3. remove the plist file itself.
    //
    // We deliberately do NOT call `launchctl disable` here. `disable` puts
    // the label on launchd's *disabled* list, which makes a subsequent
    // re-install fail with "Bootstrap failed: 5: Input/output error" until
    // the user manually `enable`s it. The default uninstall must be
    // reversible by a simple `install` — `bootout` already removes the
    // service from the loaded set, which is the right cleanup for "stop
    // running, but allow easy re-enable".
    let _ = macos_stop(runner);

    // Unlink the IPC socket. Canonical path mirrors
    // `copypaste-daemon::paths::socket_path()` on macOS:
    // `~/Library/Application Support/CopyPaste/daemon.sock`.
    if let Some(home) = fs.home_dir() {
        let sock = home.join("Library/Application Support/CopyPaste/daemon.sock");
        if fs.exists(&sock) {
            // ENOENT-style errors are tolerated — they mean the socket
            // was already cleaned up. Anything else (e.g. permission
            // denied) we surface as a warning, not a hard error, so the
            // plist still gets removed on the user's behalf.
            match fs.remove_file(&sock) {
                Ok(()) => eprintln!("removed stale IPC socket at {}", sock.display()),
                Err(e) => eprintln!(
                    "warning: could not remove socket at {}: {e}",
                    sock.display()
                ),
            }
        }
    }

    let plist = user_plist_path(fs)?;
    if fs.exists(&plist) {
        fs.remove_file(&plist)?;
        eprintln!("removed plist at {}", plist.display());
    } else {
        eprintln!("no plist to remove at {}", plist.display());
    }
    Ok(())
}
