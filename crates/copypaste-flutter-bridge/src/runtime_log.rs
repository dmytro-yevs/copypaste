//! The application process's runtime log, kept beside the daemon's.
//!
//! Both processes write into `<data dir>/logs`, so the viewer pages one
//! directory with one cursor and keeps working while the daemon is down.

use std::{
    path::{Path, PathBuf},
    sync::OnceLock,
};

use copypaste_runtime_log::{Process, RuntimeLogGuard};

struct AppLog {
    dir: PathBuf,
    _guard: Option<RuntimeLogGuard>,
}

static APP_LOG: OnceLock<AppLog> = OnceLock::new();

/// Starts the process-wide app log once; later calls keep the first directory.
pub(crate) fn init(data_dir: &Path) {
    APP_LOG.get_or_init(|| {
        let dir = data_dir.join("logs");
        let guard = copypaste_runtime_log::init(&dir, Process::App).ok();
        if guard.is_some() {
            tracing::info!("app process started");
        }
        AppLog { dir, _guard: guard }
    });
}

/// The shared log directory, once the runtime has named its data directory.
pub(crate) fn dir() -> Option<&'static Path> {
    APP_LOG.get().map(|log| log.dir.as_path())
}
