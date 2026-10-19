//! Encrypted backup and restore policy shared by platform hosts.
use crate::history_error::HistoryError;
use crate::Store;
use std::path::{Path, PathBuf};

fn clean_path(raw: &str) -> Result<PathBuf, HistoryError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        Err(HistoryError::BadPath)
    } else {
        Ok(PathBuf::from(trimmed))
    }
}

pub fn backup(store: &Store, destination: &str) -> Result<u64, HistoryError> {
    let path = clean_path(destination)?;
    if path.exists() {
        return Err(HistoryError::Exists);
    }
    if !path.parent().is_some_and(Path::is_dir) {
        return Err(HistoryError::BackupMissingDirectory);
    }
    if let Err(error) = store.backup_to(&path) {
        tracing::warn!(?error, "encrypted backup failed");
        let _ = std::fs::remove_file(&path);
        return Err(HistoryError::Backup);
    }
    restrict_backup(&path);
    Ok(std::fs::metadata(path)
        .map(|metadata| metadata.len())
        .unwrap_or(0))
}

/// Returns the restored history floor; the host publishes it before waking sync.
pub fn restore(
    store: &Store,
    db_key: &[u8; 32],
    source: &str,
    confirm: bool,
) -> Result<Option<i64>, HistoryError> {
    if !confirm {
        return Err(HistoryError::NeedsConfirmation);
    }
    let path = clean_path(source)?;
    if !path.is_file() {
        return Err(HistoryError::BackupNotFound);
    }
    store
        .restore_from(&path, db_key)
        .map_err(|error| match error {
            crate::RestoreError::InvalidBackup(error) => {
                tracing::warn!(?error, "encrypted backup validation failed");
                HistoryError::InvalidBackup
            }
            crate::RestoreError::Failed(error) => {
                tracing::warn!(?error, "encrypted restore failed");
                HistoryError::Restore
            }
        })?;
    Ok(store.oldest_version_ms().ok().flatten())
}

#[cfg(unix)]
fn restrict_backup(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    if let Err(error) = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)) {
        tracing::warn!(?error, "could not restrict the backup to its owner");
    }
}
#[cfg(not(unix))]
fn restrict_backup(_path: &Path) {}
