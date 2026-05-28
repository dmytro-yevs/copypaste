// root_window.rs — RootWindowHandle: Rust side of the v0.4 MainWindow shell.
//
// T2.2: wraps the Slint-generated `MainWindow` type (exported from
// `ui/MainWindow.slint` via `ui/appui.slint`) and applies saved `UiPrefs`
// to the window's initial property state before first paint.
//
// T4.2: wires HistoryModel (pagination, search) into MainWindow and connects
// the ItemDetailPanel callbacks (detail-copy, detail-pin, detail-delete).
//
// T4.3: wires Settings view callbacks to IPC + UiPrefs persistence.
//
// T5.2: wires keyboard-shortcut callbacks (item-copy, item-pin, item-delete,
//        request-quit) so the FocusScope in MainWindow.slint can dispatch
//        actions by index without extra IPC plumbing at this stage.
//
// ## Two-crate ClipItem note
//
// The binary crate (`main.rs`) calls `slint::include_modules!()` which
// generates its own `ClipItem` Rust type.  The library crate (`windows.rs`)
// also calls `slint::include_modules!()` from the same `.slint` source, so
// `copypaste_ui::windows::ClipItem` and `crate::ClipItem` are structurally
// identical but distinct Rust types.
//
// `HistoryModel` lives in the lib crate and stores lib-crate `ClipItem`s.
// This file bridges between them via field-for-field conversion.  Pagination
// is driven through `HistoryModel`; a window-side `Rc<VecModel<crate::ClipItem>>`
// is kept in sync and bound to `window.set_history_items(…)`.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use slint::{Model as _, SharedString, VecModel};

use crate::{ClipItem, MainWindow};
use copypaste_ui::history_model::HistoryModel;
use copypaste_ui::ipc_client::IpcClient;
use copypaste_ui::ui_prefs::{AccentColor, SettingsTab, UiPrefs};
use slint::ComponentHandle;

// ---------------------------------------------------------------------------
// Type bridge
// ---------------------------------------------------------------------------

/// Convert a lib-crate `ClipItem` into the binary-crate `ClipItem`.
///
/// Both types are Slint-generated from the same `.slint` source and have
/// identical fields; this is a field-for-field copy.
fn lib_to_bin_clip(src: copypaste_ui::windows::ClipItem) -> ClipItem {
    ClipItem {
        id: src.id,
        preview: src.preview,
        kind: src.kind,
        wall_time: src.wall_time,
        source_device: src.source_device,
        pinned: src.pinned,
        redacted: src.redacted,
    }
}

// ---------------------------------------------------------------------------
// RootWindowHandle
// ---------------------------------------------------------------------------

/// Rust-side handle for the v0.4 `MainWindow` shell.
///
/// Owns both the Slint window, the [`HistoryModel`] (IPC / pagination logic),
/// and the window-side `VecModel<ClipItem>` that is bound to
/// `MainWindow.history-items`.
pub struct RootWindowHandle {
    window: MainWindow,
    /// IPC-backed paginating model; drives fetch logic.
    #[allow(dead_code)]
    history_model: Rc<RefCell<HistoryModel>>,
    /// Window-side view model, bound to `MainWindow.history-items`.
    /// Kept in sync with `history_model` after every fetch.
    #[allow(dead_code)]
    view_model: Rc<VecModel<ClipItem>>,
}

#[allow(dead_code)]
impl RootWindowHandle {
    /// Create the window, apply saved preferences, and wire up the
    /// `HistoryModel` + detail panel.
    ///
    /// An initial page load is kicked off synchronously so the history list
    /// is populated on first paint.  Failures (daemon offline) are logged
    /// but do not prevent the window from opening.
    pub fn new(prefs: &UiPrefs, socket_path: &str) -> anyhow::Result<Self> {
        let window = MainWindow::new()?;

        // ── Preferences (UiPrefs) ──────────────────────────────────────────
        window.set_sidebar_collapsed(prefs.sidebar_collapsed);
        window.set_compact(prefs.compact);
        window.set_vibrancy(prefs.vibrancy);
        window.set_accent(match prefs.accent {
            AccentColor::Blue => 0,
            AccentColor::Purple => 1,
        });
        window.set_settings_tab(match prefs.settings_tab {
            SettingsTab::Simple => 0,
            SettingsTab::Advanced => 1,
        });

        // ── App version ────────────────────────────────────────────────────
        window.set_app_version(SharedString::from(env!("CARGO_PKG_VERSION")));

        // ── Reduced-motion accessibility (T5.4) ───────────────────────────
        // On macOS, read the accessibility display option at startup.
        // The Theme global's `reduced-motion` flag controls all animate durations.
        {
            let reduced = reduced_motion_enabled();
            window.global::<crate::Theme>().set_reduced_motion(reduced);
        }

        // ── Screen width for responsive layout (T5.3) ─────────────────────
        // On Android, set to the device logical width so the bottom-tab layout
        // activates for screens < 600dp. On desktop, the default (900px) keeps
        // the sidebar layout. Android startup code should call
        // `root.set_screen_width(physical_px / scale_factor)` after this.
        // Default 900px triggers sidebar layout on all desktop platforms.
        window.set_screen_width(900.0);

        // ── AppSettings (from IPC, best-effort) ────────────────────────────
        // The IPC wire type carries p2p_enabled + supabase fields only.
        // launch_at_login / private_mode / history_limit are UI-only for now.
        {
            let ipc_settings = IpcClient::connect(socket_path.as_ref())
                .and_then(|mut c| c.get_settings())
                .unwrap_or_default();
            let sync_url = ipc_settings.supabase_url.unwrap_or_default();
            window.set_sync_url(SharedString::from(sync_url.as_str()));
            window.set_sync_enabled(ipc_settings.p2p_enabled);
        }

        // ── Own fingerprint (from IPC, best-effort) ────────────────────────
        {
            let fp = IpcClient::connect(socket_path.as_ref())
                .and_then(|mut c| c.get_own_fingerprint())
                .unwrap_or_default();
            window.set_fingerprint(SharedString::from(&fp));
        }

        // ── History model (IPC-backed) ──────────────────────────────────────
        let history_model = Rc::new(RefCell::new(HistoryModel::new(PathBuf::from(socket_path))));

        // ── Window-side view model (binary-crate ClipItem) ──────────────────
        let view_model: Rc<VecModel<ClipItem>> = Rc::new(VecModel::default());
        window.set_history_items(view_model.clone().into());

        /// Sync all items from `HistoryModel` into the window `VecModel`.
        ///
        /// Called after every fetch (initial, next-page, search reset).
        /// Replaces the entire vec rather than appending to stay in lock-step
        /// with `HistoryModel`'s snapshot; for ≤50-item pages this is fine.
        fn sync_view_model(hm: &HistoryModel, vm: &VecModel<ClipItem>) {
            let lib_rc = hm.as_model_rc();
            let count = lib_rc.row_count();
            let items: Vec<ClipItem> = (0..count)
                .filter_map(|i| lib_rc.row_data(i))
                .map(lib_to_bin_clip)
                .collect();
            vm.set_vec(items);
        }

        // ── fetch-next-page callback ───────────────────────────────────────
        {
            let model = Rc::clone(&history_model);
            let vm = Rc::clone(&view_model);
            window.on_fetch_next_page(move || {
                if let Err(e) = model.borrow().fetch_next_page() {
                    tracing::warn!("fetch-next-page: {e}");
                } else {
                    sync_view_model(&model.borrow(), &vm);
                }
            });
        }

        // ── search-changed callback ────────────────────────────────────────
        {
            let model = Rc::clone(&history_model);
            let vm = Rc::clone(&view_model);
            window.on_search_changed(move |query| {
                if let Err(e) = model.borrow().reset_with_query(query.as_str()) {
                    tracing::warn!("search-changed: {e}");
                }
                // Sync even on error: reset_with_query clears items first,
                // so the view should reflect the empty state.
                sync_view_model(&model.borrow(), &vm);
            });
        }

        // ── item-clicked callback — copy to clipboard + open detail panel ───
        //
        // Clicking a row is the primary "give me that clip back" gesture, so
        // it must copy the item to the system clipboard via the daemon
        // `copy_item` verb. We ALSO open the detail panel so the user can see
        // the full content / pin / delete. Previously this callback only
        // opened the detail panel, so a plain click never copied anything —
        // the user had to find the hover mini-toolbar copy button.
        {
            let vm = Rc::clone(&view_model);
            let win_weak = window.as_weak();
            let socket = socket_path.to_string();
            window.on_item_clicked(move |idx| {
                let win = match win_weak.upgrade() {
                    Some(w) => w,
                    None => return,
                };
                if idx < 0 {
                    return;
                }
                match vm.row_data(idx as usize) {
                    Some(clip) => {
                        let id = clip.id.to_string();
                        tracing::info!("item-clicked (copy): id={id}");
                        // Copy to the system clipboard. Daemon-offline / failure
                        // is logged but must not block opening the detail panel.
                        match IpcClient::connect(socket.as_ref()) {
                            Ok(mut c) => {
                                if let Err(e) = c.copy_item(&id) {
                                    tracing::warn!(error = %e, "item-clicked: IPC copy_item failed");
                                }
                            }
                            Err(e) => {
                                tracing::warn!(error = %e, "item-clicked: daemon offline");
                            }
                        }
                        win.set_detail_item(clip);
                        win.set_detail_visible(true);
                    }
                    None => {
                        tracing::warn!("item-clicked: index {idx} out of bounds");
                    }
                }
            });
        }

        // ── T5.2: request-quit callback (⌘W hides the window) ────────────────
        {
            let win_weak = window.as_weak();
            window.on_request_quit(move || {
                if let Some(win) = win_weak.upgrade() {
                    let _ = win.hide();
                }
            });
        }

        // ── T5.2: item-copy callback (keyboard ↵ / ⌘C, hover toolbar) ───────
        // Wired to IpcClient::copy_item — the daemon decrypts the stored
        // ciphertext and writes plaintext to the system clipboard, returning
        // typed invalid_argument / not_found / auth_failed error codes. This
        // is the correct semantic for an explicit "copy" gesture and matches
        // the detail-panel Copy button (which already uses copy_item).
        {
            let vm = Rc::clone(&view_model);
            let socket = socket_path.to_string();
            window.on_item_copy(move |idx| {
                if idx < 0 {
                    return;
                }
                match vm.row_data(idx as usize) {
                    Some(item) => {
                        let id = item.id.to_string();
                        tracing::info!("item-copy (keyboard): id={id}");
                        match IpcClient::connect(socket.as_ref()) {
                            Ok(mut c) => {
                                if let Err(e) = c.copy_item(&id) {
                                    tracing::warn!(error = %e, "item-copy: IPC copy_item failed");
                                }
                            }
                            Err(e) => {
                                tracing::warn!(error = %e, "item-copy: daemon offline");
                            }
                        }
                    }
                    None => {
                        tracing::warn!("item-copy: index {idx} out of bounds");
                    }
                }
            });
        }

        // ── T5.2: item-pin callback (keyboard ⌘P on selected item) ───────────
        // T5.x: wired to IpcClient::pin_item — toggles the pin flag based on
        // the item's current state so the same key both pins and unpins.
        {
            let vm = Rc::clone(&view_model);
            let socket = socket_path.to_string();
            window.on_item_pin(move |idx| {
                if idx < 0 {
                    return;
                }
                match vm.row_data(idx as usize) {
                    Some(item) => {
                        let id = item.id.to_string();
                        let want_pinned = !item.pinned;
                        tracing::info!("item-pin (keyboard): id={id} pinned={want_pinned}");
                        match IpcClient::connect(socket.as_ref()) {
                            Ok(mut c) => {
                                if let Err(e) = c.pin_item(&id, want_pinned) {
                                    tracing::warn!(error = %e, "item-pin: IPC pin_item failed");
                                }
                            }
                            Err(e) => {
                                tracing::warn!(error = %e, "item-pin: daemon offline");
                            }
                        }
                    }
                    None => {
                        tracing::warn!("item-pin: index {idx} out of bounds");
                    }
                }
            });
        }

        // ── T5.2: item-delete callback (keyboard ⌫ on selected item) ─────────
        // T5.x: wired to IpcClient::delete_item — removes the item from the
        // daemon's store, then optimistically deselects + closes the detail
        // panel so the UI feels responsive regardless of daemon latency.
        {
            let vm = Rc::clone(&view_model);
            let win_weak = window.as_weak();
            let socket = socket_path.to_string();
            window.on_item_delete(move |idx| {
                if idx < 0 {
                    return;
                }
                match vm.row_data(idx as usize) {
                    Some(item) => {
                        let id = item.id.to_string();
                        tracing::info!("item-delete (keyboard): id={id}");
                        match IpcClient::connect(socket.as_ref()) {
                            Ok(mut c) => {
                                if let Err(e) = c.delete_item(&id) {
                                    tracing::warn!(error = %e, "item-delete: IPC delete_item failed");
                                }
                            }
                            Err(e) => {
                                tracing::warn!(error = %e, "item-delete: daemon offline");
                            }
                        }
                        // Deselect and close detail panel optimistically so the UI
                        // feels responsive even when the daemon call is slow.
                        if let Some(win) = win_weak.upgrade() {
                            if win.get_detail_visible() && win.get_detail_item().id == item.id {
                                win.set_detail_visible(false);
                            }
                            win.set_selected_history_index(-1);
                        }
                    }
                    None => {
                        tracing::warn!("item-delete: index {idx} out of bounds");
                    }
                }
            });
        }

        // ── detail-copy callback ───────────────────────────────────────────
        // T4.3 / T5.x: send the daemon `copy_item` verb so the selected entry
        // is decrypted and written back to the system clipboard.
        {
            let win_weak = window.as_weak();
            let socket = socket_path.to_string();
            window.on_detail_copy(move || {
                let win = match win_weak.upgrade() {
                    Some(w) => w,
                    None => return,
                };
                let item = win.get_detail_item();
                let id = item.id.to_string();
                tracing::info!("detail-copy: id={id}");
                match IpcClient::connect(socket.as_ref()) {
                    Ok(mut c) => {
                        if let Err(e) = c.copy_item(&id) {
                            tracing::warn!(error = %e, "detail-copy: IPC copy_item failed");
                        }
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "detail-copy: daemon offline");
                    }
                }
            });
        }

        // ── detail-pin callback ────────────────────────────────────────────
        // T4.3 / T5.x: toggle the pin flag via the daemon `pin_item` verb.
        {
            let win_weak = window.as_weak();
            let socket = socket_path.to_string();
            window.on_detail_pin(move || {
                let win = match win_weak.upgrade() {
                    Some(w) => w,
                    None => return,
                };
                let item = win.get_detail_item();
                let id = item.id.to_string();
                let want_pinned = !item.pinned;
                tracing::info!("detail-pin: id={id} pinned={want_pinned}");
                match IpcClient::connect(socket.as_ref()) {
                    Ok(mut c) => {
                        if let Err(e) = c.pin_item(&id, want_pinned) {
                            tracing::warn!(error = %e, "detail-pin: IPC pin_item failed");
                        }
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "detail-pin: daemon offline");
                    }
                }
            });
        }

        // ── detail-delete callback ─────────────────────────────────────────
        // T4.3 / T5.x: delete the item via the daemon `delete_item` verb, then
        // close the detail panel.
        {
            let win_weak = window.as_weak();
            let socket = socket_path.to_string();
            window.on_detail_delete(move || {
                let win = match win_weak.upgrade() {
                    Some(w) => w,
                    None => return,
                };
                let item = win.get_detail_item();
                let id = item.id.to_string();
                tracing::info!("detail-delete: id={id}");
                win.set_detail_visible(false);
                match IpcClient::connect(socket.as_ref()) {
                    Ok(mut c) => {
                        if let Err(e) = c.delete_item(&id) {
                            tracing::warn!(error = %e, "detail-delete: IPC delete_item failed");
                        }
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "detail-delete: daemon offline");
                    }
                }
            });
        }

        // ── Settings wiring ───────────────────────────────────────────────

        // save-prefs: persist UiPrefs fields and apply live
        {
            let win_weak = window.as_weak();
            window.on_save_prefs(move || {
                let Some(win) = win_weak.upgrade() else {
                    return;
                };
                let mut p = UiPrefs::load().unwrap_or_default();
                p.compact = win.get_compact();
                p.vibrancy = win.get_vibrancy();
                p.accent = match win.get_accent() {
                    1 => AccentColor::Purple,
                    _ => AccentColor::Blue,
                };
                p.sidebar_collapsed = win.get_sidebar_collapsed();
                if let Err(e) = p.save() {
                    tracing::warn!(error = %e, "save-prefs: failed to save ui-prefs");
                }
            });
        }

        // save-app-settings: persist IPC-level settings (p2p + supabase) via daemon.
        // launch_at_login / private_mode / history_limit are UI-only for now.
        {
            let socket = socket_path.to_string();
            let win_weak = window.as_weak();
            window.on_save_app_settings(move || {
                let Some(win) = win_weak.upgrade() else { return; };
                // Read back the current IPC settings first so we preserve
                // any fields we don't expose in this view.
                let mut ipc_settings = IpcClient::connect(socket.as_ref())
                    .and_then(|mut c| c.get_settings())
                    .unwrap_or_default();
                ipc_settings.p2p_enabled = win.get_sync_enabled();
                let url = win.get_sync_url().to_string();
                ipc_settings.supabase_url = if url.is_empty() { None } else { Some(url) };
                match IpcClient::connect(socket.as_ref()) {
                    Ok(mut c) => {
                        if let Err(e) = c.save_settings(&ipc_settings) {
                            tracing::warn!(error = %e, "save-app-settings: IPC save_settings failed");
                        }
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "save-app-settings: daemon offline");
                    }
                }
            });
        }

        // clear-history: IPC delete_all (T5.x — daemon verb now wired)
        {
            let socket = socket_path.to_string();
            let model = Rc::clone(&history_model);
            let vm = Rc::clone(&view_model);
            window.on_clear_history(move || {
                match IpcClient::connect(socket.as_ref()) {
                    Ok(mut c) => match c.delete_all() {
                        Ok(deleted) => {
                            tracing::info!("clear-history: daemon deleted {deleted} items");
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, "clear-history: IPC delete_all failed");
                        }
                    },
                    Err(e) => {
                        tracing::warn!(error = %e, "clear-history: daemon offline");
                    }
                }
                // Reset the local view optimistically so the list empties.
                model.borrow().reset();
                vm.set_vec(vec![]);
            });
        }

        // reset-pairings: IPC revoke_all_peers (T5.x — daemon verb now wired)
        {
            let socket = socket_path.to_string();
            window.on_reset_pairings(move || match IpcClient::connect(socket.as_ref()) {
                Ok(mut c) => match c.revoke_all_peers() {
                    Ok(revoked) => {
                        tracing::info!("reset-pairings: daemon revoked {revoked} peers");
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "reset-pairings: IPC revoke_all_peers failed");
                    }
                },
                Err(e) => {
                    tracing::warn!(error = %e, "reset-pairings: daemon offline");
                }
            });
        }

        // reset-ui-prefs: delete the file and reload defaults into window
        {
            let win_weak = window.as_weak();
            window.on_reset_ui_prefs(move || {
                let path = UiPrefs::prefs_path();
                if path.exists() {
                    if let Err(e) = std::fs::remove_file(&path) {
                        tracing::warn!(error = %e, "reset-ui-prefs: failed to delete prefs file");
                    }
                }
                let defaults = UiPrefs::default();
                if let Err(e) = defaults.save() {
                    tracing::warn!(error = %e, "reset-ui-prefs: failed to save defaults");
                }
                if let Some(win) = win_weak.upgrade() {
                    win.set_compact(defaults.compact);
                    win.set_vibrancy(defaults.vibrancy);
                    win.set_accent(match defaults.accent {
                        AccentColor::Blue => 0,
                        AccentColor::Purple => 1,
                    });
                    win.set_settings_tab(match defaults.settings_tab {
                        SettingsTab::Simple => 0,
                        SettingsTab::Advanced => 1,
                    });
                    win.set_sidebar_collapsed(defaults.sidebar_collapsed);
                }
            });
        }

        // settings-tab-changed: persist the active sub-tab
        {
            window.on_settings_tab_changed(move |tab| {
                let mut p = UiPrefs::load().unwrap_or_default();
                p.settings_tab = match tab {
                    1 => SettingsTab::Advanced,
                    _ => SettingsTab::Simple,
                };
                if let Err(e) = p.save() {
                    tracing::warn!(error = %e, "settings-tab-changed: failed to save ui-prefs");
                }
            });
        }

        // ── Initial page load ──────────────────────────────────────────────
        // Synchronous; daemon-offline error is logged but non-fatal.
        {
            let borrow = history_model.borrow();
            if let Err(e) = borrow.fetch_next_page() {
                tracing::warn!("initial history load failed (daemon offline?): {e}");
            }
            sync_view_model(&borrow, &view_model);
        }

        // ── Live auto-refresh (ISSUE 1) ─────────────────────────────────────
        // The daemon has no change/subscribe verb, so we poll. Re-fetch the
        // first page (preserving the active search query) on a repeating
        // timer so newly-copied clips appear without a manual refresh. We skip
        // the IPC round-trip entirely when the window is hidden, and skip
        // while a fetch is already in flight (HistoryModel guards `loading`)
        // so a pagination fetch is never clobbered.
        //
        // `reset_with_query` re-fetches from offset 0 — newest items sort to
        // the top, which is exactly what "show me new clips" needs. Any extra
        // pages the user had scrolled to are re-loadable via scroll.
        {
            const ROOT_AUTO_REFRESH_INTERVAL: std::time::Duration =
                std::time::Duration::from_millis(1500);
            let win_weak = window.as_weak();
            let model = Rc::clone(&history_model);
            let vm = Rc::clone(&view_model);
            let timer = slint::Timer::default();
            timer.start(
                slint::TimerMode::Repeated,
                ROOT_AUTO_REFRESH_INTERVAL,
                move || {
                    let Some(win) = win_weak.upgrade() else {
                        return;
                    };
                    if !win.window().is_visible() {
                        return;
                    }
                    // Don't fight an in-flight pagination / search fetch.
                    if model.borrow().is_loading() {
                        return;
                    }
                    let query = win.get_search_query().to_string();
                    if let Err(e) = model.borrow().reset_with_query(&query) {
                        tracing::debug!("auto-refresh: reset_with_query failed: {e}");
                    }
                    sync_view_model(&model.borrow(), &vm);
                },
            );
            // Leak so the timer ticks for the lifetime of the window.
            std::mem::forget(timer);
        }

        Ok(Self {
            window,
            history_model,
            view_model,
        })
    }

    /// Set the app-version string shown in the sidebar footer.
    pub fn set_app_version(&self, version: &str) {
        self.window.set_app_version(version.into());
    }

    /// Make the window visible.
    ///
    /// Callers are responsible for calling `set_activation_policy_regular()`
    /// and `activate_app()` on macOS before or after this call, matching the
    /// pattern used for the legacy history window.
    pub fn show(&self) {
        let _ = self.window.show();
    }

    /// Show the window and run the Slint event loop until the loop is quit.
    ///
    /// This is the primary-window entry point: `main()` calls it once the
    /// window + tray + background workers are wired, and it blocks for the
    /// lifetime of the process (returning only when `slint::quit_event_loop`
    /// fires, e.g. from the tray "Quit" item).
    pub fn run(&self) -> Result<(), slint::PlatformError> {
        self.window.run()
    }

    /// Hide the window without destroying it.
    pub fn hide(&self) {
        let _ = self.window.hide();
    }

    /// Borrow a weak reference for use in closures.
    pub fn as_weak(&self) -> slint::Weak<MainWindow> {
        self.window.as_weak()
    }
}

// ---------------------------------------------------------------------------
// Accessibility helpers
// ---------------------------------------------------------------------------

/// Returns true if the OS has the "Reduce motion" accessibility setting active.
/// On macOS uses `NSWorkspace accessibilityDisplayShouldReduceMotion` (macOS 10.12+).
/// On all other platforms returns false.
fn reduced_motion_enabled() -> bool {
    #[cfg(target_os = "macos")]
    {
        use objc2::rc::Retained;
        use objc2::runtime::{AnyClass, AnyObject, Bool};

        unsafe {
            // +[NSWorkspace sharedWorkspace]
            let cls = AnyClass::get(c"NSWorkspace").expect("NSWorkspace class must exist on macOS");
            let shared: Retained<AnyObject> = objc2::msg_send![cls, sharedWorkspace];
            // -[NSWorkspace accessibilityDisplayShouldReduceMotion]
            let result: Bool = objc2::msg_send![&*shared, accessibilityDisplayShouldReduceMotion];
            result.as_bool()
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}
