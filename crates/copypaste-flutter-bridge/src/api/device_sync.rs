//! What syncs with which device.

use copypaste_ipc::device_sync::{DeviceSyncSettings, PeerSyncPolicy, SyncContent};
use copypaste_ipc::{ConfigData, ConfigPatch, Method, ResponseData};

use super::{expect_data, RuntimeError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyncContentKinds {
    pub text: bool,
    pub images: bool,
    pub files: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceSyncPolicy {
    pub enabled: bool,
    /// Automatic sync stops; a manual send still works.
    pub paused: bool,
    pub content: SyncContentKinds,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceSyncEntry {
    pub device_id: String,
    pub policy: DeviceSyncPolicy,
}

/// This device's switches and every paired device that differs from the
/// default. A device without an entry syncs everything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceSyncPreferences {
    pub this_device: DeviceSyncPolicy,
    pub devices: Vec<DeviceSyncEntry>,
}

fn kinds(value: SyncContent) -> SyncContentKinds {
    SyncContentKinds {
        text: value.text,
        images: value.images,
        files: value.files,
    }
}

fn content(value: SyncContentKinds) -> SyncContent {
    SyncContent {
        text: value.text,
        images: value.images,
        files: value.files,
    }
}

fn preferences(config: &ConfigData) -> DeviceSyncPreferences {
    DeviceSyncPreferences {
        this_device: DeviceSyncPolicy {
            enabled: config.sync_enabled,
            paused: config.device_sync.paused,
            content: kinds(config.device_sync.this_device),
        },
        devices: config
            .device_sync
            .devices
            .iter()
            .map(|(device_id, policy)| DeviceSyncEntry {
                device_id: device_id.clone(),
                policy: DeviceSyncPolicy {
                    enabled: policy.enabled,
                    paused: policy.paused,
                    content: kinds(policy.content),
                },
            })
            .collect(),
    }
}

/// `settings` with one device's policy replaced; a default policy drops the
/// entry so the record only holds what the user changed.
fn with_device(
    mut settings: DeviceSyncSettings,
    device_id: String,
    policy: DeviceSyncPolicy,
) -> DeviceSyncSettings {
    let policy = PeerSyncPolicy {
        enabled: policy.enabled,
        paused: policy.paused,
        content: content(policy.content),
    };
    if policy == PeerSyncPolicy::default() {
        settings.devices.remove(&device_id);
    } else {
        settings.devices.insert(device_id, policy);
    }
    settings
}

async fn set(patch: ConfigPatch) -> Result<DeviceSyncPreferences, RuntimeError> {
    let applied = expect_data!(Method::SetConfig { patch }, ResponseData::Config);
    Ok(preferences(&applied.config))
}

pub async fn device_sync_preferences() -> Result<DeviceSyncPreferences, RuntimeError> {
    let applied = expect_data!(Method::GetConfig, ResponseData::Config);
    Ok(preferences(&applied.config))
}

/// Set this device's sync switch and the kinds it exchanges with every peer
/// and the cloud account.
pub async fn set_this_device_sync(
    policy: DeviceSyncPolicy,
) -> Result<DeviceSyncPreferences, RuntimeError> {
    let applied = expect_data!(Method::GetConfig, ResponseData::Config);
    set(ConfigPatch {
        sync_enabled: Some(policy.enabled),
        device_sync: Some(DeviceSyncSettings {
            paused: policy.paused,
            this_device: content(policy.content),
            ..applied.config.device_sync
        }),
        ..ConfigPatch::default()
    })
    .await
}

/// Set what this device exchanges with one paired device.
pub async fn set_device_sync_policy(
    device_id: String,
    policy: DeviceSyncPolicy,
) -> Result<DeviceSyncPreferences, RuntimeError> {
    let applied = expect_data!(Method::GetConfig, ResponseData::Config);
    set(ConfigPatch {
        device_sync: Some(with_device(applied.config.device_sync, device_id, policy)),
        ..ConfigPatch::default()
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_default_policy_removes_the_entry_and_any_other_keeps_it() {
        let off = DeviceSyncPolicy {
            enabled: false,
            paused: false,
            content: kinds(SyncContent::ALL),
        };
        let stored = with_device(DeviceSyncSettings::default(), "phone".into(), off);
        assert!(!stored.device_enabled("phone"));
        let restored = with_device(
            stored,
            "phone".into(),
            DeviceSyncPolicy {
                enabled: true,
                ..off
            },
        );
        assert!(restored.devices.is_empty());
    }
}
