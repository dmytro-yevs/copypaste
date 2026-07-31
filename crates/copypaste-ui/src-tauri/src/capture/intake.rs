//! Getting a captured clip from the platform into the database.
//!
//! Every rung 0 surface — the share sheet, the text-selection action, the
//! Quick Settings tile — and rung 2's background listener end here, and all of
//! them end at [`crate::backend::Backend::add`]. There is no second ingest
//! path: `capture.rs` in the daemon records what happened when v1 had two
//! ("the IPC one forgot the dedup probe"), and dedup, secret detection,
//! eviction and the size cap are all decisions this file must not re-make
//! (CLAUDE.md rule 1).
//!
//! # Today `add` refuses, and that is the whole dependency
//!
//! `Backend::add` on Android returns `Unsupported` until `capture::ingest`
//! moves into `copypaste-core` (ADR-0003). So this module is complete and its
//! sink is not: a captured clip is buffered, retried, and — if the refusal is
//! structural — reported. What it must never do is drop it quietly, because a
//! copy the user believes was saved and was not is the exact failure the
//! android doc calls the worst outcome.
//!
//! # Why nothing here filters what it stores
//!
//! Copying an item out of history puts it back on the system clipboard, which
//! the rung 2 listener then reports as a new copy. That is not a bug to fix
//! here: the backend's dedup probe collapses it onto the existing row, which is
//! also what happens on macOS when the daemon polls its own write. A
//! "did we just write this?" guard in this file would be a second, weaker
//! definition of the same thing.

use std::collections::VecDeque;

use copypaste_ipc::EventKind;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime};

use crate::backend::{Backend, BackendError, Result, SelectedBackend};
use crate::model::UiItem;
use crate::service::push::{ChangePayload, EVENT_CHANGED};

use super::model::{CaptureSource, Clip};
use super::{CaptureControl, SelectedCapture};

/// One clip reached the database. Carries no content — the frontend re-reads
/// through `list`, so there is one set of rules about what the WebView may see.
pub const EVENT_CAPTURED: &str = "copypaste://captured";

/// Capture state changed: armed, lost, proven, or the user changed it.
pub const EVENT_CAPTURE_STATE: &str = "copypaste://capture-state";

/// How often the drain task asks the platform for what it has captured.
///
/// This is **not** clipboard polling. The clipboard signal is a push — the
/// listener registered as the shell uid — and this only moves already-captured
/// text from Kotlin's queue into Rust's database, inside one process. A second
/// of latency to storage is invisible; what it buys is not needing a JNI
/// callback into a crate that forbids unsafe code.
const DRAIN_INTERVAL: std::time::Duration = std::time::Duration::from_secs(1);

/// How many clips may wait for a sink that is refusing before the oldest are
/// counted as lost.
///
/// Bounded because an `Unsupported` sink never recovers within a run, and an
/// unbounded queue of clipboard plaintext in memory is its own problem. The
/// count is surfaced, so the loss is visible.
const MAX_BUFFERED: usize = 128;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapturedPayload {
    pub id: String,
    pub source: CaptureSource,
    /// So the list can render the placeholder row without a second call.
    pub is_sensitive: bool,
}

/// Clips taken from the platform and not yet stored.
///
/// Separate from the loop so the retry rule is testable without a Tauri app:
/// a clip whose store failed goes back to the front of the queue, in order, and
/// is tried again before anything newer.
#[derive(Debug, Default)]
pub struct Buffer {
    queue: VecDeque<Clip>,
    dropped: u64,
}

impl Buffer {
    pub fn push_all(&mut self, clips: impl IntoIterator<Item = Clip>) {
        self.queue.extend(clips);
        while self.queue.len() > MAX_BUFFERED {
            self.queue.pop_front();
            self.dropped = self.dropped.saturating_add(1);
        }
    }

    pub fn pop(&mut self) -> Option<Clip> {
        self.queue.pop_front()
    }

    /// A clip whose store failed. Goes back where it came from, not to the end:
    /// history order follows capture order, and a retry must not reshuffle it.
    pub fn requeue(&mut self, clip: Clip) {
        self.queue.push_front(clip);
    }

    /// Clips that were taken from the platform and never stored. Reported, not
    /// swallowed.
    pub fn dropped(&self) -> u64 {
        self.dropped
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}

/// Store one clip.
///
/// `Ok(None)` means there was nothing worth storing. Blank content is dropped
/// here rather than at the backend so a share of pure whitespace does not spend
/// a round trip — the same check `commands::history::add_item` makes.
pub async fn store<B: Backend>(backend: &B, clip: &Clip) -> Result<Option<UiItem>> {
    if clip.text.trim().is_empty() {
        return Ok(None);
    }
    let item = backend.add(&clip.text).await?;
    Ok(Some(item.into()))
}

/// Capture the clipboard right now and store it — the Quick Settings tile and
/// the in-app capture button.
///
/// Synchronous with the user's gesture on purpose: they tapped, so a failure is
/// something to show them immediately rather than to buffer and retry. The read
/// itself is legal because the tap gave our activity focus.
pub async fn capture_once<R: Runtime>(
    app: &AppHandle<R>,
    source: CaptureSource,
) -> Result<Option<UiItem>> {
    let clip = {
        let capture = app.state::<SelectedCapture>();
        capture.read_now(source)?
    };
    let Some(clip) = clip else {
        return Ok(None);
    };

    let backend = app.state::<SelectedBackend>();
    let stored = store(backend.inner(), &clip).await?;
    if let Some(item) = &stored {
        announce(app, item, source, clip.at_ms).await;
    }
    Ok(stored)
}

/// Move what the platform has captured into the database, for the life of the
/// app.
///
/// Spawned once from `crate::run`, like `service::push::spawn`. On macOS the
/// drain is empty and this costs one wakeup a second doing nothing; the loop
/// still runs there rather than being `cfg`'d out, because a `cfg` here would
/// be the second one in a module that exists to have none.
pub fn spawn<R: Runtime>(app: AppHandle<R>) {
    tauri::async_runtime::spawn(async move {
        let mut buffer = Buffer::default();
        loop {
            tokio::time::sleep(DRAIN_INTERVAL).await;
            tick(&app, &mut buffer).await;
        }
    });
}

async fn tick<R: Runtime>(app: &AppHandle<R>, buffer: &mut Buffer) {
    let taken = {
        let capture = app.state::<SelectedCapture>();
        match capture.drain() {
            Ok(clips) => clips,
            Err(error) => {
                tracing::debug!(%error, "the platform had nothing to hand over");
                Vec::new()
            }
        }
    };
    let had = buffer.dropped();
    buffer.push_all(taken);
    let lost = buffer.dropped() - had;
    if lost > 0 {
        tracing::warn!(
            lost,
            "captured clips were discarded before they could be stored"
        );
        let capture = app.state::<SelectedCapture>();
        capture.note_dropped(lost);
        let _ = app.emit(EVENT_CAPTURE_STATE, capture.snapshot());
    }
    if buffer.is_empty() {
        return;
    }

    while let Some(clip) = buffer.pop() {
        let backend = app.state::<SelectedBackend>();
        match store(backend.inner(), &clip).await {
            Ok(None) => {}
            Ok(Some(item)) => announce(app, &item, clip.source, clip.at_ms).await,
            Err(error) => {
                // Back on the queue, and stop for this tick: whatever refused
                // this clip will refuse the next one, and draining the rest
                // into the same error would only lose their ordering.
                tracing::warn!(%error, "a captured clip could not be stored");
                buffer.requeue(clip);
                break;
            }
        }
    }
}

/// Tell the frontend that history changed and how the new item arrived.
async fn announce<R: Runtime>(
    app: &AppHandle<R>,
    item: &UiItem,
    source: CaptureSource,
    at_ms: i64,
) {
    {
        let capture = app.state::<SelectedCapture>();
        capture.note_stored(at_ms);
        let _ = app.emit(EVENT_CAPTURE_STATE, capture.snapshot());
    }
    let _ = app.emit(
        EVENT_CAPTURED,
        CapturedPayload {
            id: item.id().to_string(),
            source,
            is_sensitive: item.is_sensitive(),
        },
    );

    // The same event the desktop's push stream emits, so the list refreshes
    // through one code path on both platforms rather than two.
    let item_count = {
        let backend = app.state::<SelectedBackend>();
        backend.status().await.map(|s| s.item_count).unwrap_or(0)
    };
    let _ = app.emit(
        EVENT_CHANGED,
        ChangePayload {
            topic: EventKind::Items,
            item_count,
            // A capture, never a sweep: this build has no auto-wipe loop.
            swept: 0,
        },
    );
}

/// Whether a refusal is worth telling the user about rather than retrying.
///
/// `Unsupported` is structural — the ingest pipeline is not in this build — so
/// a retry loop would spin forever and the user would never learn that nothing
/// is being saved.
pub fn is_structural(error: &BackendError) -> bool {
    matches!(error, BackendError::Unsupported(_))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::testing::FakeBackend;

    fn clip(text: &str) -> Clip {
        Clip {
            text: text.into(),
            source: CaptureSource::Tile,
            at_ms: 1_700_000_000_000,
        }
    }

    #[tokio::test]
    async fn a_captured_clip_goes_through_the_one_ingest_path() {
        let backend = FakeBackend::running("2.0.0").accepting_adds();
        let stored = store(&backend, &clip("hello")).await.unwrap();
        assert!(stored.is_some());
        assert_eq!(backend.added(), vec!["hello".to_string()]);
    }

    /// Blank is not a capture. Rejected before the round trip, exactly as
    /// `add_item` rejects it.
    #[tokio::test]
    async fn whitespace_is_not_stored_and_is_not_an_error() {
        let backend = FakeBackend::running("2.0.0").accepting_adds();
        assert!(store(&backend, &clip("   \n\t ")).await.unwrap().is_none());
        assert!(backend.added().is_empty());
    }

    /// The dependency, stated as a test: until `capture::ingest` moves into
    /// `copypaste-core`, a capture on Android fails structurally rather than
    /// vanishing.
    #[tokio::test]
    async fn a_refusing_sink_produces_a_structural_error_rather_than_a_silent_drop() {
        let backend = FakeBackend::running("2.0.0");
        let err = store(&backend, &clip("hello")).await.unwrap_err();
        assert!(is_structural(&err), "{err:?}");
    }

    #[test]
    fn a_failed_clip_is_retried_before_anything_newer() {
        let mut buffer = Buffer::default();
        buffer.push_all([clip("first"), clip("second")]);
        let first = buffer.pop().unwrap();
        buffer.requeue(first);
        buffer.push_all([clip("third")]);
        assert_eq!(buffer.pop().unwrap().text, "first");
        assert_eq!(buffer.pop().unwrap().text, "second");
        assert_eq!(buffer.pop().unwrap().text, "third");
    }

    /// An overflow is a lost copy. It must be counted, because the user is
    /// owed the knowledge that history has a hole in it.
    #[test]
    fn overflowing_the_buffer_is_counted_rather_than_hidden() {
        let mut buffer = Buffer::default();
        buffer.push_all((0..MAX_BUFFERED + 5).map(|i| clip(&format!("clip {i}"))));
        assert_eq!(buffer.len(), MAX_BUFFERED);
        assert_eq!(buffer.dropped(), 5);
        // The oldest went, not the newest: the most recent copies are the ones
        // a user is most likely to still want.
        assert_eq!(buffer.pop().unwrap().text, "clip 5");
    }

    #[test]
    fn the_event_names_are_the_ones_the_frontend_listens_for() {
        assert_eq!(EVENT_CAPTURED, "copypaste://captured");
        assert_eq!(EVENT_CAPTURE_STATE, "copypaste://capture-state");
    }

    #[test]
    fn a_captured_payload_names_its_source_and_carries_no_content() {
        let json = serde_json::to_string(&CapturedPayload {
            id: "item-1".into(),
            source: CaptureSource::Share,
            is_sensitive: true,
        })
        .unwrap();
        assert_eq!(
            json,
            r#"{"id":"item-1","source":"share","isSensitive":true}"#
        );
    }
}
