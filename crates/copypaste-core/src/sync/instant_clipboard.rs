//! Select received content for the local system clipboard, across transports.

use std::sync::Mutex;

use crate::{ClipboardPayload, ClipboardWriteError, Keyring, Store, StoredItem};

const APPLIED_STAMP: &str = "instant_clipboard_applied_stamp";

/// One serialization gate per platform host, shared by all receiving sources.
#[derive(Default)]
pub struct InstantClipboard(Mutex<()>);

impl InstantClipboard {
    /// Keep local clipboard recency after its History row is deleted or evicted.
    pub fn note_local(&self, store: &Store, stamp: i64) {
        let _guard = self.0.lock().unwrap_or_else(|error| error.into_inner());
        let Ok(previous) = store.state(APPLIED_STAMP) else {
            return;
        };
        if previous
            .as_deref()
            .and_then(|stamp| stamp.parse::<i64>().ok())
            .is_some_and(|previous| previous >= stamp)
        {
            return;
        }
        if store.set_state(APPLIED_STAMP, &stamp.to_string()).is_err() {
            tracing::warn!("could not persist the local clipboard stamp");
        }
    }

    /// Receiving history succeeds even when its native clipboard is unavailable.
    /// The current settings are read inside the gate, so disabling takes effect
    /// for every subsequent write. Never replace newer local or received content.
    pub fn apply(
        &self,
        store: &Store,
        keyring: &Keyring,
        row: &StoredItem,
        settings: impl FnOnce() -> copypaste_ipc::ConfigData,
        write: impl FnOnce(&ClipboardPayload) -> Result<(), ClipboardWriteError>,
    ) {
        let _guard = self.0.lock().unwrap_or_else(|error| error.into_inner());
        let config = settings();
        if !config.sync_enabled || !config.instant_clipboard || row.deleted {
            return;
        }
        let Ok(Some((newest_id, newest_stamp))) = store.newest_live_key() else {
            return;
        };
        if newest_id != row.id || newest_stamp != row.created_at {
            return;
        }
        let Ok(stamp) = store.state(APPLIED_STAMP) else {
            return;
        };
        if stamp
            .as_deref()
            .and_then(|stamp| stamp.parse::<i64>().ok())
            .is_some_and(|stamp| stamp >= row.created_at)
        {
            return;
        }
        let Ok(mut payload) = ClipboardPayload::open(row, &keyring.item_key()) else {
            return;
        };
        // A source path belongs to the sending device. Materialize its original
        // file bytes locally instead of copying an unusable foreign path.
        if let ClipboardPayload::File {
            metadata: Some(metadata),
            ..
        } = &mut payload
        {
            metadata.source_reference = None;
        }
        match write(&payload) {
            Ok(()) => {
                if store
                    .set_state(APPLIED_STAMP, &row.created_at.to_string())
                    .is_err()
                {
                    tracing::warn!("could not persist the instant clipboard stamp");
                }
            }
            Err(_) => tracing::warn!("could not apply a received clip to the system clipboard"),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use copypaste_ipc::ConfigData;
    use copypaste_p2p::protocol::SyncItem;
    use copypaste_p2p::sync::SyncSource;

    use super::*;
    use crate::sync::testkit::{fixture, version};
    use crate::sync::{RemoteVersion, StoreSource};

    type Writes = Arc<Mutex<Vec<(String, Vec<u8>, Option<String>)>>>;

    fn receiving_source(
        f: &crate::sync::testkit::Fixture,
        settings: Arc<Mutex<ConfigData>>,
        writes: Writes,
    ) -> StoreSource {
        let gate = InstantClipboard::default();
        let store = f.store.clone();
        let keyring = f.keyring.clone();
        let source_settings = settings.clone();
        StoreSource::with_retention_settings(
            store.clone(),
            keyring.clone(),
            f.here.clone(),
            "receiver".into(),
            move || source_settings.lock().unwrap().clone(),
        )
        .on_clip_received(move |row| {
            gate.apply(
                &store,
                &keyring,
                row,
                || settings.lock().unwrap().clone(),
                |payload| {
                    let written = match payload {
                        ClipboardPayload::Text(text) => {
                            ("text".into(), text.as_bytes().to_vec(), None)
                        }
                        ClipboardPayload::Image {
                            content_type,
                            bytes,
                        } => (content_type.clone(), bytes.to_vec(), None),
                        ClipboardPayload::File { bytes, metadata } => (
                            "file".into(),
                            bytes.to_vec(),
                            metadata.as_ref().and_then(|m| m.source_reference.clone()),
                        ),
                        ClipboardPayload::Unsupported { .. } => {
                            return Err(ClipboardWriteError::UnsupportedContent);
                        }
                    };
                    writes.lock().unwrap().push(written);
                    Ok(())
                },
            );
        })
    }

    #[test]
    fn catch_up_selects_only_the_newest_clip_and_preserves_newer_local_content() {
        let f = fixture();
        let writes = Writes::default();
        let source = receiving_source(
            &f,
            Arc::new(Mutex::new(ConfigData::default())),
            writes.clone(),
        );
        let page = [version("new", "newest", 300), version("old", "older", 100)];
        assert_eq!(source.apply_versions(&page).unwrap(), [true, true]);
        assert_eq!(writes.lock().unwrap().len(), 1);
        assert_eq!(writes.lock().unwrap()[0].1, b"newest");
        assert_eq!(source.apply_versions(&page).unwrap(), [false, false]);
        let mut local = version("local", "local clipboard", 500);
        local.origin_device_id = &f.here;
        source.apply_version(&local).unwrap();
        InstantClipboard::default().note_local(&f.store, 500);
        f.store.delete("local").unwrap();
        source
            .apply_version(&version("missed", "missed remote", 400))
            .unwrap();
        source
            .apply_version(&version("equal", "same timestamp", 500))
            .unwrap();
        assert_eq!(writes.lock().unwrap().len(), 1);
        source
            .apply_version(&version("live", "new remote", 600))
            .unwrap();
        assert_eq!(writes.lock().unwrap().len(), 2);
    }

    #[test]
    fn the_live_switch_only_disables_clipboard_writes_and_keeps_history_syncing() {
        let f = fixture();
        let writes = Writes::default();
        let settings = Arc::new(Mutex::new(ConfigData::default()));
        let source = receiving_source(&f, settings.clone(), writes.clone());
        settings.lock().unwrap().instant_clipboard = false;
        assert!(source
            .apply_version(&version("off", "still synced", 100))
            .unwrap());
        assert!(f.store.get("off").unwrap().is_some());
        assert!(writes.lock().unwrap().is_empty());
        settings.lock().unwrap().instant_clipboard = true;
        source.apply_version(&version("on", "copied", 200)).unwrap();
        assert_eq!(writes.lock().unwrap().len(), 1);
        let mut deleted = version("on", "", 300);
        deleted.deleted = true;
        source.apply_version(&deleted).unwrap();
        assert_eq!(writes.lock().unwrap().len(), 1);
    }

    #[test]
    fn peer_and_cloud_receives_write_original_text_image_and_file_bytes() {
        let f = fixture();
        let writes = Writes::default();
        let source = receiving_source(
            &f,
            Arc::new(Mutex::new(ConfigData::default())),
            writes.clone(),
        );
        let text = "peer text";
        let peer = SyncItem {
            item_id: "peer-text".into(),
            content: text.into(),
            binary_content: vec![],
            payload_metadata: None,
            content_type: "text".into(),
            created_at: 100,
            deleted: false,
            content_hash: crate::compute_content_hash(text.as_bytes()),
            origin_device_id: "sender".into(),
            source_app_bundle_id: None,
            source_app_name: None,
            pinned: false,
            pin_order: None,
            pin_updated_at: 0,
        };
        assert!(source.apply(peer.clone()).unwrap());
        let mut pin = peer;
        pin.pinned = true;
        pin.pin_updated_at = 150;
        assert!(source.apply(pin).unwrap());
        assert_eq!(
            writes.lock().unwrap().len(),
            1,
            "pinning must not rewrite the clipboard"
        );

        let image_bytes = b"original image bytes";
        let image_id = crate::binary_item_id(image_bytes);
        source
            .apply_versions(&[RemoteVersion {
                item_id: &image_id,
                content_type: "image/png",
                binary_content: Some(image_bytes),
                ..version("", "", 200)
            }])
            .unwrap();
        let file_bytes = b"original file bytes";
        let file_id = crate::binary_item_id(file_bytes);
        let metadata = crate::PayloadMetadata::new(
            Some(
                crate::FileMetadata::with_source_reference(
                    "document.pdf",
                    "application/pdf",
                    "content://sender/document",
                )
                .unwrap(),
            ),
            None,
        )
        .unwrap()
        .to_json("file")
        .unwrap();
        source
            .apply_versions(&[RemoteVersion {
                item_id: &file_id,
                content_type: "file",
                binary_content: Some(file_bytes),
                payload_metadata: Some(&metadata),
                ..version("", "", 300)
            }])
            .unwrap();
        let written = writes.lock().unwrap();
        assert_eq!(written[0].1, text.as_bytes());
        assert_eq!(written[1].1, image_bytes);
        assert_eq!(written[2].1, file_bytes);
        assert!(
            written[2].2.is_none(),
            "a foreign source reference must be materialized locally"
        );
    }
}
