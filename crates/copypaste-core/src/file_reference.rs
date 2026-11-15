//! Deferred local sources: selection reads metadata; explicit transfer reads bytes.
use crate::{
    file_transfer::{self, FileTransferError},
    Keyring, Store, StoredItem,
};
use copypaste_p2p::sync::{DeferredFileSource, SyncError};
use std::{
    fs::File,
    io::Read,
    sync::{Arc, OnceLock},
};

type Resolver = Arc<dyn Fn(&str) -> std::io::Result<File> + Send + Sync>;
static RESOLVER: OnceLock<Resolver> = OnceLock::new();
pub fn set_uri_resolver(resolver: Resolver) {
    let _ = RESOLVER.set(resolver);
}
pub(crate) fn open(reference: &str) -> Result<File, FileTransferError> {
    let result = if reference.starts_with("content://") {
        RESOLVER.get().ok_or(FileTransferError::Unavailable)?(reference)
    } else {
        #[cfg(unix)]
        {
            use rustix::fs::{open, Mode, OFlags};
            open(
                reference,
                OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NONBLOCK | OFlags::NOCTTY,
                Mode::empty(),
            )
            .map(File::from)
            .map_err(std::io::Error::from)
        }
        #[cfg(not(unix))]
        {
            File::open(reference)
        }
    };
    result.map_err(|error| match error.kind() {
        std::io::ErrorKind::NotFound => FileTransferError::SourceMissing,
        std::io::ErrorKind::PermissionDenied => FileTransferError::PermissionDenied,
        _ => FileTransferError::Read,
    })
}
fn modified(metadata: &std::fs::Metadata) -> Option<String> {
    let value = metadata
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?;
    Some(format!("{}.{:09}", value.as_secs(), value.subsec_nanos()))
}

/// The accepted file reference owns only metadata, including across commit retries.
/// Files copied together keep their other members in `more`.
pub struct FileReference {
    reference: String,
    size: u64,
    modified: Option<String>,
    more: Vec<(FileReference, crate::FileMetadata)>,
}

impl FileReference {
    pub fn prepare(
        reference: &str,
        limit: u64,
        size_hint: Option<u64>,
    ) -> Result<Self, FileTransferError> {
        let (size, stamp) = if reference.starts_with("content://") {
            (
                size_hint.ok_or_else(|| {
                    FileTransferError::Remote(
                        "The file provider did not report its size. Choose a local file.".into(),
                    )
                })?,
                None,
            )
        } else {
            let metadata = std::fs::metadata(reference).map_err(|error| match error.kind() {
                std::io::ErrorKind::NotFound => FileTransferError::SourceMissing,
                std::io::ErrorKind::PermissionDenied => FileTransferError::PermissionDenied,
                _ => FileTransferError::Read,
            })?;
            if !metadata.is_file() {
                return Err(FileTransferError::NotFile);
            }
            (metadata.len(), modified(&metadata))
        };
        if size > limit {
            return Err(FileTransferError::TooLarge { size, limit });
        }
        Ok(Self {
            reference: reference.into(),
            size,
            modified: stamp,
            more: Vec::new(),
        })
    }

    /// Accept files copied together as one clip. `limit` bounds their total
    /// size, and the set holds at most [`copypaste_payload::MAX_FILES_PER_CLIP`].
    pub fn prepare_set(
        first: &str,
        first_size_hint: Option<u64>,
        more: Vec<(crate::FileMetadata, Option<u64>)>,
        limit: u64,
    ) -> Result<Self, FileTransferError> {
        let count = more.len() + 1;
        if count > copypaste_payload::MAX_FILES_PER_CLIP {
            return Err(FileTransferError::TooManyFiles {
                count,
                limit: copypaste_payload::MAX_FILES_PER_CLIP,
            });
        }
        let mut set = Self::prepare(first, limit, first_size_hint)?;
        let mut total = set.size;
        for (metadata, hint) in more {
            let reference = metadata
                .source_reference
                .clone()
                .ok_or(FileTransferError::InvalidMetadata)?;
            let member = Self::prepare(&reference, limit, hint)?;
            total = total.saturating_add(member.size);
            if total > limit {
                return Err(FileTransferError::TooLarge { size: total, limit });
            }
            set.more.push((member, metadata));
        }
        Ok(set)
    }

    /// The total size of every member.
    #[must_use]
    pub fn size(&self) -> u64 {
        self.more.iter().fold(self.size, |total, (member, _)| {
            total.saturating_add(member.size)
        })
    }

    #[must_use]
    pub fn file_count(&self) -> usize {
        self.more.len() + 1
    }

    #[allow(clippy::too_many_arguments)]
    pub fn publish(
        &self,
        store: &Store,
        key: &crate::ItemKey,
        metadata: crate::FileMetadata,
        created_at: i64,
        app_bundle_id: Option<&str>,
        app_name: Option<&str>,
        context: Option<&crate::PayloadMetadata>,
        settings: &copypaste_ipc::ConfigData,
        explicit: bool,
    ) -> Result<crate::Ingested, FileTransferError> {
        self.publish_with_rules(
            store,
            key,
            metadata,
            created_at,
            app_bundle_id,
            app_name,
            context,
            settings,
            explicit,
            None,
        )
    }

    /// [`FileReference::publish`] for a capture content rules ran on: their
    /// effects are written in the transaction that stores the item.
    #[allow(clippy::too_many_arguments)]
    pub fn publish_with_rules(
        &self,
        store: &Store,
        key: &crate::ItemKey,
        mut metadata: crate::FileMetadata,
        created_at: i64,
        app_bundle_id: Option<&str>,
        app_name: Option<&str>,
        context: Option<&crate::PayloadMetadata>,
        settings: &copypaste_ipc::ConfigData,
        explicit: bool,
        effects: Option<&crate::rules::apply::CaptureEffects>,
    ) -> Result<crate::Ingested, FileTransferError> {
        metadata.source_reference = Some(self.reference.clone());
        let item = if self.more.is_empty() {
            let offer = deferred_offer(self.size);
            file_transfer::check_limit(&offer, settings.capture_limit_bytes("file"))?;
            file_transfer::offer_item(
                &offer,
                key,
                metadata,
                created_at,
                app_bundle_id,
                app_name,
                context,
                explicit,
            )?
        } else {
            let mut files = vec![copypaste_payload::FileSetEntry::new(
                &metadata,
                &deferred_offer(self.size),
                file_transfer::sealed_reference(&self.reference),
            )];
            for (member, member_metadata) in &self.more {
                files.push(copypaste_payload::FileSetEntry::new(
                    member_metadata,
                    &deferred_offer(member.size),
                    file_transfer::sealed_reference(&member.reference),
                ));
            }
            let descriptor = copypaste_payload::FileDescriptor::set(
                files,
                uuid::Uuid::new_v4().simple().to_string(),
            )
            .ok_or(FileTransferError::InvalidMetadata)?;
            file_transfer::check_limit(&descriptor.offer(), settings.capture_limit_bytes("file"))?;
            file_transfer::sealed_offer_item(
                &descriptor,
                key,
                metadata,
                created_at,
                app_bundle_id,
                app_name,
                context,
                explicit,
            )?
        };
        let write = crate::rules::apply::capture_writer(key, effects, created_at);
        let members: Vec<_> = std::iter::once(self)
            .chain(self.more.iter().map(|(member, _)| member))
            .map(|member| {
                (
                    member.reference.as_str(),
                    member.size,
                    member.modified.as_deref(),
                )
            })
            .collect();
        let result = store.insert_file_reference(
            item,
            &members,
            !explicit,
            write.as_ref().map(|write| write as _),
        )?;
        crate::ingest::enforce_retention(store, settings);
        Ok(result)
    }
}

pub fn import(
    store: &Store,
    keyring: &Keyring,
    reference: &str,
    name: &str,
    mime: &str,
    settings: &copypaste_ipc::ConfigData,
    size_hint: Option<u64>,
) -> Result<StoredItem, FileTransferError> {
    let local = store
        .current_device_name()
        .unwrap_or_else(|_| "This device".into());
    let mut job = crate::file_operation::FileOperationGuard::start(
        store,
        "",
        name,
        copypaste_ipc::FileOperationKind::Import,
        local.clone(),
        local,
        0,
        None,
    );
    let result = (|| {
        let prepared =
            FileReference::prepare(reference, settings.capture_limit_bytes("file"), size_hint)?;
        let attributes = crate::FileMetadata::with_source_reference(name, mime, reference)
            .ok_or(FileTransferError::InvalidMetadata)?;
        let row = prepared
            .publish(
                store,
                &keyring.item_key(),
                attributes,
                crate::now_ms(),
                None,
                None,
                None,
                settings,
                true,
            )?
            .into_item();
        job.item(&row.id, prepared.size);
        Ok(row)
    })();
    job.finish(
        result.as_ref().err().map(ToString::to_string),
        false,
        result
            .as_ref()
            .ok()
            .and_then(|_| (!reference.starts_with("content://")).then(|| reference.into())),
    );
    result
}

fn deferred_offer(size: u64) -> file_transfer::FileTransfer {
    file_transfer::FileTransfer {
        size_bytes: size,
        sha256: String::new(),
        source_version: Some(uuid::Uuid::new_v4().simple().to_string()),
    }
}

/// Copy mode: keep an encrypted copy of every captured member beside its
/// link, so the files outlive their originals. The synced version does not
/// change. Returns whether any member was copied.
pub fn keep_copy(
    store: &Store,
    keyring: &Keyring,
    id: &str,
    settings: &copypaste_ipc::ConfigData,
) -> Result<bool, FileTransferError> {
    let Some(row) = store.get(id)?.filter(|row| !row.deleted) else {
        return Ok(false);
    };
    let members = store.file_references(id)?;
    let size: u64 = members.iter().map(|(_, size, _)| size).sum();
    let limit = settings
        .capture_limit_bytes(copypaste_ipc::content_type::FILE)
        .min(settings.storage_quota_bytes);
    if size > limit {
        return Err(FileTransferError::TooLarge { size, limit });
    }
    let kept: Vec<usize> = store
        .file_copies(id)?
        .into_iter()
        .map(|(index, _)| index)
        .collect();
    let mut copied = false;
    for (index, (_, size, _)) in members.iter().enumerate() {
        if kept.contains(&index) {
            continue;
        }
        let mut first = source(store, id, index)?.ok_or(FileTransferError::Unavailable)?;
        let mut hash = <sha2::Sha256 as sha2::Digest>::new();
        loop {
            let bytes = first.read_chunk().map_err(|_| FileTransferError::Read)?;
            if bytes.is_empty() {
                break;
            }
            sha2::Digest::update(&mut hash, &bytes);
        }
        first
            .verify()
            .map_err(|_| FileTransferError::SourceChanged)?;
        let body = file_transfer::FileTransfer {
            source_version: None,
            size_bytes: *size,
            sha256: hex::encode(sha2::Digest::finalize(hash)),
        };
        let mut reader = SourceReader {
            source: source(store, id, index)?.ok_or(FileTransferError::Unavailable)?,
            pending: Vec::new(),
        };
        if !store.attach_file_copy(
            id,
            index,
            &row.content_hash,
            &mut reader,
            &body,
            &keyring.item_key(),
        )? {
            break;
        }
        copied = true;
    }
    if copied {
        crate::ingest::enforce_retention(store, settings);
    }
    Ok(copied)
}

/// Streams a verified source into the chunk writer.
struct SourceReader {
    source: Box<dyn DeferredFileSource>,
    pending: Vec<u8>,
}
impl Read for SourceReader {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        if self.pending.is_empty() {
            self.pending = self
                .source
                .read_chunk()
                .map_err(|_| std::io::Error::other("source read failed"))?;
        }
        let n = self.pending.len().min(buffer.len());
        buffer[..n].copy_from_slice(&self.pending[..n]);
        self.pending.drain(..n);
        Ok(n)
    }
}

/// A member's original, or its kept copy (Copy mode) once the original is
/// missing or changed.
pub fn source_or_copy(
    store: &Store,
    keyring: &Arc<Keyring>,
    id: &str,
    index: usize,
) -> Result<Option<Box<dyn DeferredFileSource>>, FileTransferError> {
    let original = source(store, id, index);
    if matches!(original, Ok(Some(_))) {
        return original;
    }
    if store.file_copy_blob(id, index)?.is_none() {
        return original;
    }
    let size = store
        .file_references(id)?
        .get(index)
        .map(|(_, size, _)| *size)
        .ok_or(FileTransferError::Unavailable)?;
    Ok(Some(Box::new(KeptCopy {
        store: store.clone(),
        keyring: Arc::clone(keyring),
        id: id.into(),
        index,
        size,
        read: 0,
        chunk: 0,
    })))
}

/// One plaintext chunk of member `index`'s kept copy.
pub fn kept_copy_chunk(
    store: &Store,
    key: &crate::ItemKey,
    id: &str,
    index: usize,
    chunk: u64,
) -> Result<zeroize::Zeroizing<Vec<u8>>, FileTransferError> {
    let blob = store
        .file_copy_blob(id, index)?
        .ok_or(FileTransferError::Unavailable)?;
    let size = store
        .file_references(id)?
        .get(index)
        .map(|(_, size, _)| *size)
        .ok_or(FileTransferError::Unavailable)?;
    let offset = chunk * file_transfer::CHUNK_BYTES as u64;
    if offset >= size.max(1) {
        return Err(FileTransferError::Integrity);
    }
    let sealed = store
        .file_copy_chunk(id, index, &blob, chunk)?
        .ok_or(FileTransferError::Unavailable)?;
    let bytes = crate::decrypt(
        &sealed.ciphertext,
        &sealed.nonce,
        key,
        &file_transfer::chunk_id(&blob, chunk),
    )?;
    if bytes.len() as u64 != (size - offset).min(file_transfer::CHUNK_BYTES as u64) {
        return Err(FileTransferError::Integrity);
    }
    Ok(bytes)
}

/// Reads a kept copy back from encrypted chunks.
struct KeptCopy {
    store: Store,
    keyring: Arc<Keyring>,
    id: String,
    index: usize,
    size: u64,
    read: u64,
    chunk: u64,
}
impl DeferredFileSource for KeptCopy {
    fn read_chunk(&mut self) -> Result<Vec<u8>, SyncError> {
        if self.read >= self.size {
            return Ok(Vec::new());
        }
        crate::sync::blocking(|| {
            let mut bytes = kept_copy_chunk(
                &self.store,
                &self.keyring.item_key(),
                &self.id,
                self.index,
                self.chunk,
            )
            .map_err(|_| SyncError::Source("The kept copy could not be read.".into()))?;
            self.read += bytes.len() as u64;
            self.chunk += 1;
            Ok(std::mem::take(&mut *bytes))
        })
    }
    fn verify(&mut self) -> Result<(), SyncError> {
        if self.read == self.size {
            Ok(())
        } else {
            Err(SyncError::Source("The kept copy is incomplete.".into()))
        }
    }
}

/// Open the deferred source of one member; `index` is 0 for a single file.
pub fn source(
    store: &Store,
    id: &str,
    index: usize,
) -> Result<Option<Box<dyn DeferredFileSource>>, FileTransferError> {
    let Some((reference, size, stamp)) = store.file_references(id)?.into_iter().nth(index) else {
        return Ok(None);
    };
    let file = open(&reference)?;
    let metadata = file.metadata().map_err(|_| FileTransferError::Read)?;
    let regular = metadata.is_file();
    if (regular
        && (metadata.len() != size
            || stamp
                .as_ref()
                .is_some_and(|stamp| modified(&metadata).as_ref() != Some(stamp))))
        || (!regular && !reference.starts_with("content://"))
    {
        return Err(FileTransferError::SourceChanged);
    }
    Ok(Some(Box::new(Source {
        file,
        size,
        stamp: regular.then(|| modified(&metadata)).flatten(),
        read: 0,
        regular,
    })))
}
struct Source {
    file: File,
    size: u64,
    stamp: Option<String>,
    read: u64,
    regular: bool,
}
impl DeferredFileSource for Source {
    fn read_chunk(&mut self) -> Result<Vec<u8>, SyncError> {
        crate::sync::blocking(|| {
            let mut bytes =
                vec![0; (self.size - self.read).min(file_transfer::CHUNK_BYTES as u64) as usize];
            self.file
                .read_exact(&mut bytes)
                .map_err(|_| SyncError::Source("The source file could not be read.".into()))?;
            self.read += bytes.len() as u64;
            Ok(bytes)
        })
    }
    fn verify(&mut self) -> Result<(), SyncError> {
        crate::sync::blocking(|| {
            let metadata = self
                .file
                .metadata()
                .map_err(|_| SyncError::Source("The source file is unavailable.".into()))?;
            if self.read != self.size
                || (self.regular
                    && (metadata.len() != self.size || modified(&metadata) != self.stamp))
            {
                return Err(SyncError::Source(
                    "The source file changed during transfer. Add it again.".into(),
                ));
            }
            if !self.regular {
                let mut extra = [0];
                if self.file.read(&mut extra).map_err(|_| {
                    SyncError::Source("The source file could not be verified.".into())
                })? != 0
                {
                    return Err(SyncError::Source(
                        "The source file changed. Add it again.".into(),
                    ));
                }
            }
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn capturing_a_large_reference_preserves_history_and_recopy_uses_the_original() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("large.bin");
        let size = 15 * 1024 * 1024 * 1024;
        File::create(&path).unwrap().set_len(size).unwrap();
        let keyring = Keyring::from_secret(&[61; 32]);
        let store = Store::open_in_memory(&keyring.db_key()).unwrap();
        let settings = copypaste_ipc::ConfigData {
            storage_quota_bytes: 50 * 1024 * 1024,
            max_file_size_bytes: 20 * 1024 * 1024 * 1024,
            ..Default::default()
        };
        let older = crate::ingest_into(&store, &keyring, "keep my history", "text", 1, &settings)
            .unwrap()
            .into_item();
        let metadata = crate::FileMetadata::with_source_reference(
            "large.bin",
            "application/octet-stream",
            path.to_str().unwrap(),
        )
        .unwrap();
        let prepared =
            FileReference::prepare(path.to_str().unwrap(), settings.max_file_size_bytes, None)
                .unwrap();
        let first = prepared
            .publish(
                &store,
                &keyring.item_key(),
                metadata.clone(),
                2,
                None,
                None,
                None,
                &settings,
                false,
            )
            .unwrap()
            .into_item();
        assert!(store.get(&older.id).unwrap().is_some());
        assert_eq!(store.count().unwrap(), 2);
        assert!(!store.has_file_transfer(&first.id).unwrap());
        assert_eq!(store.file_reference(&first.id).unwrap().unwrap().1, size);
        store.set_pinned(&first.id, true).unwrap();
        let recopy = prepared
            .publish(
                &store,
                &keyring.item_key(),
                metadata,
                3,
                None,
                None,
                None,
                &settings,
                false,
            )
            .unwrap();
        assert!(matches!(recopy, crate::Ingested::Duplicate(_)));
        let recopy = recopy.into_item();
        assert_eq!(recopy.id, first.id);
        assert!(recopy.pinned);
        assert_eq!(store.count().unwrap(), 2);
        let origin = crate::history_item::ItemOrigin {
            device_id: "local".into(),
            device_name: None,
            device_class: Default::default(),
        };
        let copy = crate::history_actions::prepare_copy(
            &store,
            &keyring.item_key(),
            crate::history_actions::open(&store, &keyring.item_key(), &first.id).unwrap(),
            crate::history_actions::CopyOptions {
                origin: &origin,
                local_device_id: "local",
                plain_text: false,
                allow_content_uri: false,
                max_file_size_bytes: settings.max_file_size_bytes,
            },
        )
        .unwrap();
        let crate::ClipboardPayload::File {
            bytes,
            metadata: Some(metadata),
        } = copy.payload()
        else {
            panic!("file reference expected")
        };
        assert!(bytes.is_empty());
        assert!(metadata.transfer.is_none());
        assert_eq!(metadata.source_reference.as_deref(), path.to_str());
    }

    #[test]
    fn uri_selection_registers_metadata_without_opening_the_provider() {
        let keyring = Keyring::from_secret(&[46; 32]);
        let store = Store::open_in_memory(&keyring.db_key()).unwrap();
        let reference = "content://documents/large-file";
        let row = import(
            &store,
            &keyring,
            reference,
            "large.bin",
            "application/octet-stream",
            &copypaste_ipc::ConfigData::default(),
            Some(16 * 1024 * 1024),
        )
        .unwrap();
        assert_eq!(store.file_reference(&row.id).unwrap().unwrap().0, reference);
        assert_eq!(
            store.file_operations(false, None, None).unwrap()[0].phase,
            copypaste_ipc::FileOperationPhase::Completed
        );
    }

    #[test]
    fn secrecy_redacts_operation_history_and_stops_an_open_peer_reader() {
        use copypaste_p2p::sync::SyncSource;
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("private.bin");
        std::fs::write(&path, vec![7; file_transfer::CHUNK_BYTES * 2]).unwrap();
        let keyring = Arc::new(Keyring::from_secret(&[49; 32]));
        let store = Store::open_in_memory(&keyring.db_key()).unwrap();
        let settings = copypaste_ipc::ConfigData::default();
        let row = import(
            &store,
            &keyring,
            path.to_str().unwrap(),
            "private.bin",
            "application/octet-stream",
            &settings,
            None,
        )
        .unwrap();
        let original = store.file_operations(false, None, None).unwrap()[0].clone();
        let source = crate::StoreSource::new(
            store.clone(),
            Arc::clone(&keyring),
            "source".into(),
            "Source".into(),
            settings,
        );
        let mut reader = source.deferred_file(&row.id).unwrap().unwrap();
        assert_eq!(
            reader.read_chunk().unwrap().len(),
            file_transfer::CHUNK_BYTES
        );
        store.set_secret(&keyring, &row.id, true).unwrap();
        assert!(reader.read_chunk().is_err());
        store.write_file_operation(&original).unwrap();
        assert_eq!(
            store.file_operations(false, None, None).unwrap()[0].name,
            "Hidden file"
        );
        assert!(store.file_operations(false, None, None).unwrap()[0]
            .local_path
            .is_none());
        store.set_secret(&keyring, &row.id, false).unwrap();
        assert_eq!(
            store.file_operations(false, None, None).unwrap()[0].name,
            "private.bin"
        );
        assert_eq!(
            store.file_operations(false, None, None).unwrap()[0]
                .local_path
                .as_deref(),
            path.to_str()
        );
    }
    #[test]
    fn selection_records_only_metadata_and_refuses_a_changed_original() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("large.bin");
        File::create(&path)
            .unwrap()
            .set_len(8 * 1024 * 1024)
            .unwrap();
        let keyring = Keyring::from_secret(&[45; 32]);
        let store = Store::open_in_memory(&keyring.db_key()).unwrap();
        let settings = copypaste_ipc::ConfigData::default();
        let row = import(
            &store,
            &keyring,
            path.to_str().unwrap(),
            "large.bin",
            "application/octet-stream",
            &settings,
            None,
        )
        .unwrap();
        let second = import(
            &store,
            &keyring,
            path.to_str().unwrap(),
            "another.bin",
            "application/octet-stream",
            &settings,
            None,
        )
        .unwrap();
        assert_ne!(row.id, second.id);
        assert!(!store.has_file_transfer(&row.id).unwrap());
        assert_eq!(
            store.file_reference(&row.id).unwrap().unwrap().1,
            8 * 1024 * 1024
        );
        let mut reader = source(&store, &row.id, 0).unwrap().unwrap();
        assert_eq!(
            reader.read_chunk().unwrap().len(),
            file_transfer::CHUNK_BYTES
        );
        drop(reader);
        File::create(&path).unwrap().set_len(7).unwrap();
        assert!(source(&store, &row.id, 0).is_err());
        assert_eq!(store.count().unwrap(), 2);
    }
    #[test]
    fn local_selection_accepts_only_bounded_regular_files() {
        let directory = tempfile::tempdir().unwrap();
        let prepare = |path: &std::path::Path, limit| {
            FileReference::prepare(path.to_str().unwrap(), limit, None).map(|_| ())
        };
        assert!(matches!(
            prepare(&directory.path().join("absent"), 4),
            Err(FileTransferError::SourceMissing)
        ));
        assert!(matches!(
            prepare(directory.path(), 4),
            Err(FileTransferError::NotFile)
        ));
        #[cfg(any(target_os = "linux", target_os = "android"))]
        {
            use rustix::fs::{mkfifoat, Mode, CWD};
            let fifo = directory.path().join("fifo");
            mkfifoat(CWD, &fifo, Mode::RUSR | Mode::WUSR).unwrap();
            assert!(matches!(prepare(&fifo, 4), Err(FileTransferError::NotFile)));
        }
        let regular = directory.path().join("regular");
        std::fs::write(&regular, b"1234").unwrap();
        assert!(prepare(&regular, 4).is_ok());
        assert!(matches!(
            prepare(&regular, 3),
            Err(FileTransferError::TooLarge { size: 4, limit: 3 })
        ));
        #[cfg(unix)]
        {
            let link = directory.path().join("link");
            std::os::unix::fs::symlink(&regular, &link).unwrap();
            assert!(prepare(&link, 4).is_ok());
        }
    }
}
