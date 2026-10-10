//! Bounded Freedesktop icon and label lookup for Linux source applications.

use std::collections::HashSet;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use resvg::{tiny_skia, usvg};

use super::{
    normalize_png, valid_package_id, AppIcon, SourceAppLabel, MAX_SOURCE_ICON_BYTES,
    MAX_SOURCE_ICON_EDGE,
};

const MAX_DESKTOP_ENTRY_BYTES: u64 = 64 * 1024;
const MAX_THEME_DEPTH: usize = 16;
const FALLBACK_DIRECTORIES: &[&str] = &[
    "256x256/apps",
    "128x128/apps",
    "96x96/apps",
    "64x64/apps",
    "48x48/apps",
    "32x32/apps",
    "24x24/apps",
    "scalable/apps",
];

#[cfg_attr(test, allow(dead_code))]
pub(super) fn resolve(app_id: &str) -> Option<AppIcon> {
    let roots = data_roots();
    resolve_from_roots(app_id, &roots)
}

#[cfg_attr(test, allow(dead_code))]
pub(super) fn source_label(app_id: &str) -> Option<SourceAppLabel> {
    source_label_from_roots(app_id, &data_roots())
}

fn resolve_from_roots(app_id: &str, roots: &[PathBuf]) -> Option<AppIcon> {
    if !valid_package_id(app_id) {
        return None;
    }
    let icon = desktop_value(app_id, roots, "Icon")?;
    if Path::new(&icon).is_absolute() {
        return read_icon(Path::new(&icon));
    }
    if !valid_icon_name(&icon) {
        return None;
    }
    let icon_roots = icon_roots(roots);
    let theme = selected_theme();
    find_theme_icon(&icon, &theme, &icon_roots, &mut HashSet::new(), 0)
        .or_else(|| {
            (theme != "hicolor")
                .then(|| find_theme_icon(&icon, "hicolor", &icon_roots, &mut HashSet::new(), 0))?
        })
        .or_else(|| {
            roots.iter().find_map(|root| {
                read_icon(&root.join("pixmaps").join(format!("{icon}.png")))
                    .or_else(|| read_icon(&root.join("pixmaps").join(format!("{icon}.svg"))))
            })
        })
}

fn source_label_from_roots(app_id: &str, roots: &[PathBuf]) -> Option<SourceAppLabel> {
    valid_linux_app_id(app_id).then_some(())?;
    Some(SourceAppLabel(
        desktop_value(app_id, roots, "Name").unwrap_or_else(|| app_id.to_owned()),
    ))
}

fn selected_theme() -> String {
    std::env::var("GTK_THEME")
        .ok()
        .and_then(|value| value.split(':').next().map(str::to_owned))
        .filter(|theme| valid_theme_name(theme))
        .unwrap_or_else(|| "hicolor".into())
}

fn find_theme_icon(
    icon: &str,
    theme: &str,
    roots: &[PathBuf],
    visited: &mut HashSet<String>,
    depth: usize,
) -> Option<AppIcon> {
    if depth >= MAX_THEME_DEPTH || !valid_theme_name(theme) || !visited.insert(theme.into()) {
        return None;
    }
    let index = theme_index(theme, roots);
    let directories: Vec<&str> = index
        .as_ref()
        .map(|index| index.directories.iter().map(String::as_str).collect())
        .unwrap_or_else(|| FALLBACK_DIRECTORIES.to_vec());
    for directory in directories {
        for root in roots {
            for extension in ["png", "svg"] {
                if let Some(icon) = read_icon(
                    &root
                        .join(theme)
                        .join(directory)
                        .join(format!("{icon}.{extension}")),
                ) {
                    return Some(icon);
                }
            }
        }
    }
    index?
        .inherits
        .into_iter()
        .find_map(|parent| find_theme_icon(icon, &parent, roots, visited, depth + 1))
}

struct ThemeIndex {
    directories: Vec<String>,
    inherits: Vec<String>,
}

fn theme_index(theme: &str, roots: &[PathBuf]) -> Option<ThemeIndex> {
    roots.iter().find_map(|root| {
        let value = String::from_utf8(read_bounded(
            &root.join(theme).join("index.theme"),
            MAX_DESKTOP_ENTRY_BYTES,
        )?)
        .ok()?;
        let mut in_theme = false;
        let mut directories = Vec::new();
        let mut inherits = Vec::new();
        for line in value.lines().map(str::trim) {
            if line.starts_with('[') && line.ends_with(']') {
                in_theme = line == "[Icon Theme]";
                continue;
            }
            if !in_theme || line.starts_with(['#', ';']) {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            match key {
                "Directories" | "ScaledDirectories" => directories.extend(
                    value
                        .split(',')
                        .map(str::trim)
                        .filter(|value| valid_theme_path(value))
                        .map(str::to_owned),
                ),
                "Inherits" => inherits.extend(
                    value
                        .split(',')
                        .map(str::trim)
                        .filter(|value| valid_theme_name(value))
                        .map(str::to_owned),
                ),
                _ => {}
            }
        }
        (!directories.is_empty()).then_some(ThemeIndex {
            directories,
            inherits,
        })
    })
}

fn icon_roots(data_roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(home) = std::env::var_os("HOME") {
        let path = PathBuf::from(home).join(".icons");
        if path.is_absolute() {
            roots.push(path);
        }
    }
    roots.extend(data_roots.iter().map(|root| root.join("icons")));
    roots
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
    roots.extend(std::env::split_paths(&dirs).filter(|path| path.is_absolute()));
    roots
}

fn desktop_value(app_id: &str, roots: &[PathBuf], key: &str) -> Option<String> {
    roots.iter().find_map(|root| {
        desktop_value_from_path(
            &root.join("applications").join(format!("{app_id}.desktop")),
            key,
        )
    })
}

fn desktop_value_from_path(path: &Path, key: &str) -> Option<String> {
    let entry = String::from_utf8(read_bounded(path, MAX_DESKTOP_ENTRY_BYTES)?).ok()?;
    let mut in_desktop_entry = false;
    for raw_line in entry.lines() {
        let line = raw_line.trim();
        if line.starts_with('[') && line.ends_with(']') {
            in_desktop_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_desktop_entry || line.starts_with(['#', ';']) {
            continue;
        }
        let Some((candidate, value)) = line.split_once('=') else {
            continue;
        };
        if candidate == key {
            let value = value.trim();
            return (!value.is_empty() && value.len() <= 255).then(|| value.to_owned());
        }
    }
    None
}

fn valid_linux_app_id(value: &str) -> bool {
    (1..=255).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn valid_icon_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 255
        && !value.contains(['/', '\\'])
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn valid_theme_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 255
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn valid_theme_path(value: &str) -> bool {
    !value.is_empty() && value.len() <= 255 && Path::new(value).components().all(|component| {
        matches!(component, Component::Normal(name) if name.to_str().is_some_and(|name| name.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))))
    })
}

fn read_icon(path: &Path) -> Option<AppIcon> {
    match path.extension()?.to_str()? {
        "png" => normalize_png(read_bounded(path, MAX_SOURCE_ICON_BYTES as u64)?),
        "svg" => rasterize_svg(read_bounded(path, MAX_SOURCE_ICON_BYTES as u64)?),
        _ => None,
    }
}

fn rasterize_svg(svg: Vec<u8>) -> Option<AppIcon> {
    if svg.is_empty() || svg.len() > MAX_SOURCE_ICON_BYTES {
        return None;
    }
    let rejected_reference = Arc::new(AtomicBool::new(false));
    let data_reference = rejected_reference.clone();
    let string_reference = rejected_reference.clone();
    let mut options = usvg::Options::default();
    options.image_href_resolver = usvg::ImageHrefResolver {
        resolve_data: Box::new(move |_, _, _| {
            data_reference.store(true, Ordering::Relaxed);
            None
        }),
        resolve_string: Box::new(move |_, _| {
            string_reference.store(true, Ordering::Relaxed);
            None
        }),
    };
    let tree = usvg::Tree::from_data(&svg, &options).ok()?;
    if rejected_reference.load(Ordering::Relaxed) {
        return None;
    }
    let size = tree.size();
    let scale = (MAX_SOURCE_ICON_EDGE as f32 / size.width())
        .min(MAX_SOURCE_ICON_EDGE as f32 / size.height())
        .min(1.0);
    let width = (size.width() * scale).ceil() as u32;
    let height = (size.height() * scale).ceil() as u32;
    if width == 0 || height == 0 || width > MAX_SOURCE_ICON_EDGE || height > MAX_SOURCE_ICON_EDGE {
        return None;
    }
    let mut pixmap = tiny_skia::Pixmap::new(width, height)?;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    normalize_png(pixmap.encode_png().ok()?)
}

fn read_bounded(path: &Path, cap: u64) -> Option<Vec<u8>> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(rustix::fs::OFlags::NONBLOCK.bits() as i32);
    }
    let file = options.open(path).ok()?;
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
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    use image::{DynamicImage, ImageBuffer, ImageFormat, Rgba};
    use sha2::{Digest, Sha256};

    use super::*;

    fn png(width: u32, height: u32) -> Vec<u8> {
        let image = ImageBuffer::from_pixel(width, height, Rgba([0x24u8, 0x65, 0xa8, 0xff]));
        let mut bytes = Vec::new();
        DynamicImage::ImageRgba8(image)
            .write_to(&mut std::io::Cursor::new(&mut bytes), ImageFormat::Png)
            .unwrap();
        bytes
    }

    fn desktop(root: &Path, icon: &str, name: &str) {
        let applications = root.join("applications");
        std::fs::create_dir_all(&applications).unwrap();
        std::fs::write(
            applications.join("org.example.Writer.desktop"),
            format!("[Desktop Entry]\nName={name}\nIcon={icon}\n"),
        )
        .unwrap();
    }

    #[test]
    fn resolves_and_normalizes_a_256_icon_from_hicolor() {
        let root = tempfile::tempdir().unwrap();
        desktop(root.path(), "writer", "Writer");
        let icons = root.path().join("icons/hicolor/256x256/apps");
        std::fs::create_dir_all(&icons).unwrap();
        std::fs::write(icons.join("writer.png"), png(256, 128)).unwrap();
        let icon = resolve_from_roots("org.example.Writer", &[root.path().into()]).unwrap();
        assert_eq!((icon.width, icon.height), (128, 64));
        let bytes = STANDARD.decode(icon.png_base64).unwrap();
        assert_eq!(bytes.len(), 409);
        assert_eq!(
            hex::encode(Sha256::digest(bytes)),
            "6642fa85c75977a7d0e3cb1a4e480ff46e898ef0e1f697ff467bd1c24cbebedd"
        );
    }

    #[test]
    fn resolves_scalable_svg_through_theme_inheritance() {
        let root = tempfile::tempdir().unwrap();
        desktop(root.path(), "writer", "Writer");
        let active = root.path().join("icons/active");
        let parent = root.path().join("icons/parent/scalable/apps");
        std::fs::create_dir_all(&active).unwrap();
        std::fs::create_dir_all(&parent).unwrap();
        std::fs::write(
            active.join("index.theme"),
            "[Icon Theme]\nDirectories=48x48/apps\nInherits=parent\n",
        )
        .unwrap();
        std::fs::write(
            root.path().join("icons/parent/index.theme"),
            "[Icon Theme]\nDirectories=scalable/apps\n",
        )
        .unwrap();
        std::fs::write(parent.join("writer.svg"), "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"256\" height=\"128\"><rect width=\"256\" height=\"128\" fill=\"#2465a8\"/></svg>").unwrap();
        let icon = find_theme_icon(
            "writer",
            "active",
            &[root.path().join("icons")],
            &mut HashSet::new(),
            0,
        )
        .unwrap();
        assert_eq!((icon.width, icon.height), (128, 64));
    }

    #[test]
    fn rejects_path_escapes_and_external_or_oversized_svg_images() {
        let root = tempfile::tempdir().unwrap();
        desktop(root.path(), "../../not-an-icon", "Writer");
        assert!(resolve_from_roots("org.example.Writer", &[root.path().into()]).is_none());
        desktop(root.path(), "writer", "Writer");
        let icons = root.path().join("icons/hicolor/scalable/apps");
        std::fs::create_dir_all(&icons).unwrap();
        let private = root.path().join("private.png");
        std::fs::write(&private, png(1, 1)).unwrap();
        std::fs::write(
            icons.join("writer.svg"),
            format!(
                "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"16\" height=\"16\"><image href\t=\"file://{}\"/></svg>",
                private.display(),
            ),
        )
        .unwrap();
        assert!(resolve_from_roots("org.example.Writer", &[root.path().into()]).is_none());
        assert!(rasterize_svg(
            b"<svg xmlns=\"http://www.w3.org/2000/svg\"><image href=\"data:image/png;base64,AA==\"/></svg>".to_vec(),
        )
        .is_none());
        let oversized_data = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\"><image href=\"data:image/png;base64,{}\"/></svg>",
            "A".repeat(MAX_SOURCE_ICON_BYTES),
        );
        assert!(rasterize_svg(oversized_data.into_bytes()).is_none());
    }

    #[test]
    fn missing_or_deleted_desktop_entries_keep_the_verified_raw_app_id() {
        let root = tempfile::tempdir().unwrap();
        assert_eq!(
            source_label_from_roots("org.example.Writer", &[root.path().into()])
                .as_ref()
                .map(SourceAppLabel::as_str),
            Some("org.example.Writer")
        );
        desktop(root.path(), "writer", "Writer");
        std::fs::remove_file(root.path().join("applications/org.example.Writer.desktop")).unwrap();
        assert_eq!(
            source_label_from_roots("org.example.Writer", &[root.path().into()])
                .as_ref()
                .map(SourceAppLabel::as_str),
            Some("org.example.Writer")
        );
    }
}
