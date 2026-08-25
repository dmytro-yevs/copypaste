//! Android's in-process `Backend` adapter.
//!
//! Android has no background daemon, so the core store, peer, and cloud
//! services run in the app process (ADR-0002, ADR-0003). Stable operations live
//! in sibling modules; this module only maps them onto the product trait.
//!
//! Host tests use the file keystore. Android's Keystore binding remains a
//! native-runner assertion.

mod backup;
mod clear;
mod cloud;
mod items;
mod messages;
mod open;
mod pairing;
mod peers;
mod retention;
mod rows;
mod settings;
mod state;
mod transfer;

use std::path::Path;

use super::{Backend, BackendError, CaptureWrite, Page, Result};
use copypaste_core::p2p_contract;
use copypaste_ipc::{
    BackupData, ConfigApplied, ConfigPatch, DiscoveredDevice, EventData, ExportData, ExportItem,
    ImagePreview, ImportData, Item, PeerInfo, PrivateModeData, StatusData, SyncResult,
};
use messages::MSG_NO_PEER;
pub use open::{Clipboard, EmbeddedBackend};

impl Backend for EmbeddedBackend {
    async fn list(&self, limit: u32, cursor: Option<&str>) -> Result<Page> {
        items::list(self, limit, cursor).await
    }

    async fn search(&self, query: &str, limit: u32) -> Result<Page> {
        items::search(self, query, limit).await
    }

    async fn add(&self, content: &str) -> Result<Item> {
        items::add(self, content).await
    }

    async fn add_captured(
        &self,
        content: &str,
        source: crate::capture::model::CaptureSource,
        app_bundle_id: Option<&str>,
        app_name: Option<&str>,
    ) -> Result<Option<CaptureWrite>> {
        items::add_captured(self, content, source, app_bundle_id, app_name).await
    }

    async fn get(&self, id: &str) -> Result<Item> {
        items::get(self, id).await
    }

    async fn image_preview(&self, id: &str) -> Result<ImagePreview> {
        items::image_preview(self, id).await
    }

    async fn copy(&self, id: &str) -> Result<Item> {
        items::copy(self, id).await
    }

    async fn copy_as_plain_text(&self, id: &str) -> Result<Item> {
        items::copy_plain_text(self, id).await
    }

    async fn delete(&self, id: &str) -> Result<()> {
        items::delete(self, id).await
    }

    async fn history_ceiling(&self) -> Result<u64> {
        clear::ceiling(self).await
    }

    async fn clear(&self, through: Option<i64>) -> Result<u64> {
        clear::clear(self, through).await
    }

    async fn set_pinned(&self, id: &str, pinned: bool) -> Result<Item> {
        items::set_pinned(self, id, pinned).await
    }

    async fn reorder_pinned(&self, ids: &[String]) -> Result<()> {
        items::reorder_pinned(self, ids).await
    }

    async fn status(&self) -> Result<StatusData> {
        // The supervisor probes `status` at launch, and this is what makes that
        // probe start the peer listener: without it a paired device could only
        // reach this one while a peer screen happened to be open. A node that
        // will not come up is logged and stepped over — history still works.
        if let Err(e) = self.node().await {
            tracing::warn!(error = %e, "the peer node did not start");
        }
        let listen_addr = self.inner.node.get().and_then(peers::PeerNode::listen_addr);
        let mut status = self.blocking(rows::status_of).await?;
        status.listen_addr = listen_addr;
        status.device_details = Some(p2p_contract::local_device_details(
            &status.device_name,
            status.listen_addr.as_deref(),
        ));
        Ok(status)
    }

    async fn set_device_name(&self, name: &str) -> Result<()> {
        let name = name.to_string();
        let stored = self
            .blocking(move |inner| inner.state.set_device_name(&name))
            .await?;
        if let Some(node) = self.inner.node.get() {
            node.set_device_name(&stored);
        }
        Ok(())
    }

    async fn cloud_sign_in(
        &self,
        email: &str,
        password: &str,
        passphrase: &str,
    ) -> Result<copypaste_ipc::CloudStatusData> {
        self.inner
            .cloud
            .sign_in(&self.inner, email, password, passphrase)
            .await
    }

    async fn cloud_sign_up(
        &self,
        email: &str,
        password: &str,
        passphrase: &str,
    ) -> Result<copypaste_ipc::CloudStatusData> {
        self.inner
            .cloud
            .sign_up(&self.inner, email, password, passphrase)
            .await
    }

    async fn cloud_set_endpoint(
        &self,
        url: &str,
        anon_key: &str,
    ) -> Result<copypaste_ipc::CloudStatusData> {
        self.inner
            .cloud
            .set_endpoint(&self.inner, url, anon_key)
            .await
    }

    async fn cloud_sign_out(&self) -> Result<copypaste_ipc::CloudStatusData> {
        self.inner.cloud.sign_out(&self.inner).await;
        Ok(self.inner.cloud.status())
    }

    async fn cloud_status(&self) -> Result<copypaste_ipc::CloudStatusData> {
        self.inner.cloud.ensure_poller(&self.inner);
        Ok(self.inner.cloud.status())
    }

    async fn cloud_sync(&self) -> Result<copypaste_ipc::CloudSyncData> {
        self.inner.cloud.sync_now(&self.inner).await
    }

    async fn shutdown(&self) -> Result<()> {
        self.inner.cloud.shutdown();
        Ok(())
    }

    /// Known peers, with a best-effort liveness flag from discovery.
    ///
    /// `online: false` means "not seen on the network", never "unreachable" — a
    /// device on a network without multicast is reachable by address and still
    /// reads as offline.
    async fn peers(&self) -> Result<Vec<PeerInfo>> {
        Ok(self.node().await?.peers())
    }

    /// Local and one-sided: the other device keeps its half until it also
    /// unpairs, which is why this cannot fail on an unreachable peer. What it
    /// does do is remove the pre-shared key from the listener's candidates.
    async fn unpair(&self, pairing_id: &str) -> Result<()> {
        match self.node().await?.unpair(pairing_id)? {
            true => Ok(()),
            false => Err(BackendError::NotFound(MSG_NO_PEER)),
        }
    }

    async fn revoke(&self, pairing_id: &str) -> Result<()> {
        self.node().await?.revoke(pairing_id)
    }

    async fn sync(&self, pairing_id: Option<&str>) -> Result<Vec<SyncResult>> {
        self.node().await?.sync(&self.inner, pairing_id).await
    }

    async fn discovered(&self) -> Result<Vec<DiscoveredDevice>> {
        Ok(self.node().await?.discovered())
    }

    /// Re-advertise and answer as [`Backend::discovered`] does.
    ///
    /// Best-effort, like every other discovery call: a republish that fails is
    /// logged and the current table is still returned, because what the user
    /// asked for was "show me what is out there".
    async fn rescan(&self) -> Result<Vec<DiscoveredDevice>> {
        let node = self.node().await?;
        node.republish();
        Ok(node.discovered())
    }

    async fn get_config(&self) -> Result<ConfigApplied> {
        settings::get(self).await
    }

    async fn set_config(&self, patch: ConfigPatch) -> Result<ConfigApplied> {
        settings::set(self, patch).await
    }

    async fn get_private_mode(&self) -> Result<PrivateModeData> {
        settings::get_private_mode(self).await
    }

    async fn set_private_mode(&self, enabled: bool) -> Result<PrivateModeData> {
        settings::set_private_mode(self, enabled).await
    }

    async fn export(&self, limit: u32, include_sensitive: bool) -> Result<ExportData> {
        self.blocking(move |inner| transfer::export(inner, limit, include_sensitive))
            .await
    }

    async fn import(&self, items: Vec<ExportItem>) -> Result<ImportData> {
        self.blocking(move |inner| {
            let imported = transfer::import(inner, items)?;
            if imported.inserted > 0 {
                inner.note_oldest_version(inner.state.store.oldest_version_ms().ok().flatten());
                inner.publish_items(false, 0);
            }
            Ok(imported)
        })
        .await
    }

    async fn backup(&self, dest: &Path) -> Result<BackupData> {
        backup::backup(self, dest).await
    }

    async fn restore(&self, src: &Path) -> Result<()> {
        backup::restore(self, src).await
    }

    /// Auto-wipe is the embedded backend's one asynchronous history writer, so
    /// its deletions use the same event contract as the daemon.
    async fn watch(&self) -> Result<tokio::sync::mpsc::Receiver<EventData>> {
        let mut source = self.inner.events.subscribe();
        let inner = std::sync::Arc::clone(&self.inner);
        let (sender, receiver) = tokio::sync::mpsc::channel(64);
        tokio::spawn(async move {
            loop {
                let event = match source.recv().await {
                    Ok(event) => event,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        inner.items_event(false, 0)
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                };
                if sender.send(event).await.is_err() {
                    break;
                }
            }
        });
        Ok(receiver)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::PairingBackend;
    use crate::capture::model::CaptureSource;
    use std::sync::{Arc, Mutex};

    /// Records what was written, so `copy` can be asserted without a system
    /// clipboard.
    #[derive(Default)]
    pub(super) struct FakeClipboard(Mutex<Vec<String>>);

    impl FakeClipboard {
        pub(super) fn entries(&self) -> Vec<String> {
            self.0.lock().unwrap().clone()
        }
    }

    impl Clipboard for Arc<FakeClipboard> {
        fn write(
            &self,
            payload: &copypaste_core::ClipboardPayload,
        ) -> std::result::Result<(), copypaste_core::ClipboardWriteError> {
            match payload {
                copypaste_core::ClipboardPayload::Text(text) => {
                    self.0.lock().unwrap().push(text.to_string());
                    Ok(())
                }
                _ => Err(copypaste_core::ClipboardWriteError::UnsupportedContent),
            }
        }
    }

    /// `pub(super)` so a submodule's tests build the same backend rather than a
    /// second fixture that could drift from this one.
    pub(super) fn backend() -> (EmbeddedBackend, Arc<FakeClipboard>, tempfile::TempDir) {
        let dir = tempfile::TempDir::new().unwrap();
        let clipboard = Arc::new(FakeClipboard::default());
        let backend = EmbeddedBackend::open(dir.path(), Box::new(Arc::clone(&clipboard)))
            .expect("the embedded backend should open under a temp dir");
        (backend, clipboard, dir)
    }

    #[test]
    fn it_opens_without_a_caller_tokio_runtime() {
        let (_backend, _clip, _dir) = backend();
    }

    #[tokio::test]
    async fn it_opens_and_reports_an_honest_status() {
        let (backend, _clip, _dir) = backend();
        let status = backend.status().await.unwrap();
        assert_eq!(status.item_count, 0);
        assert!(
            !status.capture_running,
            "there is no capture loop in this build"
        );
        assert_eq!(status.clipboard_backend, super::messages::BACKEND_NAME);
    }

    #[tokio::test]
    async fn an_empty_history_lists_and_searches_without_failing() {
        let (backend, _clip, _dir) = backend();
        assert!(backend.list(50, None).await.unwrap().items.is_empty());
        assert!(backend
            .search("anything", 20)
            .await
            .unwrap()
            .items
            .is_empty());
    }

    /// B-1 / `CopyPaste-8ebg.57` on the platform with no daemon behind it: an
    /// item captured between two pages must not make the second page repeat a
    /// row or skip one. Both backends answer the same command, so both have to
    /// hold the same property.
    #[tokio::test]
    async fn a_capture_between_two_pages_neither_repeats_nor_skips_a_row() {
        let (backend, _clip, _dir) = backend();
        let mut original = Vec::new();
        for n in 0..6 {
            original.push(backend.add(&format!("clip {n}")).await.unwrap().id);
        }

        let first = backend.list(2, None).await.unwrap();
        let mut next = first.next_cursor.clone();
        assert!(next.is_some(), "a full page has to say where it stopped");

        backend.add("arrived mid-scroll").await.unwrap();

        let mut seen: Vec<String> = first.items.iter().map(|i| i.id.clone()).collect();
        while let Some(cursor) = next {
            let page = backend.list(2, Some(&cursor)).await.unwrap();
            seen.extend(page.items.iter().map(|i| i.id.clone()));
            next = page.next_cursor;
        }

        for id in &original {
            assert_eq!(
                seen.iter().filter(|s| *s == id).count(),
                1,
                "an item was skipped or repeated across the pages"
            );
        }
    }

    #[tokio::test]
    async fn a_forged_page_marker_is_refused_rather_than_restarting_the_list() {
        let (backend, _clip, _dir) = backend();
        backend.add("one").await.unwrap();
        for bad in ["", "not-hex!", "abcdef"] {
            assert!(
                matches!(
                    backend.list(10, Some(bad)).await.unwrap_err(),
                    BackendError::Invalid(_)
                ),
                "accepted {bad:?}"
            );
        }
    }

    #[tokio::test]
    async fn an_unknown_id_is_not_found_rather_than_a_silent_success() {
        let (backend, _clip, _dir) = backend();
        assert!(matches!(
            backend.delete("nope").await.unwrap_err(),
            BackendError::NotFound(_)
        ));
        assert!(matches!(
            backend.set_pinned("nope", true).await.unwrap_err(),
            BackendError::NotFound(_)
        ));
        assert!(matches!(
            backend.get("nope").await.unwrap_err(),
            BackendError::NotFound(_)
        ));
    }

    /// The pinned section reads back in the order asked for, and the gesture
    /// survives an id that a sync round removed between the read and the drop
    /// — the case that would otherwise lose a reorder the user just made.
    #[tokio::test]
    async fn the_pinned_section_takes_the_order_it_is_given() {
        let (backend, _clip, _dir) = backend();
        let mut ids = Vec::new();
        for content in ["first", "second", "third"] {
            let item = backend.add(content).await.unwrap();
            backend.set_pinned(&item.id, true).await.unwrap();
            ids.push(item.id);
        }

        let reversed: Vec<String> = ids.iter().rev().cloned().collect();
        backend.reorder_pinned(&reversed).await.unwrap();
        let pinned: Vec<String> = backend
            .list(50, None)
            .await
            .unwrap()
            .items
            .into_iter()
            .filter(|item| item.pinned)
            .map(|item| item.id)
            .collect();
        assert_eq!(pinned, reversed);

        let mut stale = reversed.clone();
        stale.insert(0, "gone-in-a-sync-round".to_string());
        backend.reorder_pinned(&stale).await.unwrap();
        assert_eq!(pinned.len(), 3, "the gesture must not drop a live pin");
    }

    /// A missing device is `peer_not_found`, never the item-shaped `not_found`
    /// — the split `BackendError::from_code` makes, so the pairing screen can
    /// say "that device is gone" instead of "that item is gone".
    #[tokio::test]
    async fn an_unknown_peer_is_not_found() {
        let (backend, _clip, _dir) = backend();
        assert!(backend.peers().await.unwrap().is_empty());
        assert!(matches!(
            backend.unpair("nope").await.unwrap_err(),
            BackendError::NotFound(_)
        ));

        let err = backend.sync(Some("nope")).await.unwrap_err();
        assert_eq!(err.ui_error().code, "peer_not_found", "{err:?}");
        assert!(!err.ui_error().retryable, "{err:?}");
    }

    #[tokio::test]
    async fn the_embedded_backend_owns_a_cancellable_invite() {
        let (backend, _clip, _dir) = backend();
        let invite = backend.pair_create_invite().await.unwrap();
        assert_eq!(invite.expires_in_secs, 120);
        assert_eq!(
            backend.pair_progress().await.unwrap().state,
            copypaste_ipc::PairingState::WaitingForPeer
        );
        assert_eq!(
            backend.pair_cancel().await.unwrap().state,
            copypaste_ipc::PairingState::Cancelled
        );
        assert_ne!(
            backend.pair_create_invite().await.unwrap().pairing_id,
            invite.pairing_id
        );
    }

    #[tokio::test]
    async fn revoking_an_unknown_pairing_bars_later_enrolment() {
        let (backend, _clip, _dir) = backend();
        let token = copypaste_p2p::transport::PairingToken::generate();
        backend.revoke(&token.pairing_id()).await.unwrap();

        let store = copypaste_p2p::peers::PeerStore::open(&backend.inner.state.peers_path).unwrap();
        assert!(
            store
                .upsert(copypaste_p2p::peers::Peer {
                    pairing_id: token.pairing_id(),
                    name: "stolen phone".into(),
                    psk: token.psk(),
                    last_addr: None,
                    last_seen_ms: 1,
                })
                .is_err(),
            "a revoked pairing id was enrolled anyway"
        );
    }

    /// Discovery and sync answer rather than refuse, and an empty answer is a
    /// normal one — a container has no multicast and no peers.
    #[tokio::test]
    async fn discovery_and_sync_answer_with_nothing_rather_than_refusing() {
        let (backend, _clip, _dir) = backend();
        assert!(backend.sync(None).await.unwrap().is_empty());
        assert!(backend.discovered().await.unwrap().is_empty());
        assert!(backend.rescan().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn the_live_sync_switch_blocks_sync() {
        let (backend, _clip, _dir) = backend();
        backend
            .set_config(copypaste_ipc::ConfigPatch {
                sync_enabled: Some(false),
                ..Default::default()
            })
            .await
            .unwrap();

        assert!(matches!(
            backend.sync(None).await.unwrap_err(),
            BackendError::NotReady
        ));
    }

    /// A locally captured row stores no origin, and the wire item still has to
    /// name this device — the field is documented as never empty, and the merge
    /// tie-break reads the same substitution.
    #[tokio::test]
    async fn a_captured_item_is_attributed_to_this_device() {
        let (backend, _clip, _dir) = backend();
        let item = backend.add("mine").await.unwrap();
        assert!(!item.origin_device_id.is_empty());
        assert_eq!(
            item.origin_device_name.as_deref(),
            Some("CopyPaste phone"),
            "this device is in its own name registry"
        );

        let listed = backend.list(50, None).await.unwrap();
        assert_eq!(listed.items[0].origin_device_id, item.origin_device_id);
    }

    #[tokio::test]
    async fn a_renamed_identity_is_persisted_by_the_embedded_backend() {
        let (backend, _clip, _dir) = backend();
        let device_id = backend.inner.state.device_id.clone();
        backend.set_device_name("  Kitchen Phone  ").await.unwrap();

        let stored = backend
            .inner
            .state
            .store
            .device_identity("ignored hostname")
            .unwrap();
        let status = backend.status().await.unwrap();
        assert_eq!(stored.device_id, device_id);
        assert_eq!(stored.device_name, "Kitchen Phone");
        assert_eq!(status.device_name, "Kitchen Phone");
    }

    #[tokio::test]
    async fn the_embedded_backend_refuses_a_blank_device_name() {
        let (backend, _clip, _dir) = backend();
        let before = backend.status().await.unwrap().device_name;
        let error = backend.set_device_name(" \n ").await.unwrap_err();
        assert!(matches!(error, BackendError::Invalid(_)));
        assert_eq!(backend.status().await.unwrap().device_name, before);
    }

    /// Rung 0 has no value at all unless this works: the share sheet, the
    /// text-selection action and the tile all end here.
    #[tokio::test]
    async fn a_captured_clip_is_stored_searchable_and_readable_again() {
        let (backend, _clip, _dir) = backend();
        let item = backend.add("a shared note").await.unwrap();
        assert_eq!(item.content, "a shared note");

        assert_eq!(backend.list(50, None).await.unwrap().items.len(), 1);
        assert_eq!(backend.search("shared", 20).await.unwrap().items.len(), 1);
        assert_eq!(
            backend.get(&item.id).await.unwrap().content,
            "a shared note"
        );
    }

    #[tokio::test]
    async fn a_captured_clip_keeps_the_platform_reported_source_package() {
        let (backend, _clip, _dir) = backend();
        let item = backend
            .add_captured(
                "a note from another app",
                CaptureSource::Background,
                Some("com.example.writer"),
                Some("Writer"),
            )
            .await
            .unwrap()
            .expect("capture was enabled");
        assert_eq!(
            item.item.source_app_bundle_id.as_deref(),
            Some("com.example.writer")
        );
        assert_eq!(item.item.source_app_name.as_deref(), Some("Writer"));
    }

    #[tokio::test]
    async fn external_exclusions_fail_closed_without_blocking_explicit_intake() {
        let (backend, _clip, _dir) = backend();
        backend
            .set_config(ConfigPatch {
                excluded_app_bundle_ids: Some(vec!["com.example.private".into()]),
                ..ConfigPatch::default()
            })
            .await
            .unwrap();

        assert!(backend
            .add_captured(
                "do not retain",
                CaptureSource::Background,
                Some("com.example.private"),
                Some("Private")
            )
            .await
            .unwrap()
            .is_none());
        assert!(backend
            .add_captured(
                "unknown external source",
                CaptureSource::Background,
                None,
                None
            )
            .await
            .unwrap()
            .is_none());
        assert!(backend
            .add_captured("explicit share", CaptureSource::Share, None, None)
            .await
            .unwrap()
            .is_some());
        assert_eq!(backend.list(50, None).await.unwrap().items.len(), 1);
    }

    #[tokio::test]
    async fn private_mode_suppresses_embedded_capture_without_replay() {
        let (backend, _clip, _dir) = backend();
        let confirmed = backend.set_private_mode(true).await.unwrap();
        assert!(confirmed.private_mode);
        assert!(backend.status().await.unwrap().private_mode);
        let saved: copypaste_ipc::ConfigData =
            serde_json::from_slice(&std::fs::read(backend.inner.state.settings.path()).unwrap())
                .unwrap();
        assert!(saved.private_mode);
        assert!(backend
            .add_captured("not retained", CaptureSource::Background, None, None)
            .await
            .unwrap()
            .is_none());

        backend.set_private_mode(false).await.unwrap();
        assert!(backend.list(50, None).await.unwrap().items.is_empty());
        assert!(backend
            .add_captured("retained later", CaptureSource::Background, None, None)
            .await
            .unwrap()
            .is_some());
    }

    #[tokio::test]
    async fn concurrent_private_mode_and_config_patches_preserve_both_writes() {
        let (backend, _clip, _dir) = backend();
        let start = Arc::new(tokio::sync::Barrier::new(3));

        let mode_backend = backend.clone();
        let mode_start = Arc::clone(&start);
        let mode = tokio::spawn(async move {
            mode_start.wait().await;
            mode_backend.set_private_mode(true).await.unwrap()
        });

        let config_backend = backend.clone();
        let config_start = Arc::clone(&start);
        let config = tokio::spawn(async move {
            config_start.wait().await;
            config_backend
                .set_config(ConfigPatch {
                    poll_interval_ms: Some(250),
                    ..ConfigPatch::default()
                })
                .await
                .unwrap()
        });

        start.wait().await;
        let (mode, config) = tokio::join!(mode, config);
        assert!(mode.unwrap().private_mode);
        assert_eq!(config.unwrap().config.poll_interval_ms, 250);

        let current = backend.get_config().await.unwrap().config;
        assert!(current.private_mode);
        assert_eq!(current.poll_interval_ms, 250);
        let persisted: copypaste_ipc::ConfigData =
            serde_json::from_slice(&std::fs::read(backend.inner.state.settings.path()).unwrap())
                .unwrap();
        assert_eq!(persisted, current);
    }

    #[tokio::test]
    async fn private_mode_and_epoch_converge_across_reads_and_restart() {
        let (backend, _clipboard, _dir) = backend();
        let first = backend.set_private_mode(true).await.unwrap();
        let second = backend.set_private_mode(true).await.unwrap();
        assert_eq!(first.private_mode_epoch, 1);
        assert_eq!(second.private_mode_epoch, 2);

        let read = backend.get_private_mode().await.unwrap();
        let status = backend.status().await.unwrap();
        assert_eq!(read.private_mode_epoch, second.private_mode_epoch);
        assert_eq!(status.private_mode_epoch, second.private_mode_epoch);
        assert_eq!(status.private_mode, read.private_mode);

        let path = backend.inner.state.settings.path().to_path_buf();
        drop(backend);
        let restarted = settings::EmbeddedSettings::open(path).snapshot();
        assert!(restarted.config.private_mode);
        assert_eq!(restarted.private_mode_epoch, 0);
    }

    /// The same text twice is one row. Not because this file says so — the
    /// dedup probe inside `copypaste_core::ingest` says so, and that is the
    /// point of calling it rather than re-deriving it.
    #[tokio::test]
    async fn a_replayed_android_capture_is_one_encrypted_row() {
        let (backend, _clip, dir) = backend();
        let plaintext = "same Android tile capture";
        let first = backend
            .add_captured(plaintext, CaptureSource::Tile, None, None)
            .await
            .unwrap()
            .unwrap();
        let second = backend
            .add_captured(plaintext, CaptureSource::Tile, None, None)
            .await
            .unwrap()
            .unwrap();
        assert!(first.saved);
        assert!(!second.saved);
        assert_eq!(backend.list(50, None).await.unwrap().items.len(), 1);
        drop(backend);
        let database = std::fs::read(dir.path().join("copypaste-v2.db")).unwrap();
        assert!(!database
            .windows(plaintext.len())
            .any(|bytes| bytes == plaintext.as_bytes()));
    }

    /// AGENTS.md rule 4, the write-time layer: a detected secret is stored but
    /// never indexed, on this platform as on the other.
    #[tokio::test]
    async fn a_captured_secret_is_stored_and_stays_out_of_the_index() {
        let (backend, _clip, _dir) = backend();
        let item = backend.add("AKIAIOSFODNN7EXAMPLE").await.unwrap();
        assert!(item.is_sensitive, "the detector did not flag a known key");
        assert!(item.sensitive_finding.is_none());
        assert!(
            backend
                .search("AKIAIOSFODNN7EXAMPLE", 20)
                .await
                .unwrap()
                .items
                .is_empty(),
            "a sensitive item reached the search index"
        );
        // …and it is still the user's data: reachable by id, which is how the
        // reveal gesture gets to it.
        assert!(backend.get(&item.id).await.unwrap().is_sensitive);
    }

    #[tokio::test]
    async fn inert_findings_match_the_daemon_contract_and_stay_searchable() {
        let (backend, _clip, _dir) = backend();
        let item = backend
            .add("mail alice@example.com about the release")
            .await
            .unwrap();

        assert!(!item.is_sensitive);
        let finding = item.sensitive_finding.as_ref().unwrap();
        assert_eq!(finding.label, "email");
        assert_eq!(finding.spans.len(), 1);
        assert!(!finding.redacted_preview.contains("alice@example.com"));
        assert!(backend
            .search("alice", 20)
            .await
            .unwrap()
            .items
            .iter()
            .any(|found| found.id == item.id));
    }

    #[tokio::test]
    async fn an_empty_capture_is_refused_without_storing_anything() {
        let (backend, _clip, _dir) = backend();
        assert!(matches!(
            backend.add("   ").await.unwrap_err(),
            BackendError::Invalid(_)
        ));
        assert!(backend.list(50, None).await.unwrap().items.is_empty());
    }

    /// A settings screen here shows what this build actually runs on, and says
    /// nothing needs a restart — there is no start to be read at.
    #[tokio::test]
    async fn the_settings_are_readable_even_though_they_are_not_writable() {
        let (backend, _clip, _dir) = backend();
        let applied = backend.get_config().await.unwrap();
        assert_eq!(applied.config, copypaste_ipc::ConfigData::default());
        assert!(applied.restart_required.is_empty());
    }

    #[tokio::test]
    async fn clearing_an_empty_history_is_a_success_reporting_zero() {
        let (backend, _clip, _dir) = backend();
        assert_eq!(backend.clear(None).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn mutations_are_delivered_in_commit_order() {
        let (backend, _clip, _dir) = backend();
        let mut events = backend.watch().await.unwrap();
        let first = backend.add("ordered").await.unwrap();
        backend
            .add_captured("captured", CaptureSource::Background, None, None)
            .await
            .unwrap();
        backend.delete(&first.id).await.unwrap();

        let added = events.recv().await.unwrap();
        let captured = events.recv().await.unwrap();
        let deleted = events.recv().await.unwrap();
        assert_eq!(
            (added.event, added.item_count),
            (copypaste_ipc::EventKind::Items, 1)
        );
        assert_eq!(
            (captured.event, captured.item_count),
            (copypaste_ipc::EventKind::Items, 2)
        );
        assert_eq!(
            (deleted.event, deleted.item_count),
            (copypaste_ipc::EventKind::Items, 1)
        );
        assert!(!added.captured);
        assert!(captured.captured);
        assert!(!deleted.captured);
    }

    #[tokio::test]
    async fn lag_is_coalesced_into_the_current_item_count() {
        let (backend, _clip, _dir) = backend();
        let mut events = backend.watch().await.unwrap();
        for _ in 0..200 {
            let _ = backend.inner.events.send(EventData {
                event: copypaste_ipc::EventKind::Peers,
                item_count: u64::MAX,
                captured: true,
                swept: u32::MAX,
            });
        }

        loop {
            let event = tokio::time::timeout(std::time::Duration::from_secs(1), events.recv())
                .await
                .expect("the coalesced event stayed silent")
                .expect("the change stream closed");
            if event.item_count == 0 {
                assert_eq!(event.event, copypaste_ipc::EventKind::Items);
                assert!(!event.captured);
                assert_eq!(event.swept, 0);
                break;
            }
        }
    }

    #[tokio::test]
    async fn a_subscription_can_restart_after_its_receiver_is_dropped() {
        let (backend, _clip, _dir) = backend();
        drop(backend.watch().await.unwrap());

        let mut restarted = backend.watch().await.unwrap();
        backend.add("after restart").await.unwrap();
        let event = tokio::time::timeout(std::time::Duration::from_secs(1), restarted.recv())
            .await
            .expect("the restarted stream stayed silent")
            .expect("the restarted stream closed");
        assert_eq!(event.item_count, 1);
    }
}
