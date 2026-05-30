//! Platform paths for the CopyPaste CLI.
//!
//! Only macOS is supported (ADR-012: Windows frozen/Homebrew-only).
//! A named-pipe variant (`\\.\pipe\copypaste-daemon`) was considered for
//! Windows but is aspirational and unused — `ipc.rs` uses `UnixStream` which
//! does not compile on Windows.  If Windows support is ever unfrozen, both
//! this file and `ipc.rs` will need platform-specific transports.

use std::path::PathBuf;

pub fn socket_path() -> PathBuf {
    if let Ok(p) = std::env::var("COPYPASTE_SOCKET") {
        return PathBuf::from(p);
    }
    home::home_dir()
        .expect("HOME directory must exist")
        .join("Library/Application Support/CopyPaste/daemon.sock")
}

#[allow(dead_code)]
pub fn db_path() -> PathBuf {
    if let Ok(p) = std::env::var("COPYPASTE_DB") {
        return PathBuf::from(p);
    }
    home::home_dir()
        .expect("HOME directory must exist")
        .join("Library/Application Support/CopyPaste/clipboard.db")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn socket_path_ends_with_daemon_sock() {
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
        std::env::remove_var("COPYPASTE_SOCKET");
        let p = socket_path();
        assert!(
            p.to_string_lossy().contains("CopyPaste"),
            "expected path to contain CopyPaste, got: {}",
            p.display()
        );
    }

    #[test]
    fn socket_path_env_override() {
        std::env::set_var("COPYPASTE_SOCKET", "/tmp/test.sock");
        let p = socket_path();
        std::env::remove_var("COPYPASTE_SOCKET");
        assert_eq!(p, PathBuf::from("/tmp/test.sock"));
    }

    #[test]
    fn db_path_env_override() {
        std::env::set_var("COPYPASTE_DB", "/tmp/test.db");
        let p = db_path();
        std::env::remove_var("COPYPASTE_DB");
        assert_eq!(p, PathBuf::from("/tmp/test.db"));
    }

    #[test]
    fn db_path_default_contains_copypaste() {
        std::env::remove_var("COPYPASTE_DB");
        let p = db_path();
        assert!(
            p.to_string_lossy().contains("CopyPaste"),
            "expected path to contain CopyPaste, got: {}",
            p.display()
        );
    }
}
