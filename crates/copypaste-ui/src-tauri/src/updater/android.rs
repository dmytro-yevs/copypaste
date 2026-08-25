use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::Write;
use tauri::plugin::{Builder, PluginHandle, TauriPlugin};
use tauri::{AppHandle, Manager, Wry};

use crate::backend::UiBoundaryErrorCode;

const PACKAGE: &str = "com.copypaste.app";
const CLASS: &str = "AppUpdatePlugin";
const APK_NAME: &str = "copypaste-update.apk";

pub struct AndroidUpdater(PluginHandle<Wry>);

#[derive(Serialize)]
struct InstallArgs<'a> {
    path: &'a str,
}

#[derive(Serialize)]
struct Empty {}

#[derive(Deserialize)]
struct InstallResult {
    status: String,
}

pub fn prepare_install(app: &AppHandle) -> Result<(), UiBoundaryErrorCode> {
    let result: InstallResult = app
        .state::<AndroidUpdater>()
        .0
        .run_mobile_plugin("prepareInstall", Empty {})
        .map_err(|_| UiBoundaryErrorCode::UpdateInstallFailed)?;
    match result.status.as_str() {
        "success" => Ok(()),
        "permission_required" => Err(UiBoundaryErrorCode::UpdatePermissionRequired),
        _ => Err(UiBoundaryErrorCode::UpdateInstallFailed),
    }
}

pub fn plugin() -> TauriPlugin<Wry> {
    Builder::new("android-updater")
        .setup(|app, api| {
            let handle = api.register_android_plugin(PACKAGE, CLASS)?;
            app.manage(AndroidUpdater(handle));
            Ok(())
        })
        .build()
}

pub fn stage_and_install(app: &AppHandle, bytes: &[u8]) -> Result<(), UiBoundaryErrorCode> {
    let cache = app
        .path()
        .app_cache_dir()
        .map_err(|_| UiBoundaryErrorCode::UpdateInstallFailed)?;
    std::fs::create_dir_all(&cache).map_err(|_| UiBoundaryErrorCode::UpdateInstallFailed)?;
    let path = cache.join(APK_NAME);
    let result = (|| {
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&path)
            .map_err(|_| UiBoundaryErrorCode::UpdateInstallFailed)?;
        file.write_all(bytes)
            .map_err(|_| UiBoundaryErrorCode::UpdateInstallFailed)?;
        file.sync_all()
            .map_err(|_| UiBoundaryErrorCode::UpdateInstallFailed)?;
        let path = path
            .to_str()
            .ok_or(UiBoundaryErrorCode::UpdateInstallFailed)?;
        let result: InstallResult = app
            .state::<AndroidUpdater>()
            .0
            .run_mobile_plugin("stageAndInstall", InstallArgs { path })
            .map_err(|_| UiBoundaryErrorCode::UpdateInstallFailed)?;
        match result.status.as_str() {
            "success" => Ok(()),
            "permission_required" => Err(UiBoundaryErrorCode::UpdatePermissionRequired),
            _ => Err(UiBoundaryErrorCode::UpdateInstallFailed),
        }
    })();
    let _ = std::fs::remove_file(path);
    result
}
