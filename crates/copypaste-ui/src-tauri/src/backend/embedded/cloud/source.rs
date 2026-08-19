//! The app's history, as a cloud round sees it.
//!
//! Everything about *rows* is [`copypaste_cloud::sync::StoreView`], shared with
//! the daemon. What is here is what only the app knows: that a round belongs to
//! one account **and** to one cancellation token, and where device state lives.

use std::sync::{Arc, Weak};

use copypaste_cloud::sync::{
    floor_after_round, Applied, CloudSource, LocalItem, StoreView, SyncError, UnreadableUploads,
};

use super::cursor::UploadFloor;
use super::{
    Driver, KEY_UNREADABLE_UPLOADS, KEY_UPLOAD_FLOOR, KEY_UPLOAD_FLOOR_ITEM, KEY_WATERMARK,
    KEY_WATERMARK_ITEM,
};
use crate::backend::embedded::open::Inner;

struct Round {
    driver: Weak<Driver>,
    cancel: tokio_util::sync::CancellationToken,
}

pub(super) struct StoreSource {
    inner: Arc<Inner>,
    view: StoreView,
    round: Option<Round>,
}

impl StoreSource {
    pub(super) fn new(inner: &Arc<Inner>) -> Self {
        let shared = copypaste_core::StoreSource::new(
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
        });
        Self {
            view: StoreView::new(shared, inner.state.device_id.clone()),
            inner: Arc::clone(inner),
            round: None,
        }
    }

    pub(super) fn for_round(
        inner: &Arc<Inner>,
        driver: &Arc<super::Driver>,
        cancel: &tokio_util::sync::CancellationToken,
    ) -> Self {
        let mut source = Self::new(inner);
        source.round = Some(Round {
            driver: Arc::downgrade(driver),
            cancel: cancel.clone(),
        });
        source
    }

    fn while_active<T>(&self, f: impl FnOnce() -> Result<T, SyncError>) -> Result<T, SyncError> {
        let Some(round) = self.round.as_ref() else {
            return f();
        };
        let Some(expected) = round.driver.upgrade() else {
            return Err(cancelled());
        };
        self.inner
            .cloud
            .with_driver(&expected, &round.cancel, f)
            .unwrap_or_else(|| Err(cancelled()))
    }

    pub(super) fn commit_upload_floor(&self, started_ms: i64) -> Result<(), SyncError> {
        let offer = self.view.offer();
        let candidate = floor_after_round(&offer, started_ms);
        self.while_active(|| {
            self.inner
                .cloud
                .commit_upload_floor(
                    &self.inner,
                    &UploadFloor {
                        created_at: offer.started.created_at,
                        item_id: offer.started.item_id.clone(),
                    },
                    offer.started_epoch,
                    &UploadFloor {
                        created_at: candidate.created_at,
                        item_id: candidate.item_id.clone(),
                    },
                )
                .map_err(source_error)
        })
    }
}

impl CloudSource for StoreSource {
    fn device_id(&self) -> String {
        self.view.device_id().to_string()
    }

    fn local_changes_since(&self, since_ms: i64) -> Result<Vec<LocalItem>, SyncError> {
        self.local_changes_after(since_ms, None)
    }

    fn local_changes_after(
        &self,
        since_ms: i64,
        after_item_id: Option<&str>,
    ) -> Result<Vec<LocalItem>, SyncError> {
        self.while_active(|| {
            let started_epoch = self.inner.cloud.upload_floor_epoch();
            let previous = UnreadableUploads::decode(
                self.inner
                    .state
                    .store
                    .state(KEY_UNREADABLE_UPLOADS)
                    .map_err(source_error)?
                    .as_deref(),
            );
            if previous.reset_floor {
                self.inner.note_version_written(0);
            }
            let scan = self.view.scan(
                &self.inner.state.store,
                since_ms,
                after_item_id,
                started_epoch,
                &previous,
            )?;
            if scan.unreadable != previous {
                self.inner
                    .state
                    .store
                    .set_state(KEY_UNREADABLE_UPLOADS, &scan.unreadable.encode())
                    .map_err(source_error)?;
            }
            self.inner
                .cloud
                .note_unreadable_uploads(scan.unreadable.total);
            Ok(scan.items)
        })
    }

    fn apply_remote(&self, item: LocalItem) -> Result<Applied, SyncError> {
        self.apply_remote_batch(vec![item])?
            .pop()
            .ok_or(SyncError::Source(copypaste_core::sync::MSG_STORE))
    }

    /// One page, one round check, one write transaction.
    fn apply_remote_batch(&self, items: Vec<LocalItem>) -> Result<Vec<Applied>, SyncError> {
        self.while_active(|| self.view.apply_page(items))
    }

    fn watermark(&self) -> Result<i64, SyncError> {
        self.while_active(|| {
            self.inner
                .state
                .store
                .state_ms(KEY_WATERMARK)
                .map_err(source_error)
        })
    }
    fn watermark_item_id(&self) -> Result<Option<String>, SyncError> {
        self.while_active(|| {
            self.inner
                .state
                .store
                .state(KEY_WATERMARK_ITEM)
                .map_err(source_error)
        })
    }
    fn upload_floor(&self) -> Result<i64, SyncError> {
        self.while_active(|| {
            self.inner
                .state
                .store
                .state_ms(KEY_UPLOAD_FLOOR)
                .map_err(source_error)
        })
    }
    fn upload_floor_item_id(&self) -> Result<Option<String>, SyncError> {
        self.while_active(|| {
            self.inner
                .state
                .store
                .state(KEY_UPLOAD_FLOOR_ITEM)
                .map_err(source_error)
        })
    }

    fn set_watermark(&self, ms: i64) -> Result<(), SyncError> {
        self.while_active(|| {
            self.inner
                .state
                .store
                .set_state_all(&[
                    (KEY_WATERMARK, &ms.max(0).to_string()),
                    (KEY_WATERMARK_ITEM, ""),
                ])
                .map_err(source_error)
        })
    }

    fn set_watermark_keyset(&self, ms: i64, item_id: &str) -> Result<(), SyncError> {
        self.while_active(|| {
            self.inner
                .state
                .store
                .set_state_all(&[
                    (KEY_WATERMARK, &ms.max(0).to_string()),
                    (KEY_WATERMARK_ITEM, item_id),
                ])
                .map_err(source_error)
        })
    }

    fn requeue_local_winner(&self, incoming: &LocalItem) -> Result<bool, SyncError> {
        let stamp = self.while_active(|| self.view.requeue_stamp(incoming))?;
        if let Some(stamp) = stamp {
            self.inner.note_version_written(stamp);
            return Ok(true);
        }
        Ok(false)
    }
}

fn cancelled() -> SyncError {
    SyncError::Source("the cloud account changed during this round")
}

fn source_error(error: impl std::fmt::Debug) -> SyncError {
    tracing::warn!(?error, "embedded cloud source failed");
    SyncError::Source(copypaste_core::sync::MSG_STORE)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::super::{KEY_UPLOAD_FLOOR, KEY_UPLOAD_FLOOR_ITEM};
    use super::*;
    use crate::backend::embedded::tests::backend;
    use crate::backend::Backend;
    use copypaste_cloud::auth::{Session, SupabaseAuth};
    use copypaste_cloud::rest::SupabaseRest;
    use copypaste_cloud::sync::{CloudSource, CloudSync, SensitiveGuard};
    use copypaste_cloud::{CloudConfig, SyncKey};
    use copypaste_ipc::ExportItem;
    use copypaste_p2p::protocol::{content_hash, SyncItem};
    use copypaste_p2p::sync::SyncSource;
    use tokio_util::sync::CancellationToken;

    fn driver(token: &str) -> Arc<super::super::Driver> {
        let config = CloudConfig::new("https://example.invalid", "public-anon").unwrap();
        Arc::new(CloudSync::new(
            SupabaseRest::new(config.clone()),
            SupabaseAuth::new(config.clone()),
            SyncKey::from_bytes([9; 32]),
            config,
            Session {
                access_token: token.into(),
                refresh_token: format!("refresh-{token}"),
                user_id: "user-1".into(),
                expires_at_ms: 123_000,
            },
            SensitiveGuard::new(|_| false),
        ))
    }

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
                    content: zeroize::Zeroizing::new(b"from cloud".to_vec()),
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
                content: zeroize::Zeroizing::new(text.as_bytes().to_vec()),
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

    #[tokio::test]
    async fn a_replaced_accounts_source_cannot_apply_or_advance() {
        let (backend, _clipboard, _dir) = backend();
        let stale = driver("stale");
        backend
            .inner
            .cloud
            .account
            .install(super::super::account::Account {
                email: "old@example.com".into(),
                user_id: "user-1".into(),
                driver: Arc::clone(&stale),
                cancel: CancellationToken::new(),
            });
        let cancel = backend.inner.cloud.account.round().unwrap().1;
        let source = StoreSource::for_round(&backend.inner, &stale, &cancel);
        backend
            .inner
            .cloud
            .account
            .install(super::super::account::Account {
                email: "new@example.com".into(),
                user_id: "user-2".into(),
                driver: driver("current"),
                cancel: CancellationToken::new(),
            });

        assert!(source.set_watermark_keyset(9_000, "z").is_err());
        assert!(source
            .apply_remote(LocalItem {
                item_id: "old-account-item".into(),
                content: zeroize::Zeroizing::new(b"must not land".to_vec()),
                content_type: copypaste_ipc::content_type::TEXT.into(),
                payload_metadata: None,
                created_at: 9_000,
                deleted: false,
                origin_device_id: "old-device".into(),
            })
            .is_err());

        assert_eq!(source.inner.state.store.state_ms(KEY_WATERMARK).unwrap(), 0);
        assert!(source
            .inner
            .state
            .store
            .version("old-account-item")
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn a_paused_accounts_old_source_cannot_advance() {
        let (backend, _clipboard, _dir) = backend();
        let current = driver("current");
        backend
            .inner
            .cloud
            .account
            .install(super::super::account::Account {
                email: "a@example.com".into(),
                user_id: "user-1".into(),
                driver: Arc::clone(&current),
                cancel: CancellationToken::new(),
            });
        let cancel = backend.inner.cloud.account.round().unwrap().1;
        let source = StoreSource::for_round(&backend.inner, &current, &cancel);

        backend.inner.cloud.account.interrupt();

        assert!(cancel.is_cancelled());
        assert!(source.set_watermark_keyset(9_000, "z").is_err());
        assert_eq!(source.inner.state.store.state_ms(KEY_WATERMARK).unwrap(), 0);
    }
}
