//! Filesystem path resolution for the CLI.
//!
//! `socket_path()` now delegates to [`copypaste_ipc::paths::socket_path`] —
//! the single canonical resolver — so the CLI and daemon can never diverge
//! on where the socket lives.  The local `app_support_dir()` is retained only
//! for the CLI-local `db_path()` helper.
//!
//! Windows note (ADR-012: frozen/Homebrew-only): the named-pipe variant
//! `\\.\pipe\copypaste-daemon` referenced below is ASPIRATIONAL and unused —
//! `ipc.rs` uses `UnixStream`, which does not compile on Windows. If Windows is
//! ever unfrozen, both this file and `ipc.rs` need platform-specific transports.

use std::path::PathBuf;

const APP_NAME: &str = "CopyPaste";

/// Resolve the platform-specific application data directory.
///
/// Mirrors the daemon's `try_app_support_dir` (the `home`-crate variant used
/// for the socket/DB it actually binds). Falls back to `$TMPDIR/CopyPaste`
/// when the home directory cannot be resolved, matching the daemon's
/// infallible `app_support_dir`.
fn app_support_dir() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        if let Some(home) = home::home_dir() {
            return home.join("Library/Application Support").join(APP_NAME);
        }
    }
    #[cfg(target_os = "windows")]
    {
        if let Some(appdata) = std::env::var_os("APPDATA") {
            return PathBuf::from(appdata).join(APP_NAME);
        }
        if let Some(home) = home::home_dir() {
            return home.join("AppData").join("Roaming").join(APP_NAME);
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        if let Some(xdg) = std::env::var_os("XDG_DATA_HOME") {
            return PathBuf::from(xdg).join("copypaste");
        }
        if let Some(home) = home::home_dir() {
            return home.join(".local/share").join("copypaste");
        }
    }
    std::env::temp_dir().join(APP_NAME)
}

/// Returns the IPC socket path the daemon binds.
///
/// Delegates to [`copypaste_ipc::paths::socket_path`] — the single canonical
/// resolver shared by the daemon, CLI, and UI.  See that function's
/// documentation for resolution order and platform paths.
pub fn socket_path() -> PathBuf {
    copypaste_ipc::paths::socket_path()
}

#[allow(dead_code)] // tests call this; production code routes vacuum through IPC now.
pub fn db_path() -> PathBuf {
    if let Ok(p) = std::env::var("COPYPASTE_DB") {
        return PathBuf::from(p);
    }
    app_support_dir().join("clipboard.db")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // std::env::set_var / remove_var are unsound under parallel test threads
    // (deprecated in Rust 1.80, UB on some platforms). All env-mutating tests
    // in this module must hold this lock for their full duration so they
    // never race with each other. Tests that only READ env vars (no mutation)
    // do not need the lock but are listed here anyway for documentation.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn socket_path_ends_with_daemon_sock() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        std::env::remove_var("COPYPASTE_SOCKET");
        let p = socket_path();
        assert!(
            p.to_string_lossy().ends_with("daemon.sock"),
            "expected path ending in daemon.sock, got: {}",
            p.display()
        );
    }

    #[test]
    fn socket_path_contains_copypaste() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        std::env::remove_var("COPYPASTE_SOCKET");
        let p = socket_path();
        // Case-insensitive: macOS uses "CopyPaste" (Application Support); Linux
        // uses lowercase "copypaste" ($XDG_RUNTIME_DIR / ~/.local/share).
        assert!(
            p.to_string_lossy().to_lowercase().contains("copypaste"),
            "expected path to contain copypaste, got: {}",
            p.display()
        );
    }

    #[test]
    fn socket_path_env_override() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        std::env::set_var("COPYPASTE_SOCKET", "/tmp/test.sock");
        let p = socket_path();
        std::env::remove_var("COPYPASTE_SOCKET");
        assert_eq!(p, PathBuf::from("/tmp/test.sock"));
    }

    #[test]
    fn db_path_env_override() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        std::env::set_var("COPYPASTE_DB", "/tmp/test.db");
        let p = db_path();
        std::env::remove_var("COPYPASTE_DB");
        assert_eq!(p, PathBuf::from("/tmp/test.db"));
    }

    /// On Unix the default socket lives inside the app-support dir and ends in
    /// `daemon.sock` — the exact filename the daemon binds.
    #[test]
    #[cfg(not(target_os = "windows"))]
    fn socket_path_default_ends_with_daemon_sock() {
        std::env::remove_var("COPYPASTE_SOCKET");
        let p = socket_path();
        assert!(
            p.to_string_lossy().ends_with("daemon.sock"),
            "expected path ending in daemon.sock, got: {}",
            p.display()
        );
    }

    #[test]
    fn db_path_default_contains_copypaste() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        std::env::remove_var("COPYPASTE_DB");
        let p = db_path();
        assert!(
            p.to_string_lossy().contains("CopyPaste") || p.to_string_lossy().contains("copypaste"),
            "expected db path to contain CopyPaste or copypaste, got: {}",
            p.display()
        );
    }

    /// Platform-specific default location must match the daemon's resolution so
    /// the CLI connects to the right place per-OS.
    #[test]
    fn app_support_dir_matches_platform_convention() {
        std::env::remove_var("COPYPASTE_SOCKET");
        let d = app_support_dir();
        let s = d.to_string_lossy();
        #[cfg(target_os = "macos")]
        assert!(
            s.contains("Library/Application Support") && s.ends_with(APP_NAME),
            "macOS app-support dir mismatch: {s}"
        );
        #[cfg(target_os = "windows")]
        assert!(
            s.contains(APP_NAME),
            "windows app-support dir mismatch: {s}"
        );
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        assert!(
            s.ends_with("copypaste"),
            "linux app-support dir should end with `copypaste`: {s}"
        );
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn socket_path_default_is_named_pipe() {
        std::env::remove_var("COPYPASTE_SOCKET");
        let p = socket_path();
        assert_eq!(p, PathBuf::from(r"\\.\pipe\copypaste-daemon"));
    }
}
