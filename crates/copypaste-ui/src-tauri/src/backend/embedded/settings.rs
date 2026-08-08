use std::fs;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use copypaste_ipc::{ConfigApplied, ConfigData, ConfigPatch, PrivateModeData};

use super::open::{EmbeddedBackend, Inner};
use super::{BackendError, Result};

const MSG_INVALID_SETTING: &str = "That setting isn't valid.";
const MSG_SAVE_FAILED: &str = "CopyPaste couldn't save these settings.";

#[derive(Clone)]
pub(super) struct SettingsSnapshot {
    pub(super) config: ConfigData,
    pub(super) private_mode_epoch: u64,
}

pub(super) struct EmbeddedSettings {
    path: PathBuf,
    current: RwLock<SettingsSnapshot>,
}

impl EmbeddedSettings {
    pub(super) fn open(path: PathBuf) -> Self {
        let config = fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        Self {
            path,
            current: RwLock::new(SettingsSnapshot {
                config,
                private_mode_epoch: 0,
            }),
        }
    }

    pub(super) fn snapshot(&self) -> SettingsSnapshot {
        self.current
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    fn apply(&self, inner: &Inner, patch: &ConfigPatch) -> Result<AppliedSettings> {
        let mut current = self
            .current
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let next = patch
            .apply(&current.config)
            .map_err(|_| BackendError::Invalid(MSG_INVALID_SETTING))?;
        let next_epoch = if patch.private_mode.is_some() {
            current
                .private_mode_epoch
                .checked_add(1)
                .ok_or_else(|| BackendError::internal(MSG_SAVE_FAILED))?
        } else {
            current.private_mode_epoch
        };
        write_settings(&self.path, &next)?;
        current.config = next.clone();
        current.private_mode_epoch = next_epoch;
        inner.publish_items(false, 0);
        Ok(AppliedSettings {
            config: next,
            private_mode_epoch: next_epoch,
        })
    }

    #[cfg(test)]
    pub(super) fn path(&self) -> &Path {
        &self.path
    }
}

struct AppliedSettings {
    config: ConfigData,
    private_mode_epoch: u64,
}

fn write_settings(path: &Path, settings: &ConfigData) -> Result<()> {
    let encoded =
        serde_json::to_vec(settings).map_err(|_| BackendError::internal(MSG_SAVE_FAILED))?;
    copypaste_fs::write_atomically(path, &encoded, copypaste_fs::Visibility::Inherited)
        .map_err(|_| BackendError::internal(MSG_SAVE_FAILED))
}

pub(super) async fn get(backend: &EmbeddedBackend) -> Result<ConfigApplied> {
    backend
        .blocking(move |inner| {
            let current = inner.state.settings.snapshot();
            Ok(ConfigApplied {
                config: current.config,
                restart_required: Vec::new(),
            })
        })
        .await
}

pub(super) async fn set(backend: &EmbeddedBackend, patch: ConfigPatch) -> Result<ConfigApplied> {
    backend
        .blocking(move |inner| {
            let restart_required = copypaste_ipc::ConfigData::restart_required_by(&patch)
                .into_iter()
                .map(str::to_string)
                .collect();
            let applied = inner.state.settings.apply(inner, &patch)?;
            Ok(ConfigApplied {
                config: applied.config,
                restart_required,
            })
        })
        .await
}

pub(super) async fn get_private_mode(backend: &EmbeddedBackend) -> Result<PrivateModeData> {
    backend
        .blocking(move |inner| {
            let current = inner.state.settings.snapshot();
            Ok(PrivateModeData {
                private_mode: current.config.private_mode,
                private_mode_epoch: current.private_mode_epoch,
            })
        })
        .await
}

pub(super) async fn set_private_mode(
    backend: &EmbeddedBackend,
    enabled: bool,
) -> Result<PrivateModeData> {
    backend
        .blocking(move |inner| {
            let applied = inner.state.settings.apply(
                inner,
                &ConfigPatch {
                    private_mode: Some(enabled),
                    ..ConfigPatch::default()
                },
            )?;
            Ok(PrivateModeData {
                private_mode: applied.config.private_mode,
                private_mode_epoch: applied.private_mode_epoch,
            })
        })
        .await
}
