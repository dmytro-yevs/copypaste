use std::sync::{Arc, Mutex};

use copypaste_cloud::sync::{Applied, CloudSource, LocalItem, SyncError};
use copypaste_core::RemoteVersion;

use super::{KEY_UPLOAD_FLOOR, KEY_UPLOAD_FLOOR_ITEM, KEY_WATERMARK, KEY_WATERMARK_ITEM};
use crate::backend::embedded::open::Inner;

const UPLOAD_SCAN_LIMIT: i64 = 500;

#[derive(Clone, Default)]
struct Offer {
    truncated: bool,
    last_ms: i64,
    last_id: Option<String>,
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
            ),
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
        let (ms, id) = if offer.truncated && offer.last_ms <= started_ms {
            (offer.last_ms, offer.last_id.as_deref().unwrap_or(""))
        } else {
            (started_ms, "")
        };
        self.inner
            .state
            .store
            .set_state_all(&[
                (KEY_UPLOAD_FLOOR, &ms.max(0).to_string()),
                (KEY_UPLOAD_FLOOR_ITEM, id),
            ])
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
            last_ms: rows.last().map_or(since_ms, |row| row.created_at),
            last_id: rows
                .last()
                .map(|row| row.id.clone())
                .or_else(|| after_item_id.map(str::to_owned)),
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
        let Some(local) = self
            .inner
            .state
            .store
            .version(&incoming.item_id)
            .map_err(source_error)?
        else {
            return Ok(false);
        };
        if local.created_at > incoming.created_at {
            let floor = self.upload_floor()?;
            if local.created_at < floor {
                self.inner
                    .state
                    .store
                    .set_state_all(&[
                        (KEY_UPLOAD_FLOOR, &local.created_at.to_string()),
                        (KEY_UPLOAD_FLOOR_ITEM, ""),
                    ])
                    .map_err(source_error)?;
            }
            return Ok(true);
        }
        Ok(false)
    }
}

fn source_error(error: impl std::fmt::Debug) -> SyncError {
    tracing::warn!(?error, "embedded cloud source failed");
    SyncError::Source(copypaste_core::sync::MSG_STORE)
}
