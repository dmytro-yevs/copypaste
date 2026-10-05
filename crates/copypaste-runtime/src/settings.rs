//! Persisted live settings for in-process platform runtimes.
//!
//! Desktop daemon settings and Android in-process settings share the same
//! `ConfigPatch` validation and fail-closed record decoder. Runtime effects stay
//! with the runtime that owns the affected services.

use crate::capture_admission::CaptureAdmission;
use std::sync::{Arc, Condvar, Mutex, RwLock};

use copypaste_core::Store;
use copypaste_ipc::{ConfigData, ConfigError, ConfigPatch, SettingsHealth};

const KEY_SETTINGS: &str = "settings";

#[derive(Debug)]
pub(crate) struct RuntimeSettings {
    store: Store,
    current: RwLock<SettingsState>,
    applying: Mutex<bool>,
    applied: Condvar,
    pub(crate) capture: Arc<CaptureAdmission>,
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
            capture: Arc::new(CaptureAdmission::new(config.clone())),
            current: RwLock::new(SettingsState {
                config,
                private_mode_epoch: 0,
                health: health.is_degraded().then_some(health),
            }),
            applying: Mutex::new(false),
            applied: Condvar::new(),
        }
    }

    pub(crate) fn snapshot(&self) -> SettingsSnapshot {
        let current = match self.current.read() {
            Ok(current) => current,
            Err(poison) => {
                drop(poison);
                self.capture.fail_closed();
                return SettingsSnapshot {
                    config: copypaste_core::settings_record::all_closed(),
                    private_mode_epoch: u64::MAX,
                    health: Some(SettingsHealth {
                        record_unreadable: true,
                        unreadable_fields: Vec::new(),
                    }),
                };
            }
        };
        SettingsSnapshot {
            config: current.config.clone(),
            private_mode_epoch: current.private_mode_epoch,
            health: current.health.clone(),
        }
    }

    pub(crate) fn config(&self) -> ConfigData {
        self.snapshot().config
    }

    fn enter_apply(&self) -> Result<Applying<'_>, SettingsError> {
        let mut applying = self.applying.lock().map_err(|poison| {
            drop(poison);
            self.capture.fail_closed();
            SettingsError::Store
        })?;
        while *applying {
            applying = self.applied.wait(applying).map_err(|poison| {
                drop(poison);
                self.capture.fail_closed();
                SettingsError::Store
            })?;
        }
        *applying = true;
        // Serial ownership survives, but no mutex crosses native cancellation.
        drop(applying);
        Ok(Applying { settings: self })
    }

    pub(crate) fn apply(&self, patch: &ConfigPatch) -> Result<SettingsApplied, SettingsError> {
        let _serialised = self.enter_apply()?;
        let (before, next, next_epoch) = {
            let current = self.current.read().map_err(|poison| {
                drop(poison);
                self.capture.fail_closed();
                SettingsError::Store
            })?;
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

        let transition = self
            .capture
            .transition()
            .map_err(|_| SettingsError::Store)?;
        let encoded = serde_json::to_string(&next).map_err(|_| SettingsError::Store)?;
        self.store
            .set_state(KEY_SETTINGS, &encoded)
            .map_err(|_| SettingsError::Store)?;

        let mut current = self.current.write().map_err(|poison| {
            drop(poison);
            self.capture.fail_closed();
            SettingsError::Store
        })?;
        current.config = next.clone();
        current.private_mode_epoch = next_epoch;
        current.health = None;
        drop(current);
        transition.publish(next.clone()).map_err(|_| {
            self.capture.fail_closed();
            SettingsError::Store
        })?;
        Ok(SettingsApplied {
            before,
            config: next,
            private_mode_epoch: next_epoch,
        })
    }
}

struct Applying<'a> {
    settings: &'a RuntimeSettings,
}
impl Drop for Applying<'_> {
    fn drop(&mut self) {
        let mut applying = self
            .settings
            .applying
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        *applying = false;
        self.settings.applied.notify_all();
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
    #[test]
    fn poisoned_settings_close_admission_and_refuse_policy_acknowledgement() {
        use crate::capture_admission::{CaptureKind, CaptureScope};
        for applying_lock in [false, true] {
            let (settings, _directory) = settings();
            let host = settings.capture.open_host(CaptureKind::Explicit).unwrap();
            let token = settings.capture.begin(host, Arc::new(|| {})).unwrap();
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                if applying_lock {
                    let _guard = settings.applying.lock().unwrap();
                    panic!("poison applying");
                } else {
                    let _guard = settings.current.write().unwrap();
                    panic!("poison snapshot");
                }
            }));
            assert!(settings
                .apply(&ConfigPatch {
                    private_mode: Some(true),
                    ..Default::default()
                })
                .is_err());
            assert!(settings
                .capture
                .acquire(token, CaptureScope::Read)
                .is_none());
            assert!(settings.capture.open_host(CaptureKind::Explicit).is_none());
            if !applying_lock {
                assert!(settings.snapshot().config.private_mode);
            }
        }
    }
}
