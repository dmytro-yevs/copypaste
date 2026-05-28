//! copypaste-ui — Slint MainWindow (v0.4 RootWindow) wired to copypaste-daemon
//! via Unix IPC.
//!
//! Architecture:
//!   - Slint renders the redesigned single-window `MainWindow` on the main
//!     thread. The Rust side lives in [`root_window::RootWindowHandle`], which
//!     owns the `HistoryModel` (IPC / pagination), the detail panel callbacks,
//!     and a visibility-gated live auto-refresh timer.
//!   - The auxiliary Settings / Pair windows and the macOS menu-bar tray are
//!     constructed here and wired to the same daemon socket.
//!   - IPC methods: `history_page` (list), `copy_item` (copy by id), etc.
//!
//! Data flow:
//!   Slint callback → RootWindowHandle closure → IPC call → Slint update

mod ipc_client;
mod root_window;

use anyhow::Result;
use ipc_client::IpcClient;
use std::path::PathBuf;

// Include generated Slint bindings.
slint::include_modules!();

/// Resolve the daemon IPC socket path.
///
/// H2: return Result so a missing HOME directory surfaces a clear error
/// instead of a panic that kills the process with no user-visible message.
fn daemon_socket_path() -> anyhow::Result<PathBuf> {
    Ok(home::home_dir()
        .ok_or_else(|| anyhow::anyhow!("HOME directory not found; cannot start UI"))?
        .join("Library/Application Support/CopyPaste/daemon.sock"))
}

/// Bring the UI process to the foreground.
///
/// Starting as `.accessory` (tray-only, set via `set_activation_policy_accessory()`
/// at launch instead of the former `LSUIElement=true` plist key) means a
/// freshly-shown window stays *behind* whatever the user was using. `NSApplication::activate` is the modern (Sonoma+) replacement for
/// the deprecated `activateIgnoringOtherApps:` and works back to macOS 11 via
/// the AppKit shim.
///
/// Tray menu callbacks are invoked from the Slint event loop, which runs on
/// the main thread on macOS, so `MainThreadMarker::new()` should always
/// succeed here. If it ever returns `None` we silently no-op — losing focus
/// activation is preferable to crashing the UI.
#[cfg(target_os = "macos")]
fn activate_app() {
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSApplication;
    if let Some(mtm) = MainThreadMarker::new() {
        let app = NSApplication::sharedApplication(mtm);
        app.activate();
    }
}

#[cfg(not(target_os = "macos"))]
fn activate_app() {}

/// Switch to `.regular` activation policy so the app appears in cmd-tab and
/// the Dock while the main window is visible.
///
/// Called every time the main window is shown via the tray "Open" action. The
/// policy change is idempotent — calling it when already `.regular` is a no-op.
/// Returns whether the change succeeded (failures are logged but not fatal;
/// worst case the user cannot cmd-tab to the window).
///
/// Note: `setActivationPolicy:` must be called on the main thread. Tray
/// callbacks run on the Slint event loop, which IS the main thread on macOS,
/// so `MainThreadMarker::new()` succeeds here in the same way it does in
/// `activate_app`.
#[cfg(target_os = "macos")]
fn set_activation_policy_regular() {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
    if let Some(mtm) = MainThreadMarker::new() {
        let app = NSApplication::sharedApplication(mtm);
        let ok = app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
        if !ok {
            tracing::warn!("setActivationPolicy(.regular) returned false — cmd-tab may not work");
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn set_activation_policy_regular() {}

/// Switch back to `.accessory` (tray-only) policy once no window is visible.
///
/// Called when the main window is closed. After macOS 10.9 the policy may be
/// toggled in either direction; the function logs a warning if the system
/// refuses the change (which should not happen on supported OS versions).
///
/// Apple note: changing *to* `.accessory` hides the Dock icon and removes the
/// app from cmd-tab immediately. The tray icon is unaffected — it continues to
/// work because it is driven by the Slint event loop, not the Dock.
#[cfg(target_os = "macos")]
fn set_activation_policy_accessory() {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
    if let Some(mtm) = MainThreadMarker::new() {
        let app = NSApplication::sharedApplication(mtm);
        let ok = app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
        if !ok {
            tracing::warn!(
                "setActivationPolicy(.accessory) returned false — app may stay in cmd-tab"
            );
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn set_activation_policy_accessory() {}

fn main() -> Result<()> {
    // Beta-bonus i18n: bind the gettext domain (auto-set from CARGO_PKG_NAME =
    // "copypaste-ui" by slint-build) to the `lang/` catalog directory shipped
    // with the crate. At runtime Slint resolves `@tr("…")` against
    // `lang/<locale>/LC_MESSAGES/copypaste-ui.mo`; missing locales fall back
    // to the literal msgid. Locale is selected by LC_ALL / LANG / LC_MESSAGES.
    slint::init_translations!(concat!(env!("CARGO_MANIFEST_DIR"), "/lang"));

    let socket_path = daemon_socket_path()?;

    // ── Settings window ────────────────────────────────────────────────────────
    // Constructed eagerly so it is ready when the tray "Preferences" item fires.
    // The IPC adapter bridges `ipc_client::IpcClient` (binary-private) to the
    // library's `SettingsIpc` trait without leaking IPC types into the lib crate.
    let socket_path_for_settings = socket_path.clone();
    // Try to fetch the current settings for the initial window population; fall
    // back to defaults if the daemon is offline at startup.
    let (initial_settings, initial_fp) = {
        use copypaste_ui::settings::{AppSettings as UiSettings, HistoryLimit};
        match IpcClient::connect(&socket_path_for_settings)
            .map_err(|e| e.to_string())
            .and_then(|mut c| {
                let s = c.get_settings().map_err(|e| e.to_string())?;
                let fp = c.get_own_fingerprint().map_err(|e| e.to_string())?;
                Ok((s, fp))
            }) {
            Ok((s, fp)) => {
                let ui = UiSettings {
                    launch_at_login: false,
                    private_mode: false,
                    history_limit: HistoryLimit::Hundred,
                    supabase_url: s.supabase_url.unwrap_or_default(),
                    supabase_key: s.supabase_anon_key.unwrap_or_default(),
                    device_name: String::from("My Mac"),
                };
                (ui, fp)
            }
            Err(e) => {
                tracing::warn!(error = %e, "settings pre-load failed — using defaults");
                (UiSettings::default(), String::new())
            }
        }
    };
    let settings_window = copypaste_ui::windows::SettingsWindowHandle::new(
        &initial_settings,
        env!("CARGO_PKG_VERSION"),
        &initial_fp,
    )?;
    // Wire Save / Clear History via the SettingsIpc adapter.
    {
        use copypaste_ui::settings::{AppSettings as UiSettings, HistoryLimit};
        use copypaste_ui::windows::SettingsIpc;
        use std::ops::Not;
        struct IpcAdapter(std::path::PathBuf);
        impl SettingsIpc for IpcAdapter {
            fn get_settings(&mut self) -> Result<UiSettings, String> {
                let mut c = IpcClient::connect(&self.0).map_err(|e| e.to_string())?;
                let s = c.get_settings().map_err(|e| e.to_string())?;
                Ok(UiSettings {
                    launch_at_login: false,
                    private_mode: false,
                    history_limit: HistoryLimit::Hundred,
                    supabase_url: s.supabase_url.unwrap_or_default(),
                    supabase_key: s.supabase_anon_key.unwrap_or_default(),
                    device_name: String::from("My Mac"),
                })
            }
            fn save_settings(&mut self, settings: &UiSettings) -> Result<(), String> {
                let mut c = IpcClient::connect(&self.0).map_err(|e| e.to_string())?;
                let ipc_settings = ipc_client::AppSettings {
                    p2p_enabled: settings.supabase_url.is_empty().not(),
                    supabase_url: if settings.supabase_url.is_empty() {
                        None
                    } else {
                        Some(settings.supabase_url.clone())
                    },
                    supabase_anon_key: if settings.supabase_key.is_empty() {
                        None
                    } else {
                        Some(settings.supabase_key.clone())
                    },
                };
                c.save_settings(&ipc_settings).map_err(|e| e.to_string())
            }
            fn delete_all_history(&mut self) -> Result<(), String> {
                // T5.x: wired to the daemon `delete_all` verb via IpcClient.
                let mut c = IpcClient::connect(&self.0).map_err(|e| e.to_string())?;
                let deleted = c.delete_all().map_err(|e| e.to_string())?;
                tracing::info!("delete_all_history: daemon deleted {deleted} items");
                Ok(())
            }
        }
        let adapter = std::rc::Rc::new(std::cell::RefCell::new(IpcAdapter(socket_path.clone())));
        if let Err(e) = settings_window.wire_to_ipc(adapter) {
            // M5: upgraded from warn — a failed initial load means settings are
            // in an unknown state; user will see stale/default values until restart.
            // TODO(M5-full): surface this in a Slint error banner property.
            tracing::error!(error = %e, "settings initial load failed — settings will show defaults");
        }
    }
    // Close button hides the window (does not quit).
    {
        let sw = settings_window.as_weak();
        settings_window.on_close(move || {
            if let Some(w) = sw.upgrade() {
                w.hide().ok();
            }
        });
    }

    // ── Pair window ────────────────────────────────────────────────────────────
    let socket_path_for_pair = socket_path.clone();
    let (own_fp, paired_devices) = {
        match IpcClient::connect(&socket_path_for_pair)
            .map_err(|e| e.to_string())
            .and_then(|mut c| {
                let fp = c.get_own_fingerprint().map_err(|e| e.to_string())?;
                let peers = c.list_peers().map_err(|e| e.to_string())?;
                let pd: Vec<copypaste_ui::settings::PairedDevice> = peers
                    .into_iter()
                    .map(|p| copypaste_ui::settings::PairedDevice::new(p.name, p.fingerprint))
                    .collect();
                Ok((fp, pd))
            }) {
            Ok((fp, pd)) => (fp, pd),
            Err(e) => {
                tracing::warn!(error = %e, "pair window pre-load failed — using defaults");
                (String::new(), vec![])
            }
        }
    };
    let pair_window = copypaste_ui::windows::PairWindowHandle::new(&own_fp, &paired_devices)?;
    // Wire Pair / Remove / Revoke / Close callbacks.
    {
        let socket = socket_path_for_pair.clone();
        let pw = pair_window.as_weak();
        pair_window.on_pair(move |fp| {
            let socket = socket.clone();
            let pw = pw.clone();
            std::thread::spawn(move || {
                let result = IpcClient::connect(&socket)
                    .map_err(|e| e.to_string())
                    .and_then(|mut c| c.pair_peer(&fp, "").map_err(|e| e.to_string()));
                let (msg, is_err) = match result {
                    Ok(()) => ("Paired successfully.".to_string(), false),
                    Err(e) => (format!("Pair failed: {e}"), true),
                };
                slint::invoke_from_event_loop(move || {
                    if let Some(w) = pw.upgrade() {
                        w.set_status_message(msg.into());
                        w.set_status_is_error(is_err);
                    }
                })
                .ok();
            });
        });
    }
    {
        let socket = socket_path_for_pair.clone();
        let pw = pair_window.as_weak();
        pair_window.on_remove_peer(move |fp| {
            let socket = socket.clone();
            let pw = pw.clone();
            std::thread::spawn(move || {
                let result = IpcClient::connect(&socket)
                    .map_err(|e| e.to_string())
                    .and_then(|mut c| c.unpair_peer(&fp).map_err(|e| e.to_string()));
                let (msg, is_err) = match result {
                    Ok(()) => ("Device removed.".to_string(), false),
                    Err(e) => (format!("Remove failed: {e}"), true),
                };
                slint::invoke_from_event_loop(move || {
                    if let Some(w) = pw.upgrade() {
                        w.set_status_message(msg.into());
                        w.set_status_is_error(is_err);
                    }
                })
                .ok();
            });
        });
    }
    {
        let socket = socket_path_for_pair.clone();
        let pw = pair_window.as_weak();
        pair_window.on_revoke_peer(move |fp| {
            let socket = socket.clone();
            let pw = pw.clone();
            std::thread::spawn(move || {
                let result = IpcClient::connect(&socket)
                    .map_err(|e| e.to_string())
                    .and_then(|mut c| c.revoke_peer(&fp).map(|_| ()).map_err(|e| e.to_string()));
                let (msg, is_err) = match result {
                    Ok(()) => ("Device revoked.".to_string(), false),
                    Err(e) => (format!("Revoke failed: {e}"), true),
                };
                slint::invoke_from_event_loop(move || {
                    if let Some(w) = pw.upgrade() {
                        w.set_status_message(msg.into());
                        w.set_status_is_error(is_err);
                    }
                })
                .ok();
            });
        });
    }
    {
        let pw = pair_window.as_weak();
        pair_window.on_close(move || {
            if let Some(w) = pw.upgrade() {
                w.hide().ok();
            }
        });
    }

    // ── v0.4 RootWindow (now the primary surface) ───────────────────────────
    // Load UI preferences from disk and construct the redesigned single-window
    // shell. This is the window the Slint event loop runs (see `run()` at the
    // bottom of `main`); it owns its own IPC-backed `HistoryModel`, detail
    // panel, ⌘K command palette, and a visibility-gated live auto-refresh
    // timer (installed inside `RootWindowHandle::new`).
    let ui_prefs = copypaste_ui::ui_prefs::UiPrefs::load().unwrap_or_else(|e| {
        tracing::warn!(error = %e, "ui-prefs load failed — using defaults");
        copypaste_ui::ui_prefs::UiPrefs::default()
    });
    let socket_path_str = socket_path.to_string_lossy().into_owned();
    let root_window_handle = root_window::RootWindowHandle::new(&ui_prefs, &socket_path_str)?;
    root_window_handle.set_app_version(env!("CARGO_PKG_VERSION"));

    // macOS vibrancy stub — full implementation deferred to T3.5.
    // When T3.5 lands, replace this comment with:
    //   #[cfg(target_os = "macos")]
    //   macos_vibrancy::apply_to_root_window(&root_window_handle);

    // Beta hot-fix: on macOS, install the Launch Agent plist + bootstrap the
    // daemon in the background so the user does not have to run
    // `copypaste daemon install && copypaste daemon start` after a fresh DMG
    // install. Runs in a dedicated thread — UI rendering must NOT block on
    // launchctl. See `crates/copypaste-ui/src/autostart.rs` for the flow.
    //
    // Once the daemon comes up, the RootWindow's own visibility-gated
    // auto-refresh timer (1.5 s cadence, installed in `RootWindowHandle::new`)
    // picks up the freshly-available history without any extra wiring here —
    // the window is shown at startup via `run()`, so the timer is live.
    #[cfg(target_os = "macos")]
    {
        std::thread::spawn(
            move || match copypaste_ui::autostart::ensure_daemon_running() {
                Ok(copypaste_ui::autostart::DaemonStatus::AlreadyRunning) => {
                    eprintln!("[autostart] daemon already running");
                }
                Ok(copypaste_ui::autostart::DaemonStatus::Started) => {
                    eprintln!("[autostart] daemon started via launchctl");
                }
                Ok(copypaste_ui::autostart::DaemonStatus::FailedToStart(reason)) => {
                    eprintln!("[autostart] daemon failed to start: {reason}");
                }
                Err(e) => {
                    eprintln!("[autostart] error: {e}");
                }
            },
        );
    }

    // v0.3: install the macOS menu-bar tray BEFORE Slint takes over the main
    // run loop. The tray host registers a slint::Timer that polls menu events
    // on the UI thread, so we never spin a competing native run loop.
    // Failure is non-fatal — log + continue as a window-only app.
    //
    // Bug-1 fix: LSUIElement=true has been removed from Info.plist so that
    // cmd-tab works when a window is visible. We start hidden by calling
    // set_activation_policy_accessory() here — same tray-only behaviour as
    // LSUIElement, but allows runtime flip to .regular when opening a window.
    #[cfg(target_os = "macos")]
    set_activation_policy_accessory();

    #[cfg(target_os = "macos")]
    {
        // Tray "Open History" → raise the redesigned RootWindow.
        let rw_weak = root_window_handle.as_weak();
        let on_open_history: copypaste_ui::tray_host::ActionCb = Box::new(move || {
            if let Some(win) = rw_weak.upgrade() {
                // Switch to .regular so the app appears in cmd-tab / Dock
                // while the window is visible.
                set_activation_policy_regular();
                win.show().ok();
                // .accessory-policy apps stay in the background after
                // `show()` — focus stays on whatever the user had last.
                // NSApplication::activate brings the process + its visible
                // windows to the foreground.
                activate_app();
            }
        });

        // Tray "Preferences" → show the Settings window.
        let sw_weak = settings_window.as_weak();
        let on_open_preferences: copypaste_ui::tray_host::ActionCb = Box::new(move || {
            if let Some(w) = sw_weak.upgrade() {
                set_activation_policy_regular();
                w.show().ok();
                activate_app();
            }
        });

        // Bug-6: tray "Pair Device…" → show the PairWindow.
        let pw_weak = pair_window.as_weak();
        let on_open_pair: copypaste_ui::tray_host::ActionCb = Box::new(move || {
            if let Some(w) = pw_weak.upgrade() {
                set_activation_policy_regular();
                w.show().ok();
                activate_app();
            }
        });

        // v0.3 T3: tray "Recent items" row click → paste via IPC. The
        // closure spawns a worker thread so we never block the UI on the
        // daemon socket (paste involves a write + ack).
        let paste_socket = socket_path.clone();
        let on_paste_item: copypaste_ui::tray_host::PasteCb = Box::new(move |id: &str| {
            let socket = paste_socket.clone();
            let id_owned = id.to_string();
            std::thread::spawn(move || {
                if let Err(e) = paste_item(&socket, &id_owned) {
                    tracing::warn!(error = %e, id = %id_owned, "tray paste failed");
                }
            });
        });
        let callbacks = copypaste_ui::tray_host::TrayCallbacks {
            on_open_history: Some(on_open_history),
            on_open_preferences: Some(on_open_preferences),
            on_open_pair: Some(on_open_pair),
            on_quit: None, // default = slint::quit_event_loop()
            on_paste_item: Some(on_paste_item),
            // The redesign is the default window now — no separate "preview"
            // entry. "Open History" raises the RootWindow.
            on_open_preview: None,
        };
        let tray_socket = socket_path.clone();
        if let Err(e) = copypaste_ui::tray_host::install(tray_socket, callbacks) {
            eprintln!("[tray] install failed: {e} — running without menu-bar tray");
        } else {
            // v0.3 T3: prime the tray with current history immediately and
            // then refresh on a slint::Timer so changes show up without
            // requiring the user to open the history window first.
            spawn_tray_recents_refresh(socket_path.clone());
        }
    }

    // v0.3 in-app updater (Homebrew Cask, ADR-012) ------------------------
    //
    // Periodically asks `brew outdated --cask copypaste` if a newer version
    // is published. For v0.3 we log the outcome; a follow-up wire-up will
    // surface a banner / tray-menu badge through a Slint `updates-available`
    // property. The check is macOS-only because the daemon is macOS-only;
    // gating with `cfg!(target_os = "macos")` keeps cross-compile / CI on
    // other hosts free of spurious `brew` calls.
    #[cfg(target_os = "macos")]
    std::thread::spawn(|| {
        use copypaste_ui::updater::{self, SystemRunner, UpdateStatus};
        loop {
            match updater::check_for_update(&SystemRunner) {
                UpdateStatus::UpdateAvailable(info) => {
                    // TODO(v0.3-followup): hook into Slint `updates-available`
                    // property + tray-menu "Update to vX" item.
                    eprintln!(
                        "[updater] update available: {} → {}",
                        info.current_version, info.latest_version
                    );
                }
                UpdateStatus::UpToDate => {
                    eprintln!("[updater] up to date");
                }
                UpdateStatus::BrewNotInstalled => {
                    eprintln!("[updater] brew not installed; in-app auto-update unavailable");
                    // No point in retrying every 24h if brew is absent.
                    break;
                }
                UpdateStatus::CheckFailed(e) => {
                    eprintln!("[updater] check failed: {e}");
                }
            }
            std::thread::sleep(updater::CHECK_INTERVAL);
        }
    });

    // --- Wire: main window close → revert to .accessory activation policy ---
    //
    // When the user closes the main window the app should disappear from
    // cmd-tab and the Dock again (back to tray-only / .accessory behaviour).
    // `window().on_close_requested()` fires on the Slint event loop (main
    // thread on macOS) which is where `setActivationPolicy:` must be called.
    //
    // We return `CloseRequestResponse::HideWindow` (the default) to preserve
    // Slint's hide-on-close behaviour — the process stays alive and the tray
    // continues to work.
    #[cfg(target_os = "macos")]
    {
        if let Some(win) = root_window_handle.as_weak().upgrade() {
            win.window().on_close_requested(|| {
                set_activation_policy_accessory();
                slint::CloseRequestResponse::HideWindow
            });
        }
    }

    // Run the redesigned RootWindow as the primary surface; this shows the
    // window and enters the Slint event loop, blocking until the loop is quit
    // (e.g. from the tray "Quit" item).
    root_window_handle.run()?;
    Ok(())
}

/// Call `history_page` on the daemon and return parsed results.
///
/// beta.5 Bug-3: stop collapsing every connect error into "daemon offline:" —
/// returns the raw [`IpcError`] display, which already formats
/// `IpcError::DaemonOffline` as "Daemon not running...".
///
/// macOS-only: the only caller is the tray "Recent items" refresh, which is
/// gated behind `#[cfg(target_os = "macos")]`.
#[cfg(target_os = "macos")]
fn load_history_page(
    socket_path: &std::path::Path,
    limit: u64,
    offset: u64,
) -> std::result::Result<ipc_client::HistoryPage, String> {
    let mut client = IpcClient::connect(socket_path).map_err(|e| ipc_error_to_string(&e))?;
    client
        .history_page(limit, offset)
        .map_err(|e| e.to_string())
}

/// Map an `anyhow::Error` produced by `IpcClient::connect` to a display
/// string that preserves the underlying [`ipc_client::IpcError`] formatting
/// when present (so `DaemonOffline` keeps its actionable "Daemon not running"
/// prefix). Falls back to the raw error display for non-Ipc errors.
///
/// macOS-only: only reachable via [`load_history_page`] on the tray refresh
/// path.
#[cfg(target_os = "macos")]
fn ipc_error_to_string(e: &anyhow::Error) -> String {
    if let Some(ipc_err) = e.downcast_ref::<ipc_client::IpcError>() {
        ipc_err.to_string()
    } else {
        e.to_string()
    }
}

/// Call `paste` on the daemon.
///
/// Retained for the tray "Recent items" row action, which keeps the legacy
/// `paste` semantic. The in-window click path uses the daemon `copy_item` verb
/// (wired inside `RootWindowHandle`).
///
/// macOS-only: invoked from the tray "Recent items" row callback.
#[cfg(target_os = "macos")]
fn paste_item(socket_path: &std::path::Path, id: &str) -> std::result::Result<String, String> {
    let mut client = IpcClient::connect(socket_path).map_err(|e| format!("daemon offline: {e}"))?;
    client.paste(id).map_err(|e| e.to_string())
}

/// v0.3 T3: keep the tray's "Recent items" block in sync with the daemon.
///
/// Polls `history_page(MAX_TRAY_RECENTS, 0)` every `TRAY_REFRESH_INTERVAL`
/// from a Slint timer on the UI thread, then hands the result to
/// `tray_host::update_recents` (which mutates muda menu state — must run
/// main-thread on macOS).
///
/// The IPC call itself runs synchronously inside the timer tick because
/// (a) it's bounded at MAX_TRAY_RECENTS rows so latency is in the low
/// milliseconds, and (b) `slint::Image` types on the row Vec aren't
/// involved here — the tray only needs id/preview/wall_time/type, all
/// `Send`. If profiling shows hitches we can move the read off-thread
/// and post-back via `invoke_from_event_loop`.
#[cfg(target_os = "macos")]
fn spawn_tray_recents_refresh(socket_path: PathBuf) {
    use copypaste_ui::tray_host::{update_recents, RecentTrayItem, MAX_TRAY_RECENTS};
    use std::time::Duration;

    // Refresh cadence — 5s is fast enough to feel live without burning
    // socket bandwidth. The IPC server is a single-threaded unix socket
    // so we keep concurrent reads modest.
    const TRAY_REFRESH_INTERVAL: Duration = Duration::from_secs(5);

    let refresh = move || {
        let socket = socket_path.clone();
        // Off-thread fetch so the tick stays cheap; post results back via
        // invoke_from_event_loop because `update_recents` is main-thread-
        // only on macOS (muda::Menu).
        std::thread::spawn(move || {
            let result = load_history_page(&socket, MAX_TRAY_RECENTS as u64, 0);
            let recents: Vec<RecentTrayItem> = match result {
                Ok(page) => page
                    .items
                    .into_iter()
                    .map(|e| RecentTrayItem {
                        id: e.id,
                        content_type: e.content_type,
                        preview: e.preview,
                        wall_time_ms: e.wall_time,
                    })
                    .collect(),
                Err(e) => {
                    tracing::debug!(error = %e, "tray refresh: history_page failed");
                    return;
                }
            };
            if let Err(e) = slint::invoke_from_event_loop(move || {
                update_recents(recents);
            }) {
                tracing::debug!(error = %e, "ui update dropped during event-loop shutdown");
            }
        });
    };

    // Prime once immediately so the first menu open shows real history.
    refresh();

    // Repeating timer on the Slint event loop.
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        TRAY_REFRESH_INTERVAL,
        move || {
            refresh();
        },
    );
    // Leak so the timer outlives this scope.
    std::mem::forget(timer);
}
