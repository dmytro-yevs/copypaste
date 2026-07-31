//! INV-35 — the history window is not capturable by a screen recorder.
//!
//! Protection is **on before the first frame** and stays on unless the user
//! turns it off: `contentProtected: true` in `tauri.conf.json` on the desktop,
//! `FLAG_SECURE` in `MainActivity.onCreate` on Android. This module is only the
//! *loosening* path: nothing here has to run for the window to be protected, so
//! the reveal gesture needs no ordering guarantee against it, and every failure
//! is logged and stepped over into the protected state.
//!
//! Two mechanisms because there is only one: `set_content_protected` reaches
//! `tao`'s `Window::set_content_protection`, which is macOS and Windows only —
//! on Android it compiles to nothing at all.

use tauri::{AppHandle, Runtime};

/// Apply the user's screenshot preference to the window.
///
/// `allow_screenshots` is the *setting*, so the sense is inverted here exactly
/// once: everywhere else in the app the field means what it says.
pub fn apply<R: Runtime>(app: &AppHandle<R>, allow_screenshots: bool) {
    set(app, !allow_screenshots);
}

#[cfg(not(target_os = "android"))]
fn set<R: Runtime>(app: &AppHandle<R>, protect: bool) {
    let Some(window) = super::window::main_window(app) else {
        return;
    };
    if let Err(error) = window.set_content_protected(protect) {
        tracing::warn!(%error, "screen-capture protection could not be changed");
    }
}

#[cfg(target_os = "android")]
fn set<R: Runtime>(app: &AppHandle<R>, protect: bool) {
    use tauri::Manager as _;

    let Some(state) = app.try_state::<android::ScreenProtection>() else {
        tracing::warn!("the screen-protection plugin was not registered");
        return;
    };
    state.set(protect);
}

/// The Kotlin half. Registered from `crate::run`.
///
/// Separate from `capture::android`'s plugin rather than a command on it: that
/// one is the clipboard ladder, and a window flag is not part of it.
///
/// **Never compiled on this host**, and `--features embedded-backend` does not
/// reach it — the gate is `target_os`, not the feature. Kept to one call for
/// that reason, exactly as `capture::android` is.
#[cfg(target_os = "android")]
pub mod android {
    use serde::Serialize;
    use tauri::plugin::{Builder, PluginHandle, TauriPlugin};
    use tauri::{Manager as _, Wry};

    /// Must match the package and class of
    /// `gen/android/app/src/main/java/com/copypaste/app/ScreenProtectionPlugin.kt`.
    const PLUGIN_PACKAGE: &str = "com.copypaste.app";
    const PLUGIN_CLASS: &str = "ScreenProtectionPlugin";

    #[derive(Serialize)]
    struct Args {
        protected: bool,
    }

    pub struct ScreenProtection(PluginHandle<Wry>);

    impl ScreenProtection {
        pub(super) fn set(&self, protect: bool) {
            // The reply carries nothing; `serde_json::Value` accepts the empty
            // object Kotlin resolves with, where `()` would fail to deserialise.
            let call = self.0.run_mobile_plugin::<serde_json::Value>(
                "setProtected",
                Args { protected: protect },
            );
            if let Err(error) = call {
                tracing::warn!(%error, "screen-capture protection could not be changed");
            }
        }
    }

    pub fn plugin() -> TauriPlugin<Wry> {
        Builder::new("screen-protection")
            .setup(|app, api| {
                let handle = api.register_android_plugin(PLUGIN_PACKAGE, PLUGIN_CLASS)?;
                app.manage(ScreenProtection(handle));
                Ok(())
            })
            .build()
    }
}
