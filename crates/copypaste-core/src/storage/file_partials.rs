//! Durable progress of interrupted downloads; never synchronized.
use super::{Store, StoreError};
use rusqlite::{params, OptionalExtension};

/// One member's partial download. `verified_bytes` is a chunk boundary whose
/// bytes were flushed to `partial_path` before the row was written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadPartial {
    pub item_id: String,
    pub file_index: usize,
    /// The member's checksum, or its source version when deferred.
    pub file_version: String,
    pub partial_path: String,
    pub destination: String,
    pub verified_bytes: u64,
    pub updated_at: i64,
}

impl Store {
    pub fn put_download_partial(&self, partial: &DownloadPartial) -> Result<(), StoreError> {
        self.conn()?.execute(
            "INSERT INTO file_download_partials (item_id,file_index,file_version,partial_path,destination,verified_bytes,updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(item_id,file_index) DO UPDATE SET file_version=excluded.file_version,partial_path=excluded.partial_path,destination=excluded.destination,verified_bytes=excluded.verified_bytes,updated_at=excluded.updated_at",
            params![
                partial.item_id,
                partial.file_index as i64,
                partial.file_version,
                partial.partial_path,
                partial.destination,
                partial.verified_bytes,
                partial.updated_at
            ],
        )?;
        Ok(())
    }

    pub fn download_partial(
        &self,
        id: &str,
        index: usize,
    ) -> Result<Option<DownloadPartial>, StoreError> {
        Ok(self
            .conn()?
            .query_row(
                "SELECT item_id,file_index,file_version,partial_path,destination,verified_bytes,updated_at FROM file_download_partials WHERE item_id=?1 AND file_index=?2",
                params![id, index as i64],
                partial_from_row,
            )
            .optional()?)
    }

    /// Every recorded partial, oldest first.
    pub fn download_partials(&self) -> Result<Vec<DownloadPartial>, StoreError> {
        let conn = self.conn()?;
        let mut query = conn.prepare(
            "SELECT item_id,file_index,file_version,partial_path,destination,verified_bytes,updated_at FROM file_download_partials ORDER BY updated_at,item_id,file_index",
        )?;
        let rows = query.query_map([], partial_from_row)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn remove_download_partial(&self, id: &str, index: usize) -> Result<(), StoreError> {
        self.conn()?.execute(
            "DELETE FROM file_download_partials WHERE item_id=?1 AND file_index=?2",
            params![id, index as i64],
        )?;
        Ok(())
    }
}

fn partial_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<DownloadPartial> {
    Ok(DownloadPartial {
        item_id: row.get(0)?,
        file_index: row.get::<_, i64>(1)? as usize,
        file_version: row.get(2)?,
        partial_path: row.get(3)?,
        destination: row.get(4)?,
        verified_bytes: row.get(5)?,
        updated_at: row.get(6)?,
    })
}
