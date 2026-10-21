//! Bounded durable records; progress notifications carry no file metadata.
use super::{Store, StoreError};
use copypaste_ipc::{FileOperation, FileOperationPhase};
use rusqlite::{params, OptionalExtension};

impl Store {
    pub fn write_file_operation(&self, operation: &FileOperation) -> Result<(), StoreError> {
        let mut operation = operation.clone();
        let phase = if operation.phase == FileOperationPhase::Running {
            "running"
        } else {
            "finished"
        };
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        let hidden: bool = tx.query_row("SELECT COALESCE((SELECT json_extract(payload_metadata,'$.privacy.secret') FROM clipboard_items WHERE id=?1),0)", [&operation.item_id], |row| row.get(0))?;
        if hidden {
            operation.name = "Hidden file".into();
        }
        let record = serde_json::to_string(&operation).map_err(|_| StoreError::InvalidPayload)?;
        tx.execute("INSERT INTO file_operations VALUES (?1,?2,?3,?4,?5) ON CONFLICT(id) DO UPDATE SET item_id=excluded.item_id,phase=excluded.phase,record=excluded.record", params![operation.id,operation.item_id,phase,operation.started_at,record])?;
        tx.commit()?;
        let _ = self
            .file_operation_changes
            .send(copypaste_ipc::FileOperationUpdate {
                id: Some(operation.id.clone()),
                phase: Some(operation.phase.clone()),
                written: operation.written,
                total: operation.total,
            });
        Ok(())
    }

    pub fn file_operations(
        &self,
        clear: bool,
        before: Option<&str>,
        id: Option<&str>,
    ) -> Result<Vec<FileOperation>, StoreError> {
        let conn = self.conn()?;
        if clear {
            conn.execute("DELETE FROM file_operations WHERE phase='finished'", [])?;
            let _ = self
                .file_operation_changes
                .send(copypaste_ipc::FileOperationUpdate::default());
        }
        let (sql, argument) = if let Some(id) = id {
            ("SELECT record,COALESCE((SELECT json_extract(payload_metadata,'$.privacy.secret') FROM clipboard_items WHERE id=file_operations.item_id),0) FROM file_operations WHERE id=?1", id)
        } else if let Some(before) = before {
            ("SELECT record,COALESCE((SELECT json_extract(payload_metadata,'$.privacy.secret') FROM clipboard_items WHERE id=file_operations.item_id),0) FROM file_operations WHERE phase='running' OR id IN (SELECT id FROM file_operations WHERE phase='finished' AND (started_at,id)<(SELECT started_at,id FROM file_operations WHERE id=?1) ORDER BY started_at DESC,id DESC LIMIT 50) ORDER BY started_at DESC,id DESC", before)
        } else {
            ("SELECT record,COALESCE((SELECT json_extract(payload_metadata,'$.privacy.secret') FROM clipboard_items WHERE id=file_operations.item_id),0) FROM file_operations WHERE phase='running' OR id IN (SELECT id FROM file_operations WHERE phase='finished' ORDER BY started_at DESC,id DESC LIMIT 50) ORDER BY started_at DESC,id DESC", "")
        };
        let mut query = conn.prepare(sql)?;
        let mut rows = if id.is_some() || before.is_some() {
            query.query([argument])?
        } else {
            query.query([])?
        };
        let mut records = Vec::new();
        while let Some(row) = rows.next()? {
            let record: String = row.get(0)?;
            let mut operation: FileOperation =
                serde_json::from_str(&record).map_err(|_| StoreError::InvalidPayload)?;
            if row.get::<_, bool>(1)? {
                // The manager must not bypass History's protected file actions.
                operation.name = "Hidden file".into();
                operation.local_path = None;
            }
            records.push(operation);
        }
        Ok(records)
    }

    pub fn downloaded_file_checksum(&self, id: &str) -> Result<Option<String>, StoreError> {
        Ok(self
            .conn()?
            .query_row(
                "SELECT sha256 FROM downloaded_files WHERE item_id=?1",
                [id],
                |r| r.get(0),
            )
            .optional()?)
    }
    pub fn set_downloaded_file_checksum(
        &self,
        id: &str,
        path: &str,
        hash: &str,
    ) -> Result<(), StoreError> {
        self.conn()?.execute("INSERT INTO downloaded_files VALUES (?1,?2,?3) ON CONFLICT(item_id) DO UPDATE SET destination=excluded.destination,sha256=excluded.sha256", params![id,path,hash])?;
        Ok(())
    }
    pub fn subscribe_file_operations(
        &self,
    ) -> tokio::sync::broadcast::Receiver<copypaste_ipc::FileOperationUpdate> {
        self.file_operation_changes.subscribe()
    }

    pub fn file_reference(
        &self,
        id: &str,
    ) -> Result<Option<(String, u64, Option<String>)>, StoreError> {
        Ok(self
            .conn()?
            .query_row(
                "SELECT reference,size_bytes,modified FROM file_references WHERE item_id=?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?)
    }

    pub fn insert_file_reference(
        &self,
        item: super::NewItem,
        reference: &str,
        size: u64,
        modified: Option<&str>,
    ) -> Result<super::StoredItem, StoreError> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        tx.execute("INSERT INTO clipboard_items (id,content_ciphertext,nonce,content_type,content_hash,created_at,app_bundle_id,app_name,payload_metadata,content_bytes) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)", params![item.id,item.content_ciphertext,item.nonce,item.content_type,item.content_hash,item.created_at,item.app_bundle_id,item.app_name,item.payload_metadata,item.content_ciphertext.len()+item.payload_metadata.as_ref().map_or(0, String::len)])?;
        tx.execute(
            "INSERT INTO file_references VALUES (?1,?2,?3,?4)",
            params![item.id, reference, size, modified],
        )?;
        tx.commit()?;
        drop(conn);
        self.get(&item.id)?.ok_or(StoreError::NotFound)
    }
    pub fn retains_file_reference(&self, reference: &str) -> Result<bool, StoreError> {
        Ok(self.conn()?.query_row("SELECT EXISTS(SELECT 1 FROM file_references f JOIN clipboard_items i ON i.id=f.item_id WHERE f.reference=?1 AND i.deleted=0)", [reference], |row| row.get(0))?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use copypaste_ipc::FileOperationKind;
    #[test]
    fn history_survives_reopen_pages_without_loss_and_clear_preserves_active_and_files() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("history.db");
        let saved = directory.path().join("saved.bin");
        std::fs::write(&saved, b"keep").unwrap();
        let key = [47; 32];
        let store = Store::open(&path, &key).unwrap();
        for index in 0..55 {
            let operation = FileOperation {
                id: format!("operation-{index:03}"),
                item_id: "file".into(),
                name: "file.bin".into(),
                kind: FileOperationKind::Receive,
                phase: FileOperationPhase::Completed,
                source_device: "Phone".into(),
                destination_device: "Desktop".into(),
                written: 4,
                total: 4,
                started_at: index,
                finished_at: Some(index),
                error: None,
                local_path: Some(saved.to_string_lossy().into()),
            };
            store.write_file_operation(&operation).unwrap();
        }
        let first = store.file_operations(false, None, None).unwrap();
        assert_eq!(first.len(), 50);
        let earlier = store
            .file_operations(false, Some(&first.last().unwrap().id), None)
            .unwrap();
        assert_eq!(earlier.len(), 5);
        let mut active = first[0].clone();
        active.id = "active".into();
        active.phase = FileOperationPhase::Running;
        active.finished_at = None;
        store.write_file_operation(&active).unwrap();
        let cleared = store.file_operations(true, None, None).unwrap();
        assert_eq!(cleared.len(), 1);
        assert_eq!(cleared[0].phase, FileOperationPhase::Running);
        assert!(saved.exists());
        drop(store);
        let reopened = Store::open(&path, &key).unwrap();
        assert_eq!(
            reopened.file_operations(false, None, None).unwrap()[0].phase,
            FileOperationPhase::Interrupted
        );
    }
}
