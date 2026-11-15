//! Local storage accounting, cache clearing and pre-upgrade backups.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum StorageOp {
    /// Measure local storage by what it holds.
    Footprint,
    /// Drop derived previews without touching history.
    ClearCache,
    /// The encrypted copies kept before data-format upgrades, newest first.
    UpgradeBackups,
}

impl crate::OpGates for StorageOp {
    fn requires_ready(&self) -> bool {
        !matches!(self, Self::UpgradeBackups)
    }

    fn interactive(&self) -> bool {
        false
    }

    fn blocking_weight(&self) -> u32 {
        0
    }

    fn is_long_running(&self) -> bool {
        false
    }

    fn privileged(&self) -> bool {
        matches!(self, Self::ClearCache)
    }
}

/// Local storage split by what it holds, in on-disk bytes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorageFootprint {
    pub history_bytes: u64,
    pub file_bytes: u64,
    pub cache_bytes: u64,
    pub upgrade_backup_bytes: u64,
    pub other_bytes: u64,
}

/// One encrypted copy kept before a data-format upgrade.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpgradeBackupInfo {
    /// Pass to [`crate::Method::Restore`] to restore this copy.
    pub path: String,
    pub created_at_ms: i64,
    pub from_version: u32,
    pub size_bytes: u64,
}
