//! Display attribution is independent of foreground-based admission evidence.

use objc2::{msg_send, rc::Retained, ClassType};
use objc2_app_kit::{NSPasteboard, NSRunningApplication, NSWorkspace};
use objc2_foundation::{NSArray, NSBundle, NSString};
use tracing::{info, warn};

use super::MacOsClipboard;
use crate::macos_workspace::SourceIdentity as FrontmostApp;

const MAX_SOURCE_BYTES: usize = 255;

enum Marker<'a> {
    Absent,
    Unknown,
    Bundle(&'a str),
}

impl<'a> Marker<'a> {
    fn from_bytes(bytes: &'a [u8]) -> Self {
        let Ok(id) = std::str::from_utf8(bytes) else {
            return Self::Unknown;
        };
        if bytes.len() > MAX_SOURCE_BYTES
            || !copypaste_source_app::valid_package_id(id)
            || !id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        {
            Self::Unknown
        } else {
            Self::Bundle(id)
        }
    }
}

fn identity(
    marker: Marker<'_>,
    foreground: Option<&FrontmostApp>,
    resolve_name: impl FnOnce(&str) -> Option<String>,
) -> Option<FrontmostApp> {
    match marker {
        Marker::Absent => foreground.cloned(),
        // Empty is an explicit unknown original source. Invalid or unreadable
        // declarations must not be relabelled as the currently focused app.
        Marker::Unknown => None,
        Marker::Bundle(id) => Some(FrontmostApp {
            bundle_id: Some(id.to_owned()),
            name: foreground
                .filter(|app| app.bundle_id.as_deref() == Some(id))
                .and_then(|app| app.name.clone())
                .or_else(|| resolve_name(id)),
        }),
    }
}

/// Call only inside the admitted, generation-fenced payload envelope. This
/// optional informational marker cannot authorize capture or bypass exclusions.
pub(super) fn read(
    pasteboard: &NSPasteboard,
    source_type: &NSString,
    source_probe: &NSArray<NSString>,
    foreground: Option<&FrontmostApp>,
) -> Option<FrontmostApp> {
    unsafe {
        if pasteboard.availableTypeFromArray(source_probe).is_none() {
            return identity(Marker::Absent, foreground, application_name);
        }
        let data = pasteboard.dataForType(source_type)?;
        if data.length() > MAX_SOURCE_BYTES {
            return None;
        }
        identity(
            Marker::from_bytes(data.bytes()),
            foreground,
            application_name,
        )
    }
}

fn application_name(id: &str) -> Option<String> {
    unsafe {
        let id = NSString::from_str(id);
        let applications = NSRunningApplication::runningApplicationsWithBundleIdentifier(&id);
        for index in 0..applications.len() {
            let app = applications.get_retained(index)?;
            if let Some(name) = app.localizedName().filter(|name| !name.is_empty()) {
                return Some(name.to_string());
            }
        }
        let url = NSWorkspace::sharedWorkspace().URLForApplicationWithBundleIdentifier(&id)?;
        let bundle = NSBundle::bundleWithURL(&url)?;
        for key in ["CFBundleDisplayName", "CFBundleName"] {
            let Some(value) = bundle.objectForInfoDictionaryKey(&NSString::from_str(key)) else {
                continue;
            };
            let is_string: bool = msg_send![&*value, isKindOfClass: NSString::class()];
            if is_string {
                // SAFETY: The Info.plist value's runtime class was checked above.
                let name: Retained<NSString> = Retained::cast(value);
                if !name.is_empty() {
                    return Some(name.to_string());
                }
            }
        }
        None
    }
}

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
    pub(super) fn note_attribution(
        &mut self,
        decision: &super::super::source_coverage::Decision,
        app: Option<&FrontmostApp>,
    ) {
        use super::super::source_coverage::SourceConfidence;
        let attribution = match (Attribution::from_app(app), decision.coverage.confidence()) {
            (Attribution::Unavailable, SourceConfidence::AmbiguousObservedApplications) => {
                Attribution::Ambiguous
            }
            (attribution, _) => attribution,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn foreground() -> FrontmostApp {
        FrontmostApp {
            bundle_id: Some("com.test.Editor".into()),
            name: Some("Editor".into()),
        }
    }

    #[test]
    fn unmarked_copy_uses_fresh_foreground_metadata() {
        let app = foreground();
        assert_eq!(
            identity(Marker::Absent, Some(&app), |_| panic!("no lookup needed")),
            Some(app)
        );
        assert!(identity(Marker::Absent, None, |_| panic!("no lookup needed")).is_none());
    }

    #[test]
    fn declared_source_overrides_the_focused_application() {
        let result = identity(
            Marker::from_bytes(b"com.test.Browser"),
            Some(&foreground()),
            |id| {
                assert_eq!(id, "com.test.Browser");
                Some("Browser".into())
            },
        )
        .unwrap();
        assert_eq!(result.bundle_id.as_deref(), Some("com.test.Browser"));
        assert_eq!(result.name.as_deref(), Some("Browser"));
    }

    #[test]
    fn declared_source_reuses_matching_foreground_name() {
        let app = foreground();
        assert_eq!(
            identity(
                Marker::from_bytes(b"com.test.Editor"),
                Some(&app),
                |_| panic!("no lookup needed")
            ),
            Some(app)
        );
    }

    #[test]
    fn missing_declared_application_keeps_its_id_instead_of_guessing() {
        let result = identity(
            Marker::from_bytes(b"com.test.Uninstalled"),
            Some(&foreground()),
            |_| None,
        )
        .unwrap();
        assert_eq!(result.bundle_id.as_deref(), Some("com.test.Uninstalled"));
        assert!(result.name.is_none());
    }

    #[test]
    fn empty_invalid_and_oversized_declarations_do_not_fall_back() {
        for bytes in [
            b"".as_slice(),
            b"invalid",
            b"com.test.\xff",
            b"com.test.Editor\n",
            b"/tmp/Test.app",
            &[b'x'; 256],
        ] {
            assert!(
                identity(Marker::from_bytes(bytes), Some(&foreground()), |_| panic!(
                    "invalid marker must not resolve"
                ))
                .is_none()
            );
        }
        assert!(identity(Marker::Unknown, Some(&foreground()), |_| panic!(
            "no lookup needed"
        ))
        .is_none());
    }

    #[test]
    fn native_marker_reader_uses_an_isolated_pasteboard() {
        objc2::rc::autoreleasepool(|_| unsafe {
            let pb = NSPasteboard::pasteboardWithUniqueName();
            let source = NSString::from_str("org.nspasteboard.source");
            let probe = NSArray::from_vec(vec![source.clone()]);
            let app = foreground();
            assert_eq!(read(&pb, &source, &probe, Some(&app)), Some(app.clone()));
            pb.clearContents();
            assert!(pb.setString_forType(&NSString::from_str("com.test.Editor"), &source));
            assert_eq!(read(&pb, &source, &probe, Some(&app)), Some(app.clone()));
            pb.clearContents();
            assert!(pb.setString_forType(&NSString::from_str("com.apple.finder"), &source));
            let declared = read(&pb, &source, &probe, Some(&app)).unwrap();
            assert_eq!(declared.bundle_id.as_deref(), Some("com.apple.finder"));
            assert!(declared.name.as_ref().is_some_and(|name| !name.is_empty()));
            pb.clearContents();
            assert!(pb.setString_forType(&NSString::from_str(""), &source));
            assert!(read(&pb, &source, &probe, Some(&app)).is_none());
            let _: () = msg_send![&*pb, releaseGlobally];
        });
    }
}
