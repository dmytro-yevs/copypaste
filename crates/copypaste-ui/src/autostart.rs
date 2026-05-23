//! Daemon autostart on app launch (macOS only).
//!
//! When `CopyPaste.app` launches, this module ensures the background daemon
//! is running so the user does not have to invoke `copypaste daemon install &&
//! copypaste daemon start` manually after a fresh DMG install.
//!
//! Sequence on macOS:
//!   1. Probe the daemon IPC socket — if it responds, return [`DaemonStatus::AlreadyRunning`].
//!   2. If the Launch Agent plist is missing under `~/Library/LaunchAgents/`,
//!      copy it from `CopyPaste.app/Contents/Resources/com.copypaste.daemon.plist`
//!      and substitute the `USERNAME` placeholder used for log paths.
//!   3. `launchctl bootstrap gui/<uid> <plist>` to load + start the daemon.
//!   4. Wait briefly and re-probe — return [`DaemonStatus::Started`] on success
//!      or [`DaemonStatus::FailedToStart`] if the socket never appears.
//!
//! Non-macOS targets short-circuit to [`DaemonStatus::AlreadyRunning`] so the
//! UI binary never blocks waiting on a no-op.

#![allow(dead_code)] // platform-gated; helpers stay defined on all targets so tests link.

use anyhow::{anyhow, Context, Result};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Launchd label — must match `<key>Label</key>` in the bundled plist.
pub const LAUNCHD_LABEL: &str = "com.copypaste.daemon";
/// Per-user plist installation directory (macOS LaunchAgents).
pub const USER_LAUNCH_AGENTS_DIR: &str = "Library/LaunchAgents";
/// Filename used for the launch agent plist (both source + destination).
pub const PLIST_FILENAME: &str = "com.copypaste.daemon.plist";

/// Outcome of [`ensure_daemon_running`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DaemonStatus {
    /// IPC ping succeeded on first try — nothing to do.
    AlreadyRunning,
    /// Plist installed (if needed) and `launchctl bootstrap` succeeded, then
    /// the IPC ping recovered within the wait budget.
    Started,
    /// `launchctl` either failed or the socket never came back; the UI
    /// continues launching but should surface a status-bar warning.
    FailedToStart(String),
}

/// Public entry point invoked from `main()` on app launch.
///
/// Spawn this in a `std::thread::spawn` so the Slint window can render even
/// when the daemon takes a moment to come up.
pub fn ensure_daemon_running() -> Result<DaemonStatus> {
    let mut runner = SystemRunner;
    let mut fs = SystemFs;
    let env = SystemEnv;
    ensure_daemon_running_inner(&mut runner, &mut fs, &env)
}

pub(crate) fn ensure_daemon_running_inner<R, F, E>(
    runner: &mut R,
    fs: &mut F,
    env: &E,
) -> Result<DaemonStatus>
where
    R: CommandRunner,
    F: FsOps,
    E: EnvOps,
{
    if !cfg!(target_os = "macos") {
        // Linux/Windows — autostart is not wired yet; pretend things are fine
        // so the UI does not block or display a misleading error.
        return Ok(DaemonStatus::AlreadyRunning);
    }

    // Step 1: fast path — IPC ping.
    let socket_path = daemon_socket_path(fs)?;
    if ipc_ping(&socket_path, env) {
        return Ok(DaemonStatus::AlreadyRunning);
    }

    // Step 2a: ensure the logs directory exists. launchd does NOT mkdir
    // intermediate parents for `StandardOutPath` / `StandardErrorPath`; a
    // missing `~/Library/Logs/CopyPaste/` causes the daemon to fail at
    // spawn time and the socket never appears, masquerading as a generic
    // "FailedToStart". Creating the dir up front is cheap and idempotent.
    if let Some(home) = fs.home_dir() {
        let logs_dir = home.join("Library/Logs/CopyPaste");
        let _ = fs.create_dir_all(&logs_dir);
    }

    // Step 2b: ensure plist installed and up-to-date.
    //   - Missing                                  → render + write.
    //   - Present but drifted from rendered output → re-render + reinstall.
    //     Covers: app moved (DMG drag, Downloads, AppTranslocation), or
    //     the bundled template changed across releases.
    //   - Present but ProgramArguments[0] now      → re-render + reinstall.
    //     points at a path that doesn't exist on
    //     disk (previous .app deleted but launchd
    //     kept the stale entry).
    let dst_plist = user_plist_path(fs)?;
    let src_plist = bundled_plist_path(env)?;
    if !fs.exists(&src_plist) {
        return Ok(DaemonStatus::FailedToStart(format!(
            "bundled plist missing at {}",
            src_plist.display()
        )));
    }
    let raw = fs
        .read_to_string(&src_plist)
        .with_context(|| format!("read {}", src_plist.display()))?;
    let rendered = render_plist(&raw, fs, env)?;
    let daemon_bin = resolve_daemon_binary_path(env)?;

    let needs_install = if !fs.exists(&dst_plist) {
        true
    } else {
        let installed = fs.read_to_string(&dst_plist).unwrap_or_default();
        let drift = plist_hash(&installed) != plist_hash(&rendered);
        // Don't probe daemon_bin existence in the no-drift case — on
        // dev/test setups the bundled binary path may not actually exist
        // even though everything else is consistent. We only treat
        // "dangling daemon binary" as a reinstall trigger when there's
        // also drift, OR when the installed plist still embeds the
        // unsubstituted placeholder (an older install that never had its
        // ProgramArguments rewritten).
        let stale_placeholder = installed
            .contains("/Applications/CopyPaste.app/Contents/MacOS/copypaste-daemon")
            && !rendered.contains("/Applications/CopyPaste.app/Contents/MacOS/copypaste-daemon");
        drift || stale_placeholder
    };
    // `daemon_bin` is reserved for future diagnostic use (e.g. surfacing
    // a clearer error when the resolved daemon path doesn't exist on
    // disk). Suppress unused-variable warnings without dropping the
    // lookup, which also validates `current_exe()` early.
    let _ = &daemon_bin;

    // Step 3a: resolve uid + current load state. Done here so we can reuse
    // both for the optional bootout-before-rewrite below and the bootstrap
    // decision further down — keeps shell-out count minimal.
    let uid = current_uid(runner)?;
    let mut loaded = is_daemon_loaded(runner, uid);

    if needs_install {
        if let Some(parent) = dst_plist.parent() {
            fs.create_dir_all(parent)
                .with_context(|| format!("create {}", parent.display()))?;
        }
        // If the plist is currently loaded, bootout first so the
        // subsequent bootstrap picks up the freshly written file. Best
        // effort: ignore failures.
        if loaded {
            let target = format!("gui/{uid}/{LAUNCHD_LABEL}");
            let _ = runner.run("launchctl", &["bootout".into(), target.into()]);
            // We just unloaded it — re-evaluate so the enable+bootstrap
            // path below actually runs instead of the "already loaded"
            // short-circuit.
            loaded = false;
        }
        fs.write(&dst_plist, &rendered)
            .with_context(|| format!("write {}", dst_plist.display()))?;
    }

    if loaded {
        // Already loaded but socket ping failed earlier — could be a stale
        // launchd entry. Skip bootstrap to avoid a spurious error 37; just
        // fall through to the IPC retry loop below.
    } else {
        // Step 3b: re-enable the label before bootstrap. A prior
        // `launchctl bootout` leaves the label on the *disabled* list, so
        // the subsequent `bootstrap` would fail with error 5 ("Input/output
        // error"). `enable` is idempotent — safe to call every time.
        let target = format!("gui/{uid}/{LAUNCHD_LABEL}");
        let _ = runner.run("launchctl", &["enable".into(), target.clone().into()])?;

        // Step 3c: launchctl bootstrap gui/<uid> <plist>.
        let domain = format!("gui/{uid}");
        let out = runner.run(
            "launchctl",
            &[
                "bootstrap".into(),
                domain.clone().into(),
                dst_plist.clone().into_os_string(),
            ],
        )?;
        if !out.success {
            // After the explicit `enable` above, "already loaded" responses are
            // the only benign failures. Error 37 = ALREADY_BOOTSTRAPPED is the
            // canonical code; error 5 ("Input/output error") historically also
            // meant "already loaded" on older macOS but is more commonly a real
            // failure (e.g. plist invalid) — treat 37 (and explicit text) as
            // benign, 5 as a real bootstrap failure with an actionable hint.
            let stderr_raw = out.stderr.trim();
            let stderr_lc = stderr_raw.to_lowercase();
            let benign_already_loaded = stderr_lc.contains("service already loaded")
                || stderr_lc.contains("already bootstrapped")
                || stderr_raw.contains(": 37:")
                || stderr_raw.contains("Bootstrap failed: 37");
            if !benign_already_loaded {
                // Error 5: most often means the label is still on launchd's
                // disabled list (despite our enable attempt) or the plist is
                // structurally invalid. Surface an actionable message.
                let hint =
                    if stderr_raw.contains(": 5:") || stderr_raw.contains("Input/output error") {
                        format!(
                            " — service may still be disabled. \
                         Try `launchctl enable gui/{uid}/{LAUNCHD_LABEL}` from a Terminal."
                        )
                    } else {
                        String::new()
                    };
                return Ok(DaemonStatus::FailedToStart(format!(
                    "launchctl bootstrap {} {}: {}{}",
                    domain,
                    dst_plist.display(),
                    stderr_raw,
                    hint,
                )));
            }
        }
    }

    // Step 4: wait up to ~15s, retry ping. Fresh-install daemon startup
    // includes sqlcipher key derivation (Argon2id) + DB open + IPC bind,
    // measured ~4s on a 2024 M-series Mac. The prior 2s budget caused a
    // false-positive `FailedToStart` and a permanently-cached "Daemon not
    // running" status in the UI. 15s gives generous headroom on slower
    // machines while still bounding the wait.
    for _ in 0..30 {
        env.sleep(Duration::from_millis(500));
        if ipc_ping(&socket_path, env) {
            return Ok(DaemonStatus::Started);
        }
    }

    // Final long-tail probe: log if the daemon eventually comes up well after
    // the budget so operators can see the actual startup time in logs.
    env.sleep(Duration::from_millis(1000));
    if ipc_ping(&socket_path, env) {
        tracing::info!(
            "autostart: daemon socket appeared past 15s budget — startup unusually slow"
        );
        return Ok(DaemonStatus::Started);
    }

    // Step 5: build an actionable failure message by interrogating launchd.
    // "Socket did not appear" alone is useless — `launchctl print` exposes
    // `pid` + `last exit code` which together distinguish: crash, never-
    // loaded, alive-but-not-listening.
    Ok(DaemonStatus::FailedToStart(diagnose_launchd_failure(
        runner, uid,
    )))
}

/// Shell out `launchctl print gui/<uid>/<label>` and translate the output
/// into a human-readable diagnostic for [`DaemonStatus::FailedToStart`].
///
/// Combinations:
///   - print failed entirely       → label was never loaded
///   - `pid = 0` + `exit != 0`     → crashed at least once
///   - `pid != 0` + socket missing → daemon alive but failed to bind its
///     IPC socket (typically a permission error)
fn diagnose_launchd_failure<R: CommandRunner>(runner: &mut R, uid: u32) -> String {
    let target = format!("gui/{uid}/{LAUNCHD_LABEL}");
    let out = match runner.run("launchctl", &["print".into(), target.clone().into()]) {
        Ok(o) => o,
        Err(e) => {
            return format!(
                "daemon socket did not appear within 15s; could not query launchctl: {e}"
            );
        }
    };
    if !out.success {
        return format!(
            "launchd did not load service {target}: {}",
            out.stderr.trim()
        );
    }
    let combined = format!("{}\n{}", out.stdout, out.stderr);
    let pid = parse_kv_number(&combined, "pid");
    let exit = parse_kv_number(&combined, "last exit code")
        .or_else(|| parse_kv_number(&combined, "lastexitstatus"));
    match (pid, exit) {
        (Some(0), Some(code)) if code != 0 => {
            format!("daemon crash (exit {code}) — check ~/Library/Logs/CopyPaste/daemon.err.log")
        }
        (Some(0), _) => format!(
            "daemon not currently running (pid=0). \
             Inspect: `launchctl print {target}`"
        ),
        (Some(p), _) => format!(
            "daemon process alive (pid={p}) but IPC socket not reachable — \
             check ~/Library/Logs/CopyPaste/daemon.err.log for bind errors"
        ),
        (None, _) => format!(
            "daemon socket did not appear within 15s; \
             launchctl print returned no pid. \
             Inspect: `launchctl print {target}`"
        ),
    }
}

/// Line-scan parser for `launchctl print` output ("key = value", indented,
/// case-insensitive on the key). Returns the first matching number. Used
/// for the `pid` and `last exit code` fields in
/// [`diagnose_launchd_failure`].
fn parse_kv_number(blob: &str, key: &str) -> Option<i64> {
    let needle = format!("{} = ", key.to_lowercase());
    for line in blob.lines() {
        let line_lc = line.to_lowercase();
        if let Some(idx) = line_lc.find(&needle) {
            let rest = &line[idx + needle.len()..];
            let token: String = rest
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == '-')
                .collect();
            if let Ok(n) = token.parse::<i64>() {
                return Some(n);
            }
        }
    }
    None
}

// --------------------------------------------------------------------------------------------
// Helpers
// --------------------------------------------------------------------------------------------

pub(crate) fn daemon_socket_path<F: FsOps>(fs: &F) -> Result<PathBuf> {
    let home = fs
        .home_dir()
        .ok_or_else(|| anyhow!("could not determine $HOME"))?;
    Ok(home.join("Library/Application Support/CopyPaste/daemon.sock"))
}

pub(crate) fn user_plist_path<F: FsOps>(fs: &F) -> Result<PathBuf> {
    let home = fs
        .home_dir()
        .ok_or_else(|| anyhow!("could not determine $HOME"))?;
    Ok(home.join(USER_LAUNCH_AGENTS_DIR).join(PLIST_FILENAME))
}

/// Locate the plist inside the running app bundle:
///   `CopyPaste.app/Contents/MacOS/copypaste-ui` (current exe)
///       parent → `MacOS`
///       parent → `Contents`
///       join   → `Resources/com.copypaste.daemon.plist`
pub(crate) fn bundled_plist_path<E: EnvOps>(env: &E) -> Result<PathBuf> {
    let exe = env.current_exe()?;
    let contents = exe
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| anyhow!("cannot derive Contents/ from {}", exe.display()))?;
    Ok(contents.join("Resources").join(PLIST_FILENAME))
}

/// Replace `/Users/USERNAME/` in the bundled plist with the real `$HOME` so
/// log paths point at the actual user. The bundled plist ships with
/// `USERNAME` as a placeholder for log paths (see `packaging/macos/`).
///
/// Anchored on the full `/Users/USERNAME/` path token (with trailing `/`)
/// so a future plist that legitimately contains the literal word
/// `USERNAME` somewhere else (e.g. inside `EnvironmentVariables`) is not
/// corrupted by a substring match.
pub(crate) fn substitute_username<F: FsOps>(plist: &str, fs: &F) -> String {
    let Some(home) = fs.home_dir() else {
        return plist.to_string();
    };
    // Replacement keeps the trailing slash so the resulting path is a
    // proper segment (e.g. `/Users/alice/Library/...`).
    let replacement = format!("{}/", home.display());
    plist.replace("/Users/USERNAME/", &replacement)
}

/// Resolve the daemon binary that should be embedded into
/// `ProgramArguments[0]` of the installed launchd plist.
///
/// The daemon ships as a sibling of the UI binary inside
/// `CopyPaste.app/Contents/MacOS/`. Resolving dynamically (rather than
/// trusting the hardcoded `/Applications/CopyPaste.app/...` from the
/// bundled plist) keeps autostart working when the app is launched from
/// `~/Downloads`, a DMG mount, or under AppTranslocation
/// (`/private/var/folders/.../AppTranslocation/.../CopyPaste.app/...`).
pub(crate) fn resolve_daemon_binary_path<E: EnvOps>(env: &E) -> Result<PathBuf> {
    let exe = env.current_exe()?;
    let macos_dir = exe
        .parent()
        .ok_or_else(|| anyhow!("cannot derive MacOS/ dir from {}", exe.display()))?;
    Ok(macos_dir.join("copypaste-daemon"))
}

/// Rewrite the bundled `ProgramArguments[0]` placeholder to point at the
/// resolved daemon binary. We use plain string substitution because the
/// bundled plist intentionally ships with a well-known placeholder string
/// we can swap — no full plist parser required.
pub(crate) fn substitute_program_path(plist: &str, new_path: &Path) -> String {
    const PLACEHOLDER: &str = "/Applications/CopyPaste.app/Contents/MacOS/copypaste-daemon";
    plist.replace(PLACEHOLDER, &new_path.display().to_string())
}

/// Compose all token substitutions in one pass so callers don't have to
/// remember the order. Currently: `USERNAME` (for log paths) and
/// `ProgramArguments[0]` (for the daemon binary). Keep this list in sync
/// with `packaging/macos/com.copypaste.daemon.plist`.
pub(crate) fn render_plist<F: FsOps, E: EnvOps>(plist: &str, fs: &F, env: &E) -> Result<String> {
    let with_user = substitute_username(plist, fs);
    let daemon_path = resolve_daemon_binary_path(env)?;
    Ok(substitute_program_path(&with_user, &daemon_path))
}

/// Stable hash used only for drift detection between the rendered plist
/// and the on-disk copy. The std hasher is sufficient here — we don't
/// need cryptographic strength, just "did the bytes change".
fn plist_hash(s: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

fn current_uid<R: CommandRunner>(runner: &mut R) -> Result<u32> {
    let out = runner.run("id", &["-u".into()])?;
    if !out.success {
        return Err(anyhow!("`id -u` failed: {}", out.stderr.trim()));
    }
    out.stdout
        .trim()
        .parse::<u32>()
        .map_err(|e| anyhow!("could not parse uid from `id -u`: {e}"))
}

/// `launchctl print gui/<uid>/<label>` exits 0 iff the service is currently
/// loaded in that domain. This is the source of truth for load state —
/// strictly more reliable than parsing `bootstrap` stderr. Used to avoid
/// firing a redundant bootstrap (and the spurious error 37 it would emit).
fn is_daemon_loaded<R: CommandRunner>(runner: &mut R, uid: u32) -> bool {
    let target = format!("gui/{uid}/{LAUNCHD_LABEL}");
    match runner.run("launchctl", &["print".into(), target.into()]) {
        Ok(out) => out.success,
        Err(_) => false,
    }
}

/// Minimal "is the daemon listening?" probe — does NOT depend on the
/// `copypaste-ipc` crate to keep this module trivially testable. On macOS
/// the daemon binds a Unix-domain socket; a successful `UnixStream::connect`
/// is sufficient evidence the process is alive and accepting connections.
fn ipc_ping<E: EnvOps>(socket_path: &Path, env: &E) -> bool {
    env.unix_stream_connect(socket_path)
}

// --------------------------------------------------------------------------------------------
// Abstractions for testability
// --------------------------------------------------------------------------------------------

pub(crate) struct CommandOutput {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

pub(crate) trait CommandRunner {
    fn run(&mut self, program: &str, args: &[std::ffi::OsString]) -> Result<CommandOutput>;
}

pub(crate) trait FsOps {
    fn home_dir(&self) -> Option<PathBuf>;
    fn exists(&self, path: &Path) -> bool;
    fn create_dir_all(&mut self, path: &Path) -> Result<()>;
    fn read_to_string(&self, path: &Path) -> Result<String>;
    fn write(&mut self, path: &Path, content: &str) -> Result<()>;
}

pub(crate) trait EnvOps {
    fn current_exe(&self) -> Result<PathBuf>;
    fn unix_stream_connect(&self, path: &Path) -> bool;
    fn sleep(&self, dur: Duration);
}

#[derive(Default)]
struct SystemRunner;

impl CommandRunner for SystemRunner {
    fn run(&mut self, program: &str, args: &[std::ffi::OsString]) -> Result<CommandOutput> {
        let out = std::process::Command::new(program).args(args).output()?;
        Ok(CommandOutput {
            success: out.status.success(),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        })
    }
}

struct SystemFs;

impl FsOps for SystemFs {
    fn home_dir(&self) -> Option<PathBuf> {
        home::home_dir()
    }
    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }
    fn create_dir_all(&mut self, path: &Path) -> Result<()> {
        std::fs::create_dir_all(path)?;
        Ok(())
    }
    fn read_to_string(&self, path: &Path) -> Result<String> {
        Ok(std::fs::read_to_string(path)?)
    }
    fn write(&mut self, path: &Path, content: &str) -> Result<()> {
        std::fs::write(path, content)?;
        Ok(())
    }
}

struct SystemEnv;

impl EnvOps for SystemEnv {
    fn current_exe(&self) -> Result<PathBuf> {
        Ok(std::env::current_exe()?)
    }
    fn unix_stream_connect(&self, path: &Path) -> bool {
        #[cfg(unix)]
        {
            std::os::unix::net::UnixStream::connect(path).is_ok()
        }
        #[cfg(not(unix))]
        {
            let _ = path;
            false
        }
    }
    fn sleep(&self, dur: Duration) {
        std::thread::sleep(dur);
    }
}

// --------------------------------------------------------------------------------------------
// Tests
// --------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::ffi::OsString;

    #[derive(Default)]
    struct MockRunner {
        calls: Vec<(String, Vec<String>)>,
        responses: HashMap<String, (bool, String, String)>,
    }

    impl MockRunner {
        fn with_default_uid() -> Self {
            let mut s = Self::default();
            s.responses
                .insert("id -u".into(), (true, "501\n".into(), String::new()));
            s.responses.insert(
                "launchctl bootstrap".into(),
                (true, String::new(), String::new()),
            );
            // Default: `launchctl print` exits nonzero ("not loaded") so the
            // autostart path falls through to enable+bootstrap. Tests that
            // need the "already loaded" branch override with `.set(..)`.
            s.responses.insert(
                "launchctl print".into(),
                (false, String::new(), "Could not find service".into()),
            );
            s.responses.insert(
                "launchctl enable".into(),
                (true, String::new(), String::new()),
            );
            s
        }
        #[allow(dead_code)]
        fn set(&mut self, key: &str, success: bool, stdout: &str, stderr: &str) {
            self.responses
                .insert(key.into(), (success, stdout.into(), stderr.into()));
        }
    }

    impl CommandRunner for MockRunner {
        fn run(&mut self, program: &str, args: &[OsString]) -> Result<CommandOutput> {
            let args_str: Vec<String> = args
                .iter()
                .map(|a| a.to_string_lossy().into_owned())
                .collect();
            self.calls.push((program.into(), args_str.clone()));
            let key = format!(
                "{} {}",
                program,
                args_str.first().cloned().unwrap_or_default()
            );
            let (success, stdout, stderr) =
                self.responses
                    .get(&key)
                    .cloned()
                    .unwrap_or((true, String::new(), String::new()));
            Ok(CommandOutput {
                success,
                stdout,
                stderr,
            })
        }
    }

    /// Tempdir-backed fake fs so we exercise real `std::fs` paths without
    /// mutating the user's `$HOME` — the autostart flow ends up writing the
    /// plist on disk so a real tempdir is the simplest way to keep tests
    /// hermetic.
    struct TempFs {
        home: PathBuf,
        files: HashMap<PathBuf, String>,
    }

    impl TempFs {
        fn new(home: PathBuf) -> Self {
            Self {
                home,
                files: HashMap::new(),
            }
        }
        fn seed(&mut self, path: PathBuf, content: String) {
            self.files.insert(path, content);
        }
    }

    impl FsOps for TempFs {
        fn home_dir(&self) -> Option<PathBuf> {
            Some(self.home.clone())
        }
        fn exists(&self, path: &Path) -> bool {
            self.files.contains_key(path)
        }
        fn create_dir_all(&mut self, _path: &Path) -> Result<()> {
            Ok(())
        }
        fn read_to_string(&self, path: &Path) -> Result<String> {
            self.files
                .get(path)
                .cloned()
                .ok_or_else(|| anyhow!("not found: {}", path.display()))
        }
        fn write(&mut self, path: &Path, content: &str) -> Result<()> {
            self.files.insert(path.to_path_buf(), content.into());
            Ok(())
        }
    }

    struct FakeEnv {
        exe: PathBuf,
        socket_alive_after_calls: usize,
        calls: std::cell::Cell<usize>,
    }

    impl FakeEnv {
        fn never_alive(exe: PathBuf) -> Self {
            Self {
                exe,
                socket_alive_after_calls: usize::MAX,
                calls: std::cell::Cell::new(0),
            }
        }
        fn always_alive(exe: PathBuf) -> Self {
            Self {
                exe,
                socket_alive_after_calls: 0,
                calls: std::cell::Cell::new(0),
            }
        }
        fn alive_after(exe: PathBuf, n: usize) -> Self {
            Self {
                exe,
                socket_alive_after_calls: n,
                calls: std::cell::Cell::new(0),
            }
        }
        #[allow(dead_code)]
        fn alive_after_all_retries(exe: PathBuf) -> Self {
            // 1 ping in step 1 + 30 pings in retry loop + 1 long-tail probe
            // = 32 total. Coming alive at exactly call #32 exercises the
            // long-tail success branch deterministically.
            Self {
                exe,
                socket_alive_after_calls: 31,
                calls: std::cell::Cell::new(0),
            }
        }
    }

    impl EnvOps for FakeEnv {
        fn current_exe(&self) -> Result<PathBuf> {
            Ok(self.exe.clone())
        }
        fn unix_stream_connect(&self, _path: &Path) -> bool {
            let n = self.calls.get();
            self.calls.set(n + 1);
            n >= self.socket_alive_after_calls
        }
        fn sleep(&self, _dur: Duration) {
            // Tests never actually sleep.
        }
    }

    fn fake_app_exe(tmp: &Path) -> PathBuf {
        // Mimic CopyPaste.app/Contents/MacOS/copypaste-ui inside tempdir.
        let exe = tmp.join("CopyPaste.app/Contents/MacOS/copypaste-ui");
        std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
        // Touch the file so current_exe-style lookups feel realistic; we
        // don't actually exec it.
        std::fs::write(&exe, b"").unwrap();
        exe
    }

    const SAMPLE_PLIST: &str = r#"<?xml version="1.0"?>
<plist><dict>
    <key>Label</key><string>com.copypaste.daemon</string>
    <key>StandardOutPath</key><string>/Users/USERNAME/Library/Logs/CopyPaste/daemon.out.log</string>
</dict></plist>
"#;

    #[cfg(target_os = "macos")]
    #[test]
    fn plist_install_copies_file_to_launch_agents_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let exe = fake_app_exe(tmp.path());
        let bundled = exe
            .parent()
            .unwrap() // MacOS
            .parent()
            .unwrap() // Contents
            .join("Resources")
            .join(PLIST_FILENAME);

        let mut fs = TempFs::new(tmp.path().join("home"));
        fs.seed(bundled.clone(), SAMPLE_PLIST.into());

        let mut runner = MockRunner::with_default_uid();
        // Socket comes alive after bootstrap completes, before retry budget runs out.
        let env = FakeEnv::alive_after(exe.clone(), 2);

        let status = ensure_daemon_running_inner(&mut runner, &mut fs, &env)
            .expect("autostart must not error on the happy path");

        // The plist must now exist at ~/Library/LaunchAgents/.
        let expected_dst = tmp
            .path()
            .join("home/Library/LaunchAgents")
            .join(PLIST_FILENAME);
        assert!(
            fs.exists(&expected_dst),
            "expected plist installed at {}, files: {:?}",
            expected_dst.display(),
            fs.files.keys().collect::<Vec<_>>()
        );

        // USERNAME placeholder must have been substituted with the tempdir home.
        let installed = fs.read_to_string(&expected_dst).unwrap();
        assert!(
            !installed.contains("/Users/USERNAME"),
            "USERNAME placeholder must be substituted, got: {installed}"
        );
        let home_str = tmp.path().join("home").display().to_string();
        assert!(
            installed.contains(&home_str),
            "expected substituted $HOME ({home_str}) in plist, got: {installed}"
        );

        assert!(
            matches!(status, DaemonStatus::Started),
            "expected Started, got {status:?}"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn daemon_already_running_returns_already_running() {
        let tmp = tempfile::tempdir().unwrap();
        let exe = fake_app_exe(tmp.path());

        let mut fs = TempFs::new(tmp.path().join("home"));
        // No plist seeded — flow must short-circuit BEFORE touching the plist.
        let mut runner = MockRunner::with_default_uid();
        let env = FakeEnv::always_alive(exe);

        let status = ensure_daemon_running_inner(&mut runner, &mut fs, &env).unwrap();

        assert_eq!(status, DaemonStatus::AlreadyRunning);
        // No `id` / `launchctl` calls — fast-path must not shell out.
        assert!(
            runner.calls.is_empty(),
            "expected zero shell-outs, got {:?}",
            runner.calls
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn daemon_not_running_attempts_bootstrap() {
        let tmp = tempfile::tempdir().unwrap();
        let exe = fake_app_exe(tmp.path());
        let bundled = exe
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("Resources")
            .join(PLIST_FILENAME);

        let mut fs = TempFs::new(tmp.path().join("home"));
        fs.seed(bundled, SAMPLE_PLIST.into());

        let mut runner = MockRunner::with_default_uid();
        // Socket never comes back — exercise the FailedToStart branch as well
        // as the launchctl invocation.
        let env = FakeEnv::never_alive(exe);

        let status = ensure_daemon_running_inner(&mut runner, &mut fs, &env).unwrap();

        // Must have called `id -u`, then `launchctl print` (load probe),
        // then `launchctl enable` (recovery from disabled list), then
        // `launchctl bootstrap gui/501 <plist>`, then a FINAL
        // `launchctl print` from the diagnose_launchd_failure path that
        // turns "socket never came up" into an actionable message.
        let programs: Vec<&str> = runner.calls.iter().map(|c| c.0.as_str()).collect();
        assert_eq!(
            programs,
            vec!["id", "launchctl", "launchctl", "launchctl", "launchctl"]
        );

        let print_args = &runner.calls[1].1;
        assert_eq!(print_args[0], "print");
        assert_eq!(print_args[1], "gui/501/com.copypaste.daemon");

        let enable_args = &runner.calls[2].1;
        assert_eq!(enable_args[0], "enable");
        assert_eq!(enable_args[1], "gui/501/com.copypaste.daemon");

        let bootstrap_args = &runner.calls[3].1;
        assert_eq!(bootstrap_args[0], "bootstrap");
        assert_eq!(bootstrap_args[1], "gui/501");
        assert!(
            bootstrap_args[2].ends_with(PLIST_FILENAME),
            "expected plist path as 3rd arg, got {bootstrap_args:?}"
        );

        // Diagnostic `launchctl print` re-probe after the wait budget.
        let diag_args = &runner.calls[4].1;
        assert_eq!(diag_args[0], "print");
        assert_eq!(diag_args[1], "gui/501/com.copypaste.daemon");

        // Socket never recovered → FailedToStart with an informative message
        // — must be sourced from diagnose_launchd_failure (which inspects the
        // default mock launchctl-print success and returns one of the
        // pid/exit branches), NOT the legacy "did not appear" generic.
        match status {
            DaemonStatus::FailedToStart(msg) => {
                let lc = msg.to_lowercase();
                assert!(
                    lc.contains("daemon")
                        && (lc.contains("not currently running")
                            || lc.contains("did not appear")
                            || lc.contains("crash")
                            || lc.contains("alive")
                            || lc.contains("launchd did not load")),
                    "expected diagnostic FailedToStart message, got: {msg}"
                );
            }
            other => panic!("expected FailedToStart, got {other:?}"),
        }
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn non_macos_short_circuits_to_already_running() {
        let tmp = tempfile::tempdir().unwrap();
        let exe = tmp.path().join("copypaste-ui");
        std::fs::write(&exe, b"").unwrap();
        let mut fs = TempFs::new(tmp.path().join("home"));
        let mut runner = MockRunner::with_default_uid();
        let env = FakeEnv::never_alive(exe);
        let status = ensure_daemon_running_inner(&mut runner, &mut fs, &env).unwrap();
        assert_eq!(status, DaemonStatus::AlreadyRunning);
        assert!(runner.calls.is_empty());
    }

    #[test]
    fn substitute_username_replaces_placeholder() {
        let fs = TempFs::new(PathBuf::from("/Users/alice"));
        let out = substitute_username(SAMPLE_PLIST, &fs);
        assert!(out.contains("/Users/alice/Library/Logs"), "got: {out}");
        assert!(!out.contains("/Users/USERNAME"));
    }

    /// Anchored substitution must NOT match bare `USERNAME` substrings that
    /// happen to appear outside the `/Users/USERNAME/` path token. This
    /// guards against future plist additions (e.g. an env var literally
    /// named `USERNAME`) where a naive `.replace("USERNAME", ..)` would
    /// corrupt unrelated data.
    #[test]
    fn substitute_username_does_not_touch_unrelated_username_tokens() {
        let fs = TempFs::new(PathBuf::from("/Users/alice"));
        let plist = r#"<dict>
    <key>StandardOutPath</key><string>/Users/USERNAME/Library/Logs/x.log</string>
    <key>EnvironmentVariables</key><dict>
        <key>USERNAME</key><string>literal USERNAME value</string>
    </dict>
</dict>"#;
        let out = substitute_username(plist, &fs);
        assert!(
            out.contains("/Users/alice/Library/Logs/x.log"),
            "path placeholder must be replaced, got: {out}"
        );
        assert!(
            out.contains("<key>USERNAME</key>"),
            "bare USERNAME key must NOT be rewritten, got: {out}"
        );
        assert!(
            out.contains("literal USERNAME value"),
            "bare USERNAME value must NOT be rewritten, got: {out}"
        );
    }

    /// `substitute_program_path` must rewrite the hardcoded bundle path
    /// placeholder to the resolved sibling binary so the installed plist
    /// works when launched from anywhere (Downloads, DMG mount,
    /// AppTranslocation).
    #[test]
    fn substitute_program_path_rewrites_hardcoded_app_path() {
        let plist = r#"<array>
    <string>/Applications/CopyPaste.app/Contents/MacOS/copypaste-daemon</string>
</array>"#;
        let new_path = PathBuf::from(
            "/private/var/folders/x/AppTranslocation/abc/d/CopyPaste.app/Contents/MacOS/copypaste-daemon",
        );
        let out = substitute_program_path(plist, &new_path);
        assert!(
            !out.contains("/Applications/CopyPaste.app/Contents/MacOS/copypaste-daemon"),
            "placeholder must be rewritten, got: {out}"
        );
        assert!(
            out.contains("/AppTranslocation/abc/d/CopyPaste.app/Contents/MacOS/copypaste-daemon"),
            "new path must appear, got: {out}"
        );
    }

    /// `render_plist` composes both substitutions.
    #[cfg(target_os = "macos")]
    #[test]
    fn render_plist_substitutes_both_username_and_program_path() {
        let tmp = tempfile::tempdir().unwrap();
        let exe = fake_app_exe(tmp.path());
        let fs = TempFs::new(PathBuf::from("/Users/alice"));
        let env = FakeEnv::never_alive(exe.clone());
        let plist = r#"<plist><dict>
    <key>ProgramArguments</key><array>
        <string>/Applications/CopyPaste.app/Contents/MacOS/copypaste-daemon</string>
    </array>
    <key>StandardOutPath</key><string>/Users/USERNAME/Library/Logs/CopyPaste/daemon.out.log</string>
</dict></plist>"#;
        let out = render_plist(plist, &fs, &env).unwrap();
        assert!(
            !out.contains("/Users/USERNAME"),
            "USERNAME placeholder must be substituted, got: {out}"
        );
        assert!(
            !out.contains("/Applications/CopyPaste.app/Contents/MacOS/copypaste-daemon"),
            "ProgramArguments placeholder must be substituted, got: {out}"
        );
        // Resolved daemon path is sibling of the UI exe.
        let expected_daemon = exe.parent().unwrap().join("copypaste-daemon");
        assert!(
            out.contains(&expected_daemon.display().to_string()),
            "resolved daemon path must appear, got: {out}\nexpected: {}",
            expected_daemon.display()
        );
    }

    /// Reinstall must trigger when the installed plist content drifts from
    /// the freshly rendered template (covers: bundled plist changed across
    /// releases, app moved on disk → ProgramArguments[0] must change).
    #[cfg(target_os = "macos")]
    #[test]
    fn autostart_reinstalls_plist_when_drift_detected() {
        let tmp = tempfile::tempdir().unwrap();
        let exe = fake_app_exe(tmp.path());
        let bundled = exe
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("Resources")
            .join(PLIST_FILENAME);

        let mut fs = TempFs::new(tmp.path().join("home"));
        // Bundled plist holds the placeholder for ProgramArguments.
        let bundled_content = r#"<plist><dict>
    <key>ProgramArguments</key><array>
        <string>/Applications/CopyPaste.app/Contents/MacOS/copypaste-daemon</string>
    </array>
    <key>StandardOutPath</key><string>/Users/USERNAME/Library/Logs/CopyPaste/daemon.out.log</string>
</dict></plist>"#;
        fs.seed(bundled, bundled_content.into());

        // Pre-existing installed plist with OLD content (drifted).
        let installed_path = tmp
            .path()
            .join("home/Library/LaunchAgents")
            .join(PLIST_FILENAME);
        fs.seed(
            installed_path.clone(),
            "<!-- old stale plist content -->".into(),
        );

        let mut runner = MockRunner::with_default_uid();
        let env = FakeEnv::alive_after(exe.clone(), 2);

        ensure_daemon_running_inner(&mut runner, &mut fs, &env)
            .expect("autostart should succeed on drift path");

        // The installed plist must now reflect the rendered template — no
        // longer the old stale content, and no placeholders left.
        let after = fs.read_to_string(&installed_path).unwrap();
        assert!(
            !after.contains("old stale plist content"),
            "stale plist must be overwritten, got: {after}"
        );
        assert!(
            !after.contains("/Users/USERNAME"),
            "USERNAME placeholder must be substituted on reinstall, got: {after}"
        );
        assert!(
            !after.contains("/Applications/CopyPaste.app/Contents/MacOS/copypaste-daemon"),
            "ProgramArguments placeholder must be rewritten on reinstall, got: {after}"
        );
    }

    /// `diagnose_launchd_failure` must surface a "launchd did not load"
    /// message when `launchctl print` fails (label never loaded), instead
    /// of the legacy generic "did not appear" string.
    #[cfg(target_os = "macos")]
    #[test]
    fn diagnose_failure_when_print_returns_not_loaded() {
        let mut runner = MockRunner::with_default_uid();
        // Override: launchctl print fails → label not loaded.
        runner.set(
            "launchctl print",
            false,
            "",
            "Could not find service \"com.copypaste.daemon\" in domain for port",
        );
        let msg = diagnose_launchd_failure(&mut runner, 501);
        assert!(
            msg.to_lowercase().contains("did not load"),
            "expected 'launchd did not load' diagnosis, got: {msg}"
        );
    }

    /// `diagnose_launchd_failure` must classify a `pid=0` + nonzero
    /// `last exit code` as a daemon crash.
    #[cfg(target_os = "macos")]
    #[test]
    fn diagnose_failure_classifies_crash_from_print_output() {
        let mut runner = MockRunner::with_default_uid();
        runner.set(
            "launchctl print",
            true,
            "com.copypaste.daemon = {\n\tpid = 0\n\tlast exit code = 137\n}",
            "",
        );
        let msg = diagnose_launchd_failure(&mut runner, 501);
        let lc = msg.to_lowercase();
        assert!(
            lc.contains("crash") && lc.contains("137"),
            "expected crash-with-exit-137 diagnosis, got: {msg}"
        );
    }

    #[test]
    fn parse_kv_number_extracts_first_match() {
        let blob = "foo = bar\n  pid = 42\n  last exit code = -1\n";
        assert_eq!(parse_kv_number(blob, "pid"), Some(42));
        assert_eq!(parse_kv_number(blob, "last exit code"), Some(-1));
        assert_eq!(parse_kv_number(blob, "missing"), None);
    }

    #[test]
    fn bundled_plist_path_walks_up_from_macos_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let exe = fake_app_exe(tmp.path());
        let env = FakeEnv::never_alive(exe.clone());
        let p = bundled_plist_path(&env).unwrap();
        assert!(p.ends_with("Contents/Resources/com.copypaste.daemon.plist"));
    }

    // -----------------------------------------------------------------------------
    // Beta hotfix: launchctl enable must run before bootstrap so a previously
    // bootout'd label (now on the disabled list) can recover on next app launch.
    // -----------------------------------------------------------------------------

    #[cfg(target_os = "macos")]
    #[test]
    fn autostart_calls_enable_before_bootstrap() {
        let tmp = tempfile::tempdir().unwrap();
        let exe = fake_app_exe(tmp.path());
        let bundled = exe
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("Resources")
            .join(PLIST_FILENAME);
        let mut fs = TempFs::new(tmp.path().join("home"));
        fs.seed(bundled, SAMPLE_PLIST.into());

        let mut runner = MockRunner::with_default_uid();
        let env = FakeEnv::alive_after(exe, 2);

        ensure_daemon_running_inner(&mut runner, &mut fs, &env)
            .expect("autostart should not error");

        // Find enable and bootstrap positions in call order.
        let enable_idx = runner.calls.iter().position(|(prog, args)| {
            prog == "launchctl" && args.first().map(|s| s.as_str()) == Some("enable")
        });
        let bootstrap_idx = runner.calls.iter().position(|(prog, args)| {
            prog == "launchctl" && args.first().map(|s| s.as_str()) == Some("bootstrap")
        });
        let enable_i = enable_idx.expect("enable must be called");
        let bootstrap_i = bootstrap_idx.expect("bootstrap must be called");
        assert!(
            enable_i < bootstrap_i,
            "enable must precede bootstrap, got calls: {:?}",
            runner.calls
        );

        // Enable target must be gui/<uid>/<label>.
        let enable_args = &runner.calls[enable_i].1;
        assert_eq!(enable_args[0], "enable");
        assert_eq!(enable_args[1], "gui/501/com.copypaste.daemon");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn autostart_skips_bootstrap_when_print_reports_already_loaded() {
        // `launchctl print` returns 0 → service is loaded. We must skip the
        // enable+bootstrap pair to avoid emitting a spurious error 37.
        let tmp = tempfile::tempdir().unwrap();
        let exe = fake_app_exe(tmp.path());
        let bundled = exe
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("Resources")
            .join(PLIST_FILENAME);
        let mut fs = TempFs::new(tmp.path().join("home"));
        fs.seed(bundled, SAMPLE_PLIST.into());

        let mut runner = MockRunner::with_default_uid();
        // Override default — print says loaded.
        runner.set("launchctl print", true, "", "");

        // Socket also alive (consistent with launchctl print success).
        let env = FakeEnv::always_alive(exe);

        let _status = ensure_daemon_running_inner(&mut runner, &mut fs, &env).unwrap();

        // With always_alive socket the fast-path short-circuits before any
        // launchctl call. Verify no launchctl invocations occurred at all.
        let any_launchctl = runner.calls.iter().any(|(prog, _)| prog == "launchctl");
        assert!(
            !any_launchctl,
            "fast-path must skip launchctl entirely, got: {:?}",
            runner.calls
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn autostart_error_5_failure_includes_enable_hint() {
        // Simulate: print = not loaded, enable succeeds, bootstrap fails with 5.
        // The returned FailedToStart message must include the recovery hint.
        let tmp = tempfile::tempdir().unwrap();
        let exe = fake_app_exe(tmp.path());
        let bundled = exe
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("Resources")
            .join(PLIST_FILENAME);
        let mut fs = TempFs::new(tmp.path().join("home"));
        fs.seed(bundled, SAMPLE_PLIST.into());

        let mut runner = MockRunner::with_default_uid();
        runner.set(
            "launchctl bootstrap",
            false,
            "",
            "Bootstrap failed: 5: Input/output error",
        );
        let env = FakeEnv::never_alive(exe);

        let status = ensure_daemon_running_inner(&mut runner, &mut fs, &env).unwrap();
        match status {
            DaemonStatus::FailedToStart(msg) => {
                assert!(
                    msg.contains("disabled") || msg.contains("launchctl enable"),
                    "expected enable hint for error 5, got: {msg}"
                );
            }
            other => panic!("expected FailedToStart for error 5, got {other:?}"),
        }
    }
}
