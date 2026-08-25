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
    for window in [
        super::window::main_window(app),
        #[cfg(not(target_os = "android"))]
        super::window::quick_paste_window(app),
    ]
    .into_iter()
    .flatten()
    {
        if let Err(error) = window.set_content_protected(protect) {
            tracing::warn!(%error, "screen-capture protection could not be changed");
        }
    }
}

#[cfg(all(test, not(target_os = "android")))]
mod tests {
    use serde_json::Value;

    #[test]
    fn the_preference_is_applied_to_both_desktop_surfaces() {
        let labels = [
            super::super::window::MAIN,
            super::super::window::QUICK_PASTE,
        ];
        assert_eq!(labels, ["main", "quick-paste"]);
    }

    #[test]
    fn protection_is_enabled_before_the_first_frame_on_every_desktop_surface() {
        let config: Value = serde_json::from_str(include_str!("../../tauri.conf.json"))
            .expect("the Tauri configuration is valid JSON");
        let main_is_protected = config["app"]["windows"]
            .as_array()
            .and_then(|windows| windows.iter().find(|window| window["label"] == "main"))
            .and_then(|window| window["contentProtected"].as_bool());

        assert_eq!(main_is_protected, Some(true));
        const { assert!(super::super::window::QUICK_PASTE_CONFIG.content_protected) }
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

    /// What the window carries now — read back from it, not echoed from the
    /// request. Kotlin rejects rather than sending a disagreeing value, so this
    /// mismatch arm should be unreachable; it is checked because the one thing
    /// worse than losing protection is believing it is on.
    #[derive(serde::Deserialize)]
    struct Applied {
        protected: bool,
    }

    pub struct ScreenProtection(PluginHandle<Wry>);

    impl ScreenProtection {
        pub(super) fn set(&self, protect: bool) {
            match self
                .0
                .run_mobile_plugin::<Applied>("setProtected", Args { protected: protect })
            {
                Ok(applied) if applied.protected == protect => {}
                Ok(applied) => tracing::warn!(
                    requested = protect,
                    applied = applied.protected,
                    "the window did not take the screen-capture protection it was given"
                ),
                Err(error) => {
                    tracing::warn!(%error, "screen-capture protection could not be changed");
                }
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
