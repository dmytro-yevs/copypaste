//! The test/dev backend.
//!
//! ## Deviation from §3.11 (stated, not silent)
//!
//! The manifest says non-macOS is a silent no-op with no polling cost. We keep
//! the *contract* — a default [`FakeClipboard`] never panics, never errors and
//! returns `None` forever — but we deliberately make it drivable, from
//! `COPYPASTE_FAKE_CLIPBOARD` or from `push_external`, so the MVP is
//! demonstrable on the Linux host we develop on and so the acceptance tests
//! become real tests. [`ClipboardSource::backend_name`] is surfaced over IPC
//! (`StatusResponse::clipboard_backend`) so a demo cannot be mistaken for the
//! real thing.
//!
//! It models the same machine the real backend does — a monotonic change
//! counter, one current value, no history — and it runs the *same*
//! [`ChangeTracker`], so a test written here holds for `NSPasteboard`.

use std::path::PathBuf;
use std::time::SystemTime;

use tracing::{debug, warn};

use super::change::{Change, ChangeTracker, SELF_WRITE_DELTA};
use super::{Capture, CapturePolicy, ClipboardSource, MAX_TEXT_BYTES};

/// Environment variable naming a file the [`FakeClipboard`] watches.
const FAKE_CLIPBOARD_ENV: &str = "COPYPASTE_FAKE_CLIPBOARD";

/// Test/dev backend. Polls the file named by `COPYPASTE_FAKE_CLIPBOARD` (if
/// set) and otherwise holds content in memory. Public so tests and the demo can
/// drive it.
pub struct FakeClipboard {
    tracker: ChangeTracker,
    /// The fake's `changeCount`. Bumped by `push_external` (+1), by
    /// `set_contents` (+2, mirroring clear+set) and by an observed external
    /// edit of the watched file (+1).
    change_count: i64,
    /// The single current clipboard value. `None` means "empty clipboard", or
    /// "the last change was dropped by a gate" (I-3: the cursor still moved).
    contents: Option<String>,
    watched: Option<WatchedFile>,
    rejected_too_large: u64,
    frontmost_app: Option<String>,
}

/// The watched-file half of the fake.
struct WatchedFile {
    path: PathBuf,
    /// Cheap stamp so an unchanged file is never re-read — the fake's analogue
    /// of I-1 ("no representation is read when nothing changed").
    last_seen: Option<FileStamp>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FileStamp {
    modified: Option<SystemTime>,
    len: u64,
}

impl FakeClipboard {
    pub fn new() -> Self {
        match std::env::var_os(FAKE_CLIPBOARD_ENV) {
            Some(path) if !path.is_empty() => Self::watching(PathBuf::from(path)),
            _ => Self::in_memory(),
        }
    }

    fn in_memory() -> Self {
        Self {
            tracker: ChangeTracker::new(),
            change_count: 0,
            contents: None,
            watched: None,
            rejected_too_large: 0,
            frontmost_app: None,
        }
    }

    fn watching(path: PathBuf) -> Self {
        Self {
            watched: Some(WatchedFile {
                path,
                last_seen: None,
            }),
            ..Self::in_memory()
        }
    }

    /// Simulate an external app copying something.
    ///
    /// Public API of the port even though the daemon binary usually drives the
    /// fake over IPC instead.
    #[allow(dead_code)]
    pub fn push_external(&mut self, text: &str) {
        self.contents = Some(text.to_string());
        self.change_count += 1;
    }

    /// Drive frontmost-app attribution in cross-platform integration tests.
    #[cfg(test)]
    pub fn set_frontmost_app(&mut self, bundle_id: Option<&str>) {
        self.frontmost_app = bundle_id.map(str::to_owned);
    }

    /// Fold any external edit of the watched file into the change counter.
    ///
    /// Returns without reading anything when the file has not moved, which is
    /// the property I-1 asks of the real backend.
    fn sync_watched_file(&mut self) {
        let Some(watched) = self.watched.as_mut() else {
            return;
        };
        // A missing or unreadable file is simply "no change" — the fake must be
        // a silent no-op when nothing is driving it (§3.11).
        let Ok(meta) = std::fs::metadata(&watched.path) else {
            return;
        };
        let stamp = FileStamp {
            modified: meta.modified().ok(),
            len: meta.len(),
        };
        if watched.last_seen == Some(stamp) {
            return;
        }
        watched.last_seen = Some(stamp);

        // I-18's shape: the size is checked before the bytes are copied in, and
        // the rejection is counted rather than dropped in silence (§6.5).
        if stamp.len > MAX_TEXT_BYTES as u64 {
            self.rejected_too_large += 1;
            warn!(
                bytes = stamp.len,
                cap = MAX_TEXT_BYTES,
                "clipboard text exceeds the size cap; dropped"
            );
            // I-3: acknowledge the change so it is not re-offered forever.
            self.contents = None;
            self.change_count += 1;
            return;
        }

        match std::fs::read(&watched.path) {
            Ok(bytes) => {
                // I-9: byte counts, never content, and never the path.
                let text = String::from_utf8_lossy(&bytes).into_owned();
                // Fake-only affordance: `echo hi > file` is the demo, and the
                // trailing newline is the shell's, not the user's.
                let text = text.strip_suffix('\n').unwrap_or(&text).to_string();
                self.contents = Some(text);
                self.change_count += 1;
            }
            Err(_) => {
                debug!("fake clipboard file could not be read; change ignored");
            }
        }
    }
}

impl Default for FakeClipboard {
    fn default() -> Self {
        Self::new()
    }
}

impl ClipboardSource for FakeClipboard {
    fn poll(&mut self) -> Option<Capture> {
        self.poll_with_policy(CapturePolicy {
            private_mode: false,
            excluded_app_bundle_ids: &[],
            max_item_bytes: MAX_TEXT_BYTES as u64,
        })
    }

    fn poll_with_policy(&mut self, policy: CapturePolicy<'_>) -> Option<Capture> {
        self.sync_watched_file();

        match self.tracker.observe(self.change_count) {
            Change::Unchanged => return None,
            Change::SelfWrite => {
                debug!("suppressed our own clipboard write");
                return None;
            }
            Change::Fresh { lost_intermediates } => {
                if lost_intermediates > 0 {
                    warn!(
                        lost = lost_intermediates,
                        "clipboard burst: intermediate values are irrecoverable"
                    );
                }
            }
        }

        // Private mode and app exclusions acknowledge the change after its
        // cursor was advanced above, but before any content is cloned.
        let app_bundle_id = self.frontmost_app.clone();
        if policy.private_mode
            || (!policy.excluded_app_bundle_ids.is_empty()
                && app_bundle_id.as_ref().is_none_or(|id| {
                    policy
                        .excluded_app_bundle_ids
                        .iter()
                        .any(|excluded| excluded == id)
                }))
        {
            return None;
        }

        // §3.2: whatever the burst arithmetic said, the surviving value is
        // returned. §3.20: no content dedup here — re-copying the same text is
        // a real change and ingest is what collapses it into a recency bump.
        let content = self.contents.clone()?;
        if content.len() as u64 > policy.max_item_bytes {
            self.rejected_too_large += 1;
            return None;
        }
        if content.is_empty() {
            return None;
        }
        Some(Capture {
            content,
            binary_content: None,
            file_path: None,
            file_metadata: None,
            content_type: copypaste_ipc::content_type::TEXT.to_string(),
            app_bundle_id,
        })
    }

    fn changed(&mut self) -> bool {
        self.sync_watched_file();
        !self.tracker.is_current(self.change_count)
    }

    fn set_contents(&mut self, text: &str) -> anyhow::Result<()> {
        let pre = self.change_count;
        // §3.3 step 2, from the count the write will land in — the fake's stand
        // in for what `clearContents` returns.
        self.tracker.sentinel.arm(pre + SELF_WRITE_DELTA);

        if let Some(watched) = self.watched.as_mut() {
            if let Err(err) = std::fs::write(&watched.path, text.as_bytes()) {
                // §3.3 step 5: a failed write must not leave a stale sentinel.
                self.tracker.sentinel.clear();
                // I-9 / CLAUDE.md rule 4: the message carries no path.
                return Err(anyhow::anyhow!(
                    "could not write the fake clipboard file: {}",
                    err.kind()
                ));
            }
            // Our own write must not also be seen as an external edit.
            watched.last_seen = std::fs::metadata(&watched.path).ok().map(|m| FileStamp {
                modified: m.modified().ok(),
                len: m.len(),
            });
        }

        self.contents = Some(text.to_string());
        // Models the OS rather than the sentinel: the count the write lands in
        // is what `clearContents` returned. A fake that derived this from the
        // sentinel's own arithmetic would agree with any belief, which is why
        // no test here could falsify the delta (manifest 01 §3.3).
        self.change_count = pre + SELF_WRITE_DELTA;
        Ok(())
    }

    fn backend_name(&self) -> &'static str {
        if self.watched.is_some() {
            "fake-file"
        } else {
            "fake-memory"
        }
    }

    fn rejected_too_large_count(&self) -> u64 {
        self.rejected_too_large
    }

    fn lost_intermediates_count(&self) -> u64 {
        self.tracker.lost_intermediates
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
//
// These are the acceptance tests of manifest §5 that the port makes reachable.

#[cfg(test)]
mod tests {
    use super::*;

    fn fake() -> FakeClipboard {
        FakeClipboard::in_memory()
    }

    // -- the fake, driven as a clipboard ------------------------------------

    /// T-1 — an idle clipboard is free, and stays quiet however often it is
    /// polled.
    #[test]
    fn no_change_returns_none() {
        let mut cb = fake();
        assert_eq!(cb.poll(), None);
        assert_eq!(cb.poll(), None);
        assert_eq!(cb.poll(), None);
        assert_eq!(cb.backend_name(), "fake-memory");
    }

    /// T-2 — the first change after startup is captured and is not a burst.
    #[test]
    fn push_surfaces_exactly_once() {
        let mut cb = fake();
        cb.push_external("hello");

        let captured = cb.poll().expect("a pushed value must surface");
        assert_eq!(captured.content, "hello");
        assert_eq!(captured.content_type, "text");
        assert_eq!(cb.lost_intermediates_count(), 0);

        // The change was acknowledged, not re-offered.
        assert_eq!(cb.poll(), None);
        assert_eq!(cb.poll(), None);
    }

    /// The probe the capture loop skips work on. It must never answer "no" to
    /// a pending self-write: only `poll` consumes the sentinel, and one left
    /// armed suppresses whichever genuine copy lands on that count.
    #[test]
    fn changed_tracks_poll_and_reports_a_pending_self_write() {
        let mut cb = fake();
        // I-2: startup has not been observed yet, so the first tick happens
        // whatever the clipboard holds. It settles after that one poll.
        assert!(cb.changed());
        cb.poll();
        assert!(!cb.changed(), "an idle clipboard is quiet from then on");

        cb.push_external("something");
        assert!(cb.changed());
        assert!(cb.poll().is_some());
        assert!(!cb.changed(), "poll consumed it");

        cb.set_contents("ours").unwrap();
        assert!(cb.changed(), "a self-write must still reach poll");
        assert!(cb.poll().is_none(), "and poll is what suppresses it");
        assert!(!cb.changed());
    }

    /// T-8 — a self-write is suppressed. T-9 — the suppression is one-shot, so
    /// the next genuine copy is still captured.
    #[test]
    fn set_contents_does_not_self_capture() {
        let mut cb = fake();
        cb.set_contents("pasted by us").expect("in-memory write");

        assert_eq!(cb.poll(), None, "our own write must not be re-captured");
        assert_eq!(cb.poll(), None);

        cb.push_external("copied by a user");
        assert_eq!(
            cb.poll().map(|c| c.content),
            Some("copied by a user".to_string()),
            "suppression must be for exactly one change"
        );
    }

    /// §3.2 / T-5 — the manifest's highest-value rule. Three copies with one
    /// poll: the intermediates are gone (NSPasteboard keeps no history) but the
    /// survivor MUST be captured. v1 advanced the cursor and returned a
    /// burst-only result, so the most recent item was lost permanently.
    #[test]
    fn burst_does_not_eat_the_survivor() {
        let mut cb = fake();
        // Prime the cursor: I-2 means the very first poll is never a burst, so
        // the burst arithmetic is only meaningful on a running daemon.
        cb.push_external("earlier");
        assert_eq!(cb.poll().map(|c| c.content), Some("earlier".to_string()));

        cb.push_external("one");
        cb.push_external("two");
        cb.push_external("three");

        let captured = cb.poll().expect("the surviving value must be captured");
        assert_eq!(captured.content, "three");
        assert_eq!(
            cb.lost_intermediates_count(),
            2,
            "reported as telemetry only"
        );

        // T-6 — normal capture resumes afterwards.
        cb.push_external("after-burst");
        assert_eq!(
            cb.poll().map(|c| c.content),
            Some("after-burst".to_string())
        );
    }

    /// Several rapid pushes, each polled: every one surfaces, in order, exactly
    /// once.
    #[test]
    fn rapid_pushes_each_surface() {
        let mut cb = fake();
        for text in ["a", "b", "c", "d"] {
            cb.push_external(text);
            assert_eq!(cb.poll().map(|c| c.content), Some(text.to_string()));
            assert_eq!(cb.poll(), None);
        }
        assert_eq!(cb.lost_intermediates_count(), 0);
    }

    /// §3.20 — the change-detection layer must not dedup by content. Re-copying
    /// the same text is a real change; ingest is what collapses it into a
    /// recency bump, and it can only do that if it sees it.
    #[test]
    fn recopying_the_same_text_is_re_emitted() {
        let mut cb = fake();
        cb.push_external("same");
        assert_eq!(cb.poll().map(|c| c.content), Some("same".to_string()));
        cb.push_external("same");
        assert_eq!(cb.poll().map(|c| c.content), Some("same".to_string()));
    }

    // -- the fake, driven from a file ---------------------------------------

    #[test]
    fn watched_file_is_captured_once_per_edit() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("clip");
        let mut cb = FakeClipboard::watching(path.clone());
        assert_eq!(cb.backend_name(), "fake-file");

        // A missing file is a silent no-op, never an error (§3.11's contract).
        assert_eq!(cb.poll(), None);

        // The trailing newline `echo` adds is the shell's, not the user's.
        std::fs::write(&path, b"from a file\n").expect("write");
        assert_eq!(
            cb.poll().map(|c| c.content),
            Some("from a file".to_string())
        );
        assert_eq!(cb.poll(), None);

        std::fs::write(&path, b"second").expect("write");
        assert_eq!(cb.poll().map(|c| c.content), Some("second".to_string()));
        assert_eq!(cb.poll(), None);
    }

    /// The write side of the file backend: `set_contents` puts the text where an
    /// external reader can see it, and still suppresses the self-write.
    #[test]
    fn file_backed_set_contents_does_not_self_capture() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("clip");
        let mut cb = FakeClipboard::watching(path.clone());

        cb.set_contents("pasted by us").expect("file write");
        assert_eq!(
            std::fs::read_to_string(&path).expect("read back"),
            "pasted by us"
        );
        assert_eq!(cb.poll(), None);
        assert_eq!(cb.poll(), None);

        std::fs::write(&path, b"a user copy").expect("write");
        assert_eq!(
            cb.poll().map(|c| c.content),
            Some("a user copy".to_string())
        );
    }

    /// I-39 / §6.5 — an oversized value is rejected, counted, and acknowledged
    /// (not re-offered forever). v1 dropped oversized images in complete
    /// silence.
    #[test]
    fn oversized_content_is_rejected_and_counted() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("clip");
        let mut cb = FakeClipboard::watching(path.clone());

        std::fs::write(&path, vec![b'x'; MAX_TEXT_BYTES + 1]).expect("write");
        assert_eq!(cb.poll(), None);
        assert_eq!(cb.rejected_too_large_count(), 1);
        // I-3: acknowledged, so it is not offered again.
        assert_eq!(cb.poll(), None);
        assert_eq!(cb.rejected_too_large_count(), 1);

        std::fs::write(&path, b"small again").expect("write");
        assert_eq!(
            cb.poll().map(|c| c.content),
            Some("small again".to_string())
        );
    }

    #[test]
    fn excluded_and_unknown_apps_are_acknowledged_without_reading_again() {
        let mut cb = fake();
        cb.set_frontmost_app(Some("com.example.excluded"));
        cb.push_external("secret");
        let exclusions = ["com.example.excluded".to_string()];
        assert!(cb
            .poll_with_policy(CapturePolicy {
                private_mode: false,
                excluded_app_bundle_ids: &exclusions,
                max_item_bytes: MAX_TEXT_BYTES as u64,
            })
            .is_none());
        assert!(cb.poll().is_none(), "excluded change was replayed");

        cb.set_frontmost_app(None);
        cb.push_external("unknown source");
        assert!(
            cb.poll_with_policy(CapturePolicy {
                private_mode: false,
                excluded_app_bundle_ids: &exclusions,
                max_item_bytes: MAX_TEXT_BYTES as u64,
            })
            .is_none(),
            "non-empty exclusions fail closed"
        );
        cb.push_external("normal");
        assert_eq!(
            cb.poll().map(|capture| capture.content),
            Some("normal".into())
        );
    }

    #[test]
    fn password_manager_copies_keep_their_origin_for_sensitive_ingest() {
        let mut cb = fake();
        cb.set_frontmost_app(Some("com.1password.1password"));
        cb.push_external("generated-password");
        let capture = cb.poll().expect("the sensitive floor is applied by ingest");
        assert_eq!(
            capture.app_bundle_id.as_deref(),
            Some("com.1password.1password")
        );
    }

    #[test]
    fn private_mode_suppresses_without_replaying_old_copies() {
        let mut cb = fake();
        cb.push_external("while private");
        assert!(cb
            .poll_with_policy(CapturePolicy {
                private_mode: true,
                excluded_app_bundle_ids: &[],
                max_item_bytes: MAX_TEXT_BYTES as u64,
            })
            .is_none());
        assert!(cb.poll().is_none(), "private copy was replayed");
        cb.push_external("after private");
        assert_eq!(
            cb.poll().map(|capture| capture.content),
            Some("after private".into())
        );
    }

    #[test]
    fn live_size_policy_rejects_before_the_capture_is_returned() {
        let mut cb = fake();
        cb.push_external("four");
        assert!(cb
            .poll_with_policy(CapturePolicy {
                private_mode: false,
                excluded_app_bundle_ids: &[],
                max_item_bytes: 3,
            })
            .is_none());
        assert_eq!(cb.rejected_too_large_count(), 1);
        assert!(cb.poll().is_none(), "oversized copy was replayed");
    }
}
