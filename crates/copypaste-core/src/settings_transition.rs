//! Shared settings transitions; hosts retain their native capture authority.
use copypaste_ipc::{ConfigData, ConfigError, ConfigPatch};

#[derive(Debug, thiserror::Error)]
pub enum TransitionError {
    #[error("{0}")]
    Invalid(#[from] ConfigError),
    #[error("the settings could not be saved")]
    EpochExhausted,
}

#[derive(Debug, Clone)]
pub struct SettingsTransition {
    pub before: ConfigData,
    pub config: ConfigData,
    pub private_mode_epoch: u64,
    pub capture_privacy_epoch: u64,
}
impl SettingsTransition {
    pub fn prepare(
        before: &ConfigData,
        private_mode_epoch: u64,
        capture_privacy_epoch: u64,
        patch: &ConfigPatch,
    ) -> Result<Self, TransitionError> {
        let config = patch.apply(before)?;
        let private_mode_epoch = if patch.private_mode.is_some() {
            private_mode_epoch
                .checked_add(1)
                .ok_or(TransitionError::EpochExhausted)?
        } else {
            private_mode_epoch
        };
        let privacy_changed = config.private_mode != before.private_mode
            || config.excluded_app_bundle_ids != before.excluded_app_bundle_ids
            || config.skip_secret != before.skip_secret
            || config.skip_transient != before.skip_transient;
        let capture_privacy_epoch = if privacy_changed {
            capture_privacy_epoch
                .checked_add(1)
                .ok_or(TransitionError::EpochExhausted)?
        } else {
            capture_privacy_epoch
        };
        Ok(Self {
            before: before.clone(),
            config,
            private_mode_epoch,
            capture_privacy_epoch,
        })
    }
    pub fn config(&self) -> &ConfigData {
        &self.config
    }
    pub fn resumes_capture(&self) -> bool {
        self.before.private_mode && !self.config.private_mode
    }
    pub fn should_enforce_retention(&self) -> bool {
        crate::retention::policy_tightened(&self.before, &self.config)
    }
    pub fn lan_visibility_changed(&self) -> bool {
        self.config.lan_visibility != self.before.lan_visibility
    }
    pub fn sync_enabled_changed(&self) -> Option<bool> {
        (self.config.sync_enabled != self.before.sync_enabled).then_some(self.config.sync_enabled)
    }
    /// Persist before publishing; native cancellation belongs to the host.
    pub fn persist<E>(
        &self,
        write: impl FnOnce(&str) -> Result<(), E>,
    ) -> Result<(), PersistError<E>> {
        let encoded = serde_json::to_string(&self.config).map_err(|_| PersistError::Encode)?;
        write(&encoded).map_err(PersistError::Store)
    }
}
#[derive(Debug)]
pub enum PersistError<E> {
    Encode,
    Store(E),
}
