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

/// `launchctl print gui/<uid>/<label>` exits 0 iff the service is loaded.
/// Returns Ok(true) when loaded, Ok(false) otherwise. Network / IO errors
/// from spawning launchctl bubble up as Err.
fn is_loaded<R: CommandRunner>(runner: &mut R, uid: u32) -> Result<bool> {
    let target = format!("gui/{uid}/{LAUNCHD_LABEL}");
    let out = runner.run("launchctl", &["print".into(), OsString::from(&target)])?;
    Ok(out.success)
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

/// Translate raw `launchctl` failure text into actionable advice.
///
/// Launchctl prints things like `Bootstrap failed: 5: Input/output error` —
/// useless for non-launchd-experts. We recognise the common codes and replace
/// the message; otherwise we pass through the original + a diagnostic hint.
fn friendly_launchctl_error(uid: u32, op: &str, stderr: &str) -> String {
    let s = stderr.trim();
    // Error 5 = "Input/output error" — usually "service already loaded"
    if s.contains(": 5:") || s.contains("Input/output error") {
        return "daemon already running (launchctl error 5). \
                Run `copypaste daemon restart` if you want to reload it."
            .to_string();
    }
    // Error 36 = "Operation now in progress" / "service not loaded"
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

fn macos_start<R: CommandRunner, F: FsOps>(runner: &mut R, fs: &mut F) -> Result<()> {
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
        eprintln!(
            "daemon already running (label: {LAUNCHD_LABEL}, domain: gui/{uid}). No-op."
        );
        return Ok(());
    }

    let domain = format!("gui/{uid}");
    let out = runner.run(
        "launchctl",
        &["bootstrap".into(), OsString::from(&domain), plist.clone().into_os_string()],
    )?;
    if !out.success {
        bail!("{}", friendly_launchctl_error(uid, "bootstrap", &out.stderr));
    }
    eprintln!("daemon started (label: {LAUNCHD_LABEL}, domain: {domain})");
    Ok(())
}

fn macos_stop<R: CommandRunner>(runner: &mut R) -> Result<()> {
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

fn macos_install<R: CommandRunner, F: FsOps>(runner: &mut R, fs: &mut F) -> Result<()> {
    let src = packaged_plist_path(fs)?;
    let dst = user_plist_path(fs)?;
    let uid = macos_uid(runner)?;

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
        if !fs.exists(&src) {
            bail!(
                "packaged plist not found at {}. Run from the repo root.",
                src.display()
            );
        }
        if let Some(parent) = dst.parent() {
            fs.create_dir_all(parent)?;
        }
        fs.copy(&src, &dst)?;
        eprintln!("installed plist to {}", dst.display());
    } else {
        eprintln!("plist already present at {} (skipping copy)", dst.display());
    }

    // Plist is on disk now; if not yet loaded, bootstrap it.
    // (macos_start handles its own idempotency check too, but we've just confirmed it.)
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
            // Default: `launchctl print` reports NOT loaded (exit 1) so existing
            // tests that assume a fresh load still see the bootstrap path.
            responses.insert(
                ("launchctl".into(), "print".into()),
                (false, String::new(), "Could not find service".into()),
            );
            Self { calls: Vec::new(), responses }
        }

        /// Override the response for a specific (program, first_arg) pair.
        fn set_response(
            &mut self,
            program: &str,
            first_arg: &str,
            success: bool,
            stdout: &str,
            stderr: &str,
        ) {
            self.responses.insert(
                (program.into(), first_arg.into()),
                (success, stdout.into(), stderr.into()),
            );
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

        // Expect: `id -u`, `launchctl print gui/501/...` (idempotency probe), then
        // `launchctl bootstrap gui/501 <plist>`.
        assert_eq!(runner.calls.len(), 3, "expected 3 shell-outs, got {:?}", runner.calls);
        assert_eq!(runner.calls[0].program, "id");
        assert_eq!(runner.calls[0].args, vec!["-u"]);

        assert_eq!(runner.calls[1].program, "launchctl");
        assert_eq!(runner.calls[1].args[0], "print");
        assert_eq!(runner.calls[1].args[1], "gui/501/com.copypaste.daemon");

        assert_eq!(runner.calls[2].program, "launchctl");
        assert_eq!(runner.calls[2].args[0], "bootstrap");
        assert_eq!(runner.calls[2].args[1], "gui/501");
        assert_eq!(runner.calls[2].args[2], expected_plist().to_string_lossy());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn daemon_uninstall_removes_plist_then_bootout() {
        let mut runner = MockRunner::new();
        // Simulate "currently loaded" so stop actually issues bootout.
        let mut fs = MockFs::new().with_existing(expected_plist());
        runner.set_response("launchctl", "print", true, "", "");

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

    // -----------------------------------------------------------------------------
    // Beta hotfix: idempotency + root refusal + friendly error translation
    // -----------------------------------------------------------------------------

    #[cfg(target_os = "macos")]
    #[test]
    fn start_is_idempotent_when_already_running() {
        let mut runner = MockRunner::new();
        // Pretend daemon is already loaded.
        runner.set_response("launchctl", "print", true, "", "");
        let mut fs = MockFs::new().with_existing(expected_plist());

        dispatch(DaemonAction::Start, &mut runner, &mut fs)
            .expect("start must be idempotent when already loaded");

        // Must NOT have issued bootstrap.
        let bootstrap_called = runner
            .calls
            .iter()
            .any(|c| c.program == "launchctl" && c.args.first().map(|s| s.as_str()) == Some("bootstrap"));
        assert!(
            !bootstrap_called,
            "start must not re-bootstrap when already loaded, got: {:?}",
            runner.calls
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn install_is_idempotent_when_plist_exists_and_loaded() {
        let mut runner = MockRunner::new();
        runner.set_response("launchctl", "print", true, "", "");
        // Plist already present at destination.
        let mut fs = MockFs::new().with_existing(expected_plist());

        dispatch(DaemonAction::Install, &mut runner, &mut fs)
            .expect("install must succeed as no-op when already installed+loaded");

        // No copy should have happened.
        assert!(fs.copies.is_empty(), "no copy expected, got {:?}", fs.copies);
        // No bootstrap either.
        let bootstrap_called = runner
            .calls
            .iter()
            .any(|c| c.program == "launchctl" && c.args.first().map(|s| s.as_str()) == Some("bootstrap"));
        assert!(!bootstrap_called, "no bootstrap expected, got: {:?}", runner.calls);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn install_skips_copy_when_plist_present_but_not_loaded() {
        // plist present, but launchctl print says not loaded → expect bootstrap, no copy.
        let mut runner = MockRunner::new();
        let mut fs = MockFs::new().with_existing(expected_plist());
        // default print response = not loaded

        dispatch(DaemonAction::Install, &mut runner, &mut fs)
            .expect("install ok when plist present but not loaded");

        assert!(fs.copies.is_empty(), "expected no copy, got {:?}", fs.copies);
        let bootstrap_called = runner
            .calls
            .iter()
            .any(|c| c.args.first().map(|s| s.as_str()) == Some("bootstrap"));
        assert!(bootstrap_called, "expected bootstrap, got: {:?}", runner.calls);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn start_refuses_when_running_as_root() {
        let mut runner = MockRunner::new();
        runner.set_response("id", "-u", true, "0\n", "");
        let mut fs = MockFs::new().with_existing(expected_plist());

        let err = dispatch(DaemonAction::Start, &mut runner, &mut fs)
            .expect_err("start must refuse under root");
        let msg = err.to_string();
        assert!(
            msg.contains("sudo") && (msg.contains("root") || msg.contains("user")),
            "expected sudo/root refusal, got: {msg}"
        );

        // Must NOT have attempted bootstrap.
        let bootstrap_called = runner
            .calls
            .iter()
            .any(|c| c.args.first().map(|s| s.as_str()) == Some("bootstrap"));
        assert!(!bootstrap_called, "no bootstrap expected under root, got: {:?}", runner.calls);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn install_refuses_when_running_as_root() {
        let mut runner = MockRunner::new();
        runner.set_response("id", "-u", true, "0\n", "");
        let mut fs = MockFs::new();

        let err = dispatch(DaemonAction::Install, &mut runner, &mut fs)
            .expect_err("install must refuse under root");
        assert!(
            err.to_string().contains("sudo"),
            "expected sudo refusal, got: {err}"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn error_5_translates_to_already_running_message() {
        let msg = friendly_launchctl_error(
            501,
            "bootstrap",
            "Bootstrap failed: 5: Input/output error",
        );
        assert!(
            msg.to_lowercase().contains("already running"),
            "expected friendly 'already running' text, got: {msg}"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn error_36_translates_to_not_running_message() {
        let msg = friendly_launchctl_error(
            501,
            "bootout",
            "Boot-out failed: 36: Could not find service",
        );
        assert!(
            msg.to_lowercase().contains("not running"),
            "expected friendly 'not running' text, got: {msg}"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn error_125_translates_to_wrong_domain_message() {
        let msg = friendly_launchctl_error(
            0,
            "bootstrap",
            "Bootstrap failed: 125: Domain does not support specified action",
        );
        assert!(
            msg.contains("sudo") || msg.contains("domain"),
            "expected sudo/domain advice, got: {msg}"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn unknown_error_passes_through_with_diagnostic_hint() {
        let msg = friendly_launchctl_error(501, "bootstrap", "Something weird: 99: Mystery");
        assert!(msg.contains("Mystery"), "original text must remain, got: {msg}");
        assert!(
            msg.contains("launchctl print"),
            "diagnostic hint must be present, got: {msg}"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn stop_is_idempotent_when_not_loaded() {
        let mut runner = MockRunner::new();
        // default print response: not loaded
        let mut fs = MockFs::new().with_existing(expected_plist());

        dispatch(DaemonAction::Stop, &mut runner, &mut fs)
            .expect("stop must succeed as no-op when not loaded");

        // Must NOT have issued bootout.
        let bootout_called = runner
            .calls
            .iter()
            .any(|c| c.program == "launchctl" && c.args.first().map(|s| s.as_str()) == Some("bootout"));
        assert!(!bootout_called, "no bootout expected, got: {:?}", runner.calls);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn stop_issues_bootout_when_loaded() {
        let mut runner = MockRunner::new();
        runner.set_response("launchctl", "print", true, "", "");
        let mut fs = MockFs::new().with_existing(expected_plist());

        dispatch(DaemonAction::Stop, &mut runner, &mut fs).expect("stop ok when loaded");

        let bootout_called = runner
            .calls
            .iter()
            .any(|c| c.args.first().map(|s| s.as_str()) == Some("bootout"));
        assert!(bootout_called, "expected bootout, got: {:?}", runner.calls);
    }
}
