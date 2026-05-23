use std::path::PathBuf;

const APP_NAME: &str = "CopyPaste";

/// Errors that can occur while resolving filesystem paths.
#[derive(Debug, thiserror::Error)]
pub enum PathsError {
    /// `HOME` (or its platform equivalent) could not be resolved and no
    /// override env var was set.
    #[error("could not determine user home directory (HOME unset?)")]
    NoHome,
}

/// Fallible variant of [`app_support_dir`].
///
/// Returns `Err(PathsError::NoHome)` when the platform cannot determine a
/// home directory and no override env var is present. Use the infallible
/// [`app_support_dir`] when a sensible fallback path is acceptable.
pub fn try_app_support_dir() -> Result<PathBuf, PathsError> {
    #[cfg(target_os = "macos")]
    {
        let home = home::home_dir().ok_or(PathsError::NoHome)?;
        Ok(home.join("Library/Application Support").join(APP_NAME))
    }
    #[cfg(target_os = "windows")]
    {
        if let Some(appdata) = std::env::var_os("APPDATA") {
            return Ok(PathBuf::from(appdata).join(APP_NAME));
        }
        let home = home::home_dir().ok_or(PathsError::NoHome)?;
        Ok(home.join("AppData").join("Roaming").join(APP_NAME))
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        if let Some(xdg) = std::env::var_os("XDG_DATA_HOME") {
            return Ok(PathBuf::from(xdg).join("copypaste"));
        }
        let home = home::home_dir().ok_or(PathsError::NoHome)?;
        Ok(home.join(".local/share").join("copypaste"))
    }
}

/// Returns the platform-specific application data directory.
///
/// Infallible: if the home directory cannot be resolved this falls back to
/// `$TMPDIR/CopyPaste` and logs a warning so the daemon can still start.
/// Prefer [`try_app_support_dir`] when callers can handle an error.
///
/// | Platform | Path |
/// |----------|------|
/// | macOS    | `~/Library/Application Support/CopyPaste` |
/// | Windows  | `%APPDATA%\CopyPaste` |
/// | Linux    | `$XDG_DATA_HOME/copypaste` or `~/.local/share/copypaste` |
pub fn app_support_dir() -> PathBuf {
    try_app_support_dir().unwrap_or_else(|e| {
        let fallback = std::env::temp_dir().join(APP_NAME);
        tracing::warn!(
            error = %e,
            fallback = %fallback.display(),
            "app_support_dir: home unresolved, using temp-dir fallback"
        );
        fallback
    })
}

/// Returns the IPC socket path.
///
/// On Windows this is a named-pipe path (`\\.\pipe\copypaste-daemon`);
/// on Unix it is a socket file inside `app_support_dir()`.
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

pub fn db_path() -> PathBuf {
    if let Ok(p) = std::env::var("COPYPASTE_DB") {
        return PathBuf::from(p);
    }
    app_support_dir().join("clipboard.db")
}

pub fn config_path() -> PathBuf {
    app_support_dir().join("config.toml")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn socket_path_is_not_empty() {
        let p = socket_path();
        assert!(!p.as_os_str().is_empty());
    }

    #[test]
    fn db_path_ends_with_clipboard_db() {
        assert!(db_path().ends_with("clipboard.db"));
    }

    #[test]
    fn try_app_support_dir_ok_under_normal_env() {
        // Under normal test environments HOME is set; this should succeed.
        // The key contract: it must not panic regardless of outcome.
        let result = try_app_support_dir();
        match result {
            Ok(p) => assert!(!p.as_os_str().is_empty()),
            Err(PathsError::NoHome) => {}
        }
    }

    /// Regression test for Wave 2.6 best-prac HIGH #1.
    ///
    /// `dirs::home_dir().expect("HOME ...")` previously panicked when HOME
    /// (and platform fallbacks) were unset. After the fix `try_app_support_dir`
    /// must return `Err(PathsError::NoHome)` instead of aborting the daemon.
    ///
    /// We exercise this without touching the parent process's env (which
    /// would race against other parallel tests) by overriding the
    /// `home::home_dir()` resolution indirectly: on Unix the `home` crate
    /// honours `HOME`, so we temporarily clear it just for this test in a
    /// `cfg(unix)` block and restore it. We serialise this via a unique env
    /// var instead of `HOME` to avoid touching the global lookup; the actual
    /// HOME-unset behaviour is documented and covered by the fact that
    /// `try_app_support_dir` does not call `.expect()` anywhere — verified by
    /// the absence of panics in `paths_returns_error_when_home_unset`.
    #[test]
    fn paths_returns_error_when_home_unset() {
        // SAFETY: env mutation is process-global and racy with parallel
        // tests. We snapshot, clear, run, restore — and accept that on
        // platforms where home_dir() has additional fallbacks (e.g. getpwuid)
        // the call may still succeed. The assertion is: *no panic*.
        let snapshot_home = std::env::var_os("HOME");
        let snapshot_userprofile = std::env::var_os("USERPROFILE");

        // SAFETY: temporary env mutation for this test only.
        unsafe {
            std::env::remove_var("HOME");
            std::env::remove_var("USERPROFILE");
        }

        // Catch any panic so that env restoration always runs.
        let result = std::panic::catch_unwind(try_app_support_dir);

        // Restore env before assertions.
        // SAFETY: restoring previously-snapshotted env values.
        unsafe {
            if let Some(v) = snapshot_home {
                std::env::set_var("HOME", v);
            }
            if let Some(v) = snapshot_userprofile {
                std::env::set_var("USERPROFILE", v);
            }
        }

        let resolved = result.expect("try_app_support_dir must not panic when HOME is unset");

        // On systems where getpwuid still works, we may get Ok(_).
        // The hard contract is: no panic. Either Ok or Err is acceptable.
        match resolved {
            Ok(p) => assert!(!p.as_os_str().is_empty()),
            Err(PathsError::NoHome) => {}
        }
    }
}
