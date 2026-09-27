//! UI adapter for the shared, bounded native source-icon resolver.

use copypaste_source_app::{AppIcon, SourceAppIconCache as SharedSourceAppIconCache};

use crate::model::UiSourceAppIcon;

#[derive(Default)]
pub struct SourceAppIconCache(SharedSourceAppIconCache);

impl SourceAppIconCache {
    pub fn resolve_desktop(&self, bundle_id: &str) -> Option<UiSourceAppIcon> {
        self.0.resolve_desktop(bundle_id).and_then(to_ui)
    }

    pub fn resolve_with(
        &self,
        bundle_id: &str,
        resolver: impl FnOnce(&str) -> Option<UiSourceAppIcon>,
    ) -> Option<UiSourceAppIcon> {
        self.0
            .resolve_with(bundle_id, |app_id| resolver(app_id).and_then(from_ui))
            .and_then(to_ui)
    }
}

fn to_ui(icon: AppIcon) -> Option<UiSourceAppIcon> {
    Some(UiSourceAppIcon::from_app_icon(icon))
}

fn from_ui(icon: UiSourceAppIcon) -> Option<AppIcon> {
    icon.into_app_icon()
}
