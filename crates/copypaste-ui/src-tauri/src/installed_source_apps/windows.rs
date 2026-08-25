use std::collections::BTreeSet;

use winreg::enums::{
    HKEY_CLASSES_ROOT, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY,
    KEY_WOW64_64KEY,
};
use winreg::RegKey;

use super::{finish, CatalogUnavailable, InstalledSourceApp, Result};

const APP_PATHS: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths";
const APPLICATIONS: &str = r"Applications";
const OWN_IMAGES: &[&str] = &["copypaste-ui.exe", "copypaste.exe", "copypaste-daemon.exe"];

pub(super) fn list() -> Result<Vec<InstalledSourceApp>> {
    let mut names = BTreeSet::new();
    let mut readable_catalog = false;
    for root in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
        let root = RegKey::predef(root);
        for view in [
            KEY_READ,
            KEY_READ | KEY_WOW64_32KEY,
            KEY_READ | KEY_WOW64_64KEY,
        ] {
            let Ok(paths) = root.open_subkey_with_flags(APP_PATHS, view) else {
                continue;
            };
            readable_catalog = true;
            names.extend(paths.enum_keys().filter_map(std::result::Result::ok));
        }
    }
    if !readable_catalog {
        return Err(CatalogUnavailable);
    }

    let apps = names
        .into_iter()
        .filter(|name| is_application_image(name))
        .filter(|name| !shell_suppresses(name))
        .filter_map(|name| {
            crate::source_app_icon::registered_executable(&name)?;
            let label = friendly_name(&name).unwrap_or_else(|| image_label(&name));
            Some(InstalledSourceApp { id: name, label })
        })
        .collect();
    Ok(finish(apps, true))
}

fn is_application_image(name: &str) -> bool {
    name.get(name.len().saturating_sub(4)..)
        .is_some_and(|extension| extension.eq_ignore_ascii_case(".exe"))
        && !OWN_IMAGES.iter().any(|own| name.eq_ignore_ascii_case(own))
        && !name.contains(['\\', '/', ':'])
        && !name.bytes().any(|byte| byte.is_ascii_control())
}

fn applications_key(name: &str) -> Option<RegKey> {
    RegKey::predef(HKEY_CLASSES_ROOT)
        .open_subkey_with_flags(format!(r"{APPLICATIONS}\{name}"), KEY_READ)
        .ok()
}

fn shell_suppresses(name: &str) -> bool {
    applications_key(name).is_some_and(|key| {
        key.get_raw_value("IsHostApp").is_ok() || key.get_raw_value("NoStartPage").is_ok()
    })
}

fn friendly_name(name: &str) -> Option<String> {
    let value: String = applications_key(name)?.get_value("FriendlyAppName").ok()?;
    let value = value.trim();
    (!value.is_empty() && !value.starts_with('@')).then(|| value.to_owned())
}

fn image_label(name: &str) -> String {
    name.strip_suffix(".exe")
        .or_else(|| name.strip_suffix(".EXE"))
        .unwrap_or(name)
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_stable_application_images_enter_the_catalogue() {
        assert!(is_application_image("Writer.exe"));
        assert!(is_application_image("Proton Pass.exe"));
        assert!(!is_application_image("setup.msi"));
        assert!(!is_application_image(r"C:\Apps\Writer.exe"));
        assert!(!is_application_image("copypaste-ui.exe"));
    }
}
