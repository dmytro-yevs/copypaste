use serde::Serialize;
use tauri::{ipc::Channel, AppHandle};

use crate::backend::{UiBoundaryErrorCode, UiError};

#[cfg(target_os = "android")]
pub mod android;
mod config;
#[cfg(target_os = "macos")]
mod macos;
mod windows;

#[derive(Default)]
pub struct UpdateRuntime {
    pub(crate) operation: tokio::sync::Mutex<()>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum UpdateStatus {
    Unsupported,
    Unconfigured,
    Ready,
    UpToDate,
    Available { version: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum UpdateProgress {
    Downloading { downloaded: u64, total: Option<u64> },
    Verifying,
    Installing,
}

#[tauri::command]
pub fn update_status(app: AppHandle) -> UpdateStatus {
    let _ = &app;
    #[cfg(target_os = "windows")]
    {
        windows::status(&app)
    }
    #[cfg(target_os = "macos")]
    {
        if macos::brew_path().is_some() {
            UpdateStatus::Ready
        } else {
            UpdateStatus::Unconfigured
        }
    }
    #[cfg(target_os = "android")]
    {
        if config::configured_for_app(&app) {
            UpdateStatus::Ready
        } else {
            UpdateStatus::Unconfigured
        }
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "android")))]
    {
        let _ = app;
        UpdateStatus::Unsupported
    }
}

#[tauri::command]
pub async fn check_for_update(
    app: AppHandle,
    runtime: tauri::State<'_, UpdateRuntime>,
) -> Result<UpdateStatus, UiError> {
    let _guard = runtime
        .operation
        .try_lock()
        .map_err(|_| UiError::from_boundary(UiBoundaryErrorCode::UpdateBusy))?;

    #[cfg(target_os = "windows")]
    {
        windows::check(&app).await
    }
    #[cfg(target_os = "macos")]
    {
        let _ = app;
        macos::check().await
    }
    #[cfg(target_os = "android")]
    {
        let Some(updater) = config::updater(&app, None, None)? else {
            return Ok(UpdateStatus::Unconfigured);
        };
        Ok(
            match updater.check().await.map_err(|error| {
                config::plugin_error(error, UiBoundaryErrorCode::UpdateCheckFailed)
            })? {
                Some(update) => UpdateStatus::Available {
                    version: update.version,
                },
                None => UpdateStatus::UpToDate,
            },
        )
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "android")))]
    {
        let _ = app;
        Ok(UpdateStatus::Unsupported)
    }
}

#[tauri::command]
pub async fn install_update(
    app: AppHandle,
    runtime: tauri::State<'_, UpdateRuntime>,
    expected_version: String,
    progress: Channel<UpdateProgress>,
) -> Result<UpdateStatus, UiError> {
    let _guard = runtime
        .operation
        .try_lock()
        .map_err(|_| UiError::from_boundary(UiBoundaryErrorCode::UpdateBusy))?;

    #[cfg(target_os = "windows")]
    {
        windows::install(&app, expected_version, progress).await
    }
    #[cfg(target_os = "macos")]
    {
        macos::install(app, expected_version, progress).await
    }
    #[cfg(target_os = "android")]
    {
        let Some(updater) = config::updater(&app, None, None)? else {
            return Ok(UpdateStatus::Unconfigured);
        };
        let Some(update) = updater
            .check()
            .await
            .map_err(|error| config::plugin_error(error, UiBoundaryErrorCode::UpdateCheckFailed))?
        else {
            return Ok(UpdateStatus::UpToDate);
        };
        if update.version != expected_version {
            return Ok(UpdateStatus::Available {
                version: update.version,
            });
        }
        android::prepare_install(&app).map_err(UiError::from_boundary)?;

        let mut downloaded = 0_u64;
        let progress_copy = progress.clone();
        let bytes = update
            .download(
                |chunk, total| {
                    downloaded = downloaded.saturating_add(chunk as u64);
                    let _ = progress_copy.send(UpdateProgress::Downloading { downloaded, total });
                },
                || {
                    let _ = progress.send(UpdateProgress::Verifying);
                },
            )
            .await
            .map_err(|error| {
                config::plugin_error(error, UiBoundaryErrorCode::UpdateInstallFailed)
            })?;

        let _ = progress.send(UpdateProgress::Installing);
        let app_for_stage = app.clone();
        tokio::task::spawn_blocking(move || android::stage_and_install(&app_for_stage, &bytes))
            .await
            .map_err(|_| UiError::from_boundary(UiBoundaryErrorCode::UpdateInstallFailed))?
            .map_err(UiError::from_boundary)?;
        Ok(UpdateStatus::UpToDate)
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "android")))]
    {
        let _ = (app, expected_version, progress);
        Ok(UpdateStatus::Unsupported)
    }
}
