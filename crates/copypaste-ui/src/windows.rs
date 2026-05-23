// windows.rs — Rust handles for SettingsWindow and PairWindow
// Wraps the Slint-generated component types with ergonomic constructors and callback wiring.

use slint::{ComponentHandle, Model, VecModel};
use std::rc::Rc;

use crate::settings::{AppSettings, HistoryLimit, PairedDevice};
use crate::fingerprint::{format_fingerprint_long, format_fingerprint_truncated};

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
pub fn image_preview_label(
    width: Option<u32>,
    height: Option<u32>,
    bytes: Option<u64>,
) -> String {
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
        self.window.set_own_fingerprint(format_fingerprint_long(raw_hex).into());
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
        self.window.set_status_message(slint::SharedString::default());
    }

    /// Register a callback invoked when "Pair" is clicked.
    /// Receives the fingerprint string entered by the user.
    pub fn on_pair<F: Fn(String) + 'static>(&self, cb: F) {
        self.window.on_pair(move |fp| cb(fp.to_string()));
    }
    /// Register a callback invoked when "Pair with Password" is clicked.
    /// Receives `(peer_fingerprint, password)`. The closure is responsible
    /// for calling `IpcClient::pair_with_password` and surfacing the
    /// daemon's success/error status back into the UI via [`set_status`].
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
        assert!(!s.contains('·'), "no size separator when bytes missing: {s}");
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
}
