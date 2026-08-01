use tauri::{AppHandle, Wry};

#[derive(Clone, serde::Serialize)]
pub struct SystemAccent {
    pub color: String,
}

const BROWSER_FALLBACK_ACCENT: &str = "#6366f1";

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
        use tauri::Manager as _;

        let Some(system_bars) = app.try_state::<android::SystemBars>() else {
            tracing::warn!("the Android system-bars plugin was not registered");
            return;
        };
        system_bars.apply(theme);
    }
}

/// Read the colour selected by the operating system. The fallback intentionally
/// matches the browser preview's indigo token, so a platform lookup failure
/// never leaves the UI without a usable accent.
pub fn system_accent(app: &AppHandle<Wry>) -> SystemAccent {
    let _ = app;
    #[cfg(target_os = "macos")]
    let color = macos_system_accent().unwrap_or_else(|| BROWSER_FALLBACK_ACCENT.to_owned());

    #[cfg(target_os = "android")]
    let color = app
        .try_state::<android::SystemBars>()
        .and_then(|bars| bars.system_accent())
        .unwrap_or_else(|| BROWSER_FALLBACK_ACCENT.to_owned());

    #[cfg(not(any(target_os = "macos", target_os = "android")))]
    let color = {
        let _ = app;
        BROWSER_FALLBACK_ACCENT.to_owned()
    };

    SystemAccent { color }
}

#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
fn macos_system_accent() -> Option<String> {
    use objc2_app_kit::{NSColor, NSColorSpace};

    // The sRGB components are copied immediately. AppKit owns the dynamic
    // colour and remains the source of truth when CopyPaste is restarted.
    let color = unsafe { NSColor::controlAccentColor() };
    let rgb = unsafe { color.colorUsingColorSpace(&NSColorSpace::deviceRGBColorSpace()) }?;
    let red = (unsafe { rgb.redComponent() } * 255.0)
        .round()
        .clamp(0.0, 255.0) as u8;
    let green = (unsafe { rgb.greenComponent() } * 255.0)
        .round()
        .clamp(0.0, 255.0) as u8;
    let blue = (unsafe { rgb.blueComponent() } * 255.0)
        .round()
        .clamp(0.0, 255.0) as u8;
    Some(format!("#{red:02x}{green:02x}{blue:02x}"))
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

        pub(super) fn system_accent(&self) -> Option<String> {
            self.0
                .run_mobile_plugin::<SystemAccentArg>("systemAccent", ())
                .ok()
                .and_then(|accent| valid_hex(&accent.color).then_some(accent.color))
        }
    }

    #[derive(serde::Deserialize)]
    struct SystemAccentArg {
        color: String,
    }

    fn valid_hex(value: &str) -> bool {
        value.len() == 7
            && value.starts_with('#')
            && value.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)
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
