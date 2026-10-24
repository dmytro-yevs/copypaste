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
fn open(reference: &str) -> Result<File, FileTransferError> {
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
pub struct FileReference {
    reference: String,
    size: u64,
    modified: Option<String>,
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
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn publish(
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
    ) -> Result<crate::Ingested, FileTransferError> {
        metadata.source_reference = Some(self.reference.clone());
        let offer = file_transfer::FileTransfer {
            size_bytes: self.size,
            sha256: String::new(),
            source_version: Some(uuid::Uuid::new_v4().simple().to_string()),
        };
        file_transfer::check_limit(&offer, settings.capture_limit_bytes("file"))?;
        let item = file_transfer::offer_item(
            &offer,
            key,
            metadata,
            created_at,
            app_bundle_id,
            app_name,
            context,
            explicit,
        )?;
        let result = store.insert_file_reference(
            item,
            &self.reference,
            self.size,
            self.modified.as_deref(),
            !explicit,
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

pub fn source(
    store: &Store,
    id: &str,
) -> Result<Option<Box<dyn DeferredFileSource>>, FileTransferError> {
    let Some((reference, size, stamp)) = store.file_reference(id)? else {
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
        let mut reader = source(&store, &row.id).unwrap().unwrap();
        assert_eq!(
            reader.read_chunk().unwrap().len(),
            file_transfer::CHUNK_BYTES
        );
        drop(reader);
        File::create(&path).unwrap().set_len(7).unwrap();
        assert!(source(&store, &row.id).is_err());
        assert_eq!(store.count().unwrap(), 2);
    }
}
