use copypaste_ipc::{ConfigApplied, ConfigPatch};

use super::open::Inner;
use super::state::write_settings;
use super::{BackendError, Result, MSG_INVALID_SETTING};

pub(super) fn get(inner: &Inner) -> Result<ConfigApplied> {
    Ok(ConfigApplied {
        config: inner.settings(),
        restart_required: Vec::new(),
    })
}

pub(super) fn set(inner: &Inner, patch: ConfigPatch) -> Result<ConfigApplied> {
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
}
