use std::path::PathBuf;

const APP_NAME: &str = "CopyPaste";

pub fn app_support_dir() -> PathBuf {
    home::home_dir()
        .expect("HOME directory must exist")
        .join("Library/Application Support")
        .join(APP_NAME)
}

#[cfg(unix)]
pub fn socket_path() -> PathBuf {
    app_support_dir().join("daemon.sock")
}

pub fn db_path() -> PathBuf {
    app_support_dir().join("clipboard.db")
}

pub fn config_path() -> PathBuf {
    app_support_dir().join("config.toml")
}
