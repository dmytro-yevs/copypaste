//! X11 selection-owner source attribution.
//!
//! The selection owner is the writer.  Foreground windows and titles are never
//! consulted: they describe the reader's current desktop state and may contain
//! user content.  All process paths remain transient too.

use std::io::Read;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use rustix::fs::{openat, Mode, OFlags, CWD};
use x11rb::protocol::xproto::{AtomEnum, ConnectionExt as _};

const MAX_WM_CLASS_BYTES: u32 = 512;
const MAX_DESKTOP_ENTRY_BYTES: u64 = 64 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SourceApp {
    pub(crate) id: String,
    pub(crate) name: String,
}

/// Resolve only evidence owned by the X11 selection writer. Absence is an
/// ordinary result: callers with exclusions then fail closed before TARGETS.
pub(crate) fn resolve_owner(
    connection: &x11rb::rust_connection::RustConnection,
    window: u32,
) -> Option<SourceApp> {
    let pid_atom = connection
        .intern_atom(false, b"_NET_WM_PID")
        .ok()?
        .reply()
        .ok()?
        .atom;
    let pid = connection
        .get_property(false, window, pid_atom, AtomEnum::CARDINAL, 0, 1)
        .ok()?
        .reply()
        .ok()?
        .value32()?
        .next()?;
    same_uid_regular_executable(pid)?;
    let class = connection
        .get_property(
            false,
            window,
            AtomEnum::WM_CLASS,
            AtomEnum::STRING,
            0,
            MAX_WM_CLASS_BYTES / 4,
        )
        .ok()?
        .reply()
        .ok()?
        .value;
    let class = wm_class(&class)?;
    let (id, name) = desktop_for_class(&class)?;
    Some(SourceApp { id, name })
}

/// Resolve a compositor-provided desktop application ID to its installed
/// desktop entry. The ID is evidence from the compositor; this only supplies
/// its display label and never derives an identity from desktop state.
pub(crate) fn resolve_desktop_id(id: &str) -> Option<SourceApp> {
    valid_desktop_id(id).then_some(())?;
    let (_, name) = desktop_for_class(id)?;
    Some(SourceApp {
        id: id.to_owned(),
        name,
    })
}

/// ICCCM WM_CLASS is `instance NUL class NUL`. The class (not the instance)
/// is the desktop-file convention and is usable only as a bounded package ID.
fn wm_class(bytes: &[u8]) -> Option<String> {
    let mut parts = bytes.split(|byte| *byte == 0);
    let _instance = parts.next()?;
    let class = std::str::from_utf8(parts.next()?).ok()?.trim();
    valid_desktop_id(class).then(|| class.to_owned())
}

fn same_uid_regular_executable(pid: u32) -> Option<()> {
    let process = std::fs::metadata(format!("/proc/{pid}")).ok()?;
    let current = std::fs::metadata("/proc/self").ok()?;
    let executable = std::fs::metadata(format!("/proc/{pid}/exe")).ok()?;
    (process.uid() == current.uid() && executable.is_file()).then_some(())
}

fn valid_desktop_id(value: &str) -> bool {
    (1..=255).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn desktop_for_class(class: &str) -> Option<(String, String)> {
    data_roots().into_iter().find_map(|root| {
        let applications = root.join("applications");
        let direct = applications.join(format!("{class}.desktop"));
        desktop_entry(&direct)
            .map(|name| (class.to_owned(), name))
            .or_else(|| {
                std::fs::read_dir(applications)
                    .ok()?
                    .take(512)
                    .find_map(|entry| {
                        let path = entry.ok()?.path();
                        let id = path.file_stem()?.to_str()?;
                        valid_desktop_id(id).then_some(())?;
                        let (name, startup) = desktop_entry_fields(
                            &String::from_utf8(read_bounded(&path, MAX_DESKTOP_ENTRY_BYTES)?)
                                .ok()?,
                        )?;
                        (startup.as_deref() == Some(class)).then_some((id.to_owned(), name))
                    })
            })
    })
}

fn data_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(home) = std::env::var_os("XDG_DATA_HOME") {
        let path = PathBuf::from(home);
        if path.is_absolute() {
            roots.push(path);
        }
    } else if let Some(home) = std::env::var_os("HOME") {
        roots.push(PathBuf::from(home).join(".local/share"));
    }
    let dirs =
        std::env::var_os("XDG_DATA_DIRS").unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    roots.extend(std::env::split_paths(&dirs).filter(|path| path.is_absolute()));
    roots
}

fn desktop_entry(path: &Path) -> Option<String> {
    desktop_entry_fields(&String::from_utf8(read_bounded(path, MAX_DESKTOP_ENTRY_BYTES)?).ok()?)
        .map(|(name, _)| name)
}
fn desktop_entry_fields(text: &str) -> Option<(String, Option<String>)> {
    let mut desktop = false;
    let mut name = None;
    let mut startup = None;
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') && line.ends_with(']') {
            desktop = line == "[Desktop Entry]";
            continue;
        }
        if !desktop || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key == "Name" {
            let value = value.trim();
            name = (!value.is_empty() && value.len() <= 255).then(|| value.to_owned());
        } else if key == "StartupWMClass" {
            let value = value.trim();
            startup = valid_desktop_id(value).then(|| value.to_owned());
        }
    }
    name.map(|name| (name, startup))
}

fn read_bounded(path: &Path, cap: u64) -> Option<Vec<u8>> {
    let file = std::fs::File::from(
        openat(CWD, path, OFlags::RDONLY | OFlags::NONBLOCK, Mode::empty()).ok()?,
    );
    let metadata = file.metadata().ok()?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > cap {
        return None;
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(cap.saturating_add(1))
        .read_to_end(&mut bytes)
        .ok()?;
    (bytes.len() as u64 <= cap).then_some(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wm_class_rejects_titles_paths_and_malformed_values() {
        assert_eq!(
            wm_class(b"editor\0org.example.Editor\0"),
            Some("org.example.Editor".into())
        );
        assert_eq!(wm_class(b"editor\0/opt/editor\0"), None);
        assert_eq!(wm_class(b"editor\0firefox\0"), Some("firefox".into()));
        assert_eq!(wm_class(b"editor"), None);
    }
    #[test]
    fn names_are_only_read_from_the_desktop_entry_group() {
        assert_eq!(
            desktop_entry_fields("Name=bad\n[Desktop Entry]\nName=Editor\n"),
            Some(("Editor".into(), None))
        );
    }
}
