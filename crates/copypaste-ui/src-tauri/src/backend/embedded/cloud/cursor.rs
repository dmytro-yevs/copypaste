use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};

use copypaste_core::{Store, StoreError};

use super::{KEY_UPLOAD_FLOOR, KEY_UPLOAD_FLOOR_ITEM};

#[derive(Clone, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct UploadFloor {
    pub(super) created_at: i64,
    pub(super) item_id: Option<String>,
}

pub(super) struct UploadCursor {
    lock: Mutex<()>,
    epoch: AtomicU64,
}

impl UploadCursor {
    pub(super) fn new() -> Self {
        Self {
            lock: Mutex::new(()),
            epoch: AtomicU64::new(0),
        }
    }

    pub(super) fn epoch(&self) -> u64 {
        self.epoch.load(Ordering::Acquire)
    }

    pub(super) fn reset(&self, store: &Store) -> Result<(), StoreError> {
        self.epoch.fetch_add(1, Ordering::AcqRel);
        let _guard = self.guard();
        Self::store(store, &UploadFloor::default())
    }

    pub(super) fn note_version_written(&self, store: &Store, created_at: i64) {
        self.epoch.fetch_add(1, Ordering::AcqRel);
        let _guard = self.guard();
        let incoming = UploadFloor {
            created_at: created_at.max(0),
            item_id: None,
        };
        match Self::load(store) {
            Ok(current) if current <= incoming => {}
            Ok(_) => {
                if let Err(error) = Self::store(store, &incoming) {
                    tracing::warn!(?error, "could not lower the embedded cloud upload floor");
                }
            }
            Err(error) => tracing::warn!(?error, "could not read the embedded cloud upload floor"),
        }
    }

    pub(super) fn commit(
        &self,
        store: &Store,
        started: &UploadFloor,
        started_epoch: u64,
        candidate: &UploadFloor,
    ) -> Result<(), StoreError> {
        let _guard = self.guard();
        if self.epoch() != started_epoch {
            return Ok(());
        }
        let current = Self::load(store)?;
        if current < *started {
            return Ok(());
        }
        Self::store(store, &current.max(candidate.clone()))
    }

    fn load(store: &Store) -> Result<UploadFloor, StoreError> {
        Ok(UploadFloor {
            created_at: store.state_ms(KEY_UPLOAD_FLOOR)?,
            item_id: store.state(KEY_UPLOAD_FLOOR_ITEM)?,
        })
    }

    fn store(store: &Store, floor: &UploadFloor) -> Result<(), StoreError> {
        store.set_state_all(&[
            (KEY_UPLOAD_FLOOR, &floor.created_at.max(0).to_string()),
            (
                KEY_UPLOAD_FLOOR_ITEM,
                floor.item_id.as_deref().unwrap_or(""),
            ),
        ])
    }

    fn guard(&self) -> MutexGuard<'_, ()> {
        self.lock.lock().unwrap_or_else(|error| error.into_inner())
    }
}
