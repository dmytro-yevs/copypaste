//! The host half of encrypted cloud file storage. A cloud provider module
//! uploads kept copies made here and fetches files this device could not get
//! from their origin; the bytes stay sealed outside this process.

use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::time::{Duration, Instant};

use copypaste_core::file_download::{CloudChunk, CloudFileSource};
use copypaste_core::file_transfer::{self, CHUNK_BYTES};
use copypaste_core::{Keyring, Store};
use copypaste_sync::host::{CloudFileFetch, CloudFileUpload};

use super::file_storage::{files_upload_allowed, FileJobs};
use crate::Settings;

/// Largest file kept in cloud storage (`copypaste_cloud::crypto::file`).
const MAX_CLOUD_FILE_BYTES: u64 = 64 * 1024 * 1024;
/// Waiting downloads are offered to the cloud again at most this often.
const RETRY_INTERVAL_MS: i64 = 5 * 60 * 1000;
/// A stalled consumer ends the fetch instead of blocking the provider.
const DELIVERY_TIMEOUT: Duration = Duration::from_secs(60);
const QUEUED_CHUNKS: usize = 8;

struct Active {
    item_id: String,
    file_index: u32,
    size_bytes: u64,
    sender: tokio::sync::mpsc::Sender<CloudChunk>,
}

pub(crate) struct CloudFiles {
    store: Store,
    keyring: Arc<Keyring>,
    settings: Arc<Settings>,
    device_id: String,
    jobs: Weak<FileJobs>,
    active: Mutex<Option<Active>>,
    wake: OnceLock<Box<dyn Fn() + Send + Sync>>,
    last_retry: AtomicI64,
}

impl CloudFiles {
    pub(crate) fn new(
        store: Store,
        keyring: Arc<Keyring>,
        settings: Arc<Settings>,
        device_id: String,
        jobs: &Arc<FileJobs>,
    ) -> Self {
        Self {
            store,
            keyring,
            settings,
            device_id,
            jobs: Arc::downgrade(jobs),
            active: Mutex::default(),
            wake: OnceLock::new(),
            last_retry: AtomicI64::new(0),
        }
    }

    /// How a fetch reaches the provider: the module host's sync wake.
    pub(crate) fn set_wake(&self, wake: impl Fn() + Send + Sync + 'static) {
        let _ = self.wake.set(Box::new(wake));
    }

    fn active(&self) -> std::sync::MutexGuard<'_, Option<Active>> {
        self.active
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Hands one message to the waiting download without blocking any reactor.
    fn deliver(&self, item_id: &str, file: u32, message: CloudChunk) -> Result<(), String> {
        let sender = match self.active().as_ref() {
            Some(active) if active.item_id == item_id && active.file_index == file => {
                active.sender.clone()
            }
            _ => return Err("The file is no longer requested.".into()),
        };
        let deadline = Instant::now() + DELIVERY_TIMEOUT;
        let mut message = message;
        loop {
            match sender.try_send(message) {
                Ok(()) => return Ok(()),
                Err(tokio::sync::mpsc::error::TrySendError::Full(returned))
                    if Instant::now() < deadline =>
                {
                    message = returned;
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(_) => {
                    self.active().take();
                    return Err("The download stopped.".into());
                }
            }
        }
    }
}

impl CloudFileSource for CloudFiles {
    fn fetch(
        &self,
        id: &str,
        index: usize,
        size: u64,
    ) -> Option<tokio::sync::mpsc::Receiver<CloudChunk>> {
        if size > MAX_CLOUD_FILE_BYTES || self.store.sync_provider_ids().ok()?.is_empty() {
            return None;
        }
        let (sender, receiver) = tokio::sync::mpsc::channel(QUEUED_CHUNKS);
        {
            let mut active = self.active();
            if active.is_some() {
                return None;
            }
            *active = Some(Active {
                item_id: id.to_owned(),
                file_index: u32::try_from(index).ok()?,
                size_bytes: size,
                sender,
            });
        }
        tracing::info!("cloud file download started");
        if let Some(wake) = self.wake.get() {
            wake();
        }
        Some(receiver)
    }

    fn cancel(&self, id: &str) {
        let mut active = self.active();
        if active.as_ref().is_some_and(|active| active.item_id == id) {
            active.take();
        }
    }
}

impl copypaste_modules::CloudFileHost for CloudFiles {
    fn uploads(&self, since_ms: i64, limit: usize) -> Result<Vec<CloudFileUpload>, String> {
        let settings = self.settings.config();
        if !files_upload_allowed(&settings) {
            return Ok(Vec::new());
        }
        let cap = MAX_CLOUD_FILE_BYTES
            .min(settings.capture_limit_bytes(copypaste_ipc::content_type::FILE));
        let rows = self
            .store
            .local_file_copies_since(&self.device_id, since_ms, limit)
            .map_err(|_| "History is unavailable.".to_string())?;
        let mut uploads = Vec::new();
        for row in rows
            .into_iter()
            .filter(|row| row.clipboard_privacy().allows(&settings))
        {
            for (index, blob) in self.store.file_copies(&row.id).unwrap_or_default() {
                let Ok(file_index) = u32::try_from(index) else {
                    continue;
                };
                let Ok(Some(offer)) =
                    file_transfer::member_offer(&row, &self.keyring.item_key(), file_index)
                else {
                    continue;
                };
                if offer.size_bytes <= cap {
                    uploads.push(CloudFileUpload {
                        item_id: row.id.clone(),
                        file_index,
                        size_bytes: offer.size_bytes,
                        sha256: blob,
                        created_at: row.created_at,
                    });
                }
            }
        }
        Ok(uploads)
    }

    fn read(&self, item_id: &str, file: u32, offset: u64, len: usize) -> Result<Vec<u8>, String> {
        let unavailable = || "The file is unavailable.".to_string();
        if !offset.is_multiple_of(CHUNK_BYTES as u64) {
            return Err(unavailable());
        }
        let size = self
            .store
            .file_references(item_id)
            .ok()
            .and_then(|members| members.get(file as usize).map(|(_, size, _)| *size))
            .ok_or_else(unavailable)?;
        let mut bytes = Vec::with_capacity(len);
        let mut chunk = offset / CHUNK_BYTES as u64;
        while bytes.len() < len && chunk * (CHUNK_BYTES as u64) < size {
            let plaintext = copypaste_core::file_reference::kept_copy_chunk(
                &self.store,
                &self.keyring.item_key(),
                item_id,
                file as usize,
                chunk,
            )
            .map_err(|_| unavailable())?;
            bytes.extend_from_slice(&plaintext);
            chunk += 1;
        }
        bytes.truncate(len);
        Ok(bytes)
    }

    fn gone(&self, item_ids: &[String]) -> Vec<String> {
        item_ids
            .iter()
            .filter(|id| {
                !matches!(self.store.get(id), Ok(Some(row)) if !row.deleted)
                    || self
                        .store
                        .file_copies(id)
                        .map_or(true, |copies| copies.is_empty())
            })
            .cloned()
            .collect()
    }

    fn fetches(&self) -> Vec<CloudFileFetch> {
        if let Some(active) = self.active().as_ref() {
            return vec![CloudFileFetch {
                item_id: active.item_id.clone(),
                file_index: active.file_index,
                size_bytes: active.size_bytes,
            }];
        }
        // Each provider round may have brought the objects a waiting
        // download needs; offer those downloads again, rarely.
        let now = copypaste_core::now_ms();
        let last = self.last_retry.load(Ordering::Relaxed);
        if now.saturating_sub(last) >= RETRY_INTERVAL_MS
            && self
                .last_retry
                .compare_exchange(last, now, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
        {
            if let Some(jobs) = self.jobs.upgrade() {
                jobs.retry_waiting();
            }
        }
        Vec::new()
    }

    fn chunk(&self, item_id: &str, file: u32, bytes: Vec<u8>) -> Result<(), String> {
        self.deliver(item_id, file, CloudChunk::Bytes(bytes))
    }

    fn done(&self, item_id: &str, file: u32, result: Result<String, String>) {
        match &result {
            Ok(_) => tracing::info!("cloud file download finished"),
            Err(_) => tracing::warn!("cloud file download failed"),
        }
        let _ = self.deliver(
            item_id,
            file,
            match result {
                Ok(sha256) => CloudChunk::Done(sha256),
                Err(reason) => CloudChunk::Failed(reason),
            },
        );
        self.cancel(item_id);
    }
}
