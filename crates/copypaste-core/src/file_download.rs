//! One explicit transfer per host; cancellation closes its authenticated session.
use crate::{
    file_transfer::{self, Destination, FileTransferError},
    Keyring, Store,
};
use sha2::{Digest, Sha256};
use std::{
    path::Path,
    sync::{Arc, Mutex},
};
use tokio_util::sync::CancellationToken;

#[derive(Default)]
struct DownloadState {
    active: Option<(String, CancellationToken)>,
    cancelled_before_start: Option<String>,
}
pub struct FileDownloads(Mutex<DownloadState>, Arc<tokio::sync::Semaphore>);
impl Default for FileDownloads {
    fn default() -> Self {
        Self(Mutex::default(), Arc::new(tokio::sync::Semaphore::new(1)))
    }
}
pub struct FileDownloadRequest<'a> {
    pub id: &'a str,
    pub destination: &'a Path,
    pub token: Option<&'a str>,
}
impl FileDownloads {
    /// Admission belongs to the worker, so detached requests cannot queue tasks.
    pub fn reserve_background(
        &self,
    ) -> Result<tokio::sync::OwnedSemaphorePermit, FileTransferError> {
        Arc::clone(&self.1).try_acquire_owned().map_err(|_| {
            if self.1.is_closed() {
                FileTransferError::Cancelled
            } else {
                FileTransferError::Busy
            }
        })
    }
    pub fn cancel(&self, id: &str) {
        if id.len() > 128 {
            return;
        }
        let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((current, token)) = state.active.as_ref() {
            if id == current {
                token.cancel();
                return;
            }
        }
        state.cancelled_before_start = Some(id.into());
    }
    pub fn cancel_all(&self) {
        self.1.close();
        if let Some((_, token)) = self
            .0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .active
            .as_ref()
        {
            token.cancel();
        }
    }
    #[cfg(test)]
    pub(crate) async fn save(
        &self,
        store: &Store,
        keyring: &Arc<Keyring>,
        node: &copypaste_p2p::Node,
        local_device: &str,
        id: &str,
        destination: &Path,
        settings: &copypaste_ipc::ConfigData,
    ) -> Result<(), FileTransferError> {
        self.save_with_token(
            store,
            keyring,
            node,
            local_device,
            settings,
            FileDownloadRequest {
                id,
                destination,
                token: None,
            },
        )
        .await
    }
    pub async fn save_with_token(
        &self,
        store: &Store,
        keyring: &Arc<Keyring>,
        node: &copypaste_p2p::Node,
        local_device: &str,
        settings: &copypaste_ipc::ConfigData,
        request: FileDownloadRequest<'_>,
    ) -> Result<(), FileTransferError> {
        self.save_with_progress(
            store,
            keyring,
            node,
            local_device,
            settings,
            request,
            Arc::new(|_, _| {}),
        )
        .await
    }

    /// Progress is advisory: only a successful return confirms a committed file.
    pub async fn save_with_progress(
        &self,
        store: &Store,
        keyring: &Arc<Keyring>,
        node: &copypaste_p2p::Node,
        local_device: &str,
        settings: &copypaste_ipc::ConfigData,
        request: FileDownloadRequest<'_>,
        progress: Arc<dyn Fn(u64, u64) + Send + Sync>,
    ) -> Result<(), FileTransferError> {
        if request.id.len() > 128
            || request.destination.as_os_str().len() > 4096
            || request.token.is_some_and(|token| token.len() > 128)
        {
            return Err(FileTransferError::Integrity);
        }
        let row = store.get(request.id)?.filter(|row| !row.deleted);
        let metadata = row
            .as_ref()
            .and_then(|row| row.payload_metadata.as_deref())
            .and_then(|value| crate::PayloadMetadata::from_json(value, "file"))
            .and_then(|value| value.file);
        let total = metadata
            .as_ref()
            .and_then(|metadata| metadata.transfer.as_ref())
            .map_or(0, |offer| offer.size_bytes);
        let name = metadata
            .as_ref()
            .map_or("File", |metadata| metadata.filename.as_str());
        let origin = row
            .as_ref()
            .map(|row| crate::origin_or(&row.origin_device_id, local_device).to_owned())
            .unwrap_or_else(|| "Unknown source".into());
        let source = store
            .device_names(&[origin.clone()])
            .ok()
            .and_then(|names| names.get(&origin).cloned())
            .unwrap_or(origin);
        let destination = store
            .current_device_name()
            .unwrap_or_else(|_| "This device".into());
        let operation = Arc::new(Mutex::new(
            crate::file_operation::FileOperationGuard::start(
                store,
                request.id,
                name,
                copypaste_ipc::FileOperationKind::Receive,
                source,
                destination,
                total,
                request.token,
            ),
        ));
        let tracked = Arc::clone(&operation);
        let progress = Arc::new(move |written, total| {
            crate::sync::blocking(|| {
                tracked
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .progress(written, total)
            });
            progress(written, total);
        });
        let path = request.destination.to_string_lossy().into_owned();
        let result = self
            .save_inner(
                store,
                keyring,
                node,
                local_device,
                settings,
                request,
                progress,
            )
            .await;
        crate::sync::blocking(|| {
            operation.lock().unwrap_or_else(|e| e.into_inner()).finish(
                result.as_ref().err().map(ToString::to_string),
                matches!(&result, Err(FileTransferError::Cancelled)),
                Some(path),
            )
        });
        result
    }

    async fn save_inner(
        &self,
        store: &Store,
        keyring: &Arc<Keyring>,
        node: &copypaste_p2p::Node,
        local_device: &str,
        settings: &copypaste_ipc::ConfigData,
        request: FileDownloadRequest<'_>,
        progress: Arc<dyn Fn(u64, u64) + Send + Sync>,
    ) -> Result<(), FileTransferError> {
        if self.1.is_closed() {
            return Err(FileTransferError::Cancelled);
        }
        let FileDownloadRequest {
            id,
            destination,
            token: download_token,
        } = request;
        let identity = download_token.unwrap_or(id);
        if identity.len() > 128 {
            return Err(FileTransferError::Integrity);
        }
        let token = CancellationToken::new();
        {
            let mut current = self.0.lock().unwrap_or_else(|e| e.into_inner());
            if current.active.is_some() {
                return Err(FileTransferError::Busy);
            }
            if current.cancelled_before_start.take().as_deref() == Some(identity) {
                return Err(FileTransferError::Cancelled);
            }
            current.active = Some((identity.into(), token.clone()));
        }
        let cancellation_on_drop = token.clone();
        let _release = scopeguard::guard((), |_| {
            cancellation_on_drop.cancel();
            self.0
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .active
                .take();
        });
        let row = store
            .get(id)?
            .filter(|r| !r.deleted)
            .ok_or(FileTransferError::NotFound)?;
        let offer = file_transfer::row_offer(&row, &keyring.item_key())?
            .ok_or(FileTransferError::Unavailable)?;
        let limit = settings.capture_limit_bytes(copypaste_ipc::content_type::FILE);
        file_transfer::check_limit(&offer, limit)?;
        if let Some(mut source) =
            crate::sync::blocking(|| crate::file_reference::source(store, id))?
        {
            let mut output = Destination::create(destination)?;
            let mut digest = Sha256::new();
            let mut written = 0;
            for _ in 0..offer
                .size_bytes
                .div_ceil(file_transfer::CHUNK_BYTES as u64)
                .max(1)
            {
                if token.is_cancelled() {
                    return Err(FileTransferError::Cancelled);
                }
                let bytes = crate::sync::blocking(|| source.read_chunk())
                    .map_err(|_| FileTransferError::Read)?;
                digest.update(&bytes);
                crate::sync::blocking(|| output.write_all(&bytes))
                    .map_err(file_transfer::destination_error)?;
                written += bytes.len() as u64;
                progress(written, offer.size_bytes);
            }
            source.verify().map_err(|_| FileTransferError::Read)?;
            output.finish()?;
            store.set_downloaded_file_checksum(
                id,
                &destination.to_string_lossy(),
                &hex::encode(digest.finalize()),
            )?;
            return Ok(());
        }
        if store.has_file_transfer(id)? {
            let saved_store = store;
            let saved_id = id.to_owned();
            let saved_path = destination.to_string_lossy().into_owned();
            let store = store.clone();
            let keyring = Arc::clone(keyring);
            let id = id.to_owned();
            let destination = destination.to_owned();
            let saved = tokio::task::spawn_blocking(move || {
                file_transfer::save_local_with_progress(
                    &store,
                    &keyring.item_key(),
                    &id,
                    &destination,
                    limit,
                    || token.is_cancelled(),
                    |written, total| progress(written, total),
                )
            })
            .await
            .map_err(|_| FileTransferError::Write)?;
            saved?;
            saved_store.set_downloaded_file_checksum(&saved_id, &saved_path, &offer.sha256)?;
            return Ok(());
        }
        if !settings.sync_enabled {
            return Err(FileTransferError::Remote(
                "Enable synchronization in Settings before downloading a file.".into(),
            ));
        }
        let origin = crate::origin_or(&row.origin_device_id, local_device);
        let peer = node
            .peers()
            .list()
            .into_iter()
            .find(|p| p.device_id.as_deref() == Some(origin))
            .ok_or(FileTransferError::SourceNotPaired)?;
        let mut output = Destination::create(destination)?;
        let mut hash = Sha256::new();
        let mut written = 0u64;
        let mut write_error = None;
        let operation = node.download_file(&peer, id, &offer.sha256, offer.size_bytes, |bytes| {
            // The bounded block is written off the reactor on all runtime hosts.
            let result = crate::sync::blocking(|| output.write_all(bytes));
            if let Err(error) = result {
                write_error = Some(file_transfer::destination_error(error));
                return Err(copypaste_p2p::sync::SyncError::Source(
                    "The destination could not be written.".into(),
                ));
            }
            hash.update(bytes);
            written += bytes.len() as u64;
            progress(written, offer.size_bytes);
            Ok(())
        });
        let expected_hash = tokio::select! {
            biased;
            _ = token.cancelled() => return Err(FileTransferError::Cancelled),
            result = operation => {
                match result {
                    Ok(checksum) => checksum,
                    Err(error) => return Err(write_error.unwrap_or_else(|| FileTransferError::Remote(error.to_string()))),
                }
            }
        };
        let actual_hash = hex::encode(hash.finalize());
        if written != offer.size_bytes || actual_hash != expected_hash {
            return Err(FileTransferError::Integrity);
        }
        if token.is_cancelled() {
            return Err(FileTransferError::Cancelled);
        }
        output.finish()?;
        // The explicit destination is the user's local copy. It is not added
        // to a second hidden byte cache or advertised as a new content version.
        store.set_downloaded_file_checksum(id, &destination.to_string_lossy(), &actual_hash)?;
        Ok(())
    }
}
