//! `copypaste daemon` — manage the background daemon process.
//!
//! Platform support:
//!   - macOS:   `launchctl bootstrap gui/<uid> <plist>` / `launchctl bootout gui/<uid>/<label>`
//!   - Linux:   `systemctl --user` (FROZEN — wiring documented, returns clear error)
//!   - Windows: `sc.exe` (FUTURE — returns clear error)
//!
//! All shell-outs are wrapped through `CommandRunner` so unit tests can assert the
//! constructed argv without actually invoking `launchctl` on the host.

use anyhow::{anyhow, bail, Result};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// Launchd label — must match `<key>Label</key>` in the plist.
const LAUNCHD_LABEL: &str = "com.copypaste.daemon";
/// Source plist shipped in the repo (used by `install`).
const PACKAGED_PLIST_RELATIVE: &str = "packaging/macos/com.copypaste.daemon.plist";
/// Per-user plist installation directory (macOS LaunchAgents).
const USER_LAUNCH_AGENTS_DIR: &str = "Library/LaunchAgents";

/// Public subcommand entry point. Dispatches to platform-specific logic via the
/// default `SystemRunner` (which actually shells out).
pub fn run(action: DaemonAction) -> Result<()> {
    let mut runner = SystemRunner;
    let mut fs = SystemFs;
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
        return unsupported_platform();
    }

    match action {
        DaemonAction::Start => macos_start(runner, fs),
        DaemonAction::Stop => macos_stop(runner),
        DaemonAction::Restart => {
            // bootout is allowed to fail (daemon may not be loaded)
            let _ = macos_stop(runner);
            macos_start(runner, fs)
        }
        DaemonAction::Install => macos_install(runner, fs),
        DaemonAction::Uninstall => macos_uninstall(runner, fs),
    }
}

fn unsupported_platform() -> Result<()> {
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

// --------------------------------------------------------------------------------------------
// macOS implementation
// --------------------------------------------------------------------------------------------

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

fn user_plist_path<F: FsOps>(fs: &F) -> Result<PathBuf> {
    let home = fs.home_dir().ok_or_else(|| anyhow!("could not determine $HOME"))?;
    Ok(home.join(USER_LAUNCH_AGENTS_DIR).join(format!("{LAUNCHD_LABEL}.plist")))
}

fn packaged_plist_path<F: FsOps>(fs: &F) -> Result<PathBuf> {
    // Resolve relative to current working dir — `copypaste daemon install` is expected
    // to be run from the repo root during dev. In packaged installs the user just
    // copies the plist manually.
    let cwd = fs.current_dir()?;
    Ok(cwd.join(PACKAGED_PLIST_RELATIVE))
}

fn macos_start<R: CommandRunner, F: FsOps>(runner: &mut R, fs: &mut F) -> Result<()> {
    let plist = user_plist_path(fs)?;
    if !fs.exists(&plist) {
        bail!(
            "plist not installed at {}. Run `copypaste daemon install` first.",
            plist.display()
        );
    }
    let uid = macos_uid(runner)?;
    let domain = format!("gui/{uid}");
    let out = runner.run(
        "launchctl",
        &["bootstrap".into(), OsString::from(&domain), plist.clone().into_os_string()],
    )?;
    if !out.success {
        bail!("launchctl bootstrap failed: {}", out.stderr.trim());
    }
    eprintln!("daemon started (label: {LAUNCHD_LABEL}, domain: {domain})");
    Ok(())
}

fn macos_stop<R: CommandRunner>(runner: &mut R) -> Result<()> {
    let uid = macos_uid(runner)?;
    let target = format!("gui/{uid}/{LAUNCHD_LABEL}");
    let out = runner.run("launchctl", &["bootout".into(), OsString::from(&target)])?;
    if !out.success {
        bail!("launchctl bootout failed: {}", out.stderr.trim());
    }
    eprintln!("daemon stopped (target: {target})");
    Ok(())
}

fn macos_install<R: CommandRunner, F: FsOps>(runner: &mut R, fs: &mut F) -> Result<()> {
    let src = packaged_plist_path(fs)?;
    if !fs.exists(&src) {
        bail!(
            "packaged plist not found at {}. Run from the repo root.",
            src.display()
        );
    }
    let dst = user_plist_path(fs)?;
    if let Some(parent) = dst.parent() {
        fs.create_dir_all(parent)?;
    }
    fs.copy(&src, &dst)?;
    eprintln!("installed plist to {}", dst.display());
    macos_start(runner, fs)
}

fn macos_uninstall<R: CommandRunner, F: FsOps>(runner: &mut R, fs: &mut F) -> Result<()> {
    // Best-effort bootout first; ignore errors (daemon may already be unloaded).
    let _ = macos_stop(runner);
    let plist = user_plist_path(fs)?;
    if fs.exists(&plist) {
        fs.remove_file(&plist)?;
        eprintln!("removed plist at {}", plist.display());
    } else {
        eprintln!("no plist to remove at {}", plist.display());
    }
    Ok(())
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
    fn run(&mut self, program: &str, args: &[OsString]) -> Result<CommandOutput>;
}

pub(crate) trait FsOps {
    fn home_dir(&self) -> Option<PathBuf>;
    fn current_dir(&self) -> Result<PathBuf>;
    fn exists(&self, path: &Path) -> bool;
    fn create_dir_all(&mut self, path: &Path) -> Result<()>;
    fn copy(&mut self, from: &Path, to: &Path) -> Result<()>;
    fn remove_file(&mut self, path: &Path) -> Result<()>;
}

#[derive(Default)]
struct SystemRunner;

impl CommandRunner for SystemRunner {
    fn run(&mut self, program: &str, args: &[OsString]) -> Result<CommandOutput> {
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
    fn current_dir(&self) -> Result<PathBuf> {
        Ok(std::env::current_dir()?)
    }
    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }
    fn create_dir_all(&mut self, path: &Path) -> Result<()> {
        std::fs::create_dir_all(path)?;
        Ok(())
    }
    fn copy(&mut self, from: &Path, to: &Path) -> Result<()> {
        std::fs::copy(from, to)?;
        Ok(())
    }
    fn remove_file(&mut self, path: &Path) -> Result<()> {
        std::fs::remove_file(path)?;
        Ok(())
    }
}

// --------------------------------------------------------------------------------------------
// Tests
// --------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// Recorded shell-out invocation.
    #[derive(Debug, Clone, PartialEq, Eq)]
    struct Invocation {
        program: String,
        args: Vec<String>,
    }

    struct MockRunner {
        calls: Vec<Invocation>,
        /// (program, first_arg) -> (success, stdout, stderr)
        responses: std::collections::HashMap<(String, String), (bool, String, String)>,
    }

    impl MockRunner {
        fn new() -> Self {
            let mut responses = std::collections::HashMap::new();
            // Default: `id -u` returns 501
            responses.insert(
                ("id".into(), "-u".into()),
                (true, "501\n".into(), String::new()),
            );
            // Default: launchctl succeeds
            responses.insert(
                ("launchctl".into(), "bootstrap".into()),
                (true, String::new(), String::new()),
            );
            responses.insert(
                ("launchctl".into(), "bootout".into()),
                (true, String::new(), String::new()),
            );
            Self { calls: Vec::new(), responses }
        }
    }

    impl CommandRunner for MockRunner {
        fn run(&mut self, program: &str, args: &[OsString]) -> Result<CommandOutput> {
            let args_str: Vec<String> = args
                .iter()
                .map(|a| a.to_string_lossy().into_owned())
                .collect();
            self.calls.push(Invocation {
                program: program.into(),
                args: args_str.clone(),
            });
            let key = (
                program.to_string(),
                args_str.first().cloned().unwrap_or_default(),
            );
            let (success, stdout, stderr) = self
                .responses
                .get(&key)
                .cloned()
                .unwrap_or((true, String::new(), String::new()));
            Ok(CommandOutput { success, stdout, stderr })
        }
    }

    struct MockFs {
        home: PathBuf,
        cwd: PathBuf,
        existing: HashSet<PathBuf>,
        created_dirs: Vec<PathBuf>,
        copies: Vec<(PathBuf, PathBuf)>,
        removed: Vec<PathBuf>,
    }

    impl MockFs {
        fn new() -> Self {
            Self {
                home: PathBuf::from("/Users/test"),
                cwd: PathBuf::from("/repo"),
                existing: HashSet::new(),
                created_dirs: Vec::new(),
                copies: Vec::new(),
                removed: Vec::new(),
            }
        }
        fn with_existing(mut self, p: impl Into<PathBuf>) -> Self {
            self.existing.insert(p.into());
            self
        }
    }

    impl FsOps for MockFs {
        fn home_dir(&self) -> Option<PathBuf> {
            Some(self.home.clone())
        }
        fn current_dir(&self) -> Result<PathBuf> {
            Ok(self.cwd.clone())
        }
        fn exists(&self, path: &Path) -> bool {
            self.existing.contains(path)
        }
        fn create_dir_all(&mut self, path: &Path) -> Result<()> {
            self.created_dirs.push(path.to_path_buf());
            Ok(())
        }
        fn copy(&mut self, from: &Path, to: &Path) -> Result<()> {
            self.copies.push((from.to_path_buf(), to.to_path_buf()));
            self.existing.insert(to.to_path_buf());
            Ok(())
        }
        fn remove_file(&mut self, path: &Path) -> Result<()> {
            self.removed.push(path.to_path_buf());
            self.existing.remove(path);
            Ok(())
        }
    }

    fn expected_plist() -> PathBuf {
        PathBuf::from("/Users/test/Library/LaunchAgents/com.copypaste.daemon.plist")
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn daemon_start_invokes_launchctl_with_correct_args() {
        let mut runner = MockRunner::new();
        let mut fs = MockFs::new().with_existing(expected_plist());

        dispatch(DaemonAction::Start, &mut runner, &mut fs).expect("start should succeed");

        // Expect: `id -u` then `launchctl bootstrap gui/501 <plist>`
        assert_eq!(runner.calls.len(), 2, "expected 2 shell-outs, got {:?}", runner.calls);
        assert_eq!(runner.calls[0].program, "id");
        assert_eq!(runner.calls[0].args, vec!["-u"]);

        assert_eq!(runner.calls[1].program, "launchctl");
        assert_eq!(runner.calls[1].args[0], "bootstrap");
        assert_eq!(runner.calls[1].args[1], "gui/501");
        assert_eq!(runner.calls[1].args[2], expected_plist().to_string_lossy());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn daemon_uninstall_removes_plist_then_bootout() {
        let mut runner = MockRunner::new();
        let mut fs = MockFs::new().with_existing(expected_plist());

        dispatch(DaemonAction::Uninstall, &mut runner, &mut fs)
            .expect("uninstall should succeed");

        // bootout must have been attempted
        let bootout_called = runner
            .calls
            .iter()
            .any(|c| c.program == "launchctl" && c.args.first().map(|s| s.as_str()) == Some("bootout"));
        assert!(bootout_called, "expected launchctl bootout, got {:?}", runner.calls);

        // plist must have been removed
        assert_eq!(fs.removed, vec![expected_plist()]);
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn unsupported_platform_returns_clear_error_not_panic() {
        let mut runner = MockRunner::new();
        let mut fs = MockFs::new();
        let err = dispatch(DaemonAction::Start, &mut runner, &mut fs)
            .expect_err("non-macos must return error");
        let msg = err.to_string();
        assert!(
            msg.contains("not yet wired") || msg.contains("not yet implemented") || msg.contains("unsupported"),
            "expected clear platform error, got: {msg}"
        );
    }

    /// Cross-platform variant of the unsupported-platform test: directly exercise
    /// the helper so we get coverage on macOS hosts too.
    #[test]
    fn unsupported_platform_helper_returns_error() {
        // On macOS this helper still returns Err (it's a catch-all for non-macos),
        // because cfg!(target_os = "macos") inside dispatch gates *before* calling it.
        // We test it directly here.
        let r = unsupported_platform();
        assert!(r.is_err(), "unsupported_platform must always return Err");
        let msg = r.unwrap_err().to_string();
        assert!(!msg.is_empty(), "error message must be non-empty");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn daemon_start_errors_when_plist_missing() {
        let mut runner = MockRunner::new();
        let mut fs = MockFs::new(); // plist not registered as existing
        let err = dispatch(DaemonAction::Start, &mut runner, &mut fs)
            .expect_err("start must fail when plist missing");
        assert!(err.to_string().contains("install"), "expected install hint, got: {err}");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn daemon_install_copies_plist_then_bootstraps() {
        let mut runner = MockRunner::new();
        let src = PathBuf::from("/repo/packaging/macos/com.copypaste.daemon.plist");
        let mut fs = MockFs::new().with_existing(src.clone());

        dispatch(DaemonAction::Install, &mut runner, &mut fs).expect("install ok");

        assert_eq!(fs.copies.len(), 1);
        assert_eq!(fs.copies[0].0, src);
        assert_eq!(fs.copies[0].1, expected_plist());
        // bootstrap must follow
        let bootstrap_called = runner
            .calls
            .iter()
            .any(|c| c.args.first().map(|s| s.as_str()) == Some("bootstrap"));
        assert!(bootstrap_called);
    }
}
