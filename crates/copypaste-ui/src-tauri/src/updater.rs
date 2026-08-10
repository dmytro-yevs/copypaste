use serde::Serialize;
use tauri::{ipc::Channel, AppHandle};

use crate::backend::UiError;

#[cfg(target_os = "windows")]
use std::time::Duration;
#[cfg(target_os = "windows")]
use tauri_plugin_updater::{Error as PluginError, Updater, UpdaterExt as _};

#[cfg(target_os = "windows")]
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5 * 60);

#[derive(Default)]
pub struct UpdateRuntime {
    operation: tokio::sync::Mutex<()>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum UpdateStatus {
    #[cfg_attr(target_os = "windows", allow(dead_code))]
    Unsupported,
    Unconfigured,
    Ready,
    UpToDate,
    Available {
        version: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum UpdateProgress {
    Downloading { downloaded: u64, total: Option<u64> },
    Verifying,
    Installing,
}

#[cfg(target_os = "windows")]
fn secure_endpoint(raw: &str) -> bool {
    url::Url::parse(raw).is_ok_and(|endpoint| {
        endpoint.scheme() == "https"
            && endpoint.host().is_some()
            && endpoint.username().is_empty()
            && endpoint.password().is_none()
    })
}

#[cfg(target_os = "windows")]
fn configured(value: Option<&serde_json::Value>) -> bool {
    let Some(config) = value.and_then(serde_json::Value::as_object) else {
        return false;
    };
    let Some(pubkey) = config.get("pubkey").and_then(serde_json::Value::as_str) else {
        return false;
    };
    let Some(endpoints) = config
        .get("endpoints")
        .and_then(serde_json::Value::as_array)
    else {
        return false;
    };

    !pubkey.trim().is_empty()
        && !pubkey.starts_with("__")
        && !endpoints.is_empty()
        && endpoints.iter().all(|endpoint| {
            endpoint
                .as_str()
                .is_some_and(|url| !url.contains("__TAURI_") && secure_endpoint(url))
        })
}

#[cfg(target_os = "windows")]
fn updater_configured(app: &AppHandle) -> bool {
    configured(app.config().plugins.0.get("updater"))
}

#[tauri::command]
pub fn update_status(app: AppHandle) -> UpdateStatus {
    #[cfg(target_os = "windows")]
    {
        if updater_configured(&app) {
            UpdateStatus::Ready
        } else {
            UpdateStatus::Unconfigured
        }
    }

    #[cfg(not(target_os = "windows"))]
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
    let _operation = runtime
        .operation
        .try_lock()
        .map_err(|_| UiError::new("update_busy", true))?;

    #[cfg(target_os = "windows")]
    {
        let Some(updater) = configured_updater(&app, None)? else {
            return Ok(UpdateStatus::Unconfigured);
        };
        return match updater
            .check()
            .await
            .map_err(|error| plugin_error(error, "update_check_failed"))?
        {
            Some(update) => Ok(UpdateStatus::Available {
                version: update.version.to_string(),
            }),
            None => Ok(UpdateStatus::UpToDate),
        };
    }

    #[cfg(not(target_os = "windows"))]
    Ok(UpdateStatus::Unsupported)
}

#[tauri::command]
pub async fn install_update(
    app: AppHandle,
    runtime: tauri::State<'_, UpdateRuntime>,
    expected_version: String,
    progress: Channel<UpdateProgress>,
) -> Result<UpdateStatus, UiError> {
    let _operation = runtime
        .operation
        .try_lock()
        .map_err(|_| UiError::new("update_busy", true))?;

    #[cfg(target_os = "windows")]
    {
        let Some(updater) = configured_updater(&app, Some(progress.clone()))? else {
            return Ok(UpdateStatus::Unconfigured);
        };
        let Some(update) = updater
            .check()
            .await
            .map_err(|error| plugin_error(error, "update_check_failed"))?
        else {
            return Ok(UpdateStatus::UpToDate);
        };
        let version = update.version.to_string();
        if version != expected_version {
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
            .map_err(|error| plugin_error(error, "update_install_failed"))?;

        // The pinned Windows plugin exits after dispatching the installer. If
        // that contract ever returns, do not claim an install or restart.
        Err(UiError::new("update_install_failed", false))
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = (expected_version, progress);
        Ok(UpdateStatus::Unsupported)
    }
}

#[cfg(target_os = "windows")]
fn configured_updater(
    app: &AppHandle,
    installing: Option<Channel<UpdateProgress>>,
) -> Result<Option<Updater>, UiError> {
    if !updater_configured(app) {
        return Ok(None);
    }
    let builder = app.updater_builder().timeout(REQUEST_TIMEOUT);
    let builder = if let Some(installing) = installing {
        builder.on_before_exit(move || {
            let _ = installing.send(UpdateProgress::Installing);
        })
    } else {
        builder
    };
    builder
        .build()
        .map(Some)
        .map_err(|error| plugin_error(error, "update_check_failed"))
}

#[cfg(target_os = "windows")]
fn plugin_error(error: PluginError, fallback: &'static str) -> UiError {
    let code = match error {
        PluginError::EmptyEndpoints => "update_unconfigured",
        PluginError::UnsupportedArch | PluginError::UnsupportedOs => "update_unsupported",
        PluginError::Minisign(_) | PluginError::Base64(_) | PluginError::SignatureUtf8(_) => {
            "update_signature_invalid"
        }
        PluginError::Reqwest(_)
        | PluginError::Network(_)
        | PluginError::ReleaseNotFound
        | PluginError::TargetNotFound(_)
        | PluginError::TargetsNotFound(_) => "update_network_failed",
        _ => fallback,
    };
    let retryable = matches!(
        code,
        "update_busy" | "update_check_failed" | "update_network_failed"
    );
    UiError::new(code, retryable)
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::*;

    #[test]
    fn unsigned_and_partial_updater_configs_fail_closed() {
        assert!(!configured(None));
        for value in [
            serde_json::json!({}),
            serde_json::json!({"pubkey": "key"}),
            serde_json::json!({"endpoints": ["https://updates.example.test/latest.json"]}),
            serde_json::json!({"pubkey": "key", "endpoints": []}),
            serde_json::json!({"pubkey": "key", "endpoints": ["http://updates.example.test/latest.json"]}),
            serde_json::json!({"pubkey": "__TAURI_UPDATER_PUBLIC_KEY__", "endpoints": ["__TAURI_UPDATER_ENDPOINT__"]}),
        ] {
            assert!(
                !configured(Some(&value)),
                "partial config was accepted: {value}"
            );
        }
    }

    #[test]
    fn a_complete_signed_updater_config_is_recognised() {
        let value = serde_json::json!({
            "pubkey": "RWQ0cHVibGljLWtleQ==",
            "endpoints": ["https://updates.example.test/latest.json"],
            "windows": {"installMode": "passive"}
        });
        assert!(configured(Some(&value)));
    }

    #[test]
    fn malformed_and_deceptive_endpoints_fail_closed() {
        for endpoint in [
            "https://",
            "not a URL",
            "https+file://updates.example.test/latest.json",
            "https://updates.example.test@evil.test/latest.json",
            "https://user:secret@updates.example.test/latest.json",
        ] {
            let value = serde_json::json!({"pubkey": "key", "endpoints": [endpoint]});
            assert!(
                !configured(Some(&value)),
                "endpoint was accepted: {endpoint}"
            );
        }
    }

    #[test]
    fn user_facing_errors_are_closed_codes_without_plugin_details() {
        let serialized =
            serde_json::to_string(&UiError::new("update_install_failed", false)).unwrap();
        assert_eq!(
            serialized,
            r#"{"code":"update_install_failed","retryable":false}"#
        );
        assert!(!serialized.contains('\\') && !serialized.contains('/'));
    }
}
