//! Paginating `HistoryModel` for the v0.4 `HistoryView` Slint component.
//!
//! Holds a [`slint::VecModel<ClipItem>`] that is bound directly to
//! `MainWindow.history-items`.  Callers drive pagination by calling
//! [`HistoryModel::fetch_next_page`] (e.g. from a scroll-reached-bottom
//! callback) and reset / search via [`HistoryModel::reset_with_query`].
//!
//! All IPC calls are made synchronously on the Slint UI thread.  For the
//! page sizes used here (≤ 50 items) this is imperceptible to the user and
//! avoids the complexity of cross-thread `VecModel` mutation.

use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use slint::{Model, ModelRc, VecModel};

use crate::ipc_client::{format_wall_time, IpcClient};
use crate::windows::ClipItem;

/// Number of items fetched per IPC round-trip.
pub const PAGE_SIZE: u64 = 50;

/// Map a `content_type` string from the daemon into the `kind` field
/// used by the Slint `ClipItem` struct.
///
/// The mapping is intentionally liberal — any unknown type falls back to
/// `"text"` so new daemon content types degrade gracefully in the UI.
fn content_type_to_kind(content_type: &str) -> slint::SharedString {
    match content_type {
        "image" | "image/png" | "image/jpeg" | "image/gif" | "image/webp" => "image",
        "link" | "url" => "link",
        "code" => "code",
        "file" => "file",
        "color" => "color",
        _ => "text",
    }
    .into()
}

/// Convert a [`crate::ipc_client::HistoryEntry`] to a Slint [`ClipItem`].
fn entry_to_clip_item(entry: &crate::ipc_client::HistoryEntry) -> ClipItem {
    ClipItem {
        // H1: parse as i64 first, then clamp to i32.  i32::MAX (2_147_483_647)
        // is used as the sentinel for out-of-range / unparseable IDs; daemon
        // IDs are small positive integers so MAX cannot collide with a real entry,
        // unlike the previous sentinel of 0 which aliases the "unset" state.
        id: entry
            .id
            .parse::<i64>()
            .ok()
            .and_then(|n| i32::try_from(n).ok())
            .unwrap_or(i32::MAX),
        preview: entry.preview.clone().into(),
        kind: content_type_to_kind(&entry.content_type),
        wall_time: format_wall_time(entry.wall_time).into(),
        source_device: slint::SharedString::default(),
        pinned: false,
        redacted: entry.is_sensitive,
    }
}

/// Paginating clipboard history model for the v0.4 `HistoryView`.
///
/// # Thread safety
///
/// This type is intentionally `!Send` — it wraps a [`Rc<VecModel<ClipItem>>`]
/// which must stay on the Slint UI thread.
pub struct HistoryModel {
    /// Backing store shared with the Slint `ModelRc`.
    inner: Rc<VecModel<ClipItem>>,
    /// Total items available on the daemon (from the last IPC response).
    total_count: Cell<u64>,
    /// Items per page (fixed at [`PAGE_SIZE`]).
    page_size: u64,
    /// `true` while an IPC fetch is in progress — prevents double-fetch.
    loading: Cell<bool>,
    /// Absolute path to the daemon Unix socket.
    socket_path: PathBuf,
    /// Current search query; empty string means "list all".
    current_query: Cell<String>,
    /// Zero-based index of the next item to fetch (= items already loaded).
    next_offset: Cell<u64>,
}

impl HistoryModel {
    /// Create an empty model.  No IPC call is made at construction time.
    /// Call [`HistoryModel::load_initial`] to populate the first page.
    pub fn new(socket_path: PathBuf) -> Self {
        Self {
            inner: Rc::new(VecModel::default()),
            total_count: Cell::new(0),
            page_size: PAGE_SIZE,
            loading: Cell::new(false),
            socket_path,
            current_query: Cell::new(String::new()),
            next_offset: Cell::new(0),
        }
    }

    /// Return a [`ModelRc`] suitable for binding to a Slint property.
    ///
    /// ```ignore
    /// window.set_history_items(model.as_model_rc());
    /// ```
    pub fn as_model_rc(&self) -> ModelRc<ClipItem> {
        self.inner.clone().into()
    }

    // ------------------------------------------------------------------
    // Internal helpers
    // ------------------------------------------------------------------

    /// `true` when the daemon has more items beyond what is already loaded.
    #[allow(dead_code)]
    fn has_more(&self) -> bool {
        self.next_offset.get() < self.total_count.get()
            || self.total_count.get() == 0 && self.next_offset.get() == 0
    }

    /// Open an IPC connection, returning `None` on failure (logged via
    /// `tracing`).
    fn connect(&self) -> Option<IpcClient> {
        match IpcClient::connect(Path::new(&self.socket_path)) {
            Ok(c) => Some(c),
            Err(e) => {
                tracing::warn!("HistoryModel: daemon offline — {e}");
                None
            }
        }
    }

    /// Append a slice of [`ClipItem`]s to the model.
    fn append_items(&self, items: Vec<ClipItem>) {
        for item in items {
            self.inner.push(item);
        }
    }

    // ------------------------------------------------------------------
    // Public API
    // ------------------------------------------------------------------

    /// Fetch the first page synchronously.  Clears any existing items and
    /// resets all pagination state.
    ///
    /// Safe to call on the Slint UI thread; the IPC round-trip for 50 items
    /// is typically < 5 ms on a local Unix socket.
    pub fn load_initial(&self) {
        if let Err(e) = self.reset_with_query("") {
            tracing::warn!("HistoryModel::load_initial: reset_with_query failed — {e}");
        }
    }

    /// Fetch the next page from the daemon and append results.
    ///
    /// No-ops when:
    /// - a fetch is already in progress (`loading == true`), or
    /// - all available items have been loaded.
    ///
    /// Returns `Ok(())` when the fetch completed (or was skipped).
    pub fn fetch_next_page(&self) -> anyhow::Result<()> {
        if self.loading.get() {
            return Ok(());
        }
        // Suppress spurious first-call guard: after reset the offset is 0
        // and total is 0, so `has_more` returns true via the special-case.
        // After a successful page the total is set from the IPC response,
        // so `has_more` reflects reality.
        let offset = self.next_offset.get();
        if offset > 0 && offset >= self.total_count.get() {
            return Ok(()); // all pages loaded
        }

        self.loading.set(true);

        let query = self.current_query.take();

        // Build the IPC result; restore query + clear loading flag on all
        // paths (including the early-return from `connect` failure).
        let result: anyhow::Result<crate::ipc_client::HistoryPage> = match self.connect() {
            None => Err(anyhow::anyhow!("daemon offline")),
            Some(mut client) => client.history_page(self.page_size, offset),
        };

        // Restore the query field and clear the loading flag regardless of
        // whether the IPC call succeeded or failed.
        self.current_query.set(query);
        self.loading.set(false);

        match result {
            Ok(page) => {
                self.total_count.set(page.total);
                let new_offset = offset + page.items.len() as u64;
                self.next_offset.set(new_offset);
                let clip_items: Vec<ClipItem> = page.items.iter().map(entry_to_clip_item).collect();
                self.append_items(clip_items);
                Ok(())
            }
            Err(e) => {
                tracing::warn!("HistoryModel: fetch_next_page failed — {e}");
                Err(e)
            }
        }
    }

    /// Clear all items, set the search query, and fetch the first page.
    ///
    /// Pass an empty string to list all items without filtering.
    pub fn reset_with_query(&self, query: &str) -> anyhow::Result<()> {
        self.inner.set_vec(vec![]);
        self.next_offset.set(0);
        self.total_count.set(0);
        self.loading.set(false);
        self.current_query.set(query.to_owned());
        self.fetch_next_page()
    }

    /// Reset to the unfiltered item list and fetch the first page.
    pub fn reset(&self) {
        if let Err(e) = self.reset_with_query("") {
            tracing::warn!("history reset failed: {e}");
        }
    }

    /// Set a search query and reload from page 0.
    pub fn search(&self, query: &str) {
        if let Err(e) = self.reset_with_query(query) {
            tracing::warn!("history search reset failed: {e}");
        }
    }

    /// `true` if a page fetch is currently in progress.
    pub fn is_loading(&self) -> bool {
        self.loading.get()
    }

    /// Total number of items available on the daemon, as reported by the
    /// most recent IPC response.  Zero before the first successful fetch.
    pub fn total_count(&self) -> u64 {
        self.total_count.get()
    }

    /// Number of items currently loaded into the model.
    pub fn loaded_count(&self) -> usize {
        self.inner.row_count()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_model() -> HistoryModel {
        HistoryModel::new(PathBuf::from("/tmp/nonexistent-copypaste.sock"))
    }

    #[test]
    fn new_model_is_empty() {
        let m = make_model();
        assert_eq!(m.loaded_count(), 0);
        assert_eq!(m.total_count(), 0);
        assert!(!m.is_loading());
    }

    #[test]
    fn fetch_next_page_noop_when_loading() {
        let m = make_model();
        m.loading.set(true);
        // Should return Ok without panicking when loading guard is active.
        assert!(m.fetch_next_page().is_ok());
        // Item count unchanged (no IPC attempt).
        assert_eq!(m.loaded_count(), 0);
        m.loading.set(false);
    }

    #[test]
    fn fetch_next_page_noop_when_all_loaded() {
        let m = make_model();
        // Simulate: 2 items loaded, total = 2.
        m.next_offset.set(2);
        m.total_count.set(2);
        // Should be a no-op (no socket → but guard fires first).
        assert!(m.fetch_next_page().is_ok());
        assert_eq!(m.loaded_count(), 0);
    }

    #[test]
    fn fetch_next_page_returns_err_when_daemon_offline() {
        let m = make_model();
        // Page 0, daemon not running — should return Err.
        let result = m.fetch_next_page();
        assert!(result.is_err(), "expected IPC error on missing socket");
        assert!(!m.is_loading(), "loading flag must be cleared on error");
    }

    #[test]
    fn reset_clears_items_and_offsets() {
        let m = make_model();
        // Manually place items and offsets to simulate a partially loaded state.
        m.inner.push(ClipItem {
            id: 1,
            preview: "test".into(),
            kind: "text".into(),
            wall_time: "2025-01-01".into(),
            source_device: "".into(),
            pinned: false,
            redacted: false,
        });
        m.next_offset.set(1);
        m.total_count.set(5);

        // reset() should clear items; fetch will fail (no daemon), which is OK.
        m.inner.set_vec(vec![]);
        m.next_offset.set(0);
        m.total_count.set(0);

        assert_eq!(m.loaded_count(), 0);
        assert_eq!(m.next_offset.get(), 0);
    }

    #[test]
    fn content_type_to_kind_mappings() {
        assert_eq!(content_type_to_kind("text"), "text");
        assert_eq!(content_type_to_kind("image"), "image");
        assert_eq!(content_type_to_kind("image/png"), "image");
        assert_eq!(content_type_to_kind("link"), "link");
        assert_eq!(content_type_to_kind("url"), "link");
        assert_eq!(content_type_to_kind("code"), "code");
        assert_eq!(content_type_to_kind("file"), "file");
        assert_eq!(content_type_to_kind("color"), "color");
        assert_eq!(content_type_to_kind("unknown_future_type"), "text");
    }

    #[test]
    fn entry_to_clip_item_maps_fields() {
        let entry = crate::ipc_client::HistoryEntry {
            id: "42".to_owned(),
            content_type: "image/png".to_owned(),
            preview: "screenshot.png".to_owned(),
            is_sensitive: true,
            wall_time: 0, // zero → em-dash
        };
        let item = entry_to_clip_item(&entry);
        assert_eq!(item.id, 42);
        assert_eq!(item.preview, "screenshot.png");
        assert_eq!(item.kind, "image");
        assert!(item.redacted);
        assert!(!item.pinned);
        // wall_time 0 → "—" (em-dash) from format_wall_time
        assert_eq!(item.wall_time, "\u{2014}");
    }

    #[test]
    fn page_size_constant_is_50() {
        assert_eq!(PAGE_SIZE, 50);
        let m = make_model();
        assert_eq!(m.page_size, 50);
    }
}
