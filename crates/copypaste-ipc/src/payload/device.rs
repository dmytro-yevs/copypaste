use serde::{Deserialize, Serialize};

use super::PeerInfo;

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

impl DevicePlatform {
    #[must_use]
    pub const fn current() -> Self {
        if cfg!(target_os = "macos") {
            Self::Macos
        } else if cfg!(target_os = "windows") {
            Self::Windows
        } else if cfg!(target_os = "android") {
            Self::Android
        } else {
            Self::Unknown
        }
    }

    #[must_use]
    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::Macos => "macos",
            Self::Windows => "windows",
            Self::Android => "android",
            Self::Unknown => "unknown",
        }
    }

    #[must_use]
    pub fn from_wire_name(value: &str) -> Self {
        match value {
            "macos" => Self::Macos,
            "windows" => Self::Windows,
            "android" => Self::Android,
            _ => Self::Unknown,
        }
    }
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

impl DeviceClass {
    #[must_use]
    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::Desktop => "desktop",
            Self::Laptop => "laptop",
            Self::Phone => "phone",
            Self::Tablet => "tablet",
            Self::Unknown => "unknown",
        }
    }

    #[must_use]
    pub fn from_wire_name(value: &str) -> Self {
        match value {
            "desktop" => Self::Desktop,
            "laptop" => Self::Laptop,
            "phone" => Self::Phone,
            "tablet" => Self::Tablet,
            _ => Self::Unknown,
        }
    }
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export_to = "ipc.ts"))]
#[serde(rename_all = "snake_case")]
pub enum DevicePresence {
    Online,
    Offline,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export_to = "ipc.ts"))]
pub struct DevicePresenceObservation {
    pub state: DevicePresence,
    pub last_seen_ms: i64,
    pub provenance: DeviceObservationProvenance,
    pub trust: DeviceObservationTrust,
    pub observed_at_ms: i64,
    pub fresh_until_ms: Option<i64>,
}

impl DevicePresenceObservation {
    #[must_use]
    pub fn is_current_online_at(&self, now_ms: i64) -> bool {
        self.state == DevicePresence::Online
            && self.observed_at_ms <= now_ms
            && matches!(self.fresh_until_ms, Some(fresh_until_ms) if now_ms <= fresh_until_ms)
    }
}

impl PeerInfo {
    #[must_use]
    pub fn online_projection_at(&self, now_ms: i64) -> bool {
        self.details
            .as_ref()
            .and_then(|details| details.presence.as_ref())
            .is_some_and(|presence| presence.is_current_online_at(now_ms))
    }
}

#[derive(Serialize, Deserialize)]
struct PeerInfoWire {
    pairing_id: String,
    name: String,
    last_addr: Option<String>,
    last_seen_ms: i64,
    #[serde(default)]
    online: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    details: Option<DeviceDetails>,
}

impl Serialize for PeerInfo {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        PeerInfoWire {
            pairing_id: self.pairing_id.clone(),
            name: self.name.clone(),
            last_addr: self.last_addr.clone(),
            last_seen_ms: self.last_seen_ms,
            online: self.online_projection_at(peer_wire_now_ms()),
            details: self.details.clone(),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for PeerInfo {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = PeerInfoWire::deserialize(deserializer)?;
        let mut peer = Self {
            pairing_id: wire.pairing_id,
            name: wire.name,
            last_addr: wire.last_addr,
            last_seen_ms: wire.last_seen_ms,
            online: wire.online,
            details: wire.details,
        };
        peer.online = peer.online_projection_at(peer_wire_now_ms());
        Ok(peer)
    }
}

fn peer_wire_now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_millis()).ok())
        .unwrap_or(0)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn presence(state: DevicePresence, fresh_until_ms: Option<i64>) -> DevicePresenceObservation {
        DevicePresenceObservation {
            state,
            last_seen_ms: 10,
            provenance: DeviceObservationProvenance::Observed,
            trust: DeviceObservationTrust::Local,
            observed_at_ms: 10,
            fresh_until_ms,
        }
    }

    #[test]
    fn presence_serializes_as_a_tristate_contract() {
        let value = serde_json::to_value(presence(DevicePresence::Offline, Some(20))).unwrap();
        assert_eq!(value["state"], "offline");

        let missing_state =
            serde_json::from_value::<DevicePresenceObservation>(serde_json::json!({
                "last_seen_ms": 10,
                "provenance": "observed",
                "trust": "local",
                "observed_at_ms": 10,
                "fresh_until_ms": 20,
            }));
        assert!(missing_state.is_err());
    }

    #[test]
    fn online_projection_fails_closed_when_not_current() {
        assert!(presence(DevicePresence::Online, Some(20)).is_current_online_at(20));
        assert!(!presence(DevicePresence::Online, Some(19)).is_current_online_at(20));
        assert!(!presence(DevicePresence::Online, None).is_current_online_at(20));
        assert!(!presence(DevicePresence::Offline, Some(20)).is_current_online_at(20));
        assert!(!presence(DevicePresence::Unknown, Some(20)).is_current_online_at(20));
    }
}
