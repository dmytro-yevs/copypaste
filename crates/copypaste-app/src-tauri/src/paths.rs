use std::path::PathBuf;

pub fn socket_path() -> PathBuf {
    home::home_dir()
        .expect("HOME directory must exist")
        .join("Library/Application Support/CopyPaste/daemon.sock")
}
