#[cfg(any(target_os = "windows", target_os = "android"))]
use crate::backend::UiError;
#[cfg(any(target_os = "windows", target_os = "android"))]
use tauri::AppHandle;

#[cfg(any(target_os = "windows", target_os = "android"))]
use tauri_plugin_updater::{Error as PluginError, Updater, UpdaterExt as _};

#[cfg(any(target_os = "windows", target_os = "android"))]
pub(super) fn configured(value: Option<&serde_json::Value>) -> bool {
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
            endpoint.as_str().is_some_and(|raw| {
                !raw.contains("__TAURI_")
                    && url::Url::parse(raw).is_ok_and(|url| {
                        url.scheme() == "https"
                            && url.host().is_some()
                            && url.username().is_empty()
                            && url.password().is_none()
                    })
            })
        })
}

#[cfg(any(target_os = "windows", target_os = "android"))]
pub(super) fn configured_for_app(app: &AppHandle) -> bool {
    configured(app.config().plugins.0.get("updater"))
}

#[cfg(any(target_os = "windows", target_os = "android"))]
pub(super) fn updater(
    app: &AppHandle,
    timeout: Option<std::time::Duration>,
    installing: Option<tauri::ipc::Channel<super::UpdateProgress>>,
) -> Result<Option<Updater>, UiError> {
    if !configured_for_app(app) {
        return Ok(None);
    }
    let mut builder = app.updater_builder();
    if let Some(timeout) = timeout {
        builder = builder.timeout(timeout);
    }
    #[cfg(target_os = "windows")]
    if let Some(channel) = installing {
        builder = builder.on_before_exit(move || {
            let _ = channel.send(super::UpdateProgress::Installing);
        });
    }
    #[cfg(target_os = "android")]
    let _ = installing;
    builder
        .build()
        .map(Some)
        .map_err(|error| plugin_error(error, "update_check_failed"))
}

#[cfg(any(target_os = "windows", target_os = "android"))]
pub(super) fn plugin_error(error: PluginError, fallback: &'static str) -> UiError {
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
    UiError::new(
        code,
        matches!(
            code,
            "update_busy" | "update_check_failed" | "update_network_failed"
        ),
    )
}
