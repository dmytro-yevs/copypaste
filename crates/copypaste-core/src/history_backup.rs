//! Encrypted backup and restore policy shared by platform hosts.
use crate::history_error::HistoryError;
use crate::Store;
use copypaste_fs::PendingFile;
use copypaste_ipc::TransferProgress;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::Instant;

fn clean_path(raw: &str) -> Result<PathBuf, HistoryError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        Err(HistoryError::BadPath)
    } else {
        Ok(PathBuf::from(trimmed))
    }
}

/// Write an encrypted backup to `destination`, which must not exist, and
/// return its size.
///
/// `on_progress` hears the copied pages at most every
/// [`crate::transfer::PROGRESS_INTERVAL`] plus once at the end; returning
/// `false` abandons the backup. The file appears only once complete.
pub fn backup(
    store: &Store,
    db_key: &[u8; 32],
    destination: &str,
    mut on_progress: impl FnMut(TransferProgress) -> bool,
) -> Result<u64, HistoryError> {
    let path = clean_path(destination)?;
    if path.exists() {
        return Err(HistoryError::Exists);
    }
    if !path.parent().is_some_and(Path::is_dir) {
        return Err(HistoryError::BackupMissingDirectory);
    }
    let pending = PendingFile::create(&path).map_err(write_error)?;
    let mut reported: Option<Instant> = None;
    let copied = store.backup_to(pending.temp_path(), db_key, |processed, total| {
        let due = reported.is_none_or(|at| at.elapsed() >= crate::transfer::PROGRESS_INTERVAL);
        if processed < total && !due {
            return true;
        }
        reported = Some(Instant::now());
        on_progress(TransferProgress { processed, total })
    });
    if let Err(error) = copied {
        tracing::warn!(?error, "encrypted backup failed");
        return Err(HistoryError::Backup);
    }
    let size = std::fs::metadata(pending.temp_path())
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    pending.publish().map_err(write_error)?;
    Ok(size)
}

fn write_error(error: std::io::Error) -> HistoryError {
    if error.kind() == ErrorKind::AlreadyExists {
        return HistoryError::Exists;
    }
    tracing::warn!(?error, "the encrypted backup file could not be written");
    HistoryError::Backup
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::test_support::{item, T0};

    const DB_KEY: [u8; 32] = [5u8; 32];

    fn file_store(dir: &tempfile::TempDir) -> Store {
        let store = Store::open(&dir.path().join("live.db"), &DB_KEY).unwrap();
        for n in 0..64 {
            store
                .insert(item(&format!("row {n} {}", "x".repeat(4096)), T0 + n))
                .unwrap();
        }
        store
    }

    fn temporaries(dir: &tempfile::TempDir) -> usize {
        std::fs::read_dir(dir.path())
            .unwrap()
            .filter(|entry| {
                let name = entry.as_ref().unwrap().file_name();
                name.to_string_lossy().starts_with(".copypaste-")
            })
            .count()
    }

    #[test]
    fn a_backup_reports_page_progress_and_restores() {
        let dir = tempfile::tempdir().unwrap();
        let store = file_store(&dir);
        let dest = dir.path().join("history.copypaste-backup");
        let mut reported = Vec::new();

        let size = backup(&store, &DB_KEY, dest.to_str().unwrap(), |progress| {
            reported.push(progress);
            true
        })
        .unwrap();

        assert_eq!(size, std::fs::metadata(&dest).unwrap().len());
        let last = *reported.last().unwrap();
        assert!(last.total > 0 && last.processed == last.total);
        assert!(reported
            .windows(2)
            .all(|pair| pair[0].processed <= pair[1].processed));
        assert_eq!(Store::open(&dest, &DB_KEY).unwrap().count().unwrap(), 64);
        assert_eq!(temporaries(&dir), 0);
    }

    #[test]
    fn an_abandoned_backup_leaves_nothing_at_the_destination() {
        let dir = tempfile::tempdir().unwrap();
        let store = file_store(&dir);
        let dest = dir.path().join("history.copypaste-backup");

        let result = backup(&store, &DB_KEY, dest.to_str().unwrap(), |_| false);

        assert!(matches!(result, Err(HistoryError::Backup)));
        assert!(!dest.exists());
        assert_eq!(temporaries(&dir), 0);
    }

    #[test]
    fn a_backup_never_replaces_an_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let store = file_store(&dir);
        let dest = dir.path().join("kept");
        std::fs::write(&dest, b"kept").unwrap();

        let result = backup(&store, &DB_KEY, dest.to_str().unwrap(), |_| true);

        assert!(matches!(result, Err(HistoryError::Exists)));
        assert_eq!(std::fs::read(&dest).unwrap(), b"kept");
    }
}
