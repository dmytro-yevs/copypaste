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

use clipboard_win::{raw, Clipboard};
use tracing::{debug, warn};

mod attribution;
mod read;
mod staging;
mod transcode;
#[cfg(test)]
mod two_writers;

use super::change::{Change, ChangeTracker};
use super::windows_attribution::{is_excluded, Attribution};
use super::{Capture, CapturePolicy, ClipboardSource};
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

pub struct WindowsClipboard {
    tracker: ChangeTracker,
    rejected_too_large: u64,
    formats: read::RegisteredFormats,
    staging: Option<staging::StagingArea>,
    attribution: Attribution,
    sequence_unavailable_logged: bool,
}

impl WindowsClipboard {
    pub fn new() -> std::io::Result<Self> {
        Ok(Self {
            tracker: ChangeTracker::new(),
            rejected_too_large: 0,
            formats: read::RegisteredFormats::register(),
            staging: None,
            attribution: Attribution::default(),
            sequence_unavailable_logged: false,
        })
    }

    pub fn with_data_dir(data_dir: &std::path::Path) -> std::io::Result<Self> {
        let mut clipboard = Self::new()?;
        clipboard.staging = Some(staging::StagingArea::new(data_dir)?);
        Ok(clipboard)
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

    fn reject_too_large(&mut self, bytes: u64, cap: u64) {
        self.rejected_too_large += 1;
        // I-39 / §6.5: counted and readable, not only logged. I-9: sizes, never
        // content and never a format name.
        warn!(
            bytes,
            cap, "clipboard representation exceeds the size cap; dropped"
        );
    }

    fn reject_decoded_too_large(&mut self, megabytes: u32) {
        self.rejected_too_large += 1;
        // A decoder limit proves the allocation exceeds the configured budget,
        // but it cannot safely report an exact decoded length before decoding.
        warn!(
            megabytes,
            "clipboard image exceeds the decoded-memory cap; dropped"
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

        // Private mode is a capture gate, not an ingest choice: acknowledge the
        // change without reading attribution or data.
        if policy.settings.private_mode {
            return None;
        }

        // Resolve attribution on every change so explicit app exclusions are
        // applied consistently.
        let source_app = self.source_app(count);
        self.attribution.note(source_app.as_ref());
        let app_bundle_id = source_app.as_ref().map(|app| app.id.clone());
        let app_name = source_app.map(|app| app.name);
        if is_excluded(
            &policy.settings.excluded_app_bundle_ids,
            app_bundle_id.as_deref(),
        ) {
            self.attribution.note_excluded(app_bundle_id.is_some());
            return None;
        }

        let reading = read::representation(policy, &self.formats);
        // Holding this open blocks every other application's copy and paste.
        drop(clipboard);

        let representation = match reading {
            Reading::Got(representation) => representation,
            Reading::TooLarge { bytes, cap } => {
                self.reject_too_large(bytes, cap);
                return None;
            }
            Reading::DecodedTooLarge { megabytes } => {
                self.reject_decoded_too_large(megabytes);
                return None;
            }
            Reading::Nothing => return None,
        };
        match representation {
            Representation::Text {
                content,
                content_type,
            } => Some(Capture {
                content,
                binary_content: None,
                file_path: None,
                file_metadata: None,
                content_type: content_type.to_string(),
                app_bundle_id,
                app_name,
                source_policy: super::SourcePolicyEvidence::Legacy,
            }),
            Representation::Image {
                bytes,
                content_type,
            } => Some(Capture {
                content: String::new(),
                binary_content: Some(bytes),
                file_path: None,
                file_metadata: None,
                content_type: content_type.to_string(),
                app_bundle_id,
                app_name,
                source_policy: super::SourcePolicyEvidence::Legacy,
            }),
            Representation::File { path, metadata } => Some(Capture {
                content: String::new(),
                binary_content: None,
                file_path: Some(path),
                file_metadata: Some(metadata),
                content_type: copypaste_ipc::content_type::FILE.to_string(),
                app_bundle_id,
                app_name,
                source_policy: super::SourcePolicyEvidence::Legacy,
            }),
        }
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
        metadata: Option<&copypaste_core::FileMetadata>,
    ) -> Result<(), copypaste_core::ClipboardWriteError> {
        use copypaste_core::ClipboardWriteError;

        if matches!(
            content_type,
            copypaste_ipc::content_type::HTML | copypaste_ipc::content_type::RICH_TEXT
        ) {
            let name = if content_type == copypaste_ipc::content_type::HTML {
                "HTML Format"
            } else {
                "Rich Text Format"
            };
            let html = if content_type == copypaste_ipc::content_type::HTML {
                Some(std::str::from_utf8(bytes).map_err(|_| ClipboardWriteError::Failed)?)
            } else {
                None
            };
            let format = raw::register_format(name)
                .ok_or(ClipboardWriteError::Failed)?
                .get();
            return self
                .write(|| {
                    raw::empty()?;
                    if let Some(html) = html {
                        raw::set_html(format, html)
                    } else {
                        raw::set_without_clear(format, bytes)
                    }
                })
                .map_err(|_| ClipboardWriteError::Failed);
        }
        if content_type == copypaste_ipc::content_type::FILE {
            let metadata = metadata.ok_or(ClipboardWriteError::Failed)?;
            let staging = self.staging.as_ref().ok_or(ClipboardWriteError::Failed)?;
            let path = staging
                .materialize(bytes, metadata)
                .map_err(|_| ClipboardWriteError::Failed)?;
            let path = path.to_string_lossy().into_owned();
            return self
                .write(move || {
                    raw::empty()?;
                    raw::set_file_list(&[path])
                })
                .map_err(|_| ClipboardWriteError::Failed);
        }
        if !matches!(
            copypaste_ipc::content_type::classify(content_type),
            copypaste_ipc::ContentClass::Image
        ) {
            return Err(ClipboardWriteError::UnsupportedContent);
        }
        // The clipboard's own image format is a bitmap; PNG on the clipboard is
        // a convention some applications follow and many do not.
        let bitmap = transcode::to_bitmap(bytes, copypaste_ipc::MAX_DECODED_IMAGE_MB)
            .ok_or(ClipboardWriteError::Failed)?;
        self.write(move || {
            // `set_bitmap` does not clear, so the previous owner's other
            // representations would survive beside ours.
            raw::empty()?;
            raw::set_bitmap(&bitmap)
        })
        .map_err(|_| ClipboardWriteError::Failed)
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
// `change` and attribution are testable anywhere. What is not is whether
// the sequence number moves at all, how far one write of ours moves it, and
// The first two wrong break §3.3 in the two ways `change` records.
//
// `#[ignore]`: these drive the machine's real clipboard, so a run replaces
// whatever the user had copied. Serialised by a mutex here as well as
// `--test-threads=1`, because the clipboard is a process-global singleton.

#[cfg(test)]
mod tests {
    use std::sync::{Mutex, MutexGuard};

    use clipboard_win::formats;
    use image::{DynamicImage, ImageBuffer, ImageFormat, Rgb};

    use super::*;
    use crate::clipboard::MAX_CAPTURE_BYTES;

    static CLIPBOARD: Mutex<()> = Mutex::new(());

    pub(super) fn serialised() -> MutexGuard<'static, ()> {
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

    fn png(width: u32, height: u32) -> Vec<u8> {
        encoded_image(width, height, ImageFormat::Png)
    }

    fn tiff(width: u32, height: u32) -> Vec<u8> {
        encoded_image(width, height, ImageFormat::Tiff)
    }

    fn encoded_image(width: u32, height: u32, format: ImageFormat) -> Vec<u8> {
        let image = ImageBuffer::from_pixel(width, height, Rgb([0x24u8, 0x65, 0xa8]));
        let mut bytes = Vec::new();
        DynamicImage::ImageRgb8(image)
            .write_to(&mut std::io::Cursor::new(&mut bytes), format)
            .unwrap();
        bytes
    }

    fn png_format() -> u32 {
        raw::register_format("PNG")
            .expect("register the PNG format")
            .get()
    }

    fn rtf_format() -> u32 {
        raw::register_format("Rich Text Format")
            .expect("register the RTF format")
            .get()
    }

    fn tiff_format() -> u32 {
        raw::register_format("TIFF")
            .expect("register the TIFF format")
            .get()
    }

    fn html_format() -> u32 {
        raw::register_format("HTML Format")
            .expect("register the HTML format")
            .get()
    }

    fn write_file_list(paths: &[&str]) {
        let _clipboard = Clipboard::new_attempts(OPEN_ATTEMPTS).expect("open the clipboard");
        raw::empty().expect("empty the clipboard");
        raw::set_file_list(paths).expect("set a test file list");
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

    #[test]
    #[ignore = "drives the real Windows clipboard"]
    fn an_image_only_change_captures_a_registered_png() {
        let _lock = serialised();
        let mut clipboard = WindowsClipboard::new().unwrap();
        let image = png(8, 8);

        write_formats(&[(png_format(), &image)]);
        let capture = clipboard.poll().expect("a registered PNG must be captured");
        assert_eq!(capture.content, "");
        assert_eq!(capture.content_type, copypaste_ipc::content_type::IMAGE_PNG);
        assert_eq!(capture.binary_content.as_deref(), Some(image.as_slice()));
    }

    #[test]
    #[ignore = "drives the real Windows clipboard"]
    fn text_precedes_png_and_a_registered_png_precedes_a_dib() {
        let _lock = serialised();
        let mut clipboard = WindowsClipboard::new().unwrap();
        let image = png(8, 8);
        let bitmap = transcode::to_bitmap(&image, copypaste_ipc::MAX_DECODED_IMAGE_MB)
            .expect("the test PNG becomes a DIB");
        let text = utf16("plain fallback");

        write_formats(&[
            (formats::CF_UNICODETEXT, &text),
            (png_format(), &image),
            (formats::CF_DIB, &bitmap[14..]),
        ]);
        let capture = clipboard.poll().expect("text must win");
        assert_eq!(capture.content, "plain fallback");
        assert_eq!(capture.binary_content, None);

        write_formats(&[(png_format(), &image), (formats::CF_DIB, &bitmap[14..])]);
        let capture = clipboard.poll().expect("PNG must win over DIB");
        assert_eq!(capture.binary_content.as_deref(), Some(image.as_slice()));
    }

    #[test]
    #[ignore = "drives the real Windows clipboard"]
    fn a_native_dib_is_captured_as_a_bounded_png() {
        let _lock = serialised();
        let mut clipboard = WindowsClipboard::new().unwrap();
        let image = png(8, 8);
        let bitmap = transcode::to_bitmap(&image, copypaste_ipc::MAX_DECODED_IMAGE_MB)
            .expect("the test PNG becomes a DIB");

        write_formats(&[(formats::CF_DIB, &bitmap[14..])]);
        let capture = clipboard.poll().expect("a native DIB must be captured");
        assert_eq!(capture.content_type, copypaste_ipc::content_type::IMAGE_PNG);
        let captured = capture.binary_content.expect("the DIB becomes binary PNG");
        assert!(captured.starts_with(b"\x89PNG"));
    }

    #[test]
    #[ignore = "drives the real Windows clipboard"]
    fn a_registered_tiff_keeps_its_image_type() {
        let _lock = serialised();
        let mut clipboard = WindowsClipboard::new().unwrap();
        let image = tiff(8, 8);

        write_formats(&[(tiff_format(), &image)]);
        let capture = clipboard
            .poll()
            .expect("a registered TIFF must be captured");
        assert_eq!(
            capture.content_type,
            copypaste_ipc::content_type::IMAGE_TIFF
        );
        assert_eq!(capture.binary_content.as_deref(), Some(image.as_slice()));
    }

    #[test]
    #[ignore = "drives the real Windows clipboard"]
    fn a_dib_over_the_decoded_cap_is_rejected_and_counted() {
        let _lock = serialised();
        let mut clipboard = WindowsClipboard::new().unwrap();
        let bitmap = transcode::to_bitmap(&png(1024, 1024), copypaste_ipc::MAX_DECODED_IMAGE_MB)
            .expect("the test PNG becomes a DIB");
        let settings = copypaste_ipc::ConfigData {
            max_decoded_image_mb: copypaste_ipc::MIN_DECODED_IMAGE_MB,
            ..Default::default()
        };

        write_formats(&[(formats::CF_DIB, &bitmap[14..])]);
        assert!(clipboard
            .poll_with_policy(CapturePolicy::new(&settings))
            .is_none());
        assert_eq!(clipboard.rejected_too_large_count(), 1);
    }

    #[test]
    #[ignore = "drives the real Windows clipboard"]
    fn rich_text_and_html_are_text_fallbacks_after_unicode_text() {
        let _lock = serialised();
        let mut clipboard = WindowsClipboard::new().unwrap();
        let rtf = br"{\rtf1\ansi formatted}";

        write_formats(&[(rtf_format(), rtf)]);
        let capture = clipboard.poll().expect("RTF must be captured without text");
        assert_eq!(capture.content, "{\\rtf1\\ansi formatted}");
        assert_eq!(capture.content_type, copypaste_ipc::content_type::RICH_TEXT);

        {
            let _clipboard = Clipboard::new_attempts(OPEN_ATTEMPTS).expect("open the clipboard");
            raw::empty().expect("empty the clipboard");
            raw::set_html(html_format(), "<strong>formatted</strong>").expect("set test HTML");
        }
        let capture = clipboard
            .poll()
            .expect("HTML must be captured without text");
        assert_eq!(capture.content, "<strong>formatted</strong>");
        assert_eq!(capture.content_type, copypaste_ipc::content_type::HTML);

        {
            let _clipboard = Clipboard::new_attempts(OPEN_ATTEMPTS).expect("open the clipboard");
            let mut payload = Vec::new();
            raw::get_vec(html_format(), &mut payload).expect("read test HTML allocation");
            payload.extend_from_slice(&[0, 0xff, 0xfe]);
            raw::empty().expect("empty the clipboard");
            raw::set_without_clear(html_format(), &payload).expect("set padded test HTML");
        }
        let capture = clipboard
            .poll()
            .expect("HTML allocation padding must be ignored");
        assert_eq!(capture.content, "<strong>formatted</strong>");
        assert_eq!(capture.content_type, copypaste_ipc::content_type::HTML);

        let text = utf16("plain fallback");
        write_formats(&[(formats::CF_UNICODETEXT, &text), (rtf_format(), rtf)]);
        let capture = clipboard.poll().expect("Unicode text must win");
        assert_eq!(capture.content, "plain fallback");
        assert_eq!(capture.content_type, copypaste_ipc::content_type::TEXT);
    }

    #[test]
    #[ignore = "drives the real Windows clipboard"]
    fn one_file_reference_is_captured_and_multiple_references_are_dropped() {
        let _lock = serialised();
        let mut clipboard = WindowsClipboard::new().unwrap();

        write_file_list(&[r"C:\copypaste-fixture.txt"]);
        let capture = clipboard
            .poll()
            .expect("one file reference must be captured");
        assert_eq!(capture.content_type, copypaste_ipc::content_type::FILE);
        assert_eq!(capture.content, "");
        assert!(capture.binary_content.is_none());
        assert_eq!(
            capture
                .file_metadata
                .as_ref()
                .map(|metadata| metadata.filename.as_str()),
            Some("copypaste-fixture.txt")
        );
        assert_eq!(
            capture
                .file_metadata
                .as_ref()
                .and_then(|metadata| metadata.source_reference.as_deref()),
            Some(r"C:\copypaste-fixture.txt")
        );
        assert!(capture
            .file_path
            .as_ref()
            .is_some_and(|path| path.is_absolute()));

        write_file_list(&[r"C:\one.txt", r"C:\two.txt"]);
        assert!(
            clipboard.poll().is_none(),
            "multiple file references are unsupported"
        );

        write_file_list(&[r"\\server\share\remote.txt"]);
        assert!(
            clipboard.poll().is_none(),
            "UNC paths are not local captures"
        );

        write_file_list(&[r"\\?\UNC\server\share\remote.txt"]);
        assert!(
            clipboard.poll().is_none(),
            "verbatim UNC paths are not local captures"
        );
    }

    #[test]
    #[ignore = "drives the real Windows clipboard"]
    fn a_file_paste_back_materializes_one_owner_only_file_list_entry() {
        let _lock = serialised();
        let data_dir = tempfile::tempdir().unwrap();
        let mut clipboard = WindowsClipboard::with_data_dir(data_dir.path()).unwrap();
        let metadata =
            copypaste_core::FileMetadata::new("fixture.txt", "application/octet-stream").unwrap();

        clipboard
            .set_binary_contents(
                "item-id",
                copypaste_ipc::content_type::FILE,
                b"synthetic bytes",
                Some(&metadata),
            )
            .expect("file paste-back must publish a file list");
        let mut paths = Vec::new();
        {
            let _clipboard = Clipboard::new_attempts(OPEN_ATTEMPTS).expect("open the clipboard");
            raw::get_file_list_path(&mut paths).expect("read the file list");
        }
        assert_eq!(paths.len(), 1);
        assert_eq!(
            paths[0].file_name().and_then(|name| name.to_str()),
            Some("fixture.txt")
        );
        assert_eq!(std::fs::read(&paths[0]).unwrap(), b"synthetic bytes");
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

    /// I-18, I-39, T-30, T-33. §6.5 requires the rejection counter to be
    /// asserted and user-visible.
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

    /// A source-less write stays unattributed for explicit exclusion policy.
    ///
    /// A test write owns no clipboard window. It must not inherit the
    /// foreground application, because that application did not write the
    /// clipboard and could otherwise bypass a fail-closed exclusion.
    #[test]
    #[ignore = "drives the real Windows clipboard"]
    fn an_ownerless_capture_has_no_foreground_source_fallback() {
        let _lock = serialised();
        let mut clipboard = WindowsClipboard::new().unwrap();
        write_text("attributed");

        let capture = clipboard.poll().expect("the write must be captured");
        assert!(capture.app_bundle_id.is_none());
        assert!(capture.app_name.is_none());
    }

    /// DMY-158, against the real sequence number: two ownerless writes inside
    /// one poll period are two changes. They must each resolve once rather than
    /// inherit a result from the first change. `two_writers` covers real owners
    /// and their metadata separately.
    #[test]
    #[ignore = "drives the real Windows clipboard"]
    fn each_change_inside_one_poll_period_resolves_its_own_writer() {
        let _lock = serialised();
        let mut clipboard = WindowsClipboard::new().unwrap();
        write_text("the first copy");
        assert!(
            clipboard.poll().is_some(),
            "the first write must be captured"
        );
        write_text("the second copy, a third of a second later");
        assert!(
            clipboard.poll().is_some(),
            "the second write must be captured"
        );

        assert_eq!(
            clipboard.attribution.resolutions(),
            2,
            "the second change reused the first change's identity"
        );
        assert_eq!(clipboard.attribution.unattributed_count(), 2);
    }

    /// DMY-158 before/after comparison on equal workloads.
    ///
    /// The same-process helper writes with no clipboard owner, so this measures
    /// the cache and ownerless-resolution paths only; `two_writers` exercises
    /// process-image lookup for a real owner. The "base" path reuses one
    /// sequence number, so every call after the first is a cache hit. The
    /// "change" path uses a fresh sequence number per call, so every call
    /// resolves. Printed side by side so the cost is visible.
    #[test]
    #[ignore = "drives the real Windows clipboard"]
    fn resolving_an_ownerless_source_costs_a_bounded_number_of_microseconds() {
        const ROUNDS: usize = 200;

        let _lock = serialised();
        write_text("something to attribute");
        let mut clipboard = WindowsClipboard::new().unwrap();
        let _ = clipboard.poll();

        // Base: same sequence number, exercising the cache (old behavior).
        let mut base = Vec::with_capacity(ROUNDS);
        for _ in 0..ROUNDS {
            let started = std::time::Instant::now();
            let _ = clipboard.source_app(0);
            base.push(started.elapsed().as_micros());
        }

        // Change: fresh sequence number each round (the fix).
        let mut change = Vec::with_capacity(ROUNDS);
        for round in 0..ROUNDS {
            let started = std::time::Instant::now();
            let _ = clipboard.source_app(1000 + round as i64);
            change.push(started.elapsed().as_micros());
        }

        base.sort_unstable();
        change.sort_unstable();
        let p = |samples: &[u128], pct: usize| samples[samples.len() * pct / 100];
        println!(
            "base(cached) p50={}us p95={}us; change(per-seq) p50={}us p95={}us",
            p(&base, 50),
            p(&base, 95),
            p(&change, 50),
            p(&change, 95),
        );
        assert!(
            p(&change, 95) < 20_000,
            "one attribution took {}us; a 500 ms poll cannot carry that",
            p(&change, 95)
        );
    }
}
