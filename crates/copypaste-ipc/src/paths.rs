//! Canonical socket-path resolver — the single source of truth for where the
//! CopyPaste daemon binds its Unix socket.
//!
//! ## Why this lives in `copypaste-ipc`
//!
//! The daemon (`copypaste-daemon`), the CLI (`copypaste-cli`), and the desktop
//! UI (`copypaste-ui`) all need the same socket path.  The IPC crate is already
//! a shared dependency of every consumer (it owns the wire types), so it is the
//! correct home for path resolution — avoiding a new crate while keeping
//! `copypaste-core` out of the CLI/UI (architectural constraint).
//!
//! ## Platform paths
//!
//! | Platform | Default path |
//! |----------|-------------|
//! | macOS    | `~/Library/Application Support/CopyPaste/daemon.sock` |
//! | Linux    | `$XDG_DATA_HOME/copypaste/daemon.sock` or `~/.local/share/copypaste/daemon.sock` |
//! | Windows  | `\\.\pipe\copypaste-daemon` (named pipe pseudo-path) |
//!
//! The `COPYPASTE_SOCKET` environment variable overrides the platform default
//! on all platforms — used by integration tests to redirect to a temp socket.

use std::path::PathBuf;

const APP_NAME: &str = "CopyPaste";

/// Returns the platform-specific application-data directory that the daemon
/// uses to locate (or create) its files.
///
/// Resolution order:
/// 1. `COPYPASTE_SOCKET` / `COPYPASTE_DB` callers should check their own env
///    vars before calling this — this helper only handles the base directory.
/// 2. macOS: `~/Library/Application Support/CopyPaste`
/// 3. Windows: `%APPDATA%\CopyPaste`, falling back to
///    `~/AppData/Roaming/CopyPaste`
/// 4. Linux/other: `$XDG_DATA_HOME/copypaste` or `~/.local/share/copypaste`
/// 5. Last resort: `$TMPDIR/CopyPaste` (never panics).
///
/// This function is `pub` so the daemon and CLI can use it for DB and config
/// paths as well, but the primary consumer is [`socket_path`].
pub fn app_support_dir() -> PathBuf {
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
    // Fallback: temp dir so the daemon can still start even without a home dir.
    std::env::temp_dir().join(APP_NAME)
}

/// Returns the IPC socket path.
///
/// This is the **single canonical resolver** for where the daemon binds its
/// socket and where every client (CLI, UI) connects.  All three consumers must
/// call this function — no local copies.
///
/// Resolution order:
/// 1. `COPYPASTE_SOCKET` env var, if set and non-empty.
/// 2. On Windows: `\\.\pipe\copypaste-daemon` (named-pipe pseudo-path).
/// 3. On Unix: `daemon.sock` inside [`app_support_dir()`].
///
/// # Note on src-tauri
///
/// `crates/copypaste-ui/src-tauri/src/ipc.rs` contains a third independent
/// copy of this logic (lines 16–43 as of 2026-06-21).  That call site is owned
/// by a separate lane and is tracked as a follow-up:
/// CopyPaste-c4q2.2 src-tauri follow-up.
pub fn socket_path() -> PathBuf {
    if let Ok(p) = std::env::var("COPYPASTE_SOCKET") {
        return PathBuf::from(p);
    }
    #[cfg(target_os = "windows")]
    {
        // Named pipes use a pseudo-filesystem path, not a real directory.
        PathBuf::from(r"\\.\pipe\copypaste-daemon")
    }
    #[cfg(not(target_os = "windows"))]
    {
        app_support_dir().join("daemon.sock")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // All tests that mutate env vars must hold this lock for their full
    // duration.  std::env::set_var/remove_var are unsound under concurrent
    // access (deprecated in Rust 1.80).
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn socket_path_env_override_wins() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // SAFETY: serialised via ENV_LOCK.
        unsafe { std::env::set_var("COPYPASTE_SOCKET", "/tmp/copypaste-ipc-test.sock") };
        let p = socket_path();
        // SAFETY: restoring env under the same lock.
        unsafe { std::env::remove_var("COPYPASTE_SOCKET") };
        assert_eq!(
            p,
            PathBuf::from("/tmp/copypaste-ipc-test.sock"),
            "COPYPASTE_SOCKET override must win over platform default"
        );
    }

    #[test]
    #[cfg(not(target_os = "windows"))]
    fn socket_path_default_ends_with_daemon_sock() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // SAFETY: serialised via ENV_LOCK.
        unsafe { std::env::remove_var("COPYPASTE_SOCKET") };
        let p = socket_path();
        assert!(
            p.to_string_lossy().ends_with("daemon.sock"),
            "expected path ending in daemon.sock, got: {}",
            p.display()
        );
    }

    #[test]
    #[cfg(not(target_os = "windows"))]
    fn socket_path_default_contains_copypaste() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // SAFETY: serialised via ENV_LOCK.
        unsafe { std::env::remove_var("COPYPASTE_SOCKET") };
        let p = socket_path();
        // macOS uses "CopyPaste"; Linux uses lowercase "copypaste".
        assert!(
            p.to_string_lossy().to_lowercase().contains("copypaste"),
            "expected socket path to contain 'copypaste', got: {}",
            p.display()
        );
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn socket_path_macos_uses_application_support() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // SAFETY: serialised via ENV_LOCK.
        unsafe { std::env::remove_var("COPYPASTE_SOCKET") };
        let p = socket_path();
        assert!(
            p.to_string_lossy().contains("Library/Application Support"),
            "macOS socket path should be under Library/Application Support, got: {}",
            p.display()
        );
    }

    #[test]
    #[cfg(all(unix, not(target_os = "macos")))]
    fn socket_path_linux_uses_local_share_or_xdg() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // SAFETY: serialised via ENV_LOCK.
        unsafe { std::env::remove_var("COPYPASTE_SOCKET") };
        unsafe { std::env::remove_var("XDG_DATA_HOME") };
        let p = socket_path();
        assert!(
            p.to_string_lossy().contains(".local/share/copypaste"),
            "Linux socket path should be under .local/share/copypaste, got: {}",
            p.display()
        );
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn socket_path_windows_is_named_pipe() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { std::env::remove_var("COPYPASTE_SOCKET") };
        let p = socket_path();
        assert_eq!(p, PathBuf::from(r"\\.\pipe\copypaste-daemon"));
    }
}
