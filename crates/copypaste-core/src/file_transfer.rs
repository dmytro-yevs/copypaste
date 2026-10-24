//! File offers and explicit, bounded access to encrypted local file bodies.
use std::io::{Read, Write};
use std::path::Path;

use crate::{
    ClipboardPayload, FileMetadata, ItemKey, Keyring, NewItem, PayloadMetadata, Store, StoredItem,
};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

pub use copypaste_payload::FileTransfer;
pub const CHUNK_BYTES: usize = 64 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum FileTransferError {
    #[error("{0}")]
    Remote(String),
    #[error("The selected file could not be read or changed during import.")]
    Read,
    #[error("The selected file no longer exists.")]
    SourceMissing,
    #[error("The original file changed after it was added. Add it again before downloading.")]
    SourceChanged,
    #[error("Permission to access this file was denied.")]
    PermissionDenied,
    #[error("A file already exists at the selected destination. Choose another name.")]
    DestinationExists,
    #[error("The destination folder no longer exists.")]
    MissingDirectory,
    #[error("There is not enough free disk space to save this file.")]
    NoSpace,
    #[error("The selected file's name or metadata is invalid.")]
    InvalidMetadata,
    #[error("Only regular files can be added to History.")]
    NotFile,
    #[error("The file is {size} bytes; the file size limit in Settings is {limit} bytes.")]
    TooLarge { size: u64, limit: u64 },
    #[error("The source device is unavailable. Bring it online and try Download again.")]
    Unavailable,
    #[error("Pair with the source device before downloading this file.")]
    SourceNotPaired,
    #[error("Download this file before copying it.")]
    DownloadFirst,
    #[error("The downloaded file did not match its authenticated size or SHA-256.")]
    Integrity,
    #[error("The download was cancelled.")]
    Cancelled,
    #[error("Another file transfer is in progress. Wait for it to finish.")]
    Busy,
    #[error("The file could not be saved. Check destination access and available disk space.")]
    Write,
    #[error("The file is no longer in History.")]
    NotFound,
    #[error("The encrypted file store is unavailable.")]
    Storage(#[source] crate::StoreError),
}

impl From<crate::StoreError> for FileTransferError {
    fn from(error: crate::StoreError) -> Self {
        Self::Storage(error)
    }
}
impl From<crate::CryptoError> for FileTransferError {
    fn from(_: crate::CryptoError) -> Self {
        Self::Integrity
    }
}

pub(crate) fn destination_error(error: std::io::Error) -> FileTransferError {
    match error.kind() {
        std::io::ErrorKind::PermissionDenied => FileTransferError::PermissionDenied,
        std::io::ErrorKind::AlreadyExists => FileTransferError::DestinationExists,
        std::io::ErrorKind::NotFound => FileTransferError::MissingDirectory,
        std::io::ErrorKind::StorageFull => FileTransferError::NoSpace,
        _ => FileTransferError::Write,
    }
}
fn source_error(error: std::io::Error) -> FileTransferError {
    match error.kind() {
        std::io::ErrorKind::PermissionDenied => FileTransferError::PermissionDenied,
        std::io::ErrorKind::NotFound => FileTransferError::SourceMissing,
        _ => FileTransferError::Read,
    }
}

pub fn offer(payload: &ClipboardPayload) -> Option<&FileTransfer> {
    match payload {
        ClipboardPayload::File {
            metadata: Some(metadata),
            ..
        } => metadata.transfer.as_ref(),
        _ => None,
    }
}

pub fn row_offer(
    row: &StoredItem,
    key: &ItemKey,
) -> Result<Option<FileTransfer>, FileTransferError> {
    let payload = ClipboardPayload::open(row, key)?;
    Ok(offer(&payload).cloned())
}

pub fn check_limit(offer: &FileTransfer, limit: u64) -> Result<(), FileTransferError> {
    if offer.size_bytes > limit {
        Err(FileTransferError::TooLarge {
            size: offer.size_bytes,
            limit,
        })
    } else {
        Ok(())
    }
}

/// Upgrade retained inline file rows before either transport starts. Item IDs,
/// origins, names and pins survive; the new representation is a new version so
/// existing peer/cloud cursors discover the offer. Legacy bodies are <=4 MiB.
pub fn upgrade_legacy(store: &Store, key: &ItemKey) -> Result<u64, FileTransferError> {
    store.clear_file_stages()?;
    let mut after = String::new();
    let mut count = 0;
    while let Some(row) = store.legacy_file_after(&after)? {
        after = row.id.clone();
        let Ok(payload) = ClipboardPayload::open(&row, key) else {
            tracing::warn!("An unreadable legacy file was left intact during offer migration.");
            continue;
        };
        let ClipboardPayload::File {
            bytes,
            metadata: Some(mut metadata),
        } = payload
        else {
            continue;
        };
        let transfer = FileTransfer {
            source_version: None,
            size_bytes: bytes.len() as u64,
            sha256: crate::binary::content_hash(&crate::binary::content_digest(&bytes)),
        };
        metadata.transfer = Some(transfer.clone());
        let mut context = row
            .payload_metadata
            .as_deref()
            .and_then(|json| PayloadMetadata::from_json(json, &row.content_type))
            .unwrap_or_default();
        context.file = Some(metadata);
        let descriptor = serde_json::to_vec(&transfer).map_err(|_| FileTransferError::Integrity)?;
        let item = NewItem {
            content_ciphertext: crate::binary::seal(&descriptor, key, &row.id)?,
            id: row.id,
            nonce: Vec::new(),
            content_type: row.content_type,
            content_hash: crate::binary::content_hash(&crate::binary::content_digest(&descriptor)),
            created_at: crate::now_ms().max(row.created_at.saturating_add(1)),
            search_text: None,
            app_bundle_id: row.app_bundle_id,
            app_name: row.app_name,
            payload_metadata: context.to_json(copypaste_ipc::content_type::FILE),
        };
        store.insert_file_transfer(
            item,
            &mut std::io::Cursor::new(&*bytes),
            &transfer,
            key,
            true,
            true,
        )?;
        count += 1;
    }
    Ok(count)
}

/// Hashes a descriptor in fixed-size blocks; no file-sized plaintext allocation.
pub fn digest(reader: &mut impl Read, limit: u64) -> Result<FileTransfer, FileTransferError> {
    let mut buffer = Zeroizing::new(vec![0; CHUNK_BYTES]);
    let mut hash = Sha256::new();
    let mut size = 0u64;
    loop {
        let n = reader
            .read(&mut buffer)
            .map_err(|_| FileTransferError::Read)?;
        if n == 0 {
            break;
        }
        size = size
            .checked_add(n as u64)
            .ok_or(FileTransferError::Integrity)?;
        if size > limit {
            return Err(FileTransferError::TooLarge { size, limit });
        }
        hash.update(&buffer[..n]);
    }
    Ok(FileTransfer {
        source_version: None,
        size_bytes: size,
        sha256: hex::encode(hash.finalize()),
    })
}

#[allow(clippy::too_many_arguments)]
pub fn import(
    store: &Store,
    keyring: &Keyring,
    path: &Path,
    metadata: FileMetadata,
    created_at: i64,
    app_bundle_id: Option<&str>,
    app_name: Option<&str>,
    context: Option<&PayloadMetadata>,
    settings: &copypaste_ipc::ConfigData,
    explicit: bool,
) -> Result<StoredItem, FileTransferError> {
    use std::io::{Seek, SeekFrom};
    #[cfg(unix)]
    let mut file = {
        use rustix::fs::{open, Mode, OFlags};
        std::fs::File::from(
            open(
                path,
                OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NONBLOCK | OFlags::NOCTTY,
                Mode::empty(),
            )
            .map_err(|error| source_error(error.into()))?,
        )
    };
    #[cfg(not(unix))]
    let mut file = std::fs::File::open(path).map_err(|error| {
        // Windows rejects opening a directory before handle metadata can
        // identify it. Preserve other source errors, including denied files.
        #[cfg(windows)]
        if error.kind() == std::io::ErrorKind::PermissionDenied
            && std::fs::metadata(path).is_ok_and(|metadata| metadata.is_dir())
        {
            return FileTransferError::NotFile;
        }
        source_error(error)
    })?;
    let before = file.metadata().map_err(|_| FileTransferError::Read)?;
    if !before.is_file() {
        return Err(FileTransferError::NotFile);
    }
    let limit = settings.capture_limit_bytes(copypaste_ipc::content_type::FILE);
    if before.len() > limit {
        return Err(FileTransferError::TooLarge {
            size: before.len(),
            limit,
        });
    }
    let transfer = digest(&mut file, limit)?;
    file.seek(SeekFrom::Start(0))
        .map_err(|_| FileTransferError::Read)?;
    let item = offer_item(
        &transfer,
        &keyring.item_key(),
        metadata,
        created_at,
        app_bundle_id,
        app_name,
        context,
        explicit,
    )?;
    let row = store.insert_file_transfer(
        item,
        &mut file,
        &transfer,
        &keyring.item_key(),
        explicit,
        false,
    )?;
    crate::ingest::enforce_retention(store, settings);
    Ok(row)
}

pub fn read_chunk(
    store: &Store,
    key: &ItemKey,
    id: &str,
    hash: &str,
    index: u64,
) -> Result<Zeroizing<Vec<u8>>, FileTransferError> {
    let row = store
        .get(id)?
        .filter(|row| !row.deleted)
        .ok_or(FileTransferError::NotFound)?;
    let descriptor = row_offer(&row, key)?.ok_or(FileTransferError::Unavailable)?;
    if descriptor.sha256 != hash
        || (descriptor.size_bytes == 0 && index != 0)
        || (descriptor.size_bytes > 0
            && index.saturating_mul(CHUNK_BYTES as u64) >= descriptor.size_bytes)
    {
        return Err(FileTransferError::Integrity);
    }
    if descriptor.size_bytes == 0 && store.has_file_transfer(id)? {
        return Ok(Zeroizing::new(Vec::new()));
    }
    let chunk = store
        .file_transfer_chunk(id, hash, index)?
        .ok_or(FileTransferError::Unavailable)?;
    let bytes = crate::decrypt(&chunk.ciphertext, &chunk.nonce, key, &chunk_id(hash, index))?;
    let expected = (descriptor.size_bytes - index * CHUNK_BYTES as u64).min(CHUNK_BYTES as u64);
    if bytes.len() as u64 != expected {
        return Err(FileTransferError::Integrity);
    }
    Ok(bytes)
}

pub(crate) fn chunk_id(hash: &str, index: u64) -> String {
    format!("copypaste/file-chunk/{hash}/{index}")
}

pub struct PreparedFileCopy {
    pub payload: ClipboardPayload,
    _directory: Option<tempfile::TempDir>,
}

/// Publish existing local paths directly. Only legacy encrypted snapshots need
/// scoped plaintext for a native writer; downloaded files remain user-owned.
pub fn prepare_copy(
    store: &Store,
    key: &ItemKey,
    row: &StoredItem,
    limit: u64,
) -> Result<Option<PreparedFileCopy>, FileTransferError> {
    let mut payload = ClipboardPayload::open(row, key)?;
    let Some(transfer) = offer(&payload).cloned() else {
        return Ok(None);
    };
    check_limit(&transfer, limit)?;
    let (path, directory, direct) =
        if let Some((reference, _, _)) = store.file_reference(&row.id)? {
            (reference, None, true)
        } else if store.has_file_transfer(&row.id)? {
            let mut builder = tempfile::Builder::new();
            builder.prefix("file-copy-");
            let directory = match store.file_work_directory() {
                Some(root) => builder.tempdir_in(root),
                None => builder.tempdir(),
            }
            .map_err(|_| FileTransferError::Write)?;
            let path = directory.path().join("payload");
            save_local(store, key, &row.id, &path, limit, || false)?;
            (path.to_string_lossy().into_owned(), Some(directory), false)
        } else {
            let downloaded = store
                .downloaded_file(&row.id)?
                .ok_or(FileTransferError::DownloadFirst)?;
            let mut source =
                std::fs::File::open(&downloaded).map_err(|_| FileTransferError::DownloadFirst)?;
            let mut buffer = Zeroizing::new(vec![0; CHUNK_BYTES]);
            let mut hash = Sha256::new();
            let mut total = 0u64;
            loop {
                let n = source
                    .read(&mut buffer)
                    .map_err(|_| FileTransferError::Read)?;
                if n == 0 {
                    break;
                }
                total += n as u64;
                if total > transfer.size_bytes {
                    return Err(FileTransferError::Integrity);
                }
                hash.update(&buffer[..n]);
            }
            let expected = if transfer.sha256.is_empty() {
                store
                    .downloaded_file_checksum(&row.id)?
                    .ok_or(FileTransferError::Integrity)?
            } else {
                transfer.sha256
            };
            if total != transfer.size_bytes || hex::encode(hash.finalize()) != expected {
                return Err(FileTransferError::Integrity);
            }
            (downloaded, None, true)
        };
    let ClipboardPayload::File {
        metadata: Some(metadata),
        bytes,
    } = &mut payload
    else {
        return Err(FileTransferError::Integrity);
    };
    metadata.source_reference = Some(path);
    if direct {
        metadata.transfer = None;
    }
    bytes.clear();
    Ok(Some(PreparedFileCopy {
        payload,
        _directory: directory,
    }))
}

/// Never publishes a partial destination as a successful save.
pub fn save_local(
    store: &Store,
    key: &ItemKey,
    id: &str,
    destination: &Path,
    limit: u64,
    cancelled: impl Fn() -> bool,
) -> Result<(), FileTransferError> {
    save_local_with_progress(store, key, id, destination, limit, cancelled, |_, _| {})
}

/// Reports bytes only after a successful write. Completion is confirmed by the
/// return value because validation or cancellation can still roll the file back.
pub fn save_local_with_progress(
    store: &Store,
    key: &ItemKey,
    id: &str,
    destination: &Path,
    limit: u64,
    cancelled: impl Fn() -> bool,
    mut progress: impl FnMut(u64, u64),
) -> Result<(), FileTransferError> {
    let row = store
        .get(id)?
        .filter(|row| !row.deleted)
        .ok_or(FileTransferError::NotFound)?;
    let descriptor = row_offer(&row, key)?.ok_or(FileTransferError::Unavailable)?;
    check_limit(&descriptor, limit)?;
    let mut output = Destination::create(destination)?;
    let mut hash = Sha256::new();
    let mut written = 0u64;
    for index in 0..descriptor.size_bytes.div_ceil(CHUNK_BYTES as u64) {
        if cancelled() {
            return Err(FileTransferError::Cancelled);
        }
        let bytes = read_chunk(store, key, id, &descriptor.sha256, index)?;
        hash.update(&bytes);
        output.write_all(&bytes).map_err(destination_error)?;
        written += bytes.len() as u64;
        progress(written, descriptor.size_bytes);
    }
    if hex::encode(hash.finalize()) != descriptor.sha256 {
        return Err(FileTransferError::Integrity);
    }
    if cancelled() {
        return Err(FileTransferError::Cancelled);
    }
    output.finish()
}

pub struct Destination {
    file: Option<std::fs::File>,
    path: std::path::PathBuf,
    complete: bool,
}
impl Destination {
    pub fn create(path: &Path) -> Result<Self, FileTransferError> {
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = options.open(path).map_err(destination_error)?;
        Ok(Self {
            file: Some(file),
            path: path.to_owned(),
            complete: false,
        })
    }
    pub fn write_all(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        self.file
            .as_mut()
            .expect("open destination")
            .write_all(bytes)
    }
    pub fn finish(mut self) -> Result<(), FileTransferError> {
        self.file
            .as_mut()
            .expect("open destination")
            .flush()
            .map_err(destination_error)?;
        self.file.take();
        self.complete = true;
        Ok(())
    }
}
impl Drop for Destination {
    fn drop(&mut self) {
        self.file.take();
        if !self.complete {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn offer_item(
    transfer: &FileTransfer,
    key: &ItemKey,
    mut metadata: FileMetadata,
    created_at: i64,
    app_bundle_id: Option<&str>,
    app_name: Option<&str>,
    context: Option<&PayloadMetadata>,
    explicit: bool,
) -> Result<NewItem, FileTransferError> {
    metadata.transfer = Some(transfer.clone());
    let mut context = context.cloned().unwrap_or_default();
    context.file = Some(metadata);
    let payload_metadata = context
        .to_json(copypaste_ipc::content_type::FILE)
        .ok_or(FileTransferError::InvalidMetadata)?;
    let descriptor = serde_json::to_vec(&transfer).map_err(|_| FileTransferError::Integrity)?;
    let content_hash = crate::binary::content_hash(&crate::binary::content_digest(&descriptor));
    let id = if explicit {
        format!("file-import-{}", uuid::Uuid::new_v4())
    } else {
        format!("file-offer-{}", &content_hash[..32])
    };
    Ok(NewItem {
        content_ciphertext: crate::binary::seal(&descriptor, key, &id)?,
        id,
        nonce: Vec::new(),
        content_type: copypaste_ipc::content_type::FILE.into(),
        content_hash,
        search_text: None,
        created_at,
        app_bundle_id: app_bundle_id.map(str::to_owned),
        app_name: app_name.map(str::to_owned),
        payload_metadata: Some(payload_metadata),
    })
}

#[cfg(test)]
mod progress_tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn local_progress_tracks_written_bytes_and_cancellation_rolls_back() {
        let dir = tempfile::tempdir().unwrap();
        let keyring = Arc::new(crate::Keyring::from_secret(&[43; 32]));
        let store = Store::open_in_memory(&keyring.db_key()).unwrap();
        let source = dir.path().join("source.bin");
        let bytes = vec![7; CHUNK_BYTES + 7];
        std::fs::write(&source, &bytes).unwrap();
        let row = crate::file_import::import_file(
            &store,
            &keyring,
            &source,
            "source.bin",
            "application/octet-stream",
            source.to_str(),
            &copypaste_ipc::ConfigData::default(),
        )
        .unwrap();
        let destination = dir.path().join("saved.bin");
        let mut reports = Vec::new();
        save_local_with_progress(
            &store,
            &keyring.item_key(),
            &row.id,
            &destination,
            u64::MAX,
            || false,
            |written, total| reports.push((written, total)),
        )
        .unwrap();
        assert_eq!(std::fs::read(&destination).unwrap(), bytes);
        assert_eq!(
            reports,
            vec![
                (CHUNK_BYTES as u64, bytes.len() as u64),
                (bytes.len() as u64, bytes.len() as u64)
            ]
        );

        let cancelled_destination = dir.path().join("cancelled.bin");
        let cancelled = std::cell::Cell::new(false);
        let result = save_local_with_progress(
            &store,
            &keyring.item_key(),
            &row.id,
            &cancelled_destination,
            u64::MAX,
            || cancelled.get(),
            |_, _| cancelled.set(true),
        );
        assert!(matches!(result, Err(FileTransferError::Cancelled)));
        assert!(!cancelled_destination.exists());
    }
}
