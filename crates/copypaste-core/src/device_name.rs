//! System-provided display names; persistence and manual overrides live in storage.

#[derive(Debug, Clone)]
pub struct SystemDeviceName {
    name: String,
}

impl SystemDeviceName {
    pub fn from_sources(system_name: Option<&str>, model: Option<&str>) -> Self {
        let name = system_name
            .and_then(usable_name)
            .or_else(|| model.and_then(usable_name))
            .unwrap_or_else(|| "CopyPaste device".into());
        Self { name }
    }

    pub fn as_str(&self) -> &str {
        &self.name
    }

    #[cfg(not(target_os = "android"))]
    pub fn current() -> Self {
        let system = whoami::devicename().ok();
        let model = model();
        Self::from_sources(system.as_deref(), model.as_deref())
    }
}

fn usable_name(raw: &str) -> Option<String> {
    sanitise_name(raw).filter(|name| {
        !["unknown", "localhost", "localhost.localdomain"]
            .iter()
            .any(|placeholder| name.eq_ignore_ascii_case(placeholder))
    })
}

pub(crate) fn sanitise_name(raw: &str) -> Option<String> {
    let cleaned: String = raw
        .trim()
        .chars()
        .filter(|c| !c.is_control())
        .take(copypaste_p2p::protocol::MAX_DEVICE_NAME_BYTES / 4)
        .collect();
    let cleaned = cleaned.trim().to_string();
    (!cleaned.is_empty()).then_some(cleaned)
}

#[cfg(target_os = "macos")]
fn model() -> Option<String> {
    use sysctl::Sysctl;
    sysctl::Ctl::new("hw.model").ok()?.value_string().ok()
}

#[cfg(target_os = "windows")]
fn model() -> Option<String> {
    use winreg::{enums::HKEY_LOCAL_MACHINE, RegKey};
    RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey(r"HARDWARE\DESCRIPTION\System\BIOS")
        .ok()?
        .get_value("SystemProductName")
        .ok()
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "android")))]
fn model() -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "macos")]
    #[test]
    fn native_name_matches_macos_settings() {
        let output = std::process::Command::new("/usr/sbin/scutil")
            .args(["--get", "ComputerName"])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "macOS ComputerName must be readable"
        );
        let expected = String::from_utf8(output.stdout).unwrap();
        assert_eq!(
            SystemDeviceName::current().as_str(),
            sanitise_name(&expected).unwrap()
        );
        assert!(model().is_some(), "macOS hardware model must be readable");
    }

    #[test]
    fn every_platform_uses_the_same_precedence_and_bounds() {
        assert_eq!(
            SystemDeviceName::from_sources(Some(" Office Mac "), Some("Mac16,1")).as_str(),
            "Office Mac"
        );
        assert_eq!(
            SystemDeviceName::from_sources(None, Some("Pixel 9")).as_str(),
            "Pixel 9"
        );
        for invalid in ["", " ", "\n", "UNKNOWN", "localhost"] {
            assert_eq!(
                SystemDeviceName::from_sources(Some(invalid), Some("Surface")).as_str(),
                "Surface"
            );
        }
        assert_eq!(
            SystemDeviceName::from_sources(None, None).as_str(),
            "CopyPaste device"
        );
        assert_eq!(
            SystemDeviceName::from_sources(Some("a\nb"), None).as_str(),
            "ab"
        );
        assert!(
            SystemDeviceName::from_sources(Some(&"📱".repeat(500)), None)
                .as_str()
                .len()
                <= 128
        );
    }
}
