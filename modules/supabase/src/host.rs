//! The only application service adapter used by the Supabase package.
use copypaste_cloud::credentials::{CloudStateKey, CredentialError, CredentialState};
use copypaste_module_sdk::native::HostClient;
use copypaste_sync::{
    host::SyncHostRequest,
    scan::Scan,
    space::{
        SpaceEnvelope, SpaceKey, SpaceKeyReply, SpaceNotice, SpaceNoticeReply, SpaceRecovery,
        SpaceRecoveryReply,
    },
    Applied, CloudSource, LocalItem, SyncError,
};
use std::collections::BTreeMap;
use zeroize::Zeroizing;

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
fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn hex_decode(text: &str, out: &mut [u8]) -> Option<()> {
    if text.len() != out.len() * 2 {
        return None;
    }
    for (slot, pair) in out.iter_mut().zip(text.as_bytes().chunks(2)) {
        *slot = u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok()?;
    }
    Some(())
}
impl HostStore {
    pub fn allow_space_recovery(&self) -> Result<bool, SyncError> {
        self.0
            .request(&SyncHostRequest::SpaceAllowRecovery)
            .map_err(source_error)
    }
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
    /// Best-effort: an older host refuses the report and the round stands.
    pub fn report_round(&self, error: Option<&SyncError>) {
        use copypaste_sync::host::RoundFailure;
        let failure = error.map(|error| match error {
            SyncError::Source(_) => RoundFailure::LocalHistory,
            SyncError::Encrypt => RoundFailure::Encryption,
            SyncError::Unauthorized | SyncError::InvalidCredentials | SyncError::SessionExpired => {
                RoundFailure::SignInRequired
            }
            SyncError::RateLimited => RoundFailure::RateLimited,
            SyncError::Transport(_) => RoundFailure::Unreachable,
        });
        let _ = self
            .0
            .request::<_, ()>(&SyncHostRequest::RoundFinished { failure });
    }
    pub fn commit(&self, started_ms: i64) -> Result<(), SyncError> {
        self.0
            .request(&SyncHostRequest::CommitUpload { started_ms })
            .map_err(source_error)
    }
}
impl CloudSource for HostStore {
    fn group_snapshot(&self) -> Result<Option<String>, SyncError> {
        self.0
            .request(&SyncHostRequest::GroupSnapshot)
            .map_err(source_error)
    }
    fn apply_group(&self, device_id: &str, snapshot: &str) -> Result<(), SyncError> {
        self.0
            .request(&SyncHostRequest::GroupApply {
                device_id: device_id.into(),
                snapshot: snapshot.into(),
            })
            .map_err(source_error)
    }
    fn group_sync_allowed(&self) -> bool {
        self.enabled()
    }
    fn space_key(&self, fallback: Option<&[u8; 32]>) -> Result<Option<SpaceKey>, SyncError> {
        let reply: Option<SpaceKeyReply> = self
            .0
            .request(&SyncHostRequest::SpaceKey {
                fallback: fallback.map(|key| Zeroizing::new(hex_encode(key))),
            })
            .map_err(source_error)?;
        reply
            .map(|reply| {
                let mut key = Zeroizing::new([0; 32]);
                hex_decode(&reply.key, key.as_mut())
                    .ok_or(SyncError::Source("invalid space key"))?;
                Ok(SpaceKey {
                    epoch: reply.epoch,
                    key,
                    active: reply.active,
                })
            })
            .transpose()
    }
    fn rotate_space(&self) -> Result<bool, SyncError> {
        self.0
            .request(&SyncHostRequest::SpaceRotate)
            .map_err(source_error)
    }
    fn space_envelopes(&self) -> Result<Vec<SpaceEnvelope>, SyncError> {
        self.0
            .request(&SyncHostRequest::SpaceEnvelopes)
            .map_err(source_error)
    }
    fn accept_space_envelopes(&self, envelopes: &[SpaceEnvelope]) -> Result<bool, SyncError> {
        self.0
            .request(&SyncHostRequest::SpaceAccept {
                envelopes: envelopes.to_vec(),
            })
            .map_err(source_error)
    }
    fn space_recovery(&self) -> Result<Option<SpaceRecovery>, SyncError> {
        let reply: Option<SpaceRecoveryReply> = self
            .0
            .request(&SyncHostRequest::SpaceRecovery)
            .map_err(source_error)?;
        reply
            .map(|reply| {
                let mut plaintext = Zeroizing::new(vec![0; reply.plaintext.len() / 2]);
                hex_decode(&reply.plaintext, &mut plaintext)
                    .ok_or(SyncError::Source("invalid space recovery"))?;
                Ok(SpaceRecovery {
                    epoch: reply.epoch,
                    plaintext,
                })
            })
            .transpose()
    }
    fn recover_space(&self, epoch: u64, plaintext: &[u8]) -> Result<bool, SyncError> {
        self.0
            .request(&SyncHostRequest::SpaceRecover {
                epoch,
                plaintext: Zeroizing::new(hex_encode(plaintext)),
            })
            .map_err(source_error)
    }
    fn confirm_space_member(&self, device_id: &str, epoch: u64) -> Result<(), SyncError> {
        self.0
            .request(&SyncHostRequest::SpaceConfirm {
                device_id: device_id.into(),
                epoch,
            })
            .map_err(source_error)
    }
    fn space_notice(&self) -> Result<Option<SpaceNotice>, SyncError> {
        let reply: Option<SpaceNoticeReply> = self
            .0
            .request(&SyncHostRequest::SpaceNotice)
            .map_err(source_error)?;
        reply
            .map(|reply| {
                let mut previous_key = Zeroizing::new([0; 32]);
                hex_decode(&reply.previous_key, previous_key.as_mut())
                    .ok_or(SyncError::Source("invalid space notice"))?;
                Ok(SpaceNotice {
                    previous_epoch: reply.previous_epoch,
                    previous_key,
                    recipients: reply.recipients,
                })
            })
            .transpose()
    }
    fn space_noticed(&self) -> Result<(), SyncError> {
        self.0
            .request(&SyncHostRequest::SpaceNoticed)
            .map_err(source_error)
    }
    fn space_departed(&self) -> Result<(), SyncError> {
        self.0
            .request(&SyncHostRequest::SpaceDeparted)
            .map_err(source_error)
    }
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
