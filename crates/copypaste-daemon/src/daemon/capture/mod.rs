//! Clipboard capture lifecycle: frontmost-app cache, per-tick dispatch, TTL
//! cleanup, and the Text/Image/File ingest handlers.
//!
//! Split (ADR-017) into cohesive submodules; this file is a thin re-export
//! shell preserving the exact `pub(crate)` surface `daemon/mod.rs` imports
//! (`capture::{handle_tick, run_ttl_cleanup}` and, on macOS,
//! `capture::FrontmostAppCache`).

mod cleanup;
mod file;
mod frontmost;
mod image;
mod text;
mod tick;

pub(crate) use cleanup::run_ttl_cleanup;
#[cfg(target_os = "macos")]
pub(crate) use frontmost::FrontmostAppCache;
pub(crate) use tick::handle_tick;

// Re-exported for in-crate call sites and tests that referenced the
// pre-split `daemon::capture::` path directly (e.g. `encrypt_text_for_storage`,
// `prune_history`, `handle_text`/`handle_image`/`handle_file`).
#[allow(unused_imports)]
pub(crate) use cleanup::prune_history;
#[allow(unused_imports)]
pub(crate) use file::handle_file;
#[allow(unused_imports)]
pub(crate) use image::handle_image;
#[allow(unused_imports)]
pub(crate) use text::{encrypt_text_for_storage, handle_text};
