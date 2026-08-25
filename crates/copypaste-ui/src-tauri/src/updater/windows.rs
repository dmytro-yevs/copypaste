#[cfg(target_os = "windows")]
use super::{UiError, UpdateProgress, UpdateStatus};
#[cfg(target_os = "windows")]
use tauri::{ipc::Channel, AppHandle};

#[cfg(target_os = "windows")]
use tauri_plugin_updater::UpdaterExt as _;

#[cfg(target_os = "windows")]
pub(super) fn status(app: &AppHandle) -> UpdateStatus {
    if super::config::configured_for_app(app) {
        UpdateStatus::Ready
    } else {
        UpdateStatus::Unconfigured
    }
}

#[cfg(target_os = "windows")]
pub(super) async fn check(app: &AppHandle) -> Result<UpdateStatus, UiError> {
    let Some(updater) =
        super::config::updater(app, Some(std::time::Duration::from_secs(300)), None)?
    else {
        return Ok(UpdateStatus::Unconfigured);
    };
    Ok(
        match updater
            .check()
            .await
            .map_err(|error| super::config::plugin_error(error, "update_check_failed"))?
        {
            Some(update) => UpdateStatus::Available {
                version: update.version.to_string(),
            },
            None => UpdateStatus::UpToDate,
        },
    )
}

#[cfg(target_os = "windows")]
pub(super) async fn install(
    app: &AppHandle,
    expected: String,
    progress: Channel<UpdateProgress>,
) -> Result<UpdateStatus, UiError> {
    let Some(updater) = super::config::updater(
        app,
        Some(std::time::Duration::from_secs(300)),
        Some(progress.clone()),
    )?
    else {
        return Ok(UpdateStatus::Unconfigured);
    };
    let Some(update) = updater
        .check()
        .await
        .map_err(|error| super::config::plugin_error(error, "update_check_failed"))?
    else {
        return Ok(UpdateStatus::UpToDate);
    };
    let version = update.version.to_string();
    if version != expected {
        return Ok(UpdateStatus::Available { version });
    }
    let mut downloaded = 0_u64;
    let verifying = progress.clone();
    update
        .download_and_install(
            |chunk, total| {
                downloaded = downloaded.saturating_add(chunk as u64);
                let _ = progress.send(UpdateProgress::Downloading { downloaded, total });
            },
            move || {
                let _ = verifying.send(UpdateProgress::Verifying);
            },
        )
        .await
        .map_err(|error| super::config::plugin_error(error, "update_install_failed"))?;
    Err(UiError::new("update_install_failed", false))
}
