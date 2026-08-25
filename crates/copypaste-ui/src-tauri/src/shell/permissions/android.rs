//! Kotlin facts for onboarding prompts. Policy stays in Rust [`super::policy`].

use serde::{Deserialize, Serialize};
use tauri::plugin::{Builder, PluginHandle, TauriPlugin};
use tauri::{AppHandle, Manager as _, Runtime, Wry};

use super::policy::{
    self, android_notification_authorization, android_tile_status, AndroidNotificationFacts,
    AndroidTileFacts,
};
use super::PermissionStatus;
use crate::backend::BackendError;

const PLUGIN_PACKAGE: &str = "com.copypaste.app";
const PLUGIN_CLASS: &str = "OnboardingPermissionsPlugin";
const MSG_BRIDGE: &str = "CopyPaste couldn't reach the Android permission prompt.";

pub struct AndroidPermissions(PluginHandle<Wry>);

pub fn plugin() -> TauriPlugin<Wry> {
    Builder::new("onboarding-permissions")
        .setup(|app, api| {
            let handle = api.register_android_plugin(PLUGIN_PACKAGE, PLUGIN_CLASS)?;
            app.manage(AndroidPermissions(handle));
            Ok(())
        })
        .build()
}

#[derive(Serialize, Deserialize)]
struct Empty {}

fn call<A: Serialize, T: serde::de::DeserializeOwned>(
    app: &AppHandle<impl Runtime>,
    command: &'static str,
    args: A,
) -> Result<T, BackendError> {
    let plugin = app
        .try_state::<AndroidPermissions>()
        .ok_or_else(|| BackendError::Internal(MSG_BRIDGE.into()))?;
    plugin.0.run_mobile_plugin(command, args).map_err(|error| {
        tracing::warn!(command, %error, "the onboarding permission plugin failed");
        BackendError::Internal(MSG_BRIDGE.into())
    })
}

pub fn notification_status(
    app: &AppHandle<impl Runtime>,
) -> Result<PermissionStatus, BackendError> {
    let facts: AndroidNotificationFacts = call(app, "notificationFacts", Empty {})?;
    Ok(policy::notification_status(
        android_notification_authorization(
            facts.api_level,
            facts.granted,
            facts.ever_asked,
            facts.show_rationale,
        ),
    ))
}

pub fn tile_status(app: &AppHandle<impl Runtime>) -> Result<PermissionStatus, BackendError> {
    let facts: AndroidTileFacts = call(app, "tileFacts", Empty {})?;
    Ok(android_tile_status(
        facts.api_level,
        facts.last_add_result,
        facts.result_constants,
    ))
}

pub fn request_notifications(app: &AppHandle<impl Runtime>) -> Result<(), BackendError> {
    let _: AndroidNotificationFacts = call(app, "requestNotifications", Empty {})?;
    Ok(())
}

pub fn request_tile(app: &AppHandle<impl Runtime>) -> Result<(), BackendError> {
    let _: AndroidTileFacts = call(app, "requestTile", Empty {})?;
    Ok(())
}

pub fn open_notification_settings(app: &AppHandle<impl Runtime>) -> Result<(), BackendError> {
    let _: Empty = call(app, "openNotificationSettings", Empty {})?;
    Ok(())
}
