use serde::Deserialize;
use tauri::plugin::{Builder, PluginHandle, TauriPlugin};
use tauri::{Manager as _, Wry};

use crate::backend::{BackendError, Result};

const PLUGIN_PACKAGE: &str = "com.copypaste.app";
const PLUGIN_CLASS: &str = "PairingScannerPlugin";
const MSG_BRIDGE: &str = "CopyPaste couldn't open Android's QR scanner.";

#[derive(Deserialize)]
struct ScanResult {
    value: Option<String>,
}

pub struct AndroidPairingScanner(PluginHandle<Wry>);

impl AndroidPairingScanner {
    pub async fn scan(&self) -> Result<Option<String>> {
        self.0
            .run_mobile_plugin_async::<ScanResult>("scan", ())
            .await
            .map(|result| result.value)
            .map_err(|error| {
                tracing::warn!(%error, "the Android QR scanner plugin failed");
                BackendError::Internal(MSG_BRIDGE.to_string())
            })
    }
}

pub fn plugin() -> TauriPlugin<Wry> {
    Builder::new("android-pairing-scanner")
        .setup(|app, api| {
            let handle = api.register_android_plugin(PLUGIN_PACKAGE, PLUGIN_CLASS)?;
            app.manage(AndroidPairingScanner(handle));
            Ok(())
        })
        .build()
}
