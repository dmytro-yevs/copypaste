// windows.rs — Rust handles for SettingsWindow and PairWindow
// Wraps the Slint-generated component types with ergonomic constructors and callback wiring.

use slint::{ComponentHandle, Model, VecModel};
use std::cell::RefCell;
use std::rc::Rc;

use crate::fingerprint::{format_fingerprint_long, format_fingerprint_truncated};
use crate::settings::{AppSettings, HistoryLimit, PairedDevice};

// Pull in the generated Slint components (SettingsWindow, PairWindow, etc.)
slint::include_modules!();

// ── SettingsWindow ─────────────────────────────────────────────────────────────

/// A live handle to the SettingsWindow.
pub struct SettingsWindowHandle {
    window: SettingsWindow,
}

impl SettingsWindowHandle {
    /// Create and populate the window from the given settings.
    pub fn new(
        settings: &AppSettings,
        version: &str,
        device_fingerprint: &str,
    ) -> Result<Self, slint::PlatformError> {
        let window = SettingsWindow::new()?;

        window.set_launch_at_login(settings.launch_at_login);
        window.set_private_mode(settings.private_mode);
        window.set_history_size(settings.history_limit.as_count() as i32);
        window.set_supabase_url(settings.supabase_url.clone().into());
        window.set_supabase_key(settings.supabase_key.clone().into());
        window.set_device_name(settings.device_name.clone().into());
        window.set_app_version(version.into());
        window.set_device_fingerprint(format_fingerprint_long(device_fingerprint).into());

        Ok(Self { window })
    }

    /// Register a callback invoked when the user clicks "Save".
    /// The callback receives a snapshot of the current UI state as `AppSettings`.
    pub fn on_save<F: Fn(AppSettings) + 'static>(&self, cb: F) {
        let w = self.window.as_weak();
        self.window.on_save(move || {
            if let Some(win) = w.upgrade() {
                let settings = AppSettings {
                    launch_at_login: win.get_launch_at_login(),
                    private_mode: win.get_private_mode(),
                    history_limit: HistoryLimit::from_count(win.get_history_size() as usize),
                    supabase_url: win.get_supabase_url().to_string(),
                    supabase_key: win.get_supabase_key().to_string(),
                    device_name: win.get_device_name().to_string(),
                };
                cb(settings);
            }
        });
    }

    /// Register a callback invoked when "Clear History" is clicked.
    pub fn on_clear_history<F: Fn() + 'static>(&self, cb: F) {
        self.window.on_clear_history(cb);
    }

    /// Register a callback invoked when "Connect" is clicked.
    /// Receives `(url, anon_key)`.
    pub fn on_connect_supabase<F: Fn(String, String) + 'static>(&self, cb: F) {
        self.window.on_connect_supabase(move |url, key| {
            cb(url.to_string(), key.to_string());
        });
    }

    /// Register a callback invoked when "Disconnect" is clicked.
    pub fn on_disconnect_supabase<F: Fn() + 'static>(&self, cb: F) {
        self.window.on_disconnect_supabase(cb);
    }

    /// Register a callback invoked when ESC or "Cancel" is clicked.
    pub fn on_close<F: Fn() + 'static>(&self, cb: F) {
        self.window.on_close(cb);
    }

    /// Update the sync connection status shown in the window.
    pub fn set_sync_status(&self, connected: bool, message: &str) {
        self.window.set_sync_connected(connected);
        self.window.set_sync_status_msg(message.into());
    }

    /// Push a transient status / error string into the SettingsWindow's
    /// `sync-status-msg` slot. The Slint window has no separate toast surface
    /// today, so we re-purpose the sync status line for save/clear feedback —
    /// it's the only string property already wired to a visible label.
    ///
    /// `is_error == true` flips `sync-connected` off so the status line
    /// renders in the "disconnected" style, giving the user a visual cue
    /// without adding a new Slint property (forbidden by W3 scope here).
    pub fn set_status_message(&self, message: &str, is_error: bool) {
        self.window.set_sync_status_msg(message.into());
        if is_error {
            self.window.set_sync_connected(false);
        }
    }

    /// Replace every editable field in the window with the supplied settings.
    /// Used by [`Self::wire_to_ipc`] for the initial load and by host code
    /// that wants to re-populate the form after an external mutation
    /// (e.g., the user reverted via the daemon CLI). Leaves `app_version`,
    /// `device_fingerprint`, and sync status untouched so they're not
    /// stomped when only the editable subset has changed.
    pub fn apply_settings(&self, settings: &AppSettings) {
        self.window.set_launch_at_login(settings.launch_at_login);
        self.window.set_private_mode(settings.private_mode);
        self.window
            .set_history_size(settings.history_limit.as_count() as i32);
        self.window
            .set_supabase_url(settings.supabase_url.clone().into());
        self.window
            .set_supabase_key(settings.supabase_key.clone().into());
        self.window
            .set_device_name(settings.device_name.clone().into());
    }

    /// Read the current UI state back out as an [`AppSettings`]. Mirrors the
    /// snapshot built inside [`Self::on_save`] so tests and host code can
    /// inspect "what the user would save" without dispatching the callback.
    pub fn current_settings(&self) -> AppSettings {
        AppSettings {
            launch_at_login: self.window.get_launch_at_login(),
            private_mode: self.window.get_private_mode(),
            history_limit: HistoryLimit::from_count(self.window.get_history_size() as usize),
            supabase_url: self.window.get_supabase_url().to_string(),
            supabase_key: self.window.get_supabase_key().to_string(),
            device_name: self.window.get_device_name().to_string(),
        }
    }

    /// Show the window.
    pub fn show(&self) -> Result<(), slint::PlatformError> {
        self.window.show()
    }

    /// Hide the window.
    pub fn hide(&self) -> Result<(), slint::PlatformError> {
        self.window.hide()
    }

    /// Run the event loop (blocks until window closes).
    pub fn run(&self) -> Result<(), slint::PlatformError> {
        self.window.run()
    }
}

// ── SettingsWindow ↔ IPC wiring (beta-bonus: settings-wire) ────────────────────

/// Narrow IPC surface required by [`SettingsWindowHandle::wire_to_ipc`].
///
/// Defined as a trait — not a concrete type — for three reasons:
///
/// 1. **Isolation from `ipc_client.rs`** — that module is W3-stable for this
///    wave and the lib crate intentionally does not depend on the binary's
///    private IPC client. Hosts in `main.rs` adapt their real client into
///    this trait.
/// 2. **Type adaptation** — the wire-level `ipc_client::AppSettings` has a
///    different shape (`p2p_enabled`, `supabase_url: Option<String>`, …)
///    from the UI-level [`AppSettings`] this handle owns. The host is the
///    natural place to bridge the two; this trait keeps the bridge code out
///    of the window handle.
/// 3. **Testability** — the unit tests below register a mock implementation
///    that records every call without needing a Unix-socket round-trip.
///
/// Every method returns a stringly-typed `Result` so the handle can paint
/// the message straight into `sync-status-msg` regardless of which concrete
/// error type the host's IPC client uses.
pub trait SettingsIpc {
    /// Fetch the persisted settings from the daemon. Called once when the
    /// window opens so the form reflects what's on disk.
    fn get_settings(&mut self) -> Result<AppSettings, String>;
    /// Persist the user-edited settings via the daemon. Invoked from the
    /// `Save` callback. The daemon must accept the new state atomically.
    fn save_settings(&mut self, settings: &AppSettings) -> Result<(), String>;
    /// Drop every clipboard-history row stored by the daemon. Invoked from
    /// the `Clear History` callback. Confirmation dialog is the host's
    /// responsibility (Slint has no native dialog API today, so we trust
    /// the click for now per the W3 brief).
    fn delete_all_history(&mut self) -> Result<(), String>;
}

/// Status text + error flag produced by an IPC interaction. Kept as a plain
/// value so unit tests can assert exact strings without driving a real
/// Slint window. The host (or [`SettingsWindowHandle::wire_to_ipc`]) feeds
/// the result into [`SettingsWindowHandle::set_status_message`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusUpdate {
    pub message: String,
    pub is_error: bool,
}

/// Pure-function side of the `Save` callback: dispatch to the IPC, format
/// the resulting status string. Extracted from
/// [`SettingsWindowHandle::wire_to_ipc`] so the IPC contract is exercisable
/// in headless unit tests — no Slint backend needed.
pub fn perform_save<I: SettingsIpc>(ipc: &mut I, settings: &AppSettings) -> StatusUpdate {
    match ipc.save_settings(settings) {
        Ok(()) => StatusUpdate {
            message: "Settings saved.".into(),
            is_error: false,
        },
        Err(e) => StatusUpdate {
            message: format!("Save failed: {e}"),
            is_error: true,
        },
    }
}

/// Pure-function side of the `Clear History` callback. See [`perform_save`]
/// for the rationale on the split.
pub fn perform_clear_history<I: SettingsIpc>(ipc: &mut I) -> StatusUpdate {
    match ipc.delete_all_history() {
        Ok(()) => StatusUpdate {
            message: "History cleared.".into(),
            is_error: false,
        },
        Err(e) => StatusUpdate {
            message: format!("Clear failed: {e}"),
            is_error: true,
        },
    }
}

/// Pure-function side of the initial-load path. Returns the loaded settings
/// (for the window to apply) and an optional error-status update when the
/// load fails — the window still opens with its defaults in that case.
pub fn perform_initial_load<I: SettingsIpc>(
    ipc: &mut I,
) -> (Option<AppSettings>, Option<StatusUpdate>) {
    match ipc.get_settings() {
        Ok(s) => (Some(s), None),
        Err(e) => (
            None,
            Some(StatusUpdate {
                message: format!("Load failed: {e}"),
                is_error: true,
            }),
        ),
    }
}

impl SettingsWindowHandle {
    /// Wire the `save` and `clear-history` callbacks to a [`SettingsIpc`]
    /// implementation and seed the window with the daemon's current
    /// settings.
    ///
    /// The IPC handle is shared via `Rc<RefCell<_>>` because Slint callbacks
    /// are `Fn` (not `FnMut`) and outlive any single invocation — each
    /// click needs mutable access for the next request. The handle stays on
    /// the Slint event-loop thread (single-threaded by design here), so
    /// `Rc<RefCell<_>>` is sufficient — no `Send`/`Sync` required.
    ///
    /// Returns:
    /// * `Ok(())` — the initial `get_settings()` round-trip succeeded and
    ///   the form is populated; later save/clear failures surface inline
    ///   via [`Self::set_status_message`].
    /// * `Err(msg)` — the initial load failed. The window is still shown
    ///   with its constructor defaults; the message is painted into
    ///   `sync-status-msg` so the user sees why the form is empty.
    pub fn wire_to_ipc<I: SettingsIpc + 'static>(&self, ipc: Rc<RefCell<I>>) -> Result<(), String> {
        // --- Initial load -----------------------------------------------------
        // Failing the initial load must not prevent the window from opening:
        // we surface the error inline so the user can still hit "Save" with
        // their constructor defaults and recover.
        let (loaded, load_status) = perform_initial_load(&mut *ipc.borrow_mut());
        if let Some(settings) = &loaded {
            self.apply_settings(settings);
        }
        let load_outcome = if let Some(status) = load_status {
            let msg = status.message.clone();
            self.set_status_message(&status.message, status.is_error);
            Err(msg)
        } else {
            Ok(())
        };

        // --- Save -------------------------------------------------------------
        let w = self.window.as_weak();
        let ipc_save = Rc::clone(&ipc);
        self.window.on_save(move || {
            let Some(win) = w.upgrade() else { return };
            let settings = AppSettings {
                launch_at_login: win.get_launch_at_login(),
                private_mode: win.get_private_mode(),
                history_limit: HistoryLimit::from_count(win.get_history_size() as usize),
                supabase_url: win.get_supabase_url().to_string(),
                supabase_key: win.get_supabase_key().to_string(),
                device_name: win.get_device_name().to_string(),
            };
            let status = perform_save(&mut *ipc_save.borrow_mut(), &settings);
            win.set_sync_status_msg(status.message.into());
            if status.is_error {
                win.set_sync_connected(false);
            }
        });

        // --- Clear history ----------------------------------------------------
        // Slint exposes no native confirmation dialog yet; per the W3 brief
        // we trust the click. Once a dialog primitive lands, gate the IPC
        // call behind it here without changing the trait.
        let w = self.window.as_weak();
        let ipc_clear = Rc::clone(&ipc);
        self.window.on_clear_history(move || {
            let Some(win) = w.upgrade() else { return };
            let status = perform_clear_history(&mut *ipc_clear.borrow_mut());
            win.set_sync_status_msg(status.message.into());
            if status.is_error {
                win.set_sync_connected(false);
            }
        });

        load_outcome
    }
}

// ── Empty-state hint helpers (Wave 3.1) ────────────────────────────────────────

/// The empty-state hint shown in the PairWindow when no peers are paired.
/// Returned as a `(title, hint)` pair so tests can verify both lines without
/// driving the Slint component.
///
/// `peer_count == 0` → user-facing troubleshooting hint.
/// `peer_count >  0` → `(None, None)` indicating the list itself should render.
pub fn pair_window_empty_hint(peer_count: usize) -> Option<(&'static str, &'static str)> {
    if peer_count == 0 {
        Some((
            "No peers discovered.",
            "Ensure other device is on same Wi-Fi and running CopyPaste.",
        ))
    } else {
        None
    }
}

// ── HistoryWindow image previews (Wave 3.4) ────────────────────────────────────

/// Format a one-line label for an image clipboard item shown in the
/// HistoryWindow. Falls back gracefully when dimensions / size are unknown
/// (the daemon currently only ships metadata — the raw bytes are fetched
/// lazily via a future `get_image_thumbnail(id)` IPC method, see TODO in
/// `main.rs`).
///
/// Examples:
///   `image_preview_label(Some(1920), Some(1080), Some(452_000))`
///     → `"Image  1920×1080 · 441 KB"`
///   `image_preview_label(None, None, None)` → `"Image"`
pub fn image_preview_label(width: Option<u32>, height: Option<u32>, bytes: Option<u64>) -> String {
    let mut out = String::from("Image");
    if let (Some(w), Some(h)) = (width, height) {
        out.push_str(&format!("  {w}×{h}"));
    }
    if let Some(b) = bytes {
        out.push_str(" · ");
        out.push_str(&format_byte_size(b));
    }
    out
}

/// Human-readable byte size — KB / MB with one decimal once we cross 1 MB.
fn format_byte_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * 1024;
    if bytes < KB {
        format!("{bytes} B")
    } else if bytes < MB {
        format!("{} KB", bytes / KB)
    } else {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    }
}

// ── HistoryWindow search filtering (beta-bonus: slint-search) ──────────────────

/// Minimal item shape used by [`filter_history_items`].
///
/// Defined as a trait rather than coupling to the generated Slint
/// `HistoryItem` so unit tests can exercise the filter without spinning up
/// a Slint runtime. The UI layer in `main.rs` adapts the generated rows
/// into this shape before calling the filter and swaps the resulting model
/// into the window.
pub trait SearchableHistoryItem {
    fn preview(&self) -> &str;
}

/// Filter clipboard-history items by a case-insensitive substring match on
/// the user-visible `preview` text.
///
/// Empty / whitespace-only queries return every item unchanged so the
/// debounced `search-changed("")` we fire when the user clears the field
/// restores the original list without a daemon round-trip.
///
/// Lowercasing is done once per call on the query (not per item) to keep
/// the filter cheap even when the history holds a full `PAGE_SIZE = 50`
/// page.
pub fn filter_history_items<'a, T: SearchableHistoryItem>(
    items: &'a [T],
    query: &str,
) -> Vec<&'a T> {
    let needle = query.trim().to_lowercase();
    if needle.is_empty() {
        return items.iter().collect();
    }
    items
        .iter()
        .filter(|item| item.preview().to_lowercase().contains(&needle))
        .collect()
}

// ── PairWindow ─────────────────────────────────────────────────────────────────

/// A live handle to the PairWindow.
pub struct PairWindowHandle {
    window: PairWindow,
}

impl PairWindowHandle {
    /// Create the window with the device's own fingerprint and the current paired device list.
    pub fn new(
        own_fingerprint: &str,
        paired_devices: &[PairedDevice],
    ) -> Result<Self, slint::PlatformError> {
        let window = PairWindow::new()?;

        window.set_own_fingerprint(format_fingerprint_long(own_fingerprint).into());
        Self::apply_device_model(&window, paired_devices);

        Ok(Self { window })
    }

    fn apply_device_model(window: &PairWindow, devices: &[PairedDevice]) {
        let entries: Vec<PairedDeviceEntry> = devices
            .iter()
            .map(|d| PairedDeviceEntry {
                name: d.name.clone().into(),
                fingerprint: d.fingerprint.clone().into(),
                fingerprint_short: format_fingerprint_truncated(&d.fingerprint).into(),
            })
            .collect();
        let model = Rc::new(VecModel::from(entries));
        window.set_paired_devices(model.into());
    }

    /// Update the own fingerprint displayed (call after key generation completes).
    pub fn set_own_fingerprint(&self, raw_hex: &str) {
        self.window
            .set_own_fingerprint(format_fingerprint_long(raw_hex).into());
    }

    /// Replace the paired device list entirely.
    pub fn set_paired_devices(&self, devices: &[PairedDevice]) {
        Self::apply_device_model(&self.window, devices);
    }

    /// Show a status message (success or error).
    pub fn set_status(&self, message: &str, is_error: bool) {
        self.window.set_status_message(message.into());
        self.window.set_status_is_error(is_error);
    }

    /// Clear the status message.
    pub fn clear_status(&self) {
        self.window
            .set_status_message(slint::SharedString::default());
    }

    /// Register a callback invoked when "Pair" is clicked.
    /// Receives the fingerprint string entered by the user.
    pub fn on_pair<F: Fn(String) + 'static>(&self, cb: F) {
        self.window.on_pair(move |fp| cb(fp.to_string()));
    }
    /// Register a callback invoked when "Pair with Password" is clicked.
    /// Receives `(peer_fingerprint, password)`. The closure is responsible
    /// for calling `IpcClient::pair_with_password` and surfacing the
    /// daemon's success/error status back into the UI via [`Self::set_status`].
    ///
    /// Beta W3.2 — wires the new Slint `pair-with-password(string, string)`
    /// callback added to `PairWindow.slint`.
    pub fn on_pair_with_password<F: Fn(String, String) + 'static>(&self, cb: F) {
        self.window
            .on_pair_with_password(move |fp, pw| cb(fp.to_string(), pw.to_string()));
    }

    /// Return the user-facing empty-state hint for the current peer list, if any.
    /// Convenience wrapper over [`pair_window_empty_hint`] for callers that
    /// already hold a window handle.
    pub fn empty_hint(&self) -> Option<(&'static str, &'static str)> {
        pair_window_empty_hint(self.window.get_paired_devices().row_count())
    }

    /// Register a callback invoked when "Remove" is clicked on a paired device.
    /// Receives the full fingerprint of the device to remove.
    pub fn on_remove_peer<F: Fn(String) + 'static>(&self, cb: F) {
        self.window.on_remove_peer(move |fp| cb(fp.to_string()));
    }

    /// T4 (v0.3) — register a callback invoked when the user confirms the
    /// revoke dialog. Receives the full fingerprint of the peer to revoke.
    ///
    /// Callers should route this to `crate::ipc_client::IpcClient::revoke_peer`
    /// (defined in the `copypaste-ui` binary, not exported by the library)
    /// rather than `unpair_peer`: the former additionally writes a row to
    /// the SQLite `revoked_devices` audit table consumed by the v1.0
    /// cryptographic revocation protocol.
    pub fn on_revoke_peer<F: Fn(String) + 'static>(&self, cb: F) {
        self.window.on_revoke_peer(move |fp| cb(fp.to_string()));
    }

    /// Register a callback invoked when ESC or "Close" is clicked.
    pub fn on_close<F: Fn() + 'static>(&self, cb: F) {
        self.window.on_close(cb);
    }

    /// Show the window.
    pub fn show(&self) -> Result<(), slint::PlatformError> {
        self.window.show()
    }

    /// Hide the window.
    pub fn hide(&self) -> Result<(), slint::PlatformError> {
        self.window.hide()
    }

    /// Run the event loop (blocks until window closes).
    pub fn run(&self) -> Result<(), slint::PlatformError> {
        self.window.run()
    }
}

/// Beta W3.2 — minimum number of Unicode scalars required for a PAKE
/// pairing password. Mirrors `IpcClient::MIN_PAIR_PASSWORD_LEN` and the
/// daemon-side check; kept in three places intentionally so each layer
/// fails fast on its own.
pub const MIN_PAIR_PASSWORD_LEN: usize = 6;

/// Validate a PAKE pairing password using Unicode scalar counts. The Slint
/// UI uses `character-count >= 6` inline; this helper is the equivalent for
/// Rust callers (e.g., when the UI delegates validation back to a Rust
/// callback before invoking IPC).
pub fn is_valid_pair_password(password: &str) -> bool {
    password.chars().count() >= MIN_PAIR_PASSWORD_LEN
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pair_window_empty_peer_list_renders_hint_text() {
        // Wave 3.1 fix #27: an empty peer list must yield a troubleshooting
        // hint with both a title and a guidance line — not just silence.
        let hint = pair_window_empty_hint(0).expect("empty list must produce a hint");
        let (title, body) = hint;

        assert!(
            title.contains("No peers"),
            "title should mention no peers, got: {title}"
        );
        assert!(
            body.contains("Wi-Fi") && body.contains("CopyPaste"),
            "body must reference Wi-Fi + CopyPaste so the user knows what to check, got: {body}"
        );
        // The Slint footprint stays in sync with the helper, so the rendered
        // hint matches exactly what we assert here.
    }

    // --- Wave 3.4: image preview label ---

    #[test]
    fn image_preview_label_with_full_metadata() {
        let s = image_preview_label(Some(1920), Some(1080), Some(452_000));
        assert!(s.starts_with("Image"), "must start with Image: {s}");
        assert!(s.contains("1920×1080"), "must show dimensions: {s}");
        assert!(s.contains("441 KB"), "must show size in KB: {s}");
    }

    #[test]
    fn image_preview_label_without_metadata_is_safe() {
        assert_eq!(image_preview_label(None, None, None), "Image");
    }

    #[test]
    fn image_preview_label_dimensions_only() {
        let s = image_preview_label(Some(64), Some(32), None);
        assert!(s.contains("64×32"), "dimensions only: {s}");
        assert!(
            !s.contains('·'),
            "no size separator when bytes missing: {s}"
        );
    }

    #[test]
    fn format_byte_size_thresholds() {
        assert_eq!(format_byte_size(512), "512 B");
        assert_eq!(format_byte_size(2048), "2 KB");
        assert_eq!(format_byte_size(1_500_000), "1.4 MB");
    }

    #[test]
    fn pair_window_password_validation_matches_min_length() {
        // beta-W3.2: the Rust-side helper and the daemon-side check must
        // agree on the 6-char Unicode-scalar minimum.
        assert_eq!(MIN_PAIR_PASSWORD_LEN, 6);
        assert!(!is_valid_pair_password(""));
        assert!(!is_valid_pair_password("12345"));
        assert!(is_valid_pair_password("123456"));
        assert!(
            is_valid_pair_password("парол1"),
            "6-scalar Cyrillic password must pass — counts characters, not bytes"
        );
        assert!(
            !is_valid_pair_password("ab漢"),
            "3-scalar multibyte password must fail"
        );
    }

    #[test]
    fn pair_window_with_peers_skips_hint() {
        assert!(
            pair_window_empty_hint(1).is_none(),
            "non-empty peer list must not render the empty-state hint"
        );
        assert!(
            pair_window_empty_hint(42).is_none(),
            "large peer list must not render the empty-state hint"
        );
    }

    // --- beta-bonus (slint-search): history search filter ---

    struct StubItem(&'static str);
    impl SearchableHistoryItem for StubItem {
        fn preview(&self) -> &str {
            self.0
        }
    }

    #[test]
    fn search_query_filters_history_items_substring_match() {
        let items = vec![
            StubItem("hello world"),
            StubItem("goodbye moon"),
            StubItem("rust programming"),
        ];
        let filtered = filter_history_items(&items, "world");
        assert_eq!(
            filtered.len(),
            1,
            "expected one substring match, got {}",
            filtered.len()
        );
        assert_eq!(filtered[0].preview(), "hello world");
    }

    #[test]
    fn search_query_empty_returns_all_items() {
        let items = vec![StubItem("a"), StubItem("b"), StubItem("c")];
        assert_eq!(
            filter_history_items(&items, "").len(),
            3,
            "empty query keeps every row"
        );
        assert_eq!(
            filter_history_items(&items, "   ").len(),
            3,
            "whitespace-only query is treated as empty so clearing the field restores the list"
        );
    }

    #[test]
    fn search_query_case_insensitive() {
        let items = vec![
            StubItem("Hello World"),
            StubItem("RUST is FUN"),
            StubItem("nothing"),
        ];
        let upper = filter_history_items(&items, "WORLD");
        assert_eq!(
            upper.len(),
            1,
            "uppercase query must match lowercase preview"
        );
        assert_eq!(upper[0].preview(), "Hello World");

        let lower = filter_history_items(&items, "rust");
        assert_eq!(
            lower.len(),
            1,
            "lowercase query must match uppercase preview"
        );
        assert_eq!(lower[0].preview(), "RUST is FUN");
    }

    // --- beta-bonus (settings-wire): SettingsIpc wiring contract ---
    //
    // These tests exercise the IPC wiring without instantiating a real
    // SettingsWindow — Slint windows need a backend that isn't available in
    // a headless `cargo test` run. The split between `wire_to_ipc` and the
    // `perform_*` helpers exists precisely to make this testable.

    /// Records every call so tests can assert on the IPC contract.
    #[derive(Default)]
    struct MockIpc {
        get_calls: usize,
        saved: Vec<AppSettings>,
        cleared: usize,
        next_get: Option<Result<AppSettings, String>>,
        next_save: Option<Result<(), String>>,
        next_clear: Option<Result<(), String>>,
    }

    impl SettingsIpc for MockIpc {
        fn get_settings(&mut self) -> Result<AppSettings, String> {
            self.get_calls += 1;
            self.next_get
                .take()
                .unwrap_or_else(|| Ok(AppSettings::default()))
        }
        fn save_settings(&mut self, settings: &AppSettings) -> Result<(), String> {
            self.saved.push(settings.clone());
            self.next_save.take().unwrap_or(Ok(()))
        }
        fn delete_all_history(&mut self) -> Result<(), String> {
            self.cleared += 1;
            self.next_clear.take().unwrap_or(Ok(()))
        }
    }

    #[test]
    fn settings_save_callback_invokes_ipc_method() {
        // The Save callback (and its `perform_save` core) must:
        //   1. forward the exact AppSettings snapshot to the IPC layer; and
        //   2. translate Ok/Err into the right user-visible status string.
        // Drift on either side breaks the W3 contract — pinned here.
        let mut ipc = MockIpc::default();
        let edited = AppSettings {
            launch_at_login: true,
            private_mode: true,
            history_limit: HistoryLimit::FiveHundred,
            supabase_url: "https://x.supabase.co".into(),
            supabase_key: "anon".into(),
            device_name: "Beta Mac".into(),
        };

        let ok_status = perform_save(&mut ipc, &edited);
        assert_eq!(
            ipc.saved.len(),
            1,
            "save_settings must be called exactly once"
        );
        let recorded = &ipc.saved[0];
        assert!(recorded.launch_at_login);
        assert!(recorded.private_mode);
        assert_eq!(recorded.history_limit, HistoryLimit::FiveHundred);
        assert_eq!(recorded.supabase_url, "https://x.supabase.co");
        assert_eq!(recorded.supabase_key, "anon");
        assert_eq!(recorded.device_name, "Beta Mac");
        assert!(
            !ok_status.is_error,
            "successful save must not mark status as error"
        );
        assert!(
            ok_status.message.contains("saved"),
            "success message should mention 'saved', got: {}",
            ok_status.message
        );

        // Failure path: error message reaches the UI verbatim and is flagged.
        ipc.next_save = Some(Err("daemon offline".into()));
        let err_status = perform_save(&mut ipc, &edited);
        assert!(
            err_status.is_error,
            "failed save must flag the status as an error"
        );
        assert!(
            err_status.message.contains("daemon offline"),
            "error message must include the IPC failure reason, got: {}",
            err_status.message
        );

        // Clear-history path uses the same dispatch shape — sanity-check it
        // here so a future split of the helpers doesn't silently drift.
        let cleared_status = perform_clear_history(&mut ipc);
        assert_eq!(
            ipc.cleared, 1,
            "delete_all_history must be invoked once per click"
        );
        assert!(!cleared_status.is_error);
    }

    #[test]
    fn settings_load_populates_window_fields() {
        // The initial-load helper must:
        //   1. invoke `get_settings` exactly once;
        //   2. surface the returned AppSettings so the window can apply them;
        //   3. on failure, return a flagged status string and leave the
        //      "loaded" slot empty so the constructor defaults persist.
        let mut ipc = MockIpc::default();
        let stored = AppSettings {
            launch_at_login: true,
            private_mode: false,
            history_limit: HistoryLimit::Fifty,
            supabase_url: "https://loaded.example".into(),
            supabase_key: "k".into(),
            device_name: "Loaded Mac".into(),
        };
        ipc.next_get = Some(Ok(stored.clone()));

        let (loaded, err_status) = perform_initial_load(&mut ipc);
        assert_eq!(
            ipc.get_calls, 1,
            "get_settings must be called exactly once on open"
        );
        assert!(
            err_status.is_none(),
            "successful load must not emit an error status"
        );
        let got = loaded.expect("successful load yields the settings the window will apply");
        assert_eq!(got.launch_at_login, stored.launch_at_login);
        assert_eq!(got.private_mode, stored.private_mode);
        assert_eq!(got.history_limit, stored.history_limit);
        assert_eq!(got.supabase_url, stored.supabase_url);
        assert_eq!(got.supabase_key, stored.supabase_key);
        assert_eq!(got.device_name, stored.device_name);

        // Failure path: window stays openable with defaults; status flagged.
        let mut bad_ipc = MockIpc {
            next_get: Some(Err("socket missing".into())),
            ..MockIpc::default()
        };
        let (loaded, err_status) = perform_initial_load(&mut bad_ipc);
        assert!(
            loaded.is_none(),
            "failed load must NOT supply settings — window keeps its defaults"
        );
        let status = err_status.expect("failed load must yield an error status");
        assert!(status.is_error);
        assert!(
            status.message.contains("socket missing"),
            "error message must include the failure reason, got: {}",
            status.message
        );
    }
}
