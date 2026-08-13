//! What Windows attribution decides, apart from the syscalls that answer it.
//!
//! The same split as [`super::windows_optout`], for the same reason: what only
//! one platform can run is what regresses unnoticed. Every rule here — which
//! change an identity belongs to, what a user's exclusion entry means, and what
//! an unattributable change costs — is exercised by `cargo test` on any host.

use tracing::{info, warn};

/// The process that wrote the clipboard, as an item carries it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SourceApp {
    /// The process image file name, lowercased: `1password.exe`.
    ///
    /// Windows has no bundle identifier. The file name is the only stable
    /// identity a process reliably has, and lowercasing it is what makes it
    /// comparable — `copypaste_core::sensitive::is_password_manager_app`
    /// lowercases before matching, and [`is_excluded`] compares canonical keys.
    pub(crate) id: String,
    /// The same name without its extension, for display.
    pub(crate) name: String,
}

impl SourceApp {
    /// The *path* is dropped here rather than later: it contains the local user
    /// name (I-9), and this value is about to be stored on an item.
    pub(crate) fn from_image_path(path: &str) -> Option<Self> {
        let file_name = file_name(path)?;
        let name = file_name
            .rsplit_once('.')
            .map_or(file_name, |(stem, _)| stem)
            .to_owned();
        Some(Self {
            id: file_name.to_lowercase(),
            name,
        })
    }
}

/// `std::path::Path::file_name` answers with the *host's* separators, so off
/// Windows it would call an entire `C:\…` path the file name and these tests
/// would pass while the shipped behaviour did the opposite.
fn file_name(path: &str) -> Option<&str> {
    let trimmed = path.trim().trim_matches('"');
    let name = trimmed.rsplit(['\\', '/', ':']).next()?.trim();
    (!name.is_empty()).then_some(name)
}

/// The source application of the clipboard change now being polled.
#[derive(Default)]
pub(crate) struct Attribution {
    resolved: Option<(i64, Option<SourceApp>)>,
    available: Option<bool>,
    resolutions: u64,
    unattributed: u64,
    excluded: u64,
}

impl Attribution {
    /// The writer is a property of *the change*, not of the moment the poll ran.
    ///
    /// Manifest 01 §3.9(c) bounds how stale an identity may be by a TTL just
    /// above one poll period; on Windows the bound can be zero, because
    /// `GetClipboardSequenceNumber` names the change the identity belongs to.
    /// A 750 ms TTL spans more than one 500 ms tick, so an ordinary copy
    /// followed by a password manager's copy inherited the first app — past
    /// both the exclusion list and the credential-store sensitivity floor
    /// (DMY-158, the shape of CopyPaste-8ebg.57).
    pub(crate) fn for_change(
        &mut self,
        change: i64,
        resolve: impl FnOnce() -> Option<SourceApp>,
    ) -> Option<SourceApp> {
        if let Some((seen, app)) = &self.resolved {
            if *seen == change {
                return app.clone();
            }
        }
        let app = resolve();
        self.resolutions += 1;
        if app.is_none() {
            self.unattributed += 1;
        }
        self.resolved = Some((change, app.clone()));
        app
    }

    /// Said once per transition, not once per poll: an exclusion list that
    /// silently matches nothing is the failure this makes visible.
    pub(super) fn note(&mut self, app: Option<&SourceApp>) {
        let available = app.is_some();
        if self.available == Some(available) {
            return;
        }
        self.available = Some(available);
        if available {
            info!(
                changes = self.resolutions(),
                unattributed = self.unattributed_count(),
                "Windows capture source attribution is available"
            );
        } else {
            // I-9: counts, never the process or the path that could not be
            // opened. An elevated writer cannot be opened by an unelevated
            // daemon, and a non-empty exclusion list then drops its copies.
            warn!(
                changes = self.resolutions(),
                unattributed = self.unattributed_count(),
                "Windows could not identify the source application for clipboard capture"
            );
        }
    }

    pub(super) fn note_excluded(&mut self, attributed: bool) {
        self.excluded += 1;
        if !attributed {
            warn!(
                dropped = self.excluded_count(),
                unattributed = self.unattributed_count(),
                "a clipboard change with no source application was dropped by the exclusion list"
            );
        }
    }

    /// How many times a writer was asked for: one per change.
    pub(super) fn resolutions(&self) -> u64 {
        self.resolutions
    }

    pub(super) fn unattributed_count(&self) -> u64 {
        self.unattributed
    }

    pub(super) fn excluded_count(&self) -> u64 {
        self.excluded
    }
}

/// One exclusion entry, reduced to the identity a Windows process actually has.
///
/// Users type what they see. `Chrome.exe` in Task Manager, `chrome` in a
/// shortcut, and a path pasted from Explorer's address bar all name one
/// process, and an entry that silently matches nothing is an exclusion the user
/// believes is protecting them.
pub(super) fn exclusion_key(entry: &str) -> Option<String> {
    let name = file_name(entry)?;
    let name = name
        .len()
        .checked_sub(4)
        .filter(|&stem| name[stem..].eq_ignore_ascii_case(".exe"))
        .map_or(name, |stem| &name[..stem]);
    (!name.is_empty()).then(|| name.to_lowercase())
}

/// I-7 / manifest 01 §3.9(b): a non-empty list fails closed on a change whose
/// writer could not be identified. An empty list stays open, because failing
/// closed there would disable capture for everyone.
pub(super) fn is_excluded(list: &[String], id: Option<&str>) -> bool {
    if list.is_empty() {
        return false;
    }
    let Some(id) = id.and_then(exclusion_key) else {
        return true;
    };
    list.iter()
        .filter_map(|entry| exclusion_key(entry))
        .any(|entry| entry == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(id: &str) -> SourceApp {
        SourceApp::from_image_path(id).expect("a file name is an image path")
    }

    /// DMY-158. The poll interval is 500 ms and the cache was 750 ms, so this is
    /// two ordinary copies a third of a second apart: the second inherited the
    /// first app, and a password manager's copy reached ingest as ordinary text
    /// from Notepad — sensitive floor not applied, exclusion list not consulted.
    #[test]
    fn a_second_change_is_attributed_to_its_own_writer_not_the_previous_one() {
        let mut attribution = Attribution::default();
        let ordinary = attribution.for_change(11, || Some(app(r"C:\Windows\notepad.exe")));
        let credential = attribution.for_change(12, || {
            Some(app(
                r"C:\Users\ann\AppData\Local\1Password\app\8\1Password.exe",
            ))
        });

        assert_eq!(ordinary.expect("resolved").id, "notepad.exe");
        let credential = credential.expect("resolved");
        assert_eq!(credential.id, "1password.exe");
        assert!(
            copypaste_core::sensitive::is_password_manager_app(&credential.id),
            "the credential store must reach ingest as itself, or its copy is indexed"
        );
        assert_eq!(attribution.resolutions(), 2);
    }

    /// The cache still exists, and still covers what it was for: one change is
    /// resolved once however many times the poll asks.
    #[test]
    fn one_change_is_resolved_once() {
        let mut attribution = Attribution::default();
        for _ in 0..4 {
            assert_eq!(
                attribution
                    .for_change(7, || Some(app("notepad.exe")))
                    .expect("resolved")
                    .id,
                "notepad.exe"
            );
        }
        assert_eq!(attribution.resolutions(), 1);
    }

    /// Caching failures is what stops a transient failure becoming a syscall
    /// storm within one change; counting them is what makes it visible.
    #[test]
    fn a_failure_is_cached_for_its_own_change_and_counted() {
        let mut attribution = Attribution::default();
        assert!(attribution.for_change(3, || None).is_none());
        assert!(attribution
            .for_change(3, || Some(app("notepad.exe")))
            .is_none());
        assert!(attribution.for_change(4, || None).is_none());
        assert_eq!(attribution.resolutions(), 2);
        assert_eq!(attribution.unattributed_count(), 2);
    }

    /// The identifier is what an item carries and what the UI shows. A path in
    /// it discloses the local user name (I-9).
    #[test]
    fn an_identity_carries_no_path_and_no_case() {
        for path in [
            r"C:\Program Files\Google\Chrome\Application\chrome.exe",
            r"\\?\C:\Program Files\Google\Chrome\Application\CHROME.EXE",
            r"\Device\HarddiskVolume4\Program Files\Google\Chrome\Application\Chrome.exe",
        ] {
            let app = SourceApp::from_image_path(path).expect("a source app");
            assert_eq!(app.id, "chrome.exe", "{path}");
            assert!(!app.id.contains('\\') && !app.id.contains('/'), "{path}");
            assert!(!app.name.contains('\\'), "{path}");
        }
        assert_eq!(
            SourceApp::from_image_path(r"C:\Users\ann\AppData\Local\1Password\1Password.exe")
                .expect("a source app")
                .name,
            "1Password",
            "the display name keeps the case the user recognises"
        );
        assert!(SourceApp::from_image_path("").is_none());
        assert!(SourceApp::from_image_path(r"C:\Program Files\").is_none());
    }

    /// DMY-158: every one of these is the same process, and every one of them
    /// used to be an entry that matched nothing at all.
    #[test]
    fn every_way_a_user_writes_one_process_names_it() {
        let excluded = "chrome.exe";
        for entry in [
            "chrome.exe",
            "Chrome.exe",
            "CHROME.EXE",
            "chrome",
            "Chrome",
            "  chrome.exe  ",
            r"C:\Program Files\Google\Chrome\Application\chrome.exe",
            r#""C:\Program Files\Google\Chrome\Application\chrome.exe""#,
            "C:/Program Files/Google/Chrome/Application/chrome.exe",
        ] {
            assert!(
                is_excluded(&[entry.to_owned()], Some(excluded)),
                "{entry} did not exclude {excluded}"
            );
        }
        for entry in ["", "   ", ".exe", "chromium.exe", "chrome.exe.bak"] {
            assert!(
                !is_excluded(&[entry.to_owned()], Some(excluded)),
                "{entry} excluded {excluded}"
            );
        }
    }

    /// I-7, T-57, T-58.
    #[test]
    fn an_unidentified_writer_is_excluded_only_when_the_list_is_not_empty() {
        assert!(!is_excluded(&[], None));
        assert!(!is_excluded(&[], Some("notepad.exe")));
        assert!(is_excluded(&["chrome".to_owned()], None));
        assert!(!is_excluded(&["chrome".to_owned()], Some("notepad.exe")));
    }

    /// A drop is counted whether or not it was attributable, so "the exclusion
    /// list is eating everything" is answerable from the counts alone.
    #[test]
    fn drops_are_counted_apart_from_the_failures_that_caused_them() {
        let mut attribution = Attribution::default();
        attribution.for_change(1, || None);
        attribution.note(None);
        attribution.note_excluded(false);
        let known = app("notepad.exe");
        attribution.for_change(2, || Some(known.clone()));
        attribution.note(Some(&known));
        attribution.note_excluded(true);
        assert_eq!(attribution.excluded_count(), 2);
        assert_eq!(attribution.unattributed_count(), 1);
    }
}
