//! Applications installed on this desktop, each named by the identifier
//! capture attribution records for it: a macOS bundle ID, a lowercase
//! Windows image name, or a Linux desktop entry ID.

use std::collections::HashSet;

use crate::AppIcon;

/// The most applications one listing returns.
pub const MAX_INSTALLED_APPS: usize = 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledApp {
    pub id: String,
    pub name: String,
}

/// Installed applications, deduplicated by identifier and sorted by name.
pub fn installed_apps() -> Vec<InstalledApp> {
    finish(platform::list())
}

/// One installed application by its identifier.
pub fn installed_app(id: &str) -> Option<InstalledApp> {
    (!id.is_empty() && id.len() <= 255).then_some(())?;
    platform::find(id)
}

/// The application's icon, resolved without caching.
pub fn app_icon(id: &str) -> Option<AppIcon> {
    crate::valid_package_id(id).then_some(())?;
    crate::resolve_desktop(id)
}

fn finish(apps: Vec<InstalledApp>) -> Vec<InstalledApp> {
    let mut seen = HashSet::new();
    let mut apps: Vec<_> = apps
        .into_iter()
        .filter(|app| !app.name.trim().is_empty() && seen.insert(app.id.clone()))
        .collect();
    apps.sort_by_cached_key(|app| (app.name.to_lowercase(), app.id.clone()));
    apps.truncate(MAX_INSTALLED_APPS);
    apps
}

/// The file path of a Windows `DisplayIcon` value such as
/// `"C:\App\app.exe",0`.
#[cfg(any(target_os = "windows", test))]
pub(crate) fn display_icon_path(value: &str) -> &str {
    let path = value
        .rsplit_once(',')
        .filter(|(_, index)| index.trim().parse::<i32>().is_ok())
        .map_or(value, |(path, _)| path);
    path.trim().trim_matches('"').trim()
}

/// The lowercase image name a `DisplayIcon` value names; uninstallers and
/// icon files name no application.
#[cfg(any(target_os = "windows", test))]
fn display_icon_image(value: &str) -> Option<String> {
    let path = display_icon_path(value);
    let name = path.rsplit(['\\', '/']).next()?.trim().to_lowercase();
    let stem = name.strip_suffix(".exe")?;
    let uninstaller = stem.starts_with("unins") || stem.starts_with("uninst");
    (!stem.is_empty() && !uninstaller && crate::valid_package_id(&name)).then_some(name)
}

#[cfg(target_os = "macos")]
mod platform {
    use std::path::{Path, PathBuf};

    use objc2::rc::{autoreleasepool, Retained};
    use objc2::runtime::NSObjectProtocol;
    use objc2_app_kit::NSWorkspace;
    use objc2_foundation::{NSDictionary, NSFileManager, NSObject, NSString};

    use super::{InstalledApp, MAX_INSTALLED_APPS};

    const MAX_DIRECTORY_ENTRIES: usize = 2048;

    pub(super) fn list() -> Vec<InstalledApp> {
        let mut roots = vec![
            PathBuf::from("/Applications"),
            PathBuf::from("/System/Applications"),
        ];
        if let Some(home) = std::env::var_os("HOME") {
            roots.push(PathBuf::from(home).join("Applications"));
        }
        let mut bundles = Vec::new();
        for root in roots {
            collect(&root, 1, &mut bundles);
        }
        bundles
            .iter()
            .filter_map(|path| autoreleasepool(|_| describe(path)))
            .collect()
    }

    pub(super) fn find(id: &str) -> Option<InstalledApp> {
        autoreleasepool(|_| {
            let workspace = unsafe { NSWorkspace::sharedWorkspace() };
            let url = unsafe {
                workspace.URLForApplicationWithBundleIdentifier(&NSString::from_str(id))
            }?;
            let path = unsafe { url.path() }?;
            Some(InstalledApp {
                id: id.to_owned(),
                name: display_name(&path),
            })
        })
    }

    /// Bundles directly in [dir], and in its plain subfolders such as
    /// `Utilities` while [depth] allows.
    fn collect(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.take(MAX_DIRECTORY_ENTRIES).filter_map(Result::ok) {
            if out.len() >= MAX_INSTALLED_APPS * 2 {
                return;
            }
            let path = entry.path();
            if path.extension().is_some_and(|extension| extension == "app") {
                out.push(path);
            } else if depth > 0 && entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                collect(&path, depth - 1, out);
            }
        }
    }

    fn describe(bundle: &Path) -> Option<InstalledApp> {
        let path = NSString::from_str(bundle.to_str()?);
        let plist = NSString::from_str(bundle.join("Contents/Info.plist").to_str()?);
        #[allow(deprecated)]
        let info =
            unsafe { NSDictionary::<NSString, NSObject>::dictionaryWithContentsOfFile(&plist) }?;
        let value = unsafe { info.objectForKey(&NSString::from_str("CFBundleIdentifier")) }?;
        if !value.is_kind_of::<NSString>() {
            return None;
        }
        let id = unsafe { Retained::cast::<NSString>(value) }.to_string();
        (!id.is_empty() && id.len() <= 255).then(|| InstalledApp {
            id,
            name: display_name(&path),
        })
    }

    fn display_name(path: &NSString) -> String {
        let name = unsafe { NSFileManager::defaultManager().displayNameAtPath(path) }.to_string();
        name.strip_suffix(".app")
            .map_or(name.clone(), str::to_owned)
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use super::{display_icon_image, InstalledApp};
    use crate::registry;

    pub(super) fn list() -> Vec<InstalledApp> {
        let mut apps: Vec<InstalledApp> = registry::uninstall_entries()
            .into_iter()
            .filter_map(|(display_icon, name)| {
                Some(InstalledApp {
                    id: display_icon_image(&display_icon)?,
                    name,
                })
            })
            .collect();
        apps.extend(
            registry::image_names()
                .unwrap_or_default()
                .into_iter()
                .map(|image| image.to_lowercase())
                .filter(|image| crate::valid_package_id(image) && image.ends_with(".exe"))
                .map(|id| InstalledApp {
                    name: id.trim_end_matches(".exe").to_owned(),
                    id,
                }),
        );
        apps
    }

    /// Matches as capture exclusions do: without case or `.exe`.
    pub(super) fn find(id: &str) -> Option<InstalledApp> {
        let id = id.trim().to_lowercase();
        let stem = id.strip_suffix(".exe").unwrap_or(&id);
        list()
            .into_iter()
            .find(|app| app.id.strip_suffix(".exe") == Some(stem))
    }
}

#[cfg(target_os = "linux")]
mod platform {
    use super::InstalledApp;

    pub(super) fn list() -> Vec<InstalledApp> {
        crate::linux::installed()
            .into_iter()
            .map(|(id, name)| InstalledApp { id, name })
            .collect()
    }

    pub(super) fn find(id: &str) -> Option<InstalledApp> {
        crate::linux::installed_name(id).map(|name| InstalledApp {
            id: id.to_owned(),
            name,
        })
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
mod platform {
    use super::InstalledApp;

    pub(super) fn list() -> Vec<InstalledApp> {
        Vec::new()
    }

    pub(super) fn find(_id: &str) -> Option<InstalledApp> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(id: &str, name: &str) -> InstalledApp {
        InstalledApp {
            id: id.into(),
            name: name.into(),
        }
    }

    #[test]
    fn listings_are_deduplicated_named_and_sorted() {
        let apps = finish(vec![
            app("com.example.zed", "zed"),
            app("com.example.writer", "Writer"),
            app("com.example.writer", "Writer copy"),
            app("com.example.blank", " "),
            app("com.example.atlas", "Atlas"),
        ]);
        assert_eq!(
            apps,
            [
                app("com.example.atlas", "Atlas"),
                app("com.example.writer", "Writer"),
                app("com.example.zed", "zed"),
            ]
        );
    }

    #[test]
    fn display_icons_name_the_image_capture_records() {
        let image = |value| display_icon_image(value);
        assert_eq!(
            image(r#""C:\Program Files\Editor\Editor.exe",0"#).as_deref(),
            Some("editor.exe")
        );
        assert_eq!(
            image(r"C:\Program Files\Proton Pass\Proton Pass.exe").as_deref(),
            Some("proton pass.exe")
        );
        assert_eq!(image(r"C:\Program Files\Editor\unins000.exe"), None);
        assert_eq!(image(r"C:\Program Files\Editor\editor.ico"), None);
    }
}
