//! Background file work: Copy-mode copies of captured originals and automatic
//! downloads of files other devices made. Each kind runs one job at a time from
//! a bounded queue; no task exists while its queue is empty.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, OnceLock};

use copypaste_core::file_download::{FileDownloadRequest, FileDownloads};
use copypaste_core::file_transfer::FileTransferError;
use copypaste_core::{Keyring, Store};
use copypaste_ipc::{ConfigData, FileStorageMode};
use copypaste_p2p::Node;

use crate::Settings;

/// Longest each queue may grow; later work is dropped and logged.
const MAX_QUEUED: usize = 64;

#[derive(Default)]
struct Queues {
    copies: VecDeque<String>,
    downloads: VecDeque<String>,
    /// Downloads whose origin was unreachable, retried when it is seen again.
    waiting: VecDeque<(String, String)>,
    copying: bool,
    downloading: bool,
}

pub(crate) struct FileJobs {
    store: Store,
    keyring: Arc<Keyring>,
    settings: Arc<Settings>,
    downloads: Arc<FileDownloads>,
    node: Arc<Node>,
    device_id: String,
    queues: Mutex<Queues>,
    /// Lets hosts without an ambient reactor (Android JNI capture) spawn.
    handle: OnceLock<tokio::runtime::Handle>,
}

enum Outcome {
    Done,
    Wait(String),
}

/// Whether files may be fetched from `origin` under the per-device policy.
pub(crate) fn files_allowed_from(settings: &ConfigData, origin: &str) -> bool {
    settings.sync_allows(copypaste_ipc::content_type::FILE, &[origin])
}

/// Whether this device's files may go to cloud storage.
pub(crate) fn files_upload_allowed(settings: &ConfigData) -> bool {
    settings.sync_allows(copypaste_ipc::content_type::FILE, &[])
}

fn auto_download_on(settings: &ConfigData) -> bool {
    settings.auto_download_files && !settings.file_save_folder.is_empty()
}

impl FileJobs {
    pub(crate) fn new(
        store: Store,
        keyring: Arc<Keyring>,
        settings: Arc<Settings>,
        downloads: Arc<FileDownloads>,
        node: Arc<Node>,
        device_id: String,
    ) -> Self {
        Self {
            store,
            keyring,
            settings,
            downloads,
            node,
            device_id,
            queues: Mutex::default(),
            handle: OnceLock::new(),
        }
    }

    fn queues(&self) -> std::sync::MutexGuard<'_, Queues> {
        self.queues
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn spawn(
        self: &Arc<Self>,
        work: impl std::future::Future<Output = ()> + Send + 'static,
    ) -> bool {
        match tokio::runtime::Handle::try_current()
            .ok()
            .or_else(|| self.handle.get().cloned())
        {
            Some(handle) => {
                handle.spawn(work);
                true
            }
            None => false,
        }
    }

    /// Records the reactor and runs work queued before one was known.
    pub(crate) fn attach(self: &Arc<Self>, handle: tokio::runtime::Handle) {
        let _ = self.handle.set(handle);
        self.run_copies();
        self.run_downloads();
    }

    /// A file was captured or imported on this device.
    pub(crate) fn captured(self: &Arc<Self>, id: &str) {
        if self.settings.get().file_storage != FileStorageMode::Copy
            || !matches!(self.store.file_reference(id), Ok(Some(_)))
        {
            return;
        }
        if push(&mut self.queues().copies, id) {
            self.run_copies();
        }
    }

    /// Another device's file version was stored here.
    pub(crate) fn received(self: &Arc<Self>, id: &str) {
        if !auto_download_on(&self.settings.get()) {
            return;
        }
        if push(&mut self.queues().downloads, id) {
            self.run_downloads();
        }
    }

    /// A sync session with `device_id` succeeded; retry what waited on it.
    pub(crate) fn reachable(self: &Arc<Self>, device_id: &str) {
        let mut queues = self.queues();
        if queues.waiting.is_empty() {
            return;
        }
        let (ready, waiting): (VecDeque<_>, VecDeque<_>) = std::mem::take(&mut queues.waiting)
            .into_iter()
            .partition(|(origin, _)| origin == device_id);
        queues.waiting = waiting;
        for (_, id) in ready {
            push(&mut queues.downloads, &id);
        }
        drop(queues);
        self.run_downloads();
    }

    /// Offers every waiting download again, for a cloud copy may now exist.
    pub(crate) fn retry_waiting(self: &Arc<Self>) {
        let mut queues = self.queues();
        for (_, id) in std::mem::take(&mut queues.waiting) {
            push(&mut queues.downloads, &id);
        }
        drop(queues);
        self.run_downloads();
    }

    fn run_copies(self: &Arc<Self>) {
        {
            let mut queues = self.queues();
            if queues.copying || queues.copies.is_empty() {
                return;
            }
            queues.copying = true;
        }
        let jobs = Arc::clone(self);
        let spawned = self.spawn(async move {
            while let Some(id) =
                jobs.next(|queues| &mut queues.copies, |queues| &mut queues.copying)
            {
                let worker = Arc::clone(&jobs);
                let result = tokio::task::spawn_blocking(move || {
                    let settings = worker.settings.config();
                    if settings.file_storage != FileStorageMode::Copy {
                        return Ok(false);
                    }
                    copypaste_core::file_reference::keep_copy(
                        &worker.store,
                        &worker.keyring,
                        &id,
                        &settings,
                    )
                })
                .await;
                match result {
                    Ok(Ok(_)) => {}
                    Ok(Err(error)) => tracing::warn!(%error, "a captured file was kept as a link"),
                    Err(_) => tracing::warn!("a file copy worker stopped"),
                }
            }
        });
        if !spawned {
            self.queues().copying = false;
        }
    }

    fn run_downloads(self: &Arc<Self>) {
        {
            let mut queues = self.queues();
            if queues.downloading || queues.downloads.is_empty() {
                return;
            }
            queues.downloading = true;
        }
        let jobs = Arc::clone(self);
        let spawned = self.spawn(async move {
            while let Some(id) = jobs.next(
                |queues| &mut queues.downloads,
                |queues| &mut queues.downloading,
            ) {
                if let Outcome::Wait(origin) = jobs.download(&id).await {
                    let mut queues = jobs.queues();
                    if queues.waiting.len() >= MAX_QUEUED {
                        queues.waiting.pop_front();
                    }
                    if !queues.waiting.iter().any(|(_, queued)| *queued == id) {
                        queues.waiting.push_back((origin, id));
                    }
                }
            }
        });
        if !spawned {
            self.queues().downloading = false;
        }
    }

    /// The next queued id, or clears the running flag under the same lock.
    fn next(
        &self,
        queue: impl Fn(&mut Queues) -> &mut VecDeque<String>,
        running: impl Fn(&mut Queues) -> &mut bool,
    ) -> Option<String> {
        let mut queues = self.queues();
        let next = queue(&mut queues).pop_front();
        if next.is_none() {
            *running(&mut queues) = false;
        }
        next
    }

    async fn download(&self, id: &str) -> Outcome {
        let settings = self.settings.config();
        if !auto_download_on(&settings) {
            return Outcome::Done;
        }
        let Ok(Some(row)) = self.store.get(id) else {
            return Outcome::Done;
        };
        let origin = copypaste_core::origin_or(&row.origin_device_id, &self.device_id).to_owned();
        if row.deleted
            || origin == self.device_id
            || row.clipboard_privacy().secret
            || !files_allowed_from(&settings, &origin)
            || !matches!(self.store.downloaded_file(id), Ok(None))
        {
            return Outcome::Done;
        }
        let Ok(_slot) = self.downloads.reserve_queued().await else {
            return Outcome::Done;
        };
        let result = self
            .downloads
            .save_with_progress(
                &self.store,
                &self.keyring,
                &self.node,
                &self.device_id,
                &settings,
                FileDownloadRequest {
                    id,
                    destination: std::path::Path::new(&settings.file_save_folder),
                    token: None,
                },
                Arc::new(|_, _| {}),
            )
            .await;
        match result {
            Ok(_) => Outcome::Done,
            Err(
                FileTransferError::Remote(_)
                | FileTransferError::Unavailable
                | FileTransferError::SourceNotPaired
                | FileTransferError::Busy,
            ) => Outcome::Wait(origin),
            Err(error) => {
                tracing::warn!(%error, "an incoming file was not downloaded automatically");
                Outcome::Done
            }
        }
    }
}

/// Queues `id` once; false when it was already queued or the queue is full.
fn push(queue: &mut VecDeque<String>, id: &str) -> bool {
    if queue.iter().any(|queued| queued == id) {
        return false;
    }
    if queue.len() >= MAX_QUEUED {
        tracing::warn!("the file work queue is full; a file was skipped");
        return false;
    }
    queue.push_back(id.to_owned());
    true
}

#[cfg(test)]
mod tests {
    use crate::tests::fixture;
    use copypaste_ipc::{ConfigPatch, FileStorageMode, Method};
    use std::time::Duration;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn copy_mode_keeps_an_added_file_after_its_original_is_gone() {
        let (runtime, dir) = fixture();
        runtime
            .settings
            .apply(&ConfigPatch {
                file_storage: Some(FileStorageMode::Copy),
                ..Default::default()
            })
            .unwrap();
        let original = dir.path().join("notes.txt");
        std::fs::write(&original, b"kept bytes").unwrap();
        let added = runtime.request_blocking(
            1,
            Method::ImportFileReference {
                reference: original.to_string_lossy().into_owned(),
                filename: "notes.txt".into(),
                mime_type: "text/plain".into(),
                size_hint: None,
            },
        );
        assert!(added.ok, "{added:?}");
        let id = runtime.store.list(1, 0).unwrap().pop().unwrap().id;
        tokio::time::timeout(Duration::from_secs(5), async {
            while runtime.store.file_copies(&id).unwrap().is_empty() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("the copy is kept");
        std::fs::remove_file(&original).unwrap();
        let kept = copypaste_core::file_reference::kept_copy_chunk(
            &runtime.store,
            &runtime.keyring.item_key(),
            &id,
            0,
            0,
        )
        .unwrap();
        assert_eq!(&*kept, b"kept bytes");
    }
}
