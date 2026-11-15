//! Interrupted downloads continue once per start, after peers can reappear.
use crate::Runtime;
use std::{
    path::Path,
    sync::{atomic::Ordering, Arc},
    time::Duration,
};

/// Long enough for discovery and the first peer sync after a start.
const RESUME_AFTER_START: Duration = Duration::from_secs(20);

impl Runtime {
    /// Remove stale partial downloads, then resume the rest one at a time.
    /// A source that is still offline leaves its partial for a later Resume.
    pub fn start_download_resume(self: &Arc<Self>) -> Option<tokio::task::JoinHandle<()>> {
        if self.download_resume_started.swap(true, Ordering::AcqRel) {
            return None;
        }
        let runtime = Arc::clone(self);
        let mut shutdown = self.shutdown.subscribe();
        Some(tokio::spawn(async move {
            tokio::select! {
                _ = shutdown.changed() => return,
                _ = tokio::time::sleep(RESUME_AFTER_START) => {}
            }
            let pending = {
                let store = runtime.store.clone();
                let keyring = Arc::clone(&runtime.keyring);
                tokio::task::spawn_blocking(move || {
                    copypaste_core::file_partial::sweep(&store, &keyring);
                    copypaste_core::file_download::pending(&store, &keyring).unwrap_or_default()
                })
                .await
                .unwrap_or_default()
            };
            if !pending.is_empty() {
                tracing::info!("resuming {} interrupted downloads", pending.len());
            }
            for (item_id, destination) in pending {
                if *shutdown.borrow() {
                    return;
                }
                // A download the user started has priority; it resumes on its own.
                let Ok(_slot) = runtime.file_downloads.reserve_background() else {
                    return;
                };
                let settings = runtime.settings.config();
                let request = copypaste_core::file_download::FileDownloadRequest {
                    id: &item_id,
                    destination: Path::new(&destination),
                    token: None,
                };
                let _ = runtime
                    .file_downloads
                    .save_with_progress(
                        &runtime.store,
                        &runtime.keyring,
                        runtime.p2p.node(),
                        runtime.meta.device_id(),
                        &settings,
                        request,
                        Arc::new(|_, _| {}),
                    )
                    .await;
            }
        }))
    }
}
