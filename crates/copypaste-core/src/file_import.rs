//! Explicit file imports: separate History rows, shared encrypted original bytes.
use crate::{ingest::IngestError, Keyring, NewItem, Store, StoredItem};
use std::{fs::File, io::Read, path::Path};

#[derive(Debug, thiserror::Error)]
pub enum FileImportError {
    #[error("The selected file could not be read or changed during import.")]
    Read,
    #[error("Only regular files can be imported.")]
    NotFile,
    #[error("The file exceeds the configured History size limit.")]
    TooLarge,
    #[error(transparent)]
    Ingest(#[from] IngestError),
}

pub fn import_file(
    store: &Store,
    keyring: &Keyring,
    path: &Path,
    filename: &str,
    mime_type: &str,
    source_reference: Option<&str>,
    settings: &copypaste_ipc::ConfigData,
) -> Result<StoredItem, FileImportError> {
    let metadata = match source_reference {
        Some(reference) => {
            crate::FileMetadata::with_source_reference(filename, mime_type, reference)
        }
        None => crate::FileMetadata::new(filename, mime_type),
    }
    .ok_or(IngestError::InvalidMetadata)?;
    // Open nonblocking on Unix so a dropped FIFO can never hang an import.
    #[cfg(unix)]
    let file = {
        use rustix::fs::{open, Mode, OFlags};
        File::from(
            open(
                path,
                OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NONBLOCK | OFlags::NOCTTY,
                Mode::empty(),
            )
            .map_err(|_| FileImportError::Read)?,
        )
    };
    #[cfg(not(unix))]
    let file = File::open(path).map_err(|_| FileImportError::Read)?;
    let before = file.metadata().map_err(|_| FileImportError::Read)?;
    if !before.is_file() {
        return Err(FileImportError::NotFile);
    }
    let cap = copypaste_ipc::MAX_CONTENT_BYTES as u64;
    if before.len() > cap {
        return Err(FileImportError::TooLarge);
    }
    let mut reader = file.take(cap + 1);
    let mut bytes = zeroize::Zeroizing::new(Vec::new());
    reader
        .read_to_end(&mut bytes)
        .map_err(|_| FileImportError::Read)?;
    let after = reader
        .get_ref()
        .metadata()
        .map_err(|_| FileImportError::Read)?;
    if bytes.len() as u64 > cap {
        return Err(FileImportError::TooLarge);
    }
    if before.len() != bytes.len() as u64
        || before.len() != after.len()
        || before.modified().ok() != after.modified().ok()
    {
        return Err(FileImportError::Read);
    }
    let image_type = image::guess_format(&bytes).ok().filter(|format| {
        matches!(
            format,
            image::ImageFormat::Png
                | image::ImageFormat::Tiff
                | image::ImageFormat::Jpeg
                | image::ImageFormat::Gif
                | image::ImageFormat::WebP
                | image::ImageFormat::Bmp
        )
    });
    let content_type = image_type.map_or(copypaste_ipc::content_type::FILE, |format| {
        format.to_mime_type()
    });
    if bytes.len() as u64 > settings.capture_limit_bytes(content_type) {
        return Err(FileImportError::TooLarge);
    }
    let payload_metadata = crate::PayloadMetadata::new(Some(metadata), None)
        .and_then(|value| value.to_json(content_type))
        .ok_or(IngestError::InvalidMetadata)?;
    let digest = crate::binary::content_digest(&bytes);
    let id = format!(
        "{}{}",
        crate::binary_reference::IMPORT_ID_PREFIX,
        uuid::Uuid::new_v4()
    );
    let ciphertext = crate::binary_reference::seal(&bytes, &digest, &keyring.item_key(), &id)
        .map_err(IngestError::from)?;
    let item = store
        .insert_imported_file(NewItem {
            id,
            content_ciphertext: ciphertext,
            nonce: Vec::new(),
            content_type: content_type.to_owned(),
            content_hash: crate::binary::content_hash(&digest),
            search_text: None,
            created_at: crate::now_ms(),
            app_bundle_id: None,
            app_name: None,
            payload_metadata: Some(payload_metadata),
        })
        .map_err(IngestError::from)?;
    crate::ingest::enforce_retention(store, settings);
    Ok(item)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ClipboardPayload, PayloadMetadata};

    fn fixture() -> (Store, Keyring, tempfile::TempDir) {
        let keyring = Keyring::from_secret(&[23; 32]);
        (
            Store::open_in_memory(&keyring.db_key()).unwrap(),
            keyring,
            tempfile::tempdir().unwrap(),
        )
    }
    fn import(store: &Store, key: &Keyring, path: &Path, name: &str) -> StoredItem {
        import_file(
            store,
            key,
            path,
            name,
            "application/pdf",
            Some("content://documents/original"),
            &Default::default(),
        )
        .unwrap()
    }
    fn blob_count(store: &Store) -> u64 {
        store.imported_payload_count()
    }

    #[test]
    fn identical_files_keep_two_named_rows_and_one_encrypted_payload() {
        let (store, key, dir) = fixture();
        let path = dir.path().join("source");
        let bytes = b"%PDF original bytes\0\xff";
        std::fs::write(&path, bytes).unwrap();
        let first = import(&store, &key, &path, "a.pdf");
        let second = import(&store, &key, &path, "b.pdf");
        assert_ne!(first.id, second.id);
        assert_eq!(store.count().unwrap(), 2);
        assert_eq!(blob_count(&store), 1);
        std::fs::remove_file(path).unwrap();
        for (item, name) in [(&first, "a.pdf"), (&second, "b.pdf")] {
            let row = store.get(&item.id).unwrap().unwrap();
            let ClipboardPayload::File {
                bytes: recovered,
                metadata,
            } = ClipboardPayload::open(&row, &key.item_key()).unwrap()
            else {
                panic!("expected file");
            };
            assert_eq!(&*recovered, bytes);
            let metadata = metadata.unwrap();
            assert_eq!(metadata.filename, name);
            assert_eq!(
                metadata.source_reference.as_deref(),
                Some("content://documents/original")
            );
            let saved = dir.path().join(name);
            ClipboardPayload::open(&row, &key.item_key())
                .unwrap()
                .save_file_to(&saved)
                .unwrap();
            assert_eq!(std::fs::read(saved).unwrap(), bytes);
            assert!(
                crate::open_binary(&row.content_ciphertext, &key.item_key(), "another-row")
                    .is_err()
            );
        }
        store.delete(&first.id).unwrap();
        assert_eq!(blob_count(&store), 1);
        assert!(
            ClipboardPayload::open(&store.get(&second.id).unwrap().unwrap(), &key.item_key())
                .is_ok()
        );
        store.delete(&second.id).unwrap();
        assert_eq!(blob_count(&store), 0);
    }

    #[test]
    fn backup_restore_preserves_shared_files_and_their_names() {
        let (store, key, dir) = fixture();
        let path = dir.path().join("source");
        std::fs::write(&path, b"original").unwrap();
        let a = import(&store, &key, &path, "a.pdf");
        let b = import(&store, &key, &path, "b.pdf");
        let backup = dir.path().join("backup.db");
        store.backup_to(&backup).unwrap();
        store.delete_all().unwrap();
        assert_eq!(blob_count(&store), 0);
        store.restore_from(&backup, &key.db_key()).unwrap();
        assert_eq!(store.count().unwrap(), 2);
        assert_eq!(blob_count(&store), 1);
        for id in [a.id, b.id] {
            assert_eq!(
                &*crate::open_binary(
                    &store.get(&id).unwrap().unwrap().content_ciphertext,
                    &key.item_key(),
                    &id
                )
                .unwrap(),
                b"original"
            );
        }
    }

    #[test]
    fn synced_imports_remain_distinct_and_share_a_payload_on_the_receiver() {
        let (store, key, dir) = fixture();
        let (peer, peer_key, _) = fixture();
        let path = dir.path().join("source");
        std::fs::write(&path, b"original").unwrap();
        for name in ["a.pdf", "b.pdf"] {
            let item = import(&store, &key, &path, name);
            let incoming = crate::sync::RemoteVersion {
                item_id: &item.id,
                content: "",
                binary_content: Some(b"original"),
                payload_metadata: item.payload_metadata.as_deref(),
                content_type: &item.content_type,
                created_at: item.created_at,
                deleted: false,
                content_hash: None,
                origin_device_id: "source-device",
                app_bundle_id: None,
                app_name: None,
            };
            assert!(
                crate::sync::apply_remote_version(&peer, &peer_key, "receiver", &incoming).unwrap()
            );
            let row = peer.get(&item.id).unwrap().unwrap();
            assert_eq!(
                &*crate::open_binary(&row.content_ciphertext, &peer_key.item_key(), &row.id)
                    .unwrap(),
                b"original"
            );
        }
        assert_eq!(peer.count().unwrap(), 2);
        assert_eq!(blob_count(&peer), 1);
    }

    #[test]
    fn imports_images_without_reencoding_and_retains_file_metadata() {
        let (store, key, dir) = fixture();
        let path = dir.path().join("photo.jpg");
        let image = image::DynamicImage::new_rgb8(3, 2);
        let mut buffer = std::io::Cursor::new(Vec::new());
        image
            .write_to(&mut buffer, image::ImageFormat::Jpeg)
            .unwrap();
        std::fs::write(&path, buffer.get_ref()).unwrap();
        let row = import_file(
            &store,
            &key,
            &path,
            "photo.jpg",
            "image/jpeg",
            None,
            &Default::default(),
        )
        .unwrap();
        assert_eq!(row.content_type, "image/jpeg");
        let metadata =
            PayloadMetadata::from_json(row.payload_metadata.as_deref().unwrap(), &row.content_type)
                .unwrap();
        assert_eq!(metadata.file.unwrap().filename, "photo.jpg");
        assert_eq!(
            &*crate::open_binary(&row.content_ciphertext, &key.item_key(), &row.id).unwrap(),
            buffer.get_ref()
        );
        let preview = crate::thumbnail_png(buffer.get_ref(), 16, Some(128), None).unwrap();
        assert_eq!((preview.width, preview.height), (3, 2));
    }

    #[test]
    fn accepts_empty_files_and_rejects_directories_oversize_and_bad_metadata() {
        let (store, key, dir) = fixture();
        let path = dir.path().join("empty");
        std::fs::write(&path, b"").unwrap();
        let row = import(&store, &key, &path, "empty.pdf");
        assert!(
            crate::open_binary(&row.content_ciphertext, &key.item_key(), &row.id)
                .unwrap()
                .is_empty()
        );
        assert!(matches!(
            import_file(
                &store,
                &key,
                dir.path(),
                "dir",
                "application/octet-stream",
                None,
                &Default::default()
            ),
            Err(FileImportError::NotFile)
        ));
        std::fs::write(&path, b"12345").unwrap();
        let settings = copypaste_ipc::ConfigData {
            max_file_size_bytes: 4,
            ..Default::default()
        };
        assert!(matches!(
            import_file(
                &store,
                &key,
                &path,
                "large.pdf",
                "application/pdf",
                None,
                &settings
            ),
            Err(FileImportError::TooLarge)
        ));
        assert!(import_file(
            &store,
            &key,
            &path,
            "../bad",
            "application/pdf",
            None,
            &Default::default()
        )
        .is_err());
        assert_eq!(store.count().unwrap(), 1);
    }
}
