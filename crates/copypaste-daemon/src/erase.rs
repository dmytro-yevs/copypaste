//! One-shot removal of the device secret after the app deleted its history.
//!
//! It runs in the daemon executable because the macOS Keychain accessor
//! trusts only the daemon's signature.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use anyhow::{ensure, Context};
use copypaste_core::Keyring;

pub use copypaste_ipc::ERASE_DEVICE_SECRET_ARGUMENT as ARGUMENT;

/// A secret shared by every history of this account is kept while another
/// history on this account (the standalone daemon's) still opens with it.
pub fn erase_device_secret(data_dir: Option<OsString>) -> anyhow::Result<()> {
    let data_dir = PathBuf::from(data_dir.context("a data directory is required")?);
    ensure!(
        data_dir.is_absolute(),
        "the data directory must be absolute"
    );
    if Keyring::device_secret_is_shared() && other_history_present(&data_dir) {
        return Ok(());
    }
    Keyring::erase_device_secret(&data_dir).context("remove the device secret")
}

fn other_history_present(data_dir: &Path) -> bool {
    let standalone = copypaste_ipc::database_path();
    standalone.parent() != Some(data_dir) && standalone.exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_relative_or_missing_directory_is_refused() {
        assert!(erase_device_secret(None).is_err());
        assert!(erase_device_secret(Some("relative".into())).is_err());
    }
}
