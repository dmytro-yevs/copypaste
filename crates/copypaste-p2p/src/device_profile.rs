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

/// Android-native facts supplied by the host before the P2P runtime starts.
/// Android's resource configuration is the authoritative source for phone
/// versus tablet; Rust must not infer it from ABI or model text.
#[cfg(target_os = "android")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AndroidHardwareProfile {
    pub model: Option<String>,
    pub os_version: Option<String>,
    pub device_class: DeviceClass,
}

impl DeviceProfile {
    #[must_use]
    pub fn current() -> Self {
        let os = os_info::get();
        let detected_os_name = match os.os_type() {
            os_info::Type::Unknown => None,
            known => bounded_text(known.to_string(), 64),
        };
        let detected_os_version = match os.version() {
            os_info::Version::Unknown => None,
            known => bounded_text(known.to_string(), 64),
        };
        let native = native_metadata();
        Self {
            app_version: Some(env!("CARGO_PKG_VERSION").to_string()),
            protocol_version: Some(PROTOCOL_VERSION),
            platform: DevicePlatform::current(),
            device_class: native.device_class,
            os_name: native.os_name.or(detected_os_name),
            os_version: native.os_version.or(detected_os_version),
            model: native.model,
        }
    }

    /// Supplies Android's platform-owned metadata before the P2P runtime starts.
    #[cfg(target_os = "android")]
    pub fn set_android_hardware_profile(profile: AndroidHardwareProfile) {
        let _ = ANDROID_HARDWARE.set(NativeMetadata {
            device_class: profile.device_class,
            os_name: Some("Android".to_string()),
            os_version: profile.os_version.and_then(|value| bounded_text(value, 64)),
            model: profile.model.and_then(|value| bounded_text(value, 128)),
        });
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct NativeMetadata {
    device_class: DeviceClass,
    os_name: Option<String>,
    os_version: Option<String>,
    model: Option<String>,
}

fn bounded_text(value: String, max_bytes: usize) -> Option<String> {
    let mut bounded = String::new();
    for character in value
        .trim()
        .chars()
        .filter(|character| !character.is_control())
    {
        if bounded.len() + character.len_utf8() > max_bytes {
            break;
        }
        bounded.push(character);
    }
    (!bounded.is_empty()).then_some(bounded)
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "android")))]
fn native_metadata() -> NativeMetadata {
    NativeMetadata::default()
}

#[cfg(target_os = "android")]
static ANDROID_HARDWARE: std::sync::OnceLock<NativeMetadata> = std::sync::OnceLock::new();

#[cfg(target_os = "android")]
fn native_metadata() -> NativeMetadata {
    ANDROID_HARDWARE.get().cloned().unwrap_or(NativeMetadata {
        os_name: Some("Android".to_string()),
        ..NativeMetadata::default()
    })
}

#[cfg(target_os = "macos")]
fn native_metadata() -> NativeMetadata {
    static HARDWARE: std::sync::OnceLock<NativeMetadata> = std::sync::OnceLock::new();
    HARDWARE.get_or_init(read_macos_hardware).clone()
}

#[cfg(target_os = "macos")]
fn read_macos_hardware() -> NativeMetadata {
    let model = macos_model_identifier();
    NativeMetadata {
        device_class: model.as_deref().map(macos_device_class).unwrap_or_default(),
        model,
        ..NativeMetadata::default()
    }
}

#[cfg(target_os = "macos")]
fn macos_model_identifier() -> Option<String> {
    use sysctl::Sysctl;

    sysctl::Ctl::new("hw.model")
        .ok()?
        .value_string()
        .ok()
        .and_then(|model| bounded_text(model, 128))
}

#[cfg(target_os = "macos")]
fn macos_device_class(model: &str) -> DeviceClass {
    let normalized = model.to_ascii_lowercase();
    if normalized.contains("macbook") {
        DeviceClass::Laptop
    } else if ["imac", "mac mini", "mac pro", "mac studio"]
        .iter()
        .any(|prefix| normalized.starts_with(prefix))
    {
        DeviceClass::Desktop
    } else if is_macos_model_identifier(&normalized) {
        macos_device_class_from_battery(macos_has_internal_battery())
    } else {
        DeviceClass::Unknown
    }
}

#[cfg(target_os = "macos")]
fn is_macos_model_identifier(model: &str) -> bool {
    let Some((family, revision)) = model
        .strip_prefix("mac")
        .and_then(|value| value.split_once(','))
    else {
        return false;
    };
    !family.is_empty()
        && !revision.is_empty()
        && family.bytes().all(|value| value.is_ascii_digit())
        && revision.bytes().all(|value| value.is_ascii_digit())
}

#[cfg(target_os = "macos")]
fn macos_device_class_from_battery(has_internal_battery: bool) -> DeviceClass {
    if has_internal_battery {
        DeviceClass::Laptop
    } else {
        DeviceClass::Desktop
    }
}

#[cfg(target_os = "macos")]
fn macos_has_internal_battery() -> bool {
    let mut command = std::process::Command::new("/usr/bin/pmset");
    command.args(["-g", "batt"]);
    command_output_with_timeout(&mut command, std::time::Duration::from_millis(250))
        .filter(|output| output.status.success())
        .is_some_and(|output| macos_battery_output_has_internal_battery(&output.stdout))
}

#[cfg(target_os = "macos")]
fn macos_battery_output_has_internal_battery(output: &[u8]) -> bool {
    String::from_utf8_lossy(output)
        .split_whitespace()
        .any(|value| value.starts_with("-InternalBattery"))
}

#[cfg(target_os = "windows")]
fn native_metadata() -> NativeMetadata {
    let hardware = windows_hardware();
    NativeMetadata {
        device_class: hardware.device_class,
        model: hardware.model,
        ..NativeMetadata::default()
    }
}

#[cfg(target_os = "windows")]
fn windows_hardware() -> NativeMetadata {
    static HARDWARE: std::sync::OnceLock<NativeMetadata> = std::sync::OnceLock::new();
    HARDWARE.get_or_init(read_windows_hardware).clone()
}

#[cfg(target_os = "windows")]
fn read_windows_hardware() -> NativeMetadata {
    NativeMetadata {
        device_class: windows_chassis_class(),
        model: windows_model(),
        ..NativeMetadata::default()
    }
}

#[cfg(target_os = "windows")]
fn windows_model() -> Option<String> {
    use winreg::{enums::HKEY_LOCAL_MACHINE, RegKey};

    RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey(r"HARDWARE\DESCRIPTION\System\BIOS")
        .ok()?
        .get_value::<String, _>("SystemProductName")
        .ok()
        .and_then(|model| bounded_text(model, 128))
}

#[cfg(target_os = "windows")]
fn windows_chassis_class() -> DeviceClass {
    let mut command = std::process::Command::new("powershell.exe");
    command.args([
        "-NoLogo",
        "-NoProfile",
        "-NonInteractive",
        "-Command",
        "(Get-CimInstance -ClassName Win32_SystemEnclosure).ChassisTypes -join ','",
    ]);
    let output = command_output_with_timeout(&mut command, std::time::Duration::from_millis(250))
        .filter(|output| output.status.success());
    let Some(output) = output else {
        return DeviceClass::Unknown;
    };
    let chassis_types = String::from_utf8_lossy(&output.stdout);
    let types = chassis_types
        .trim()
        .split(',')
        .filter_map(|value| value.trim().parse::<u8>().ok());
    windows_device_class_from_chassis(types)
}

/// Runs a platform probe without ever allowing a child process to hold up
/// discovery or runtime startup. A timed-out child is killed and reaped before
/// returning the fallback value.
#[cfg(any(target_os = "macos", target_os = "windows", test))]
fn command_output_with_timeout(
    command: &mut std::process::Command,
    timeout: std::time::Duration,
) -> Option<std::process::Output> {
    use std::process::Stdio;

    let child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    child_output_with_timeout(child, timeout)
}

#[cfg(any(target_os = "macos", target_os = "windows", test))]
fn child_output_with_timeout(
    mut child: std::process::Child,
    timeout: std::time::Duration,
) -> Option<std::process::Output> {
    let deadline = std::time::Instant::now().checked_add(timeout)?;
    loop {
        match child.try_wait().ok()? {
            Some(_) => return child.wait_with_output().ok(),
            None if std::time::Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
            None => std::thread::sleep(std::time::Duration::from_millis(5)),
        }
    }
}

#[cfg(target_os = "windows")]
fn windows_device_class_from_chassis(types: impl Iterator<Item = u8>) -> DeviceClass {
    let types = types.collect::<Vec<_>>();
    if types.iter().any(|kind| matches!(kind, 30 | 31 | 32)) {
        DeviceClass::Tablet
    } else if types.iter().any(|kind| matches!(kind, 8..=14)) {
        DeviceClass::Laptop
    } else if types.iter().any(|kind| matches!(kind, 3..=7 | 15..=24)) {
        DeviceClass::Desktop
    } else {
        DeviceClass::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_platform_text_without_splitting_unicode() {
        assert_eq!(
            bounded_text("  Pixel 9\n".to_string(), 128).as_deref(),
            Some("Pixel 9")
        );
        assert_eq!(bounded_text("éé".to_string(), 3).as_deref(), Some("é"));
        assert_eq!(bounded_text("\n\t".to_string(), 64), None);
    }

    #[test]
    fn current_profile_is_wire_safe() {
        let profile = DeviceProfile::current();
        assert!(profile.app_version.is_some());
        assert_eq!(profile.protocol_version, Some(PROTOCOL_VERSION));
        assert!(profile
            .os_name
            .as_ref()
            .is_none_or(|value| value.len() <= 64));
        assert!(profile
            .os_version
            .as_ref()
            .is_none_or(|value| value.len() <= 64));
        assert!(profile
            .model
            .as_ref()
            .is_none_or(|value| value.len() <= 128));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn classifies_known_macos_models() {
        assert_eq!(macos_device_class("MacBookAir10,1"), DeviceClass::Laptop);
        assert_eq!(macos_device_class("Mac Studio"), DeviceClass::Desktop);
        assert!(is_macos_model_identifier("mac16,13"));
        assert!(!is_macos_model_identifier("macbookair10,1"));
        assert_eq!(macos_device_class_from_battery(true), DeviceClass::Laptop);
        assert_eq!(macos_device_class_from_battery(false), DeviceClass::Desktop);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn only_internal_battery_output_marks_an_apple_silicon_mac_as_portable() {
        assert!(macos_battery_output_has_internal_battery(
            b"Now drawing from 'AC Power'\n -InternalBattery-0 (id=1) 80%; charging\n"
        ));
        assert!(!macos_battery_output_has_internal_battery(
            b"Now drawing from 'AC Power'\n -UPS-0 (id=1) 80%; charging\n"
        ));
    }

    #[cfg(unix)]
    #[test]
    fn timed_platform_probe_kills_and_reaps_a_stalled_child() {
        let mut command = std::process::Command::new("/bin/sh");
        command.args(["-c", "sleep 1"]);
        let started = std::time::Instant::now();
        assert!(
            command_output_with_timeout(&mut command, std::time::Duration::from_millis(10))
                .is_none()
        );
        assert!(started.elapsed() < std::time::Duration::from_millis(500));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn maps_windows_chassis_types_without_model_heuristics() {
        assert_eq!(
            windows_device_class_from_chassis([10].into_iter()),
            DeviceClass::Laptop
        );
        assert_eq!(
            windows_device_class_from_chassis([30].into_iter()),
            DeviceClass::Tablet
        );
        assert_eq!(
            windows_device_class_from_chassis([3].into_iter()),
            DeviceClass::Desktop
        );
        assert_eq!(
            windows_device_class_from_chassis([1].into_iter()),
            DeviceClass::Unknown
        );
    }
}
