//! Runtime-owned file-operation lifecycle, independent of any UI listener.
use crate::Store;
use copypaste_ipc::{FileOperation, FileOperationKind, FileOperationPhase};

pub struct FileOperationGuard {
    store: Store,
    record: FileOperation,
    percent: u64,
}

impl FileOperationGuard {
    pub fn start(
        store: &Store,
        item_id: &str,
        name: &str,
        kind: FileOperationKind,
        source: String,
        destination: String,
        total: u64,
        token: Option<&str>,
    ) -> Self {
        let record = FileOperation {
            id: token
                .map(str::to_owned)
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
            item_id: item_id.into(),
            name: name.chars().take(255).collect(),
            kind,
            phase: FileOperationPhase::Running,
            source_device: source,
            destination_device: destination,
            written: 0,
            total,
            started_at: crate::now_ms(),
            finished_at: None,
            error: None,
            local_path: None,
        };
        if let Err(error) = store.write_file_operation(&record) {
            tracing::warn!(%error, "could not record file operation");
        }
        Self {
            store: store.clone(),
            record,
            percent: u64::MAX,
        }
    }
    pub fn item(&mut self, id: &str, total: u64) {
        self.record.item_id = id.into();
        self.record.total = total;
    }
    pub fn progress(&mut self, written: u64, total: u64) {
        self.record.written = written.min(total);
        self.record.total = total;
        let percent = if total == 0 {
            100
        } else {
            ((u128::from(written) * 100) / u128::from(total)).min(100) as u64
        };
        if self.percent != percent {
            self.percent = percent;
            if let Err(error) = self.store.write_file_operation(&self.record) {
                tracing::warn!(%error, "could not update file operation");
            }
        }
    }
    pub fn finish(&mut self, error: Option<String>, cancelled: bool, path: Option<String>) {
        self.record.phase = if cancelled {
            FileOperationPhase::Cancelled
        } else if error.is_some() {
            FileOperationPhase::Failed
        } else {
            FileOperationPhase::Completed
        };
        self.record.error = if cancelled { None } else { error };
        self.record.finished_at = Some(crate::now_ms());
        self.record.local_path = path;
        if self.record.phase == FileOperationPhase::Completed
            && self.record.kind != FileOperationKind::Import
        {
            self.record.written = self.record.total;
        }
        if let Err(error) = self.store.write_file_operation(&self.record) {
            tracing::warn!(%error, "could not finish file operation");
        }
    }
}
impl Drop for FileOperationGuard {
    fn drop(&mut self) {
        if self.record.phase == FileOperationPhase::Running {
            self.record.phase = FileOperationPhase::Interrupted;
            self.record.error = Some("The file operation was interrupted. Try again.".into());
            self.record.finished_at = Some(crate::now_ms());
            let _ = self.store.write_file_operation(&self.record);
        }
    }
}
