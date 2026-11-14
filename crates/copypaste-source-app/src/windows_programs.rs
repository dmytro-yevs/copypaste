//! The executable an installed Windows program runs as, chosen from its
//! uninstall registration so it matches the process image name capture
//! attribution records for it.

/// One uninstall registration with environment references already expanded.
#[derive(Debug, Clone, Default)]
pub(crate) struct Program {
    pub name: String,
    pub display_icon: Option<String>,
    pub install_location: Option<String>,
}

/// The file system facts the choice depends on.
pub(crate) trait Files {
    fn is_file(&self, path: &str) -> bool;

    /// The `.exe` files directly in [dir].
    fn executables(&self, dir: &str) -> Vec<String>;
}

/// The program's application executable: its `DisplayIcon` when that is one,
/// else a registered (`App Paths`) or present executable in its install or
/// icon folder that its name picks out. `None` when the registration names
/// only installers, uninstallers, cached packages or icon files.
pub(crate) fn executable(
    program: &Program,
    registered: &[String],
    files: &impl Files,
) -> Option<String> {
    let icon = program.display_icon.as_deref().map(display_icon_path);
    if let Some(icon) = icon.filter(|icon| application_executable(icon) && files.is_file(icon)) {
        return Some(icon.to_owned());
    }
    let folders = program
        .install_location
        .as_deref()
        .map(folder)
        .into_iter()
        .chain(icon.and_then(parent).map(folder));
    for folder in folders.filter(|folder| !system_managed(folder) && !shared_folder(folder)) {
        let inside = |path: &&String| {
            parent(path).is_some_and(|parent| {
                let parent = parent.to_lowercase().replace('/', "\\");
                parent == folder || parent.starts_with(&format!("{folder}\\"))
            })
        };
        let chosen = best_match(
            registered
                .iter()
                .filter(inside)
                .filter(|path| files.is_file(path)),
            &program.name,
        )
        .or_else(|| best_match(files.executables(&folder).iter(), &program.name));
        if chosen.is_some() {
            return chosen;
        }
    }
    None
}

/// The file path of a `DisplayIcon` value such as `"C:\App\app.exe",0`.
pub(crate) fn display_icon_path(value: &str) -> &str {
    let path = value
        .rsplit_once(',')
        .filter(|(_, index)| index.trim().parse::<i32>().is_ok())
        .map_or(value, |(path, _)| path);
    path.trim().trim_matches('"').trim()
}

/// The lowercase image name of [path].
pub(crate) fn image_name(path: &str) -> String {
    file_name(path).to_lowercase()
}

/// Whether [path] can be an application's process image: an `.exe` that is
/// not an installer, uninstaller or updater, and not a copy Windows Installer
/// or a bundle's package cache keeps.
pub(crate) fn application_executable(path: &str) -> bool {
    let name = image_name(path);
    let Some(stem) = name.strip_suffix(".exe") else {
        return false;
    };
    let installer = stem.starts_with("unins")
        || stem == "arpproducticon"
        || ["install", "setup", "update", "redist"]
            .iter()
            .any(|word| stem.contains(word));
    !installer
        && crate::valid_package_id(&name)
        && parent(path).is_none_or(|parent| !system_managed(&folder(parent)))
}

fn system_managed(folder: &str) -> bool {
    let folder = format!("{folder}\\");
    folder.contains("\\windows\\installer\\") || folder.contains("\\package cache\\")
}

/// A drive or a folder many programs install into, which names no one
/// program.
fn shared_folder(folder: &str) -> bool {
    let name = file_name(folder);
    folder.matches('\\').count() < 2
        || [
            "program files",
            "program files (x86)",
            "programs",
            "local",
            "roaming",
        ]
        .contains(&name)
}

/// The one candidate, else the one whose file name best matches [name].
fn best_match<'a>(candidates: impl Iterator<Item = &'a String>, name: &str) -> Option<String> {
    let candidates: Vec<&String> = candidates
        .filter(|path| application_executable(path))
        .collect();
    if let [only] = candidates.as_slice() {
        return Some((*only).clone());
    }
    let wanted = letters(name);
    let scored: Vec<(u8, &String)> = candidates
        .into_iter()
        .map(|path| {
            let stem = letters(
                file_name(path)
                    .rsplit_once('.')
                    .map_or("", |(stem, _)| stem),
            );
            (similarity(&stem, &wanted), path)
        })
        .filter(|(score, _)| *score > 0)
        .collect();
    let best = scored.iter().map(|(score, _)| *score).max()?;
    match scored
        .iter()
        .filter(|(score, _)| *score == best)
        .collect::<Vec<_>>()[..]
    {
        [(_, path)] => Some((*path).clone()),
        _ => None,
    }
}

fn similarity(stem: &str, name: &str) -> u8 {
    if stem.is_empty() || name.is_empty() {
        0
    } else if stem == name {
        3
    } else if stem.len() >= 3 && (name.starts_with(stem) || stem.starts_with(name)) {
        2
    } else if stem.len() >= 3 && name.contains(stem) {
        1
    } else {
        0
    }
}

fn letters(value: &str) -> String {
    value
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|character| character.to_ascii_lowercase())
        .collect()
}

fn file_name(path: &str) -> &str {
    path.rsplit(['\\', '/']).next().unwrap_or(path).trim()
}

fn parent(path: &str) -> Option<&str> {
    path.rsplit_once(['\\', '/']).map(|(parent, _)| parent)
}

/// A comparable folder: lowercase, backslashes, no quotes or trailing slash.
fn folder(path: &str) -> String {
    path.trim()
        .trim_matches('"')
        .trim_end_matches(['\\', '/'])
        .to_lowercase()
        .replace('/', "\\")
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use super::*;

    #[derive(Default)]
    struct Disk {
        files: HashSet<String>,
        folders: HashMap<String, Vec<String>>,
    }

    impl Disk {
        fn with(paths: &[&str]) -> Self {
            let mut disk = Self::default();
            for path in paths {
                disk.files.insert((*path).to_owned());
                disk.folders
                    .entry(folder(parent(path).unwrap()))
                    .or_default()
                    .push((*path).to_owned());
            }
            disk
        }
    }

    impl Files for Disk {
        fn is_file(&self, path: &str) -> bool {
            self.files.contains(path)
        }

        fn executables(&self, dir: &str) -> Vec<String> {
            self.folders
                .get(&folder(dir))
                .into_iter()
                .flatten()
                .filter(|path| path.to_lowercase().ends_with(".exe"))
                .cloned()
                .collect()
        }
    }

    fn program(name: &str, icon: Option<&str>, location: Option<&str>) -> Program {
        Program {
            name: name.into(),
            display_icon: icon.map(Into::into),
            install_location: location.map(Into::into),
        }
    }

    #[test]
    fn a_display_icon_that_is_the_application_names_it() {
        let disk = Disk::with(&[r"C:\Program Files\Editor\Editor.exe"]);
        let editor = program(
            "Editor",
            Some(r#""C:\Program Files\Editor\Editor.exe",0"#),
            None,
        );
        assert_eq!(
            executable(&editor, &[], &disk).map(|path| image_name(&path)),
            Some("editor.exe".into())
        );
    }

    #[test]
    fn installer_and_cached_package_icons_are_never_the_identity() {
        for path in [
            r"C:\Windows\Installer\{90160000-0011-0000-1000-0000000FF1CE}\ARPPRODUCTICON.exe",
            r"C:\ProgramData\Package Cache\{0a1b}\VC_redist.x64.exe",
            r"C:\ProgramData\Package Cache\{0a1b}\python-3.12.0-amd64.exe",
            r"C:\Program Files\Editor\unins000.exe",
            r"C:\Program Files\Editor\Uninstall Editor.exe",
            r"C:\Program Files\Editor\setup.exe",
            r"C:\Program Files\Editor\editor.ico",
        ] {
            assert!(!application_executable(path), "{path}");
        }
        assert!(application_executable(
            r"C:\Program Files\Proton Pass\Proton Pass.exe"
        ));
    }

    #[test]
    fn an_msi_program_is_found_in_its_install_location_by_name() {
        let disk = Disk::with(&[
            r"C:\Program Files\Zoom\bin\Zoom.exe",
            r"C:\Program Files\Zoom\bin\CptHost.exe",
            r"C:\Program Files\Zoom\bin\Installer.exe",
        ]);
        let zoom = program(
            "Zoom Workplace (64-bit)",
            Some(r"C:\Windows\Installer\{3109C49B}\ARPPRODUCTICON.exe"),
            Some(r"C:\Program Files\Zoom\bin\"),
        );
        assert_eq!(
            executable(&zoom, &[], &disk).map(|path| image_name(&path)),
            Some("zoom.exe".into())
        );
    }

    #[test]
    fn a_registered_executable_under_the_install_location_wins() {
        let disk = Disk::with(&[
            r"C:\Apps\Studio\Studio.exe",
            r"C:\Apps\Studio\bin\studio64.exe",
        ]);
        let studio = program("Code Studio", None, Some(r"C:\Apps\Studio"));
        let registered = [r"C:\Apps\Studio\bin\studio64.exe".to_owned()];
        assert_eq!(
            executable(&studio, &registered, &disk).map(|path| image_name(&path)),
            Some("studio64.exe".into())
        );
    }

    #[test]
    fn an_icon_file_points_at_its_folder() {
        let disk = Disk::with(&[r"C:\Users\me\AppData\Local\Chat\Chat.exe"]);
        let chat = program(
            "Chat",
            Some(r"C:\Users\me\AppData\Local\Chat\app.ico"),
            None,
        );
        assert_eq!(
            executable(&chat, &[], &disk).map(|path| image_name(&path)),
            Some("chat.exe".into())
        );
    }

    #[test]
    fn registrations_naming_no_application_are_dropped() {
        let disk = Disk::with(&[
            r"C:\ProgramData\Package Cache\{0a1b}\VC_redist.x64.exe",
            r"C:\Program Files\Tools\alpha.exe",
            r"C:\Program Files\Tools\beta.exe",
        ]);
        let redist = program(
            "Microsoft Visual C++ Redistributable",
            Some(r"C:\ProgramData\Package Cache\{0a1b}\VC_redist.x64.exe,0"),
            None,
        );
        assert_eq!(executable(&redist, &[], &disk), None);
        let ambiguous = program("Tools", None, Some(r"C:\Program Files\Tools"));
        assert_eq!(executable(&ambiguous, &[], &disk), None);
        let shared = program("Tools Alpha", None, Some(r"C:\Program Files\"));
        let registered = [r"C:\Program Files\Tools\alpha.exe".to_owned()];
        assert_eq!(executable(&shared, &registered, &disk), None);
        let missing = program(
            "Gone",
            Some(r"C:\Program Files\Gone\gone.exe"),
            Some(r"C:\Program Files\Gone"),
        );
        assert_eq!(executable(&missing, &[], &disk), None);
    }
}
