//! Persisted live settings for in-process platform runtimes.
//!
//! Desktop daemon settings and Android in-process settings share the same
//! `ConfigPatch` validation and fail-closed record decoder. Runtime effects stay
//! with the runtime that owns the affected services.

use std::sync::{Mutex, RwLock};

use copypaste_core::Store;
use copypaste_ipc::{ConfigData, ConfigError, ConfigPatch, SettingsHealth};

const KEY_SETTINGS: &str = "settings";

#[derive(Debug)]
pub(crate) struct RuntimeSettings {
    store: Store,
    current: RwLock<SettingsState>,
    applying: Mutex<()>,
}

#[derive(Debug, Clone)]
struct SettingsState {
    config: ConfigData,
    private_mode_epoch: u64,
    health: Option<SettingsHealth>,
}

#[derive(Debug, Clone)]
pub(crate) struct SettingsSnapshot {
    pub(crate) config: ConfigData,
    pub(crate) private_mode_epoch: u64,
    pub(crate) health: Option<SettingsHealth>,
}

#[derive(Debug, Clone)]
pub(crate) struct SettingsApplied {
    pub(crate) before: ConfigData,
    pub(crate) config: ConfigData,
    pub(crate) private_mode_epoch: u64,
}

impl RuntimeSettings {
    pub(crate) fn load(store: &Store) -> Self {
        let (config, health) = match store.state(KEY_SETTINGS) {
            Ok(Some(raw)) => copypaste_core::settings_record::read(&raw),
            Ok(None) => (ConfigData::default(), SettingsHealth::default()),
            Err(_) => (
                copypaste_core::settings_record::all_closed(),
                SettingsHealth {
                    record_unreadable: true,
                    unreadable_fields: Vec::new(),
                },
            ),
        };
        Self {
            store: store.clone(),
            current: RwLock::new(SettingsState {
                config,
                private_mode_epoch: 0,
                health: health.is_degraded().then_some(health),
            }),
            applying: Mutex::new(()),
        }
    }

    pub(crate) fn snapshot(&self) -> SettingsSnapshot {
        let current = self
            .current
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        SettingsSnapshot {
            config: current.config.clone(),
            private_mode_epoch: current.private_mode_epoch,
            health: current.health.clone(),
        }
    }

    pub(crate) fn config(&self) -> ConfigData {
        self.snapshot().config
    }

    pub(crate) fn apply(&self, patch: &ConfigPatch) -> Result<SettingsApplied, SettingsError> {
        let _serialised = self
            .applying
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let (before, next, next_epoch) = {
            let current = self
                .current
                .read()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let next = patch.apply(&current.config)?;
            let next_epoch = if patch.private_mode.is_some() {
                current
                    .private_mode_epoch
                    .checked_add(1)
                    .ok_or(SettingsError::Store)?
            } else {
                current.private_mode_epoch
            };
            (current.config.clone(), next, next_epoch)
        };

        let encoded = serde_json::to_string(&next).map_err(|_| SettingsError::Store)?;
        self.store
            .set_state(KEY_SETTINGS, &encoded)
            .map_err(|_| SettingsError::Store)?;

        let mut current = self
            .current
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        current.config = next.clone();
        current.private_mode_epoch = next_epoch;
        current.health = None;
        Ok(SettingsApplied {
            before,
            config: next,
            private_mode_epoch: next_epoch,
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum SettingsError {
    #[error("{0}")]
    Invalid(#[from] ConfigError),
    #[error("the settings could not be saved")]
    Store,
}

#[cfg(test)]
mod tests {
    use super::*;
    use copypaste_core::Keyring;

    fn settings() -> (RuntimeSettings, tempfile::TempDir) {
        let directory = tempfile::tempdir().unwrap();
        let keyring = Keyring::from_secret(&[17; 32]);
        let store = Store::open(&directory.path().join("history.db"), &keyring.db_key()).unwrap();
        (RuntimeSettings::load(&store), directory)
    }

    #[test]
    fn settings_persist_and_private_mode_advances_its_epoch() {
        let (settings, _directory) = settings();
        let applied = settings
            .apply(&ConfigPatch {
                private_mode: Some(true),
                retention_days: Some(30),
                ..Default::default()
            })
            .unwrap();

        assert!(applied.config.private_mode);
        assert_eq!(applied.config.retention_days, 30);
        assert_eq!(applied.private_mode_epoch, 1);
        assert!(settings.snapshot().health.is_none());
    }

    #[test]
    fn rejected_patch_does_not_change_the_live_value() {
        let (settings, _directory) = settings();
        let before = settings.config();
        assert!(settings
            .apply(&ConfigPatch {
                history_limit: Some(1),
                ..Default::default()
            })
            .is_err());
        assert_eq!(settings.config(), before);
    }
}
