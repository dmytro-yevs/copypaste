//! A [`Backend`] that answers from memory, for the tests in this crate.
//!
//! It exists so the supervisor and the command layer can be tested without a
//! socket or a database. Only the operations those tests exercise return
//! anything interesting; the rest refuse, loudly enough that a test which
//! reached one by accident fails rather than passing on a default.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use copypaste_ipc::{
    BackupData, ConfigApplied, ConfigPatch, DiscoveredDevice, EventData, ExportData, ExportItem,
    ImagePreview, ImportData, Item, PairingInviteData, PairingProgressData, PeerInfo,
    PrivateModeData, StatusData, SyncResult,
};

use super::{Backend, BackendError, Page, PairingBackend, Result};

/// What the fake should do when asked for `status`.
#[derive(Debug, Clone)]
enum Liveness {
    /// Nothing is listening.
    Unreachable,
    /// A daemon reporting this version.
    Running(String),
    /// Something answered, badly.
    Failing,
}

#[derive(Debug)]
pub struct FakeBackend {
    liveness: Liveness,
    shutdown: AtomicBool,
    page: Mutex<Page>,
    /// `None` when `add` refuses, which is the default: a test that stored
    /// something by accident should fail rather than pass on a stub.
    added: Option<Mutex<Vec<String>>>,
}

impl FakeBackend {
    fn with(liveness: Liveness) -> Self {
        Self {
            liveness,
            shutdown: AtomicBool::new(false),
            page: Mutex::new(Page::default()),
            added: None,
        }
    }

    pub fn unreachable() -> Self {
        Self::with(Liveness::Unreachable)
    }

    pub fn running(version: &str) -> Self {
        Self::with(Liveness::Running(version.to_string()))
    }

    pub fn failing() -> Self {
        Self::with(Liveness::Failing)
    }

    /// Seed what `list` and `search` return.
    pub fn with_page(self, page: Page) -> Self {
        *self.page.lock().unwrap() = page;
        self
    }

    /// Let `add` succeed, recording what it was given.
    ///
    /// For `crate::capture::intake`, which has to be tested against a backend
    /// that stores — the Android one does not yet, and that refusal is the
    /// thing being tested around.
    pub fn accepting_adds(mut self) -> Self {
        self.added = Some(Mutex::new(Vec::new()));
        self
    }

    /// What `add` was given, oldest first.
    pub fn added(&self) -> Vec<String> {
        self.added
            .as_ref()
            .map(|a| a.lock().unwrap().clone())
            .unwrap_or_default()
    }
}

fn refused() -> BackendError {
    BackendError::Unsupported("The fake backend does not do that.")
}

impl Backend for FakeBackend {
    async fn list(&self, _limit: u32, _cursor: Option<&str>) -> Result<Page> {
        Ok(self.page.lock().unwrap().clone())
    }

    async fn search(&self, _query: &str, _limit: u32) -> Result<Page> {
        Ok(self.page.lock().unwrap().clone())
    }

    async fn add(&self, content: &str) -> Result<Item> {
        let Some(added) = self.added.as_ref() else {
            return Err(refused());
        };
        let mut added = added.lock().unwrap();
        added.push(content.to_string());
        Ok(Item {
            id: format!("item-{}", added.len()),
            content: content.to_string(),
            content_type: "text/plain".into(),
            created_at: 0,
            pinned: false,
            // The detector lives behind the backend, so the fake stands in for
            // it with the one rule the intake tests need: anything containing
            // this marker is treated as a secret.
            is_sensitive: content.contains("AKIA"),
            sensitive_finding: None,
            origin_device_id: "fake-device".into(),
            origin_device_name: None,
            source_app_bundle_id: None,
            source_app_name: None,
            too_large_to_sync: false,
            truncated: false,
        })
    }

    async fn get(&self, id: &str) -> Result<Item> {
        let page = self.page.lock().unwrap();
        page.items
            .iter()
            .find(|item| item.id == id)
            .cloned()
            .ok_or_else(refused)
    }

    async fn image_preview(&self, _id: &str) -> Result<ImagePreview> {
        Err(refused())
    }

    async fn copy(&self, _id: &str) -> Result<Item> {
        Err(refused())
    }

    async fn copy_as_plain_text(&self, _id: &str) -> Result<Item> {
        Err(refused())
    }

    async fn delete(&self, _id: &str) -> Result<()> {
        Err(refused())
    }

    async fn clear(&self, _through: Option<i64>) -> Result<u64> {
        Err(refused())
    }

    async fn history_ceiling(&self) -> Result<u64> {
        Err(refused())
    }

    async fn set_pinned(&self, _id: &str, _pinned: bool) -> Result<Item> {
        Err(refused())
    }

    async fn reorder_pinned(&self, _ids: &[String]) -> Result<()> {
        Err(refused())
    }

    async fn status(&self) -> Result<StatusData> {
        if self.shutdown.load(Ordering::Relaxed) {
            return Err(BackendError::Unreachable);
        }
        match &self.liveness {
            Liveness::Unreachable => Err(BackendError::Unreachable),
            Liveness::Failing => Err(BackendError::NotReady),
            Liveness::Running(version) => Ok(StatusData {
                device_name: "Test device".into(),
                version: version.clone(),
                protocol_version: copypaste_ipc::PROTOCOL_VERSION,
                item_count: 0,
                capture_running: true,
                clipboard_backend: "fake".into(),
                private_mode: false,
                private_mode_epoch: 0,
                counters: copypaste_ipc::DiagnosticCounters::default(),
                settings_health: None,
            }),
        }
    }

    async fn set_device_name(&self, _name: &str) -> Result<()> {
        Ok(())
    }

    async fn cloud_sign_in(
        &self,
        _email: &str,
        _password: &str,
        _passphrase: &str,
    ) -> Result<copypaste_ipc::CloudStatusData> {
        Err(refused())
    }

    async fn cloud_sign_out(&self) -> Result<copypaste_ipc::CloudStatusData> {
        Err(refused())
    }

    async fn cloud_status(&self) -> Result<copypaste_ipc::CloudStatusData> {
        Err(refused())
    }

    async fn cloud_sync(&self) -> Result<copypaste_ipc::CloudSyncData> {
        Err(refused())
    }

    async fn shutdown(&self) -> Result<()> {
        self.shutdown.store(true, Ordering::Relaxed);
        Ok(())
    }

    async fn peers(&self) -> Result<Vec<PeerInfo>> {
        Err(refused())
    }

    async fn unpair(&self, _pairing_id: &str) -> Result<()> {
        Err(refused())
    }

    async fn revoke(&self, _pairing_id: &str) -> Result<()> {
        Err(refused())
    }

    async fn sync(&self, _pairing_id: Option<&str>) -> Result<Vec<SyncResult>> {
        Err(refused())
    }

    async fn discovered(&self) -> Result<Vec<DiscoveredDevice>> {
        Err(refused())
    }

    async fn rescan(&self) -> Result<Vec<DiscoveredDevice>> {
        Err(refused())
    }

    async fn get_config(&self) -> Result<ConfigApplied> {
        Err(refused())
    }

    async fn set_config(&self, _patch: ConfigPatch) -> Result<ConfigApplied> {
        Err(refused())
    }

    async fn get_private_mode(&self) -> Result<PrivateModeData> {
        Ok(PrivateModeData {
            private_mode: false,
            private_mode_epoch: 0,
        })
    }

    async fn set_private_mode(&self, _enabled: bool) -> Result<PrivateModeData> {
        Err(refused())
    }

    async fn export(&self, _limit: u32, _include_sensitive: bool) -> Result<ExportData> {
        Err(refused())
    }

    async fn import(&self, _items: Vec<ExportItem>) -> Result<ImportData> {
        Err(refused())
    }

    async fn backup(&self, _dest: &Path) -> Result<BackupData> {
        Err(refused())
    }

    async fn restore(&self, _src: &Path) -> Result<()> {
        Err(refused())
    }

    async fn watch(&self) -> Result<tokio::sync::mpsc::Receiver<EventData>> {
        Err(refused())
    }
}

impl PairingBackend for FakeBackend {
    async fn pair_create_invite(&self) -> Result<PairingInviteData> {
        Err(refused())
    }

    async fn pair_join(&self, _code: &str, _addr: &str) -> Result<PairingProgressData> {
        Err(refused())
    }

    async fn pair_progress(&self) -> Result<PairingProgressData> {
        Err(refused())
    }

    async fn pair_confirm(&self, _accept: bool) -> Result<PairingProgressData> {
        Err(refused())
    }

    async fn pair_cancel(&self) -> Result<PairingProgressData> {
        Err(refused())
    }
}
