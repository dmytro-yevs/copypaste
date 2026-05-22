use std::path::PathBuf;

pub fn socket_path() -> PathBuf {
    if let Ok(p) = std::env::var("COPYPASTE_SOCKET") {
        return PathBuf::from(p);
    }
    home::home_dir()
        .expect("HOME directory must exist")
        .join("Library/Application Support/CopyPaste/daemon.sock")
}
