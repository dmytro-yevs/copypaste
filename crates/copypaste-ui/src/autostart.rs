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

    // Step 2: ensure plist installed.
    let dst_plist = user_plist_path(fs)?;
    if !fs.exists(&dst_plist) {
        let src_plist = bundled_plist_path(env)?;
        if !fs.exists(&src_plist) {
            return Ok(DaemonStatus::FailedToStart(format!(
                "bundled plist missing at {}",
                src_plist.display()
            )));
        }
        if let Some(parent) = dst_plist.parent() {
            fs.create_dir_all(parent)
                .with_context(|| format!("create {}", parent.display()))?;
        }
        let raw = fs
            .read_to_string(&src_plist)
            .with_context(|| format!("read {}", src_plist.display()))?;
        let rendered = substitute_username(&raw, fs);
        fs.write(&dst_plist, &rendered)
            .with_context(|| format!("write {}", dst_plist.display()))?;
    }

    // Step 3: launchctl bootstrap gui/<uid> <plist>.
    let uid = current_uid(runner)?;
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
        // `launchctl bootstrap` is idempotent only if the service is not
        // already loaded. Treat the "Bootstrap failed: 5: Input/output error"
        // (already loaded) case as success and let the ping confirm it.
        let stderr = out.stderr.trim().to_lowercase();
        let benign_already_loaded = stderr.contains("service already loaded")
            || stderr.contains("already bootstrapped")
            || out.stderr.contains("Bootstrap failed: 37");
        if !benign_already_loaded {
            return Ok(DaemonStatus::FailedToStart(format!(
                "launchctl bootstrap {} {}: {}",
                domain,
                dst_plist.display(),
                out.stderr.trim()
            )));
        }
    }

    // Step 4: wait ~2s, retry ping a few times.
    for _ in 0..10 {
        env.sleep(Duration::from_millis(200));
        if ipc_ping(&socket_path, env) {
            return Ok(DaemonStatus::Started);
        }
    }

    Ok(DaemonStatus::FailedToStart(
        "daemon socket did not appear within 2s after bootstrap".into(),
    ))
}

// --------------------------------------------------------------------------------------------
// Helpers
// --------------------------------------------------------------------------------------------

pub(crate) fn daemon_socket_path<F: FsOps>(fs: &F) -> Result<PathBuf> {
    let home = fs.home_dir().ok_or_else(|| anyhow!("could not determine $HOME"))?;
    Ok(home
        .join("Library/Application Support/CopyPaste/daemon.sock"))
}

pub(crate) fn user_plist_path<F: FsOps>(fs: &F) -> Result<PathBuf> {
    let home = fs.home_dir().ok_or_else(|| anyhow!("could not determine $HOME"))?;
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
pub(crate) fn substitute_username<F: FsOps>(plist: &str, fs: &F) -> String {
    let Some(home) = fs.home_dir() else { return plist.to_string(); };
    plist.replace("/Users/USERNAME", &home.display().to_string())
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
            s.responses.insert(
                "id -u".into(),
                (true, "501\n".into(), String::new()),
            );
            s.responses.insert(
                "launchctl bootstrap".into(),
                (true, String::new(), String::new()),
            );
            s
        }
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
            let (success, stdout, stderr) = self
                .responses
                .get(&key)
                .cloned()
                .unwrap_or((true, String::new(), String::new()));
            Ok(CommandOutput { success, stdout, stderr })
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
            Self { home, files: HashMap::new() }
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
            .parent().unwrap()           // MacOS
            .parent().unwrap()           // Contents
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
            .parent().unwrap()
            .parent().unwrap()
            .join("Resources")
            .join(PLIST_FILENAME);

        let mut fs = TempFs::new(tmp.path().join("home"));
        fs.seed(bundled, SAMPLE_PLIST.into());

        let mut runner = MockRunner::with_default_uid();
        // Socket never comes back — exercise the FailedToStart branch as well
        // as the launchctl invocation.
        let env = FakeEnv::never_alive(exe);

        let status = ensure_daemon_running_inner(&mut runner, &mut fs, &env).unwrap();

        // Must have called `id -u` then `launchctl bootstrap gui/501 <plist>`.
        let programs: Vec<&str> = runner.calls.iter().map(|c| c.0.as_str()).collect();
        assert_eq!(programs, vec!["id", "launchctl"]);
        let launchctl_args = &runner.calls[1].1;
        assert_eq!(launchctl_args[0], "bootstrap");
        assert_eq!(launchctl_args[1], "gui/501");
        assert!(
            launchctl_args[2].ends_with(PLIST_FILENAME),
            "expected plist path as 3rd arg, got {launchctl_args:?}"
        );

        // Socket never recovered → FailedToStart with informative message.
        match status {
            DaemonStatus::FailedToStart(msg) => {
                assert!(
                    msg.contains("did not appear"),
                    "expected socket-timeout message, got: {msg}"
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

    #[test]
    fn bundled_plist_path_walks_up_from_macos_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let exe = fake_app_exe(tmp.path());
        let env = FakeEnv::never_alive(exe.clone());
        let p = bundled_plist_path(&env).unwrap();
        assert!(p.ends_with("Contents/Resources/com.copypaste.daemon.plist"));
    }
}
