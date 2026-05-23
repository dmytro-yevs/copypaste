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
fn friendly_launchctl_error(uid: u32, op: &str, stderr: &str) -> String {
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
    // Note: do NOT resolve the source plist here — `packaged_plist_path` bails
    // when no candidate exists. If the dst already has the plist, we don't
    // need the src at all, so resolve lazily below.
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
    fn current_exe(&self) -> Result<PathBuf>;
    fn exists(&self, path: &Path) -> bool;
    fn create_dir_all(&mut self, path: &Path) -> Result<()>;
    fn remove_file(&mut self, path: &Path) -> Result<()>;
    fn read_to_string(&self, path: &Path) -> Result<String>;
    fn write(&mut self, path: &Path, content: &str) -> Result<()>;
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
    fn current_exe(&self) -> Result<PathBuf> {
        Ok(std::env::current_exe()?)
    }
    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }
    fn create_dir_all(&mut self, path: &Path) -> Result<()> {
        std::fs::create_dir_all(path)?;
        Ok(())
    }
    fn remove_file(&mut self, path: &Path) -> Result<()> {
        std::fs::remove_file(path)?;
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
            Self {
                calls: Vec::new(),
                responses,
            }
        }

        /// Override the response for a specific (program, first_arg) pair.
        #[allow(dead_code)]
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

    struct MockFs {
        home: PathBuf,
        cwd: PathBuf,
        exe: PathBuf,
        existing: HashSet<PathBuf>,
        created_dirs: Vec<PathBuf>,
        removed: Vec<PathBuf>,
        files: std::collections::HashMap<PathBuf, String>,
    }

    impl MockFs {
        #[allow(dead_code)]
        fn new() -> Self {
            Self {
                home: PathBuf::from("/Users/test"),
                cwd: PathBuf::from("/repo"),
                // Default: pretend we're running from target/release/copypaste
                // in a repo at `/repo`, so the dev-path candidate resolves to
                // `/repo/packaging/macos/com.copypaste.daemon.plist`.
                exe: PathBuf::from("/repo/target/release/copypaste"),
                existing: HashSet::new(),
                created_dirs: Vec::new(),
                removed: Vec::new(),
                files: std::collections::HashMap::new(),
            }
        }
        #[allow(dead_code)]
        fn with_existing(mut self, p: impl Into<PathBuf>) -> Self {
            self.existing.insert(p.into());
            self
        }
        #[allow(dead_code)]
        fn with_file(mut self, p: impl Into<PathBuf>, content: impl Into<String>) -> Self {
            let p = p.into();
            self.existing.insert(p.clone());
            self.files.insert(p, content.into());
            self
        }
        #[allow(dead_code)]
        fn with_exe(mut self, exe: impl Into<PathBuf>) -> Self {
            self.exe = exe.into();
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
        fn current_exe(&self) -> Result<PathBuf> {
            Ok(self.exe.clone())
        }
        fn exists(&self, path: &Path) -> bool {
            self.existing.contains(path)
        }
        fn create_dir_all(&mut self, path: &Path) -> Result<()> {
            self.created_dirs.push(path.to_path_buf());
            Ok(())
        }
        fn remove_file(&mut self, path: &Path) -> Result<()> {
            self.removed.push(path.to_path_buf());
            self.existing.remove(path);
            self.files.remove(path);
            Ok(())
        }
        fn read_to_string(&self, path: &Path) -> Result<String> {
            self.files
                .get(path)
                .cloned()
                .ok_or_else(|| anyhow!("file not seeded: {}", path.display()))
        }
        fn write(&mut self, path: &Path, content: &str) -> Result<()> {
            self.existing.insert(path.to_path_buf());
            self.files.insert(path.to_path_buf(), content.to_string());
            Ok(())
        }
    }

    #[allow(dead_code)]
    fn expected_plist() -> PathBuf {
        PathBuf::from("/Users/test/Library/LaunchAgents/com.copypaste.daemon.plist")
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn daemon_start_invokes_launchctl_with_correct_args() {
        let mut runner = MockRunner::new();
        let mut fs = MockFs::new().with_existing(expected_plist());

        dispatch(DaemonAction::Start, &mut runner, &mut fs).expect("start should succeed");

        // Expect: `id -u`, `launchctl print gui/501/...` (idempotency probe),
        // `launchctl enable gui/501/...` (re-enable in case label was disabled),
        // then `launchctl bootstrap gui/501 <plist>`.
        assert_eq!(
            runner.calls.len(),
            4,
            "expected 4 shell-outs, got {:?}",
            runner.calls
        );
        assert_eq!(runner.calls[0].program, "id");
        assert_eq!(runner.calls[0].args, vec!["-u"]);

        assert_eq!(runner.calls[1].program, "launchctl");
        assert_eq!(runner.calls[1].args[0], "print");
        assert_eq!(runner.calls[1].args[1], "gui/501/com.copypaste.daemon");

        assert_eq!(runner.calls[2].program, "launchctl");
        assert_eq!(runner.calls[2].args[0], "enable");
        assert_eq!(runner.calls[2].args[1], "gui/501/com.copypaste.daemon");

        assert_eq!(runner.calls[3].program, "launchctl");
        assert_eq!(runner.calls[3].args[0], "bootstrap");
        assert_eq!(runner.calls[3].args[1], "gui/501");
        assert_eq!(runner.calls[3].args[2], expected_plist().to_string_lossy());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn daemon_uninstall_removes_plist_then_bootout() {
        let mut runner = MockRunner::new();
        // Simulate "currently loaded" so stop actually issues bootout.
        let mut fs = MockFs::new().with_existing(expected_plist());
        runner.set_response("launchctl", "print", true, "", "");

        dispatch(DaemonAction::Uninstall, &mut runner, &mut fs).expect("uninstall should succeed");

        // bootout must have been attempted
        let bootout_called = runner.calls.iter().any(|c| {
            c.program == "launchctl" && c.args.first().map(|s| s.as_str()) == Some("bootout")
        });
        assert!(
            bootout_called,
            "expected launchctl bootout, got {:?}",
            runner.calls
        );

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
            msg.contains("not yet wired")
                || msg.contains("not yet implemented")
                || msg.contains("unsupported"),
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
        assert!(
            err.to_string().contains("install"),
            "expected install hint, got: {err}"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn daemon_install_copies_plist_then_bootstraps() {
        let mut runner = MockRunner::new();
        let src = PathBuf::from("/repo/packaging/macos/com.copypaste.daemon.plist");
        let plist_template = r#"<?xml version="1.0"?>
<plist><dict>
    <key>StandardOutPath</key>
    <string>/Users/USERNAME/Library/Logs/CopyPaste/daemon.out.log</string>
</dict></plist>"#;
        let mut fs = MockFs::new().with_file(src.clone(), plist_template);

        dispatch(DaemonAction::Install, &mut runner, &mut fs).expect("install ok");

        // Install now reads+substitutes+writes (not raw copy) so we verify the
        // destination file contents instead of the copies vector.
        let written = fs
            .files
            .get(&expected_plist())
            .expect("plist must be written to destination");
        assert!(
            !written.contains("/Users/USERNAME"),
            "USERNAME placeholder must be substituted, got: {written}"
        );
        assert!(
            written.contains("/Users/test/Library/Logs/CopyPaste"),
            "expected substituted $HOME ({}), got: {written}",
            "/Users/test"
        );

        // bootstrap must follow
        let bootstrap_called = runner
            .calls
            .iter()
            .any(|c| c.args.first().map(|s| s.as_str()) == Some("bootstrap"));
        assert!(bootstrap_called);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn install_re_renders_stale_plist_with_username_placeholder() {
        // Plist already at destination but contains the unsubstituted
        // `/Users/USERNAME` token (older installs). The install command must
        // rewrite it in-place rather than skipping.
        let mut runner = MockRunner::new();
        let stale = r#"<?xml version="1.0"?>
<plist><dict>
    <key>StandardOutPath</key>
    <string>/Users/USERNAME/Library/Logs/CopyPaste/daemon.out.log</string>
</dict></plist>"#;
        let mut fs = MockFs::new().with_file(expected_plist(), stale);
        // launchctl print default = not loaded → install will fall through to bootstrap.

        dispatch(DaemonAction::Install, &mut runner, &mut fs).expect("install ok");

        let written = fs
            .files
            .get(&expected_plist())
            .expect("plist must still exist after re-render");
        assert!(
            !written.contains("/Users/USERNAME"),
            "stale USERNAME placeholder must be re-rendered, got: {written}"
        );
        assert!(
            written.contains("/Users/test/Library/Logs/CopyPaste"),
            "expected substituted $HOME, got: {written}"
        );
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
        let bootstrap_called = runner.calls.iter().any(|c| {
            c.program == "launchctl" && c.args.first().map(|s| s.as_str()) == Some("bootstrap")
        });
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

        // No write should have happened (file was already present + loaded).
        assert!(
            fs.files.is_empty(),
            "no write expected when already-installed+loaded, got {:?}",
            fs.files.keys().collect::<Vec<_>>()
        );
        // No bootstrap either.
        let bootstrap_called = runner.calls.iter().any(|c| {
            c.program == "launchctl" && c.args.first().map(|s| s.as_str()) == Some("bootstrap")
        });
        assert!(
            !bootstrap_called,
            "no bootstrap expected, got: {:?}",
            runner.calls
        );
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

        // Plist was already present (via `with_existing`, no file content seeded),
        // so no fresh write should have occurred — install must short-circuit the copy.
        assert!(
            fs.files.is_empty(),
            "expected no write, got {:?}",
            fs.files.keys().collect::<Vec<_>>()
        );
        let bootstrap_called = runner
            .calls
            .iter()
            .any(|c| c.args.first().map(|s| s.as_str()) == Some("bootstrap"));
        assert!(
            bootstrap_called,
            "expected bootstrap, got: {:?}",
            runner.calls
        );
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
        assert!(
            !bootstrap_called,
            "no bootstrap expected under root, got: {:?}",
            runner.calls
        );
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
    fn error_5_translates_to_bootstrap_failed_with_enable_hint() {
        // Error 5 is bootstrap genuine failure (most often: label on disabled
        // list). Should mention the enable hint, NOT "already running".
        let msg =
            friendly_launchctl_error(501, "bootstrap", "Bootstrap failed: 5: Input/output error");
        assert!(
            !msg.to_lowercase().contains("already running"),
            "error 5 must NOT be classified as 'already running', got: {msg}"
        );
        assert!(
            msg.to_lowercase().contains("disabled") || msg.contains("launchctl enable"),
            "expected 'enable' hint for error 5, got: {msg}"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn error_37_translates_to_already_running_message() {
        // Error 37 = ALREADY_BOOTSTRAPPED is the canonical "already loaded".
        let msg = friendly_launchctl_error(
            501,
            "bootstrap",
            "Bootstrap failed: 37: Operation already in progress",
        );
        assert!(
            msg.to_lowercase().contains("already running"),
            "expected friendly 'already running' text for error 37, got: {msg}"
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
        assert!(
            msg.contains("Mystery"),
            "original text must remain, got: {msg}"
        );
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
        let bootout_called = runner.calls.iter().any(|c| {
            c.program == "launchctl" && c.args.first().map(|s| s.as_str()) == Some("bootout")
        });
        assert!(
            !bootout_called,
            "no bootout expected, got: {:?}",
            runner.calls
        );
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

    // -----------------------------------------------------------------------------
    // Beta hotfix #2: `launchctl enable` must be called before every bootstrap so
    // the daemon can recover even when the label is on launchd's disabled list.
    // -----------------------------------------------------------------------------

    #[cfg(target_os = "macos")]
    #[test]
    fn start_calls_enable_before_bootstrap() {
        let mut runner = MockRunner::new();
        let mut fs = MockFs::new().with_existing(expected_plist());

        dispatch(DaemonAction::Start, &mut runner, &mut fs).expect("start ok");

        // Find positions of `enable` and `bootstrap` in launchctl call order.
        let enable_idx = runner.calls.iter().position(|c| {
            c.program == "launchctl" && c.args.first().map(|s| s.as_str()) == Some("enable")
        });
        let bootstrap_idx = runner.calls.iter().position(|c| {
            c.program == "launchctl" && c.args.first().map(|s| s.as_str()) == Some("bootstrap")
        });
        let enable_i = enable_idx.expect("enable must be called");
        let bootstrap_i = bootstrap_idx.expect("bootstrap must be called");
        assert!(
            enable_i < bootstrap_i,
            "enable must precede bootstrap, got calls: {:?}",
            runner.calls
        );

        // Enable must target the full gui/<uid>/<label> path.
        let enable_call = &runner.calls[enable_i];
        assert_eq!(enable_call.args[0], "enable");
        assert_eq!(enable_call.args[1], "gui/501/com.copypaste.daemon");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn enable_failure_does_not_abort_start() {
        // `launchctl enable` failures are ignored — they can happen for benign
        // reasons (label never seen by launchd yet) and any real problem will
        // surface on the subsequent `bootstrap` call.
        let mut runner = MockRunner::new();
        runner.set_response("launchctl", "enable", false, "", "some enable error");
        let mut fs = MockFs::new().with_existing(expected_plist());

        dispatch(DaemonAction::Start, &mut runner, &mut fs)
            .expect("start must not abort when enable fails");

        // bootstrap must still have been attempted.
        let bootstrap_called = runner
            .calls
            .iter()
            .any(|c| c.args.first().map(|s| s.as_str()) == Some("bootstrap"));
        assert!(
            bootstrap_called,
            "expected bootstrap to proceed despite enable failure"
        );
    }

    // -----------------------------------------------------------------------------
    // Beta hotfix #3: `daemon install` discovers plist via current_exe
    // (production .app bundle path) and falls back to dev / cwd paths.
    // -----------------------------------------------------------------------------

    #[cfg(target_os = "macos")]
    #[test]
    fn daemon_install_finds_plist_in_app_bundle() {
        // Simulate /Applications/CopyPaste.app layout.
        let exe = PathBuf::from("/Applications/CopyPaste.app/Contents/MacOS/copypaste");
        let bundle_plist = PathBuf::from(
            "/Applications/CopyPaste.app/Contents/Resources/com.copypaste.daemon.plist",
        );
        let mut runner = MockRunner::new();
        let mut fs = MockFs::new()
            .with_exe(exe)
            .with_file(bundle_plist.clone(), SAMPLE_PLIST_FOR_INSTALL);

        dispatch(DaemonAction::Install, &mut runner, &mut fs).expect("install ok");

        // Must have read from the bundle plist and written to user LaunchAgents.
        let written = fs
            .files
            .get(&expected_plist())
            .expect("plist must be written to user LaunchAgents");
        assert!(
            !written.contains("/Users/USERNAME"),
            "USERNAME placeholder must be substituted, got: {written}"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn daemon_install_finds_plist_via_repo_fallback() {
        // Dev path: target/release/copypaste — exe-derived candidate 2 hits
        // /repo/packaging/macos/com.copypaste.daemon.plist.
        let mut runner = MockRunner::new();
        let mut fs = MockFs::new().with_file(
            "/repo/packaging/macos/com.copypaste.daemon.plist",
            SAMPLE_PLIST_FOR_INSTALL,
        );

        dispatch(DaemonAction::Install, &mut runner, &mut fs).expect("install ok from dev path");

        assert!(
            fs.files.contains_key(&expected_plist()),
            "plist must be installed at user LaunchAgents, got: {:?}",
            fs.files.keys().collect::<Vec<_>>()
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn daemon_install_reports_clear_error_when_no_plist_anywhere() {
        // No plist seeded at any candidate path — must bail with a message
        // listing the paths it checked.
        let mut runner = MockRunner::new();
        let mut fs = MockFs::new();

        let err = dispatch(DaemonAction::Install, &mut runner, &mut fs)
            .expect_err("install must fail when no plist exists anywhere");
        let msg = err.to_string();
        assert!(
            msg.contains("not found") && msg.contains("Looked in"),
            "expected 'not found / Looked in' error, got: {msg}"
        );
        // Must enumerate at least the cwd-relative candidate.
        assert!(
            msg.contains("packaging/macos/com.copypaste.daemon.plist"),
            "expected cwd candidate in error, got: {msg}"
        );
    }

    #[allow(dead_code)]
    const SAMPLE_PLIST_FOR_INSTALL: &str = r#"<?xml version="1.0"?>
<plist><dict>
    <key>StandardOutPath</key>
    <string>/Users/USERNAME/Library/Logs/CopyPaste/daemon.out.log</string>
</dict></plist>"#;
}
