//! Where the bytes of local storage go, and the disposable part of them.

use std::path::Path;

use super::{Store, StoreError};

/// Directory entries one measurement visits outside the database, so a data
/// directory with an unexpected number of files cannot stall the request.
const WALK_BUDGET: usize = 20_000;

/// Local storage split by what it holds. Sizes are on-disk bytes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Footprint {
    /// The history database, less the files and previews inside it.
    pub history_bytes: u64,
    /// Encrypted file payloads kept in the history database.
    pub file_bytes: u64,
    /// Decoded previews the history can rebuild on demand.
    pub cache_bytes: u64,
    /// Encrypted copies kept before data-format upgrades.
    pub upgrade_backup_bytes: u64,
    /// Logs, modules, device and sync state.
    pub other_bytes: u64,
}

impl Store {
    /// Measure `data_dir`, which holds this store's database.
    pub fn footprint(&self, data_dir: &Path) -> Result<Footprint, StoreError> {
        let conn = self.conn()?;
        // length() reads a blob's size from its record header, not its pages.
        let (file_bytes, cache_bytes): (i64, i64) = conn.query_row(
            "SELECT \
               COALESCE((SELECT SUM(LENGTH(ciphertext) + LENGTH(nonce)) FROM file_transfer_chunks), 0) + \
               COALESCE((SELECT SUM(LENGTH(p.ciphertext)) FROM shared_binary_payloads p \
                  WHERE p.id IN (SELECT blob_id FROM history_file_payloads)), 0), \
               COALESCE((SELECT SUM(LENGTH(ciphertext) + LENGTH(nonce)) FROM history_preview_cache), 0)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        drop(conn);
        let file_bytes = u64::try_from(file_bytes).unwrap_or(0);
        let cache_bytes = u64::try_from(cache_bytes).unwrap_or(0);

        let database = self
            .path
            .as_deref()
            .and_then(|path| path.file_name())
            .map(|name| name.to_string_lossy().into_owned());
        let mut budget = WALK_BUDGET;
        let (mut database_bytes, mut upgrade_backup_bytes, mut other_bytes) = (0, 0, 0);
        for entry in std::fs::read_dir(data_dir)?.filter_map(Result::ok) {
            let name = entry.file_name().to_string_lossy().into_owned();
            let size = tree_size(&entry.path(), &mut budget);
            if database.as_deref().is_some_and(|db| name.starts_with(db)) {
                database_bytes += size;
            } else if name == crate::upgrade_backups::DIR_NAME {
                upgrade_backup_bytes += size;
            } else {
                other_bytes += size;
            }
        }
        Ok(Footprint {
            history_bytes: database_bytes.saturating_sub(file_bytes + cache_bytes),
            file_bytes,
            cache_bytes,
            upgrade_backup_bytes,
            other_bytes,
        })
    }

    /// Drop every derived preview and return the pages they held to the
    /// file system. History, files and settings are untouched. Returns the
    /// preview bytes dropped.
    pub fn clear_preview_cache(&self) -> Result<u64, StoreError> {
        let conn = self.conn()?;
        let tx = conn.unchecked_transaction()?;
        let freed: i64 = tx.query_row(
            "SELECT COALESCE(SUM(LENGTH(ciphertext) + LENGTH(nonce)), 0) FROM history_preview_cache",
            [],
            |row| row.get(0),
        )?;
        tx.execute("DELETE FROM history_preview_cache", [])?;
        tx.commit()?;
        drop(conn);
        self.request_space_reclaim();
        Ok(u64::try_from(freed).unwrap_or(0))
    }
}

fn tree_size(path: &Path, budget: &mut usize) -> u64 {
    if *budget == 0 {
        return 0;
    }
    *budget -= 1;
    let Ok(metadata) = std::fs::symlink_metadata(path) else {
        return 0;
    };
    if !metadata.is_dir() {
        return metadata.len();
    }
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    entries
        .filter_map(Result::ok)
        .map(|entry| tree_size(&entry.path(), budget))
        .sum()
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{item, T0};
    use super::*;

    const KEY: [u8; 32] = [7u8; 32];

    #[test]
    fn previews_count_as_cache_and_clearing_them_keeps_history() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("history.db"), &KEY).unwrap();
        store.insert(item("kept", T0)).unwrap();
        let conn = store.conn().unwrap();
        let id: String = conn
            .query_row("SELECT id FROM clipboard_items", [], |row| row.get(0))
            .unwrap();
        conn.execute(
            "INSERT INTO history_preview_cache (item_id, nonce, ciphertext) VALUES (?1, X'00', zeroblob(4095))",
            [&id],
        )
        .unwrap();
        drop(conn);
        std::fs::create_dir(dir.path().join("logs")).unwrap();
        std::fs::write(dir.path().join("logs/app.log"), [0u8; 10]).unwrap();

        let before = store.footprint(dir.path()).unwrap();
        assert_eq!(before.cache_bytes, 4096);
        assert_eq!(before.other_bytes, 10);
        assert!(before.history_bytes > 0);

        assert_eq!(store.clear_preview_cache().unwrap(), 4096);
        assert_eq!(store.footprint(dir.path()).unwrap().cache_bytes, 0);
        assert_eq!(store.count().unwrap(), 1);
    }
}
