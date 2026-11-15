//! One explicit transfer per host; cancellation closes its authenticated session.
//! A multi-file clip downloads into one folder with combined progress, and a
//! remote member resumes from its last durable chunk after an interruption.
use crate::{
    file_partial::PartialFile,
    file_transfer::{self, Destination, FileTransfer, FileTransferError},
    Keyring, Store,
};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use tokio_util::sync::CancellationToken;

/// Transport failures retried in place before a download pauses.
const RECONNECT_DELAYS_MS: [u64; 3] = [1_000, 4_000, 15_000];

struct Member {
    index: usize,
    name: String,
    offer: FileTransfer,
}

fn members(row: &crate::StoredItem, keyring: &Keyring) -> Result<Vec<Member>, FileTransferError> {
    let payload = crate::ClipboardPayload::open(row, &keyring.item_key())?;
    let aggregate = file_transfer::offer(&payload)
        .cloned()
        .ok_or(FileTransferError::Unavailable)?;
    let descriptor = payload
        .file_descriptor()
        .ok_or(FileTransferError::Integrity)?;
    if descriptor.files.is_empty() {
        let name = match &payload {
            crate::ClipboardPayload::File {
                metadata: Some(metadata),
                ..
            } => metadata.filename.clone(),
            _ => "File".into(),
        };
        return Ok(vec![Member {
            index: 0,
            name,
            offer: aggregate,
        }]);
    }
    Ok(descriptor
        .files
        .iter()
        .enumerate()
        .map(|(index, file)| Member {
            index,
            name: file.filename.clone(),
            offer: file.offer(),
        })
        .collect())
}

/// A free name in `folder`, never one already chosen in this download.
fn free_path(folder: &Path, name: &str, taken: &HashSet<PathBuf>) -> PathBuf {
    let candidate = folder.join(name);
    let free = |path: &Path| {
        !taken.contains(path) && !path.exists() && !crate::file_partial::partial_path(path).exists()
    };
    if free(&candidate) {
        return candidate;
    }
    let stem = Path::new(name)
        .file_stem()
        .map_or_else(|| name.into(), |stem| stem.to_string_lossy().into_owned());
    let extension = Path::new(name)
        .extension()
        .map(|extension| format!(".{}", extension.to_string_lossy()));
    (2..)
        .map(|number| {
            folder.join(format!(
                "{stem} ({number}){}",
                extension.as_deref().unwrap_or("")
            ))
        })
        .find(|path| free(path))
        .expect("an unbounded sequence has a free name")
}

#[derive(Default)]
struct DownloadState {
    active: Option<(String, CancellationToken)>,
    cancelled_before_start: Option<String>,
}
pub struct FileDownloads(
    Mutex<DownloadState>,
    Arc<tokio::sync::Semaphore>,
    std::sync::OnceLock<Arc<dyn CloudFileSource>>,
);
impl Default for FileDownloads {
    fn default() -> Self {
        Self(
            Mutex::default(),
            Arc::new(tokio::sync::Semaphore::new(1)),
            std::sync::OnceLock::new(),
        )
    }
}

/// What a cloud provider delivers for one requested member, in order.
#[derive(Debug)]
pub enum CloudChunk {
    Bytes(Vec<u8>),
    /// The member's SHA-256 from its sealed manifest.
    Done(String),
    Failed(String),
}

/// Fetches files from cloud storage when their origin device is unreachable.
pub trait CloudFileSource: Send + Sync {
    /// Starts fetching member `index` of `id`; `None` when no provider can
    /// serve it now.
    fn fetch(
        &self,
        id: &str,
        index: usize,
        size: u64,
    ) -> Option<tokio::sync::mpsc::Receiver<CloudChunk>>;
    /// Withdraws a fetch that ended on this side.
    fn cancel(&self, id: &str);
}

/// Longest wait for the next part of a cloud fetch.
const CLOUD_IDLE: std::time::Duration = std::time::Duration::from_secs(60);
pub struct FileDownloadRequest<'a> {
    pub id: &'a str,
    pub destination: &'a Path,
    pub token: Option<&'a str>,
}
impl FileDownloads {
    /// Lets downloads fall back to cloud storage. Set once per host.
    pub fn set_cloud(&self, cloud: Arc<dyn CloudFileSource>) {
        let _ = self.2.set(cloud);
    }
    /// Waits for the transfer slot instead of failing while one is busy.
    pub async fn reserve_queued(
        &self,
    ) -> Result<tokio::sync::OwnedSemaphorePermit, FileTransferError> {
        Arc::clone(&self.1)
            .acquire_owned()
            .await
            .map_err(|_| FileTransferError::Cancelled)
    }
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
        let file_count = row
            .as_ref()
            .and_then(|row| members(row, keyring).ok())
            .map_or(1, |members| members.len());
        let name = metadata
            .as_ref()
            .map_or("File", |metadata| metadata.filename.as_str());
        let origin = row
            .as_ref()
            .map(|row| crate::origin_or(&row.origin_device_id, local_device).to_owned())
            .unwrap_or_else(|| "Unknown source".into());
        let source = store
            .device_names(std::slice::from_ref(&origin))
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
        {
            let mut operation = operation.lock().unwrap_or_else(|e| e.into_inner());
            operation.files(file_count);
            operation.destination(&request.destination.to_string_lossy());
        }
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
        tracing::info!("file download started: {file_count} files");
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
        match &result {
            Ok(()) => tracing::info!("file download finished"),
            Err(FileTransferError::Cancelled) => tracing::info!("file download cancelled"),
            // Waiting for the source; retried when it is reachable again.
            Err(error @ (FileTransferError::Unavailable | FileTransferError::Busy)) => {
                tracing::debug!("file download deferred: {}", error.kind());
            }
            Err(error) => tracing::warn!("file download failed: {}", error.kind()),
        }
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
        let members = members(&row, keyring)?;
        // An Android save folder is a document tree: files are completed in
        // a private staging folder, then exported into it.
        let tree = destination
            .to_str()
            .filter(|folder| folder.starts_with("content://"));
        let staging = match tree {
            Some(_) => Some(staging_folder(store, id)?),
            None => None,
        };
        let destination = staging.as_deref().unwrap_or(destination);
        // A save folder takes a single file under its own name.
        let folder =
            (members.len() > 1 || staging.is_some() || destination.is_dir()).then_some(destination);
        if folder.is_some_and(|folder| !folder.is_dir()) {
            return Err(FileTransferError::MissingDirectory);
        }
        let saved = self
            .save_members(
                store,
                keyring,
                node,
                local_device,
                settings,
                (id, &row, &offer, &members),
                (destination, folder),
                &token,
                &progress,
            )
            .await;
        if let (Some(tree), Some(staging)) = (tree, &staging) {
            let exported = saved.and_then(|()| export_members(store, id, &members, tree, staging));
            let _ = std::fs::remove_dir_all(staging);
            return exported;
        }
        saved
    }

    #[allow(clippy::too_many_arguments)]
    async fn save_members(
        &self,
        store: &Store,
        keyring: &Arc<Keyring>,
        node: &copypaste_p2p::Node,
        local_device: &str,
        settings: &copypaste_ipc::ConfigData,
        (id, row, offer, members): (&str, &crate::StoredItem, &FileTransfer, &[Member]),
        (destination, folder): (&Path, Option<&Path>),
        token: &CancellationToken,
        progress: &Arc<dyn Fn(u64, u64) + Send + Sync>,
    ) -> Result<(), FileTransferError> {
        let limit = settings.capture_limit_bytes(copypaste_ipc::content_type::FILE);
        let references = crate::sync::blocking(|| store.file_references(id))?;
        if !references.is_empty() {
            let mut taken = HashSet::new();
            let mut done = 0;
            for member in members {
                let target = match folder {
                    Some(folder) => free_path(folder, &member.name, &taken),
                    None => destination.to_owned(),
                };
                taken.insert(target.clone());
                let hash = copy_reference(
                    (store, keyring),
                    id,
                    member,
                    &target,
                    token,
                    done,
                    offer.size_bytes,
                    progress,
                )
                .await?;
                store.set_downloaded_member(id, member.index, &target.to_string_lossy(), &hash)?;
                done += member.offer.size_bytes;
            }
            return Ok(());
        }
        if store.has_file_transfer(id)? {
            let destination = match folder {
                Some(folder) => free_path(folder, &members[0].name, &HashSet::new()),
                None => destination.to_owned(),
            };
            let saved_store = store;
            let saved_id = id.to_owned();
            let saved_path = destination.to_string_lossy().into_owned();
            let store = store.clone();
            let keyring = Arc::clone(keyring);
            let id = id.to_owned();
            let token = token.clone();
            let progress = Arc::clone(progress);
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
        if !settings.sync_allows(&row.content_type, &[origin]) {
            return Err(FileTransferError::Remote(
                "File sync with this device is turned off in Devices.".into(),
            ));
        }
        let peer = node
            .peers()
            .list()
            .into_iter()
            .find(|p| p.device_id.as_deref() == Some(origin));
        crate::sync::blocking(|| crate::file_partial::sweep(store, keyring));
        let mut taken = HashSet::new();
        let mut done = 0;
        for member in members {
            let target = match folder {
                Some(folder) => {
                    if saved_in(store, id, member, folder)? {
                        done += member.offer.size_bytes;
                        progress(done, offer.size_bytes);
                        continue;
                    }
                    store
                        .download_partial(id, member.index)?
                        .map(|partial| PathBuf::from(partial.destination))
                        .filter(|path| path.parent() == Some(folder) && !path.exists())
                        .unwrap_or_else(|| free_path(folder, &member.name, &taken))
                }
                None => destination.to_owned(),
            };
            taken.insert(target.clone());
            let direct = match &peer {
                Some(peer) => {
                    RemoteMember {
                        store,
                        node,
                        peer,
                        id,
                        member,
                        token,
                        done,
                        total: offer.size_bytes,
                        progress,
                    }
                    .run(&target)
                    .await
                }
                None => Err(FileTransferError::SourceNotPaired),
            };
            let hash = match direct {
                // The origin is unreachable: a cloud copy may still exist.
                Err(
                    error @ (FileTransferError::SourceNotPaired
                    | FileTransferError::Remote(_)
                    | FileTransferError::Unavailable),
                ) => match self.2.get() {
                    Some(cloud) => CloudMember {
                        store,
                        cloud: cloud.as_ref(),
                        id,
                        member,
                        token,
                        done,
                        total: offer.size_bytes,
                        progress,
                    }
                    .run(&target)
                    .await
                    .map_err(|cloud_error| match cloud_error {
                        FileTransferError::Unavailable => error,
                        other => other,
                    })?,
                    None => return Err(error),
                },
                other => other?,
            };
            store.set_downloaded_member(id, member.index, &target.to_string_lossy(), &hash)?;
            done += member.offer.size_bytes;
        }
        Ok(())
    }
}

/// A member of a folder download already saved and unchanged in that folder.
fn saved_in(
    store: &Store,
    id: &str,
    member: &Member,
    folder: &Path,
) -> Result<bool, FileTransferError> {
    Ok(store
        .downloaded_members(id)?
        .into_iter()
        .find(|(index, _)| *index == member.index)
        .map(|(_, path)| PathBuf::from(path))
        .is_some_and(|path| {
            path.parent() == Some(folder)
                && std::fs::metadata(&path)
                    .is_ok_and(|metadata| metadata.len() == member.offer.size_bytes)
        }))
}

/// A private folder a document-tree save completes in.
fn staging_folder(store: &Store, id: &str) -> Result<PathBuf, FileTransferError> {
    let root = store
        .file_work_directory()
        .map(Path::to_path_buf)
        .unwrap_or_else(std::env::temp_dir)
        .join("folder-saves")
        .join(
            id.chars()
                .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
                .collect::<String>(),
        );
    std::fs::create_dir_all(&root).map_err(|_| FileTransferError::Write)?;
    Ok(root)
}

/// Moves saved members from staging into a document tree and records them there.
fn export_members(
    store: &Store,
    id: &str,
    members: &[Member],
    tree: &str,
    staging: &Path,
) -> Result<(), FileTransferError> {
    let saved = store.downloaded_members(id)?;
    for member in members {
        let Some((_, path)) = saved.iter().find(|(index, _)| *index == member.index) else {
            continue;
        };
        let path = PathBuf::from(path);
        if path.parent() != Some(staging) {
            continue;
        }
        let hash = store
            .downloaded_member_checksum(id, member.index)?
            .unwrap_or_default();
        let location =
            file_transfer::export_to_folder(tree, &member.name, "application/octet-stream", &path)?;
        store.set_downloaded_member(id, member.index, &location, &hash)?;
    }
    Ok(())
}

/// One member fetched from cloud storage into the same resumable partial.
struct CloudMember<'a> {
    store: &'a Store,
    cloud: &'a dyn CloudFileSource,
    id: &'a str,
    member: &'a Member,
    token: &'a CancellationToken,
    done: u64,
    total: u64,
    progress: &'a Arc<dyn Fn(u64, u64) + Send + Sync>,
}

impl CloudMember<'_> {
    async fn run(&self, target: &Path) -> Result<String, FileTransferError> {
        let offer = &self.member.offer;
        let mut chunks = self
            .cloud
            .fetch(self.id, self.member.index, offer.size_bytes)
            .ok_or(FileTransferError::Unavailable)?;
        let _release = scopeguard::guard((), |_| self.cloud.cancel(self.id));
        let version = crate::file_partial::member_version(offer);
        let mut partial = crate::sync::blocking(|| {
            PartialFile::open(self.store, self.id, self.member.index, &version, target)
        })?;
        // The cloud stream always starts at the first byte.
        crate::sync::blocking(|| partial.restart())?;
        loop {
            let next = tokio::select! {
                biased;
                _ = self.token.cancelled() => {
                    crate::sync::blocking(|| partial.discard());
                    return Err(FileTransferError::Cancelled);
                }
                next = tokio::time::timeout(CLOUD_IDLE, chunks.recv()) => next,
            };
            match next {
                Ok(Some(CloudChunk::Bytes(bytes))) => {
                    if partial.written() + bytes.len() as u64 > offer.size_bytes {
                        crate::sync::blocking(|| partial.discard());
                        return Err(FileTransferError::Integrity);
                    }
                    crate::sync::blocking(|| partial.write(&bytes))
                        .map_err(file_transfer::destination_error)?;
                    (self.progress)(self.done + partial.written(), self.total);
                }
                Ok(Some(CloudChunk::Done(expected))) => {
                    if partial.written() != offer.size_bytes
                        || partial.hash() != expected
                        || (!offer.sha256.is_empty() && offer.sha256 != expected)
                    {
                        crate::sync::blocking(|| partial.discard());
                        return Err(FileTransferError::Integrity);
                    }
                    crate::sync::blocking(|| partial.publish(target))?;
                    return Ok(expected);
                }
                Ok(Some(CloudChunk::Failed(reason))) => {
                    crate::sync::blocking(|| partial.discard());
                    return Err(FileTransferError::Remote(reason));
                }
                Ok(None) | Err(_) => {
                    crate::sync::blocking(|| partial.discard());
                    return Err(FileTransferError::Unavailable);
                }
            }
        }
    }
}

/// Copy one member of a clip captured on this device from its original, or
/// from its kept copy once the original is gone.
#[allow(clippy::too_many_arguments)]
async fn copy_reference(
    (store, keyring): (&Store, &Arc<Keyring>),
    id: &str,
    member: &Member,
    target: &Path,
    token: &CancellationToken,
    done: u64,
    total: u64,
    progress: &Arc<dyn Fn(u64, u64) + Send + Sync>,
) -> Result<String, FileTransferError> {
    let mut source = crate::sync::blocking(|| {
        crate::file_reference::source_or_copy(store, keyring, id, member.index)
    })?
    .ok_or(FileTransferError::Unavailable)?;
    let mut output = Destination::create(target)?;
    let mut digest = Sha256::new();
    let mut written = 0;
    for _ in 0..member
        .offer
        .size_bytes
        .div_ceil(file_transfer::CHUNK_BYTES as u64)
        .max(1)
    {
        if token.is_cancelled() {
            return Err(FileTransferError::Cancelled);
        }
        let bytes =
            crate::sync::blocking(|| source.read_chunk()).map_err(|_| FileTransferError::Read)?;
        digest.update(&bytes);
        crate::sync::blocking(|| output.write_all(&bytes))
            .map_err(file_transfer::destination_error)?;
        written += bytes.len() as u64;
        progress(done + written, total);
    }
    source.verify().map_err(|_| FileTransferError::Read)?;
    output.finish()?;
    Ok(hex::encode(digest.finalize()))
}

/// One remote member, written to a partial file that survives interruption.
struct RemoteMember<'a> {
    store: &'a Store,
    node: &'a copypaste_p2p::Node,
    peer: &'a copypaste_p2p::Peer,
    id: &'a str,
    member: &'a Member,
    token: &'a CancellationToken,
    /// Bytes of earlier members, for combined progress.
    done: u64,
    total: u64,
    progress: &'a Arc<dyn Fn(u64, u64) + Send + Sync>,
}

impl RemoteMember<'_> {
    async fn run(&self, target: &Path) -> Result<String, FileTransferError> {
        let offer = &self.member.offer;
        let version = crate::file_partial::member_version(offer);
        let mut partial = crate::sync::blocking(|| {
            PartialFile::open(self.store, self.id, self.member.index, &version, target)
        })?;
        let mut attempt = 0;
        let checksum = loop {
            let before = partial.written();
            let start_index = before / file_transfer::CHUNK_BYTES as u64;
            let mut write_error = None;
            let range = copypaste_p2p::FileRange {
                id: self.id,
                file: self.member.index as u32,
                sha256: &offer.sha256,
                size: offer.size_bytes,
                start_index,
            };
            let operation = self.node.download_file_range(self.peer, range, |bytes| {
                // The bounded block is written off the reactor on all runtime hosts.
                if let Err(error) = crate::sync::blocking(|| partial.write(bytes)) {
                    write_error = Some(error);
                    return Err(copypaste_p2p::sync::SyncError::Source(
                        "The destination could not be written.".into(),
                    ));
                }
                (self.progress)(self.done + partial.written(), self.total);
                Ok(())
            });
            let result = tokio::select! {
                biased;
                _ = self.token.cancelled() => None,
                result = operation => Some(result),
            };
            let Some(result) = result else {
                crate::sync::blocking(|| partial.discard());
                return Err(FileTransferError::Cancelled);
            };
            match result {
                Ok(checksum) => break checksum,
                Err(copypaste_p2p::sync::SyncError::Source(reason))
                    if reason == copypaste_p2p::FILE_RANGE_UNSUPPORTED && start_index > 0 =>
                {
                    crate::sync::blocking(|| partial.restart())?;
                }
                Err(
                    copypaste_p2p::sync::SyncError::Channel(_)
                    | copypaste_p2p::sync::SyncError::Protocol(_),
                ) if write_error.is_none() && attempt < RECONNECT_DELAYS_MS.len() => {
                    crate::sync::blocking(|| partial.checkpoint())?;
                    let delay = std::time::Duration::from_millis(RECONNECT_DELAYS_MS[attempt]);
                    attempt += 1;
                    tokio::select! {
                        biased;
                        _ = self.token.cancelled() => {
                            crate::sync::blocking(|| partial.discard());
                            return Err(FileTransferError::Cancelled);
                        }
                        _ = tokio::time::sleep(delay) => {}
                    }
                }
                Err(error) => {
                    // Keep a verified prefix when the source was unreachable
                    // or the disk failed; a source that refused or broke the
                    // stream after sending bytes starts over next time.
                    let refused = matches!(error, copypaste_p2p::sync::SyncError::Source(_))
                        && write_error.is_none()
                        && partial.written() > before;
                    if refused || partial.written() == 0 {
                        crate::sync::blocking(|| partial.discard());
                    } else {
                        let _ = crate::sync::blocking(|| partial.checkpoint());
                    }
                    return Err(write_error
                        .map(file_transfer::destination_error)
                        .unwrap_or_else(|| FileTransferError::Remote(error.to_string())));
                }
            }
        };
        let expected = if offer.sha256.is_empty() {
            checksum
        } else {
            offer.sha256.clone()
        };
        if partial.written() != offer.size_bytes || partial.hash() != expected {
            crate::sync::blocking(|| partial.discard());
            return Err(FileTransferError::Integrity);
        }
        if self.token.is_cancelled() {
            crate::sync::blocking(|| partial.discard());
            return Err(FileTransferError::Cancelled);
        }
        crate::sync::blocking(|| partial.publish(target))?;
        Ok(expected)
    }
}

/// Clips with interrupted downloads and the destination each was saving to:
/// the file, or the folder of a multi-file clip.
pub fn pending(
    store: &Store,
    keyring: &Keyring,
) -> Result<Vec<(String, PathBuf)>, FileTransferError> {
    let mut pending: Vec<(String, PathBuf)> = Vec::new();
    for partial in store.download_partials()? {
        if pending.iter().any(|(id, _)| *id == partial.item_id) {
            continue;
        }
        let Some(row) = store.get(&partial.item_id)?.filter(|row| !row.deleted) else {
            continue;
        };
        let destination = PathBuf::from(&partial.destination);
        let destination = if members(&row, keyring)?.len() > 1 {
            destination
                .parent()
                .map_or(destination.clone(), Path::to_path_buf)
        } else {
            destination
        };
        pending.push((partial.item_id, destination));
    }
    Ok(pending)
}
