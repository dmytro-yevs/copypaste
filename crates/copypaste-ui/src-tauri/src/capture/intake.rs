//! Getting a captured clip from the platform into the database.
//!
//! Every rung 0 surface — the share sheet, the text-selection action, the
//! Quick Settings tile — and rung 2's background listener end here, and all of
//! them end at [`crate::backend::Backend::add`]. There is no second ingest
//! path: `capture.rs` in the daemon records what happened when v1 had two
//! ("the IPC one forgot the dedup probe"), and dedup, secret detection,
//! eviction and the size cap are all decisions this file must not re-make
//! (AGENTS.md rule 1).
//!
//! # The backend is the one ingest path
//!
//! Android's embedded backend calls `copypaste_core::ingest`, the same shared
//! path used by the daemon. This module therefore only buffers clips around a
//! transient backend failure; it never decides deduplication, secret handling
//! or eviction itself. A structural error is still surfaced rather than
//! retried forever, because a copy the user believes was saved and was not is
//! the exact failure the android doc calls the worst outcome.
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
use std::future::Future;

use copypaste_ipc::EventKind;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Listener, Manager, Runtime};

use crate::backend::{Backend, BackendError, Result, SelectedBackend};
use crate::events::TauriEventName;
use crate::model::UiItem;
use crate::service::push::ChangePayload;

use super::model::{CaptureSource, Clip};
use super::{CaptureControl, SelectedCapture};

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
/// Bounded because a structural backend refusal cannot recover within a run,
/// and an unbounded queue of clipboard plaintext in memory is its own problem.
/// The count is surfaced, so the loss is visible.
const MAX_BUFFERED: usize = 128;

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export_to = "ipc.ts"))]
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

struct PrivateGate {
    desired: bool,
    applied: Option<bool>,
    clear_before_public: bool,
}

impl PrivateGate {
    fn starting(desired: bool) -> Self {
        Self {
            desired,
            applied: None,
            clear_before_public: desired,
        }
    }

    fn request(&mut self, enabled: bool) {
        if enabled {
            self.clear_before_public = true;
        }
        self.desired = enabled;
    }

    fn fail_closed(&self) -> bool {
        self.desired || self.applied != Some(false)
    }
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

    pub fn reject(&mut self) {
        self.dropped = self.dropped.saturating_add(1);
    }

    fn discard_all(&mut self) -> u64 {
        let lost = self.queue.len() as u64;
        self.dropped = self.dropped.saturating_add(lost);
        self.queue.clear();
        lost
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
    let item = backend
        .add_captured(
            &clip.text,
            clip.source_app_bundle_id.as_deref(),
            clip.source_app_name.as_deref(),
        )
        .await?;
    Ok(item.map(Into::into))
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
        sync_after_capture(app).await;
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
    let (private_mode_tx, mut private_mode_rx) = tokio::sync::mpsc::unbounded_channel();
    app.listen(TauriEventName::PrivateModeChanged.as_str(), move |event| {
        let Ok(enabled) = serde_json::from_str::<bool>(event.payload()) else {
            tracing::warn!("private mode changed without a boolean value");
            return;
        };
        let _ = private_mode_tx.send(enabled);
    });

    spawn_intake_task(async move {
        let mut buffer = Buffer::default();
        let initial = loop {
            let backend = app.state::<SelectedBackend>();
            match backend.get_private_mode().await {
                Ok(mode) => break mode.private_mode,
                Err(error) => {
                    tracing::debug!(%error, "private mode is not available yet");
                    tokio::time::sleep(DRAIN_INTERVAL).await;
                }
            }
        };
        let mut private_gate = PrivateGate::starting(initial);
        synchronize_private_mode(&app, &mut private_gate);

        while let Ok(enabled) = private_mode_rx.try_recv() {
            if enabled {
                let lost = buffer.discard_all();
                report_dropped(&app, lost);
            }
            private_gate.request(enabled);
            synchronize_private_mode(&app, &mut private_gate);
        }

        let mut drain_interval = tokio::time::interval(DRAIN_INTERVAL);
        drain_interval.tick().await;
        loop {
            tokio::select! {
                Some(enabled) = private_mode_rx.recv() => {
                    if enabled {
                        let lost = buffer.discard_all();
                        report_dropped(&app, lost);
                    }
                    private_gate.request(enabled);
                    synchronize_private_mode(&app, &mut private_gate);
                }
                _ = drain_interval.tick() => {
                    synchronize_private_mode(&app, &mut private_gate);
                    tick(&app, &mut buffer, private_gate.fail_closed()).await;
                }
            }
        }
    });
}

fn spawn_intake_task(task: impl Future<Output = ()> + Send + 'static) {
    tauri::async_runtime::spawn(task);
}

fn synchronize_private_mode<R: Runtime>(app: &AppHandle<R>, gate: &mut PrivateGate) {
    if gate.applied == Some(gate.desired) {
        return;
    }

    if !gate.desired && gate.clear_before_public {
        let capture = app.state::<SelectedCapture>();
        if capture.set_private_mode(true).is_err() {
            gate.applied = None;
            return;
        }
        gate.applied = Some(true);
        gate.clear_before_public = false;
    }

    let applied = {
        let capture = app.state::<SelectedCapture>();
        capture.set_private_mode(gate.desired)
    };
    match applied {
        Ok(()) => {
            gate.applied = Some(gate.desired);
            gate.clear_before_public = false;
        }
        Err(error) => {
            tracing::debug!(%error, "the platform private-mode gate is not available yet");
            gate.applied = None;
            if gate.desired {
                gate.clear_before_public = true;
            }
        }
    }
}

async fn tick<R: Runtime>(app: &AppHandle<R>, buffer: &mut Buffer, private_mode: bool) {
    let taken = take_platform_batch(private_mode, || {
        let capture = app.state::<SelectedCapture>();
        capture.drain()
    });
    let taken = match taken {
        Ok(clips) => clips,
        Err(error) => {
            tracing::debug!(%error, "the platform had nothing to hand over");
            Vec::new()
        }
    };
    push_captured(app, buffer, taken);
    drain_buffer(app, buffer).await;
}

fn take_platform_batch(
    private_mode: bool,
    drain: impl FnOnce() -> Result<Vec<Clip>>,
) -> Result<Vec<Clip>> {
    if private_mode {
        return Ok(Vec::new());
    }
    drain()
}

fn push_captured<R: Runtime>(app: &AppHandle<R>, buffer: &mut Buffer, taken: Vec<Clip>) {
    let had = buffer.dropped();
    buffer.push_all(taken);
    let lost = buffer.dropped() - had;
    report_dropped(app, lost);
}

fn report_dropped<R: Runtime>(app: &AppHandle<R>, lost: u64) {
    if lost > 0 {
        tracing::warn!(
            lost,
            "captured clips were discarded before they could be stored"
        );
        let capture = app.state::<SelectedCapture>();
        capture.note_dropped(lost);
        let _ = app.emit(TauriEventName::CaptureState.as_str(), capture.snapshot());
    }
}

async fn drain_buffer<R: Runtime>(app: &AppHandle<R>, buffer: &mut Buffer) {
    if buffer.is_empty() {
        return;
    }

    while let Some(clip) = buffer.pop() {
        let backend = app.state::<SelectedBackend>();
        match store(backend.inner(), &clip).await {
            Ok(None) => {}
            Ok(Some(item)) => {
                announce(app, &item, clip.source, clip.at_ms).await;
                sync_after_capture(app).await;
            }
            Err(error) => {
                tracing::warn!(%error, "a captured clip could not be stored");
                let had = buffer.dropped();
                if remember_store_failure(buffer, clip, &error) {
                    report_dropped(app, buffer.dropped() - had);
                } else {
                    break;
                }
            }
        }
    }
}

async fn sync_after_capture<R: Runtime>(app: &AppHandle<R>) {
    #[cfg(target_os = "android")]
    {
        let backend = app.state::<SelectedBackend>();
        let Ok(config) = backend.get_config().await else {
            return;
        };
        if !config.config.sync_enabled {
            return;
        }
        if let Err(error) = backend.sync(None).await {
            tracing::debug!(%error, "could not sync the captured item");
        }
    }

    #[cfg(not(target_os = "android"))]
    let _ = app;
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
        let _ = app.emit(TauriEventName::CaptureState.as_str(), capture.snapshot());
    }
    let _ = app.emit(
        TauriEventName::Captured.as_str(),
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
        TauriEventName::Changed.as_str(),
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
/// `Unsupported` is structural, so a retry loop would spin forever and the
/// user would never learn that nothing is being saved.
pub fn is_structural(error: &BackendError) -> bool {
    matches!(
        error,
        BackendError::Unsupported(_) | BackendError::Invalid(_)
    )
}

pub fn remember_store_failure(buffer: &mut Buffer, clip: Clip, error: &BackendError) -> bool {
    if is_structural(error) {
        buffer.reject();
        true
    } else {
        buffer.requeue(clip);
        false
    }
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
            source_app_bundle_id: None,
            source_app_name: None,
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

    /// A structural backend failure is surfaced rather than turned into a
    /// silent drop. The Android embedded backend itself accepts this path.
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

    #[test]
    fn an_oversized_clip_is_dropped_so_later_clips_still_drain() {
        let mut buffer = Buffer::default();
        buffer.push_all([clip("too large"), clip("still wanted")]);
        let oversized = buffer.pop().unwrap();

        let continue_drain = remember_store_failure(
            &mut buffer,
            oversized,
            &BackendError::Invalid("That item is larger than the size limit you set."),
        );

        assert!(continue_drain);
        assert_eq!(buffer.dropped(), 1);
        assert_eq!(buffer.pop().unwrap().text, "still wanted");
        assert!(buffer.is_empty());
    }

    #[test]
    fn a_transient_store_failure_is_retried_before_anything_newer() {
        let mut buffer = Buffer::default();
        buffer.push_all([clip("first"), clip("second")]);
        let first = buffer.pop().unwrap();

        let continue_drain = remember_store_failure(&mut buffer, first, &BackendError::Unreachable);

        assert!(!continue_drain);
        assert_eq!(buffer.dropped(), 0);
        assert_eq!(buffer.pop().unwrap().text, "first");
        assert_eq!(buffer.pop().unwrap().text, "second");
    }

    #[test]
    fn invalid_and_unsupported_refusals_are_structural() {
        assert!(is_structural(&BackendError::Invalid("too large")));
        assert!(is_structural(&BackendError::Unsupported("not yet")));
        assert!(!is_structural(&BackendError::Unreachable));
        assert!(!is_structural(&BackendError::NotReady));
    }

    #[test]
    fn platform_queue_keeps_ownership_until_the_privacy_gate_opens() {
        let called = std::cell::Cell::new(false);

        let taken = take_platform_batch(true, || {
            called.set(true);
            Ok(vec![clip("must remain on Android")])
        })
        .unwrap();

        assert!(taken.is_empty());
        assert!(!called.get());
    }

    #[test]
    fn public_gate_takes_one_platform_batch_exactly_once() {
        let calls = std::cell::Cell::new(0);

        let taken = take_platform_batch(false, || {
            calls.set(calls.get() + 1);
            Ok(vec![clip("one Android batch")])
        })
        .unwrap();

        assert_eq!(calls.get(), 1);
        assert_eq!(taken.len(), 1);
        assert_eq!(taken[0].text, "one Android batch");
    }

    #[test]
    fn intake_starts_without_a_caller_tokio_runtime() {
        let (sent, received) = std::sync::mpsc::sync_channel(1);
        spawn_intake_task(async move {
            sent.send(()).unwrap();
        });

        received
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("the Tauri executor did not start the intake task");
    }

    #[test]
    fn entering_private_mode_discards_clips_waiting_for_retry() {
        let mut buffer = Buffer::default();
        buffer.push_all([clip("failed before private mode")]);

        let lost = buffer.discard_all();

        assert!(buffer.is_empty());
        assert_eq!(lost, 1);
        assert_eq!(buffer.dropped(), 1);
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
