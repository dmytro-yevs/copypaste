//! Relay receive-cursor persistence (CopyPaste-hf40 / CopyPaste-1jms.24).
//!
//! A `(wall_time, id)` keyset watermark so pagination is deterministic even
//! within one millisecond and a daemon restart resumes forward.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::token::write_token_0600;

/// `(wall_time, id)` keyset watermark so pagination is deterministic even within
/// one millisecond and a restart resumes forward.
///
/// CopyPaste-hf40 / CopyPaste-1jms.24: persisted to `relay_watermark.json` so
/// that on daemon restart the receive loop resumes from the last-seen cursor
/// instead of re-fetching everything from `(0, 0)`.
#[derive(Clone, Copy, Default, Serialize, Deserialize)]
pub(super) struct Watermark {
    pub(super) wall: u64,
    pub(super) id: i64,
}

pub(super) const RELAY_WATERMARK_FILE: &str = "relay_watermark.json";

/// Path to the persisted relay receive watermark file.
pub(super) fn watermark_path() -> Option<PathBuf> {
    crate::paths::try_app_support_dir()
        .ok()
        .map(|d| d.join(RELAY_WATERMARK_FILE))
}

/// Load the relay watermark from disk. Returns `Watermark::default()` when the
/// file is absent, unreadable, or malformed — all treated as "start from zero".
pub(super) fn load_watermark() -> Watermark {
    let Some(path) = watermark_path() else {
        return Watermark::default();
    };
    let raw = match std::fs::read_to_string(&path) {
        Ok(r) => r,
        // Missing file is the normal first-run case — not a warning.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Watermark::default(),
        Err(e) => {
            tracing::warn!("relay-sync: failed to read watermark file: {e}; starting from zero");
            return Watermark::default();
        }
    };
    match serde_json::from_str::<Watermark>(&raw) {
        Ok(wm) => {
            tracing::debug!(
                wall = wm.wall,
                id = wm.id,
                "relay-sync: loaded persisted watermark"
            );
            wm
        }
        Err(e) => {
            tracing::warn!("relay-sync: watermark file malformed ({e}); starting from zero");
            Watermark::default()
        }
    }
}

/// Persist the relay watermark to a `0600` file via atomic rename so a reader
/// never sees a partial write. Best-effort: failures are logged and the
/// in-memory watermark continues to be used for this run.
pub(super) fn save_watermark(wm: Watermark) {
    let Some(path) = watermark_path() else {
        tracing::warn!("relay-sync: cannot resolve data dir to save watermark");
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let json = match serde_json::to_string(&wm) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!("relay-sync: failed to serialise watermark: {e}");
            return;
        }
    };
    if let Err(e) = write_token_0600(&path, &json) {
        tracing::warn!("relay-sync: failed to persist watermark: {e}");
    }
}
