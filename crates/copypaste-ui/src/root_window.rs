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
// Downstream tasks that will build on this handle:
//   T5.x  — command palette, keyboard shortcuts.
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

use std::path::PathBuf;
use std::rc::Rc;
use std::cell::RefCell;

use slint::{Model as _, SharedString, VecModel};

use copypaste_ui::history_model::HistoryModel;
use copypaste_ui::ipc_client::IpcClient;
use copypaste_ui::ui_prefs::{AccentColor, SettingsTab, UiPrefs};
use crate::{ClipItem, MainWindow};
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
        let history_model = Rc::new(RefCell::new(
            HistoryModel::new(PathBuf::from(socket_path)),
        ));

        // ── Window-side view model (binary-crate ClipItem) ──────────────────
        let view_model: Rc<VecModel<ClipItem>> = Rc::new(VecModel::default());
        window.set_history_items(view_model.clone().into());

        /// Sync all items from `HistoryModel` into the window `VecModel`.
        ///
        /// Called after every fetch (initial, next-page, search reset).
        /// Replaces the entire vec rather than appending to stay in lock-step
        /// with `HistoryModel`'s snapshot; for ≤50-item pages this is fine.
        fn sync_view_model(
            hm: &HistoryModel,
            vm: &VecModel<ClipItem>,
        ) {
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

        // ── item-clicked callback — open detail panel ──────────────────────
        {
            let vm = Rc::clone(&view_model);
            let win_weak = window.as_weak();
            window.on_item_clicked(move |idx| {
                let win = match win_weak.upgrade() {
                    Some(w) => w,
                    None => return,
                };
                match vm.row_data(idx as usize) {
                    Some(clip) => {
                        win.set_detail_item(clip);
                        win.set_detail_visible(true);
                    }
                    None => {
                        tracing::warn!("item-clicked: index {idx} out of bounds");
                    }
                }
            });
        }

        // ── detail-copy callback ───────────────────────────────────────────
        {
            let win_weak = window.as_weak();
            window.on_detail_copy(move || {
                let win = match win_weak.upgrade() {
                    Some(w) => w,
                    None => return,
                };
                let item = win.get_detail_item();
                tracing::info!("detail-copy: id={}", item.id);
                // TODO T4.3: send IPC copy command with item.id
            });
        }

        // ── detail-pin callback ────────────────────────────────────────────
        {
            let win_weak = window.as_weak();
            window.on_detail_pin(move || {
                let win = match win_weak.upgrade() {
                    Some(w) => w,
                    None => return,
                };
                let item = win.get_detail_item();
                tracing::info!("detail-pin: id={} pinned={}", item.id, item.pinned);
                // TODO T4.3: send IPC pin/unpin command with item.id
            });
        }

        // ── detail-delete callback ─────────────────────────────────────────
        {
            let win_weak = window.as_weak();
            window.on_detail_delete(move || {
                let win = match win_weak.upgrade() {
                    Some(w) => w,
                    None => return,
                };
                let item = win.get_detail_item();
                tracing::info!("detail-delete: id={}", item.id);
                win.set_detail_visible(false);
                // TODO T4.3: send IPC delete command with item.id
            });
        }

        // ── Settings wiring ───────────────────────────────────────────────

        // save-prefs: persist UiPrefs fields and apply live
        {
            let win_weak = window.as_weak();
            window.on_save_prefs(move || {
                let Some(win) = win_weak.upgrade() else { return; };
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

        // clear-history: IPC delete_all (stub until daemon implements the verb)
        {
            let socket = socket_path.to_string();
            let model = Rc::clone(&history_model);
            let vm = Rc::clone(&view_model);
            window.on_clear_history(move || {
                match IpcClient::connect(socket.as_ref()) {
                    Ok(_c) => {
                        // TODO: call c.delete_all() once daemon implements the verb.
                        tracing::info!("clear-history: TODO delete_all IPC not yet implemented");
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "clear-history: daemon offline");
                    }
                }
                // Reset the local view optimistically so the list empties.
                model.borrow().reset();
                vm.set_vec(vec![]);
            });
        }

        // reset-pairings: IPC revoke_all_peers (stub until daemon implements it)
        {
            let socket = socket_path.to_string();
            window.on_reset_pairings(move || {
                match IpcClient::connect(socket.as_ref()) {
                    Ok(_c) => {
                        // TODO: call c.revoke_all_peers() once daemon implements the verb.
                        tracing::info!("reset-pairings: TODO revoke_all_peers IPC not yet implemented");
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "reset-pairings: daemon offline");
                    }
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

        Ok(Self { window, history_model, view_model })
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

    /// Hide the window without destroying it.
    pub fn hide(&self) {
        let _ = self.window.hide();
    }

    /// Borrow a weak reference for use in closures.
    pub fn as_weak(&self) -> slint::Weak<MainWindow> {
        self.window.as_weak()
    }
}
