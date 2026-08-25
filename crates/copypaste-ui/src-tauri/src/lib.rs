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

// `deny` rather than `forbid`, for exactly one exception: [`android_context`]
// has to call `ndk_context::initialize_android_context`, which takes raw
// pointers because JNI does. Everything else in this crate stays safe, and the
// lint still fails the build for any second exception that does not say so at
// its own module.
#![deny(unsafe_code)]

#[cfg(any(target_os = "android", feature = "embedded-backend"))]
pub mod android_context;
pub mod backend;
pub mod capture;
pub mod command_contract;
pub mod commands;
#[cfg(all(feature = "dev-web-bridge", not(target_os = "android")))]
pub mod dev_web_bridge;
pub mod events;
#[cfg(not(target_os = "android"))]
mod installed_source_apps;
pub mod model;
#[cfg(target_os = "android")]
pub mod network_discovery;
mod pairing_presentation;
pub mod service;
pub mod shell;
pub mod source_app_icon;
#[cfg(feature = "typescript")]
pub mod typescript;
mod updater;
pub use updater::{UpdateProgress, UpdateStatus};

use backend::SelectedBackend;
use service::Supervisor;
use tauri::{AppHandle, Manager as _, Runtime};

/// Build and run the app.
///
/// On Android there is no `main`: the activity loads `libcopypaste_ui_lib.so`
/// and calls into it, so the entry point has to be generated. Without this the
/// library builds and the CLI then refuses it — "does not include required
/// runtime symbols" — which reads like a linker problem and is not one.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // The dialog plugin is registered for `commands::transfer`, which drives it
    // from Rust. It is deliberately not granted to the WebView in
    // `capabilities/`: a picked path is used on this side and never handed
    // across (AGENTS.md rule 4). The notification plugin is Rust-only for the
    // same reason — `shell::notify` posts, and a JS grant would let the page
    // post anything it liked in the app's name.
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_deep_link::init());

    // Desktop plugins. The `cfg` is here, at the assembly point, rather than
    // inside `shell` — one gate, at the one place that knows it is building a
    // desktop app.
    #[cfg(not(target_os = "android"))]
    let builder = builder
        .plugin(shell::hotkey::plugin())
        .plugin(shell::autostart::plugin());

    #[cfg(target_os = "windows")]
    let builder = builder.plugin(tauri_plugin_updater::Builder::new().build());

    #[cfg(target_os = "android")]
    let builder = builder
        .plugin(
            tauri_plugin_updater::Builder::new()
                .target("android-universal")
                .build(),
        )
        .plugin(updater::android::plugin());

    // The Kotlin half of the capture ladder. Its setup registers the Android
    // plugin and publishes the `SelectedCapture` the commands are written
    // against; on the desktop the equivalent is managed below.
    #[cfg(target_os = "android")]
    let builder = builder
        .plugin(capture::android::init())
        .plugin(network_discovery::plugin())
        .plugin(pairing_presentation::android::plugin())
        .plugin(shell::appearance::android::plugin())
        // `set_content_protected` is a no-op on Android (tao gates it to macOS
        // and Windows), so INV-35's Android half is `FLAG_SECURE` and needs a
        // Kotlin call to reach it.
        .plugin(shell::protection::android::plugin())
        .plugin(shell::permissions::android::plugin());

    builder
        .on_window_event(shell::window::on_event)
        .setup(|app| {
            let runtime_log_dir = runtime_log_dir(app.handle())?;
            app.manage(copypaste_runtime_log::init(
                &runtime_log_dir,
                copypaste_runtime_log::Process::App,
            )?);
            service::startup_diagnostics::app_started();
            app.manage(make_backend(app)?);
            app.manage(source_app_icon::SourceAppIconCache::default());
            app.manage(commands::transfer::PendingImportState::default());
            app.manage(updater::UpdateRuntime::default());
            #[cfg(target_os = "windows")]
            {
                let handle = app.handle().clone();
                let abort = std::sync::Arc::new(move || {
                    let handle = handle.clone();
                    tauri::async_runtime::spawn(async move {
                        use backend::PairingBackend as _;
                        let backend = handle.state::<SelectedBackend>();
                        let _ = backend.pair_cancel().await;
                    });
                });
                app.manage(pairing_presentation::PairingPresenter::new(
                    pairing_presentation::windows_ui(abort),
                ));
            }
            #[cfg(target_os = "macos")]
            {
                let handle = app.handle().clone();
                let abort = std::sync::Arc::new(move || {
                    let handle = handle.clone();
                    tauri::async_runtime::spawn(async move {
                        use backend::PairingBackend as _;
                        let backend = handle.state::<SelectedBackend>();
                        let _ = backend.pair_cancel().await;
                    });
                });
                app.manage(pairing_presentation::PairingPresenter::new(
                    pairing_presentation::macos_ui(abort),
                ));
            }
            #[cfg(not(any(target_os = "android", target_os = "windows", target_os = "macos")))]
            app.manage(pairing_presentation::PairingPresenter::default());
            app.manage(Supervisor::default());
            app.manage(shell::shortcut::ShortcutSettings::load(app.handle())?);

            #[cfg(target_os = "android")]
            {
                let discovery = app
                    .state::<network_discovery::AndroidNetworkDiscovery>()
                    .inner()
                    .clone();
                tauri::async_runtime::spawn(async move {
                    if !discovery.acquire().await {
                        tracing::warn!(
                            "Wi-Fi multicast lock is unavailable; LAN discovery may be limited"
                        );
                    }
                });
            }

            #[cfg(not(target_os = "android"))]
            app.manage(capture::desktop::DesktopCapture::default());

            // The main window has normal macOS app identity. Quick Paste uses
            // Accessory only while dismissing without a prior app (AT-39).
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Regular);

            #[cfg(not(target_os = "android"))]
            {
                let handle = app.handle();
                shell::tray::build(handle)?;
                app.state::<shell::shortcut::ShortcutSettings>()
                    .register_startup(handle);
            }

            #[cfg(target_os = "macos")]
            shell::evidence_ax::bootstrap(app.handle());

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
            app.manage(service::push::spawn(app.handle().clone()));

            // Moves what the platform captured into the database. Inert on the
            // desktop, where the daemon is the thing doing the capturing.
            capture::intake::spawn(app.handle().clone());

            Ok(())
        })
        .invoke_handler(command_contract::ui_invoke_handler!())
        .build(tauri::generate_context!())
        .expect("could not start CopyPaste")
        .run(|app, event| {
            #[cfg(target_os = "macos")]
            if matches!(
                event,
                tauri::RunEvent::Reopen {
                    has_visible_windows: false,
                    ..
                }
            ) {
                shell::window::show_main(app);
            }

            // ADR-0004: quitting the app stops the service it started. `Exit`
            // rather than `ExitRequested` because the latter can be cancelled,
            // and a cancelled quit that had already killed the daemon would
            // leave a running app with no history.
            if matches!(event, tauri::RunEvent::Exit) {
                service::startup_diagnostics::app_stopping();
                if let Some(push) = app.try_state::<service::push::PushMonitor>() {
                    push.stop();
                }
                if let Some(supervisor) = app.try_state::<Supervisor>() {
                    let backend = app.state::<SelectedBackend>();
                    tauri::async_runtime::block_on(supervisor.shutdown(backend.inner()));
                }
            }
        });
}

/// The app and desktop daemon intentionally share one private log directory;
/// Android resolves its app-private storage through Tauri instead.
pub(crate) fn runtime_log_dir<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    #[cfg(target_os = "android")]
    {
        return Ok(app.path().app_data_dir()?.join("logs"));
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        Ok(copypaste_ipc::data_dir().join("logs"))
    }
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

#[cfg(test)]
mod tests {
    fn production_source() -> &'static str {
        let marker = "#[cfg(test)]\r\nmod tests";
        include_str!("lib.rs")
            .split_once(marker)
            .or_else(|| include_str!("lib.rs").split_once("#[cfg(test)]\nmod tests"))
            .expect("the test module follows production assembly")
            .0
    }

    #[test]
    fn app_assembly_registers_the_window_lifecycle_policy() {
        let compact_source = production_source().split_whitespace().collect::<String>();
        let registration = [".on_window_", "event(shell::window::on_", "event)"].concat();

        assert!(
            compact_source.contains(&registration),
            "the app builder must register shell::window::on_event"
        );
    }

    #[test]
    fn app_assembly_starts_with_regular_macos_identity() {
        let production_source = production_source();

        assert!(production_source
            .contains("app.set_activation_policy(tauri::ActivationPolicy::Regular);"));
        assert!(!production_source.contains("ActivationPolicy::Accessory"));
    }

    #[test]
    fn macos_reopen_reveals_the_main_window_when_none_are_visible() {
        let compact_source = production_source().split_whitespace().collect::<String>();

        assert!(
            compact_source.contains("tauri::RunEvent::Reopen{")
                && compact_source.contains("has_visible_windows:false")
                && compact_source.contains("shell::window::show_main(app);"),
            "the macOS reopen event must show the main window when the app is still running"
        );
    }

    #[test]
    fn macos_main_window_preserves_defaults_at_binding_minimum() {
        let config: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.macos.conf.json"))
                .expect("the macOS Tauri config is valid JSON");
        let main = &config["app"]["windows"][0];

        assert_eq!(config["bundle"]["macOS"]["minimumSystemVersion"], "14.0");
        assert_eq!(main["label"], "main");
        assert_eq!(main["width"], 1100);
        assert_eq!(main["height"], 760);
        assert_eq!(main["minWidth"], 720);
        assert_eq!(main["minHeight"], 460);
        assert_eq!(main["skipTaskbar"], false);
        assert_eq!(main["titleBarStyle"], "Overlay");
        assert_eq!(main["hiddenTitle"], true);
        assert_eq!(main["transparent"], true);
    }

    /// The Start Menu shortcut created by NSIS provides the AppUserModelID
    /// required for toast delivery. `currentUser` keeps installation and the
    /// HKCU launch-at-login entry under the same user's control. The window
    /// block is repeated because platform config replaces the base array.
    #[test]
    fn windows_ships_an_installer_and_a_sized_main_window() {
        let config: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.windows.conf.json"))
                .expect("the Windows Tauri config is valid JSON");
        let release: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.windows.release.conf.json"))
                .expect("the Windows release config is valid JSON");
        let signed: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.windows.signed.conf.template.json"))
                .expect("the signed Windows release template is valid JSON");
        let hooks = include_str!("../windows/hooks.nsh");

        assert_eq!(config["bundle"]["targets"], serde_json::json!(["nsis"]));
        assert_eq!(config["bundle"]["windows"]["allowDowngrades"], false);
        assert_eq!(
            config["bundle"]["windows"]["nsis"]["installMode"],
            "currentUser"
        );
        assert_eq!(
            config["bundle"]["windows"]["nsis"]["installerHooks"],
            "windows/hooks.nsh"
        );
        assert_eq!(
            config["bundle"]["windows"]["nsis"]["installerIcon"],
            "icons/icon.ico"
        );

        let main = &config["app"]["windows"][0];
        assert_eq!(main["label"], "main");
        assert_eq!(main["width"], 1100);
        assert_eq!(main["height"], 760);
        assert_eq!(main["minWidth"], 720);
        assert_eq!(main["minHeight"], 460);
        assert_eq!(main["transparent"], false);
        // INV-35 holds on Windows through SetWindowDisplayAffinity, so the
        // screenshot exclusion must be asserted here as well as on macOS.
        assert_eq!(main["contentProtected"], true);
        assert_eq!(
            release["bundle"]["externalBin"],
            serde_json::json!(["binaries/copypaste", "binaries/copypaste-daemon"])
        );
        assert_eq!(signed["bundle"]["createUpdaterArtifacts"], true);
        assert_eq!(
            signed["bundle"]["externalBin"],
            serde_json::json!(["binaries/copypaste", "binaries/copypaste-daemon"])
        );
        assert_eq!(
            signed["plugins"]["updater"]["pubkey"],
            "__TAURI_UPDATER_PUBLIC_KEY__"
        );
        assert_eq!(
            signed["plugins"]["updater"]["endpoints"],
            serde_json::json!(["__TAURI_UPDATER_ENDPOINT__"])
        );
        assert_eq!(
            signed["plugins"]["updater"]["windows"]["installMode"],
            "passive"
        );
        assert!(hooks.matches("copypaste.exe\" shutdown").count() >= 2);
        assert!(hooks.contains("$UpdateMode <> 1"));
        assert!(hooks.contains("StartupApproved\\Run"));
    }
}
