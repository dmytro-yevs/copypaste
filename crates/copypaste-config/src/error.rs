use std::path::PathBuf;
use thiserror::Error;

/// Errors surfaced by [`crate::AppConfig`] load/save and path resolution.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// Underlying filesystem I/O failure (read, write, create_dir_all).
    #[error("config I/O error at {path:?}: {source}")]
    Io {
        /// Filesystem path that triggered the I/O error.
        path: PathBuf,
        /// Underlying `std::io::Error`.
        #[source]
        source: std::io::Error,
    },

    /// JSON (de)serialisation failure.
    #[error("config JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Could not resolve the per-platform project data directory.
    #[error("failed to resolve project data directory (HOME unset or non-standard platform)")]
    Path,
}

impl ConfigError {
    pub(crate) fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }
}
