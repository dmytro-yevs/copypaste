//! The Windows backend: the system clipboard and `GetClipboardSequenceNumber`.
//!
//! `arboard`, already in the tree behind Tauri, exposes no sequence number, no
//! arbitrary registered format and no size before read — so polling it means
//! reading the clipboard's *content* every tick, which is what I-1 forbids and
//! what the do-not-record formats exist to prevent. `clipboard-win` is
//! arboard's own Windows layer, one level down, and has all three.
//!
//! The state machine is [`super::change`], covered on every host. The ignored
//! tests below drive the real Windows clipboard and are release-gate tests.

use std::time::Instant;

use clipboard_win::{raw, Clipboard};
use tracing::{debug, warn};

mod attribution;
mod read;
mod transcode;

use super::change::{Change, ChangeTracker};
use super::windows_optout as optout;
use super::{Capture, CapturePolicy, ClipboardSource};
use attribution::SourceApp;
use read::{Reading, Representation};

/// `OpenClipboard` fails while another process holds the clipboard open, which
/// is ordinary on a machine running a second clipboard tool. Retried inside the
/// crate, and a change we still could not open is left unacknowledged so the
/// next tick sees it again — dropping it would lose a copy the user made, which
/// is the outcome AGENTS.md rule 4 ranks worst.
const OPEN_ATTEMPTS: usize = 10;

/// The largest sequence-number delta a write of ours may claim as its own.
///
/// `GetClipboardSequenceNumber` counts representation changes, not logical
/// writes. A real Windows run observed five increments from `set_string` as
/// Windows published and synthesised its text formats. The bound leaves room
/// for additional system-provided representations while still refusing a
/// clearly unrelated burst (CopyPaste-8yzf).
const MAX_SELF_WRITE_DELTA: i64 = 16;

/// Clipboard formats registered once, not once per tick (§3.12's rule in
/// Windows spelling: `RegisterClipboardFormatW` interns a name for the life of
/// the session, so the atom is a process-lifetime constant).
struct OptOutFormats {
    presence: Vec<u32>,
    flags: Vec<u32>,
}

impl OptOutFormats {
    fn register() -> Self {
        Self {
            presence: register_all(&optout::PRESENCE_MARKERS),
            flags: register_all(&optout::FLAG_MARKERS),
        }
    }
}

fn register_all(names: &[&str]) -> Vec<u32> {
    names
        .iter()
        .filter_map(|name| match raw::register_format(name) {
            Some(id) => Some(id.get()),
            None => {
                // Not fatal, and not silent: this build can no longer see one
                // of the ways a password manager says "do not record me".
                warn!(
                    marker = *name,
                    "a clipboard opt-out format could not be registered"
                );
                None
            }
        })
        .collect()
}

pub struct WindowsClipboard {
    tracker: ChangeTracker,
    rejected_too_large: u64,
    opt_out: OptOutFormats,
    source_cache: Option<(Instant, Option<SourceApp>)>,
    attribution_available: Option<bool>,
    sequence_unavailable_logged: bool,
}

impl WindowsClipboard {
    pub fn new() -> std::io::Result<Self> {
        Ok(Self {
            tracker: ChangeTracker::new(),
            rejected_too_large: 0,
            opt_out: OptOutFormats::register(),
            source_cache: None,
            attribution_available: None,
            sequence_unavailable_logged: false,
        })
    }

    /// The change count. `None` means the process cannot read it at all, which
    /// is a broken backend rather than an idle clipboard, so it is said once.
    fn sequence_number(&mut self) -> Option<i64> {
        match raw::seq_num() {
            Some(count) => {
                self.sequence_unavailable_logged = false;
                Some(i64::from(count.get()))
            }
            None => {
                if !self.sequence_unavailable_logged {
                    self.sequence_unavailable_logged = true;
                    warn!("the clipboard sequence number is unavailable; capture is suspended");
                }
                None
            }
        }
    }

    /// The two presence markers, probed with `IsClipboardFormatAvailable`.
    ///
    /// I-5 / §3.4 in Windows spelling, and stricter than the macOS path can be:
    /// this needs neither the clipboard open nor any data read, so a marked
    /// password is never even mapped into this process. A clipboard carrying a
    /// marker *and* ordinary text is dropped entirely; that is intentional.
    fn marked_do_not_record(&self) -> bool {
        self.opt_out
            .presence
            .iter()
            .any(|&format| raw::is_format_avail(format))
    }

    /// The two flag markers. Requires the clipboard open, and reads four bytes
    /// of flag — never a representation.
    fn flag_forbids_capture(&self) -> bool {
        self.opt_out.flags.iter().any(|&format| {
            if !raw::is_format_avail(format) {
                return false;
            }
            let mut payload = Vec::new();
            let read = raw::get_vec(format, &mut payload).is_ok();
            optout::flag_forbids_capture(read.then_some(payload.as_slice()))
        })
    }

    fn reject_too_large(&mut self, bytes: u64, cap: u64) {
        self.rejected_too_large += 1;
        // I-39 / §6.5: counted and readable, not only logged. I-9: sizes, never
        // content and never a format name.
        warn!(
            bytes,
            cap, "clipboard representation exceeds the size cap; dropped"
        );
    }

    /// §3.3 step 2, from an observed count rather than a predicted one.
    ///
    /// [`super::change::SelfWriteSentinel::arm`] records why that asymmetry
    /// matters: a cell armed *above* our write swallows whichever genuine copy
    /// lands there, one at or below it is merely inert. Windows offers no
    /// equivalent of the count `clearContents` returns, so the count is read
    /// back after the clipboard is closed.
    ///
    /// Arming after the write is safe here only because a poll cannot run in
    /// between: `AppState::clipboard` hands out one mutex guard, and the caller
    /// holds it for the whole of `set_contents`. Do not move this behind a
    /// second lock without restoring the ordering.
    fn arm_self_write(&mut self, before: Option<i64>) {
        let Some(after) = self.sequence_number() else {
            // §3.3 step 5: better to re-capture our own paste-back once than to
            // leave a cell armed at a count that may swallow a real copy.
            self.tracker.sentinel.clear();
            return;
        };
        if before.is_some_and(|before| after - before > MAX_SELF_WRITE_DELTA) {
            // Another application wrote during ours, so the change now on the
            // clipboard is theirs. Reported, not corrected: correcting it is
            // what would drop their copy (CopyPaste-8yzf).
            self.tracker.sentinel.clear();
            debug!("another application wrote to the clipboard during our write");
            return;
        }
        self.tracker.sentinel.arm(after);
    }

    fn write(&mut self, set: impl FnOnce() -> clipboard_win::SysResult<()>) -> anyhow::Result<()> {
        let before = self.sequence_number();
        let clipboard = Clipboard::new_attempts(OPEN_ATTEMPTS)
            // I-9 / AGENTS.md rule 4: no paths, no content, no user name.
            .map_err(|_| anyhow::anyhow!("the clipboard could not be opened"))?;
        let written = set();
        drop(clipboard);
        // The sequence number is read after `CloseClipboard`, which is where
        // Windows has finished publishing the change.
        if written.is_err() {
            // Deliberately no `sentinel.clear()` here. The sentinel is armed
            // only on success, so there is nothing of ours to undo, and
            // clearing would disarm an *earlier* paste-back that has not been
            // polled yet — which is how a genuine copy gets swallowed.
            return Err(anyhow::anyhow!("the clipboard rejected the write"));
        }
        self.arm_self_write(before);
        Ok(())
    }
}

impl ClipboardSource for WindowsClipboard {
    fn poll(&mut self) -> Option<Capture> {
        let settings = copypaste_ipc::ConfigData::default();
        self.poll_with_policy(CapturePolicy::new(&settings))
    }

    fn poll_with_policy(&mut self, policy: CapturePolicy<'_>) -> Option<Capture> {
        // I-1: one syscall and no allocation on an unchanged clipboard, which
        // is neither opened nor probed.
        let count = self.sequence_number()?;
        if self.tracker.is_current(count) {
            return None;
        }

        if self.marked_do_not_record() {
            // I-9: no content, no format name.
            debug!(
                change_count = count,
                "clipboard change is marked do-not-record; dropped"
            );
            // I-3: acknowledged, or it is re-offered forever. This also
            // consumes the sentinel if the marked change was somehow ours.
            self.tracker.observe(count);
            return None;
        }

        // Everything below reads from the clipboard, so it must be open first.
        // The cursor has deliberately not moved yet: a clipboard we could not
        // open is a change we have not seen.
        let Ok(clipboard) = Clipboard::new_attempts(OPEN_ATTEMPTS) else {
            debug!(
                change_count = count,
                "the clipboard could not be opened; the change is retried"
            );
            return None;
        };

        match self.tracker.observe(count) {
            Change::Unchanged => return None,
            Change::SelfWrite => {
                debug!(change_count = count, "suppressed our own clipboard write");
                return None;
            }
            Change::Fresh { lost_intermediates } => {
                if lost_intermediates > 0 {
                    // §3.1/§3.2: telemetry only. The clipboard keeps no
                    // history, so the intermediates are gone — but the
                    // surviving value is captured below, never replaced by a
                    // "burst happened" result.
                    warn!(
                        lost = lost_intermediates,
                        change_count = count,
                        "clipboard burst: intermediate values are irrecoverable"
                    );
                }
            }
        }

        if self.flag_forbids_capture() {
            debug!(
                change_count = count,
                "clipboard change is flagged do-not-record; dropped"
            );
            return None;
        }

        // Private mode is a capture gate, not an ingest choice: acknowledge the
        // change without reading attribution or data.
        if policy.settings.private_mode {
            return None;
        }

        let source_app = self.source_app();
        self.note_attribution(source_app.as_ref());
        let app_bundle_id = source_app.as_ref().map(|app| app.id.clone());
        let app_name = source_app.map(|app| app.name);
        // A non-empty exclusion list fails closed when the source cannot be
        // resolved; with an empty list the same `None` is safe because content
        // detection remains active.
        if !policy.settings.excluded_app_bundle_ids.is_empty()
            && app_bundle_id.as_ref().is_none_or(|id| {
                policy
                    .settings
                    .excluded_app_bundle_ids
                    .iter()
                    .any(|excluded| excluded == id)
            })
        {
            return None;
        }

        let reading = read::representation(policy);
        // Holding this open blocks every other application's copy and paste.
        drop(clipboard);

        let representation = match reading {
            Reading::Got(representation) => representation,
            Reading::TooLarge { bytes, cap } => {
                self.reject_too_large(bytes, cap);
                return None;
            }
            Reading::Nothing => return None,
        };
        let Representation::Text(content) = representation;
        Some(Capture {
            content,
            binary_content: None,
            file_path: None,
            file_metadata: None,
            content_type: copypaste_ipc::content_type::TEXT.to_string(),
            app_bundle_id,
            app_name,
        })
    }

    fn changed(&mut self) -> bool {
        match self.sequence_number() {
            Some(count) => !self.tracker.is_current(count),
            None => false,
        }
    }

    fn set_contents(&mut self, text: &str) -> anyhow::Result<()> {
        self.write(|| raw::set_string(text))
    }

    fn set_binary_contents(
        &mut self,
        _item_id: &str,
        content_type: &str,
        bytes: &[u8],
        _metadata: Option<&copypaste_core::FileMetadata>,
    ) -> anyhow::Result<()> {
        if content_type == copypaste_ipc::content_type::FILE {
            // File paste-back needs a managed plaintext staging directory.
            return Err(anyhow::anyhow!(
                "this build cannot paste a file back on Windows"
            ));
        }
        if !matches!(
            copypaste_ipc::content_type::classify(content_type),
            copypaste_ipc::content_type::Kind::Image
        ) {
            return Err(anyhow::anyhow!(
                "the clipboard cannot write this binary content type"
            ));
        }
        // The clipboard's own image format is a bitmap; PNG on the clipboard is
        // a convention some applications follow and many do not.
        let bitmap = transcode::to_bitmap(bytes, copypaste_ipc::MAX_DECODED_IMAGE_MB)
            .ok_or_else(|| anyhow::anyhow!("the image could not be converted for the clipboard"))?;
        self.write(move || {
            // `set_bitmap` does not clear, so the previous owner's other
            // representations would survive beside ours.
            raw::empty()?;
            raw::set_bitmap(&bitmap)
        })
    }

    fn backend_name(&self) -> &'static str {
        // Contains "system" deliberately: the app decides whether a backend is
        // real by matching `/pasteboard|nspasteboard|system/i`, and a name it
        // does not recognise puts a "this is not the system clipboard" warning
        // in front of every Windows user.
        "windows-system-clipboard"
    }

    fn rejected_too_large_count(&self) -> u64 {
        self.rejected_too_large
    }

    fn lost_intermediates_count(&self) -> u64 {
        self.tracker.lost_intermediates
    }
}

// Tests — the half of manifest 01 that only Windows can answer
//
// `change` and `windows_optout` are testable anywhere. What is not is whether
// the sequence number moves at all, how far one write of ours moves it, and
// whether `IsClipboardFormatAvailable` sees what a password manager registers.
// The first two wrong break §3.3 in the two ways `change` records.
//
// `#[ignore]`: these drive the machine's real clipboard, so a run replaces
// whatever the user had copied. Serialised by a mutex here as well as
// `--test-threads=1`, because the clipboard is a process-global singleton.

#[cfg(test)]
mod tests {
    use std::sync::{Mutex, MutexGuard};

    use clipboard_win::formats;

    use super::*;
    use crate::clipboard::MAX_CAPTURE_BYTES;

    static CLIPBOARD: Mutex<()> = Mutex::new(());

    fn serialised() -> MutexGuard<'static, ()> {
        CLIPBOARD.lock().unwrap_or_else(|held| held.into_inner())
    }

    fn sequence_number() -> i64 {
        i64::from(
            raw::seq_num()
                .expect("the sequence number is unavailable")
                .get(),
        )
    }

    /// `CF_UNICODETEXT` as another application writes it: UTF-16, NUL
    /// terminated, little endian.
    fn utf16(text: &str) -> Vec<u8> {
        text.encode_utf16()
            .chain(Some(0))
            .flat_map(u16::to_le_bytes)
            .collect()
    }

    /// What another application does: open, empty, write its representations.
    fn write_formats(reps: &[(u32, &[u8])]) {
        let _clipboard = Clipboard::new_attempts(OPEN_ATTEMPTS).expect("open the clipboard");
        raw::empty().expect("empty the clipboard");
        for &(format, bytes) in reps {
            raw::set_without_clear(format, bytes).expect("set a test representation");
        }
    }

    fn write_text(text: &str) {
        write_formats(&[(formats::CF_UNICODETEXT, &utf16(text))]);
        assert!(
            raw::is_format_avail(formats::CF_UNICODETEXT),
            "the clipboard refused a test write"
        );
    }

    /// T-21, T-4, I-1, I-2. Also the first question of all: does the sequence
    /// number move on a headless runner?
    #[test]
    #[ignore = "drives the real Windows clipboard"]
    fn a_change_by_another_app_is_captured_exactly_once() {
        let _lock = serialised();
        let mut clipboard = WindowsClipboard::new().unwrap();

        let before = sequence_number();
        write_text("copypaste — a check ✓");
        let after = sequence_number();
        assert!(
            after > before,
            "the sequence number did not move for a write ({before} -> {after}); \
             nothing in this backend can work if it does not"
        );

        let capture = clipboard.poll().expect("the write must be captured");
        assert_eq!(capture.content, "copypaste — a check ✓");
        assert_eq!(capture.content_type, copypaste_ipc::content_type::TEXT);
        assert_eq!(
            clipboard.lost_intermediates_count(),
            0,
            "I-2: a first poll at an arbitrary sequence number is not a burst"
        );
        assert!(
            clipboard.poll().is_none(),
            "T-4: an unchanged clipboard must yield nothing"
        );
    }

    #[test]
    #[ignore = "drives the real Windows clipboard"]
    fn non_text_only_changes_are_acknowledged_without_capture() {
        let _lock = serialised();
        let mut clipboard = WindowsClipboard::new().unwrap();

        write_formats(&[(formats::CF_DIB, b"not a decoded image")]);
        assert!(clipboard.poll().is_none());
        assert!(clipboard.poll().is_none(), "the cursor must still advance");

        write_formats(&[
            (formats::CF_UNICODETEXT, &utf16("plain fallback")),
            (formats::CF_DIB, b"ignored"),
        ]);
        let capture = clipboard.poll().expect("plain text must win when offered");
        assert_eq!(capture.content, "plain fallback");
        assert_eq!(capture.content_type, copypaste_ipc::content_type::TEXT);
    }

    /// The measurement `MAX_SELF_WRITE_DELTA` is a guess about. A delta beyond
    /// it means every paste-back is treated as somebody else's change and comes
    /// back as a capture — the duplicate-on-copy bug, in slow motion.
    #[test]
    #[ignore = "drives the real Windows clipboard"]
    fn a_self_write_moves_the_sequence_number_within_the_claimable_delta() {
        let _lock = serialised();
        let mut clipboard = WindowsClipboard::new().unwrap();
        write_text("something copied earlier");
        let _ = clipboard.poll();

        let before = sequence_number();
        clipboard
            .set_contents("pasted by us")
            .expect("the write failed");
        let after = sequence_number();
        assert!(
            after - before <= MAX_SELF_WRITE_DELTA,
            "one write moved the sequence number {before} -> {after}; \
             MAX_SELF_WRITE_DELTA is {MAX_SELF_WRITE_DELTA}, so no paste-back is ever suppressed"
        );
    }

    /// T-8, T-9 and the Fix-4 / "DUP-ON-COPY" pair, asserted as behaviour: our
    /// own write must not come back, and the next genuine copy must.
    #[test]
    #[ignore = "drives the real Windows clipboard"]
    fn our_own_write_is_suppressed_and_the_next_genuine_copy_is_not() {
        let _lock = serialised();
        let mut clipboard = WindowsClipboard::new().unwrap();
        write_text("something copied earlier");
        let _ = clipboard.poll();

        clipboard
            .set_contents("pasted by us")
            .expect("the write failed");
        if let Some(capture) = clipboard.poll() {
            panic!(
                "T-8: our own paste-back came back as a capture ({:?}) — this is the \
                 duplicate-on-copy bug, and the sentinel is now armed at a count no \
                 write of ours will reach",
                capture.content
            );
        }

        write_text("a genuine copy");
        assert_eq!(
            clipboard
                .poll()
                .expect(
                    "T-9: a genuine copy was swallowed — a sentinel that was never \
                     consumed suppresses whichever real copy lands on it"
                )
                .content,
            "a genuine copy"
        );
    }

    /// §3.2, the manifest's highest-value rule: a burst must never be returned
    /// *instead of* the surviving value. T-5, T-6.
    #[test]
    #[ignore = "drives the real Windows clipboard"]
    fn a_burst_reports_its_losses_and_still_returns_the_survivor() {
        let _lock = serialised();
        let mut clipboard = WindowsClipboard::new().unwrap();
        write_text("seed");
        let _ = clipboard.poll();

        write_text("one");
        write_text("two");
        write_text("latest");

        let capture = clipboard
            .poll()
            .expect("§3.2: the surviving clipboard value must still be captured");
        assert_eq!(capture.content, "latest");
        let lost = clipboard.lost_intermediates_count();
        assert!(lost > 0, "the burst must be reported as telemetry");

        write_text("after-burst");
        assert_eq!(
            clipboard
                .poll()
                .expect("T-6: normal capture resumes")
                .content,
            "after-burst"
        );
        assert!(clipboard.lost_intermediates_count() >= lost);
    }

    /// I-5, T-14, T-15, T-16 in Windows spelling. The marker is written
    /// *alongside* ordinary text, which is the shape a password manager
    /// produces and the case that must still be dropped entirely.
    #[test]
    #[ignore = "drives the real Windows clipboard"]
    fn every_presence_marker_drops_the_change_and_advances_the_cursor() {
        let _lock = serialised();
        let mut clipboard = WindowsClipboard::new().unwrap();

        for name in optout::PRESENCE_MARKERS {
            let format = raw::register_format(name)
                .expect("register the marker")
                .get();
            let text = utf16("a master password");
            write_formats(&[(formats::CF_UNICODETEXT, &text), (format, &[0])]);
            assert!(
                raw::is_format_avail(format),
                "{name} could not be put on the clipboard"
            );
            assert!(
                clipboard.poll().is_none(),
                "{name} did not suppress the change"
            );
            assert!(
                clipboard.poll().is_none(),
                "I-3: {name} left the change to be re-offered forever"
            );
        }

        write_text("an ordinary copy");
        assert_eq!(
            clipboard
                .poll()
                .expect("a marked change must not wedge the poll")
                .content,
            "an ordinary copy"
        );
    }

    /// The other half of the ruleset, where presence is not the signal: the
    /// same format permits capture at 1 and forbids it at 0.
    #[test]
    #[ignore = "drives the real Windows clipboard"]
    fn a_zero_flag_drops_the_change_and_a_one_flag_does_not() {
        let _lock = serialised();
        let mut clipboard = WindowsClipboard::new().unwrap();

        for name in optout::FLAG_MARKERS {
            let format = raw::register_format(name)
                .expect("register the marker")
                .get();
            let text = utf16("a master password");
            write_formats(&[
                (formats::CF_UNICODETEXT, &text),
                (format, &0u32.to_le_bytes()),
            ]);
            assert!(
                clipboard.poll().is_none(),
                "{name} = 0 did not suppress the change"
            );

            write_formats(&[
                (formats::CF_UNICODETEXT, &text),
                (format, &1u32.to_le_bytes()),
            ]);
            assert!(
                clipboard.poll().is_some(),
                "{name} = 1 permits capture and must not drop the change"
            );
        }
    }

    /// I-18, I-39, T-30, T-33. §6.5 is why the counter is asserted and not just
    /// the rejection: v1 wired a counter nothing read.
    #[test]
    #[ignore = "drives the real Windows clipboard"]
    fn text_over_the_cap_is_rejected_and_counted_and_the_boundary_is_kept() {
        let _lock = serialised();
        let mut clipboard = WindowsClipboard::new().unwrap();

        write_text(&"a".repeat(MAX_CAPTURE_BYTES + 1));
        assert!(clipboard.poll().is_none(), "T-30: over the cap by one byte");
        assert_eq!(
            clipboard.rejected_too_large_count(),
            1,
            "I-39: a rejection must be counted, not only logged"
        );

        write_text(&"a".repeat(MAX_CAPTURE_BYTES));
        let capture = clipboard.poll().expect("T-33: len == cap is accepted");
        assert_eq!(capture.content.len(), MAX_CAPTURE_BYTES);
        assert_eq!(clipboard.rejected_too_large_count(), 1);
    }

    /// An empty clipboard is a change like any other: nothing to capture, and
    /// the cursor still moves (I-3).
    #[test]
    #[ignore = "drives the real Windows clipboard"]
    fn a_cleared_clipboard_yields_nothing_and_keeps_polling() {
        let _lock = serialised();
        let mut clipboard = WindowsClipboard::new().unwrap();
        write_text("before the clear");
        let _ = clipboard.poll();

        write_formats(&[]);
        assert!(clipboard.poll().is_none());
        assert!(clipboard.poll().is_none());

        write_text("after the clear");
        assert_eq!(
            clipboard.poll().expect("capture must resume").content,
            "after the clear"
        );
    }

    /// Manifest 07's signal: the origin is what makes a capture sensitive, and
    /// it must survive the poll for ingest to apply the floor.
    ///
    /// A test write owns no clipboard window, so this exercises the foreground
    /// fallback and needs an interactive desktop — with no foreground window
    /// there is nothing to attribute to and it fails rather than passing blind.
    #[test]
    #[ignore = "drives the real Windows clipboard"]
    fn a_capture_carries_the_process_that_owns_the_clipboard() {
        let _lock = serialised();
        let mut clipboard = WindowsClipboard::new().unwrap();
        write_text("attributed");

        let capture = clipboard.poll().expect("the write must be captured");
        let id = capture
            .app_bundle_id
            .expect("no source application resolved");
        assert!(
            id.ends_with(".exe"),
            "expected a process image name, got {id:?}"
        );
        assert_eq!(id, id.to_lowercase(), "the identifier must be canonical");
        assert!(
            !id.contains('\\') && !id.contains('/'),
            "I-9: the source identifier must not carry a path"
        );
    }
}
