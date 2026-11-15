//! Local storage accounting, cache clearing and pre-upgrade backups.

use copypaste_core::upgrade_backups;
use copypaste_ipc::storage::{StorageFootprint, StorageOp, UpgradeBackupInfo};
use copypaste_ipc::{Response, ResponseData};

use crate::messages::storage_error;
use crate::Runtime;

impl Runtime {
    pub(crate) fn storage(&self, id: u64, op: StorageOp) -> Response {
        match op {
            StorageOp::Footprint => self.storage_footprint(id),
            StorageOp::ClearCache => match self.store.clear_preview_cache() {
                Ok(_) => self.storage_footprint(id),
                Err(error) => storage_error(id, "clear cache", &error),
            },
            StorageOp::UpgradeBackups => Response::ok(
                id,
                ResponseData::UpgradeBackups(
                    upgrade_backups::list(&self.data_dir.join(upgrade_backups::DIR_NAME))
                        .into_iter()
                        .map(|backup| UpgradeBackupInfo {
                            path: backup.path.to_string_lossy().into_owned(),
                            created_at_ms: backup.created_at_ms,
                            from_version: backup.from_version,
                            size_bytes: backup.size_bytes,
                        })
                        .collect(),
                ),
            ),
        }
    }

    fn storage_footprint(&self, id: u64) -> Response {
        match self.store.footprint(&self.data_dir) {
            Ok(footprint) => Response::ok(
                id,
                ResponseData::StorageFootprint(StorageFootprint {
                    history_bytes: footprint.history_bytes,
                    file_bytes: footprint.file_bytes,
                    cache_bytes: footprint.cache_bytes,
                    upgrade_backup_bytes: footprint.upgrade_backup_bytes,
                    other_bytes: footprint.other_bytes,
                }),
            ),
            Err(error) => storage_error(id, "storage footprint", &error),
        }
    }
}
