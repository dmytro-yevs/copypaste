//! Space key rounds: learn sealed epochs, rotate after removals, publish.
//!
//! Runs before history so a removal learned in this round already decides
//! which key this round writes with.
use std::collections::BTreeSet;
use std::sync::Arc;

use super::{AuthApi, CloudSource, CloudSync, RestApi, SyncError};
use crate::crypto::{decrypt_row, encrypt_row, SyncKey};
use copypaste_sync::space::{SpaceEnvelope, RECOVERY_RECIPIENT};

/// The key rows are written and read with.
pub(super) struct RowKey {
    pub(super) epoch: u64,
    pub(super) key: SyncKey,
}

pub(super) struct SpaceRound {
    row: Option<Arc<RowKey>>,
    active: bool,
    supported: bool,
    cleaned: Option<u64>,
    /// Envelopes are fetched only after a sign that the key may have moved:
    /// the first round, a changed roster row, or rows under a newer epoch.
    fetch: bool,
    /// Envelopes are published only after the roster or the key changed.
    publish: bool,
}

impl SpaceRound {
    pub(super) fn new(passphrase: Option<&SyncKey>) -> Self {
        Self {
            row: passphrase.map(|key| {
                Arc::new(RowKey {
                    epoch: 0,
                    key: SyncKey::from_bytes(*key.to_bytes()),
                })
            }),
            active: true,
            supported: false,
            cleaned: None,
            fetch: true,
            publish: true,
        }
    }
}

fn recovery_aad(epoch: u64) -> String {
    format!("copypaste-space-recovery-{epoch}")
}
fn notice_aad(epoch: u64, recipient: &str) -> String {
    format!("copypaste-space-removal-{epoch}-{recipient}")
}

impl<R: RestApi, A: AuthApi> CloudSync<R, A> {
    pub(super) fn row_key(&self) -> Option<Arc<RowKey>> {
        self.lock_space().row.clone()
    }
    /// The key and epoch this round writes with, for data sealed outside the
    /// row path (cloud file objects). Changes after a rotation; read it per use.
    pub fn current_space_key(&self) -> Option<(u64, zeroize::Zeroizing<[u8; 32]>)> {
        self.row_key().map(|row| (row.epoch, row.key.to_bytes()))
    }
    /// Something in the account changed: check and publish envelopes next.
    pub(super) fn note_space_change(&self) {
        let mut round = self.lock_space();
        round.fetch = true;
        round.publish = true;
    }
    pub(super) fn space_check_pending(&self) -> bool {
        self.lock_space().fetch
    }
    pub(super) fn note_newer_epoch(&self) {
        self.lock_space().fetch = true;
    }
    pub(super) fn space_active(&self) -> bool {
        self.lock_space().active
    }
    fn lock_space(&self) -> std::sync::MutexGuard<'_, SpaceRound> {
        self.space.lock().unwrap_or_else(|e| e.into_inner())
    }
    fn refresh_row_key(&self, source: &dyn CloudSource) -> Result<(), SyncError> {
        let fallback = self.key.as_ref().map(SyncKey::to_bytes);
        let current = source.space_key(fallback.as_deref())?;
        let mut round = self.lock_space();
        match current {
            Some(space) => {
                round.active = space.active;
                if round
                    .row
                    .as_ref()
                    .is_none_or(|row| row.epoch != space.epoch || *row.key.to_bytes() != *space.key)
                {
                    round.row = Some(Arc::new(RowKey {
                        epoch: space.epoch,
                        key: SyncKey::from_bytes(*space.key),
                    }));
                    round.publish = true;
                    // Rows skipped under the old key are read again.
                    self.group_downloaded
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .clear();
                }
            }
            // An application without spaces keeps the passphrase key.
            None => round.active = true,
        }
        Ok(())
    }

    pub(super) async fn load_space(&self, source: &dyn CloudSource) -> Result<(), SyncError> {
        let fetch = {
            let mut round = self.lock_space();
            round.row.is_none() || std::mem::take(&mut round.fetch)
        };
        if !fetch {
            return self.refresh_row_key(source);
        }
        let device_id = source.device_id();
        let fetched = self
            .execute(|token| {
                let device_id = device_id.clone();
                async move { self.rest.fetch_space_envelopes(&token, &device_id).await }
            })
            .await?;
        self.lock_space().supported = fetched.is_some();
        let Some(envelopes) = fetched else {
            return Ok(());
        };
        let (recovery, members): (Vec<SpaceEnvelope>, Vec<SpaceEnvelope>) = envelopes
            .into_iter()
            .partition(|envelope| envelope.recipient == RECOVERY_RECIPIENT);
        let members: Vec<_> = members
            .into_iter()
            .filter(|envelope| envelope.recipient == device_id)
            .collect();
        if !members.is_empty() {
            source.accept_space_envelopes(&members)?;
        }
        // A removal notice is a member's signed roster under the key this
        // device still holds; applying it is how a removed device learns.
        if let Some(current) = self.row_key() {
            for envelope in members.iter().filter(|e| e.epoch > current.epoch) {
                if let Ok(roster) = decrypt_row(
                    &envelope.ciphertext,
                    &envelope.nonce,
                    &current.key,
                    &notice_aad(envelope.epoch, &device_id),
                ) {
                    if let Ok(json) = std::str::from_utf8(&roster) {
                        source.apply_group(&envelope.sender, json)?;
                    }
                }
            }
        }
        // Rows arrive newest first; the host takes one only without a key.
        if let Some(passphrase) = &self.key {
            for envelope in &recovery {
                let Ok(plaintext) = decrypt_row(
                    &envelope.ciphertext,
                    &envelope.nonce,
                    passphrase,
                    &recovery_aad(envelope.epoch),
                ) else {
                    continue;
                };
                if source.recover_space(envelope.epoch, &plaintext)? {
                    break;
                }
            }
        }
        self.refresh_row_key(source)
    }

    /// Returns whether this round rotated the key.
    pub(super) async fn publish_space(&self, source: &dyn CloudSource) -> Result<bool, SyncError> {
        if !self.lock_space().supported || !self.space_active() {
            return Ok(false);
        }
        let rotated = source.rotate_space()?;
        if rotated {
            self.refresh_row_key(source)?;
        }
        if !std::mem::take(&mut self.lock_space().publish) {
            return Ok(rotated);
        }
        let Some(row_key) = self.row_key() else {
            return Ok(rotated);
        };
        let epoch = row_key.epoch;
        let device_id = source.device_id();
        let index = self
            .execute(|token| async move { self.rest.fetch_space_index(&token, epoch).await })
            .await?;
        let sealed: BTreeSet<&str> = index.iter().map(|entry| entry.recipient.as_str()).collect();
        let mut outgoing: Vec<SpaceEnvelope> = source
            .space_envelopes()?
            .into_iter()
            .filter(|envelope| {
                envelope.epoch == epoch && !sealed.contains(envelope.recipient.as_str())
            })
            .collect();
        if let (Some(passphrase), false) = (&self.key, sealed.contains(RECOVERY_RECIPIENT)) {
            if let Some(recovery) = source.space_recovery()?.filter(|r| r.epoch == epoch) {
                let (nonce, ciphertext) =
                    encrypt_row(&recovery.plaintext, passphrase, &recovery_aad(epoch))
                        .map_err(|_| SyncError::Encrypt)?;
                outgoing.push(SpaceEnvelope {
                    epoch,
                    sender: device_id.clone(),
                    recipient: RECOVERY_RECIPIENT.into(),
                    nonce,
                    ciphertext,
                });
            }
        }
        let notice = source.space_notice()?;
        if let (Some(notice), Some(roster)) = (&notice, source.group_snapshot()?) {
            let previous = SyncKey::from_bytes(*notice.previous_key);
            for recipient in &notice.recipients {
                let (nonce, ciphertext) =
                    encrypt_row(roster.as_bytes(), &previous, &notice_aad(epoch, recipient))
                        .map_err(|_| SyncError::Encrypt)?;
                outgoing.push(SpaceEnvelope {
                    epoch,
                    sender: device_id.clone(),
                    recipient: recipient.clone(),
                    nonce,
                    ciphertext,
                });
            }
        }
        for batch in outgoing.chunks(crate::rest::space::SPACE_PAGE_SIZE) {
            self.execute(
                |token| async move { self.rest.insert_space_envelopes(&token, batch).await },
            )
            .await?;
        }
        if notice.is_some() {
            source.space_noticed()?;
        }
        if epoch > 0 && self.lock_space().cleaned != Some(epoch) {
            let sender = device_id.clone();
            self.execute(|token| {
                let sender = sender.clone();
                async move {
                    self.rest
                        .delete_space_envelopes(&token, &sender, epoch)
                        .await
                }
            })
            .await?;
            self.lock_space().cleaned = Some(epoch);
        }
        Ok(rotated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rest::{group::GroupVersion, space::SpaceIndexEntry, CloudItem, GroupRow};
    use crate::sync::{
        fakes::{config, item, key, session, FakeAuth, FakeRest},
        transport::TransportFault,
    };
    use copypaste_p2p::{DeviceProfile, Node, PeerStore};
    use copypaste_sync::{space::SpaceRecovery, Applied, LocalItem, PullCursor};
    use std::collections::BTreeMap;
    use std::sync::Mutex;

    /// One account: clipboard rows, roster rows and key envelopes.
    #[derive(Clone, Default)]
    struct Backend {
        items: Arc<FakeRest>,
        groups: Arc<Mutex<BTreeMap<String, GroupRow>>>,
        envelopes: Arc<Mutex<Vec<SpaceEnvelope>>>,
        envelope_reads: Arc<std::sync::atomic::AtomicUsize>,
    }
    impl RestApi for Backend {
        async fn list_since(
            &self,
            token: &str,
            position: i64,
            after: Option<&str>,
            limit: u32,
        ) -> Result<Vec<crate::rest::RowListing>, TransportFault> {
            self.items.list_since(token, position, after, limit).await
        }
        async fn fetch_rows(
            &self,
            token: &str,
            ids: &[String],
        ) -> Result<crate::rest::RowPage, TransportFault> {
            self.items.fetch_rows(token, ids).await
        }
        async fn upsert(&self, token: &str, items: &[CloudItem]) -> Result<(), TransportFault> {
            self.items.upsert(token, items).await
        }
        async fn fetch_group_catalog(
            &self,
            _: &str,
            after: Option<&str>,
        ) -> Result<Vec<GroupVersion>, TransportFault> {
            Ok(self
                .groups
                .lock()
                .unwrap()
                .values()
                .filter(|row| after.is_none_or(|id| row.device_id.as_str() > id))
                .map(|row| GroupVersion {
                    device_id: row.device_id.clone(),
                    ciphertext_digest: row.ciphertext_digest(),
                })
                .collect())
        }
        async fn fetch_groups(
            &self,
            _: &str,
            ids: &[String],
        ) -> Result<Vec<GroupRow>, TransportFault> {
            Ok(self
                .groups
                .lock()
                .unwrap()
                .values()
                .filter(|row| ids.contains(&row.device_id))
                .cloned()
                .collect())
        }
        async fn upsert_group(&self, _: &str, row: &GroupRow) -> Result<(), TransportFault> {
            self.groups
                .lock()
                .unwrap()
                .insert(row.device_id.clone(), row.clone());
            Ok(())
        }
        async fn fetch_space_envelopes(
            &self,
            _: &str,
            device_id: &str,
        ) -> Result<Option<Vec<SpaceEnvelope>>, TransportFault> {
            self.envelope_reads
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let mut rows: Vec<_> = self
                .envelopes
                .lock()
                .unwrap()
                .iter()
                .filter(|e| e.recipient == device_id || e.recipient == RECOVERY_RECIPIENT)
                .cloned()
                .collect();
            rows.sort_by_key(|row| std::cmp::Reverse(row.epoch));
            Ok(Some(rows))
        }
        async fn fetch_space_index(
            &self,
            _: &str,
            epoch: u64,
        ) -> Result<Vec<SpaceIndexEntry>, TransportFault> {
            Ok(self
                .envelopes
                .lock()
                .unwrap()
                .iter()
                .filter(|e| e.epoch == epoch)
                .map(|e| SpaceIndexEntry {
                    sender: e.sender.clone(),
                    recipient: e.recipient.clone(),
                })
                .collect())
        }
        async fn insert_space_envelopes(
            &self,
            _: &str,
            rows: &[SpaceEnvelope],
        ) -> Result<(), TransportFault> {
            let mut stored = self.envelopes.lock().unwrap();
            for row in rows {
                assert!(row.is_valid());
                if !stored.iter().any(|e| {
                    (e.epoch, &e.sender, &e.recipient) == (row.epoch, &row.sender, &row.recipient)
                }) {
                    stored.push(row.clone());
                }
            }
            Ok(())
        }
        async fn delete_space_envelopes(
            &self,
            _: &str,
            sender: &str,
            before: u64,
        ) -> Result<(), TransportFault> {
            self.envelopes
                .lock()
                .unwrap()
                .retain(|e| e.sender != sender || e.epoch >= before);
            Ok(())
        }
    }

    /// A device: its group, its history and its cursor.
    struct Device {
        _dir: tempfile::TempDir,
        node: Node,
        id: String,
        items: Mutex<BTreeMap<String, LocalItem>>,
        cursor: Mutex<PullCursor>,
    }
    fn convert<T: serde::Serialize, U: serde::de::DeserializeOwned>(value: &T) -> U {
        serde_json::from_value(serde_json::to_value(value).unwrap()).unwrap()
    }
    impl Device {
        fn new(id: &str) -> Self {
            let dir = tempfile::tempdir().unwrap();
            let node = Node::new(
                PeerStore::open(
                    &dir.path().join("peers.json"),
                    &copypaste_p2p::PairingToken::generate().psk(),
                )
                .unwrap(),
                None,
                0,
                true,
            );
            Self {
                _dir: dir,
                node,
                id: id.into(),
                items: Mutex::default(),
                cursor: Mutex::default(),
            }
        }
        fn copy(&self, id: &str, at: i64, text: &str) {
            let mut local = item(id, at, text);
            local.origin_device_id = self.id.clone();
            self.items.lock().unwrap().insert(id.into(), local);
        }
        fn text(&self, id: &str) -> Option<String> {
            self.items
                .lock()
                .unwrap()
                .get(id)
                .map(|item| String::from_utf8(item.content.to_vec()).unwrap())
        }
    }
    impl CloudSource for Device {
        fn group_snapshot(&self) -> Result<Option<String>, SyncError> {
            let peers = self.node.peers();
            let snapshot = peers
                .group_snapshot(&self.id, &self.id, DeviceProfile::current(), None)
                .unwrap();
            Ok(peers.group_exportable().then(|| snapshot.encode().unwrap()))
        }
        fn apply_group(&self, id: &str, snapshot: &str) -> Result<(), SyncError> {
            let _ = self.node.import_cloud_group(id, snapshot);
            Ok(())
        }
        fn group_sync_allowed(&self) -> bool {
            self.node.peers().group_exportable()
        }
        fn space_key(
            &self,
            fallback: Option<&[u8; 32]>,
        ) -> Result<Option<copypaste_sync::space::SpaceKey>, SyncError> {
            let peers = self.node.peers();
            if let (Some(key), true) = (fallback, peers.group_active_member()) {
                if peers.space_key().is_none() {
                    let _ = peers.register_space_key(key);
                }
            }
            Ok(peers
                .space_key()
                .map(|key| copypaste_sync::space::SpaceKey {
                    epoch: key.epoch,
                    key: key.key,
                    active: peers.group_active_member(),
                }))
        }
        fn rotate_space(&self) -> Result<bool, SyncError> {
            Ok(self.node.peers().rotate_space_if_needed().unwrap())
        }
        fn space_envelopes(&self) -> Result<Vec<SpaceEnvelope>, SyncError> {
            Ok(self
                .node
                .peers()
                .space_envelopes()
                .unwrap()
                .iter()
                .map(convert)
                .collect())
        }
        fn accept_space_envelopes(&self, envelopes: &[SpaceEnvelope]) -> Result<bool, SyncError> {
            Ok(envelopes.iter().fold(false, |changed, envelope| {
                changed
                    | self
                        .node
                        .peers()
                        .accept_space_envelope(&convert(envelope))
                        .unwrap_or(false)
            }))
        }
        fn space_recovery(&self) -> Result<Option<SpaceRecovery>, SyncError> {
            Ok(self
                .node
                .peers()
                .space_recovery_plaintext()
                .map(|(epoch, plaintext)| SpaceRecovery { epoch, plaintext }))
        }
        fn recover_space(&self, epoch: u64, plaintext: &[u8]) -> Result<bool, SyncError> {
            Ok(self
                .node
                .peers()
                .accept_space_recovery(epoch, plaintext)
                .unwrap_or(false))
        }
        fn confirm_space_member(&self, id: &str, epoch: u64) -> Result<(), SyncError> {
            self.node.peers().confirm_space_member(id, epoch).unwrap();
            Ok(())
        }
        fn space_departed(&self) -> Result<(), SyncError> {
            self.node.peers().forget_departed_space().unwrap();
            Ok(())
        }
        fn space_notice(&self) -> Result<Option<copypaste_sync::space::SpaceNotice>, SyncError> {
            Ok(self.node.peers().space_removal_notice().map(|notice| {
                copypaste_sync::space::SpaceNotice {
                    previous_epoch: notice.previous_epoch,
                    previous_key: notice.previous_key,
                    recipients: notice.recipients,
                }
            }))
        }
        fn space_noticed(&self) -> Result<(), SyncError> {
            self.node.peers().clear_space_removal_notice().unwrap();
            Ok(())
        }
        fn device_id(&self) -> String {
            self.id.clone()
        }
        fn local_changes_since(&self, since: i64) -> Result<Vec<LocalItem>, SyncError> {
            Ok(self
                .items
                .lock()
                .unwrap()
                .values()
                .filter(|item| item.origin_device_id == self.id && item.created_at >= since)
                .cloned()
                .collect())
        }
        fn apply_remote(&self, incoming: LocalItem) -> Result<Applied, SyncError> {
            self.items
                .lock()
                .unwrap()
                .insert(incoming.item_id.clone(), incoming);
            Ok(Applied::Merged)
        }
        fn pull_cursor(&self) -> Result<PullCursor, SyncError> {
            Ok(self.cursor.lock().unwrap().clone())
        }
        fn set_pull_cursor(&self, cursor: &PullCursor) -> Result<(), SyncError> {
            *self.cursor.lock().unwrap() = cursor.clone();
            Ok(())
        }
    }

    fn driver(backend: &Backend) -> CloudSync<Backend, FakeAuth> {
        CloudSync::new(
            backend.clone(),
            FakeAuth::default(),
            key(),
            config(),
            session("t"),
        )
    }
    fn pairing_of(owner: &Device, device: &str) -> String {
        owner
            .node
            .peers()
            .list()
            .into_iter()
            .find(|peer| peer.device_id.as_deref() == Some(device))
            .unwrap()
            .pairing_id
            .clone()
    }

    #[tokio::test]
    async fn a_forged_epoch_with_a_forged_envelope_does_not_stall_the_rows_behind_it() {
        // Whoever can write the account's rows can also write an envelope for
        // any recipient and epoch: neither is a reason to wait.
        let backend = Backend::default();
        let device = Device::new("mac");
        let (nonce, ciphertext) = crate::crypto::encrypt_row(b"forged", &key(), "forged").unwrap();
        let mut forged =
            CloudItem::from_sealed_b64("forged", ciphertext, nonce, "text", 1_000, "ghost");
        forged.sign_epoch(&key(), 999);
        backend.items.write(vec![forged]);
        backend.items.write(vec![crate::sync::fakes::cloud_row(
            "real", 2_000, "genuine",
        )]);
        backend.envelopes.lock().unwrap().push(SpaceEnvelope {
            epoch: 999,
            sender: "ghost".into(),
            recipient: "mac".into(),
            nonce: "0".repeat(48),
            ciphertext: "forged".into(),
        });

        let stats = driver(&backend).sync(&device).await.unwrap();

        assert_eq!(stats.applied, 1);
        assert_eq!(device.text("real").as_deref(), Some("genuine"));
        assert_eq!(device.text("forged"), None);
    }

    #[tokio::test]
    async fn removing_a_device_rotates_the_key_for_the_remaining_members_only() {
        let backend = Backend::default();
        let (a, b, c) = (
            Device::new("mac"),
            Device::new("android"),
            Device::new("windows"),
        );
        let (da, db, dc) = (driver(&backend), driver(&backend), driver(&backend));
        let devices = [(&a, &da), (&b, &db), (&c, &dc)];
        for _ in 0..2 {
            for (device, sync) in devices {
                sync.sync(device).await.unwrap();
            }
        }
        a.copy("before", 1_000, "shared with everyone");
        da.sync(&a).await.unwrap();
        db.sync(&b).await.unwrap();
        dc.sync(&c).await.unwrap();
        assert_eq!(c.text("before").as_deref(), Some("shared with everyone"));
        assert_eq!(a.node.peers().space_info().unwrap().epoch, 0);

        // The Mac removes the Windows device.
        a.node.revoke(&pairing_of(&a, "windows"), 10).unwrap();
        da.sync(&a).await.unwrap();
        assert_eq!(a.node.peers().space_info().unwrap().epoch, 1);
        // The removed device gets only a notice, readable with the old key.
        let notice = backend
            .envelopes
            .lock()
            .unwrap()
            .iter()
            .find(|e| e.recipient == "windows" && e.epoch == 1)
            .cloned()
            .unwrap();
        assert!(crate::crypto::decrypt_row(
            &notice.ciphertext,
            &notice.nonce,
            &key(),
            &notice_aad(1, "windows")
        )
        .is_ok());
        db.sync(&b).await.unwrap();
        assert_eq!(b.node.peers().space_info().unwrap().epoch, 1);
        assert!(b
            .node
            .peers()
            .list()
            .iter()
            .all(|p| p.device_id.as_deref() != Some("windows")));

        // New data reaches the remaining member and never the removed one.
        b.copy("after", 2_000, "only for current members");
        db.sync(&b).await.unwrap();
        da.sync(&a).await.unwrap();
        assert_eq!(a.text("after").as_deref(), Some("only for current members"));
        let _ = dc.sync(&c).await;
        assert_eq!(c.text("after"), None);
        let row = backend.items.rows.lock().unwrap()["after"].clone();
        assert_eq!(row.key_epoch(), Some(1));
        assert!(row.verify_epoch(&key()).is_err());
        assert!(
            crate::crypto::decrypt_row(&row.ciphertext, &row.nonce, &key(), &row.item_id).is_err()
        );

        // The removed device learns of its removal from a notice under the
        // key it still holds, and forgets that key.
        assert!(!c.node.peers().group_active());
        assert!(c.node.peers().space_key().is_none());
        assert!(c.node.peers().is_empty());

        // What it might still write under the old key is refused.
        c.copy("forged", 3_000, "from the removed device");
        let _ = dc.sync(&c).await;
        let forged = backend.items.rows.lock().unwrap().get("forged").cloned();
        if forged.is_none() {
            let mut row = CloudItem::from_sealed_b64(
                "forged",
                crate::crypto::encrypt_row(b"forged", &key(), "forged")
                    .unwrap()
                    .1,
                crate::crypto::encrypt_row(b"forged", &key(), "forged")
                    .unwrap()
                    .0,
                "text/plain",
                3_000,
                "windows",
            );
            row.sign_epoch(&key(), 0);
            backend.items.write(vec![row]);
        }
        da.sync(&a).await.unwrap();
        db.sync(&b).await.unwrap();
        assert_eq!(a.text("forged"), None);
        assert_eq!(b.text("forged"), None);

        // A round without changes reads no envelopes.
        da.sync(&a).await.unwrap();
        let reads = backend
            .envelope_reads
            .load(std::sync::atomic::Ordering::Relaxed);
        da.sync(&a).await.unwrap();
        assert_eq!(
            backend
                .envelope_reads
                .load(std::sync::atomic::Ordering::Relaxed),
            reads
        );

        // Every remaining member is confirmed on the new key.
        assert!(a.node.peers().space_info().unwrap().waiting.is_empty());
        assert!(b.node.peers().space_info().unwrap().waiting.is_empty());
    }
}
