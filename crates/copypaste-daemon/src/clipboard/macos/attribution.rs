use std::time::{Duration, Instant};

use objc2_app_kit::NSWorkspace;
use tracing::{info, warn};

use super::MacOsClipboard;

#[derive(Clone)]
pub(super) struct FrontmostApp {
    /// Some foreground helpers do not advertise a bundle identifier even
    /// though AppKit can still tell us their localized name. Keep the two
    /// independently so the UI never turns a known source into “Unknown app”.
    pub(super) bundle_id: Option<String>,
    pub(super) name: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Attribution {
    Bundle,
    NameOnly,
    Unavailable,
}

impl Attribution {
    pub(super) fn from_app(app: Option<&FrontmostApp>) -> Self {
        match app {
            Some(FrontmostApp {
                bundle_id: Some(_), ..
            }) => Self::Bundle,
            Some(FrontmostApp { name: Some(_), .. }) => Self::NameOnly,
            _ => Self::Unavailable,
        }
    }
}

impl MacOsClipboard {
    pub(super) fn note_attribution(&mut self, app: Option<&FrontmostApp>) {
        let attribution = Attribution::from_app(app);
        if self.last_attribution == Some(attribution) {
            return;
        }
        self.last_attribution = Some(attribution);
        match attribution {
            Attribution::Bundle => info!("macOS capture source attribution is available"),
            Attribution::NameOnly => {
                info!("macOS capture source attribution is available without a bundle identifier")
            }
            Attribution::Unavailable => {
                warn!("macOS could not identify the source application for clipboard capture")
            }
        }
    }

    pub(super) fn frontmost_app(&mut self) -> Option<FrontmostApp> {
        if let Some((seen, app)) = &self.frontmost {
            if seen.elapsed() < Duration::from_millis(750) {
                return app.clone();
            }
        }
        let app = unsafe {
            NSWorkspace::sharedWorkspace()
                .frontmostApplication()
                .and_then(|app| {
                    let bundle_id = app.bundleIdentifier().map(|id| id.to_string());
                    let name = app.localizedName().map(|name| name.to_string());
                    // A foreground application can be an unbundled helper.
                    // It has neither value only when AppKit genuinely cannot
                    // attribute the foreground process.
                    (bundle_id.is_some() || name.is_some())
                        .then_some(FrontmostApp { bundle_id, name })
                })
        };
        self.frontmost = Some((Instant::now(), app.clone()));
        app
    }
}
