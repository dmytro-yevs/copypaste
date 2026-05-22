use std::path::PathBuf;

const APP_NAME: &str = "CopyPaste";

/// Returns the platform-specific application data directory.
///
/// | Platform | Path |
/// |----------|------|
/// | macOS    | `~/Library/Application Support/CopyPaste` |
/// | Windows  | `%APPDATA%\CopyPaste` |
/// | Linux    | `$XDG_DATA_HOME/copypaste` or `~/.local/share/copypaste` |
pub fn app_support_dir() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        home::home_dir()
            .expect("HOME directory must exist")
            .join("Library/Application Support")
            .join(APP_NAME)
    }
    #[cfg(target_os = "windows")]
    {
        // %APPDATA% → C:\Users\<name>\AppData\Roaming
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                home::home_dir()
                    .expect("HOME must exist")
                    .join("AppData")
                    .join("Roaming")
            })
            .join(APP_NAME)
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        // Linux / other Unix: follow XDG Base Directory spec.
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                home::home_dir()
                    .expect("HOME must exist")
                    .join(".local/share")
            })
            .join("copypaste")
    }
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
}
