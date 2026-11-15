//! Application-owned encrypted state and history services for sync modules.
use crate::ModuleError;
use copypaste_core::Store;
use copypaste_sync::{
    account_state::SIGN_OUT_KEYS,
    host::{
        CloudFileFetch, CloudFileUpload, FileBytes, SyncHostRequest, UNREADABLE_UPLOADS,
        UPLOAD_FLOOR, UPLOAD_FLOOR_ITEM,
    },
    store::{floor_after_round, Offer, StoreView},
    unreadable::{UnreadableUploads, UploadFloor},
    Applied,
};
use serde_json::Value;
use std::{collections::BTreeMap, sync::Mutex};

/// File work a cloud provider does for the host: uploading kept copies and
/// delivering files the host could not get from their origin device.
pub trait CloudFileHost: Send + Sync {
    fn uploads(&self, since_ms: i64, limit: usize) -> Result<Vec<CloudFileUpload>, String>;
    fn read(&self, item_id: &str, file: u32, offset: u64, len: usize) -> Result<Vec<u8>, String>;
    fn gone(&self, item_ids: &[String]) -> Vec<String>;
    fn fetches(&self) -> Vec<CloudFileFetch>;
    fn chunk(&self, item_id: &str, file: u32, bytes: Vec<u8>) -> Result<(), String>;
    fn done(&self, item_id: &str, file: u32, result: Result<String, String>);
}

/// Largest plaintext range one `ReadCloudFile` returns.
const MAX_FILE_READ_BYTES: usize = 1024 * 1024;

pub struct SyncServices {
    files: Option<std::sync::Arc<dyn CloudFileHost>>,
    group: Option<GroupServices>,
    space: Option<std::sync::Arc<dyn copypaste_sync::space::SpaceHost>>,
    store: Store,
    view: StoreView,
    enabled: Box<dyn Fn() -> bool + Send + Sync>,
    remote_change: Box<dyn Fn(i64) + Send + Sync>,
    progress: Mutex<BTreeMap<String, Progress>>,
    /// The last round outcome logged, so a failing loop logs once.
    round: Mutex<Option<Option<copypaste_sync::host::RoundFailure>>>,
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
            files: None,
            group: None,
            space: None,
            store,
            view,
            enabled: Box::new(enabled),
            remote_change: Box::new(remote_change),
            progress: Mutex::default(),
            round: Mutex::default(),
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
    pub fn with_space(
        mut self,
        space: std::sync::Arc<dyn copypaste_sync::space::SpaceHost>,
    ) -> Self {
        self.space = Some(space);
        self
    }
    fn space_request(&self, request: SyncHostRequest) -> Result<Value, ModuleError> {
        use copypaste_sync::space::{SpaceKeyReply, SpaceRecoveryReply};
        self.require_enabled()?;
        let Some(space) = self.space.as_ref() else {
            // Without a device space, answer as `SyncSource`'s defaults do:
            // the passphrase key stays in use and nothing rotates.
            return Ok(match request {
                SyncHostRequest::SpaceRotate
                | SyncHostRequest::SpaceAccept { .. }
                | SyncHostRequest::SpaceRecover { .. }
                | SyncHostRequest::SpaceAllowRecovery => Value::Bool(false),
                SyncHostRequest::SpaceEnvelopes => Value::Array(Vec::new()),
                _ => Value::Null,
            });
        };
        let state = |_| ModuleError::State;
        let value = match request {
            SyncHostRequest::SpaceKey { fallback } => {
                let fallback = fallback
                    .map(|hex| {
                        let mut key = zeroize::Zeroizing::new([0; 32]);
                        hex::decode_to_slice(hex.as_bytes(), key.as_mut())
                            .map(|()| key)
                            .map_err(|_| ModuleError::State)
                    })
                    .transpose()?;
                let reply =
                    space
                        .key(fallback.as_deref())
                        .map_err(state)?
                        .map(|key| SpaceKeyReply {
                            epoch: key.epoch,
                            key: zeroize::Zeroizing::new(hex::encode(*key.key)),
                            active: key.active,
                        });
                serde_json::to_value(reply)
            }
            SyncHostRequest::SpaceRotate => serde_json::to_value(space.rotate().map_err(state)?),
            SyncHostRequest::SpaceEnvelopes => {
                serde_json::to_value(space.envelopes().map_err(state)?)
            }
            SyncHostRequest::SpaceAccept { envelopes } => {
                if envelopes.len() > 64 || envelopes.iter().any(|e| !e.is_valid()) {
                    return Err(ModuleError::State);
                }
                serde_json::to_value(space.accept(&envelopes).map_err(state)?)
            }
            SyncHostRequest::SpaceRecovery => {
                let reply = space
                    .recovery()
                    .map_err(state)?
                    .map(|recovery| SpaceRecoveryReply {
                        epoch: recovery.epoch,
                        plaintext: zeroize::Zeroizing::new(hex::encode(&*recovery.plaintext)),
                    });
                serde_json::to_value(reply)
            }
            SyncHostRequest::SpaceRecover { epoch, plaintext } => {
                if plaintext.len() > 32 * 1024 {
                    return Err(ModuleError::State);
                }
                let plaintext = zeroize::Zeroizing::new(
                    hex::decode(plaintext.as_bytes()).map_err(|_| ModuleError::State)?,
                );
                serde_json::to_value(space.recover(epoch, &plaintext).map_err(state)?)
            }
            SyncHostRequest::SpaceAllowRecovery => {
                serde_json::to_value(space.allow_recovery().map_err(state)?)
            }
            SyncHostRequest::SpaceConfirm { device_id, epoch } => {
                if device_id.len() > 128 {
                    return Err(ModuleError::State);
                }
                space.confirm(&device_id, epoch).map_err(state)?;
                Ok(Value::Null)
            }
            SyncHostRequest::SpaceDeparted => {
                space.departed().map_err(state)?;
                Ok(Value::Null)
            }
            SyncHostRequest::SpaceNotice => {
                let reply = space.notice().map_err(state)?.map(|notice| {
                    copypaste_sync::space::SpaceNoticeReply {
                        previous_epoch: notice.previous_epoch,
                        previous_key: zeroize::Zeroizing::new(hex::encode(*notice.previous_key)),
                        recipients: notice.recipients,
                    }
                });
                serde_json::to_value(reply)
            }
            SyncHostRequest::SpaceNoticed => {
                space.noticed().map_err(state)?;
                Ok(Value::Null)
            }
            _ => return Err(ModuleError::State),
        };
        value.map_err(|_| ModuleError::State)
    }
    pub fn with_files(mut self, files: std::sync::Arc<dyn CloudFileHost>) -> Self {
        self.files = Some(files);
        self
    }
    fn files(&self) -> Result<&dyn CloudFileHost, ModuleError> {
        self.files
            .as_deref()
            .ok_or_else(|| ModuleError::Invalid("Cloud file storage is unavailable.".into()))
    }
    pub fn enabled(&self) -> bool {
        (self.enabled)()
    }
    fn key(id: &str, key: &str) -> Result<String, ModuleError> {
        copypaste_core::storage::module_state_key(id, key)
            .map_err(|_| ModuleError::Invalid("Invalid sync state key.".into()))
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
            .forget_module_records(id)
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
            request @ (SyncHostRequest::SpaceKey { .. }
            | SyncHostRequest::SpaceRotate
            | SyncHostRequest::SpaceEnvelopes
            | SyncHostRequest::SpaceAccept { .. }
            | SyncHostRequest::SpaceRecovery
            | SyncHostRequest::SpaceRecover { .. }
            | SyncHostRequest::SpaceAllowRecovery
            | SyncHostRequest::SpaceConfirm { .. }
            | SyncHostRequest::SpaceDeparted
            | SyncHostRequest::SpaceNotice
            | SyncHostRequest::SpaceNoticed) => self.space_request(request),
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
                let values = self
                    .store
                    .module_state(id, &keys)
                    .map_err(|_| ModuleError::State)?;
                serde_json::to_value(values).map_err(|_| ModuleError::State)
            }
            SyncHostRequest::WriteState { values } => {
                let _progress = self.progress.lock().map_err(|_| ModuleError::State)?;
                self.store
                    .set_module_state(id, &values)
                    .map_err(|_| ModuleError::State)?;
                Ok(Value::Null)
            }
            SyncHostRequest::ClearState { keys } => {
                self.store
                    .clear_module_state_keys(id, &keys)
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
            SyncHostRequest::CloudFileUploads { since_ms, limit } => {
                self.require_enabled()?;
                let uploads = self
                    .files()?
                    .uploads(since_ms, limit.min(64) as usize)
                    .map_err(ModuleError::Invalid)?;
                serde_json::to_value(uploads).map_err(|_| ModuleError::State)
            }
            SyncHostRequest::ReadCloudFile {
                item_id,
                file_index,
                offset,
                len,
            } => {
                self.require_enabled()?;
                let bytes = self
                    .files()?
                    .read(
                        &item_id,
                        file_index,
                        offset,
                        (len as usize).min(MAX_FILE_READ_BYTES),
                    )
                    .map_err(ModuleError::Invalid)?;
                serde_json::to_value(FileBytes(bytes)).map_err(|_| ModuleError::State)
            }
            SyncHostRequest::GoneFiles { item_ids } => {
                serde_json::to_value(self.files()?.gone(&item_ids)).map_err(|_| ModuleError::State)
            }
            SyncHostRequest::CloudFileFetches => {
                self.require_enabled()?;
                serde_json::to_value(self.files()?.fetches()).map_err(|_| ModuleError::State)
            }
            SyncHostRequest::CloudFileChunk {
                item_id,
                file_index,
                chunk,
            } => {
                if chunk.0.len() > MAX_FILE_READ_BYTES {
                    return Err(ModuleError::Invalid("The file chunk is too large.".into()));
                }
                self.files()?
                    .chunk(&item_id, file_index, chunk.0)
                    .map_err(ModuleError::Invalid)?;
                Ok(Value::Null)
            }
            SyncHostRequest::CloudFileDone {
                item_id,
                file_index,
                sha256,
                error,
            } => {
                self.files()?.done(
                    &item_id,
                    file_index,
                    sha256
                        .ok_or_else(|| error.unwrap_or_else(|| "The file was not fetched.".into())),
                );
                Ok(Value::Null)
            }
            SyncHostRequest::RoundFinished { failure } => {
                self.note_round(failure);
                Ok(Value::Null)
            }
            SyncHostRequest::FileRoundFinished {
                uploaded,
                failed,
                removed,
            } => {
                if uploaded > 0 || removed > 0 {
                    tracing::info!("cloud files: {uploaded} uploaded, {removed} removed");
                }
                if failed > 0 {
                    tracing::warn!("cloud file upload failed for {failed} files");
                }
                Ok(Value::Null)
            }
        }
    }
    /// Logs a round outcome: every round at debug, a new failure or a
    /// recovery once.
    fn note_round(&self, failure: Option<copypaste_sync::host::RoundFailure>) {
        use copypaste_sync::host::RoundFailure;
        let previous = self
            .round
            .lock()
            .map(|mut last| last.replace(failure))
            .unwrap_or(None);
        match failure {
            None => {
                tracing::debug!("cloud sync round finished");
                if matches!(previous, Some(Some(_))) {
                    tracing::info!("cloud sync recovered");
                }
            }
            Some(kind) if previous != Some(Some(kind)) => match kind {
                RoundFailure::SignInRequired => {
                    tracing::error!("cloud sync stopped: {}", kind.as_str());
                }
                _ => tracing::warn!("cloud sync failed: {}", kind.as_str()),
            },
            Some(_) => tracing::debug!("cloud sync round failed again"),
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
