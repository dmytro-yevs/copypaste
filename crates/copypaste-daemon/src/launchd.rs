//! launchd LaunchAgent plist management for macOS autostart.
//!
//! Generates `com.copypaste.daemon.plist` and installs/uninstalls it via
//! `launchctl` so the daemon starts at login automatically.

use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Label used in the plist and by launchctl.
pub const LABEL: &str = "com.copypaste.daemon";

/// Fallible variant of [`launch_agents_dir`].
///
/// Returns [`LaunchdError::NoHome`] if the user's home directory cannot be
/// determined.
pub fn try_launch_agents_dir() -> Result<PathBuf, LaunchdError> {
    let home = home::home_dir().ok_or(LaunchdError::NoHome)?;
    Ok(home.join("Library/LaunchAgents"))
}

/// Path to the user-level LaunchAgents directory.
///
/// Infallible — falls back to `$TMPDIR/LaunchAgents` and logs a warning if the
/// home directory cannot be resolved so that install attempts surface a real
/// error later instead of panicking inside a getter.
pub fn launch_agents_dir() -> PathBuf {
    try_launch_agents_dir().unwrap_or_else(|e| {
        let fallback = std::env::temp_dir().join("LaunchAgents");
        tracing::warn!(
            error = %e,
            fallback = %fallback.display(),
            "launch_agents_dir: home unresolved, using temp-dir fallback"
        );
        fallback
    })
}

/// Destination plist path.
pub fn plist_path() -> PathBuf {
    launch_agents_dir().join(format!("{LABEL}.plist"))
}

/// Fallible variant of [`log_dir`].
pub fn try_log_dir() -> Result<PathBuf, LaunchdError> {
    let home = home::home_dir().ok_or(LaunchdError::NoHome)?;
    Ok(home.join("Library/Logs/CopyPaste"))
}

/// Log directory for daemon stdout/stderr.
///
/// Infallible — falls back to `$TMPDIR/CopyPaste-Logs` if the home directory
/// cannot be resolved.
pub fn log_dir() -> PathBuf {
    try_log_dir().unwrap_or_else(|e| {
        let fallback = std::env::temp_dir().join("CopyPaste-Logs");
        tracing::warn!(
            error = %e,
            fallback = %fallback.display(),
            "log_dir: home unresolved, using temp-dir fallback"
        );
        fallback
    })
}

/// Generate plist XML for the given binary path.
///
/// `binary_path` should be an absolute path to the `copypaste-daemon` binary
/// (typically resolved via `std::env::current_exe()`).
///
/// IMPORTANT: this output is kept byte-for-byte consistent (modulo the
/// substituted `binary_path` and resolved log dir) with the single
/// source-of-truth plist shipped in the app bundle,
/// `packaging/macos/com.copypaste.daemon.plist`. Both must agree on
/// `KeepAlive` (respawn only on crash, never on a clean exit so `bootout`
/// and `daemon stop` work), `ProcessType`, the `daemon.out.log` /
/// `daemon.err.log` filenames, and `ThrottleInterval`. If you change one,
/// change the other (and the autostart render path in
/// `crates/copypaste-ui/src/autostart.rs`).
pub fn generate_plist(binary_path: &Path) -> String {
    let binary_str = binary_path.to_string_lossy();
    let log_dir = log_dir();
    let stdout = log_dir
        .join("daemon.out.log")
        .to_string_lossy()
        .into_owned();
    let stderr = log_dir
        .join("daemon.err.log")
        .to_string_lossy()
        .into_owned();

    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{LABEL}</string>

    <key>ProgramArguments</key>
    <array>
        <string>{binary_str}</string>
    </array>

    <key>RunAtLoad</key>
    <true/>

    <!--
        Respawn only on crash; a clean exit (user invoked `daemon stop` or
        the daemon shut down intentionally) must NOT trigger a relaunch.
        Without this, `KeepAlive=<true/>` would fight `launchctl bootout`
        and the UI's quit/restart flows. Keep in sync with
        packaging/macos/com.copypaste.daemon.plist.
    -->
    <key>KeepAlive</key>
    <dict>
        <key>SuccessfulExit</key>
        <false/>
        <key>Crashed</key>
        <true/>
    </dict>

    <key>ProcessType</key>
    <string>Interactive</string>

    <key>StandardOutPath</key>
    <string>{stdout}</string>

    <key>StandardErrorPath</key>
    <string>{stderr}</string>

    <key>EnvironmentVariables</key>
    <dict>
        <key>RUST_LOG</key>
        <string>info</string>
    </dict>

    <!--
        Bumped from 10 → 30 so a flapping daemon (e.g. config error,
        permission denial) doesn't burn CPU in a tight respawn loop.
    -->
    <key>ThrottleInterval</key>
    <integer>30</integer>
</dict>
</plist>
"#
    )
}

/// Error type for launchd operations.
#[derive(Debug, thiserror::Error)]
pub enum LaunchdError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("launchctl {command} failed (exit {code}): {stderr}")]
    LaunchctlFailed {
        command: String,
        code: i32,
        stderr: String,
    },

    #[error("could not determine current executable path: {0}")]
    CurrentExe(io::Error),

    #[error("could not determine user home directory (HOME unset?)")]
    NoHome,
}

/// Install the plist to `~/Library/LaunchAgents/` and load it with launchctl.
///
/// If already loaded, re-loads (unload then load) to pick up any changes.
pub fn install() -> Result<(), LaunchdError> {
    let binary = std::env::current_exe().map_err(LaunchdError::CurrentExe)?;
    install_with_binary(&binary)
}

/// Install using an explicit binary path (useful for tests / custom installs).
pub fn install_with_binary(binary: &Path) -> Result<(), LaunchdError> {
    // Ensure directories exist. Use the fallible accessors here so install
    // returns a real error instead of silently writing into a temp-dir
    // fallback (which would never auto-start at login).
    let agents_dir = try_launch_agents_dir()?;
    std::fs::create_dir_all(&agents_dir)?;
    std::fs::create_dir_all(try_log_dir()?)?;

    let dest = agents_dir.join(format!("{LABEL}.plist"));
    let plist_content = generate_plist(binary);
    std::fs::write(&dest, &plist_content)?;

    // Unload first (ignore error — might not be loaded yet)
    let _ = launchctl(&["unload", "-w", &dest.to_string_lossy()]);

    // Load
    launchctl(&["load", "-w", &dest.to_string_lossy()])?;
    Ok(())
}

/// Unload the agent and remove the plist from `~/Library/LaunchAgents/`.
pub fn uninstall() -> Result<(), LaunchdError> {
    let dest = plist_path();
    if dest.exists() {
        let _ = launchctl(&["unload", "-w", &dest.to_string_lossy()]);
        std::fs::remove_file(&dest)?;
    }
    Ok(())
}

/// Returns `true` if the plist file is present in LaunchAgents.
pub fn is_installed() -> bool {
    plist_path().exists()
}

/// Build the launchctl argument list for load/unload operations.
///
/// This is separated so unit tests can verify the correct arguments
/// without actually running launchctl.
pub fn launchctl_load_args(plist: &Path) -> Vec<String> {
    vec![
        "load".to_string(),
        "-w".to_string(),
        plist.to_string_lossy().into_owned(),
    ]
}

/// Build the launchctl argument list for unload operations.
pub fn launchctl_unload_args(plist: &Path) -> Vec<String> {
    vec![
        "unload".to_string(),
        "-w".to_string(),
        plist.to_string_lossy().into_owned(),
    ]
}

/// Run `launchctl` with the given arguments.
fn launchctl(args: &[&str]) -> Result<(), LaunchdError> {
    let output = Command::new("launchctl").args(args).output()?;
    if !output.status.success() {
        let code = output.status.code().unwrap_or(-1);
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        return Err(LaunchdError::LaunchctlFailed {
            command: args.join(" "),
            code,
            stderr,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn plist_contains_label() {
        let binary = PathBuf::from("/usr/local/bin/copypaste-daemon");
        let xml = generate_plist(&binary);
        assert!(xml.contains("<string>com.copypaste.daemon</string>"));
    }

    #[test]
    fn plist_contains_binary_path() {
        let binary = PathBuf::from("/usr/local/bin/copypaste-daemon");
        let xml = generate_plist(&binary);
        assert!(xml.contains("<string>/usr/local/bin/copypaste-daemon</string>"));
    }

    #[test]
    fn plist_contains_run_at_load() {
        let binary = PathBuf::from("/tmp/test-daemon");
        let xml = generate_plist(&binary);
        assert!(xml.contains("<key>RunAtLoad</key>"));
        assert!(xml.contains("<true/>"));
    }

    #[test]
    fn plist_contains_keep_alive() {
        let binary = PathBuf::from("/tmp/test-daemon");
        let xml = generate_plist(&binary);
        assert!(xml.contains("<key>KeepAlive</key>"));
        assert!(xml.contains("<key>SuccessfulExit</key>"));
        assert!(xml.contains("<false/>"));
        // Reconciled with packaging/macos/com.copypaste.daemon.plist: respawn
        // on crash only.
        assert!(xml.contains("<key>Crashed</key>"));
        assert!(xml.contains("<true/>"));
    }

    #[test]
    fn plist_contains_process_type_and_throttle() {
        // Both reconciled in from the packaged source-of-truth plist.
        let binary = PathBuf::from("/tmp/test-daemon");
        let xml = generate_plist(&binary);
        assert!(xml.contains("<key>ProcessType</key>"));
        assert!(xml.contains("<string>Interactive</string>"));
        assert!(xml.contains("<key>ThrottleInterval</key>"));
        assert!(xml.contains("<integer>30</integer>"));
    }

    #[test]
    fn plist_contains_log_paths() {
        let binary = PathBuf::from("/tmp/test-daemon");
        let xml = generate_plist(&binary);
        // Filenames match the packaged plist: daemon.out.log / daemon.err.log.
        assert!(xml.contains("daemon.out.log"));
        assert!(xml.contains("daemon.err.log"));
        assert!(xml.contains("Library/Logs/CopyPaste"));
    }

    #[test]
    fn plist_is_valid_xml() {
        let binary = PathBuf::from("/tmp/copypaste-daemon");
        let xml = generate_plist(&binary);
        // Basic XML structure validation
        assert!(xml.starts_with("<?xml version=\"1.0\""));
        assert!(xml.contains("<!DOCTYPE plist"));
        assert!(xml.contains("<plist version=\"1.0\">"));
        assert!(xml.ends_with("</plist>\n"));
    }

    #[test]
    fn plist_contains_rust_log_env() {
        let binary = PathBuf::from("/tmp/test-daemon");
        let xml = generate_plist(&binary);
        assert!(xml.contains("<key>RUST_LOG</key>"));
        assert!(xml.contains("<string>info</string>"));
    }

    #[test]
    fn launchctl_load_args_correct() {
        let plist = PathBuf::from("/tmp/com.copypaste.daemon.plist");
        let args = launchctl_load_args(&plist);
        assert_eq!(args, vec!["load", "-w", "/tmp/com.copypaste.daemon.plist"]);
    }

    #[test]
    fn launchctl_unload_args_correct() {
        let plist = PathBuf::from("/tmp/com.copypaste.daemon.plist");
        let args = launchctl_unload_args(&plist);
        assert_eq!(
            args,
            vec!["unload", "-w", "/tmp/com.copypaste.daemon.plist"]
        );
    }

    #[test]
    fn plist_path_is_in_launch_agents() {
        let p = plist_path();
        assert!(p.to_string_lossy().contains("Library/LaunchAgents"));
        assert!(p.to_string_lossy().ends_with("com.copypaste.daemon.plist"));
    }

    #[test]
    fn label_constant_matches_plist() {
        let binary = PathBuf::from("/tmp/daemon");
        let xml = generate_plist(&binary);
        assert!(xml.contains(LABEL));
    }
}
