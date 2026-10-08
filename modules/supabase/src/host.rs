//! The only application service adapter used by the Supabase package.
use copypaste_cloud::credentials::{CloudStateKey, CredentialError, CredentialState};
use copypaste_module_sdk::native::HostClient;
use copypaste_sync::{
    host::SyncHostRequest, scan::Scan, Applied, CloudSource, LocalItem, SyncError,
};
use std::collections::BTreeMap;

#[derive(Clone)]
pub struct HostStore(pub HostClient);
impl CredentialState for HostStore {
    fn state(&self, key: &str) -> Result<Option<String>, CredentialError> {
        let mut values: BTreeMap<String, Option<String>> = self
            .0
            .request(&SyncHostRequest::ReadState {
                keys: vec![key.into()],
            })
            .map_err(|_| CredentialError::Host)?;
        Ok(values.remove(key).flatten())
    }
    fn set_state_all(&self, entries: &[(&str, &str)]) -> Result<(), CredentialError> {
        self.0
            .request::<_, ()>(&SyncHostRequest::WriteState {
                values: entries
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect(),
            })
            .map_err(|_| CredentialError::Host)
    }
    fn clear_state(&self, keys: &[&str]) -> Result<(), CredentialError> {
        self.0
            .request::<_, ()>(&SyncHostRequest::ClearState {
                keys: keys.iter().map(|k| k.to_string()).collect(),
            })
            .map_err(|_| CredentialError::Host)
    }
}
impl HostStore {
    pub fn enabled(&self) -> bool {
        self.0
            .request(&SyncHostRequest::SyncEnabled)
            .unwrap_or(false)
    }
    fn stamp(&self, key: CloudStateKey) -> Result<i64, SyncError> {
        Ok(self
            .state(key.as_str())
            .map_err(source_error)?
            .and_then(|s| s.parse().ok())
            .unwrap_or(0)
            .max(0))
    }
    pub fn commit(&self, started_ms: i64) -> Result<(), SyncError> {
        self.0
            .request(&SyncHostRequest::CommitUpload { started_ms })
            .map_err(source_error)
    }
}
impl CloudSource for HostStore {
    fn device_id(&self) -> String {
        self.0
            .request(&SyncHostRequest::DeviceId)
            .unwrap_or_default()
    }
    fn local_changes_since(&self, since_ms: i64) -> Result<Vec<LocalItem>, SyncError> {
        self.local_changes_after(since_ms, None)
    }
    fn local_changes_after(
        &self,
        since_ms: i64,
        after_item_id: Option<&str>,
    ) -> Result<Vec<LocalItem>, SyncError> {
        let scan: Scan = self
            .0
            .request(&SyncHostRequest::Scan {
                since_ms,
                after_item_id: after_item_id.map(str::to_owned),
            })
            .map_err(source_error)?;
        Ok(scan.items)
    }
    fn apply_remote(&self, item: LocalItem) -> Result<Applied, SyncError> {
        self.apply_remote_batch(vec![item])?
            .pop()
            .ok_or(SyncError::Source("invalid host result"))
    }
    fn apply_remote_batch(&self, items: Vec<LocalItem>) -> Result<Vec<Applied>, SyncError> {
        // One maximum-sized value always fits the independently bounded host
        // channel; split network pages before crossing that boundary.
        let mut result = Vec::with_capacity(items.len());
        for item in items {
            let mut applied: Vec<Applied> = self
                .0
                .request(&SyncHostRequest::Apply { items: vec![item] })
                .map_err(source_error)?;
            result.append(&mut applied);
        }
        Ok(result)
    }
    fn watermark(&self) -> Result<i64, SyncError> {
        self.stamp(CloudStateKey::WatermarkMs)
    }
    fn upload_floor(&self) -> Result<i64, SyncError> {
        self.stamp(CloudStateKey::UploadFloorMs)
    }
    fn upload_floor_item_id(&self) -> Result<Option<String>, SyncError> {
        self.state(CloudStateKey::UploadFloorItemId.as_str())
            .map_err(source_error)
    }
    fn watermark_item_id(&self) -> Result<Option<String>, SyncError> {
        self.state(CloudStateKey::WatermarkItemId.as_str())
            .map_err(source_error)
    }
    fn requeue_local_winner(&self, incoming: &LocalItem) -> Result<bool, SyncError> {
        self.0
            .request(&SyncHostRequest::Requeue {
                incoming: incoming.clone(),
            })
            .map_err(source_error)
    }
    fn set_watermark(&self, ms: i64) -> Result<(), SyncError> {
        self.set_watermark_keyset(ms, "")
    }
    fn set_watermark_keyset(&self, ms: i64, item_id: &str) -> Result<(), SyncError> {
        self.set_state_all(&[
            (CloudStateKey::WatermarkMs.as_str(), &ms.to_string()),
            (CloudStateKey::WatermarkItemId.as_str(), item_id),
        ])
        .map_err(source_error)
    }
}
fn source_error<T>(_: T) -> SyncError {
    SyncError::Source("encrypted history service unavailable")
}
