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
#[derive(Default)]
pub struct FileDownloads(Mutex<DownloadState>);
pub struct FileDownloadRequest<'a> {
    pub id: &'a str,
    pub destination: &'a Path,
    pub token: Option<&'a str>,
}
impl FileDownloads {
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
        if store.has_file_transfer(id)? {
            let store = store.clone();
            let keyring = Arc::clone(keyring);
            let id = id.to_owned();
            let destination = destination.to_owned();
            return tokio::task::spawn_blocking(move || {
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
        tokio::select! {
            biased;
            _ = token.cancelled() => return Err(FileTransferError::Cancelled),
            result = operation => {
                if let Err(error) = result {
                    return Err(write_error.unwrap_or_else(|| FileTransferError::Remote(error.to_string())));
                }
            }
        }
        if written != offer.size_bytes || hex::encode(hash.finalize()) != offer.sha256 {
            return Err(FileTransferError::Integrity);
        }
        if token.is_cancelled() {
            return Err(FileTransferError::Cancelled);
        }
        output.finish()?;
        // The explicit destination is the user's local copy. It is not added
        // to a second hidden byte cache or advertised as a new content version.
        store.set_downloaded_file(id, &destination.to_string_lossy())?;
        Ok(())
    }
}
