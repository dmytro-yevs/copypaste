use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export_to = "ipc.ts"))]
#[serde(rename_all = "snake_case")]
pub enum DeviceObservationProvenance {
    SelfReported,
    Observed,
    Measured,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export_to = "ipc.ts"))]
#[serde(rename_all = "snake_case")]
pub enum DeviceObservationTrust {
    Local,
    Unverified,
    Authenticated,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export_to = "ipc.ts"))]
#[serde(rename_all = "snake_case")]
pub enum DevicePlatform {
    Macos,
    Windows,
    Android,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export_to = "ipc.ts"))]
#[serde(rename_all = "snake_case")]
pub enum DeviceClass {
    Desktop,
    Laptop,
    Phone,
    Tablet,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export_to = "ipc.ts"))]
pub struct DeviceProfileObservation {
    pub display_name: String,
    pub app_version: Option<String>,
    pub protocol_version: Option<u32>,
    pub platform: DevicePlatform,
    pub device_class: DeviceClass,
    pub os_name: Option<String>,
    pub os_version: Option<String>,
    pub model: Option<String>,
    pub provenance: DeviceObservationProvenance,
    pub trust: DeviceObservationTrust,
    pub observed_at_ms: i64,
    pub fresh_until_ms: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export_to = "ipc.ts"))]
pub struct DeviceEndpointObservation {
    pub lan_endpoint: String,
    pub provenance: DeviceObservationProvenance,
    pub trust: DeviceObservationTrust,
    pub observed_at_ms: i64,
    pub fresh_until_ms: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export_to = "ipc.ts"))]
pub struct DeviceLatencyObservation {
    pub connect_latency_ms: u64,
    pub provenance: DeviceObservationProvenance,
    pub trust: DeviceObservationTrust,
    pub observed_at_ms: i64,
    pub fresh_until_ms: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export_to = "ipc.ts"))]
pub struct DevicePresenceObservation {
    pub online: bool,
    pub last_seen_ms: i64,
    pub provenance: DeviceObservationProvenance,
    pub trust: DeviceObservationTrust,
    pub observed_at_ms: i64,
    pub fresh_until_ms: Option<i64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export_to = "ipc.ts"))]
#[serde(tag = "availability", rename_all = "snake_case")]
pub enum ExternalNetworkObservation {
    #[default]
    Unavailable,
    Available {
        value: String,
        provenance: DeviceObservationProvenance,
        trust: DeviceObservationTrust,
        observed_at_ms: i64,
        fresh_until_ms: Option<i64>,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export_to = "ipc.ts"))]
pub struct DeviceDetails {
    pub profile: Option<DeviceProfileObservation>,
    pub endpoint: Option<DeviceEndpointObservation>,
    pub latency: Option<DeviceLatencyObservation>,
    pub presence: Option<DevicePresenceObservation>,
    pub public_ip: ExternalNetworkObservation,
    pub geo: ExternalNetworkObservation,
}
