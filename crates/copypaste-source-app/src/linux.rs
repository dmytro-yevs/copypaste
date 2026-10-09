//! Freedesktop desktop-entry icon lookup for Linux source applications.
//!
//! A clipboard owner may only provide a stable desktop ID.  This module turns
//! that ID into the existing bounded [`AppIcon`](super::AppIcon) representation
//! without executing a desktop-entry `Exec` value or invoking a shell tool.

use std::path::{Path, PathBuf};

use super::{normalize_png, valid_package_id, AppIcon};

const MAX_DESKTOP_ENTRY_BYTES: u64 = 64 * 1024;

#[cfg_attr(test, allow(dead_code))]
pub(super) fn resolve(app_id: &str) -> Option<AppIcon> {
    let roots = data_roots();
    resolve_from_roots(app_id, &roots)
}

fn resolve_from_roots(app_id: &str, roots: &[PathBuf]) -> Option<AppIcon> {
    // Linux desktop IDs use a `.desktop` filename. Requiring the same bounded
    // ID vocabulary that storage and cache keys use makes a path traversal
    // impossible before any filesystem access.
    if !valid_package_id(app_id) {
        return None;
    }
    let desktop = roots
        .iter()
        .map(|root| root.join("applications").join(format!("{app_id}.desktop")))
        .find(|path| is_small_regular_file(path))?;
    let icon = desktop_icon(&desktop)?;

    if Path::new(&icon).is_absolute() {
        return read_png(Path::new(&icon));
    }
    if !valid_icon_name(&icon) {
        return None;
    }

    // The icon-theme specification requires hicolor as the fallback theme.
    // SVG needs a renderer and is deliberately not rasterized here: keeping the
    // source-icon byte/decoder limits in one place matters more than inventing
    // a second image pipeline.
    roots.iter().find_map(|root| {
        ["128x128", "96x96", "64x64", "48x48", "32x32", "24x24"]
            .iter()
            .find_map(|size| {
                read_png(
                    &root
                        .join("icons/hicolor")
                        .join(size)
                        .join("apps")
                        .join(format!("{icon}.png")),
                )
            })
            .or_else(|| read_png(&root.join("pixmaps").join(format!("{icon}.png"))))
    })
}

#[cfg_attr(test, allow(dead_code))]
fn data_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(home) = std::env::var_os("XDG_DATA_HOME") {
        let home = PathBuf::from(home);
        if home.is_absolute() {
            roots.push(home);
        }
    } else if let Some(home) = std::env::var_os("HOME") {
        roots.push(PathBuf::from(home).join(".local/share"));
    }

    let dirs =
        std::env::var_os("XDG_DATA_DIRS").unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    roots.extend(
        std::env::split_paths(&dirs)
            .filter(|path| path.is_absolute())
            .filter(|path| !path.as_os_str().is_empty()),
    );
    roots
}

fn is_small_regular_file(path: &Path) -> bool {
    path.metadata()
        .ok()
        .is_some_and(|metadata| metadata.is_file() && metadata.len() <= MAX_DESKTOP_ENTRY_BYTES)
}

fn desktop_icon(path: &Path) -> Option<String> {
    let entry = std::fs::read_to_string(path).ok()?;
    let mut in_desktop_entry = false;
    for raw_line in entry.lines() {
        let line = raw_line.trim();
        if line.starts_with('[') && line.ends_with(']') {
            in_desktop_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_desktop_entry || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key == "Icon" {
            let icon = value.trim();
            return (!icon.is_empty() && icon.len() <= 255).then(|| icon.to_owned());
        }
    }
    None
}

fn valid_icon_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 255
        && !value.contains(['/', '\\'])
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn read_png(path: &Path) -> Option<AppIcon> {
    let metadata = path.metadata().ok()?;
    if !metadata.is_file()
        || metadata.len() == 0
        || metadata.len() > super::MAX_SOURCE_ICON_BYTES as u64
    {
        return None;
    }
    normalize_png(std::fs::read(path).ok()?)
}

#[cfg(test)]
mod tests {
    use image::{DynamicImage, ImageBuffer, ImageFormat, Rgba};

    use super::*;

    fn png() -> Vec<u8> {
        let image = ImageBuffer::from_pixel(2, 3, Rgba([0x24u8, 0x65, 0xa8, 0xff]));
        let mut bytes = Vec::new();
        DynamicImage::ImageRgba8(image)
            .write_to(&mut std::io::Cursor::new(&mut bytes), ImageFormat::Png)
            .unwrap();
        bytes
    }

    #[test]
    fn resolves_a_png_from_hicolor_for_a_desktop_id() {
        let root = tempfile::tempdir().unwrap();
        let applications = root.path().join("applications");
        std::fs::create_dir_all(&applications).unwrap();
        std::fs::write(
            applications.join("org.example.Writer.desktop"),
            "[Desktop Entry]\nName=Writer\nIcon=writer\n",
        )
        .unwrap();
        let icons = root.path().join("icons/hicolor/64x64/apps");
        std::fs::create_dir_all(&icons).unwrap();
        std::fs::write(icons.join("writer.png"), png()).unwrap();

        let icon = resolve_from_roots("org.example.Writer", &[root.path().into()]).unwrap();
        assert_eq!((icon.width, icon.height), (2, 3));
    }

    #[test]
    fn desktop_icon_never_turns_an_icon_name_into_a_path() {
        let root = tempfile::tempdir().unwrap();
        let applications = root.path().join("applications");
        std::fs::create_dir_all(&applications).unwrap();
        std::fs::write(
            applications.join("org.example.Writer.desktop"),
            "[Desktop Entry]\nIcon=../../not-an-icon\n",
        )
        .unwrap();

        assert!(resolve_from_roots("org.example.Writer", &[root.path().into()]).is_none());
    }
}
