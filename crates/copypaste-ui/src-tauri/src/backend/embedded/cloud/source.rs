use std::sync::{Arc, Mutex};

use copypaste_cloud::sync::{Applied, CloudSource, LocalItem, SyncError};
use copypaste_core::RemoteVersion;

use super::cursor::UploadFloor;
use super::{KEY_UPLOAD_FLOOR, KEY_UPLOAD_FLOOR_ITEM, KEY_WATERMARK, KEY_WATERMARK_ITEM};
use crate::backend::embedded::open::Inner;

const UPLOAD_SCAN_LIMIT: i64 = 500;

#[derive(Clone, Default)]
struct Offer {
    truncated: bool,
    last: UploadFloor,
    started: UploadFloor,
    started_epoch: u64,
}

pub(super) struct StoreSource {
    inner: Arc<Inner>,
    shared: copypaste_core::StoreSource,
    last_offer: Mutex<Offer>,
}

impl StoreSource {
    pub(super) fn new(inner: &Arc<Inner>) -> Self {
        Self {
            shared: copypaste_core::StoreSource::new(
                inner.state.store.clone(),
                Arc::clone(&inner.state.keyring),
                Arc::clone(&inner.state.detector),
                inner.state.device_id.clone(),
                inner.state.device_name(),
                inner.settings(),
            )
            .on_applied({
                let inner = Arc::downgrade(inner);
                move |created_at| {
                    if let Some(inner) = inner.upgrade() {
                        inner.note_cloud_version_applied(created_at);
                    }
                }
            }),
            inner: Arc::clone(inner),
            last_offer: Mutex::new(Offer::default()),
        }
    }

    pub(super) fn commit_upload_floor(&self, started_ms: i64) -> Result<(), SyncError> {
        let offer = self
            .last_offer
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        let candidate = if offer.truncated && offer.last.created_at <= started_ms {
            offer.last
        } else {
            UploadFloor {
                created_at: started_ms,
                item_id: None,
            }
        };
        self.inner
            .cloud
            .commit_upload_floor(&self.inner, &offer.started, offer.started_epoch, &candidate)
            .map_err(source_error)
    }
}

impl CloudSource for StoreSource {
    fn device_id(&self) -> String {
        self.inner.state.device_id.clone()
    }

    fn local_changes_since(&self, since_ms: i64) -> Result<Vec<LocalItem>, SyncError> {
        self.local_changes_after(since_ms, None)
    }

    fn local_changes_after(
        &self,
        since_ms: i64,
        after_item_id: Option<&str>,
    ) -> Result<Vec<LocalItem>, SyncError> {
        let started_epoch = self.inner.cloud.upload_floor_epoch();
        let rows = self
            .inner
            .state
            .store
            .versions_after(since_ms, after_item_id, UPLOAD_SCAN_LIMIT)
            .map_err(source_error)?;
        *self
            .last_offer
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Offer {
            truncated: rows.len() as i64 >= UPLOAD_SCAN_LIMIT,
            last: rows.last().map_or_else(
                || UploadFloor {
                    created_at: since_ms,
                    item_id: after_item_id.map(str::to_owned),
                },
                |row| UploadFloor {
                    created_at: row.created_at,
                    item_id: Some(row.id.clone()),
                },
            ),
            started: UploadFloor {
                created_at: since_ms,
                item_id: after_item_id.map(str::to_owned),
            },
            started_epoch,
        };
        let mut items = Vec::with_capacity(rows.len());
        for row in rows {
            let content = if row.deleted {
                Vec::new()
            } else if let Ok(bytes) = self.shared.open_bytes(&row) {
                bytes
            } else {
                continue;
            };
            items.push(LocalItem {
                origin_device_id: copypaste_core::origin_or(
                    &row.origin_device_id,
                    &self.inner.state.device_id,
                )
                .to_string(),
                item_id: row.id,
                content,
                content_type: row.content_type,
                payload_metadata: row.payload_metadata,
                created_at: row.created_at,
                deleted: row.deleted,
            });
        }
        Ok(items)
    }

    fn apply_remote(&self, item: LocalItem) -> Result<Applied, SyncError> {
        let text = copypaste_ipc::content_type::is_text(&item.content_type)
            .then(|| String::from_utf8_lossy(&item.content));
        let applied = self
            .shared
            .apply_version(&RemoteVersion {
                item_id: &item.item_id,
                content: text.as_deref().unwrap_or(""),
                binary_content: (!item.deleted
                    && !copypaste_ipc::content_type::is_text(&item.content_type))
                .then_some(item.content.as_slice()),
                payload_metadata: item.payload_metadata.as_deref(),
                content_type: &item.content_type,
                created_at: item.created_at,
                deleted: item.deleted,
                content_hash: None,
                origin_device_id: &item.origin_device_id,
            })
            .map_err(|_| SyncError::Source(copypaste_core::sync::MSG_STORE))?;
        Ok(if applied {
            Applied::Merged
        } else {
            Applied::Declined(item)
        })
    }

    fn watermark(&self) -> Result<i64, SyncError> {
        self.inner
            .state
            .store
            .state_ms(KEY_WATERMARK)
            .map_err(source_error)
    }
    fn watermark_item_id(&self) -> Result<Option<String>, SyncError> {
        self.inner
            .state
            .store
            .state(KEY_WATERMARK_ITEM)
            .map_err(source_error)
    }
    fn upload_floor(&self) -> Result<i64, SyncError> {
        self.inner
            .state
            .store
            .state_ms(KEY_UPLOAD_FLOOR)
            .map_err(source_error)
    }
    fn upload_floor_item_id(&self) -> Result<Option<String>, SyncError> {
        self.inner
            .state
            .store
            .state(KEY_UPLOAD_FLOOR_ITEM)
            .map_err(source_error)
    }

    fn set_watermark(&self, ms: i64) -> Result<(), SyncError> {
        self.inner
            .state
            .store
            .set_state_all(&[
                (KEY_WATERMARK, &ms.max(0).to_string()),
                (KEY_WATERMARK_ITEM, ""),
            ])
            .map_err(source_error)
    }

    fn set_watermark_keyset(&self, ms: i64, item_id: &str) -> Result<(), SyncError> {
        self.inner
            .state
            .store
            .set_state_all(&[
                (KEY_WATERMARK, &ms.max(0).to_string()),
                (KEY_WATERMARK_ITEM, item_id),
            ])
            .map_err(source_error)
    }

    fn requeue_local_winner(&self, incoming: &LocalItem) -> Result<bool, SyncError> {
        let text = copypaste_ipc::content_type::is_text(&incoming.content_type)
            .then(|| String::from_utf8_lossy(&incoming.content));
        let stamp = copypaste_core::local_winner_stamp(
            &self.inner.state.store,
            &self.inner.state.device_id,
            &RemoteVersion {
                item_id: &incoming.item_id,
                content: text.as_deref().unwrap_or(""),
                binary_content: (!incoming.deleted
                    && !copypaste_ipc::content_type::is_text(&incoming.content_type))
                .then_some(incoming.content.as_slice()),
                payload_metadata: incoming.payload_metadata.as_deref(),
                content_type: &incoming.content_type,
                created_at: incoming.created_at,
                deleted: incoming.deleted,
                content_hash: None,
                origin_device_id: &incoming.origin_device_id,
            },
        )
        .map_err(|_| SyncError::Source(copypaste_core::sync::MSG_STORE))?;
        if let Some(stamp) = stamp {
            self.inner.note_version_written(stamp);
            return Ok(true);
        }
        Ok(false)
    }
}

fn source_error(error: impl std::fmt::Debug) -> SyncError {
    tracing::warn!(?error, "embedded cloud source failed");
    SyncError::Source(copypaste_core::sync::MSG_STORE)
}

#[cfg(test)]
mod tests {
    use super::super::{KEY_UPLOAD_FLOOR, KEY_UPLOAD_FLOOR_ITEM};
    use super::*;
    use crate::backend::embedded::tests::backend;
    use crate::backend::Backend;
    use copypaste_cloud::sync::CloudSource;
    use copypaste_ipc::ExportItem;
    use copypaste_p2p::protocol::{content_hash, SyncItem};
    use copypaste_p2p::sync::SyncSource;

    fn set_floor(source: &StoreSource, created_at: i64, item_id: &str) {
        source
            .inner
            .state
            .store
            .set_state_all(&[
                (KEY_UPLOAD_FLOOR, &created_at.to_string()),
                (KEY_UPLOAD_FLOOR_ITEM, item_id),
            ])
            .unwrap();
    }

    #[tokio::test]
    async fn a_peer_version_below_the_cloud_floor_is_offered_to_cloud() {
        let (backend, _clipboard, _dir) = backend();
        let cloud = StoreSource::new(&backend.inner);
        set_floor(&cloud, 50_000, "z");
        let peer = crate::backend::embedded::peers::source(&backend.inner);
        let text = "from a peer";
        let applied = peer
            .apply(SyncItem {
                item_id: "peer-item".into(),
                content: text.into(),
                binary_content: Vec::new(),
                payload_metadata: None,
                content_type: copypaste_ipc::content_type::TEXT.into(),
                created_at: 1_000,
                deleted: false,
                content_hash: content_hash(text),
                origin_device_id: "peer-device".into(),
                pinned: false,
                pin_order: None,
                pin_updated_at: 0,
            })
            .unwrap();

        assert!(applied);
        assert_eq!(cloud.upload_floor().unwrap(), 1_000);
        assert_eq!(cloud.upload_floor_item_id().unwrap(), None);
        assert!(cloud
            .local_changes_after(1_000, None)
            .unwrap()
            .iter()
            .any(|item| item.item_id == "peer-item"));
    }

    #[tokio::test]
    async fn a_cloud_merge_lowers_every_existing_peer_cursor() {
        let (backend, _clipboard, _dir) = backend();
        let node = backend.node().await.unwrap();
        node.record_cursor("peer-a", 50_000);
        node.record_cursor("peer-b", 60_000);
        let cloud = StoreSource::new(&backend.inner);

        assert_eq!(
            cloud
                .apply_remote(LocalItem {
                    item_id: "cloud-item".into(),
                    content: b"from cloud".to_vec(),
                    content_type: copypaste_ipc::content_type::TEXT.into(),
                    payload_metadata: None,
                    created_at: 2_000,
                    deleted: false,
                    origin_device_id: "cloud-device".into(),
                })
                .unwrap(),
            Applied::Merged
        );

        assert_eq!(node.cursor("peer-a").relay_floor_ms, Some(2_000));
        assert_eq!(node.cursor("peer-b").relay_floor_ms, Some(2_000));
    }

    #[tokio::test]
    async fn old_import_and_delete_stamps_reopen_the_cloud_scan() {
        let (backend, _clipboard, _dir) = backend();
        let cloud = StoreSource::new(&backend.inner);
        set_floor(&cloud, 50_000, "z");
        backend
            .import(vec![ExportItem {
                content: "restored".into(),
                content_type: copypaste_ipc::content_type::TEXT.into(),
                created_at: 1_000,
                pinned: false,
                is_sensitive: false,
            }])
            .await
            .unwrap();
        assert_eq!(cloud.upload_floor().unwrap(), 1_000);

        let id = backend.list(10, None).await.unwrap().items[0].id.clone();
        let future_floor = copypaste_core::now_ms() + 60_000;
        set_floor(&cloud, future_floor, "z");
        backend.delete(&id).await.unwrap();

        assert!(cloud.upload_floor().unwrap() < future_floor);
        assert_eq!(cloud.upload_floor_item_id().unwrap(), None);
        assert!(cloud
            .local_changes_after(cloud.upload_floor().unwrap(), None)
            .unwrap()
            .iter()
            .any(|item| item.item_id == id && item.deleted));
    }

    #[tokio::test]
    async fn a_paused_scan_cannot_commit_over_a_concurrent_lowered_keyset() {
        let (backend, _clipboard, _dir) = backend();
        let cloud = StoreSource::new(&backend.inner);
        set_floor(&cloud, 5_000, "z");
        assert!(cloud
            .local_changes_after(5_000, Some("z"))
            .unwrap()
            .is_empty());

        backend.inner.note_version_written(1_000);
        cloud.commit_upload_floor(9_000).unwrap();

        assert_eq!(cloud.upload_floor().unwrap(), 1_000);
        assert_eq!(cloud.upload_floor_item_id().unwrap(), None);
    }

    #[tokio::test]
    async fn an_equal_timestamp_local_tombstone_is_republished_to_cloud() {
        let (backend, _clipboard, _dir) = backend();
        let cloud = StoreSource::new(&backend.inner);
        let peer = crate::backend::embedded::peers::source(&backend.inner);
        let text = "deleted locally";
        peer.apply(SyncItem {
            item_id: "same-stamp-delete".into(),
            content: text.into(),
            binary_content: Vec::new(),
            payload_metadata: None,
            content_type: copypaste_ipc::content_type::TEXT.into(),
            created_at: 1_000,
            deleted: false,
            content_hash: content_hash(text),
            origin_device_id: "peer-device".into(),
            pinned: false,
            pin_order: None,
            pin_updated_at: 0,
        })
        .unwrap();
        backend
            .inner
            .state
            .store
            .delete("same-stamp-delete")
            .unwrap();
        let tombstone = backend
            .inner
            .state
            .store
            .version("same-stamp-delete")
            .unwrap()
            .unwrap();
        set_floor(&cloud, tombstone.created_at + 50_000, "z");

        assert!(cloud
            .requeue_local_winner(&LocalItem {
                item_id: "same-stamp-delete".into(),
                content: text.as_bytes().to_vec(),
                content_type: copypaste_ipc::content_type::TEXT.into(),
                payload_metadata: None,
                created_at: tombstone.created_at,
                deleted: false,
                origin_device_id: "cloud-device".into(),
            })
            .unwrap());
        assert_eq!(cloud.upload_floor().unwrap(), tombstone.created_at);
        assert_eq!(cloud.upload_floor_item_id().unwrap(), None);
    }
}
