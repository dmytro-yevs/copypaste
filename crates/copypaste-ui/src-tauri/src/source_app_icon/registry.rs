//! `App Paths`: the image name an item carries, back to an executable path.
//!
//! The path is recovered transiently and dropped after icon extraction. It is
//! never stored or sent to the WebView because it can disclose a username.

use std::collections::BTreeSet;
use std::path::PathBuf;

use winreg::enums::{
    HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY,
    REG_EXPAND_SZ,
};
use winreg::types::FromRegValue;
use winreg::{RegKey, RegValue, HKEY};

const APP_PATHS: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths";

pub(crate) fn image_names() -> Option<BTreeSet<String>> {
    image_names_at(APP_PATHS)
}

fn image_names_at(base: &str) -> Option<BTreeSet<String>> {
    let mut names = BTreeSet::new();
    let mut readable = false;
    for (root, access) in search_order() {
        let Ok(paths) = RegKey::predef(root).open_subkey_with_flags(base, access) else {
            continue;
        };
        readable = true;
        names.extend(paths.enum_keys().filter_map(Result::ok));
    }
    readable.then_some(names)
}

/// App Paths first, then System32: Windows tools such as `cmd.exe` register no
/// App Path but still have a shell icon.
pub(crate) fn executable(image_name: &str) -> Option<PathBuf> {
    if let Some(path) = lookup(APP_PATHS, image_name) {
        return Some(path);
    }
    let root = std::env::var("SystemRoot").ok()?;
    let path = PathBuf::from(root).join("System32").join(image_name);
    path.is_file().then_some(path)
}

/// Per-user installs win over machine-wide installs; each hive's native view
/// wins before the two explicit WOW64 views.
fn search_order() -> [(HKEY, u32); 6] {
    [
        (HKEY_CURRENT_USER, KEY_READ),
        (HKEY_CURRENT_USER, KEY_READ | KEY_WOW64_32KEY),
        (HKEY_CURRENT_USER, KEY_READ | KEY_WOW64_64KEY),
        (HKEY_LOCAL_MACHINE, KEY_READ),
        (HKEY_LOCAL_MACHINE, KEY_READ | KEY_WOW64_32KEY),
        (HKEY_LOCAL_MACHINE, KEY_READ | KEY_WOW64_64KEY),
    ]
}

fn lookup(base: &str, image_name: &str) -> Option<PathBuf> {
    let subkey = format!(r"{base}\{image_name}");
    search_order()
        .into_iter()
        .find_map(|(root, access)| read_app_path(root, &subkey, access))
}

fn read_app_path(root: HKEY, subkey: &str, access: u32) -> Option<PathBuf> {
    let key = RegKey::predef(root)
        .open_subkey_with_flags(subkey, access)
        .ok()?;
    let value = key.get_raw_value("").ok()?;
    let path = PathBuf::from(executable_path(&value)?);
    path.is_file().then_some(path)
}

/// Expansion follows the registry value kind. Expanding a literal `REG_SZ`
/// containing percent signs would invent a path the registry did not name.
fn executable_path(value: &RegValue) -> Option<String> {
    let decoded = String::from_reg_value(value).ok()?;
    let expanded = if value.vtype == REG_EXPAND_SZ {
        winsafe::ExpandEnvironmentStrings(&decoded).unwrap_or(decoded)
    } else {
        decoded
    };
    let path = expanded.trim().trim_matches('"').trim();
    (!path.is_empty()).then(|| path.to_owned())
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU32, Ordering};

    use winreg::enums::{KEY_WRITE, REG_DWORD, REG_SZ};
    use winreg::RegValue;

    use super::*;

    fn system32(name: &str) -> String {
        format!(
            "{}\\System32\\{name}",
            std::env::var("SystemRoot").expect("SystemRoot is always set on Windows")
        )
    }

    fn string_value(kind: winreg::enums::RegType, value: &str, terminated: bool) -> RegValue {
        let mut units = value.encode_utf16().collect::<Vec<_>>();
        if terminated {
            units.push(0);
        }
        RegValue {
            bytes: units.into_iter().flat_map(u16::to_le_bytes).collect(),
            vtype: kind,
        }
    }

    /// Isolated from the real App Paths key so a failed test cannot leave a
    /// fake application registered with the user's shell.
    struct Hive {
        base: String,
    }

    impl Hive {
        fn new(label: &str) -> Self {
            static NEXT: AtomicU32 = AtomicU32::new(0);
            let unique = NEXT.fetch_add(1, Ordering::Relaxed);
            Self {
                base: format!(
                    "SOFTWARE\\CopyPaste-icon-test-{}-{label}-{unique}",
                    std::process::id()
                ),
            }
        }

        fn set(&self, image_name: &str, kind: winreg::enums::RegType, value: &str) {
            self.write(image_name, string_value(kind, value, true));
        }

        fn set_unterminated(&self, image_name: &str, value: &str) {
            self.write(image_name, string_value(REG_SZ, value, false));
        }

        fn set_dword(&self, image_name: &str, value: u32) {
            self.write(
                image_name,
                RegValue {
                    bytes: value.to_le_bytes().to_vec(),
                    vtype: REG_DWORD,
                },
            );
        }

        fn write(&self, image_name: &str, value: RegValue) {
            let (key, _) = RegKey::predef(HKEY_CURRENT_USER)
                .create_subkey_with_flags(format!(r"{}\{image_name}", self.base), KEY_WRITE)
                .expect("the test could not create a registry key");
            key.set_raw_value("", &value)
                .expect("the test could not write a registry value");
        }

        fn find(&self, image_name: &str) -> Option<PathBuf> {
            lookup(&self.base, image_name)
        }

        fn names(&self) -> Option<BTreeSet<String>> {
            image_names_at(&self.base)
        }

        fn exists(&self) -> bool {
            RegKey::predef(HKEY_CURRENT_USER)
                .open_subkey_with_flags(&self.base, KEY_READ)
                .is_ok()
        }
    }

    impl Drop for Hive {
        fn drop(&mut self) {
            let _ = RegKey::predef(HKEY_CURRENT_USER).delete_subkey_all(&self.base);
        }
    }

    #[test]
    fn hkcu_is_searched_before_hklm_and_all_registry_views_are_probed() {
        let order = search_order();
        assert!(order[..3]
            .iter()
            .all(|&(root, _)| root == HKEY_CURRENT_USER));
        assert!(order[3..]
            .iter()
            .all(|&(root, _)| root == HKEY_LOCAL_MACHINE));
        for root in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
            assert_eq!(
                order
                    .iter()
                    .filter(|&&(candidate, _)| candidate == root)
                    .map(|&(_, access)| access)
                    .collect::<Vec<_>>(),
                vec![
                    KEY_READ,
                    KEY_READ | KEY_WOW64_32KEY,
                    KEY_READ | KEY_WOW64_64KEY
                ]
            );
        }
    }

    #[test]
    fn catalogue_and_resolver_share_the_same_registration() {
        let hive = Hive::new("catalogue");
        hive.set("Writer.exe", REG_SZ, &system32("cmd.exe"));
        hive.set("Reader.exe", REG_SZ, &system32("notepad.exe"));

        let names = hive.names().expect("the catalogue is readable");
        assert!(names.contains("Writer.exe"));
        assert!(names.contains("Reader.exe"));
        assert_eq!(
            hive.find("Writer.exe"),
            Some(PathBuf::from(system32("cmd.exe")))
        );
    }

    #[test]
    fn path_normalization_is_kind_aware_and_strips_shell_quotes() {
        let literal = r"%SystemRoot%\System32\cmd.exe";
        assert_eq!(
            executable_path(&string_value(REG_SZ, literal, true)).as_deref(),
            Some(literal)
        );
        let expanded = executable_path(&string_value(REG_EXPAND_SZ, literal, true))
            .expect("REG_EXPAND_SZ expands");
        assert!(expanded.to_lowercase().ends_with(r"\system32\cmd.exe"));
        assert!(!expanded.contains('%'));
        assert_eq!(
            executable_path(&string_value(REG_SZ, "  \"C:\\Apps\\Writer.exe\" ", true)).as_deref(),
            Some(r"C:\Apps\Writer.exe")
        );
        assert!(executable_path(&string_value(REG_SZ, "\"\"", true)).is_none());
    }

    #[test]
    fn terminated_and_unterminated_values_both_resolve() {
        let hive = Hive::new("termination");
        hive.set("terminated.exe", REG_SZ, &system32("cmd.exe"));
        hive.set_unterminated("unterminated.exe", &system32("cmd.exe"));
        assert!(hive.find("terminated.exe").is_some());
        assert!(hive.find("unterminated.exe").is_some());
    }

    #[test]
    fn a_long_value_is_read_whole_by_winreg() {
        let hive = Hive::new("long-value");
        let padded = format!("{}{}", " ".repeat(3_000), system32("cmd.exe"));
        hive.set("long.exe", REG_SZ, &padded);
        assert_eq!(
            hive.find("long.exe"),
            Some(PathBuf::from(system32("cmd.exe")))
        );
    }

    #[test]
    fn a_non_string_missing_key_or_missing_file_resolves_to_nothing() {
        let hive = Hive::new("refusals");
        hive.set_dword("number.exe", 1);
        hive.set(
            "absent.exe",
            REG_SZ,
            &system32("copypaste-no-such-file.exe"),
        );
        assert!(hive.find("number.exe").is_none());
        assert!(hive.find("absent.exe").is_none());
        assert!(hive.find("never-written.exe").is_none());
    }

    #[test]
    fn every_view_reads_an_hkcu_registration() {
        let hive = Hive::new("views");
        hive.set("view.exe", REG_SZ, &system32("cmd.exe"));
        let subkey = format!(r"{}\view.exe", hive.base);
        for access in [
            KEY_READ,
            KEY_READ | KEY_WOW64_32KEY,
            KEY_READ | KEY_WOW64_64KEY,
        ] {
            assert!(read_app_path(HKEY_CURRENT_USER, &subkey, access).is_some());
        }
    }

    #[test]
    fn the_isolated_hive_is_removed_on_drop() {
        let base;
        {
            let hive = Hive::new("cleanup");
            hive.set("cleanup.exe", REG_SZ, &system32("cmd.exe"));
            assert!(hive.exists());
            base = hive.base.clone();
        }
        assert!(RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey_with_flags(base, KEY_READ)
            .is_err());
    }
}
