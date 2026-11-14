//! Applications installed on this desktop, for choosing one by name. Android
//! lists its launcher apps in the platform host instead.

use super::ClipImagePreview;

/// An application by the identifier capture attribution records for it.
#[derive(Debug, Clone)]
pub struct InstalledApplication {
    pub id: String,
    pub name: String,
}

/// Installed applications sorted by name; nothing is retained afterwards.
pub fn installed_applications() -> Vec<InstalledApplication> {
    #[cfg(not(target_os = "android"))]
    {
        copypaste_source_app::installed_apps()
            .into_iter()
            .map(|app| InstalledApplication {
                id: app.id,
                name: app.name,
            })
            .collect()
    }
    #[cfg(target_os = "android")]
    Vec::new()
}

/// One installed application, or `None` when it is not installed here.
pub fn installed_application(id: String) -> Option<InstalledApplication> {
    #[cfg(not(target_os = "android"))]
    {
        copypaste_source_app::installed_app(&id).map(|app| InstalledApplication {
            id: app.id,
            name: app.name,
        })
    }
    #[cfg(target_os = "android")]
    {
        let _ = id;
        None
    }
}

/// The application's icon as PNG, resolved on demand and not cached.
pub fn installed_application_icon(id: String) -> Option<ClipImagePreview> {
    #[cfg(not(target_os = "android"))]
    {
        use base64::{engine::general_purpose::STANDARD, Engine as _};

        let icon = copypaste_source_app::app_icon(&id)?;
        Some(ClipImagePreview {
            png_bytes: STANDARD.decode(icon.png_base64).ok()?,
            width: icon.width,
            height: icon.height,
        })
    }
    #[cfg(target_os = "android")]
    {
        let _ = id;
        None
    }
}
