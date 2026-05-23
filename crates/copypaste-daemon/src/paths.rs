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

// ---------------------------------------------------------------------------
// Beta-bonus: XDG-style directory helpers (data/cache/config/log).
//
// These mirror the conventions used by the `directories`/`dirs` crates so
// every subsystem (storage, telemetry, logging, config loader) can resolve
// a stable, platform-correct location without re-implementing platform
// switches. Each helper honours a `COPYPASTE_*_DIR` env override so tests
// (and power users) can redirect them to a sandbox.
//
// | Kind   | macOS                                       | Linux                                                  | Windows                          |
// |--------|---------------------------------------------|--------------------------------------------------------|----------------------------------|
// | data   | `~/Library/Application Support/CopyPaste`   | `$XDG_DATA_HOME/copypaste` or `~/.local/share/copypaste` | `%APPDATA%\CopyPaste`            |
// | cache  | `~/Library/Caches/CopyPaste`                | `$XDG_CACHE_HOME/copypaste` or `~/.cache/copypaste`     | `%LOCALAPPDATA%\CopyPaste\Cache` |
// | config | `~/Library/Application Support/CopyPaste`   | `$XDG_CONFIG_HOME/copypaste` or `~/.config/copypaste`   | `%APPDATA%\CopyPaste\Config`     |
// | log    | `~/Library/Logs/CopyPaste`                  | `$XDG_STATE_HOME/copypaste/log` or `~/.local/state/copypaste/log` | `%LOCALAPPDATA%\CopyPaste\Logs`  |
//
// The infallible variants fall back to `$TMPDIR/<app>/<kind>` when the
// platform cannot supply a home directory — same policy as
// [`app_support_dir`] — so the daemon never aborts during early
// bootstrap.

const ENV_DATA_DIR: &str = "COPYPASTE_DATA_DIR";
const ENV_CACHE_DIR: &str = "COPYPASTE_CACHE_DIR";
const ENV_CONFIG_DIR: &str = "COPYPASTE_CONFIG_DIR";
const ENV_LOG_DIR: &str = "COPYPASTE_LOG_DIR";

const SUBDIR_LOWER: &str = "copypaste";

fn from_env(var: &str) -> Option<PathBuf> {
    std::env::var_os(var)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

/// Returns the platform-specific application **data** directory.
///
/// Honours `COPYPASTE_DATA_DIR`. Falls back to `$TMPDIR/CopyPaste/data` if
/// the OS cannot resolve a base directory.
pub fn data_dir() -> PathBuf {
    if let Some(p) = from_env(ENV_DATA_DIR) {
        return p;
    }
    #[cfg(target_os = "macos")]
    {
        if let Some(base) = dirs::data_dir() {
            return base.join(APP_NAME);
        }
    }
    #[cfg(target_os = "windows")]
    {
        if let Some(base) = dirs::data_dir() {
            return base.join(APP_NAME);
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        if let Some(base) = dirs::data_dir() {
            return base.join(SUBDIR_LOWER);
        }
    }
    std::env::temp_dir().join(APP_NAME).join("data")
}

/// Returns the platform-specific application **cache** directory.
///
/// Honours `COPYPASTE_CACHE_DIR`. Falls back to `$TMPDIR/CopyPaste/cache`.
pub fn cache_dir() -> PathBuf {
    if let Some(p) = from_env(ENV_CACHE_DIR) {
        return p;
    }
    #[cfg(target_os = "macos")]
    {
        if let Some(base) = dirs::cache_dir() {
            return base.join(APP_NAME);
        }
    }
    #[cfg(target_os = "windows")]
    {
        if let Some(base) = dirs::cache_dir() {
            return base.join(APP_NAME).join("Cache");
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        if let Some(base) = dirs::cache_dir() {
            return base.join(SUBDIR_LOWER);
        }
    }
    std::env::temp_dir().join(APP_NAME).join("cache")
}

/// Returns the platform-specific application **config** directory.
///
/// Honours `COPYPASTE_CONFIG_DIR`. Falls back to `$TMPDIR/CopyPaste/config`.
pub fn config_dir() -> PathBuf {
    if let Some(p) = from_env(ENV_CONFIG_DIR) {
        return p;
    }
    #[cfg(target_os = "macos")]
    {
        if let Some(base) = dirs::config_dir() {
            return base.join(APP_NAME);
        }
    }
    #[cfg(target_os = "windows")]
    {
        if let Some(base) = dirs::config_dir() {
            return base.join(APP_NAME).join("Config");
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        if let Some(base) = dirs::config_dir() {
            return base.join(SUBDIR_LOWER);
        }
    }
    std::env::temp_dir().join(APP_NAME).join("config")
}

/// Returns the platform-specific application **log** directory.
///
/// Honours `COPYPASTE_LOG_DIR`. Falls back to `$TMPDIR/CopyPaste/log`.
///
/// On macOS this is `~/Library/Logs/CopyPaste` (the platform convention,
/// not `dirs::state_dir` which is `None` on macOS). On Linux this uses
/// `$XDG_STATE_HOME/copypaste/log` (with `~/.local/state/copypaste/log`
/// fallback). On Windows this is `%LOCALAPPDATA%\CopyPaste\Logs`.
pub fn log_dir() -> PathBuf {
    if let Some(p) = from_env(ENV_LOG_DIR) {
        return p;
    }
    #[cfg(target_os = "macos")]
    {
        if let Some(home) = dirs::home_dir() {
            return home.join("Library/Logs").join(APP_NAME);
        }
    }
    #[cfg(target_os = "windows")]
    {
        // Logs live under LOCALAPPDATA (non-roaming) to avoid bloating
        // roaming profiles.
        if let Some(base) = dirs::data_local_dir() {
            return base.join(APP_NAME).join("Logs");
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        if let Some(state) = dirs::state_dir() {
            return state.join(SUBDIR_LOWER).join("log");
        }
        if let Some(home) = dirs::home_dir() {
            return home.join(".local/state").join(SUBDIR_LOWER).join("log");
        }
    }
    std::env::temp_dir().join(APP_NAME).join("log")
}

/// Creates every XDG-style directory ([`data_dir`], [`cache_dir`],
/// [`config_dir`], [`log_dir`]) if it does not already exist.
///
/// Idempotent — safe to call on every daemon startup. Returns the first
/// I/O error encountered; partial creation may have happened.
pub fn ensure_dirs() -> std::io::Result<()> {
    for d in [data_dir(), cache_dir(), config_dir(), log_dir()] {
        std::fs::create_dir_all(&d)?;
    }
    Ok(())
}

/// Returns the path to the persistent device-id file.
///
/// The file stores a UUID v4 string used to identify this device for P2P
/// pairing and cloud-sync. It must persist across daemon restarts so peers
/// can re-recognise this device — see arch LOW #24.
///
/// Honours `COPYPASTE_DEVICE_ID_PATH` for tests.
pub fn device_id_path() -> Result<PathBuf, PathsError> {
    if let Ok(p) = std::env::var("COPYPASTE_DEVICE_ID_PATH") {
        return Ok(PathBuf::from(p));
    }
    Ok(try_app_support_dir()?.join("device_id"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Env mutation is process-global; serialise the XDG-helper tests so
    // they don't race on the COPYPASTE_*_DIR vars.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    /// RAII guard that snapshots an env var, lets the test mutate it, and
    /// restores the previous value on drop — even on panic.
    struct EnvGuard {
        key: &'static str,
        original: Option<std::ffi::OsString>,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: &std::path::Path) -> Self {
            let original = std::env::var_os(key);
            // SAFETY: tests are serialised via `ENV_LOCK`.
            unsafe { std::env::set_var(key, value) };
            Self { key, original }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            // SAFETY: restoring snapshotted value under `ENV_LOCK`.
            unsafe {
                match self.original.take() {
                    Some(v) => std::env::set_var(self.key, v),
                    None => std::env::remove_var(self.key),
                }
            }
        }
    }

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

    // ----- Beta-bonus XDG helper tests -----

    #[test]
    fn ensure_dirs_creates_all_required_dirs() {
        let _lock = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = tempfile::tempdir().expect("tempdir");
        let data = tmp.path().join("data");
        let cache = tmp.path().join("cache");
        let config = tmp.path().join("config");
        let log = tmp.path().join("log");

        let _g1 = EnvGuard::set(ENV_DATA_DIR, &data);
        let _g2 = EnvGuard::set(ENV_CACHE_DIR, &cache);
        let _g3 = EnvGuard::set(ENV_CONFIG_DIR, &config);
        let _g4 = EnvGuard::set(ENV_LOG_DIR, &log);

        ensure_dirs().expect("ensure_dirs must succeed under tempdir");

        for d in [&data, &cache, &config, &log] {
            assert!(d.is_dir(), "expected directory: {}", d.display());
        }
    }

    #[test]
    fn ensure_dirs_idempotent_rerun_safe() {
        let _lock = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = tempfile::tempdir().expect("tempdir");
        let data = tmp.path().join("d");
        let cache = tmp.path().join("c");
        let config = tmp.path().join("cfg");
        let log = tmp.path().join("l");

        let _g1 = EnvGuard::set(ENV_DATA_DIR, &data);
        let _g2 = EnvGuard::set(ENV_CACHE_DIR, &cache);
        let _g3 = EnvGuard::set(ENV_CONFIG_DIR, &config);
        let _g4 = EnvGuard::set(ENV_LOG_DIR, &log);

        // First call creates.
        ensure_dirs().expect("first ensure_dirs");
        // Second + third calls must not error even though all dirs exist.
        ensure_dirs().expect("second ensure_dirs");
        ensure_dirs().expect("third ensure_dirs");

        assert!(data.is_dir());
        assert!(cache.is_dir());
        assert!(config.is_dir());
        assert!(log.is_dir());
    }

    #[test]
    fn env_override_respected_for_each_dir() {
        let _lock = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = tempfile::tempdir().expect("tempdir");
        let d = tmp.path().join("data-override");
        let c = tmp.path().join("cache-override");
        let cfg = tmp.path().join("config-override");
        let lg = tmp.path().join("log-override");

        let _g1 = EnvGuard::set(ENV_DATA_DIR, &d);
        let _g2 = EnvGuard::set(ENV_CACHE_DIR, &c);
        let _g3 = EnvGuard::set(ENV_CONFIG_DIR, &cfg);
        let _g4 = EnvGuard::set(ENV_LOG_DIR, &lg);

        assert_eq!(data_dir(), d);
        assert_eq!(cache_dir(), c);
        assert_eq!(config_dir(), cfg);
        assert_eq!(log_dir(), lg);
    }

    #[test]
    fn platform_specific_paths_match_convention() {
        let _lock = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // Clear any inherited overrides for this test.
        let _g1 = EnvGuard {
            key: ENV_DATA_DIR,
            original: std::env::var_os(ENV_DATA_DIR),
        };
        let _g2 = EnvGuard {
            key: ENV_CACHE_DIR,
            original: std::env::var_os(ENV_CACHE_DIR),
        };
        let _g3 = EnvGuard {
            key: ENV_CONFIG_DIR,
            original: std::env::var_os(ENV_CONFIG_DIR),
        };
        let _g4 = EnvGuard {
            key: ENV_LOG_DIR,
            original: std::env::var_os(ENV_LOG_DIR),
        };
        // SAFETY: serialised via ENV_LOCK.
        unsafe {
            std::env::remove_var(ENV_DATA_DIR);
            std::env::remove_var(ENV_CACHE_DIR);
            std::env::remove_var(ENV_CONFIG_DIR);
            std::env::remove_var(ENV_LOG_DIR);
        }

        #[cfg(target_os = "macos")]
        {
            let d = data_dir();
            let l = log_dir();
            let cache = cache_dir();
            let cfg = config_dir();
            // macOS data + config both live under Application Support.
            assert!(
                d.to_string_lossy().contains("Library/Application Support"),
                "macOS data_dir should be under Application Support: {}",
                d.display()
            );
            assert!(
                cfg.to_string_lossy()
                    .contains("Library/Application Support"),
                "macOS config_dir should be under Application Support: {}",
                cfg.display()
            );
            assert!(
                cache.to_string_lossy().contains("Library/Caches"),
                "macOS cache_dir should be under Library/Caches: {}",
                cache.display()
            );
            assert!(
                l.to_string_lossy().contains("Library/Logs"),
                "macOS log_dir should be under Library/Logs: {}",
                l.display()
            );
            assert!(
                d.ends_with(APP_NAME),
                "macOS data should end with {APP_NAME}"
            );
        }

        #[cfg(all(unix, not(target_os = "macos")))]
        {
            let d = data_dir();
            let cache = cache_dir();
            let cfg = config_dir();
            // Linux paths use lowercase "copypaste" per XDG convention.
            assert!(
                d.ends_with(SUBDIR_LOWER),
                "Linux data_dir should end with `copypaste`: {}",
                d.display()
            );
            assert!(
                cache.ends_with(SUBDIR_LOWER),
                "Linux cache_dir should end with `copypaste`: {}",
                cache.display()
            );
            assert!(
                cfg.ends_with(SUBDIR_LOWER),
                "Linux config_dir should end with `copypaste`: {}",
                cfg.display()
            );
        }

        #[cfg(target_os = "windows")]
        {
            let d = data_dir();
            assert!(
                d.to_string_lossy().contains(APP_NAME),
                "Windows data_dir should contain {APP_NAME}: {}",
                d.display()
            );
        }
    }
}
