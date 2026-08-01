use serde::Deserialize;
use tauri::plugin::{Builder, PluginHandle, TauriPlugin};
use tauri::{Manager as _, Wry};

const PLUGIN_PACKAGE: &str = "com.copypaste.app";
const PLUGIN_CLASS: &str = "NetworkDiscoveryPlugin";

#[derive(Deserialize)]
struct Availability {
    available: bool,
}

/// Android drops multicast packets unless an app holds this system-managed
/// Wi-Fi lock. Discovery keeps working on non-Wi-Fi networks without it.
#[derive(Clone)]
pub struct AndroidNetworkDiscovery(PluginHandle<Wry>);

impl AndroidNetworkDiscovery {
    pub async fn acquire(&self) -> bool {
        self.0
            .run_mobile_plugin_async::<Availability>("acquire", ())
            .await
            .map(|result| result.available)
            .unwrap_or(false)
    }
}

pub fn plugin() -> TauriPlugin<Wry> {
    Builder::new("android-network-discovery")
        .setup(|app, api| {
            let handle = api.register_android_plugin(PLUGIN_PACKAGE, PLUGIN_CLASS)?;
            app.manage(AndroidNetworkDiscovery(handle));
            Ok(())
        })
        .build()
}
