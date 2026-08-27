//! Bounded, non-secret device metadata shared through discovery and Noise.

use serde::{Deserialize, Serialize};

use copypaste_ipc::{DeviceClass, DevicePlatform};

use crate::protocol::PROTOCOL_VERSION;

/// Claims made by a device about itself. Trust comes from the channel carrying
/// the value: mDNS is unverified, while the same value inside Noise is
/// authenticated to the pairing key.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceProfile {
    pub app_version: Option<String>,
    pub protocol_version: Option<u32>,
    pub platform: DevicePlatform,
    pub device_class: DeviceClass,
    pub os_name: Option<String>,
    pub os_version: Option<String>,
    pub model: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthenticatedDeviceProfile {
    pub profile: DeviceProfile,
    pub observed_at_ms: i64,
    pub fresh_until_ms: i64,
}

impl DeviceProfile {
    #[must_use]
    pub fn current() -> Self {
        let os = os_info::get();
        let os_name = match os.os_type() {
            os_info::Type::Unknown => None,
            known => Some(known.to_string()),
        };
        let os_version = match os.version() {
            os_info::Version::Unknown => None,
            known => Some(known.to_string()),
        };
        Self {
            app_version: Some(env!("CARGO_PKG_VERSION").to_string()),
            protocol_version: Some(PROTOCOL_VERSION),
            platform: DevicePlatform::current(),
            // The OS target does not distinguish a MacBook from a Mac mini or
            // an Android phone from a tablet. Unknown is truthful until a
            // platform-native model source supplies the distinction.
            device_class: DeviceClass::Unknown,
            os_name,
            os_version,
            model: None,
        }
    }
}
