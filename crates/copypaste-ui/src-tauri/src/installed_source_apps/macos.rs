use std::path::{Path, PathBuf};

use plist::{Dictionary, Value};
use walkdir::WalkDir;

use super::{finish, CatalogUnavailable, InstalledSourceApp, Result};

const APP_EXTENSION: &str = "app";
const OWN_BUNDLE_ID: &str = "com.copypaste.app";
const MAX_APPLICATION_DIRECTORY_DEPTH: usize = 6;

pub(super) fn list() -> Result<Vec<InstalledSourceApp>> {
    list_from_roots(application_roots())
}

fn list_from_roots(roots: impl IntoIterator<Item = PathBuf>) -> Result<Vec<InstalledSourceApp>> {
    let mut apps = Vec::new();
    let mut readable_root = false;

    for root in roots {
        if std::fs::read_dir(&root).is_err() {
            continue;
        }
        readable_root = true;
        let mut walker = WalkDir::new(root)
            .follow_links(false)
            .max_depth(MAX_APPLICATION_DIRECTORY_DEPTH)
            .sort_by_file_name()
            .into_iter();
        while let Some(entry) = walker.next() {
            let Ok(entry) = entry else { continue };
            if entry.depth() == 0 || !is_app_bundle(entry.path()) {
                continue;
            }
            if let Some(app) = read_app(entry.path()) {
                apps.push(app);
            }
            if entry.file_type().is_dir() {
                // `skip_current_dir` pops the containing directory for a
                // symlink, which made Safari.app truncate `/Applications`.
                walker.skip_current_dir();
            }
        }
    }

    if !readable_root {
        return Err(CatalogUnavailable);
    }
    Ok(finish(apps, false))
}

fn application_roots() -> Vec<PathBuf> {
    let mut roots = vec![
        PathBuf::from("/Applications"),
        PathBuf::from("/System/Applications"),
        PathBuf::from("/Network/Applications"),
    ];
    if let Some(user) = directories::UserDirs::new() {
        roots.push(user.home_dir().join("Applications"));
    }
    roots
}

fn is_app_bundle(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case(APP_EXTENSION))
}

fn read_app(path: &Path) -> Option<InstalledSourceApp> {
    let info = Value::from_file(path.join("Contents/Info.plist")).ok()?;
    app_from_info(path, info.as_dictionary()?)
}

fn app_from_info(path: &Path, info: &Dictionary) -> Option<InstalledSourceApp> {
    if info.get("LSUIElement").and_then(Value::as_boolean) == Some(true)
        || info.get("LSBackgroundOnly").and_then(Value::as_boolean) == Some(true)
    {
        return None;
    }
    let executable = info
        .get("CFBundleExecutable")
        .and_then(Value::as_string)
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    let id = info
        .get("CFBundleIdentifier")
        .and_then(Value::as_string)
        .map(str::trim)
        .filter(|value| !value.is_empty() && *value != OWN_BUNDLE_ID)?;
    let label = ["CFBundleDisplayName", "CFBundleName"]
        .into_iter()
        .find_map(|key| {
            info.get(key)
                .and_then(Value::as_string)
                .map(str::trim)
                .filter(|value| !value.is_empty())
        })
        .or_else(|| path.file_stem().and_then(|name| name.to_str()))
        .unwrap_or(executable);
    Some(InstalledSourceApp {
        id: id.to_owned(),
        label: label.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_app(root: &Path, name: &str, id: &str, role: Option<&str>) -> PathBuf {
        let path = root.join(format!("{name}.app"));
        std::fs::create_dir_all(path.join("Contents/MacOS")).unwrap();
        std::fs::write(path.join("Contents/MacOS").join(name), b"executable").unwrap();
        let mut info = Dictionary::new();
        info.insert("CFBundleIdentifier".into(), id.into());
        info.insert("CFBundleExecutable".into(), name.into());
        info.insert("CFBundleDisplayName".into(), name.into());
        if let Some(role) = role {
            info.insert(role.into(), true.into());
        }
        Value::Dictionary(info)
            .to_file_xml(path.join("Contents/Info.plist"))
            .unwrap();
        path
    }

    #[test]
    fn bundle_metadata_keeps_launchable_apps_and_drops_agents() {
        let path = Path::new("/Applications/Writer.app");
        let mut info = Dictionary::new();
        info.insert("CFBundleIdentifier".into(), "com.example.writer".into());
        info.insert("CFBundleExecutable".into(), "Writer".into());
        info.insert("CFBundleDisplayName".into(), "Example Writer".into());
        assert_eq!(
            app_from_info(path, &info),
            Some(InstalledSourceApp {
                id: "com.example.writer".into(),
                label: "Example Writer".into(),
            })
        );

        info.insert("LSUIElement".into(), true.into());
        assert_eq!(app_from_info(path, &info), None);
    }

    #[test]
    fn app_symlink_does_not_truncate_its_application_directory() {
        let root = tempfile::tempdir().unwrap();
        let target = write_app(root.path(), "SafariTarget", "com.apple.Safari", None);
        let applications = root.path().join("Applications");
        std::fs::create_dir(&applications).unwrap();
        std::os::unix::fs::symlink(&target, applications.join("Safari.app")).unwrap();
        write_app(&applications, "Writer", "com.example.writer", None);

        let apps = list_from_roots([applications]).unwrap();

        assert!(apps.iter().any(|app| app.id == "com.apple.Safari"));
        assert!(apps.iter().any(|app| app.id == "com.example.writer"));
    }

    #[test]
    fn application_domains_include_personal_local_network_and_system_roots() {
        let roots = application_roots();
        assert!(roots.contains(&PathBuf::from("/Applications")));
        assert!(roots.contains(&PathBuf::from("/Network/Applications")));
        assert!(roots.contains(&PathBuf::from("/System/Applications")));
        let user = directories::UserDirs::new().unwrap();
        assert!(roots.contains(&user.home_dir().join("Applications")));
        assert!(!roots.contains(&PathBuf::from("/System/Library/CoreServices")));
    }

    #[test]
    fn catalogue_keeps_launchers_and_excludes_app_roles() {
        let root = tempfile::tempdir().unwrap();
        let applications = root.path().join("Applications");
        std::fs::create_dir(&applications).unwrap();
        write_app(&applications, "Writer", "com.example.writer", None);
        write_app(
            &applications,
            "MenuAgent",
            "com.example.menu-agent",
            Some("LSUIElement"),
        );
        write_app(
            &applications,
            "Worker",
            "com.example.worker",
            Some("LSBackgroundOnly"),
        );

        let apps = list_from_roots([applications]).unwrap();

        assert_eq!(
            apps,
            vec![InstalledSourceApp {
                id: "com.example.writer".into(),
                label: "Writer".into(),
            }]
        );
    }
}
