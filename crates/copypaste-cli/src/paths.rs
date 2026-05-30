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
        assert!(
            p.to_string_lossy().contains("CopyPaste"),
            "expected path to contain CopyPaste, got: {}",
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

    #[test]
    fn db_path_default_contains_copypaste() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        std::env::remove_var("COPYPASTE_DB");
        let p = db_path();
        assert!(
            p.to_string_lossy().contains("CopyPaste"),
            "expected path to contain CopyPaste, got: {}",
            p.display()
        );
    }
}
