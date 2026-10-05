use tracing::{info, warn};

use super::MacOsClipboard;
use crate::macos_workspace::SourceIdentity as FrontmostApp;

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

    pub(super) fn frontmost_app(
        &mut self,
        generation: i64,
        excluded: &[String],
    ) -> Option<FrontmostApp> {
        crate::macos_workspace::source_identity(self.source_observation, generation, excluded)
    }
}
