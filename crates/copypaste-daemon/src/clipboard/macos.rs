//! The real backend: `NSPasteboard`.
//!
//! CopyPaste develops on Linux, so this file neither compiles nor runs on the
//! host it is written on. The logic is manifest 01; the Cocoa spelling is not,
//! and the tests at the bottom are what settles it — they drive the real
//! general pasteboard on macOS CI, which is where `changeCount`, the `+2` of a
//! self-write and the size gate stop being API reading.
//! Anything they do not touch still is.
//!
//! # macOS 16 will make this read a user-visible event
//!
//! macOS 16 alerts the user when an app reads the general pasteboard
//! programmatically, and adds `NSPasteboard.accessBehavior` to request
//! always-allow. Every clipboard manager on the platform is affected; polling
//! `changeCount` without reading is not what trips it, `dataForType` is.
//!
//! Two things this is *not*. It is not the TCC problem in ADR-0001 — that one
//! comes from ad-hoc signing and is why the app needs no Accessibility
//! permission, whereas this arrives however the app is signed and cannot be
//! bought off with a Developer ID. And it is not a reason to poll less often:
//! the alert is about reading at all, so a slower loop would only make capture
//! worse without making the prompt rarer.
//!
//! Left as a note rather than an implementation because the API cannot be
//! exercised here and guessing at its shape is how a binding-level assumption
//! becomes a bug. Whoever first builds on macOS 16 should decide where the
//! always-allow request belongs — most likely the app at first run, not the
//! daemon mid-poll, since a prompt raised by a background process with no
//! window is a prompt with no context.

// objc2 0.2/0.5 marks a shifting set of pasteboard accessors `unsafe`.
// Wrapping every call and allowing the redundant ones keeps this module
// compiling across binding revisions instead of flipping with each bump.
#![allow(unused_unsafe)]

use objc2::rc::{autoreleasepool, Retained};
use std::path::Path;

use objc2_app_kit::NSPasteboard;
use objc2_foundation::{NSArray, NSData, NSString};
use tracing::{debug, info, warn};

mod attribution;
mod file;

use super::change::{clear_sentinel_on_delta_mismatch, Change, ChangeTracker, SELF_WRITE_DELTA};
use super::{Capture, CapturePolicy, ClipboardSource, MAX_CAPTURE_BYTES};
#[cfg(test)]
use crate::macos_workspace::SourceIdentity as FrontmostApp;
use attribution::Attribution;

/// UTIs spelled literally rather than pulled from `NSPasteboardType*`
/// statics: the values are frozen by the OS and by nspasteboard.org, and
/// this way the module does not depend on which binding revision exports
/// which constant.
const UTI_TEXT: &str = "public.utf8-plain-text";
const UTI_RTF: &str = "public.rtf";
const UTI_HTML: &str = "public.html";
const UTI_PNG: &str = "public.png";
const UTI_TIFF: &str = "public.tiff";
const UTI_FILE_URL: &str = "public.file-url";
const UTI_SOURCE: &str = "org.nspasteboard.source";
/// §3.12 (CopyPaste-pbre): the invariant UTI strings are process-lifetime
/// constants, built once and reused. allocating ~12 fresh Cocoa strings on
/// every changed tick.
///
/// A `thread_local` rather than a `static`: these are strong references, so
/// they are deliberately *not* in the per-tick autorelease pool, and this
/// form needs no `Send`/`Sync` claim about `NSString` from whichever
/// binding revision is in the tree.
struct Utis {
    text: Retained<NSString>,
    text_probe: Retained<NSArray<NSString>>,
    rtf: Retained<NSString>,
    rtf_probe: Retained<NSArray<NSString>>,
    html: Retained<NSString>,
    html_probe: Retained<NSArray<NSString>>,
    png: Retained<NSString>,
    png_probe: Retained<NSArray<NSString>>,
    tiff: Retained<NSString>,
    tiff_probe: Retained<NSArray<NSString>>,
    file_url: Retained<NSString>,
    file_url_probe: Retained<NSArray<NSString>>,
    source: Retained<NSString>,
    source_probe: Retained<NSArray<NSString>>,
}

impl Utis {
    fn new() -> Self {
        let text = NSString::from_str(UTI_TEXT);
        let rtf = NSString::from_str(UTI_RTF);
        let html = NSString::from_str(UTI_HTML);
        let png = NSString::from_str(UTI_PNG);
        let tiff = NSString::from_str(UTI_TIFF);
        let file_url = NSString::from_str(UTI_FILE_URL);
        let source = NSString::from_str(UTI_SOURCE);
        // `from_vec`, not `from_slice`: the latter needs `T: IsRetainable`, and
        // `NSString` is `ImmutableWithMutableSubclass<NSMutableString>`, which
        // is not. Taking owned `Retained`s is the supported path for it.
        Self {
            text_probe: NSArray::from_vec(vec![text.clone()]),
            rtf_probe: NSArray::from_vec(vec![rtf.clone()]),
            html_probe: NSArray::from_vec(vec![html.clone()]),
            png_probe: NSArray::from_vec(vec![png.clone()]),
            tiff_probe: NSArray::from_vec(vec![tiff.clone()]),
            file_url_probe: NSArray::from_vec(vec![file_url.clone()]),
            source_probe: NSArray::from_vec(vec![source.clone()]),
            text,
            rtf,
            html,
            png,
            tiff,
            file_url,
            source,
        }
    }
}

thread_local! {
    static UTIS: Utis = Utis::new();
}

/// `NSPasteboard`-backed clipboard source.
///
/// Holds no Cocoa objects: the general pasteboard is a cheap singleton
/// lookup performed inside each poll's autorelease pool, which keeps this
/// type unconditionally `Send` for the poll task.
pub struct MacOsClipboard {
    tracker: ChangeTracker,
    rejected_too_large: u64,
    last_attribution: Option<Attribution>,
    source_coverage: super::source_coverage::GenerationCoverage,
    staging: super::file_materialize::StagingArea,
}

impl MacOsClipboard {
    pub fn new(data_dir: &Path) -> std::io::Result<Self> {
        Ok(Self {
            tracker: ChangeTracker::new(),
            rejected_too_large: 0,
            last_attribution: None,
            source_coverage: Default::default(),
            staging: super::file_materialize::StagingArea::new(data_dir)?,
        })
    }
}

impl ClipboardSource for MacOsClipboard {
    fn poll(&mut self) -> Option<Capture> {
        let settings = copypaste_ipc::ConfigData::default();
        self.poll_with_policy(CapturePolicy::new(&settings))
    }

    fn poll_with_policy(&mut self, policy: CapturePolicy<'_>) -> Option<Capture> {
        // I-17: one autorelease pool around the entire Cocoa interaction.
        // Without it, autoreleased NSString/NSData accumulate on the async
        // worker thread and reserved memory grows without bound.
        autoreleasepool(|_pool| {
            let pb = unsafe { NSPasteboard::generalPasteboard() };

            // I-1: the change-count comparison is the first thing we do; an
            // unchanged pasteboard performs zero reads and zero allocations.
            // Sample history BEFORE changeCount. A later activation cannot be
            // acknowledged as belonging to this already-observed generation.
            let observation = crate::macos_workspace::source_observation();
            let count = unsafe { pb.changeCount() } as i64;
            match self.tracker.observe(count) {
                Change::Unchanged => {
                    self.source_coverage.unchanged(count, observation);
                    return None;
                }
                Change::SelfWrite => {
                    self.source_coverage.consume(count, observation, None, true);
                    debug!(change_count = count, "suppressed our own pasteboard write");
                    return None;
                }
                Change::Fresh { lost_intermediates } => {
                    if lost_intermediates > 0 {
                        // §3.1/§3.2: telemetry only. NSPasteboard keeps no
                        // history, so the intermediates are gone — but the
                        // surviving value is captured below, never replaced
                        // by a "burst happened" result.
                        warn!(
                            lost = lost_intermediates,
                            change_count = count,
                            "pasteboard burst: intermediate values are irrecoverable"
                        );
                    }
                }
            }

            // Private mode must be a capture gate, not merely an ingest choice:
            // acknowledge the change without reading either attribution or data.
            if policy.settings.private_mode {
                self.source_coverage.consume(count, observation, None, true);
                return None;
            }

            let decision = observation
                .and_then(|sample| {
                    crate::macos_workspace::source_decision(
                        self.source_coverage.boundary(),
                        count,
                        sample,
                    )
                })
                .unwrap_or_else(|| {
                    super::source_coverage::Decision::unavailable(count, observation)
                });
            let current_count = unsafe { pb.changeCount() } as i64;
            self.source_coverage.consume(
                count,
                observation,
                Some(&decision),
                current_count == count,
            );
            let read_valid = || {
                decision.fence(crate::macos_workspace::source_observation(), unsafe {
                    pb.changeCount()
                }
                    as i64)
            };
            decision.materialize(
                &policy.settings.excluded_app_bundle_ids,
                || {
                    let observation = crate::macos_workspace::source_observation();
                    (unsafe { pb.changeCount() } as i64, observation)
                },
                || {
                    let (uti, content_type) = UTIS.with(|utis| unsafe {
                        for (probe, uti, content_type) in [
                            (
                                &utis.file_url_probe,
                                &utis.file_url,
                                copypaste_ipc::content_type::FILE,
                            ),
                            (
                                &utis.text_probe,
                                &utis.text,
                                copypaste_ipc::content_type::TEXT,
                            ),
                            (
                                &utis.rtf_probe,
                                &utis.rtf,
                                copypaste_ipc::content_type::RICH_TEXT,
                            ),
                            (
                                &utis.html_probe,
                                &utis.html,
                                copypaste_ipc::content_type::HTML,
                            ),
                            (
                                &utis.png_probe,
                                &utis.png,
                                copypaste_ipc::content_type::IMAGE_PNG,
                            ),
                            (
                                &utis.tiff_probe,
                                &utis.tiff,
                                copypaste_ipc::content_type::IMAGE_TIFF,
                            ),
                        ] {
                            if !read_valid() {
                                return None;
                            }
                            if pb.availableTypeFromArray(probe).is_some() {
                                if !read_valid() {
                                    return None;
                                }
                                // A selected representation's read failure is
                                // terminal; never fall through to another type.
                                return Some((uti.clone(), content_type));
                            }
                        }
                        None
                    })?;
                    if !read_valid() {
                        return None;
                    }
                    let app = UTIS.with(|utis| {
                        attribution::read(
                            &pb,
                            &utis.source,
                            &utis.source_probe,
                            decision.identity.as_ref(),
                        )
                    });
                    if !read_valid() {
                        return None;
                    }
                    self.note_attribution(&decision, app.as_ref());
                    let app_bundle_id = app.as_ref().and_then(|app| app.bundle_id.clone());
                    let app_name = app.and_then(|app| app.name);
                    // Declared sources may add a denial, never remove the
                    // pre-read foreground coverage gate.
                    if app_bundle_id
                        .as_ref()
                        .is_some_and(|id| policy.settings.excluded_app_bundle_ids.contains(id))
                    {
                        return None;
                    }
                    if content_type == copypaste_ipc::content_type::FILE {
                        let (bytes, metadata) = match file::read(
                            &pb,
                            &uti,
                            policy.limit_bytes(content_type),
                            read_valid,
                        ) {
                            Ok(result) => result,
                            Err(reason) => {
                                if matches!(
                                    reason,
                                    file::FileInputError::Read(
                                        super::file_capture::FileReadError::TooLarge
                                    )
                                ) {
                                    self.rejected_too_large += 1;
                                }
                                file::reject(reason);
                                return None;
                            }
                        };
                        info!("file capture materialized");
                        let capture = Capture {
                            content: String::new(),
                            binary_content: Some(bytes),
                            file_path: None,
                            file_metadata: Some(metadata),
                            content_type: content_type.to_string(),
                            app_bundle_id,
                            app_name,
                            source_policy: super::SourcePolicyEvidence::MacOs(
                                decision.coverage.clone(),
                            ),
                        };
                        return read_valid().then_some(capture);
                    }
                    let data = unsafe { pb.dataForType(&uti) }?;
                    if !read_valid() {
                        return None;
                    }

                    // I-18 (CopyPaste-1f5c): `length` is a field read, `to_vec` on a
                    // multi-GiB item is a multi-GiB allocation. Check first.
                    let len = unsafe { data.length() };
                    let cap = usize::try_from(policy.limit_bytes(content_type))
                        .unwrap_or(MAX_CAPTURE_BYTES);
                    if len > cap {
                        self.rejected_too_large += 1;
                        // I-39 / §6.5: counted, not silently dropped.
                        warn!(
                            bytes = len,
                            cap, "pasteboard representation exceeds the size cap; dropped"
                        );
                        return None;
                    }

                    if !read_valid() {
                        return None;
                    }
                    let bytes = unsafe { data.bytes() }.to_vec();
                    if !read_valid() {
                        return None;
                    }
                    if copypaste_ipc::content_type::is_text(content_type) {
                        // Clipboard text representations can contain malformed UTF-8.
                        // §3.6's precedent is lossy conversion rather than dropping
                        // the user's copy, and I-37 forbids panicking on a malformed
                        // payload.
                        let content = String::from_utf8_lossy(&bytes).into_owned();
                        if content.is_empty() {
                            return None;
                        }
                        let capture = Capture {
                            content,
                            binary_content: None,
                            file_path: None,
                            file_metadata: None,
                            content_type: content_type.to_string(),
                            app_bundle_id,
                            app_name,
                            source_policy: super::SourcePolicyEvidence::MacOs(
                                decision.coverage.clone(),
                            ),
                        };
                        return read_valid().then_some(capture);
                    }
                    let capture = Capture {
                        content: String::new(),
                        binary_content: Some(bytes),
                        file_path: None,
                        file_metadata: None,
                        content_type: content_type.to_string(),
                        app_bundle_id,
                        app_name,
                        source_policy: super::SourcePolicyEvidence::MacOs(
                            decision.coverage.clone(),
                        ),
                    };
                    read_valid().then_some(capture)
                },
            )
        })
    }

    fn changed(&mut self) -> bool {
        autoreleasepool(|_pool| {
            let pb = unsafe { NSPasteboard::generalPasteboard() };
            let observation = crate::macos_workspace::source_observation();
            let count = unsafe { pb.changeCount() } as i64;
            let changed = !self.tracker.is_current(count);
            if !changed {
                self.source_coverage.unchanged(count, observation);
            }
            changed
        })
    }

    fn set_contents(&mut self, text: &str) -> anyhow::Result<()> {
        // I-17 again: the paste-back path needs the pool as much as the poll.
        autoreleasepool(|_pool| {
            let pb = unsafe { NSPasteboard::generalPasteboard() };
            let pre = unsafe { pb.changeCount() } as i64;

            let value = NSString::from_str(text);
            let ok = UTIS.with(|utis| unsafe {
                // §3.3 step 2, armed from the count `clearContents` returns
                // rather than a predicted one, and before `setString` makes the
                // content visible. A poll landing between the two sees an empty
                // pasteboard, which yields nothing and keeps polling.
                self.tracker.sentinel.arm(pb.clearContents() as i64);
                pb.setString_forType(&value, &utis.text)
            });
            if !ok {
                // §3.3 step 5.
                self.tracker.sentinel.clear();
                // I-9 / AGENTS.md rule 4: no paths, no content.
                return Err(anyhow::anyhow!("the pasteboard rejected the write"));
            }

            // CopyPaste-8yzf: a third-party write landing in the middle leaves
            // the armed count below the current one, where it can never match.
            // There is nothing to repair — reported, not corrected, because
            // correcting it is what would suppress their content as ours.
            let actual = unsafe { pb.changeCount() } as i64;
            if actual != pre + SELF_WRITE_DELTA {
                if clear_sentinel_on_delta_mismatch() {
                    self.tracker.sentinel.clear();
                }
                debug!(
                    expected = pre + SELF_WRITE_DELTA,
                    actual, "another app wrote to the pasteboard during our write"
                );
            }
            Ok(())
        })
    }

    fn set_binary_contents(
        &mut self,
        _item_id: &str,
        content_type: &str,
        bytes: &[u8],
        metadata: Option<&copypaste_core::FileMetadata>,
    ) -> Result<(), copypaste_core::ClipboardWriteError> {
        use copypaste_core::ClipboardWriteError;

        let uti = match content_type {
            copypaste_ipc::content_type::HTML => UTI_HTML,
            copypaste_ipc::content_type::RICH_TEXT => UTI_RTF,
            copypaste_ipc::content_type::IMAGE_PNG => UTI_PNG,
            copypaste_ipc::content_type::IMAGE_TIFF => UTI_TIFF,
            "image/jpeg" => "public.jpeg",
            "image/gif" => "com.compuserve.gif",
            "image/bmp" => "com.microsoft.bmp",
            "image/webp" => "org.webmproject.webp",
            copypaste_ipc::content_type::FILE => UTI_FILE_URL,
            _ => {
                return Err(ClipboardWriteError::UnsupportedContent);
            }
        };
        let file_url = if content_type == copypaste_ipc::content_type::FILE {
            let metadata = metadata.ok_or(ClipboardWriteError::Failed)?;
            let path = self
                .staging
                .materialize(bytes, metadata)
                .map_err(|_| ClipboardWriteError::Failed)?;
            Some(url::Url::from_file_path(path).map_err(|_| ClipboardWriteError::Failed)?)
        } else {
            None
        };
        autoreleasepool(|_pool| {
            let pb = unsafe { NSPasteboard::generalPasteboard() };
            let pre = unsafe { pb.changeCount() } as i64;
            let bytes = file_url
                .as_ref()
                .map_or(bytes, |url| url.as_str().as_bytes());
            let data = unsafe {
                NSData::dataWithBytes_length(bytes.as_ptr().cast_mut().cast(), bytes.len())
            };
            let uti = NSString::from_str(uti);
            self.tracker
                .sentinel
                .arm(unsafe { pb.clearContents() } as i64);
            if !unsafe { pb.setData_forType(Some(&data), &uti) } {
                self.tracker.sentinel.clear();
                return Err(ClipboardWriteError::Failed);
            }
            let actual = unsafe { pb.changeCount() } as i64;
            if actual != pre + SELF_WRITE_DELTA {
                if clear_sentinel_on_delta_mismatch() {
                    self.tracker.sentinel.clear();
                }
                debug!(
                    expected = pre + SELF_WRITE_DELTA,
                    actual, "another app wrote to the pasteboard during our write"
                );
            }
            Ok(())
        })
    }

    fn backend_name(&self) -> &'static str {
        "nspasteboard"
    }

    fn rejected_too_large_count(&self) -> u64 {
        self.rejected_too_large
    }

    fn lost_intermediates_count(&self) -> u64 {
        self.tracker.lost_intermediates
    }
}

// Tests — the half of manifest 01 that only a Mac can answer
//
// `super::change` made the state machine testable anywhere. What stayed
// untestable is this file: whether `changeCount` moves at all without a window
// server, whether `clearContents` + `setString:forType:` is really the +2 of
// §4. Each is an assumption about the bindings, and each one wrong breaks a
// rule the manifest records a production bug for.
//
// `#[ignore]`: the general pasteboard is the machine's own clipboard, so a run
// replaces whatever was copied. Serialised for the reason §5 gives — it is a
// process-global singleton — by a mutex here as well as `--test-threads=1` in
// CI, so a plain `cargo test -- --ignored` is not silently racy.

#[cfg(test)]
mod tests {
    use std::sync::{Mutex, MutexGuard};

    use objc2::{rc::Retained, runtime::ProtocolObject};
    use objc2_app_kit::NSPasteboardWriting;
    use objc2_foundation::{NSData, NSURL};

    use super::*;

    static PASTEBOARD: Mutex<()> = Mutex::new(());

    fn serialised() -> MutexGuard<'static, ()> {
        PASTEBOARD.lock().unwrap_or_else(|held| held.into_inner())
    }

    fn test_clipboard() -> (tempfile::TempDir, MacOsClipboard) {
        let data_dir = tempfile::tempdir().unwrap();
        let clipboard = MacOsClipboard::new(data_dir.path()).unwrap();
        (data_dir, clipboard)
    }

    fn change_count() -> i64 {
        autoreleasepool(|_| unsafe { NSPasteboard::generalPasteboard().changeCount() as i64 })
    }

    /// What another application does: `clearContents` and then one `setData`
    /// per representation it offers.
    fn write_types(reps: &[(&str, &[u8])]) {
        autoreleasepool(|_| unsafe {
            let pb = NSPasteboard::generalPasteboard();
            let _ = pb.clearContents();
            for &(uti, bytes) in reps {
                let data = NSData::with_bytes(bytes);
                let _ = pb.setData_forType(Some(&data), &NSString::from_str(uti));
            }
        });
    }

    /// Whether the pasteboard offers a UTI — the same question the poll asks,
    /// so a test can assert its own precondition instead of trusting the
    /// return value of `setData:forType:`.
    fn offers(uti: &str) -> bool {
        autoreleasepool(|_| unsafe {
            let types = NSArray::from_vec(vec![NSString::from_str(uti)]);
            NSPasteboard::generalPasteboard()
                .availableTypeFromArray(&types)
                .is_some()
        })
    }

    fn write_text(text: &str) {
        write_types(&[(UTI_TEXT, text.as_bytes())]);
        assert!(offers(UTI_TEXT), "the pasteboard refused a test write");
    }

    fn write_file_urls(paths: &[&std::path::Path]) {
        autoreleasepool(|_| unsafe {
            let objects: Vec<Retained<ProtocolObject<dyn NSPasteboardWriting>>> = paths
                .iter()
                .map(|path| {
                    let path = NSString::from_str(path.to_str().unwrap());
                    ProtocolObject::from_retained(NSURL::fileURLWithPath(&path))
                })
                .collect();
            let objects = NSArray::from_vec(objects);
            let pb = NSPasteboard::generalPasteboard();
            let _ = pb.clearContents();
            assert!(
                pb.writeObjects(&objects),
                "the file URLs were not put on the pasteboard"
            );
        });
    }

    fn clear() {
        autoreleasepool(|_| unsafe {
            let _ = NSPasteboard::generalPasteboard().clearContents();
        });
    }

    #[test]
    fn source_attribution_preserves_a_name_when_a_helper_has_no_bundle_id() {
        assert_eq!(
            Attribution::from_app(Some(&FrontmostApp {
                bundle_id: None,
                name: Some("Helper".to_owned()),
            })),
            Attribution::NameOnly
        );
    }

    #[test]
    #[ignore = "drives the real NSPasteboard"]
    fn file_write_uses_the_active_data_directory_and_preserves_the_filename() {
        let _lock = serialised();
        let data_dir = tempfile::tempdir().unwrap();
        let mut clipboard = MacOsClipboard::new(data_dir.path()).unwrap();
        let metadata =
            copypaste_core::FileMetadata::new("Résumé 2026.pdf", "application/pdf").unwrap();

        clipboard
            .set_binary_contents(
                "untrusted-remote-id",
                copypaste_ipc::content_type::FILE,
                b"decrypted file bytes",
                Some(&metadata),
            )
            .unwrap();

        let path = autoreleasepool(|_| unsafe {
            let file_url = NSString::from_str(UTI_FILE_URL);
            let bytes = NSPasteboard::generalPasteboard()
                .dataForType(&file_url)
                .expect("the file URL was not put on the pasteboard")
                .bytes()
                .to_vec();
            let url = url::Url::parse(&String::from_utf8(bytes).unwrap()).unwrap();
            url.to_file_path().unwrap()
        });
        assert!(path.starts_with(data_dir.path().join("paste-files")));
        assert_eq!(path.file_name().unwrap(), "Résumé 2026.pdf");
        assert_eq!(std::fs::read(path).unwrap(), b"decrypted file bytes");
    }

    /// T-21, T-4, I-1, I-2. Also the first question of all: does `changeCount`
    /// move on a headless runner?
    #[test]
    #[ignore = "drives the real NSPasteboard"]
    fn a_change_by_another_app_is_captured_exactly_once() {
        let _lock = serialised();
        let (_data_dir, mut clipboard) = test_clipboard();

        let before = change_count();
        write_text("copypaste — a check ✓");
        let after = change_count();
        assert!(
            after > before,
            "changeCount did not move for a write ({before} -> {after}); \
             nothing in this backend can work if it does not"
        );

        let capture = clipboard.poll().expect("the write must be captured");
        assert_eq!(capture.content, "copypaste — a check ✓");
        assert_eq!(capture.content_type, copypaste_ipc::content_type::TEXT);
        assert_eq!(
            clipboard.lost_intermediates_count(),
            0,
            "I-2: a first poll at an arbitrary change count is not a burst"
        );

        assert!(
            clipboard.poll().is_none(),
            "T-4: an unchanged pasteboard must yield nothing"
        );
    }

    /// §4's measured self-write delta. Split from the suppression test so the
    /// OS observation and the user-visible protocol carry separate verdicts.
    #[test]
    #[ignore = "drives the real NSPasteboard"]
    fn a_self_write_moves_the_count_by_the_measured_delta() {
        let _lock = serialised();
        let (_data_dir, mut clipboard) = test_clipboard();
        write_text("something copied earlier");
        let _ = clipboard.poll();

        let pre = change_count();
        clipboard
            .set_contents("pasted by us")
            .expect("the write failed");
        let actual = change_count();
        assert_eq!(
            actual - pre,
            SELF_WRITE_DELTA,
            "§4 measured clearContents + setString:forType: as one generation. \
             Observed {pre} -> {actual}; the sentinel must name {}",
            pre + SELF_WRITE_DELTA
        );
    }

    #[test]
    #[ignore = "drives the real NSPasteboard"]
    fn text_wins_when_rich_text_and_an_image_are_also_offered() {
        let _lock = serialised();
        let (_data_dir, mut clipboard) = test_clipboard();

        write_types(&[
            (UTI_TEXT, b"plain fallback"),
            (UTI_RTF, b"{\\rtf1 ignored}"),
            (UTI_HTML, b"<p>ignored</p>"),
            (UTI_PNG, b"ignored"),
        ]);
        assert!(offers(UTI_TEXT) && offers(UTI_RTF) && offers(UTI_HTML) && offers(UTI_PNG));
        let capture = clipboard.poll().expect("plain text must win when offered");
        assert_eq!(capture.content, "plain fallback");
        assert_eq!(capture.content_type, copypaste_ipc::content_type::TEXT);
    }

    #[test]
    #[ignore = "drives the real NSPasteboard"]
    fn a_file_url_wins_over_its_textual_filename_fallback() {
        let _lock = serialised();
        let (_data_dir, mut clipboard) = test_clipboard();
        let fixture_dir = tempfile::tempdir().unwrap();
        let path = fixture_dir.path().join("fixture.bin");
        std::fs::write(&path, b"file bytes").unwrap();
        let url = url::Url::from_file_path(&path).unwrap();

        write_types(&[
            (UTI_TEXT, b"fixture.bin"),
            (UTI_FILE_URL, url.as_str().as_bytes()),
        ]);
        let capture = clipboard.poll().expect("the file URL must win");
        assert_eq!(capture.content_type, copypaste_ipc::content_type::FILE);
        assert!(capture.file_path.is_none());
        assert_eq!(
            capture.binary_content.as_deref(),
            Some(b"file bytes".as_slice())
        );
    }

    #[test]
    #[ignore = "drives the real NSPasteboard"]
    fn rich_text_and_html_prefer_a_plain_text_representation_when_cocoa_offers_one() {
        let _lock = serialised();
        let (_data_dir, mut clipboard) = test_clipboard();

        for (uti, content, content_type) in [
            (
                UTI_RTF,
                b"{\\rtf1 synthetic rich text}".as_slice(),
                copypaste_ipc::content_type::RICH_TEXT,
            ),
            (
                UTI_HTML,
                b"<p>synthetic html</p>".as_slice(),
                copypaste_ipc::content_type::HTML,
            ),
        ] {
            write_types(&[(uti, content)]);
            assert!(offers(uti), "{uti} was not put on the pasteboard");
            let expected_type = if offers(UTI_TEXT) {
                copypaste_ipc::content_type::TEXT
            } else {
                content_type
            };
            let capture = clipboard.poll().expect("rich fallback must be captured");
            assert!(!capture.content.is_empty(), "{uti} produced empty content");
            assert_eq!(capture.content_type, expected_type);
            assert!(capture.binary_content.is_none());
        }
    }

    #[test]
    #[ignore = "drives the real NSPasteboard"]
    fn one_local_file_url_freezes_its_bytes_and_metadata() {
        let _lock = serialised();
        let (_data_dir, mut clipboard) = test_clipboard();
        let fixture_dir = tempfile::tempdir().unwrap();
        let path = fixture_dir.path().join("fixture.bin");
        std::fs::write(&path, b"synthetic file fixture").unwrap();
        let url = url::Url::from_file_path(&path).unwrap();

        write_types(&[(UTI_FILE_URL, url.as_str().as_bytes())]);
        assert!(
            offers(UTI_FILE_URL),
            "the file URL was not put on the pasteboard"
        );
        let capture = clipboard.poll().expect("a local file URL must be captured");
        assert_eq!(capture.content_type, copypaste_ipc::content_type::FILE);
        assert!(capture.file_path.is_none());
        assert_eq!(
            capture.file_metadata,
            copypaste_core::FileMetadata::with_source_reference(
                "fixture.bin",
                "application/octet-stream",
                path.to_string_lossy(),
            )
        );
        assert_eq!(
            capture.binary_content.as_deref(),
            Some(b"synthetic file fixture".as_slice())
        );

        write_types(&[(UTI_FILE_URL, b"https://example.invalid/fixture.bin")]);
        assert!(
            clipboard.poll().is_none(),
            "a non-local URL must not produce a file capture"
        );

        let second = fixture_dir.path().join("second.bin");
        std::fs::write(&second, b"second fixture").unwrap();
        write_file_urls(&[path.as_path(), second.as_path()]);
        assert!(
            clipboard.poll().is_none(),
            "a multi-file change must not silently capture its first path"
        );
    }

    #[test]
    #[ignore = "drives the real NSPasteboard and native NSURL resolution"]
    fn native_url_path_and_asserted_file_reference_preserve_owned_fixture_bytes() {
        use std::ffi::{CStr, OsString};
        use std::os::unix::{ffi::OsStringExt, fs::MetadataExt};

        let _lock = serialised();
        let fixture_dir = tempfile::tempdir().unwrap();
        for (filename, len, reference) in [
            ("noextension", 32, false),
            ("Résumé space λ %#", 87, false),
            ("reference-file", 32, true),
        ] {
            let path = fixture_dir.path().join(filename);
            let bytes = vec![if reference { 9 } else { 7 }; len];
            std::fs::write(&path, &bytes).unwrap();
            autoreleasepool(|_| unsafe {
                let mut url = NSURL::fileURLWithPath(&NSString::from_str(path.to_str().unwrap()));
                if reference {
                    url = url
                        .fileReferenceURL()
                        .expect("native reference URL must exist");
                    assert!(
                        url.isFileReferenceURL(),
                        "path URL cannot qualify reference resolution"
                    );
                } else {
                    assert!(!url.isFileReferenceURL());
                }
                let expected_url = url
                    .filePathURL()
                    .expect("native URL must resolve to a path URL");
                assert!(!expected_url.isFileReferenceURL());
                let expected_path = std::path::PathBuf::from(OsString::from_vec(
                    CStr::from_ptr(expected_url.fileSystemRepresentation().as_ptr())
                        .to_bytes()
                        .to_vec(),
                ));
                let objects: Retained<NSArray<ProtocolObject<dyn NSPasteboardWriting>>> =
                    NSArray::from_vec(vec![ProtocolObject::from_retained(url)]);
                let pb = NSPasteboard::generalPasteboard();
                let _ = pb.clearContents();
                assert!(pb.writeObjects(&objects));
                let (captured, metadata) =
                    file::read(&pb, &NSString::from_str(UTI_FILE_URL), 1024, || true).unwrap();
                assert_eq!(captured, bytes);
                assert_eq!(
                    metadata.filename,
                    expected_path.file_name().unwrap().to_str().unwrap()
                );
                assert_eq!(metadata.source_reference.as_deref(), expected_path.to_str());
                let locator = std::path::Path::new(metadata.source_reference.as_deref().unwrap());
                assert!(locator.is_absolute());
                assert_eq!(std::fs::read(locator).unwrap(), bytes);
                let locator_metadata = std::fs::metadata(locator).unwrap();
                let fixture_metadata = std::fs::metadata(&path).unwrap();
                assert_eq!(
                    (locator_metadata.dev(), locator_metadata.ino()),
                    (fixture_metadata.dev(), fixture_metadata.ino())
                );
            });
        }
    }
    #[test]
    #[ignore = "drives the real NSPasteboard"]
    fn native_file_plus_another_item_is_explicitly_unsupported() {
        let _lock = serialised();
        let fixture_dir = tempfile::tempdir().unwrap();
        let path = fixture_dir.path().join("single-file");
        std::fs::write(&path, b"bytes").unwrap();
        autoreleasepool(|_| unsafe {
            let url = NSURL::fileURLWithPath(&NSString::from_str(path.to_str().unwrap()));
            let objects: Retained<NSArray<ProtocolObject<dyn NSPasteboardWriting>>> =
                NSArray::from_vec(vec![
                    ProtocolObject::from_retained(url),
                    ProtocolObject::from_retained(NSString::from_str("second item")),
                ]);
            let pb = NSPasteboard::generalPasteboard();
            let _ = pb.clearContents();
            assert!(pb.writeObjects(&objects));
            assert_eq!(pb.pasteboardItems().unwrap().len(), 2);
            assert_eq!(
                file::read(&pb, &NSString::from_str(UTI_FILE_URL), 1024, || true),
                Err(file::FileInputError::UnsupportedItemCount)
            );
        });
    }

    /// T-8, T-9 and the Fix-4 / "DUP-ON-COPY" pair, asserted as behaviour
    /// rather than as arithmetic: whatever the delta turns out to be, our own
    /// write must not come back, and the next genuine copy must.
    #[test]
    #[ignore = "drives the real NSPasteboard"]
    fn our_own_write_is_suppressed_and_the_next_genuine_copy_is_not() {
        let _lock = serialised();
        let (_data_dir, mut clipboard) = test_clipboard();
        write_text("something copied earlier");
        let _ = clipboard.poll();

        clipboard
            .set_contents("pasted by us")
            .expect("the write failed");
        if let Some(capture) = clipboard.poll() {
            panic!(
                "T-8: our own paste-back came back as a capture ({:?}) — this is \
                 the duplicate-on-copy bug, and the sentinel is now armed at a \
                 count no write of ours will reach",
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
    #[ignore = "drives the real NSPasteboard"]
    fn a_burst_reports_its_losses_and_still_returns_the_survivor() {
        let _lock = serialised();
        let (_data_dir, mut clipboard) = test_clipboard();
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
        assert_eq!(clipboard.lost_intermediates_count(), lost);
    }

    /// I-18, I-39, T-30, T-33. §6.5 requires both rejection and a visible
    /// counter; an oversized item may not vanish silently.
    #[test]
    #[ignore = "drives the real NSPasteboard"]
    fn text_over_the_cap_is_rejected_and_counted_and_the_boundary_is_kept() {
        let _lock = serialised();
        let (_data_dir, mut clipboard) = test_clipboard();

        let oversized = vec![b'a'; MAX_CAPTURE_BYTES + 1];
        write_types(&[(UTI_TEXT, &oversized)]);
        assert!(offers(UTI_TEXT), "the oversized write never landed");
        assert!(clipboard.poll().is_none(), "T-30: over the cap by one byte");
        assert_eq!(
            clipboard.rejected_too_large_count(),
            1,
            "I-39: a rejection must be counted, not only logged"
        );

        let at_the_cap = vec![b'a'; MAX_CAPTURE_BYTES];
        write_types(&[(UTI_TEXT, &at_the_cap)]);
        assert!(offers(UTI_TEXT), "the boundary write never landed");
        let capture = clipboard.poll().expect("T-33: len == cap is accepted");
        assert_eq!(capture.content.len(), MAX_CAPTURE_BYTES);
        assert_eq!(clipboard.rejected_too_large_count(), 1);
    }

    #[test]
    #[ignore = "drives the real NSPasteboard"]
    fn image_only_changes_are_captured_and_use_the_image_limit() {
        let _lock = serialised();
        let (_data_dir, mut clipboard) = test_clipboard();
        let settings = copypaste_ipc::ConfigData {
            max_image_size_bytes: copypaste_ipc::MIN_IMAGE_SIZE_BYTES,
            ..Default::default()
        };

        use base64::{engine::general_purpose::STANDARD, Engine as _};
        let image = STANDARD
            .decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNgYAAAAAMAASsJTYQAAAAASUVORK5CYII=")
            .unwrap();
        write_types(&[(UTI_PNG, &image)]);
        assert!(offers(UTI_PNG), "the image-only write never landed");
        let capture = clipboard
            .poll_with_policy(CapturePolicy::new(&settings))
            .expect("a native PNG must be captured");
        assert_eq!(capture.content_type, copypaste_ipc::content_type::IMAGE_PNG);
        assert_eq!(capture.content, "");
        assert_eq!(capture.binary_content.as_deref(), Some(image.as_slice()));

        let oversized = vec![0; copypaste_ipc::MIN_IMAGE_SIZE_BYTES as usize + 1];
        write_types(&[(UTI_PNG, &oversized)]);
        assert!(offers(UTI_PNG), "the image-only write never landed");
        assert!(clipboard
            .poll_with_policy(CapturePolicy::new(&settings))
            .is_none());
        assert_eq!(clipboard.rejected_too_large_count(), 1);
        assert!(
            clipboard
                .poll_with_policy(CapturePolicy::new(&settings))
                .is_none(),
            "the cursor must advance for a rejected image"
        );
    }

    /// An empty pasteboard is a change like any other: nothing to capture, and
    /// the cursor still moves (I-3).
    #[test]
    #[ignore = "drives the real NSPasteboard"]
    fn a_cleared_pasteboard_yields_nothing_and_keeps_polling() {
        let _lock = serialised();
        let (_data_dir, mut clipboard) = test_clipboard();
        write_text("before the clear");
        let _ = clipboard.poll();

        clear();
        assert!(clipboard.poll().is_none());
        assert!(clipboard.poll().is_none());

        write_text("after the clear");
        assert_eq!(
            clipboard.poll().expect("capture must resume").content,
            "after the clear"
        );
    }
}
