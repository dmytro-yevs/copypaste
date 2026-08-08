use copypaste_ipc::{ConfigApplied, ConfigPatch, PrivateModeData};

use super::open::EmbeddedBackend;
use super::state::write_settings;
use super::{BackendError, Result};

const MSG_INVALID_SETTING: &str = "That setting isn't valid.";

pub(super) async fn get(backend: &EmbeddedBackend) -> Result<ConfigApplied> {
    backend
        .blocking(move |inner| {
            Ok(ConfigApplied {
                config: inner.settings(),
                restart_required: Vec::new(),
            })
        })
        .await
}

pub(super) async fn set(backend: &EmbeddedBackend, patch: ConfigPatch) -> Result<ConfigApplied> {
    backend
        .blocking(move |inner| {
            let current = inner.settings();
            let next = patch
                .apply(&current)
                .map_err(|_| BackendError::Invalid(MSG_INVALID_SETTING))?;
            let restart_required = copypaste_ipc::ConfigData::restart_required_by(&patch)
                .into_iter()
                .map(str::to_string)
                .collect();
            write_settings(&inner.state.settings_path, &next)?;
            *inner
                .state
                .settings
                .write()
                .expect("settings lock poisoned") = next.clone();
            inner.publish_items(false, 0);
            Ok(ConfigApplied {
                config: next,
                restart_required,
            })
        })
        .await
}

pub(super) async fn get_private_mode(backend: &EmbeddedBackend) -> Result<PrivateModeData> {
    backend
        .blocking(move |inner| {
            Ok(PrivateModeData {
                private_mode: inner.settings().private_mode,
            })
        })
        .await
}

pub(super) async fn set_private_mode(
    backend: &EmbeddedBackend,
    enabled: bool,
) -> Result<PrivateModeData> {
    let applied = set(
        backend,
        ConfigPatch {
            private_mode: Some(enabled),
            ..ConfigPatch::default()
        },
    )
    .await?;
    Ok(PrivateModeData {
        private_mode: applied.config.private_mode,
    })
}
