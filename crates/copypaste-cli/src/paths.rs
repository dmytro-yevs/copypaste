use std::path::PathBuf;

pub fn socket_path() -> PathBuf {
    home::home_dir()
        .expect("HOME directory must exist")
        .join("Library/Application Support/CopyPaste/daemon.sock")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn socket_path_ends_with_daemon_sock() {
        let p = socket_path();
        assert!(
            p.to_string_lossy().ends_with("daemon.sock"),
            "expected path ending in daemon.sock, got: {}",
            p.display()
        );
    }

    #[test]
    fn socket_path_contains_copypaste() {
        let p = socket_path();
        assert!(
            p.to_string_lossy().contains("CopyPaste"),
            "expected path to contain CopyPaste, got: {}",
            p.display()
        );
    }
}
