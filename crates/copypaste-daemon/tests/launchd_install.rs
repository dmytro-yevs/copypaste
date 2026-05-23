//! Beta-bonus: `launchd` LaunchAgent plist install/uninstall tests.
//!
//! Scope: covers the on-disk side-effects of `copypaste_daemon::launchd`
//! (plist write, overwrite-on-rerun, removal, idempotency, plutil lint, and
//! the error path when the LaunchAgents directory cannot be created).
//!
//! ## How `launchctl` is "mocked"
//!
//! These tests deliberately avoid calling [`launchd::install`] /
//! [`launchd::install_with_binary`] in the happy path because both end with
//! a real `launchctl load -w …` invocation that would register the daemon
//! with the current user's launchd domain — a real, persistent side-effect
//! on the dev machine and CI runners.
//!
//! Instead, the file-write half of `install` is reproduced inline using the
//! crate's own public helpers ([`launchd::generate_plist`],
//! [`launchd::try_launch_agents_dir`], [`launchd::plist_path`]) so the
//! contract under test is exactly what `install_with_binary` writes to disk,
//! without ever spawning `launchctl`. The argument-building helpers
//! (`launchctl_load_args` / `launchctl_unload_args`) are unit-tested next to
//! the implementation in `src/launchd.rs`.
//!
//! The "invalid program path" test does call `install_with_binary` because
//! it deliberately points HOME at a regular file — `create_dir_all` then
//! fails with `NotADirectory` before `launchctl` is ever reached, so no
//! real launchd registration occurs.
//!
//! [`uninstall`] is safe to call: it only invokes `launchctl unload` when
//! the plist file already exists, the error is discarded via `let _ = …`,
//! and we only call it against plists living inside our `tempdir` HOME
//! (never loaded by anyone, so unload is a no-op that returns a non-zero
//! exit code that the implementation ignores).
//!
//! ## HOME isolation
//!
//! The `home` crate respects `$HOME` on Unix (see `home-0.5.12/src/lib.rs`,
//! "Returns the value of the `HOME` environment variable if it is set even
//! if it is an empty string"). Each test installs a `tempfile::TempDir`,
//! points `HOME` at it via `env::set_var`, runs the assertions, then
//! restores the previous `HOME` in a `HomeGuard` drop impl so a panicking
//! test still leaves the global env clean.
//!
//! `env::set_var` is process-global, so the whole suite is `#[serial]` to
//! prevent interleaving with itself or with other tests that read `$HOME`
//! (e.g. anything under `tests/` that touches `dirs::home_dir`).
//!
//! Linux / Windows: gated `#[cfg(target_os = "macos")]` because the plist
//! layout, the `Library/LaunchAgents` path, and the `plutil` binary are all
//! macOS-only. On other platforms this file compiles to an empty test bin.

#![cfg(target_os = "macos")]

use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use copypaste_daemon::launchd;
use serial_test::serial;
use tempfile::TempDir;

/// RAII guard that swaps `$HOME` for the test's duration and restores the
/// previous value on drop — even if the test panics.
struct HomeGuard {
    previous: Option<OsString>,
}

impl HomeGuard {
    fn set(new_home: &Path) -> Self {
        let previous = env::var_os("HOME");
        // SAFETY: serialized via `#[serial]` so no concurrent reader/writer
        // of $HOME inside this process. The guard restores on drop.
        unsafe {
            env::set_var("HOME", new_home);
        }
        Self { previous }
    }
}

impl Drop for HomeGuard {
    fn drop(&mut self) {
        unsafe {
            match &self.previous {
                Some(v) => env::set_var("HOME", v),
                None => env::remove_var("HOME"),
            }
        }
    }
}

/// Reproduce the file-write half of `launchd::install_with_binary` without
/// ever spawning `launchctl`. Returns the path to the freshly-written
/// plist so callers can stat / lint / remove it.
fn write_plist_for_test(binary: &Path) -> PathBuf {
    let agents_dir = launchd::try_launch_agents_dir()
        .expect("HOME is set by HomeGuard, try_launch_agents_dir cannot fail here");
    fs::create_dir_all(&agents_dir).expect("create LaunchAgents dir under tempdir HOME");
    // Mirror install_with_binary: also create the log dir so a real install
    // would have somewhere to write daemon.log / daemon.err.
    fs::create_dir_all(launchd::try_log_dir().expect("try_log_dir w/ HOME set"))
        .expect("create log dir under tempdir HOME");

    let dest = agents_dir.join(format!("{}.plist", launchd::LABEL));
    let xml = launchd::generate_plist(binary);
    fs::write(&dest, xml).expect("write plist");
    dest
}

#[test]
#[serial]
fn install_writes_plist_to_launch_agents_path() {
    let tmp = TempDir::new().expect("tempdir");
    let _guard = HomeGuard::set(tmp.path());

    let binary = PathBuf::from("/usr/local/bin/copypaste-daemon");
    let plist = write_plist_for_test(&binary);

    // Plist must land inside the HOME we configured, not the real user's.
    let expected = tmp
        .path()
        .join("Library/LaunchAgents/com.copypaste.daemon.plist");
    assert_eq!(plist, expected, "plist path must derive from $HOME");
    assert!(plist.exists(), "plist file was not written");
    assert_eq!(
        launchd::plist_path(),
        expected,
        "public plist_path() agrees"
    );

    // Sanity: the file we wrote actually contains the binary path we passed
    // and the canonical label, not stale content from a previous run.
    let contents = fs::read_to_string(&plist).expect("read plist back");
    assert!(contents.contains("/usr/local/bin/copypaste-daemon"));
    assert!(contents.contains(launchd::LABEL));
}

#[test]
#[serial]
fn install_idempotent_rerun_overwrites_cleanly() {
    let tmp = TempDir::new().expect("tempdir");
    let _guard = HomeGuard::set(tmp.path());

    // First "install" with binary A.
    let plist = write_plist_for_test(&PathBuf::from("/opt/old/copypaste-daemon"));
    let first = fs::read_to_string(&plist).expect("read first plist");
    assert!(first.contains("/opt/old/copypaste-daemon"));

    // Second "install" with binary B — must overwrite, not append, not error.
    let plist2 = write_plist_for_test(&PathBuf::from("/opt/new/copypaste-daemon"));
    assert_eq!(plist, plist2, "rerun targets the same path");

    let second = fs::read_to_string(&plist).expect("read second plist");
    assert!(
        second.contains("/opt/new/copypaste-daemon"),
        "new binary path missing after rerun"
    );
    assert!(
        !second.contains("/opt/old/copypaste-daemon"),
        "stale binary path still present — file was appended to instead of overwritten"
    );

    // And there's only ever one plist in the directory.
    let entries: Vec<_> = fs::read_dir(plist.parent().unwrap())
        .expect("read LaunchAgents dir")
        .filter_map(|e| e.ok())
        .collect();
    assert_eq!(entries.len(), 1, "exactly one plist should exist");
}

#[test]
#[serial]
fn uninstall_removes_plist_file() {
    let tmp = TempDir::new().expect("tempdir");
    let _guard = HomeGuard::set(tmp.path());

    let plist = write_plist_for_test(&PathBuf::from("/usr/local/bin/copypaste-daemon"));
    assert!(
        plist.exists(),
        "precondition: plist exists before uninstall"
    );

    // uninstall() will call `launchctl unload -w <plist>` — that returns a
    // non-zero exit code because nothing ever loaded this tempdir plist,
    // but the implementation swallows that via `let _ = …` and proceeds to
    // remove the file. We only need to assert the post-condition.
    launchd::uninstall().expect("uninstall should succeed even if unload no-ops");
    assert!(!plist.exists(), "plist file should be removed by uninstall");
}

#[test]
#[serial]
fn uninstall_idempotent_safe_when_not_installed() {
    let tmp = TempDir::new().expect("tempdir");
    let _guard = HomeGuard::set(tmp.path());

    // Precondition: no plist anywhere under this HOME.
    assert!(
        !launchd::plist_path().exists(),
        "tempdir HOME should start clean"
    );
    assert!(
        !launchd::is_installed(),
        "is_installed must be false in a fresh HOME"
    );

    // First call: nothing to do, returns Ok.
    launchd::uninstall().expect("uninstall on missing plist must succeed");
    // Second call: still nothing to do, still Ok.
    launchd::uninstall().expect("uninstall must be idempotent");

    // And `is_installed` stays false.
    assert!(!launchd::is_installed());
}

#[test]
#[serial]
fn plist_passes_plutil_lint() {
    // `plutil` ships with macOS at /usr/bin/plutil. If for some reason it's
    // missing (sandboxed CI, exotic image), skip rather than fail — the
    // structural assertions in src/launchd.rs::tests already cover XML shape.
    let plutil = PathBuf::from("/usr/bin/plutil");
    if !plutil.exists() {
        eprintln!("plutil not present — skipping plutil lint test");
        return;
    }

    let tmp = TempDir::new().expect("tempdir");
    let _guard = HomeGuard::set(tmp.path());
    let plist = write_plist_for_test(&PathBuf::from("/usr/local/bin/copypaste-daemon"));

    let output = Command::new(&plutil)
        .arg("-lint")
        .arg(&plist)
        .output()
        .expect("spawn plutil");
    assert!(
        output.status.success(),
        "plutil -lint failed: status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
#[serial]
fn install_with_invalid_program_path_returns_error() {
    // Point $HOME at a regular file (not a directory). `install_with_binary`
    // will then try `create_dir_all(<file>/Library/LaunchAgents)` and get a
    // `NotADirectory` IO error, which surfaces as `LaunchdError::Io` long
    // before `launchctl` is ever invoked. This is the cleanest way to
    // exercise the error branch without registering anything with launchd.
    //
    // The "invalid program path" /nonexistent in the binary argument is
    // intentionally NOT what triggers the error: `generate_plist` accepts
    // any path string (launchctl is what validates the program at load
    // time, and we deliberately don't run launchctl here). The error path
    // we can actually hit without side-effects is the directory-creation
    // failure above.
    let tmp = TempDir::new().expect("tempdir");
    let bogus_home = tmp.path().join("home-is-a-file");
    fs::write(&bogus_home, b"not a directory").expect("seed bogus HOME file");

    let _guard = HomeGuard::set(&bogus_home);

    let result = launchd::install_with_binary(Path::new("/nonexistent/copypaste-daemon"));
    assert!(
        result.is_err(),
        "install_with_binary must error when HOME is unusable"
    );
    let err = result.unwrap_err();
    // Must be the IO branch (create_dir_all failed), not LaunchctlFailed —
    // that would mean we somehow reached launchctl, which would be a real
    // side-effect.
    match err {
        launchd::LaunchdError::Io(_) => { /* expected */ }
        other => panic!("expected LaunchdError::Io from create_dir_all, got {other:?}"),
    }

    // And no plist was leaked next to the bogus HOME file.
    let leaked = bogus_home.join("Library/LaunchAgents/com.copypaste.daemon.plist");
    assert!(
        !leaked.exists(),
        "no plist should be written on the error path"
    );
}
