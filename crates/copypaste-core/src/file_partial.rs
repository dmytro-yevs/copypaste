//! A remote download written beside its destination and published only after
//! verification. Its verified prefix survives restarts and disconnects.
use crate::{
    file_transfer::{destination_error, FileTransfer, FileTransferError, CHUNK_BYTES},
    storage::DownloadPartial,
    Keyring, Store,
};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};
use zeroize::Zeroizing;

/// Flush and record progress every 4 MiB: at most this much is re-fetched
/// after a crash, and the database sees one write per checkpoint.
const CHECKPOINT_BYTES: u64 = 64 * CHUNK_BYTES as u64;
/// Partials untouched this long are abandoned and removed.
const STALE_AFTER_MS: i64 = 14 * 24 * 60 * 60 * 1000;
const PARTIAL_SUFFIX: &str = ".copypaste-partial";

pub(crate) fn partial_path(destination: &Path) -> PathBuf {
    let mut path = destination.as_os_str().to_owned();
    path.push(PARTIAL_SUFFIX);
    PathBuf::from(path)
}

/// What identifies a member's bytes: its checksum, or its source version.
pub(crate) fn member_version(offer: &FileTransfer) -> String {
    if offer.sha256.is_empty() {
        offer.source_version.clone().unwrap_or_default()
    } else {
        offer.sha256.clone()
    }
}

pub(crate) struct PartialFile {
    store: Store,
    record: DownloadPartial,
    file: Option<File>,
    hash: Sha256,
    written: u64,
}

impl PartialFile {
    /// Continue the recorded partial of this member when it still matches,
    /// otherwise start an empty one. The destination itself must not exist.
    pub(crate) fn open(
        store: &Store,
        id: &str,
        index: usize,
        version: &str,
        destination: &Path,
    ) -> Result<Self, FileTransferError> {
        if destination.exists() {
            return Err(FileTransferError::DestinationExists);
        }
        let path = partial_path(destination);
        let record = DownloadPartial {
            item_id: id.into(),
            file_index: index,
            file_version: version.into(),
            partial_path: path.to_string_lossy().into_owned(),
            destination: destination.to_string_lossy().into_owned(),
            verified_bytes: 0,
            updated_at: crate::now_ms(),
        };
        let previous = store.download_partial(id, index)?;
        let mut resume = 0;
        if let Some(previous) = &previous {
            if previous.file_version == record.file_version
                && previous.partial_path == record.partial_path
                && previous.destination == record.destination
            {
                resume = previous.verified_bytes;
            } else {
                let _ = std::fs::remove_file(&previous.partial_path);
                store.remove_download_partial(id, index)?;
            }
        }
        let mut partial = Self {
            store: store.clone(),
            record,
            file: None,
            hash: Sha256::new(),
            written: 0,
        };
        if resume > 0 && partial.reopen(&path, resume).is_ok() {
            partial.written = resume;
        } else {
            partial.hash = Sha256::new();
            if previous.is_some_and(|previous| previous.partial_path == partial.record.partial_path)
            {
                let _ = std::fs::remove_file(&path);
            }
            partial.file = Some(copypaste_fs::create_private(&path).map_err(destination_error)?);
        }
        partial.save()?;
        Ok(partial)
    }

    /// Re-hash the verified prefix and drop anything written after it.
    fn reopen(&mut self, path: &Path, verified: u64) -> std::io::Result<()> {
        let mut file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)?;
        if file.metadata()?.len() < verified {
            return Err(std::io::ErrorKind::UnexpectedEof.into());
        }
        let mut buffer = Zeroizing::new(vec![0; CHUNK_BYTES]);
        let mut remaining = verified;
        while remaining > 0 {
            let size = remaining.min(CHUNK_BYTES as u64) as usize;
            file.read_exact(&mut buffer[..size])?;
            self.hash.update(&buffer[..size]);
            remaining -= size as u64;
        }
        file.set_len(verified)?;
        file.seek(SeekFrom::Start(verified))?;
        self.file = Some(file);
        Ok(())
    }

    pub(crate) fn written(&self) -> u64 {
        self.written
    }

    pub(crate) fn hash(&self) -> String {
        hex::encode(self.hash.clone().finalize())
    }

    pub(crate) fn write(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        self.file.as_mut().expect("open partial").write_all(bytes)?;
        self.hash.update(bytes);
        self.written += bytes.len() as u64;
        if self.written - self.record.verified_bytes >= CHECKPOINT_BYTES {
            self.checkpoint()
                .map_err(|_| std::io::Error::other("The download progress could not be saved."))?;
        }
        Ok(())
    }

    /// Make every written byte durable, then record it as verified.
    pub(crate) fn checkpoint(&mut self) -> Result<(), FileTransferError> {
        self.file
            .as_ref()
            .expect("open partial")
            .sync_data()
            .map_err(destination_error)?;
        self.save()
    }

    /// Start over for a source that cannot resume.
    pub(crate) fn restart(&mut self) -> Result<(), FileTransferError> {
        let file = self.file.as_mut().expect("open partial");
        file.set_len(0).map_err(destination_error)?;
        file.seek(SeekFrom::Start(0)).map_err(destination_error)?;
        self.hash = Sha256::new();
        self.written = 0;
        self.save()
    }

    fn save(&mut self) -> Result<(), FileTransferError> {
        self.record.verified_bytes = self.written;
        self.record.updated_at = crate::now_ms();
        self.store.put_download_partial(&self.record)?;
        Ok(())
    }

    /// Remove the partial and its record; the download starts over next time.
    pub(crate) fn discard(mut self) {
        self.file.take();
        let _ = std::fs::remove_file(&self.record.partial_path);
        let _ = self
            .store
            .remove_download_partial(&self.record.item_id, self.record.file_index);
    }

    /// Move the verified file to its destination without replacing anything.
    pub(crate) fn publish(mut self, destination: &Path) -> Result<(), FileTransferError> {
        let file = self.file.take().expect("open partial");
        file.sync_data().map_err(destination_error)?;
        drop(file);
        let partial = PathBuf::from(&self.record.partial_path);
        match std::fs::hard_link(&partial, destination) {
            Ok(()) => {
                let _ = std::fs::remove_file(&partial);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                return Err(FileTransferError::DestinationExists);
            }
            // File systems without hard links still publish atomically.
            Err(_) if !destination.exists() => {
                std::fs::rename(&partial, destination).map_err(destination_error)?;
            }
            Err(_) => return Err(FileTransferError::DestinationExists),
        }
        self.store
            .remove_download_partial(&self.record.item_id, self.record.file_index)?;
        Ok(())
    }
}

/// Remove partials whose clip is gone, whose member changed, whose file is
/// missing, or that have not progressed for [`STALE_AFTER_MS`].
pub fn sweep(store: &Store, keyring: &Keyring) {
    let Ok(partials) = store.download_partials() else {
        return;
    };
    let now = crate::now_ms();
    for partial in partials {
        let current = store
            .get(&partial.item_id)
            .ok()
            .flatten()
            .filter(|row| !row.deleted)
            .and_then(|row| {
                crate::file_transfer::member_offer(
                    &row,
                    &keyring.item_key(),
                    partial.file_index as u32,
                )
                .ok()
                .flatten()
            });
        let live = current.is_some_and(|offer| member_version(&offer) == partial.file_version)
            && now.saturating_sub(partial.updated_at) < STALE_AFTER_MS
            && Path::new(&partial.partial_path).is_file();
        if !live {
            let _ = std::fs::remove_file(&partial.partial_path);
            let _ = store.remove_download_partial(&partial.item_id, partial.file_index);
        }
    }
}
