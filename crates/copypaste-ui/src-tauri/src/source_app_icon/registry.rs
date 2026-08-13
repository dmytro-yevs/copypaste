//! `App Paths`: the image name an item carries, back to an executable path.
//!
//! The path is recovered transiently and dropped once the shell has answered
//! with an icon. Storing it would disclose the local user name (I-9).

use std::path::PathBuf;
use std::ptr;

use windows_sys::Win32::Foundation::{ERROR_MORE_DATA, ERROR_SUCCESS, MAX_PATH};
use windows_sys::Win32::System::Environment::ExpandEnvironmentStringsW;
use windows_sys::Win32::System::Registry::{
    RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE,
    KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY, REG_EXPAND_SZ, REG_SZ,
};

const APP_PATHS: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths";

pub(super) fn app_path(image_name: &str) -> Option<PathBuf> {
    lookup(APP_PATHS, image_name)
}

/// HKCU before HKLM, and each in the native view before either WOW64 view.
///
/// A per-user install (Vivaldi, VS Code) registers under HKCU only and must
/// win over a machine-wide one; a 32-bit installer on a 64-bit host writes
/// under `Wow6432Node`, which the native view does not see at all.
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
    let subkey = wide(&format!("{base}\\{image_name}"));
    search_order()
        .into_iter()
        .find_map(|(root, access)| read_app_path(root, &subkey, access))
}

fn read_app_path(root: HKEY, subkey: &[u16], access: u32) -> Option<PathBuf> {
    let (kind, value) = read_default_value(root, subkey, access)?;
    let path = PathBuf::from(executable_path(kind, &value)?);
    path.is_file().then_some(path)
}

/// Expansion is a property of the value's *kind*, not of its text. A `REG_SZ`
/// holding a literal `%SystemRoot%` is a path with a per-cent sign in it, and
/// expanding it anyway invents a path the registry did not name.
fn executable_path(kind: u32, value: &str) -> Option<String> {
    let expanded = if kind == REG_EXPAND_SZ {
        expand_env(value)
    } else {
        value.to_owned()
    };
    // Explorer's address bar and the shell both hand back quoted paths.
    let path = expanded.trim().trim_matches('"').trim();
    (!path.is_empty()).then(|| path.to_owned())
}

fn read_default_value(root: HKEY, subkey: &[u16], access: u32) -> Option<(u32, String)> {
    let key = OpenKey::open(root, subkey, access)?;
    // SAFETY: `key` is open for the whole of the call and closed by its guard.
    unsafe { read_open_default(key.0) }
}

struct OpenKey(HKEY);

impl OpenKey {
    fn open(root: HKEY, subkey: &[u16], access: u32) -> Option<Self> {
        let mut key: HKEY = ptr::null_mut();
        // SAFETY: `subkey` is NUL terminated by `wide`; `key` is written only
        // when the call succeeds.
        let status = unsafe { RegOpenKeyExW(root, subkey.as_ptr(), 0, access, &raw mut key) };
        (status == ERROR_SUCCESS && !key.is_null()).then_some(Self(key))
    }
}

impl Drop for OpenKey {
    fn drop(&mut self) {
        // SAFETY: the handle came from a successful `RegOpenKeyExW` and is
        // closed exactly once.
        unsafe { RegCloseKey(self.0) };
    }
}

unsafe fn read_open_default(key: HKEY) -> Option<(u32, String)> {
    let mut kind = 0u32;
    let mut buffer = vec![0u16; MAX_PATH as usize];
    let (status, bytes) = query(key, &mut buffer, &mut kind);
    let (status, bytes) = if status == ERROR_MORE_DATA {
        // `bytes` now holds the size the value needs; the spare unit leaves
        // room for a terminator the value may not carry.
        buffer = vec![0u16; bytes.div_ceil(2) as usize + 1];
        query(key, &mut buffer, &mut kind)
    } else {
        (status, bytes)
    };
    if status != ERROR_SUCCESS {
        return None;
    }
    decode_string(kind, &buffer, bytes).map(|value| (kind, value))
}

unsafe fn query(key: HKEY, buffer: &mut [u16], kind: &mut u32) -> (u32, u32) {
    let mut bytes = (buffer.len() * 2) as u32;
    let status = RegQueryValueExW(
        key,
        ptr::null(),
        ptr::null(),
        kind,
        buffer.as_mut_ptr().cast(),
        &raw mut bytes,
    );
    (status, bytes)
}

/// A registry string is not required to be terminated, and one that is must not
/// keep the NUL: `PathBuf::from("cmd.exe\0")` names no file on disk.
fn decode_string(kind: u32, buffer: &[u16], bytes: u32) -> Option<String> {
    if !matches!(kind, REG_SZ | REG_EXPAND_SZ) {
        return None;
    }
    let units = (bytes as usize / 2).min(buffer.len());
    let value = &buffer[..units];
    let value = match value.iter().position(|&unit| unit == 0) {
        Some(end) => &value[..end],
        None => value,
    };
    let value = String::from_utf16(value).ok()?;
    (!value.is_empty()).then_some(value)
}

fn expand_env(value: &str) -> String {
    let source = wide(value);
    // SAFETY: `source` is NUL terminated; the sizing call writes nothing.
    let needed = unsafe { ExpandEnvironmentStringsW(source.as_ptr(), ptr::null_mut(), 0) };
    if needed == 0 {
        return value.to_owned();
    }
    let mut buffer = vec![0u16; needed as usize];
    // SAFETY: `buffer` holds the count the sizing call asked for, in units.
    let written =
        unsafe { ExpandEnvironmentStringsW(source.as_ptr(), buffer.as_mut_ptr(), needed) };
    if written == 0 || written > needed {
        return value.to_owned();
    }
    let units = (written as usize).saturating_sub(1);
    String::from_utf16(&buffer[..units]).unwrap_or_else(|_| value.to_owned())
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU32, Ordering};

    use windows_sys::Win32::System::Registry::{
        RegCreateKeyExW, RegDeleteTreeW, RegSetValueExW, KEY_WRITE, REG_DWORD,
        REG_OPTION_NON_VOLATILE,
    };

    use super::*;

    fn system32(name: &str) -> String {
        format!(
            "{}\\System32\\{name}",
            std::env::var("SystemRoot").expect("SystemRoot is always set on Windows")
        )
    }

    /// An isolated HKCU subtree, deleted when the test ends.
    ///
    /// Deliberately not the real `App Paths` key: a test that writes there
    /// leaves the user's shell carrying entries for executables that never
    /// existed if it dies before its cleanup runs.
    struct Hive {
        base: String,
    }

    impl Hive {
        fn new(label: &str) -> Self {
            static NEXT: AtomicU32 = AtomicU32::new(0);
            let unique = NEXT.fetch_add(1, Ordering::Relaxed);
            // One key, not a shared container with a child: deleting a subkey
            // leaves its parent behind, and an empty `CopyPaste-icon-test` key
            // in the user's hive is still litter from a test.
            let base = format!(
                "SOFTWARE\\CopyPaste-icon-test-{}-{label}-{unique}",
                std::process::id()
            );
            Self { base }
        }

        fn set(&self, image_name: &str, kind: u32, value: &str) {
            self.write(image_name, kind, &wide(value));
        }

        /// Without the terminator `wide` would add, which is a value shape the
        /// registry permits and the decoder must survive.
        fn set_unterminated(&self, image_name: &str, value: &str) {
            self.write(
                image_name,
                REG_SZ,
                &value.encode_utf16().collect::<Vec<_>>(),
            );
        }

        fn set_dword(&self, image_name: &str, value: u32) {
            let key = self.create(image_name);
            let bytes = value.to_le_bytes();
            let status =
                unsafe { RegSetValueExW(key.0, ptr::null(), 0, REG_DWORD, bytes.as_ptr(), 4) };
            assert_eq!(status, ERROR_SUCCESS, "the test could not write a DWORD");
        }

        fn write(&self, image_name: &str, kind: u32, units: &[u16]) {
            let key = self.create(image_name);
            let status = unsafe {
                RegSetValueExW(
                    key.0,
                    ptr::null(),
                    0,
                    kind,
                    units.as_ptr().cast(),
                    (units.len() * 2) as u32,
                )
            };
            assert_eq!(status, ERROR_SUCCESS, "the test could not write a value");
        }

        fn create(&self, image_name: &str) -> OpenKey {
            let subkey = wide(&format!("{}\\{image_name}", self.base));
            let mut key: HKEY = ptr::null_mut();
            let status = unsafe {
                RegCreateKeyExW(
                    HKEY_CURRENT_USER,
                    subkey.as_ptr(),
                    0,
                    ptr::null(),
                    REG_OPTION_NON_VOLATILE,
                    KEY_WRITE,
                    ptr::null(),
                    &raw mut key,
                    ptr::null_mut(),
                )
            };
            assert_eq!(status, ERROR_SUCCESS, "the test could not create a key");
            OpenKey(key)
        }

        fn find(&self, image_name: &str) -> Option<PathBuf> {
            lookup(&self.base, image_name)
        }

        fn exists(&self) -> bool {
            OpenKey::open(HKEY_CURRENT_USER, &wide(&self.base), KEY_READ).is_some()
        }
    }

    impl Drop for Hive {
        fn drop(&mut self) {
            let subkey = wide(&self.base);
            unsafe { RegDeleteTreeW(HKEY_CURRENT_USER, subkey.as_ptr()) };
        }
    }

    #[test]
    fn hkcu_is_searched_before_hklm_and_both_wow64_views_are_probed() {
        let order = search_order();
        assert_eq!(order.len(), 6);
        assert!(
            order[..3]
                .iter()
                .all(|&(root, _)| root == HKEY_CURRENT_USER),
            "a per-user install must win over a machine-wide one"
        );
        assert!(order[3..]
            .iter()
            .all(|&(root, _)| root == HKEY_LOCAL_MACHINE));
        for root in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
            let views: Vec<u32> = order
                .iter()
                .filter(|&&(candidate, _)| candidate == root)
                .map(|&(_, access)| access)
                .collect();
            assert_eq!(
                views,
                vec![
                    KEY_READ,
                    KEY_READ | KEY_WOW64_32KEY,
                    KEY_READ | KEY_WOW64_64KEY
                ]
            );
        }
    }

    #[test]
    fn only_reg_expand_sz_is_expanded() {
        let literal = r"%SystemRoot%\System32\cmd.exe";
        assert_eq!(
            executable_path(REG_SZ, literal).as_deref(),
            Some(literal),
            "a REG_SZ value is a path, per-cent signs and all"
        );
        let expanded = executable_path(REG_EXPAND_SZ, literal).expect("expanded");
        assert_ne!(expanded, literal);
        assert!(expanded.to_lowercase().ends_with(r"\system32\cmd.exe"));
        assert!(!expanded.contains('%'));
    }

    #[test]
    fn quotes_and_padding_are_stripped_and_an_empty_value_names_nothing() {
        assert_eq!(
            executable_path(REG_SZ, "  \"C:\\Apps\\chrome.exe\" ").as_deref(),
            Some(r"C:\Apps\chrome.exe")
        );
        assert!(executable_path(REG_SZ, "").is_none());
        assert!(executable_path(REG_SZ, "   ").is_none());
        assert!(executable_path(REG_SZ, "\"\"").is_none());
    }

    #[test]
    fn a_value_is_decoded_whether_or_not_it_is_terminated() {
        let terminated = wide("cmd.exe");
        assert_eq!(
            decode_string(REG_SZ, &terminated, (terminated.len() * 2) as u32).as_deref(),
            Some("cmd.exe")
        );
        let unterminated: Vec<u16> = "cmd.exe".encode_utf16().collect();
        assert_eq!(
            decode_string(REG_SZ, &unterminated, (unterminated.len() * 2) as u32).as_deref(),
            Some("cmd.exe")
        );
        assert_eq!(
            decode_string(REG_EXPAND_SZ, &terminated, (terminated.len() * 2) as u32).as_deref(),
            Some("cmd.exe")
        );
    }

    #[test]
    fn a_decoder_refuses_a_wrong_kind_and_survives_a_bad_length() {
        let value = wide("cmd.exe");
        assert!(decode_string(REG_DWORD, &value, 8).is_none());
        assert!(decode_string(REG_SZ, &value, 0).is_none());
        assert!(
            decode_string(REG_SZ, &value, u32::MAX).is_some(),
            "a length past the buffer must clamp, not panic"
        );
        assert_eq!(
            decode_string(REG_SZ, &value, 7).as_deref(),
            Some("cmd"),
            "an odd byte count rounds down to whole UTF-16 units"
        );
        assert!(decode_string(REG_SZ, &[0, 0, 0], 6).is_none());
    }

    #[test]
    fn a_reg_sz_value_resolves_to_an_executable() {
        let hive = Hive::new("reg-sz");
        hive.set("copypaste-fake-a.exe", REG_SZ, &system32("cmd.exe"));
        assert_eq!(
            hive.find("copypaste-fake-a.exe"),
            Some(PathBuf::from(system32("cmd.exe")))
        );
    }

    #[test]
    fn a_reg_expand_sz_value_is_expanded_and_a_reg_sz_one_is_not() {
        let hive = Hive::new("expansion");
        let literal = r"%SystemRoot%\System32\cmd.exe";
        hive.set("copypaste-fake-expand.exe", REG_EXPAND_SZ, literal);
        hive.set("copypaste-fake-literal.exe", REG_SZ, literal);

        assert_eq!(
            hive.find("copypaste-fake-expand.exe"),
            Some(PathBuf::from(system32("cmd.exe")))
        );
        assert!(
            hive.find("copypaste-fake-literal.exe").is_none(),
            "a REG_SZ value was expanded; the kind is what decides"
        );
    }

    #[test]
    fn a_quoted_value_resolves_and_an_unterminated_one_does_too() {
        let hive = Hive::new("shapes");
        hive.set(
            "copypaste-fake-quoted.exe",
            REG_SZ,
            &format!("\"{}\"", system32("cmd.exe")),
        );
        hive.set_unterminated("copypaste-fake-raw.exe", &system32("cmd.exe"));
        assert_eq!(
            hive.find("copypaste-fake-quoted.exe"),
            Some(PathBuf::from(system32("cmd.exe")))
        );
        assert_eq!(
            hive.find("copypaste-fake-raw.exe"),
            Some(PathBuf::from(system32("cmd.exe")))
        );
    }

    /// The initial buffer is `MAX_PATH` units, so a longer value returns
    /// `ERROR_MORE_DATA` and is only readable on the second attempt. A retry
    /// that kept the first buffer would truncate the path and resolve nothing.
    #[test]
    fn a_value_longer_than_the_first_buffer_is_read_whole() {
        let hive = Hive::new("more-data");
        let padded = format!("{}{}", " ".repeat(600), system32("cmd.exe"));
        assert!(padded.len() > MAX_PATH as usize);
        hive.set("copypaste-fake-long.exe", REG_SZ, &padded);
        assert_eq!(
            hive.find("copypaste-fake-long.exe"),
            Some(PathBuf::from(system32("cmd.exe")))
        );
    }

    #[test]
    fn a_non_string_value_a_missing_key_and_a_missing_file_resolve_to_nothing() {
        let hive = Hive::new("refusals");
        hive.set_dword("copypaste-fake-dword.exe", 1);
        hive.set(
            "copypaste-fake-absent.exe",
            REG_SZ,
            &system32("copypaste-no-such-file.exe"),
        );
        assert!(hive.find("copypaste-fake-dword.exe").is_none());
        assert!(hive.find("copypaste-fake-absent.exe").is_none());
        assert!(hive.find("copypaste-fake-never-written.exe").is_none());
    }

    /// HKCU is not WOW64-redirected outside `Software\Classes`, so all three
    /// views must answer for a value written once — a view flag that broke the
    /// read would silently cost every 32-bit registration.
    #[test]
    fn every_view_reads_a_value_written_to_hkcu() {
        let hive = Hive::new("views");
        hive.set("copypaste-fake-view.exe", REG_SZ, &system32("cmd.exe"));
        let subkey = wide(&format!("{}\\copypaste-fake-view.exe", hive.base));
        for access in [
            KEY_READ,
            KEY_READ | KEY_WOW64_32KEY,
            KEY_READ | KEY_WOW64_64KEY,
        ] {
            assert!(
                read_app_path(HKEY_CURRENT_USER, &subkey, access).is_some(),
                "view {access:#x} could not read the value"
            );
        }
    }

    #[test]
    fn the_test_hive_is_removed_when_it_is_dropped() {
        let base = {
            let hive = Hive::new("cleanup");
            hive.set("copypaste-fake-cleanup.exe", REG_SZ, &system32("cmd.exe"));
            assert!(hive.exists());
            Hive {
                base: hive.base.clone(),
            }
        };
        // `base` is a second guard over the same path, so its own drop is a
        // no-op after this assertion.
        assert!(
            !base.exists(),
            "the isolated test key outlived the test that made it"
        );
    }
}
