use std::path::Path;

use copypaste_ipc::BackupData;

use super::{BackendError, EmbeddedBackend, Result};

const MSG_BACKUP_FAILED: &str = "The encrypted backup couldn't be created.";
const MSG_RESTORE_FAILED: &str =
    "The restore could not be completed; this device's history is unchanged.";

pub(super) async fn backup(backend: &EmbeddedBackend, dest: &Path) -> Result<BackupData> {
    let dest = dest.to_owned();
    backend
        .blocking(move |inner| {
            inner
                .state
                .store
                .backup_to(&dest)
                .map_err(|_| BackendError::internal(MSG_BACKUP_FAILED))?;
            let size_bytes = std::fs::metadata(dest).map(|m| m.len()).unwrap_or(0);
            Ok(BackupData { size_bytes })
        })
        .await
}

pub(super) async fn restore(backend: &EmbeddedBackend, src: &Path) -> Result<()> {
    let src = src.to_owned();
    backend
        .blocking(move |inner| {
            inner
                .state
                .store
                .restore_from(&src, &inner.state.keyring.db_key(), &inner.state.detector)
                .map_err(|_| BackendError::internal(MSG_RESTORE_FAILED))?;
            inner.note_oldest_version(inner.state.store.oldest_version_ms().ok().flatten());
            inner.publish_items(false, 0);
            Ok(())
        })
        .await
}

#[cfg(test)]
mod tests {
    use super::super::tests::backend;
    use crate::backend::Backend;

    #[tokio::test]
    async fn backup_and_restore_round_trip_in_process() {
        let (backend, _clip, dir) = backend();
        backend.add("survives restart").await.unwrap();
        let device_id = backend.inner.state.device_id.clone();
        let backup = dir.path().join("history.cpbak");
        assert!(backend.backup(&backup).await.unwrap().size_bytes > 0);
        backend.set_device_name("Current phone").await.unwrap();
        backend.clear().await.unwrap();
        backend.restore(&backup).await.unwrap();
        assert_eq!(backend.list(10, None).await.unwrap().items.len(), 1);
        assert_eq!(backend.status().await.unwrap().device_name, "Current phone");
        let names = backend
            .inner
            .state
            .store
            .device_names(std::slice::from_ref(&device_id))
            .unwrap();
        assert_eq!(names[&device_id], "Current phone");
    }

    #[tokio::test]
    async fn invalid_restore_preserves_live_history_and_hides_paths() {
        let (backend, _clip, dir) = backend();
        backend.add("keep this").await.unwrap();
        let bad = dir.path().join("broken.cpbak");
        std::fs::write(&bad, b"bad").unwrap();
        let shown = backend.restore(&bad).await.unwrap_err().to_string();
        assert!(!shown.contains('/'));
        assert_eq!(backend.list(10, None).await.unwrap().items.len(), 1);
    }
}
