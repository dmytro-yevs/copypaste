use tauri::{AppHandle, Wry};

#[cfg(target_os = "android")]
use tauri::Manager as _;

#[cfg(not(target_os = "android"))]
use tauri::Theme;

/// Keep the native shell in the same resolved appearance as the WebView.
///
/// The frontend resolves `system` before it calls this function, so the native
/// title bar and Android system-bar icons never have to infer a user setting.
pub fn apply(app: &AppHandle<Wry>, theme: NativeTheme) {
    #[cfg(not(target_os = "android"))]
    app.set_theme(Some(theme.into()));

    #[cfg(target_os = "android")]
    {
        let Some(system_bars) = app.try_state::<android::SystemBars>() else {
            tracing::warn!("the Android system-bars plugin was not registered");
            return;
        };
        system_bars.apply(theme);
    }
}

#[derive(Clone, Copy, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NativeTheme {
    Dark,
    Light,
}

#[cfg(not(target_os = "android"))]
impl From<NativeTheme> for Theme {
    fn from(value: NativeTheme) -> Self {
        match value {
            NativeTheme::Dark => Self::Dark,
            NativeTheme::Light => Self::Light,
        }
    }
}

#[cfg(target_os = "android")]
pub mod android {
    use serde::Serialize;
    use tauri::plugin::{Builder, PluginHandle, TauriPlugin};
    use tauri::{Manager as _, Wry};

    use super::NativeTheme;

    const PLUGIN_PACKAGE: &str = "com.copypaste.app";
    const PLUGIN_CLASS: &str = "SystemBarsPlugin";

    #[derive(Serialize)]
    #[serde(rename_all = "lowercase")]
    enum ThemeArg {
        Dark,
        Light,
    }

    impl From<NativeTheme> for ThemeArg {
        fn from(value: NativeTheme) -> Self {
            match value {
                NativeTheme::Dark => Self::Dark,
                NativeTheme::Light => Self::Light,
            }
        }
    }

    pub struct SystemBars(PluginHandle<Wry>);

    impl SystemBars {
        pub(super) fn apply(&self, theme: NativeTheme) {
            if let Err(error) = self
                .0
                .run_mobile_plugin::<serde_json::Value>("setTheme", ThemeArg::from(theme))
            {
                tracing::warn!(%error, "the Android system bars could not be themed");
            }
        }
    }

    pub fn plugin() -> TauriPlugin<Wry> {
        Builder::new("android-system-bars")
            .setup(|app, api| {
                let handle = api.register_android_plugin(PLUGIN_PACKAGE, PLUGIN_CLASS)?;
                app.manage(SystemBars(handle));
                Ok(())
            })
            .build()
    }
}
