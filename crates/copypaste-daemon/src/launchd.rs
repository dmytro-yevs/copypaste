//! launchd LaunchAgent plist management for macOS autostart.
//!
//! Generates `com.copypaste.daemon.plist` and installs/uninstalls it via
//! `launchctl` so the daemon starts at login automatically.

use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Label used in the plist and by launchctl.
pub const LABEL: &str = "com.copypaste.daemon";

/// Path to the user-level LaunchAgents directory.
pub fn launch_agents_dir() -> PathBuf {
    home::home_dir()
        .expect("HOME directory must exist")
        .join("Library/LaunchAgents")
}

/// Destination plist path.
pub fn plist_path() -> PathBuf {
    launch_agents_dir().join(format!("{LABEL}.plist"))
}

/// Log directory for daemon stdout/stderr.
pub fn log_dir() -> PathBuf {
    home::home_dir()
        .expect("HOME directory must exist")
        .join("Library/Logs/CopyPaste")
}

/// Generate plist XML for the given binary path.
///
/// `binary_path` should be an absolute path to the `copypaste-daemon` binary
/// (typically resolved via `std::env::current_exe()`).
pub fn generate_plist(binary_path: &Path) -> String {
    let binary_str = binary_path.to_string_lossy();
    let log_dir = log_dir();
    let stdout = log_dir.join("daemon.log").to_string_lossy().into_owned();
    let stderr = log_dir.join("daemon.err").to_string_lossy().into_owned();

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

    <key>KeepAlive</key>
    <dict>
        <key>SuccessfulExit</key>
        <false/>
    </dict>

    <key>StandardOutPath</key>
    <string>{stdout}</string>

    <key>StandardErrorPath</key>
    <string>{stderr}</string>

    <key>EnvironmentVariables</key>
    <dict>
        <key>RUST_LOG</key>
        <string>info</string>
    </dict>
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
    // Ensure directories exist
    let agents_dir = launch_agents_dir();
    std::fs::create_dir_all(&agents_dir)?;
    std::fs::create_dir_all(log_dir())?;

    let dest = plist_path();
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
    }

    #[test]
    fn plist_contains_log_paths() {
        let binary = PathBuf::from("/tmp/test-daemon");
        let xml = generate_plist(&binary);
        assert!(xml.contains("daemon.log"));
        assert!(xml.contains("daemon.err"));
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
        assert_eq!(args, vec!["unload", "-w", "/tmp/com.copypaste.daemon.plist"]);
    }

    #[test]
    fn plist_path_is_in_launch_agents() {
        let p = plist_path();
        assert!(p.to_string_lossy().contains("Library/LaunchAgents"));
        assert!(p
            .to_string_lossy()
            .ends_with("com.copypaste.daemon.plist"));
    }

    #[test]
    fn label_constant_matches_plist() {
        let binary = PathBuf::from("/tmp/daemon");
        let xml = generate_plist(&binary);
        assert!(xml.contains(LABEL));
    }
}
