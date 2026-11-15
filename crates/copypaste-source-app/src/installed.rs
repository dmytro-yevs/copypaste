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

/// The application's icon, resolved without the capture icon cache.
pub fn app_icon(id: &str) -> Option<AppIcon> {
    (!id.is_empty() && id.len() <= 255).then_some(())?;
    platform::icon(id)
}

fn finish(apps: Vec<InstalledApp>) -> Vec<InstalledApp> {
    finish_by(apps, |app| app)
}

/// [items] deduplicated by application identifier, named, sorted by name and
/// bounded.
fn finish_by<T>(items: Vec<T>, app: impl Fn(&T) -> &InstalledApp) -> Vec<T> {
    let mut seen = HashSet::new();
    let mut items: Vec<_> = items
        .into_iter()
        .filter(|item| !app(item).name.trim().is_empty() && seen.insert(app(item).id.clone()))
        .collect();
    items.sort_by_cached_key(|item| (app(item).name.to_lowercase(), app(item).id.clone()));
    items.truncate(MAX_INSTALLED_APPS);
    items
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

    pub(super) fn icon(id: &str) -> Option<crate::AppIcon> {
        crate::valid_package_id(id).then_some(())?;
        crate::resolve_desktop(id)
    }

    fn display_name(path: &NSString) -> String {
        let name = unsafe { NSFileManager::defaultManager().displayNameAtPath(path) }.to_string();
        name.strip_suffix(".app")
            .map_or(name.clone(), str::to_owned)
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex, PoisonError};
    use std::time::{Duration, Instant};

    use super::{finish_by, InstalledApp};
    use crate::windows_programs::{self, application_executable, image_name, Files};
    use crate::{registry, win_icon, AppIcon};

    /// How long one registry listing answers lookups and icons; a picker and
    /// the settings rows it serves reuse it instead of rescanning per row.
    const INDEX_TTL: Duration = Duration::from_secs(30);
    const MAX_FOLDER_ENTRIES: usize = 256;

    struct Entry {
        app: InstalledApp,
        path: Option<PathBuf>,
    }

    static INDEX: Mutex<Option<(Instant, Arc<[Entry]>)>> = Mutex::new(None);

    pub(super) fn list() -> Vec<InstalledApp> {
        index().iter().map(|entry| entry.app.clone()).collect()
    }

    /// Matches as capture exclusions do: without case or `.exe`.
    pub(super) fn find(id: &str) -> Option<InstalledApp> {
        indexed(id, |entry| entry.app.clone())
    }

    pub(super) fn icon(id: &str) -> Option<AppIcon> {
        crate::valid_package_id(id).then_some(())?;
        let path = indexed(id, |entry| entry.path.clone())
            .flatten()
            .or_else(|| registry::executable(id))?;
        win_icon::icon_at(&path)
    }

    fn indexed<T>(id: &str, value: impl FnOnce(&Entry) -> T) -> Option<T> {
        let id = id.trim().to_lowercase();
        let stem = id.strip_suffix(".exe").unwrap_or(&id);
        index()
            .iter()
            .find(|entry| entry.app.id.strip_suffix(".exe") == Some(stem))
            .map(value)
    }

    /// The cached listing, rebuilt once it is older than [INDEX_TTL]; a stale
    /// one is dropped before the rebuild.
    fn index() -> Arc<[Entry]> {
        let mut cached = INDEX.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some((built, entries)) = cached.as_ref() {
            if built.elapsed() < INDEX_TTL {
                return entries.clone();
            }
        }
        *cached = None;
        let entries: Arc<[Entry]> = build().into();
        *cached = Some((Instant::now(), entries.clone()));
        entries
    }

    fn build() -> Vec<Entry> {
        let app_paths = registry::app_paths();
        let registered: Vec<String> = app_paths
            .iter()
            .filter_map(|(_, path)| path.clone())
            .collect();
        let mut entries: Vec<Entry> = registry::uninstall_programs()
            .iter()
            .filter_map(|program| {
                let path = windows_programs::executable(program, &registered, &Disk)?;
                Some(Entry {
                    app: InstalledApp {
                        id: image_name(&path),
                        name: program.name.clone(),
                    },
                    path: Some(PathBuf::from(path)),
                })
            })
            .collect();
        entries.extend(
            app_paths
                .into_iter()
                .filter(|(image, _)| application_executable(image))
                .map(|(image, path)| {
                    let id = image.to_lowercase();
                    Entry {
                        app: InstalledApp {
                            name: id.trim_end_matches(".exe").to_owned(),
                            id,
                        },
                        path: path.map(PathBuf::from),
                    }
                }),
        );
        finish_by(entries, |entry| &entry.app)
    }

    struct Disk;

    impl Files for Disk {
        fn is_file(&self, path: &str) -> bool {
            Path::new(path).is_file()
        }

        fn executables(&self, dir: &str) -> Vec<String> {
            let Ok(entries) = std::fs::read_dir(dir) else {
                return Vec::new();
            };
            entries
                .take(MAX_FOLDER_ENTRIES)
                .filter_map(|entry| {
                    let entry = entry.ok()?;
                    entry.file_type().ok()?.is_file().then_some(())?;
                    let path = entry.path();
                    path.extension()
                        .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
                        .then_some(())?;
                    Some(path.to_str()?.to_owned())
                })
                .collect()
        }
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

    pub(super) fn icon(id: &str) -> Option<crate::AppIcon> {
        crate::linux::resolve(id)
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

    pub(super) fn icon(_id: &str) -> Option<crate::AppIcon> {
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
}
