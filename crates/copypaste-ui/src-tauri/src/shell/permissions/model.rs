//! Shared permission DTOs. Policy lives in [`super::policy`]; each OS fills facts.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "typescript",
    ts(rename = "PermissionHost", export_to = "ipc.ts")
)]
#[serde(rename_all = "snake_case")]
pub enum PermissionHost {
    Macos,
    Windows,
    Android,
    Linux,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "typescript",
    ts(rename = "OnboardingPermissionId", export_to = "ipc.ts")
)]
#[serde(rename_all = "snake_case")]
pub enum PermissionId {
    Notifications,
    Tile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "typescript",
    ts(rename = "OnboardingPermissionStatus", export_to = "ipc.ts")
)]
#[serde(rename_all = "snake_case")]
pub enum PermissionStatus {
    Prompt,
    Granted,
    Denied,
    NotRequired,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "typescript",
    ts(rename = "OnboardingPermissionItem", export_to = "ipc.ts")
)]
#[serde(rename_all = "camelCase")]
pub struct PermissionItem {
    pub id: PermissionId,
    pub status: PermissionStatus,
    /// Product rule: nothing here is a gate. The shell may always continue.
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "typescript",
    ts(rename = "OnboardingPermissions", export_to = "ipc.ts")
)]
#[serde(rename_all = "camelCase")]
pub struct OnboardingPermissions {
    pub platform: PermissionHost,
    pub notifications: PermissionItem,
    pub tile: PermissionItem,
    /// Clipboard capture itself never needs a TCC / runtime grant on the
    /// shipped platforms. Android background capture is optional Shizuku.
    pub clipboard_status: PermissionStatus,
}

impl PermissionItem {
    pub fn of(id: PermissionId, status: PermissionStatus) -> Self {
        Self {
            id,
            status,
            required: false,
        }
    }
}

impl OnboardingPermissions {
    pub fn assemble(
        platform: PermissionHost,
        notifications: PermissionStatus,
        tile: PermissionStatus,
    ) -> Self {
        Self {
            platform,
            notifications: PermissionItem::of(PermissionId::Notifications, notifications),
            tile: PermissionItem::of(PermissionId::Tile, tile),
            clipboard_status: PermissionStatus::NotRequired,
        }
    }
}
