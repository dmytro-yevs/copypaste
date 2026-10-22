//! Membership uses the existing account/key, recovery policy and sync cadence.
//! Full roster anti-entropy does not depend on clipboard cursors or retention.
use super::{AuthApi, CloudSource, CloudSync, RestApi, SyncError};
use crate::{
    crypto::{decrypt_row, encrypt_row},
    rest::{
        group::{GROUP_CATALOG_PAGE_SIZE, GROUP_PAGE_SIZE, MAX_GROUP_BYTES, MAX_GROUP_ROWS},
        GroupRow,
    },
};
use sha2::{Digest, Sha256};

pub(super) struct GroupUpload {
    digest: [u8; 32],
    ciphertext_digest: String,
}

impl<R: RestApi, A: AuthApi> CloudSync<R, A> {
    pub(super) async fn sync_group(&self, source: &dyn CloudSource) -> Result<(), SyncError> {
        if source.group_snapshot()?.is_none() {
            return Ok(());
        }
        // Pull removals before publishing local state or uploading history.
        let mut after: Option<String> = None;
        let mut count = 0;
        let local_id = source.device_id();
        let mut local_hash = None;
        loop {
            let versions = self
                .execute(|token| {
                    let after = after.clone();
                    async move {
                        self.rest
                            .fetch_group_catalog(&token, after.as_deref())
                            .await
                    }
                })
                .await?;
            if versions.len() > GROUP_CATALOG_PAGE_SIZE {
                return Err(SyncError::Source("invalid device group page"));
            }
            let full = versions.len() == GROUP_CATALOG_PAGE_SIZE;
            let mut changed = Vec::new();
            for version in versions {
                count += 1;
                if count > MAX_GROUP_ROWS
                    || after.as_ref().is_some_and(|id| version.device_id <= *id)
                    || version.validate().is_err()
                {
                    return Err(SyncError::Source("device group page did not advance"));
                }
                after = Some(version.device_id.clone());
                if version.device_id == local_id {
                    local_hash = Some(version.ciphertext_digest);
                    continue;
                }
                if self
                    .group_downloaded
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .get(&version.device_id)
                    == Some(&version.ciphertext_digest)
                {
                    continue;
                }
                changed.push(version.device_id);
            }
            self.download_groups(source, &changed).await?;
            if !full {
                break;
            }
        }
        if !source.group_sync_allowed() {
            return Err(SyncError::Source("this device was removed from the group"));
        }
        let Some(snapshot) = source.group_snapshot()? else {
            return Ok(());
        };
        if snapshot.len() > MAX_GROUP_BYTES {
            return Err(SyncError::Source("device group is too large"));
        }
        let digest: [u8; 32] = Sha256::digest(snapshot.as_bytes()).into();
        if self
            .group_uploaded
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .is_some_and(|uploaded| {
                uploaded.digest == digest
                    && local_hash.as_ref() == Some(&uploaded.ciphertext_digest)
            })
        {
            return Ok(());
        }
        let mut row = GroupRow {
            device_id: local_id,
            ciphertext: String::new(),
            nonce: String::new(),
        };
        let (nonce, ciphertext) = encrypt_row(snapshot.as_bytes(), &self.key, &row.aad_id())
            .map_err(|_| SyncError::Source("device group could not be encrypted"))?;
        row.nonce = nonce;
        row.ciphertext = ciphertext;
        let outgoing = &row;
        self.execute(|token| async move { self.rest.upsert_group(&token, outgoing).await })
            .await?;
        *self
            .group_uploaded
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = Some(GroupUpload {
            digest,
            ciphertext_digest: row.ciphertext_digest(),
        });
        Ok(())
    }

    async fn download_groups(
        &self,
        source: &dyn CloudSource,
        changed: &[String],
    ) -> Result<(), SyncError> {
        for ids in changed.chunks(GROUP_PAGE_SIZE) {
            let rows = self
                .execute(|token| async move { self.rest.fetch_groups(&token, ids).await })
                .await?;
            if rows.len() > GROUP_PAGE_SIZE || rows.iter().any(|row| !ids.contains(&row.device_id))
            {
                return Err(SyncError::Source("invalid device group page"));
            }
            for row in rows {
                if row.validate().is_err() {
                    continue;
                }
                // Authentication failures are isolated. Storage failures reach
                // the caller and are never cached as a successful application.
                if let Ok(plaintext) =
                    decrypt_row(&row.ciphertext, &row.nonce, &self.key, &row.aad_id())
                {
                    if plaintext.len() <= MAX_GROUP_BYTES {
                        if let Ok(json) = std::str::from_utf8(&plaintext) {
                            source.apply_group(&row.device_id, json)?;
                            if !source.group_sync_allowed() {
                                return Err(SyncError::Source(
                                    "this device was removed from the group",
                                ));
                            }
                        }
                    }
                }
                let mut downloaded = self
                    .group_downloaded
                    .lock()
                    .unwrap_or_else(|e| e.into_inner());
                if downloaded.len() >= MAX_GROUP_ROWS && !downloaded.contains_key(&row.device_id) {
                    downloaded.pop_first();
                }
                let ciphertext_digest = row.ciphertext_digest();
                downloaded.insert(row.device_id, ciphertext_digest);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sync::{
        fakes::{config, key, session, FakeAuth},
        transport::TransportFault,
    };
    use copypaste_p2p::{DeviceProfile, Node, PeerStore};
    use copypaste_sync::{Applied, LocalItem};
    use std::{
        collections::BTreeMap,
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc, Mutex,
        },
    };

    #[derive(Clone, Default)]
    struct Backend {
        rows: Arc<Mutex<BTreeMap<String, GroupRow>>>,
        writes: Arc<AtomicUsize>,
        downloads: Arc<AtomicUsize>,
    }
    impl RestApi for Backend {
        async fn fetch_since(
            &self,
            _: &str,
            _: i64,
            _: Option<&str>,
            _: u32,
        ) -> Result<Vec<crate::rest::CloudItem>, TransportFault> {
            panic!("membership must not read clipboard rows")
        }
        async fn upsert(
            &self,
            _: &str,
            _: &[crate::rest::CloudItem],
        ) -> Result<(), TransportFault> {
            panic!("membership must not write clipboard rows")
        }
        async fn fetch_group_catalog(
            &self,
            _: &str,
            after: Option<&str>,
        ) -> Result<Vec<crate::rest::group::GroupVersion>, TransportFault> {
            Ok(self
                .rows
                .lock()
                .unwrap()
                .values()
                .filter(|row| after.is_none_or(|id| row.device_id.as_str() > id))
                .take(GROUP_CATALOG_PAGE_SIZE)
                .map(|row| crate::rest::group::GroupVersion {
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
            self.downloads.fetch_add(1, Ordering::Relaxed);
            Ok(self
                .rows
                .lock()
                .unwrap()
                .values()
                .filter(|row| ids.contains(&row.device_id))
                .cloned()
                .collect())
        }
        async fn upsert_group(&self, _: &str, row: &GroupRow) -> Result<(), TransportFault> {
            self.writes.fetch_add(1, Ordering::Relaxed);
            self.rows
                .lock()
                .unwrap()
                .insert(row.device_id.clone(), row.clone());
            Ok(())
        }
    }
    struct Source {
        _dir: tempfile::TempDir,
        node: Node,
        id: String,
        fail_storage: std::sync::atomic::AtomicBool,
    }
    impl Source {
        fn new(id: &str) -> Self {
            let dir = tempfile::tempdir().unwrap();
            let node = Node::new(
                PeerStore::open(&dir.path().join("peers.json"), &[7; 32]).unwrap(),
                None,
                0,
                true,
            );
            Self {
                _dir: dir,
                node,
                id: id.into(),
                fail_storage: std::sync::atomic::AtomicBool::new(false),
            }
        }
    }
    impl CloudSource for Source {
        fn group_snapshot(&self) -> Result<Option<String>, SyncError> {
            if !self.node.peers().group_active() {
                return Ok(None);
            }
            self.node
                .peers()
                .group_snapshot(&self.id, &self.id, DeviceProfile::current(), None)
                .unwrap()
                .encode()
                .map(Some)
                .map_err(|_| SyncError::Source("group unavailable"))
        }
        fn apply_group(&self, id: &str, snapshot: &str) -> Result<(), SyncError> {
            if self.fail_storage.load(Ordering::Relaxed) {
                return Err(SyncError::Source("group storage unavailable"));
            }
            self.node
                .import_cloud_group(id, snapshot)
                .or_else(|error| match error {
                    copypaste_p2p::group::GroupImportError::Rejected => Ok(false),
                    error => Err(error),
                })
                .map(|_| ())
                .map_err(|_| SyncError::Source("invalid group"))
        }
        fn group_sync_allowed(&self) -> bool {
            self.node.peers().group_active()
        }
        fn device_id(&self) -> String {
            self.id.clone()
        }
        fn local_changes_since(&self, _: i64) -> Result<Vec<LocalItem>, SyncError> {
            unreachable!()
        }
        fn apply_remote(&self, _: LocalItem) -> Result<Applied, SyncError> {
            unreachable!()
        }
        fn watermark(&self) -> Result<i64, SyncError> {
            unreachable!()
        }
        fn set_watermark(&self, _: i64) -> Result<(), SyncError> {
            unreachable!()
        }
    }

    #[tokio::test]
    async fn encrypted_cloud_membership_converges_without_lan_and_stops_an_excluded_device() {
        assert_eq!(MAX_GROUP_BYTES, copypaste_p2p::group::MAX_GROUP_BYTES);
        let backend = Backend::default();
        let make_driver = || {
            CloudSync::new(
                backend.clone(),
                FakeAuth::default(),
                key(),
                config(),
                session("token"),
            )
        };
        let (a, b, c) = (
            Source::new("mac"),
            Source::new("android"),
            Source::new("windows"),
        );
        let (da, db, dc) = (make_driver(), make_driver(), make_driver());
        da.sync_group(&a).await.unwrap();
        db.sync_group(&b).await.unwrap();
        dc.sync_group(&c).await.unwrap();
        da.sync_group(&a).await.unwrap();
        db.sync_group(&b).await.unwrap();
        for source in [&a, &b, &c] {
            assert_eq!(source.node.peers().len(), 2);
        }
        let ap = a
            .node
            .peers()
            .list()
            .into_iter()
            .find(|p| p.device_id.as_deref() == Some("windows"))
            .unwrap();
        let cp = c
            .node
            .peers()
            .list()
            .into_iter()
            .find(|p| p.device_id.as_deref() == Some("mac"))
            .unwrap();
        assert_eq!(ap.pairing_id, cp.pairing_id);
        assert!(ap.psk_matches(&cp.psk));
        for row in backend.rows.lock().unwrap().values() {
            assert!(!row.ciphertext.contains("signing_key"));
            assert!(!row.ciphertext.contains("exchange_key"));
        }
        let writes = backend.writes.load(Ordering::Relaxed);
        da.sync_group(&a).await.unwrap();
        assert_eq!(
            backend.writes.load(Ordering::Relaxed),
            writes,
            "unchanged group must not write to the backend"
        );
        let downloads = backend.downloads.load(Ordering::Relaxed);
        da.sync_group(&a).await.unwrap();
        assert_eq!(
            backend.downloads.load(Ordering::Relaxed),
            downloads,
            "unchanged catalogs must not download membership payloads"
        );
        a.node.revoke(&ap.pairing_id, 10).unwrap();
        da.sync_group(&a).await.unwrap();
        db.sync_group(&b).await.unwrap();
        assert!(dc.sync_group(&c).await.is_err());
        assert!(!c.node.peers().group_active());
        assert_eq!(c.node.peers().len(), 0);
        assert_eq!(b.node.peers().len(), 1);
    }

    #[tokio::test]
    async fn own_writer_is_repaired_after_account_only_tampering_or_deletion() {
        let backend = Backend::default();
        let a = Source::new("mac");
        let b = Source::new("android");
        let make = || {
            CloudSync::new(
                backend.clone(),
                FakeAuth::default(),
                key(),
                config(),
                session("token"),
            )
        };
        let da = make();
        let db = make();
        da.sync_group(&a).await.unwrap();
        db.sync_group(&b).await.unwrap();
        da.sync_group(&a).await.unwrap();
        db.sync_group(&b).await.unwrap();
        let before = a.group_snapshot().unwrap();
        let original = backend.rows.lock().unwrap().get("mac").unwrap().clone();
        {
            let mut rows = backend.rows.lock().unwrap();
            let row = rows.get_mut("mac").unwrap();
            row.ciphertext = "AAAA".into();
        }
        db.sync_group(&b).await.unwrap();
        let writes = backend.writes.load(Ordering::Relaxed);
        da.sync_group(&a).await.unwrap();
        assert_eq!(
            a.group_snapshot().unwrap(),
            before,
            "repair needs no local group change"
        );
        assert_eq!(backend.writes.load(Ordering::Relaxed), writes + 1);
        let repaired = backend.rows.lock().unwrap().get("mac").unwrap().clone();
        assert_ne!(repaired.nonce, original.nonce);
        db.sync_group(&b).await.unwrap();
        assert_eq!(
            db.group_downloaded.lock().unwrap().get("mac"),
            Some(&repaired.ciphertext_digest()),
            "a running account observes the repair without a restart"
        );
        backend.rows.lock().unwrap().remove("mac");
        da.sync_group(&a).await.unwrap();
        assert!(backend.rows.lock().unwrap().contains_key("mac"));
        let writes = backend.writes.load(Ordering::Relaxed);
        da.sync_group(&a).await.unwrap();
        assert_eq!(
            backend.writes.load(Ordering::Relaxed),
            writes,
            "healthy unchanged state stays write-free"
        );
    }

    #[tokio::test]
    async fn membership_storage_failure_stops_the_round_instead_of_being_skipped() {
        let backend = Backend::default();
        let a = Source::new("a");
        let b = Source::new("b");
        let da = CloudSync::new(
            backend.clone(),
            FakeAuth::default(),
            key(),
            config(),
            session("token"),
        );
        let db = CloudSync::new(
            backend.clone(),
            FakeAuth::default(),
            key(),
            config(),
            session("token"),
        );
        da.sync_group(&a).await.unwrap();
        b.fail_storage.store(true, Ordering::Relaxed);
        assert_eq!(
            db.sync_group(&b).await,
            Err(SyncError::Source("group storage unavailable"))
        );
        assert_eq!(backend.writes.load(Ordering::Relaxed), 1);
        assert_eq!(b.node.peers().len(), 0);
    }
}
