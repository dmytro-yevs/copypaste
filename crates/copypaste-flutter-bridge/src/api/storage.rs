//! Local storage usage, cache clearing, pre-upgrade backups and the
//! startup clipboard restore.

use copypaste_ipc::storage::{StorageFootprint, StorageOp};
use copypaste_ipc::{Method, ResponseData};

use super::{expect_data, RuntimeError};

/// Local storage split by what it holds, in on-disk bytes.
pub struct StorageBreakdown {
    pub history_bytes: u64,
    pub file_bytes: u64,
    pub cache_bytes: u64,
    pub upgrade_backup_bytes: u64,
    pub other_bytes: u64,
}

/// One encrypted copy kept before a data-format upgrade.
pub struct UpgradeBackupEntry {
    /// Restore it with `restore_history(path)`.
    pub path: String,
    pub created_at_ms: i64,
    pub from_version: u32,
    pub size_bytes: u64,
}

fn breakdown(footprint: StorageFootprint) -> StorageBreakdown {
    StorageBreakdown {
        history_bytes: footprint.history_bytes,
        file_bytes: footprint.file_bytes,
        cache_bytes: footprint.cache_bytes,
        upgrade_backup_bytes: footprint.upgrade_backup_bytes,
        other_bytes: footprint.other_bytes,
    }
}

pub async fn storage_breakdown() -> Result<StorageBreakdown, RuntimeError> {
    Ok(breakdown(expect_data!(
        Method::Storage {
            op: StorageOp::Footprint
        },
        ResponseData::StorageFootprint
    )))
}

/// Drops derived previews and returns the storage that remains.
pub async fn clear_storage_cache() -> Result<StorageBreakdown, RuntimeError> {
    Ok(breakdown(expect_data!(
        Method::Storage {
            op: StorageOp::ClearCache
        },
        ResponseData::StorageFootprint
    )))
}

pub async fn upgrade_backups() -> Result<Vec<UpgradeBackupEntry>, RuntimeError> {
    let backups = expect_data!(
        Method::Storage {
            op: StorageOp::UpgradeBackups
        },
        ResponseData::UpgradeBackups
    );
    Ok(backups
        .into_iter()
        .map(|backup| UpgradeBackupEntry {
            path: backup.path,
            created_at_ms: backup.created_at_ms,
            from_version: backup.from_version,
            size_bytes: backup.size_bytes,
        })
        .collect())
}

/// Android only: the desktop daemon restores at its own start.
pub async fn restore_clipboard_on_start() -> bool {
    #[cfg(target_os = "android")]
    return tokio::task::spawn_blocking(crate::runtime_android::restore_clipboard_on_start)
        .await
        .unwrap_or(false);
    #[cfg(not(target_os = "android"))]
    false
}
