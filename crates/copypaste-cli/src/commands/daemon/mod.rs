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
//!
//! ## Module layout
//!
//! - [`runner`]   — `CommandRunner` / `FsOps` traits + production impls.
//! - [`platform`] — macOS launchd functions; `unsupported_platform` for others.

mod platform;
mod runner;

// Re-export the testability traits so the inline test module can implement them
// for mock types without having to qualify the path.
pub(crate) use runner::{CommandRunner, FsOps};

use anyhow::Result;

/// Public subcommand entry point. Dispatches to platform-specific logic via the
/// default `SystemRunner` (which actually shells out).
pub fn run(action: DaemonAction) -> Result<()> {
    let mut runner = runner::SystemRunner;
    let mut fs = runner::SystemFs;
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
        return platform::unsupported_platform();
    }

    match action {
        DaemonAction::Start => platform::macos_start(runner, fs),
        DaemonAction::Stop => platform::macos_stop(runner),
        DaemonAction::Restart => {
            // bootout is allowed to fail (daemon may not be loaded)
            let _ = platform::macos_stop(runner);
            platform::macos_start(runner, fs)
        }
        DaemonAction::Install => platform::macos_install(runner, fs),
        DaemonAction::Uninstall => platform::macos_uninstall(runner, fs),
    }
}

// --------------------------------------------------------------------------------------------
// Tests
// --------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::platform::unsupported_platform;
    // `friendly_launchctl_error` is exercised only by the macOS launchctl tests
    // below; gate the import so non-macOS targets (Linux CI) don't flag it unused
    // under -D warnings.
    #[cfg(target_os = "macos")]
    use super::platform::friendly_launchctl_error;
    use super::runner::CommandOutput;
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
        // Builder helper — not every test uses it, so the compiler sees it as
        // dead in some configurations.
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
        fn run(
            &mut self,
            program: &str,
            args: &[std::ffi::OsString],
        ) -> anyhow::Result<CommandOutput> {
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
        home: std::path::PathBuf,
        cwd: std::path::PathBuf,
        exe: std::path::PathBuf,
        existing: HashSet<std::path::PathBuf>,
        created_dirs: Vec<std::path::PathBuf>,
        removed: Vec<std::path::PathBuf>,
        files: std::collections::HashMap<std::path::PathBuf, String>,
    }

    impl MockFs {
        // Builder pattern — individual builder methods are not called by every
        // test, so unused-in-this-cfg warnings appear on some platforms.
        #[allow(dead_code)]
        fn new() -> Self {
            Self {
                home: std::path::PathBuf::from("/Users/test"),
                cwd: std::path::PathBuf::from("/repo"),
                // Default: pretend we're running from target/release/copypaste
                // in a repo at `/repo`, so the dev-path candidate resolves to
                // `/repo/packaging/macos/com.copypaste.daemon.plist`.
                exe: std::path::PathBuf::from("/repo/target/release/copypaste"),
                existing: HashSet::new(),
                created_dirs: Vec::new(),
                removed: Vec::new(),
                files: std::collections::HashMap::new(),
            }
        }
        #[allow(dead_code)] // builder helper, not called by every test
        fn with_existing(mut self, p: impl Into<std::path::PathBuf>) -> Self {
            self.existing.insert(p.into());
            self
        }
        #[allow(dead_code)] // builder helper, not called by every test
        fn with_file(
            mut self,
            p: impl Into<std::path::PathBuf>,
            content: impl Into<String>,
        ) -> Self {
            let p = p.into();
            self.existing.insert(p.clone());
            self.files.insert(p, content.into());
            self
        }
        #[allow(dead_code)] // builder helper, not called by every test
        fn with_exe(mut self, exe: impl Into<std::path::PathBuf>) -> Self {
            self.exe = exe.into();
            self
        }
    }

    impl FsOps for MockFs {
        fn home_dir(&self) -> Option<std::path::PathBuf> {
            Some(self.home.clone())
        }
        fn current_dir(&self) -> anyhow::Result<std::path::PathBuf> {
            Ok(self.cwd.clone())
        }
        fn current_exe(&self) -> anyhow::Result<std::path::PathBuf> {
            Ok(self.exe.clone())
        }
        fn exists(&self, path: &std::path::Path) -> bool {
            self.existing.contains(path)
        }
        fn create_dir_all(&mut self, path: &std::path::Path) -> anyhow::Result<()> {
            self.created_dirs.push(path.to_path_buf());
            Ok(())
        }
        fn remove_file(&mut self, path: &std::path::Path) -> anyhow::Result<()> {
            self.removed.push(path.to_path_buf());
            self.existing.remove(path);
            self.files.remove(path);
            Ok(())
        }
        fn read_to_string(&self, path: &std::path::Path) -> anyhow::Result<String> {
            self.files
                .get(path)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("file not seeded: {}", path.display()))
        }
        fn write(&mut self, path: &std::path::Path, content: &str) -> anyhow::Result<()> {
            self.existing.insert(path.to_path_buf());
            self.files.insert(path.to_path_buf(), content.to_string());
            Ok(())
        }
    }

    // Used by macOS-gated tests; dead on non-macOS where those tests are skipped.
    #[allow(dead_code)]
    fn expected_plist() -> std::path::PathBuf {
        std::path::PathBuf::from("/Users/test/Library/LaunchAgents/com.copypaste.daemon.plist")
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

    /// Uninstall must unlink the IPC socket if it exists. A leftover
    /// socket file causes the next `start` to fail with EADDRINUSE on
    /// bind.
    #[cfg(target_os = "macos")]
    #[test]
    fn daemon_uninstall_removes_leftover_socket() {
        let socket = std::path::PathBuf::from(
            "/Users/test/Library/Application Support/CopyPaste/daemon.sock",
        );
        let mut runner = MockRunner::new();
        runner.set_response("launchctl", "print", true, "", "");
        let mut fs = MockFs::new()
            .with_existing(expected_plist())
            .with_existing(socket.clone());

        dispatch(DaemonAction::Uninstall, &mut runner, &mut fs).expect("uninstall ok");

        assert!(
            fs.removed.contains(&socket),
            "expected socket {} to be removed, removed={:?}",
            socket.display(),
            fs.removed
        );
        // Plist still removed too.
        assert!(
            fs.removed.contains(&expected_plist()),
            "expected plist still removed alongside socket"
        );
    }

    /// Uninstall must NOT call `launchctl disable` by default — disabling
    /// the label makes a subsequent `install` fail with "Bootstrap failed:
    /// 5" until the user manually re-enables it.
    #[cfg(target_os = "macos")]
    #[test]
    fn daemon_uninstall_does_not_disable_label() {
        let mut runner = MockRunner::new();
        runner.set_response("launchctl", "print", true, "", "");
        let mut fs = MockFs::new().with_existing(expected_plist());

        dispatch(DaemonAction::Uninstall, &mut runner, &mut fs).expect("uninstall ok");

        let disable_called = runner.calls.iter().any(|c| {
            c.program == "launchctl" && c.args.first().map(|s| s.as_str()) == Some("disable")
        });
        assert!(
            !disable_called,
            "uninstall must NOT call launchctl disable, got: {:?}",
            runner.calls
        );
    }

    /// `macos_install` must create the logs dir so launchd can open
    /// `StandardOutPath` / `StandardErrorPath` without ENOENT.
    #[cfg(target_os = "macos")]
    #[test]
    fn daemon_install_creates_logs_dir() {
        let mut runner = MockRunner::new();
        let src = std::path::PathBuf::from("/repo/packaging/macos/com.copypaste.daemon.plist");
        let mut fs = MockFs::new().with_file(src, SAMPLE_PLIST_FOR_INSTALL);

        dispatch(DaemonAction::Install, &mut runner, &mut fs).expect("install ok");

        let logs_dir = std::path::PathBuf::from("/Users/test/Library/Logs/CopyPaste");
        assert!(
            fs.created_dirs.contains(&logs_dir),
            "expected logs dir {} to be created, got: {:?}",
            logs_dir.display(),
            fs.created_dirs
        );
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
        let src = std::path::PathBuf::from("/repo/packaging/macos/com.copypaste.daemon.plist");
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
        let exe = std::path::PathBuf::from("/Applications/CopyPaste.app/Contents/MacOS/copypaste");
        let bundle_plist = std::path::PathBuf::from(
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

    // Sample fixture for install tests; dead on non-macOS where those tests are skipped.
    #[allow(dead_code)]
    const SAMPLE_PLIST_FOR_INSTALL: &str = r#"<?xml version="1.0"?>
<plist><dict>
    <key>StandardOutPath</key>
    <string>/Users/USERNAME/Library/Logs/CopyPaste/daemon.out.log</string>
</dict></plist>"#;
}
