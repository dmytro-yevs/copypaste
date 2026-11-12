//! Application-owned encrypted state and history services for sync modules.
use crate::ModuleError;
use copypaste_core::Store;
use copypaste_sync::{
    account_state::SIGN_OUT_KEYS,
    host::{SyncHostRequest, UNREADABLE_UPLOADS, UPLOAD_FLOOR, UPLOAD_FLOOR_ITEM},
    store::{floor_after_round, Offer, StoreView},
    unreadable::{UnreadableUploads, UploadFloor},
    Applied,
};
use serde_json::Value;
use std::{collections::BTreeMap, sync::Mutex};

pub struct SyncServices {
    group: Option<GroupServices>,
    store: Store,
    view: StoreView,
    enabled: Box<dyn Fn() -> bool + Send + Sync>,
    remote_change: Box<dyn Fn(i64) + Send + Sync>,
    progress: Mutex<BTreeMap<String, Progress>>,
}
type ApplyGroup = dyn Fn(&str, &str) -> Result<(), String> + Send + Sync;
struct GroupServices {
    revision: Box<dyn Fn() -> u64 + Send + Sync>,
    snapshot: Box<dyn Fn() -> Result<Option<String>, String> + Send + Sync>,
    apply: Box<ApplyGroup>,
}
#[derive(Default)]
struct Progress {
    epoch: u64,
    offer: Option<Offer>,
}
impl SyncServices {
    pub fn new(
        store: Store,
        view: StoreView,
        enabled: impl Fn() -> bool + Send + Sync + 'static,
        remote_change: impl Fn(i64) + Send + Sync + 'static,
    ) -> Self {
        Self {
            group: None,
            store,
            view,
            enabled: Box::new(enabled),
            remote_change: Box::new(remote_change),
            progress: Mutex::default(),
        }
    }
    pub fn with_group(
        mut self,
        snapshot: impl Fn() -> Result<Option<String>, String> + Send + Sync + 'static,
        apply: impl Fn(&str, &str) -> Result<(), String> + Send + Sync + 'static,
        revision: impl Fn() -> u64 + Send + Sync + 'static,
    ) -> Self {
        self.group = Some(GroupServices {
            snapshot: Box::new(snapshot),
            apply: Box::new(apply),
            revision: Box::new(revision),
        });
        self
    }
    pub fn enabled(&self) -> bool {
        (self.enabled)()
    }
    fn key(id: &str, key: &str) -> Result<String, ModuleError> {
        if key.is_empty()
            || key.len() > 96
            || !key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
        {
            return Err(ModuleError::Invalid("Invalid sync state key.".into()));
        }
        Ok(format!("module:{id}:{key}"))
    }
    fn floor(&self, id: &str) -> Result<UploadFloor, ModuleError> {
        Ok(UploadFloor {
            created_at: self
                .store
                .state_ms(&Self::key(id, UPLOAD_FLOOR)?)
                .map_err(|_| ModuleError::State)?,
            item_id: self
                .store
                .state(&Self::key(id, UPLOAD_FLOOR_ITEM)?)
                .map_err(|_| ModuleError::State)?,
        })
    }
    fn set_floor(&self, id: &str, floor: &UploadFloor) -> Result<(), ModuleError> {
        self.store
            .set_state_all(&[
                (&Self::key(id, UPLOAD_FLOOR)?, &floor.created_at.to_string()),
                (
                    &Self::key(id, UPLOAD_FLOOR_ITEM)?,
                    floor.item_id.as_deref().unwrap_or(""),
                ),
            ])
            .map_err(|_| ModuleError::State)
    }
    /// Preserve old-stamped local and peer writes while a provider is offline.
    pub fn note_version(&self, id: &str, stamp: i64) -> Result<(), ModuleError> {
        let mut progress = self.progress.lock().map_err(|_| ModuleError::State)?;
        let entry = progress.entry(id.into()).or_default();
        entry.epoch = entry.epoch.wrapping_add(1);
        let floor = self.floor(id)?;
        if stamp.max(0) <= floor.created_at {
            self.set_floor(
                id,
                &UploadFloor {
                    created_at: stamp.max(0),
                    item_id: None,
                },
            )?;
        }
        Ok(())
    }
    pub(crate) fn note_versions(&self, stamp: i64) -> Result<(), ModuleError> {
        for id in self
            .store
            .sync_provider_ids()
            .map_err(|_| ModuleError::State)?
        {
            self.note_version(&id, stamp)?;
        }
        Ok(())
    }
    /// Offers rows to every cloud provider again although its upload floor
    /// has passed them: they join the retry set the next scan reads first.
    pub fn reoffer(&self, item_ids: &[String]) -> Result<(), ModuleError> {
        if item_ids.is_empty() {
            return Ok(());
        }
        let _progress = self.progress.lock().map_err(|_| ModuleError::State)?;
        for id in self
            .store
            .sync_provider_ids()
            .map_err(|_| ModuleError::State)?
        {
            let key = Self::key(&id, UNREADABLE_UPLOADS)?;
            let previous = self.store.state(&key).map_err(|_| ModuleError::State)?;
            let mut retry = UnreadableUploads::decode(previous.as_deref());
            if retry.reset_floor {
                // The next scan starts from the beginning and reaches them.
                continue;
            }
            retry.ids.extend(item_ids.iter().cloned());
            self.store
                .set_state(&key, &retry.encode())
                .map_err(|_| ModuleError::State)?;
        }
        Ok(())
    }
    pub(crate) fn clear(&self, id: &str) -> Result<(), ModuleError> {
        self.store
            .clear_module_state(id)
            .map_err(|_| ModuleError::State)?;
        self.store
            .clear_semantic_index(id)
            .map_err(|_| ModuleError::State)?;
        self.progress
            .lock()
            .map_err(|_| ModuleError::State)?
            .remove(id);
        Ok(())
    }

    /// Clears only the provider-defined signed-out account state. Provider
    /// availability must not retain credentials after the user signs out.
    pub(crate) fn clear_cloud_credentials(&self, id: &str) -> Result<(), ModuleError> {
        let keys = SIGN_OUT_KEYS
            .iter()
            .map(|key| Self::key(id, key.as_str()))
            .collect::<Result<Vec<_>, _>>()?;
        self.store
            .clear_state(&keys.iter().map(String::as_str).collect::<Vec<_>>())
            .map_err(|_| ModuleError::State)
    }
    /// Results stay a JSON tree because its sorted object keys are the exact
    /// reply bytes installed modules already receive.
    pub(crate) fn request(&self, id: &str, request: SyncHostRequest) -> Result<Value, ModuleError> {
        match request {
            SyncHostRequest::GroupSnapshot => {
                self.require_enabled()?;
                let snapshot = self
                    .group
                    .as_ref()
                    .map(|group| (group.snapshot)())
                    .transpose()
                    .map_err(|_| ModuleError::State)?
                    .flatten();
                serde_json::to_value(snapshot).map_err(|_| ModuleError::State)
            }
            SyncHostRequest::GroupApply {
                device_id,
                snapshot,
            } => {
                self.require_enabled()?;
                if device_id.len() > 128 || snapshot.len() > 768 * 1024 {
                    return Err(ModuleError::State);
                }
                let group = self.group.as_ref().ok_or(ModuleError::State)?;
                (group.apply)(&device_id, &snapshot).map_err(|_| ModuleError::State)?;
                Ok(Value::Null)
            }
            SyncHostRequest::Active => Ok(Value::Null),
            SyncHostRequest::DeviceId => Ok(Value::String(self.view.device_id().into())),
            SyncHostRequest::SyncEnabled => Ok(Value::Bool(self.enabled())),
            SyncHostRequest::Revision => Ok(serde_json::json!(self
                .progress
                .lock()
                .map_err(|_| ModuleError::State)?
                .get(id)
                .map_or(0, |entry| entry.epoch)
                .wrapping_add(self.group.as_ref().map_or(0, |group| (group.revision)())))),
            SyncHostRequest::ReadState { keys } => {
                if keys.len() > 64 {
                    return Err(ModuleError::State);
                }
                let values = keys
                    .into_iter()
                    .map(|key| {
                        let value = self
                            .store
                            .state(&Self::key(id, &key)?)
                            .map_err(|_| ModuleError::State)?;
                        Ok((key, value))
                    })
                    .collect::<Result<BTreeMap<_, _>, ModuleError>>()?;
                serde_json::to_value(values).map_err(|_| ModuleError::State)
            }
            SyncHostRequest::WriteState { values } => {
                if values.len() > 64 || values.values().any(|v| v.len() > 128 * 1024) {
                    return Err(ModuleError::State);
                }
                let values = values
                    .into_iter()
                    .map(|(key, value)| Ok((Self::key(id, &key)?, value)))
                    .collect::<Result<Vec<_>, ModuleError>>()?;
                let _progress = self.progress.lock().map_err(|_| ModuleError::State)?;
                self.store
                    .set_state_all(
                        &values
                            .iter()
                            .map(|(k, v)| (k.as_str(), v.as_str()))
                            .collect::<Vec<_>>(),
                    )
                    .map_err(|_| ModuleError::State)?;
                Ok(Value::Null)
            }
            SyncHostRequest::ClearState { keys } => {
                if keys.len() > 64 {
                    return Err(ModuleError::State);
                }
                let keys = keys
                    .iter()
                    .map(|key| Self::key(id, key))
                    .collect::<Result<Vec<_>, _>>()?;
                self.store
                    .clear_state(&keys.iter().map(String::as_str).collect::<Vec<_>>())
                    .map_err(|_| ModuleError::State)?;
                Ok(Value::Null)
            }
            SyncHostRequest::Scan {
                since_ms,
                after_item_id,
            } => {
                self.require_enabled()?;
                let mut progress = self.progress.lock().map_err(|_| ModuleError::State)?;
                let entry = progress.entry(id.into()).or_default();
                let previous = self
                    .store
                    .state(&Self::key(id, UNREADABLE_UPLOADS)?)
                    .map_err(|_| ModuleError::State)?;
                let unreadable = UnreadableUploads::decode(previous.as_deref());
                if unreadable.reset_floor {
                    self.set_floor(id, &UploadFloor::default())?;
                    entry.epoch = entry.epoch.wrapping_add(1);
                }
                let scan = self
                    .view
                    .scan_bounded(
                        &self.store,
                        since_ms,
                        after_item_id.as_deref(),
                        entry.epoch,
                        &unreadable,
                        8 * 1024 * 1024,
                    )
                    .map_err(|_| ModuleError::State)?;
                self.store
                    .set_state(
                        &Self::key(id, UNREADABLE_UPLOADS)?,
                        &scan.unreadable.encode(),
                    )
                    .map_err(|_| ModuleError::State)?;
                entry.offer = Some(scan.offer.clone());
                serde_json::to_value(scan).map_err(|_| ModuleError::State)
            }
            SyncHostRequest::Apply { items } => {
                self.require_enabled()?;
                if items.len() > 500
                    || items
                        .iter()
                        .any(|item| item.content.len() > copypaste_ipc::MAX_CONTENT_BYTES)
                {
                    return Err(ModuleError::State);
                }
                let stamp = items.iter().map(|item| item.created_at).min().unwrap_or(0);
                let applied = self
                    .view
                    .apply_page(items)
                    .map_err(|_| ModuleError::State)?;
                if applied.iter().any(|item| matches!(item, Applied::Merged)) {
                    (self.remote_change)(stamp);
                }
                serde_json::to_value(applied).map_err(|_| ModuleError::State)
            }
            SyncHostRequest::Requeue { incoming } => {
                self.require_enabled()?;
                let stamp = self
                    .view
                    .requeue_stamp(&incoming)
                    .map_err(|_| ModuleError::State)?;
                if let Some(stamp) = stamp {
                    self.note_version(id, stamp)?;
                }
                Ok(Value::Bool(stamp.is_some()))
            }
            SyncHostRequest::CommitUpload { started_ms } => {
                self.require_enabled()?;
                let progress = self.progress.lock().map_err(|_| ModuleError::State)?;
                let Some(entry) = progress.get(id) else {
                    return Err(ModuleError::State);
                };
                let Some(offer) = &entry.offer else {
                    return Err(ModuleError::State);
                };
                let current = self.floor(id)?;
                if offer.started_epoch == entry.epoch && current >= offer.started {
                    self.set_floor(id, &current.max(floor_after_round(offer, started_ms)))?;
                }
                Ok(Value::Null)
            }
        }
    }
    fn require_enabled(&self) -> Result<(), ModuleError> {
        if self.enabled() {
            Ok(())
        } else {
            Err(ModuleError::Disabled)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use copypaste_core::{compute_content_hash, Keyring, NewItem, StoreSource};
    fn service() -> SyncServices {
        device(5, "device").0
    }
    fn device(secret: u8, id: &str) -> (SyncServices, Arc<Keyring>) {
        let keyring = Arc::new(Keyring::from_secret(&[secret; 32]));
        let store = Store::open_in_memory(&keyring.db_key()).unwrap();
        let source = StoreSource::new(
            store.clone(),
            Arc::clone(&keyring),
            id.into(),
            "test".into(),
            Default::default(),
        );
        let services = SyncServices::new(store, StoreView::new(source, id.into()), || true, |_| {});
        (services, keyring)
    }
    use std::sync::Arc;
    #[test]
    fn late_same_millisecond_writes_fence_upload_completion() {
        let service = service();
        service
            .set_floor(
                "copypaste.supabase",
                &UploadFloor {
                    created_at: 1000,
                    item_id: Some("zz".into()),
                },
            )
            .unwrap();
        service
            .request(
                "copypaste.supabase",
                SyncHostRequest::Scan {
                    since_ms: 1000,
                    after_item_id: Some("zz".into()),
                },
            )
            .unwrap();
        service.note_version("copypaste.supabase", 1000).unwrap();
        service
            .request(
                "copypaste.supabase",
                SyncHostRequest::CommitUpload { started_ms: 2000 },
            )
            .unwrap();
        assert_eq!(
            service.floor("copypaste.supabase").unwrap(),
            UploadFloor {
                created_at: 1000,
                item_id: None
            }
        );
    }
    #[test]
    fn encrypted_state_is_namespaced_and_removal_preserves_other_owners() {
        let service = service();
        for id in ["copypaste.supabase", "copypaste.other"] {
            service
                .request(
                    id,
                    SyncHostRequest::WriteState {
                        values: BTreeMap::from([("refresh_token".into(), "secret".into())]),
                    },
                )
                .unwrap();
        }
        service.clear("copypaste.supabase").unwrap();
        assert_eq!(
            service
                .store
                .state("module:copypaste.supabase:refresh_token")
                .unwrap(),
            None
        );
        assert_eq!(
            service
                .store
                .state("module:copypaste.other:refresh_token")
                .unwrap(),
            Some("secret".into())
        );
        assert!(service
            .request(
                "copypaste.supabase",
                SyncHostRequest::ReadState {
                    keys: vec!["../other".into()]
                }
            )
            .is_err());
    }

    #[test]
    fn removal_cleanup_clears_derived_search_rows_with_module_state() {
        let service = service();
        let row = service
            .store
            .insert(NewItem {
                id: "item".into(),
                content_ciphertext: vec![1],
                nonce: vec![1],
                content_type: "text/plain".into(),
                content_hash: compute_content_hash(b"indexed text"),
                search_text: Some("indexed text".into()),
                created_at: 1,
                app_bundle_id: None,
                app_name: None,
                payload_metadata: None,
            })
            .unwrap();
        let work = service.store.semantic_work("scope").unwrap().unwrap();
        assert_eq!(work.item_id, row.id);
        service
            .store
            .save_semantic_work("copypaste.fixture", "scope", &work, &[vec![1.0]])
            .unwrap();
        assert_eq!(
            service
                .store
                .semantic_matches("scope", &[1.0], 0.8)
                .unwrap()
                .scores
                .len(),
            1
        );

        service.clear("copypaste.fixture").unwrap();

        assert!(service
            .store
            .semantic_matches("scope", &[1.0], 0.8)
            .unwrap()
            .scores
            .is_empty());
    }

    /// The shipped cloud module rejects any `item_id` outside this pattern
    /// (its validator and the server check constraint), failing the batch.
    fn crosses_cloud_id_contract(id: &str) -> bool {
        (1..=128).contains(&id.len())
            && id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    }

    #[test]
    fn organization_records_round_trip_through_the_cloud_host_path() {
        const MODULE: &str = "copypaste.supabase";
        let (sender, sender_keys) = device(61, "sender");
        let (receiver, _) = device(62, "receiver");
        let key = sender_keys.item_key();
        let clip = copypaste_core::ingest::ingest_into(
            &sender.store,
            &sender_keys,
            "pinned and tagged",
            "text",
            100,
            &Default::default(),
        )
        .unwrap()
        .into_item()
        .id;
        sender.store.set_pinned_synced(&clip, true, &key).unwrap();
        sender
            .store
            .mutate_history_tags(
                &key,
                &copypaste_ipc::HistoryTagMutation::Save {
                    id: None,
                    name: "Work".into(),
                },
            )
            .unwrap();
        let tag = sender.store.history_tags(&[]).unwrap().tags.remove(0).id;
        sender
            .store
            .mutate_history_tags(
                &key,
                &copypaste_ipc::HistoryTagMutation::Assign {
                    clip_ids: vec![clip.clone()],
                    tag_ids: vec![tag.clone()],
                    assigned: true,
                },
            )
            .unwrap();

        let scan: copypaste_sync::scan::Scan = serde_json::from_value(
            sender
                .request(
                    MODULE,
                    SyncHostRequest::Scan {
                        since_ms: 0,
                        after_item_id: None,
                    },
                )
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            scan.items.len(),
            4,
            "one clip and three organization records"
        );
        for item in &scan.items {
            assert!(
                crosses_cloud_id_contract(&item.item_id),
                "{} cannot be uploaded",
                item.item_id
            );
        }

        let applied: Vec<Applied> = serde_json::from_value(
            receiver
                .request(MODULE, SyncHostRequest::Apply { items: scan.items })
                .unwrap(),
        )
        .unwrap();
        assert!(applied.iter().all(|item| matches!(item, Applied::Merged)));
        assert!(receiver.store.get(&clip).unwrap().unwrap().pinned);
        let tags = receiver
            .store
            .history_tags(std::slice::from_ref(&clip))
            .unwrap();
        assert_eq!(tags.tags.len(), 1);
        assert_eq!(tags.tags[0].name, "Work");
        assert_eq!(tags.assignments[&clip], vec![tag]);
    }

    #[test]
    fn short_lived_clips_and_their_tombstones_stay_out_of_cloud_until_kept() {
        use copypaste_core::storage::expiry::Clocks;
        const MODULE: &str = "copypaste.supabase";
        const MIN: i64 = 60 * 1000;
        let (service, keys) = device(63, "here");
        let key = keys.item_key();
        let now = copypaste_core::now_ms();
        let capture = |text: &str| {
            copypaste_core::ingest::ingest_into(
                &service.store,
                &keys,
                text,
                "text",
                now,
                &Default::default(),
            )
            .unwrap()
            .into_item()
            .id
        };
        let (short, gone, long) = (capture("482913"), capture("gone"), capture("long"));
        for (id, duration) in [(&short, 10 * MIN), (&gone, 10 * MIN), (&long, 120 * MIN)] {
            service
                .store
                .set_clip_expiry(&key, id, duration, false, Clocks::at(now))
                .unwrap()
                .unwrap();
        }
        service.store.delete(&gone).unwrap();
        let scan = |since_ms| -> Vec<String> {
            let scan: copypaste_sync::scan::Scan = serde_json::from_value(
                service
                    .request(
                        MODULE,
                        SyncHostRequest::Scan {
                            since_ms,
                            after_item_id: None,
                        },
                    )
                    .unwrap(),
            )
            .unwrap();
            scan.items.into_iter().map(|item| item.item_id).collect()
        };

        let first = scan(0);
        assert_eq!(first.len(), 2, "the long-lived clip and its note");
        assert!(first.contains(&long));
        let started_ms = copypaste_core::now_ms();
        service
            .request(MODULE, SyncHostRequest::CommitUpload { started_ms })
            .unwrap();

        let kept = service
            .store
            .keep_clip(&key, &short, false)
            .unwrap()
            .unwrap();
        service.reoffer(&kept.reoffer).unwrap();
        let second = scan(service.floor(MODULE).unwrap().created_at);
        assert!(second.contains(&short), "offered again after keep");
        assert_eq!(second.len(), 2, "the kept clip and its new note");
        assert!(!second.contains(&gone));
    }
}
