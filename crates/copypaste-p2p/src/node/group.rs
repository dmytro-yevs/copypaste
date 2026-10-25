//! The transport-independent membership seam and P2P roster exchange.
use super::{Node, NodeError, NoiseChannel};
use crate::{
    group::GroupSnapshot,
    protocol::{SyncMessage, PROTOCOL_VERSION},
    sync::{SyncChannel, SyncSource},
};

impl Node {
    pub(super) fn current_peer(&self, peer: &crate::Peer) -> Option<crate::Peer> {
        self.peers.get(&peer.pairing_id).or_else(|| {
            let id = peer.device_id.as_deref()?;
            self.peers
                .list()
                .into_iter()
                .find(|current| current.device_id.as_deref() == Some(id))
        })
    }
    /// One membership refresh may precede a content connection. Its distinct
    /// key and server-enforced scope never authorize clipboard or file access.
    pub(super) async fn connect_peer(
        &self,
        addr: std::net::SocketAddr,
        peer: &crate::Peer,
    ) -> Result<(crate::Session, crate::Peer), NodeError> {
        let Some(mut current) = self.current_peer(peer) else {
            // Explicit content-free probe callers may supply an ephemeral
            // credential. Content operations still require a persisted lease.
            let session = crate::Session::connect(addr, &peer.psk)
                .await
                .map_err(|_| NodeError::Handshake)?;
            self.note_authenticated(&peer.pairing_id);
            return Ok((session, peer.clone()));
        };
        for epoch_refresh in 0..=1 {
            let active = self
                .begin_active_sync(&current.pairing_id)
                .ok_or(NodeError::NoPeer)?;
            let cancelled = active.cancelled();
            let connected = tokio::select! {
                biased;
                _ = cancelled.cancelled() => None,
                result = self.connect_peer_once(addr,&current) => Some(result),
            };
            drop(active);
            if let Some(result) = connected {
                return result;
            }
            let updated = self
                .current_peer(&current)
                .filter(|peer| peer.pairing_id != current.pairing_id);
            match (epoch_refresh, updated) {
                (0, Some(peer)) => current = peer,
                _ => return Err(NodeError::Cancelled),
            }
        }
        Err(NodeError::Cancelled)
    }
    async fn connect_peer_once(
        &self,
        addr: std::net::SocketAddr,
        peer: &crate::Peer,
    ) -> Result<(crate::Session, crate::Peer), NodeError> {
        let peer = self.current_peer(peer).unwrap_or_else(|| peer.clone());
        if let Ok(session) = crate::Session::connect(addr, &peer.psk).await {
            self.note_authenticated(&peer.pairing_id);
            return Ok((session, peer));
        }
        let key = peer
            .device_id
            .as_deref()
            .and_then(|id| self.peers.metadata_key(id))
            .ok_or(NodeError::Handshake)?;
        let session = crate::Session::connect_membership(addr, &key)
            .await
            .map_err(|_| NodeError::Handshake)?;
        self.note_authenticated(&peer.pairing_id);
        let mut channel = NoiseChannel::new(session);
        let refreshed = tokio::time::timeout(super::SESSION_TIMEOUT, async {
            let snapshot = self.peers.stored_group().map_err(group_error)?;
            channel
                .send(SyncMessage::Group {
                    protocol_version: PROTOCOL_VERSION,
                    snapshot,
                })
                .await
                .map_err(super::dial::session_error)?;
            let SyncMessage::Group { snapshot, .. } =
                channel.recv().await.map_err(super::dial::session_error)?
            else {
                return Err(NodeError::InvalidData);
            };
            self.apply_group(&snapshot, Some(&peer.pairing_id), false)
                .map_err(group_error)?;
            channel.wait_for_close().await;
            Ok::<_, NodeError>(())
        })
        .await
        .map_err(|_| NodeError::Timeout)
        .and_then(|result| result);
        channel.close().await;
        refreshed?;
        let peer = self.current_peer(&peer).ok_or(NodeError::NoPeer)?;
        let session = crate::Session::connect(addr, &peer.psk)
            .await
            .map_err(|_| NodeError::Handshake)?;
        Ok((session, peer))
    }
    pub fn prepare_group<S: SyncSource>(&self, source: &S) -> Result<GroupSnapshot, NodeError> {
        self.peers
            .group_snapshot(
                &source.device_id(),
                &source.device_name(),
                self.group_profile
                    .get_or_init(|| source.device_profile())
                    .clone(),
                self.listen_addr().and_then(|addr| addr.parse().ok()),
            )
            .map_err(group_error)
    }
    pub fn export_group<S: SyncSource>(&self, source: &S) -> Result<Option<String>, NodeError> {
        let snapshot = self.prepare_group(source)?;
        if !self.peers.group_active() {
            return Ok(None);
        }
        snapshot.encode().map(Some).map_err(|_| {
            NodeError::Diagnostic(copypaste_ipc::sync_issue::SyncIssueCode::LocalProtocolError)
        })
    }
    /// The cloud provider calls this only after account authentication and
    /// ciphertext authentication with the existing sync passphrase.
    pub fn import_cloud_group(
        &self,
        device_id: &str,
        json: &str,
    ) -> Result<bool, crate::group::GroupImportError> {
        use crate::{group::GroupImportError, PeerStoreError};
        let snapshot = GroupSnapshot::decode(json).map_err(|_| GroupImportError::Rejected)?;
        if snapshot.roster.issuer != device_id {
            return Err(GroupImportError::Rejected);
        }
        self.apply_group(&snapshot, None, false)
            .map_err(|error| match error {
                PeerStoreError::Invalid(_) | PeerStoreError::Revoked => GroupImportError::Rejected,
                _ => GroupImportError::Persistence,
            })
    }
    fn apply_group(
        &self,
        snapshot: &GroupSnapshot,
        pairing_id: Option<&str>,
        explicit: bool,
    ) -> Result<bool, crate::PeerStoreError> {
        self.change_group(|| self.peers.merge_group(snapshot, pairing_id, explicit))
    }
    pub(super) fn confirm_group_admission(&self, pairing_id: &str) -> Result<(), NodeError> {
        self.change_group(|| {
            self.peers.confirm_group_admission(pairing_id)?;
            Ok(true)
        })
        .map(|_| ())
        .map_err(group_error)
    }
    fn change_group(
        &self,
        update: impl FnOnce() -> Result<bool, crate::PeerStoreError>,
    ) -> Result<bool, crate::PeerStoreError> {
        let (changed, sessions, old) = {
            // Keep membership projection and new-session admission serialized
            // with the same lock used by local unpair/revoke.
            let active = self.active_syncs.lock().unwrap_or_else(|e| e.into_inner());
            let old = self.peers.list();
            let changed = update()?;
            let mut sessions = Vec::new();
            for peer in &old {
                if self.peers.get(&peer.pairing_id).is_none() {
                    for session in Self::cancel_active_syncs_locked(&active, &peer.pairing_id) {
                        session.cancel();
                        sessions.push(session);
                    }
                }
            }
            (changed, sessions, old)
        };
        // Retain the original states: a cancelled lease may leave the
        // registry before its already-running blocking source operation ends.
        for session in sessions {
            session.drain();
        }
        for peer in old {
            if self.peers.get(&peer.pairing_id).is_none() {
                self.forget_peer_state(&peer.pairing_id);
            }
        }
        if changed {
            self.republish();
        }
        Ok(changed)
    }
    pub(super) async fn initiate_group<S: SyncSource>(
        &self,
        channel: &mut NoiseChannel,
        source: &S,
        pairing_id: &str,
        explicit: bool,
    ) -> Result<(), NodeError> {
        channel
            .send(SyncMessage::Group {
                protocol_version: PROTOCOL_VERSION,
                snapshot: self.prepare_group(source)?,
            })
            .await
            .map_err(super::dial::session_error)?;
        let SyncMessage::Group { snapshot, .. } =
            channel.recv().await.map_err(super::dial::session_error)?
        else {
            return Err(NodeError::InvalidData);
        };
        self.apply_group(&snapshot, Some(pairing_id), explicit)
            .map_err(group_error)?;
        self.sync_activity.evidence(
            pairing_id,
            copypaste_ipc::sync_issue::SyncEvidence::GroupAccepted,
        );
        if !self.peers.group_active() {
            return Err(NodeError::NoPeer);
        }
        Ok(())
    }
    pub(super) async fn respond_group<S: SyncSource>(
        &self,
        channel: &mut NoiseChannel,
        source: &S,
        pairing_id: &str,
        snapshot: GroupSnapshot,
        explicit: bool,
    ) -> Result<(), NodeError> {
        self.prepare_group(source)?;
        self.apply_group(&snapshot, Some(pairing_id), explicit)
            .map_err(group_error)?;
        self.sync_activity.evidence(
            pairing_id,
            copypaste_ipc::sync_issue::SyncEvidence::GroupAccepted,
        );
        // The response includes new admissions and removals learned above.
        channel
            .send(SyncMessage::Group {
                protocol_version: PROTOCOL_VERSION,
                snapshot: self.prepare_group(source)?,
            })
            .await
            .map_err(super::dial::session_error)?;
        if !self.peers.group_active() {
            return Err(NodeError::NoPeer);
        }
        Ok(())
    }
}

fn group_error(error: crate::PeerStoreError) -> NodeError {
    use copypaste_ipc::sync_issue::SyncIssueCode as Code;
    let code = match error {
        crate::PeerStoreError::Invalid(_) | crate::PeerStoreError::Revoked => {
            Code::MembershipRejected
        }
        crate::PeerStoreError::Io(error) => match error.kind() {
            std::io::ErrorKind::StorageFull => Code::StorageFull,
            std::io::ErrorKind::PermissionDenied => Code::StoragePermissionDenied,
            _ => Code::MembershipUnavailable,
        },
        crate::PeerStoreError::Corrupt
        | crate::PeerStoreError::Encrypt
        | crate::PeerStoreError::AlreadyPaired
        | crate::PeerStoreError::TooManyPairings
        | crate::PeerStoreError::TooManyRevocations
        | crate::PeerStoreError::Contended
        | crate::PeerStoreError::Poisoned => Code::MembershipUnavailable,
    };
    NodeError::Diagnostic(code)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        sync::testutil::{item, TestSource},
        PairingToken, Peer, PeerStore,
    };
    use std::sync::Arc;
    use tokio::{net::TcpListener, sync::watch};

    struct Device {
        _dir: tempfile::TempDir,
        node: Arc<Node>,
        source: Arc<TestSource>,
        shutdown: watch::Sender<bool>,
        listener: tokio::task::JoinHandle<()>,
    }
    impl Device {
        async fn new(id: &str) -> Self {
            let dir = tempfile::tempdir().unwrap();
            let node = Arc::new(Node::new(
                PeerStore::open(&dir.path().join("peers.json"), &[7; 32]).unwrap(),
                None,
                0,
                true,
            ));
            let source = Arc::new(TestSource::new(id, vec![]));
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            node.set_listen_addr(listener.local_addr().unwrap());
            let (shutdown, rx) = watch::channel(false);
            let task = tokio::spawn(super::super::listen(
                Arc::clone(&node),
                listener,
                Arc::clone(&source),
                |_, _| {},
                || {},
                rx,
            ));
            Self {
                _dir: dir,
                node,
                source,
                shutdown,
                listener: task,
            }
        }
        async fn sync(&self, other: &Self) {
            let peer = self
                .node
                .peers()
                .list()
                .into_iter()
                .find(|p| p.device_id.as_deref() == Some(&other.source.device_id()))
                .unwrap();
            self.node
                .sync_one(&peer, self.source.as_ref())
                .await
                .unwrap();
        }
        async fn stop(self) {
            let _ = self.shutdown.send(true);
            self.listener.await.unwrap();
        }
    }
    fn old_pair(a: &Device, b: &Device) {
        let token = PairingToken::generate();
        for (local, remote) in [(a, b), (b, a)] {
            local
                .node
                .peers()
                .upsert(Peer {
                    pairing_id: token.pairing_id(),
                    device_id: Some(remote.source.device_id()),
                    name: remote.source.device_name(),
                    psk: token.psk(),
                    last_addr: remote.node.listen_addr().unwrap().parse().ok(),
                    last_seen_ms: 0,
                    profile: None,
                    profile_observed_at_ms: 0,
                })
                .unwrap();
        }
    }

    async fn fresh_pair(a: &Device, b: &Device) {
        let invite = b.node.pair_create_invite().unwrap();
        a.node
            .pair_join(
                &invite.code,
                &b.node.listen_addr().unwrap(),
                a.source.as_ref(),
            )
            .await
            .unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            while b.node.pair_progress().phase != crate::PairingPhase::AwaitingConfirmation {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        a.node.pair_confirm(true).unwrap();
        b.node.pair_confirm(true).unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            while a.node.pair_progress().phase != crate::PairingPhase::Confirmed
                || b.node.pair_progress().phase != crate::PairingPhase::Confirmed
            {
                assert_ne!(a.node.pair_progress().phase, crate::PairingPhase::Failed);
                assert_ne!(b.node.pair_progress().phase, crate::PairingPhase::Failed);
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn imported_removal_drains_source_work_even_after_its_session_lease_leaves_the_registry()
    {
        let a = Device::new("a").await;
        let b = Device::new("b").await;
        old_pair(&a, &b);
        a.sync(&b).await;
        let id = b.node.peers().list()[0].pairing_id.clone();
        let active = b.node.begin_active_sync(&id).unwrap();
        let cancelled = active.cancelled();
        let operation = active.state.enter_operation().unwrap();
        a.node.unpair(&id).unwrap();
        let snapshot = a
            .node
            .prepare_group(a.source.as_ref())
            .unwrap()
            .encode()
            .unwrap();
        let receiver = Arc::clone(&b.node);
        let applying =
            tokio::task::spawn_blocking(move || receiver.import_cloud_group("a", &snapshot));
        tokio::time::timeout(std::time::Duration::from_secs(2), cancelled.cancelled())
            .await
            .unwrap();
        drop(active);
        assert!(
            !applying.is_finished(),
            "membership removal acknowledged before admitted source work drained"
        );
        drop(operation);
        assert!(applying.await.unwrap().unwrap());
        assert!(b.node.peers().is_empty());
        a.stop().await;
        b.stop().await;
    }

    #[tokio::test]
    async fn legacy_chain_becomes_a_group_and_syncs_without_the_middle_device() {
        let mac = Device::new("mac").await;
        let android = Device::new("android").await;
        let windows = Device::new("windows").await;
        old_pair(&mac, &android);
        let old_key = mac.node.peers().list()[0].psk;
        mac.sync(&android).await;
        old_pair(&android, &windows);
        android.sync(&windows).await;
        mac.sync(&android).await;
        assert_eq!(mac.node.peers().len(), 2);
        assert_eq!(windows.node.peers().len(), 2);
        assert!(mac
            .node
            .peers()
            .list()
            .iter()
            .any(|p| p.device_id.as_deref() == Some("android") && p.psk_matches(&old_key)));
        android.stop().await;
        mac.source
            .apply(item("direct", 100, "without Android", "mac"))
            .unwrap();
        mac.sync(&windows).await;
        assert_eq!(
            windows.source.get("direct").unwrap().content,
            "without Android"
        );

        // Keys and group identity survive reopening, without another ceremony.
        let reopened = PeerStore::open(&mac._dir.path().join("peers.json"), &[7; 32]).unwrap();
        assert_eq!(reopened.len(), 2);
        let m = reopened
            .list()
            .into_iter()
            .find(|p| p.device_id.as_deref() == Some("windows"))
            .unwrap();
        let w = windows
            .node
            .peers()
            .list()
            .into_iter()
            .find(|p| p.device_id.as_deref() == Some("mac"))
            .unwrap();
        assert_eq!(m.pairing_id, w.pairing_id);
        assert!(m.psk_matches(&w.psk));
        mac.stop().await;
        windows.stop().await;
    }

    #[tokio::test]
    async fn offline_membership_replay_cannot_restore_a_removed_device() {
        let a = Device::new("a").await;
        let b = Device::new("b").await;
        let c = Device::new("c").await;
        old_pair(&a, &b);
        a.sync(&b).await;
        old_pair(&b, &c);
        b.sync(&c).await;
        a.sync(&b).await;
        let stale_b = b
            .node
            .prepare_group(b.source.as_ref())
            .unwrap()
            .encode()
            .unwrap();
        let removed = a
            .node
            .peers()
            .list()
            .into_iter()
            .find(|p| p.device_id.as_deref() == Some("c"))
            .unwrap();
        a.node.unpair(&removed.pairing_id).unwrap();
        a.sync(&b).await;
        assert_eq!(b.node.peers().len(), 1);
        a.node.import_cloud_group("b", &stale_b).unwrap();
        assert_eq!(a.node.peers().len(), 1);
        let stale_c = c
            .node
            .prepare_group(c.source.as_ref())
            .unwrap()
            .encode()
            .unwrap();
        assert!(a.node.import_cloud_group("c", &stale_c).is_err());
        let removed_b = b
            .node
            .prepare_group(b.source.as_ref())
            .unwrap()
            .encode()
            .unwrap();
        c.node.import_cloud_group("b", &removed_b).unwrap();
        assert!(!c.node.peers().group_active());
        assert_eq!(c.node.peers().len(), 0);
        a.stop().await;
        b.stop().await;
        c.stop().await;
    }

    #[tokio::test]
    async fn forged_roster_changes_neither_membership_nor_transport_keys() {
        let a = Device::new("a").await;
        let b = Device::new("b").await;
        old_pair(&a, &b);
        a.sync(&b).await;
        let mut forged = b.node.prepare_group(b.source.as_ref()).unwrap();
        forged.roster.removed.insert("a".into(), 0);
        assert!(a
            .node
            .import_cloud_group("b", &forged.encode().unwrap())
            .is_err());
        assert!(a.node.peers().group_active());
        assert_eq!(a.node.peers().len(), 1);
        a.stop().await;
        b.stop().await;
    }

    #[tokio::test]
    async fn fresh_sas_readmission_rotates_the_removed_generation_without_restoring_old_credentials(
    ) {
        let a = Device::new("a").await;
        let b = Device::new("b").await;
        old_pair(&a, &b);
        a.sync(&b).await;
        let old = a.node.peers().list()[0].pairing_id.clone();
        a.node.revoke(&old, 1).unwrap();
        let removal = a
            .node
            .prepare_group(a.source.as_ref())
            .unwrap()
            .encode()
            .unwrap();
        b.node.import_cloud_group("a", &removal).unwrap();
        assert!(!b.node.peers().group_active());
        fresh_pair(&a, &b).await;
        a.sync(&b).await;
        assert!(a.node.peers().group_active());
        assert!(b.node.peers().group_active());
        assert_eq!(a.node.peers().len(), 1);
        assert_eq!(b.node.peers().len(), 1);
        assert!(a.node.peers().get(&old).is_none());
        assert_eq!(
            a.node
                .prepare_group(a.source.as_ref())
                .unwrap()
                .roster
                .members["b"]
                .admission,
            1
        );
        assert_eq!(
            b.node
                .prepare_group(b.source.as_ref())
                .unwrap()
                .roster
                .members["b"]
                .admission,
            1
        );
        a.stop().await;
        b.stop().await;
    }
    #[tokio::test]
    async fn offline_member_rotates_a_legacy_credential_after_another_members_readmission() {
        let a = Device::new("mac").await;
        let b = Device::new("android").await;
        let c = Device::new("windows").await;
        old_pair(&a, &b);
        a.sync(&b).await;
        old_pair(&b, &c);
        b.sync(&c).await;
        a.sync(&b).await;
        let old = a
            .node
            .peers()
            .list()
            .into_iter()
            .find(|peer| peer.device_id.as_deref() == Some("android"))
            .unwrap();
        let removed = c
            .node
            .peers()
            .list()
            .into_iter()
            .find(|peer| peer.device_id.as_deref() == Some("android"))
            .unwrap();
        c.node.revoke(&removed.pairing_id, 1).unwrap();
        let removal = c
            .node
            .prepare_group(c.source.as_ref())
            .unwrap()
            .encode()
            .unwrap();
        b.node.import_cloud_group("windows", &removal).unwrap();
        fresh_pair(&b, &c).await;
        b.sync(&c).await;
        // Mac missed both removal and SAS. Windows remains its authenticated
        // bridge and introduces the newly certified Android generation.
        a.sync(&c).await;
        assert!(a.node.peers().get(&old.pairing_id).is_none());
        a.source
            .apply(item("after-readmission", 200, "restored group", "mac"))
            .unwrap();
        a.sync(&b).await;
        assert_eq!(
            b.source.get("after-readmission").unwrap().content,
            "restored group"
        );
        a.stop().await;
        b.stop().await;
        c.stop().await;
    }

    #[tokio::test]
    async fn membership_only_channel_heals_rotated_epochs_without_an_unchanged_bridge() {
        let a = Device::new("mac").await;
        let b = Device::new("android").await;
        let c = Device::new("windows").await;
        old_pair(&a, &b);
        a.sync(&b).await;
        old_pair(&b, &c);
        b.sync(&c).await;
        a.sync(&b).await;
        let bc = b
            .node
            .peers()
            .list()
            .into_iter()
            .find(|peer| peer.device_id.as_deref() == Some("windows"))
            .unwrap();
        let cb = c
            .node
            .peers()
            .list()
            .into_iter()
            .find(|peer| peer.device_id.as_deref() == Some("android"))
            .unwrap();
        b.node.unpair(&bc.pairing_id).unwrap();
        c.node.unpair(&cb.pairing_id).unwrap();
        fresh_pair(&b, &c).await;
        b.sync(&c).await;
        a.source
            .apply(item("rotated", 300, "all local", "mac"))
            .unwrap();
        // Both of Mac's old content keys have changed. Only the independently
        // scoped membership credential can recover the current epochs.
        a.sync(&b).await;
        a.sync(&c).await;
        assert_eq!(b.source.get("rotated").unwrap().content, "all local");
        assert_eq!(c.source.get("rotated").unwrap().content, "all local");
        a.stop().await;
        b.stop().await;
        c.stop().await;
    }

    #[tokio::test]
    async fn membership_credentials_never_authorize_file_access_or_a_revoked_member() {
        let a = Device::new("a").await;
        let b = Device::new("b").await;
        old_pair(&a, &b);
        a.sync(&b).await;
        let key = a.node.peers().metadata_key("b").unwrap();
        let addr = b.node.listen_addr().unwrap().parse().unwrap();
        let mut session = crate::Session::connect_membership(addr, &key)
            .await
            .unwrap();
        let request = SyncMessage::FileRequest {
            protocol_version: PROTOCOL_VERSION,
            item_id: "secret-file".into(),
            sha256: String::new(),
            size: 1,
            index: 0,
        };
        request.validate().unwrap();
        session.send(&request).await.unwrap();
        assert!(session.recv::<SyncMessage>().await.unwrap().is_none());
        assert!(
            crate::Session::connect(addr, &key).await.is_err(),
            "membership key must not match a content PSK"
        );
        b.node
            .revoke(&b.node.peers().list()[0].pairing_id, 1)
            .unwrap();
        assert!(crate::Session::connect_membership(addr, &key)
            .await
            .is_err());
        a.stop().await;
        b.stop().await;
    }
}
