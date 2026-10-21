//! Explicit file imports publish a small offer and retain encrypted local chunks.
pub use crate::file_transfer::FileTransferError as FileImportError;
use crate::{Keyring, Store, StoredItem};
use std::path::Path;

pub fn import_file(
    store: &Store,
    keyring: &Keyring,
    path: &Path,
    filename: &str,
    mime_type: &str,
    source_reference: Option<&str>,
    settings: &copypaste_ipc::ConfigData,
) -> Result<StoredItem, FileImportError> {
    let local = store
        .current_device_name()
        .unwrap_or_else(|_| "This device".into());
    let mut job = crate::file_operation::FileOperationGuard::start(
        store,
        "",
        filename,
        copypaste_ipc::FileOperationKind::Import,
        local.clone(),
        local,
        0,
        None,
    );
    let result = (|| {
        let metadata = match source_reference {
            Some(reference) => {
                crate::FileMetadata::with_source_reference(filename, mime_type, reference)
            }
            None => crate::FileMetadata::new(filename, mime_type),
        }
        .ok_or(FileImportError::InvalidMetadata)?;
        crate::file_transfer::import(
            store,
            keyring,
            path,
            metadata,
            crate::now_ms(),
            None,
            None,
            None,
            settings,
            true,
        )
    })();
    if let Ok(row) = &result {
        let total = crate::file_transfer::row_offer(row, &keyring.item_key())
            .ok()
            .flatten()
            .map_or(0, |offer| offer.size_bytes);
        job.item(&row.id, total);
    }
    job.finish(result.as_ref().err().map(ToString::to_string), false, None);
    result
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

    #[test]
    fn legacy_file_upgrade_preserves_identity_pin_name_and_original_bytes() {
        let (store, key, dir) = fixture();
        let metadata = crate::FileMetadata::new("retained.pdf", "application/pdf").unwrap();
        let row = crate::ingest_binary_into_with_capture_context(
            &store,
            &key,
            b"retained original bytes",
            "file",
            1,
            None,
            Some(&metadata),
            &Default::default(),
        )
        .unwrap()
        .into_item();
        store.set_pinned(&row.id, true).unwrap();
        assert_eq!(
            crate::file_transfer::upgrade_legacy(&store, &key.item_key()).unwrap(),
            1
        );
        let upgraded = store.get(&row.id).unwrap().unwrap();
        assert!(upgraded.pinned);
        assert!(upgraded.created_at > row.created_at);
        assert_eq!(upgraded.origin_device_id, row.origin_device_id);
        let payload = ClipboardPayload::open(&upgraded, &key.item_key()).unwrap();
        let ClipboardPayload::File {
            metadata: Some(metadata),
            ..
        } = payload
        else {
            panic!("expected file offer")
        };
        assert_eq!(metadata.filename, "retained.pdf");
        let destination = dir.path().join("upgraded.pdf");
        crate::file_transfer::save_local(
            &store,
            &key.item_key(),
            &row.id,
            &destination,
            u64::MAX,
            || false,
        )
        .unwrap();
        assert_eq!(
            std::fs::read(destination).unwrap(),
            b"retained original bytes"
        );
        assert_eq!(
            crate::file_transfer::upgrade_legacy(&store, &key.item_key()).unwrap(),
            0
        );
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
        store.file_transfer_blob_count()
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
            assert!(recovered.len() < 256);
            assert_ne!(&*recovered, bytes);
            let metadata = metadata.unwrap();
            assert_eq!(metadata.filename, name);
            assert_eq!(
                metadata.source_reference.as_deref(),
                Some("content://documents/original")
            );
            let saved = dir.path().join(name);
            crate::file_transfer::save_local(
                &store,
                &key.item_key(),
                &row.id,
                &saved,
                u64::MAX,
                || false,
            )
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
            let path = dir.path().join(format!("restored-{id}"));
            crate::file_transfer::save_local(&store, &key.item_key(), &id, &path, u64::MAX, || {
                false
            })
            .unwrap();
            assert_eq!(std::fs::read(path).unwrap(), b"original");
        }
    }

    #[test]
    fn synced_imports_remain_distinct_without_transferring_their_file_bodies() {
        let (store, key, dir) = fixture();
        let (peer, peer_key, _) = fixture();
        let path = dir.path().join("source");
        std::fs::write(&path, b"original").unwrap();
        for name in ["a.pdf", "b.pdf"] {
            let item = import(&store, &key, &path, name);
            let descriptor =
                crate::open_binary(&item.content_ciphertext, &key.item_key(), &item.id).unwrap();
            let incoming = crate::sync::RemoteVersion {
                item_id: &item.id,
                content: "",
                binary_content: Some(&descriptor),
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
            assert!(!peer.has_file_transfer(&row.id).unwrap());
            let payload = ClipboardPayload::open(&row, &peer_key.item_key()).unwrap();
            assert_eq!(payload.byte_len(), 8);
            assert_eq!(
                crate::file_transfer::offer(&payload).unwrap().sha256,
                crate::binary::content_hash(&crate::binary::content_digest(b"original"))
            );
        }
        assert_eq!(peer.count().unwrap(), 2);
        assert_eq!(blob_count(&peer), 0);
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
        assert_eq!(row.content_type, "file");
        let metadata =
            PayloadMetadata::from_json(row.payload_metadata.as_deref().unwrap(), &row.content_type)
                .unwrap();
        assert_eq!(metadata.file.unwrap().filename, "photo.jpg");
        let saved = dir.path().join("saved-photo.jpg");
        crate::file_transfer::save_local(
            &store,
            &key.item_key(),
            &row.id,
            &saved,
            u64::MAX,
            || false,
        )
        .unwrap();
        assert_eq!(&std::fs::read(saved).unwrap(), buffer.get_ref());
        let preview = crate::thumbnail_png(buffer.get_ref(), 16, Some(128), None).unwrap();
        assert_eq!((preview.width, preview.height), (3, 2));
    }

    #[test]
    fn accepts_empty_files_and_rejects_directories_oversize_and_bad_metadata() {
        let (store, key, dir) = fixture();
        let path = dir.path().join("empty");
        std::fs::write(&path, b"").unwrap();
        let row = import(&store, &key, &path, "empty.pdf");
        let destination = dir.path().join("saved-empty.pdf");
        crate::file_transfer::save_local(
            &store,
            &key.item_key(),
            &row.id,
            &destination,
            u64::MAX,
            || false,
        )
        .unwrap();
        assert!(std::fs::read(destination).unwrap().is_empty());
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
            Err(FileImportError::TooLarge { .. })
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
