//! Clipboard monitor: polls NSPasteboard (macOS) for text and image changes.
//!
//! Split (ADR-017) into cohesive submodules; this file is a thin re-export
//! facade so existing `crate::clipboard::*` call sites elsewhere in the
//! crate (sync_common, sync_orch, ipc, daemon/capture) keep resolving
//! unchanged.

mod content;
mod macos_util;
mod meta;
mod monitor;

pub use content::{ClipboardContent, ClipboardError, SKIPPED_BATCH_THRESHOLD};
pub use meta::{
    build_file_meta_json, build_image_meta_json, image_content_hash, image_thumb_file_id,
};
pub use monitor::ClipboardMonitor;
