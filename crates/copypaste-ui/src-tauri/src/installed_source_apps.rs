//! Installed, user-launchable applications for the source-exclusion picker.

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InstalledSourceApp {
    pub(crate) id: String,
    pub(crate) label: String,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct CatalogUnavailable;

pub(crate) type Result<T> = std::result::Result<T, CatalogUnavailable>;

#[cfg(target_os = "macos")]
pub(crate) fn list() -> Result<Vec<InstalledSourceApp>> {
    macos::list()
}

#[cfg(target_os = "windows")]
pub(crate) fn list() -> Result<Vec<InstalledSourceApp>> {
    windows::list()
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub(crate) fn list() -> Result<Vec<InstalledSourceApp>> {
    Ok(Vec::new())
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn finish(
    mut apps: Vec<InstalledSourceApp>,
    ids_ignore_ascii_case: bool,
) -> Vec<InstalledSourceApp> {
    apps.retain(|app| !app.id.trim().is_empty() && !app.label.trim().is_empty());
    let mut seen = std::collections::HashSet::new();
    apps.retain(|app| {
        let key = if ids_ignore_ascii_case {
            app.id.to_lowercase()
        } else {
            app.id.clone()
        };
        seen.insert(key)
    });
    apps.sort_by(|left, right| {
        left.label
            .to_lowercase()
            .cmp(&right.label.to_lowercase())
            .then_with(|| left.id.to_lowercase().cmp(&right.id.to_lowercase()))
    });
    apps
}
