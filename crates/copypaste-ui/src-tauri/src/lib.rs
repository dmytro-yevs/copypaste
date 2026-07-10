//! Tauri desktop UI for CopyPaste — a thin shell. The React frontend talks to
//! the daemon over the Unix-socket IPC via the `ipc_call` command (`ipc.rs`).
//! This crate never links `copypaste-core`; all data access is IPC-only.

mod config;
mod daemon_lifecycle;
mod ipc;
mod notifications;
mod pairing;
mod popup;
mod tray;

#[cfg(target_os = "macos")]
mod event_tap;

use config::{
    apply_launch_at_login, load_ui_config, AllowScreenshots, CurrentPopupPosition, CurrentShortcut,
    UiConfig,
};
use pairing::PairingPollStop;
use std::sync::Mutex;
use tauri::{Listener, Manager, State};
use tray::{PrivateModeMenuItem, RecentSubmenu, TrayResyncStop};

// ---------------------------------------------------------------------------
// Tauri entry point
// ---------------------------------------------------------------------------

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let cfg = UiConfig::default(); // will be overwritten in setup after handle is available
    tauri::Builder::default()
        .manage(CurrentShortcut(Mutex::new(cfg.popup_shortcut)))
        .manage(CurrentPopupPosition(Mutex::new(cfg.popup_position)))
        // CopyPaste-6uy9: allow-screenshots state — initialised to the UiConfig
        // default (false = protection ON); overwritten in setup once the handle
        // is available and the persisted config is read.
        .manage(AllowScreenshots(Mutex::new(cfg.allow_screenshots)))
        .manage(daemon_lifecycle::DaemonChild::default())
        .manage(daemon_lifecycle::DaemonSpawnError::default())
        .manage(daemon_lifecycle::DaemonLifecycleGen::default())
        // V-21-A: placeholder populated by setup_tray; background poller uses
        // it to re-sync the checkmark after the daemon socket becomes ready.
        .manage(PrivateModeMenuItem(Mutex::new(None)))
        // Recent-resync: placeholder populated by setup_tray; background poller
        // rebuilds the submenu once the daemon responds and then periodically.
        .manage(RecentSubmenu(Mutex::new(None)))
        // Stop flag for the recent-resync background thread; set on app exit.
        .manage(TrayResyncStop(std::sync::Arc::new(
            std::sync::atomic::AtomicBool::new(false),
        )))
        // Stop flag for the incoming-pairing poller thread; set on app exit.
        .manage(PairingPollStop(std::sync::Arc::new(
            std::sync::atomic::AtomicBool::new(false),
        )))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            // No extra CLI args needed; the app launches the daemon itself.
            None::<Vec<&str>>,
        ))
        .invoke_handler(tauri::generate_handler![
            ipc::ipc_call,
            ipc::pairing_qr_svg,
            ipc::reset_database,
            daemon_lifecycle::app_version,
            daemon_lifecycle::restart_daemon,
            daemon_lifecycle::get_daemon_error,
            config::get_popup_shortcut,
            config::get_default_popup_shortcut,
            config::set_popup_shortcut,
            popup::check_accessibility_permission,
            popup::request_accessibility_permission,
            popup::paste_to_frontmost,
            popup::paste_plain_text,
            popup::hide_popup,
            popup::play_copy_sound,
            notifications::show_copy_notification,
            notifications::check_notification_permission,
            config::get_allow_screenshots,
            config::set_allow_screenshots,
            config::get_launch_at_login,
            config::set_launch_at_login,
            ipc::read_logs,
            ipc::log_dir_path,
            ipc::open_item_file,
            popup::focus_main_window,
            popup::set_native_appearance,
        ])
        .setup(|app| {
            // Load persisted config now that we have the app handle.
            let persisted = load_ui_config(app.handle());
            {
                let state: State<CurrentShortcut> = app.state();
                let mut guard = state.0.lock().expect("mutex poisoned");
                *guard = persisted.popup_shortcut.clone();
            }
            {
                let state: State<CurrentPopupPosition> = app.state();
                let mut guard = state.0.lock().expect("mutex poisoned");
                *guard = persisted.popup_position.clone();
            }
            {
                // CopyPaste-6uy9: overwrite AllowScreenshots with the persisted
                // value so the window builder below sees the correct setting.
                let state: State<AllowScreenshots> = app.state();
                let mut guard = state.0.lock().expect("mutex poisoned");
                *guard = persisted.allow_screenshots;
            }

            // App-owned daemon lifecycle: start the daemon on a background
            // thread so the tray and window render immediately (MED-b fix).
            // The result is stored in DaemonSpawnError state and emitted as
            // the "daemon-spawn-result" event; the UI reads it via
            // get_daemon_error or the event listener.
            daemon_lifecycle::ensure_daemon_running_async(app.handle().clone());

            // Register macOS-only managed state.
            #[cfg(target_os = "macos")]
            {
                app.manage(popup::PriorApp(Mutex::new(None)));
                app.manage(popup::TapActive(Mutex::new(false)));
            }

            // Apply persisted launch-at-login preference idempotently.
            apply_launch_at_login(app.handle(), persisted.launch_at_login);

            tray::setup_tray(app)?;
            // Non-fatal: a saved accelerator the OS/plugin can't register
            // (reserved combo, or a format the plugin rejects) must NOT crash
            // app startup. The value is already loaded into CurrentShortcut
            // above, and on macOS the CGEventTap can still handle it.
            if let Err(e) = register_popup_shortcut(app.handle(), &persisted.popup_shortcut) {
                tracing::warn!(
                    "startup: failed to register popup shortcut '{}': {e} \
                     (value preserved; re-set it in Settings if it doesn't trigger)",
                    persisted.popup_shortcut
                );
            }
            // M1: popup is lazy-created on first hotkey press (toggle_popup),
            // not at app launch — saves ~84 MB idle RSS.
            popup::setup_main_window(app);

            // V-21-A: Startup race — the tray was built before the daemon socket
            // was necessarily ready, so `get_private_mode` may have defaulted to
            // false even though the daemon persisted private_mode=true.  Spawn a
            // background thread that polls until the daemon responds, then
            // re-syncs the CheckMenuItem to the daemon's true value.
            tray::spawn_tray_private_mode_resync(app.handle().clone());

            // M4: Push path — refresh the tray CheckMenuItem whenever private
            // mode is toggled anywhere (Settings window, or the tray itself
            // re-emitting after its own toggle). The Settings frontend emits
            // `private-mode-changed` with the daemon-confirmed bool as payload.
            {
                let listen_handle = app.handle().clone();
                app.listen("private-mode-changed", move |event| {
                    // Payload is a JSON-encoded bool (e.g. "true").
                    let Ok(new_value) = serde_json::from_str::<bool>(event.payload()) else {
                        tracing::warn!(
                            "private-mode-changed: unparseable payload {:?}",
                            event.payload()
                        );
                        return;
                    };
                    // Reuse the exact CheckMenuItem update pattern from the
                    // startup re-sync handler (spawn_tray_private_mode_resync).
                    if let Some(state) = listen_handle.try_state::<PrivateModeMenuItem>() {
                        if let Ok(guard) = state.0.lock() {
                            if let Some(ref item) = *guard {
                                let current = item.is_checked().unwrap_or(!new_value);
                                if current != new_value {
                                    let _ = item.set_checked(new_value);
                                }
                            }
                        }
                    }
                });
            }

            // Recent-resync: the tray Recent submenu was built at startup before
            // the daemon was ready, so it likely shows a placeholder.  Spawn a
            // background poller that rebuilds it once the daemon responds and
            // then refreshes it periodically so the items stay current.
            tray::spawn_tray_recent_resync(app.handle().clone());

            // Incoming-pairing poller: detect responder-side SAS requests even
            // when the user is not on the Devices tab.  Fires a system
            // notification + brings the window forward + emits an
            // `incoming-pairing` event that App.tsx routes to the modal.
            pairing::spawn_incoming_pairing_poller(app.handle().clone());

            #[cfg(target_os = "macos")]
            {
                // CopyPaste-6uy9: pass the persisted allow-screenshots flag so
                // setup_macos can skip set_content_protected when the user has
                // explicitly opted in.
                popup::setup_macos(app, persisted.allow_screenshots);
                popup::try_install_event_tap(app.handle(), &persisted.popup_shortcut);
            }

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building CopyPaste UI")
        .run(|handle, event| {
            // App-owned daemon lifecycle: when the WHOLE app exits, stop the
            // daemon we started. `RunEvent::Exit` fires only on a real quit
            // (tray "Quit" → `app.exit(0)`, or process termination). Closing
            // just the main WINDOW hides it to the tray (see
            // `setup_main_window`) and never reaches Exit, so the daemon
            // correctly survives a window close (standard macOS pattern).
            if let tauri::RunEvent::Exit = event {
                // Signal the recent-resync background thread to exit before we
                // tear down the daemon, so it doesn't make IPC calls against a
                // dead socket during teardown.
                if let Some(stop) = handle.try_state::<TrayResyncStop>() {
                    stop.0.store(true, std::sync::atomic::Ordering::Relaxed);
                }
                // Signal the incoming-pairing poller thread to exit.
                if let Some(stop) = handle.try_state::<PairingPollStop>() {
                    stop.0.store(true, std::sync::atomic::Ordering::Relaxed);
                }
                // Release the macOS CGEventTap (tap, run-loop source, CFMachPort,
                // and the boxed trigger callback) so nothing leaks on quit.
                #[cfg(target_os = "macos")]
                event_tap::uninstall();
                daemon_lifecycle::stop_daemon(handle);
            }
        });
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Register the global shortcut that toggles the quick-paste popup.
pub(crate) fn register_popup_shortcut(
    handle: &tauri::AppHandle,
    accelerator: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

    let handle_clone = handle.clone();
    handle
        .global_shortcut()
        .on_shortcut(accelerator, move |_app, _shortcut, event| {
            if event.state() == ShortcutState::Pressed {
                popup::toggle_popup(&handle_clone);
            }
        })
        .map_err(|e| -> Box<dyn std::error::Error> {
            format!("global_shortcut register error: {e}").into()
        })?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use popup::poll_until_frontmost;
    use std::sync::atomic::{AtomicU32, Ordering};

    use super::*;

    // -------------------------------------------------------------------------
    // CopyPaste-78hg: poll_until_frontmost — bounded polling helper
    // -------------------------------------------------------------------------

    #[test]
    fn poll_until_frontmost_returns_immediately_when_first_check_matches() {
        // probe always returns the target bundle ID on the first call.
        let count = AtomicU32::new(0);
        let result = poll_until_frontmost(
            "com.example.target",
            || {
                count.fetch_add(1, Ordering::Relaxed);
                Some("com.example.target".to_owned())
            },
            10,
            std::time::Duration::from_millis(1),
        );
        // Should have returned true on the very first successful probe.
        assert!(result, "expected true when bundle matches immediately");
        assert_eq!(
            count.load(Ordering::Relaxed),
            1,
            "probe should be called exactly once"
        );
    }

    #[test]
    fn poll_until_frontmost_returns_true_after_a_few_retries() {
        // Fail the first 2 probes, then succeed on the 3rd.
        let count = AtomicU32::new(0);
        let result = poll_until_frontmost(
            "com.example.target",
            || {
                let n = count.fetch_add(1, Ordering::Relaxed);
                if n < 2 {
                    Some("com.other.app".to_owned()) // wrong foreground app
                } else {
                    Some("com.example.target".to_owned())
                }
            },
            20,
            std::time::Duration::from_millis(1),
        );
        assert!(result, "expected true when bundle eventually matches");
        assert_eq!(
            count.load(Ordering::Relaxed),
            3,
            "probe called 3 times (2 misses + 1 hit)"
        );
    }

    #[test]
    fn poll_until_frontmost_returns_false_after_timeout() {
        // probe always returns a different app — never matches.
        let count = AtomicU32::new(0);
        let result = poll_until_frontmost(
            "com.example.target",
            || {
                count.fetch_add(1, Ordering::Relaxed);
                Some("com.other.app".to_owned())
            },
            5, // only 5 iterations maximum
            std::time::Duration::from_millis(1),
        );
        assert!(!result, "expected false after exhausting iterations");
        assert_eq!(
            count.load(Ordering::Relaxed),
            5,
            "probe called exactly max_iter times"
        );
    }

    #[test]
    fn poll_until_frontmost_returns_false_when_probe_returns_none() {
        // probe returns None (cannot determine frontmost app) — no match possible.
        let result = poll_until_frontmost(
            "com.example.target",
            || None,
            5,
            std::time::Duration::from_millis(1),
        );
        assert!(
            !result,
            "expected false when frontmost app is indeterminate"
        );
    }

    // -------------------------------------------------------------------------
    // CopyPaste-sqw0: DEFAULT_POPUP_SHORTCUT / get_default_popup_shortcut
    // -------------------------------------------------------------------------

    #[test]
    fn default_popup_shortcut_constant_is_non_empty() {
        // Guard against accidentally blanking the constant.
        assert!(!config::DEFAULT_POPUP_SHORTCUT.is_empty());
    }

    #[test]
    fn default_popup_shortcut_fn_matches_constant() {
        // The serde default function must return the same value as the constant
        // so they can never silently drift apart.
        assert_eq!(
            config::default_popup_shortcut(),
            config::DEFAULT_POPUP_SHORTCUT,
            "default_popup_shortcut() diverged from DEFAULT_POPUP_SHORTCUT constant"
        );
    }

    #[test]
    fn default_popup_shortcut_value_matches_ts_expectation() {
        // This test is the cross-language source-of-truth check:
        // SettingsView.tsx (sqw0) uses "CmdOrCtrl+Shift+V" as its hardcoded
        // fallback.  If you change the Rust constant, update the TS comment too.
        // See: crates/copypaste-ui/src/views/SettingsView.tsx, `DEFAULT_POPUP_SHORTCUT`.
        assert_eq!(
            config::DEFAULT_POPUP_SHORTCUT,
            "CmdOrCtrl+Shift+V",
            "DEFAULT_POPUP_SHORTCUT value changed — update SettingsView.tsx to match"
        );
    }

    #[test]
    fn uiconfig_default_uses_shortcut_constant() {
        let cfg = config::UiConfig::default();
        assert_eq!(
            cfg.popup_shortcut,
            config::DEFAULT_POPUP_SHORTCUT,
            "UiConfig::default() popup_shortcut must equal DEFAULT_POPUP_SHORTCUT"
        );
    }
}
