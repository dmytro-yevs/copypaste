use tracing::{info, warn};

use super::MacOsClipboard;
use crate::macos_workspace::SourceIdentity as FrontmostApp;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Attribution {
    Bundle,
    NameOnly,
    Unavailable,
    Ambiguous,
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
    pub(super) fn note_attribution(&mut self, decision: &super::super::source_coverage::Decision) {
        use super::super::source_coverage::SourceConfidence;
        let attribution = match decision.coverage.confidence() {
            SourceConfidence::SingleObservedApplication => {
                Attribution::from_app(decision.identity.as_ref())
            }
            SourceConfidence::AmbiguousObservedApplications => Attribution::Ambiguous,
            SourceConfidence::Unavailable => Attribution::Unavailable,
        };
        if self.last_attribution == Some(attribution) {
            return;
        }
        self.last_attribution = Some(attribution);
        match attribution {
            Attribution::Bundle => info!("macOS capture source attribution is available"),
            Attribution::NameOnly => {
                info!("macOS capture source attribution is available without a bundle identifier")
            }
            Attribution::Ambiguous => info!("macOS capture source attribution is ambiguous"),
            Attribution::Unavailable => {
                warn!("macOS could not identify the source application for clipboard capture")
            }
        }
    }
}
