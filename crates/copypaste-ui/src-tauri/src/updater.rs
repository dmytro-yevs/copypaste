use serde::Serialize;
use tauri::{ipc::Channel, AppHandle};

use crate::backend::UiError;

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
    #[cfg(target_os = "windows")]
    {
        return windows::status(&app);
    }
    #[cfg(target_os = "macos")]
    {
        return if macos::brew_path().is_some() {
            UpdateStatus::Ready
        } else {
            UpdateStatus::Unconfigured
        };
    }
    #[cfg(target_os = "android")]
    {
        return if config::configured_for_app(&app) {
            UpdateStatus::Ready
        } else {
            UpdateStatus::Unconfigured
        };
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
        .map_err(|_| UiError::new("update_busy", true))?;

    #[cfg(target_os = "windows")]
    {
        return windows::check(&app).await;
    }
    #[cfg(target_os = "macos")]
    {
        let _ = app;
        return macos::check().await;
    }
    #[cfg(target_os = "android")]
    {
        let Some(updater) = config::updater(&app, None, None)? else {
            return Ok(UpdateStatus::Unconfigured);
        };
        return Ok(
            match updater
                .check()
                .await
                .map_err(|error| config::plugin_error(error, "update_check_failed"))?
            {
                Some(update) => UpdateStatus::Available {
                    version: update.version,
                },
                None => UpdateStatus::UpToDate,
            },
        );
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
        .map_err(|_| UiError::new("update_busy", true))?;

    #[cfg(target_os = "windows")]
    {
        return windows::install(&app, expected_version, progress).await;
    }
    #[cfg(target_os = "macos")]
    {
        return macos::install(app, expected_version, progress).await;
    }
    #[cfg(target_os = "android")]
    {
        let Some(updater) = config::updater(&app, None, None)? else {
            return Ok(UpdateStatus::Unconfigured);
        };
        let Some(update) = updater
            .check()
            .await
            .map_err(|error| config::plugin_error(error, "update_check_failed"))?
        else {
            return Ok(UpdateStatus::UpToDate);
        };
        if update.version != expected_version {
            return Ok(UpdateStatus::Available {
                version: update.version,
            });
        }
        android::prepare_install(&app)
            .map_err(|code| UiError::new(code, code == "update_permission_required"))?;

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
            .map_err(|error| config::plugin_error(error, "update_install_failed"))?;

        let _ = progress.send(UpdateProgress::Installing);
        let app_for_stage = app.clone();
        tokio::task::spawn_blocking(move || android::stage_and_install(&app_for_stage, &bytes))
            .await
            .map_err(|_| UiError::new("update_install_failed", false))?
            .map_err(|code| UiError::new(code, code == "update_permission_required"))?;
        return Ok(UpdateStatus::UpToDate);
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "android")))]
    {
        let _ = (app, expected_version, progress);
        Ok(UpdateStatus::Unsupported)
    }
}
