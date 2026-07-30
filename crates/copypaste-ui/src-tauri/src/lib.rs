//! CopyPaste — one Tauri v2 app for macOS and Android (ADR-0002).
//!
//! This file does one job: assemble the app. The parts it assembles are:
//!
//! * [`capture`] — the Android clipboard ladder (rung 0 and rung 2), and the
//!   one state that says whether it is actually working. On macOS it is a
//!   constant, so the same commands and the same status component serve both.
//! * [`backend`] — one command surface over two backends. macOS talks IPC to
//!   the daemon over its `0600` Unix socket; Android runs `copypaste-core` and
//!   `copypaste-p2p` in the app process, because Android hosts no daemon. The
//!   choice is a compile-time type alias, [`backend::SelectedBackend`].
//! * [`commands`] — the `#[tauri::command]` surface, written once against that
//!   alias and containing no `cfg` at all. Identical command names and shapes
//!   on both platforms is what stops the React side growing platform branches.
//! * [`model`] — the boundary that discards a sensitive item's plaintext before
//!   it can reach the WebView.
//! * [`service`] — who starts and stops the daemon. The app does, and it stops
//!   only what it started (ADR-0004).
//! * [`shell`] — menu-bar item, popover placement, global hotkey, launch at
//!   login. All Tauri plugins; no native code in this crate.
//!
//! # What has been verified, and what has not
//!
//! This host is Linux with no macOS SDK and no Android SDK. So:
//!
//! * The **desktop** path compiles and its tests run here. It has not been run
//!   on macOS, so the tray, the popover behaviour, the hotkey and
//!   launch-at-login are unexercised — they are correct by API reading, not by
//!   observation.
//! * The **Android** path compiles only under `--features embedded-backend`,
//!   on Linux, which type-checks it but exercises nothing Android-specific. It
//!   has never been built for an Android target and `cargo tauri android` has
//!   never been run.
//!
//! ADR-0003 lists what is outstanding, including the two refactors in crates
//! this change does not own that the Android backend needs before it is whole.

#![forbid(unsafe_code)]

pub mod backend;
pub mod capture;
pub mod commands;
pub mod model;
pub mod service;
pub mod shell;

use backend::SelectedBackend;
use service::Supervisor;
use tauri::Manager as _;

/// Build and run the app.
pub fn run() {
    let builder = tauri::Builder::default().plugin(tauri_plugin_clipboard_manager::init());

    // Desktop plugins. The `cfg` is here, at the assembly point, rather than
    // inside `shell` — one gate, at the one place that knows it is building a
    // desktop app.
    #[cfg(not(target_os = "android"))]
    let builder = builder
        .plugin(shell::hotkey::plugin())
        .plugin(shell::autostart::plugin());

    // The Kotlin half of the capture ladder. Its setup registers the Android
    // plugin and publishes the `SelectedCapture` the commands are written
    // against; on the desktop the equivalent is managed below.
    #[cfg(target_os = "android")]
    let builder = builder.plugin(capture::android::init());

    builder
        .setup(|app| {
            app.manage(make_backend(app)?);
            app.manage(Supervisor::default());

            #[cfg(not(target_os = "android"))]
            app.manage(capture::desktop::DesktopCapture::default());

            // A menu-bar app, not a windowed one: no Dock icon, no app menu,
            // and the popover does not steal the active application. Without
            // this the tray icon and a Dock icon both appear, which is two
            // entry points to one window.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            #[cfg(not(target_os = "android"))]
            {
                let handle = app.handle();
                shell::tray::build(handle)?;
                // A hotkey the OS refuses — because another app already has it
                // — is not a reason to fail to start. The app is still fully
                // usable from the menu-bar item, so this is reported and
                // stepped over.
                if let Err(message) =
                    shell::hotkey::register(handle, shell::hotkey::default_shortcut())
                {
                    tracing::warn!(%message, "the global shortcut was not registered");
                }
            }

            // ADR-0004: opening the app starts the background service.
            //
            // Spawned rather than awaited. `setup` blocks the first paint, and
            // a cold start opens SQLCipher and derives a key — the user would
            // watch a blank window do it. The frontend already has a state for
            // "not running yet" and picks the service up when it answers, so
            // the slow path is visible instead of invisible.
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let backend = handle.state::<SelectedBackend>();
                let supervisor = handle.state::<Supervisor>();
                if let Err(error) = supervisor.start(backend.inner()).await {
                    // Not fatal: the offline screen offers the same start
                    // again, with a sentence the user can act on.
                    tracing::warn!(%error, "the background service was not started");
                }
            });

            // The change stream, for as long as the app runs. Falls back to
            // the frontend's poll whenever it is not delivering (finding 15).
            service::push::spawn(app.handle().clone());

            // Moves what the platform captured into the database. Inert on the
            // desktop, where the daemon is the thing doing the capturing.
            capture::intake::spawn(app.handle().clone());

            Ok(())
        })
        .on_window_event(|_window, _event| {
            #[cfg(not(target_os = "android"))]
            shell::window::on_event(_window, _event);
        })
        .invoke_handler(tauri::generate_handler![
            // history
            commands::history::list,
            commands::history::search,
            commands::history::add_item,
            commands::history::copy_item,
            commands::history::reveal_item,
            commands::history::delete_item,
            commands::history::delete_all,
            commands::history::set_pinned,
            commands::history::reorder_pinned,
            // state
            commands::status::status,
            // clipboard capture, and the Android ladder
            commands::capture::capture_state,
            commands::capture::capture_refresh,
            commands::capture::capture_arm,
            commands::capture::capture_disarm,
            commands::capture::capture_set_enabled,
            commands::capture::capture_now,
            commands::capture::capture_toast_explanation,
            commands::capture::capture_set_toast_suppressed,
            // the background service, and the window
            commands::service::service_state,
            commands::service::start_service,
            commands::service::restart_service,
            commands::service::hide_window,
            // peers
            commands::peers::pair_create,
            commands::peers::pair_accept,
            commands::peers::peers,
            commands::peers::unpair,
            commands::peers::sync_now,
            commands::peers::discovered,
            commands::peers::rescan,
        ])
        .build(tauri::generate_context!())
        .expect("could not start CopyPaste")
        .run(|app, event| {
            // ADR-0004: quitting the app stops the service it started. `Exit`
            // rather than `ExitRequested` because the latter can be cancelled,
            // and a cancelled quit that had already killed the daemon would
            // leave a running app with no history.
            if matches!(event, tauri::RunEvent::Exit) {
                if let Some(supervisor) = app.try_state::<Supervisor>() {
                    supervisor.stop();
                }
            }
        });
}

/// Construct the backend this target uses.
///
/// Two bodies, one signature. The desktop one cannot fail — it holds no
/// resources and connects per call — while the Android one opens the database
/// and the keystore, which is exactly the work the daemon does at startup on
/// the other platform.
#[cfg(not(target_os = "android"))]
fn make_backend<R: tauri::Runtime>(
    _app: &tauri::App<R>,
) -> Result<SelectedBackend, Box<dyn std::error::Error>> {
    Ok(backend::daemon::DaemonBackend::new())
}

#[cfg(target_os = "android")]
fn make_backend<R: tauri::Runtime>(
    app: &tauri::App<R>,
) -> Result<SelectedBackend, Box<dyn std::error::Error>> {
    use tauri::Manager;

    // The Android app's own private directory, from Tauri's path resolver
    // rather than from `copypaste_ipc::data_dir()`. The shared helper resolves
    // through `directories`, which has no notion of an Android context, and
    // guessing a path on Android is how an app ends up writing somewhere the
    // OS will delete.
    let data_dir = app.path().app_data_dir()?;

    let clipboard = AppClipboard {
        app: app.handle().clone(),
    };
    Ok(backend::embedded::EmbeddedBackend::open(
        &data_dir,
        Box::new(clipboard),
    )?)
}

/// Bridges the embedded backend's `Clipboard` trait onto the Tauri plugin.
///
/// Lives here rather than in `backend::embedded` so that module stays free of
/// Tauri types and can be tested with a fake.
#[cfg(target_os = "android")]
struct AppClipboard<R: tauri::Runtime> {
    app: tauri::AppHandle<R>,
}

#[cfg(target_os = "android")]
impl<R: tauri::Runtime> backend::embedded::Clipboard for AppClipboard<R> {
    fn set_text(&self, text: &str) -> Result<(), &'static str> {
        use tauri_plugin_clipboard_manager::ClipboardExt;
        self.app
            .clipboard()
            .write_text(text.to_string())
            .map_err(|_| "That item couldn't be copied to the clipboard.")
    }
}
