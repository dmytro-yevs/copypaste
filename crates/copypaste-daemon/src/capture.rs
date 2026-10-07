//! The capture loop, and the daemon's wrapper around the shared ingest path.
//!
//! The pipeline itself is [`copypaste_core::ingest_into`], re-exported here so
//! this crate's callers name it where they always did. It lives in the core
//! because Android links the core in-process and cannot depend on this crate,
//! which is a binary with no `lib` target. A second ingest path could bypass
//! dedup or retention.
//!
//! Manifest 01's data-loss rules that this file is responsible for:
//!
//! * **I-36** — no failure inside the pipeline may kill the poll loop. Every
//!   tick result is logged and the loop continues.
//! * Nothing acknowledges a capture without having stored it: the tick awaits
//!   the ingest before it returns, and shutdown is observed between ticks, not
//!   inside one.

use std::borrow::Borrow;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use copypaste_source_app::SourceAppIconCache;
use tokio::sync::watch;
use tracing::{debug, error, info, warn};

pub use copypaste_core::{IngestError, Ingested};

fn source_icon_cache() -> &'static SourceAppIconCache {
    static CACHE: std::sync::OnceLock<SourceAppIconCache> = std::sync::OnceLock::new();
    CACHE.get_or_init(SourceAppIconCache::default)
}

#[cfg(test)]
static TEST_PERSIST_MODE: Mutex<Option<TestPersistMode>> = Mutex::new(None);
#[cfg(test)]
static TEST_PERSIST_SERIAL: Mutex<()> = Mutex::new(());
#[cfg(test)]
static TEST_CAPTURE_PHASES: Mutex<TestCapturePhases> = Mutex::new(TestCapturePhases {
    busy_completed: None,
    drain: None,
    persist: None,
});

#[cfg(test)]
struct TestCapturePhases {
    busy_completed: Option<tokio::sync::oneshot::Sender<()>>,
    drain: Option<TestWorkerGate>,
    persist: Option<TestWorkerGate>,
}

#[cfg(test)]
struct TestWorkerGate {
    entered: tokio::sync::oneshot::Sender<()>,
    release: std::sync::mpsc::Receiver<()>,
}

#[cfg(test)]
enum TestWorkerPhase {
    Drain,
    Persist,
}

#[cfg(test)]
fn test_busy_completed() {
    let signal = TEST_CAPTURE_PHASES
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .busy_completed
        .take();
    if let Some(signal) = signal {
        let _ = signal.send(());
    }
}

#[cfg(test)]
fn test_worker_gate(phase: TestWorkerPhase) {
    let gate = {
        let mut phases = TEST_CAPTURE_PHASES
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        match phase {
            TestWorkerPhase::Drain => phases.drain.take(),
            TestWorkerPhase::Persist => phases.persist.take(),
        }
    };
    if let Some(gate) = gate {
        gate.entered.send(()).expect("worker phase observer");
        gate.release.recv().expect("worker phase release");
    }
}

#[cfg(test)]
#[derive(Clone, Copy)]
enum TestPersistMode {
    Busy,
    Failed,
    Panic,
}

use crate::AppState;

// Manifest 01 §4's 500 ms perceived-instant default now lives in
// `copypaste_ipc::ConfigData::default`, because it is a setting rather than a
// constant. Below ~100 ms the poll loop's own cost becomes visible; above ~5 s
// bursts become the norm rather than the exception, which is what the bounds in
// `copypaste_ipc::config` encode.

/// Poll the clipboard until shutdown.
///
/// The interval is read from the settings at every tick rather than captured
/// into a `tokio::time::Interval` once. That is what makes `poll_interval_ms` a
/// live setting.
pub async fn run(state: Arc<AppState>, mut shutdown: watch::Receiver<bool>) -> anyhow::Result<()> {
    state.set_capture_running(true);
    info!(
        backend = state.backend_name(),
        interval_ms = state.settings.get().poll_interval_ms,
        "clipboard capture started"
    );

    let pending = Arc::new(Mutex::new(None));
    let mut accepted_failure: Option<anyhow::Error> = None;

    let result = 'capture: loop {
        let wait = Duration::from_millis(state.settings.get().poll_interval_ms);
        tokio::select! {
            _ = shutdown.changed() => {
                while pending.lock().unwrap_or_else(|e| e.into_inner()).is_some() {
                    let worker_state = Arc::clone(&state);
                    let worker_pending = Arc::clone(&pending);
                    let result = tokio::task::spawn_blocking(move || drain_pending(&worker_state, &worker_pending)).await;
                    match result {
                        Ok(CaptureOutcome::Retried) => {
                            warn!("capture pending during shutdown is still transiently unavailable");
                        }
                        Ok(
                            CaptureOutcome::NoCapture
                            | CaptureOutcome::Stored
                            | CaptureOutcome::PolicyCancelled
                            | CaptureOutcome::InputRejected,
                        ) => {}
                        Ok(CaptureOutcome::Failed(error)) => {
                            break 'capture Err(accepted_failure.unwrap_or_else(|| anyhow::Error::new(error).context(
                                "the accepted clipboard capture could not be persisted during shutdown",
                            )));
                        }
                        Ok(CaptureOutcome::AuthorityUnavailable) => {
                            break 'capture Err(accepted_failure.unwrap_or_else(|| anyhow::anyhow!(
                                "the accepted clipboard capture lost its settings authority during shutdown",
                            )));
                        }
                        Err(error) => {
                            break 'capture Err(accepted_failure.unwrap_or_else(|| anyhow::Error::new(error).context(
                                "the accepted clipboard capture panicked during shutdown",
                            )));
                        }
                    }
                    if pending.lock().unwrap_or_else(|e| e.into_inner()).is_some() {
                        let interval_ms = state.settings.get().poll_interval_ms;
                        tokio::time::sleep(Duration::from_millis(interval_ms)).await;
                    }
                }
                break accepted_failure.map_or(Ok(()), Err);
            },
            // `sleep` rather than a ticker: a late tick must not cause a burst
            // of catch-up ticks — the clipboard has no backlog to drain, only a
            // current value — and the wait is recomputed each time round.
            _ = tokio::time::sleep(wait) => {
                // Bound to a local so the clipboard guard is released before
                // the handoff below rather than held across it.
                let changed = pending.lock().unwrap_or_else(|e| e.into_inner()).is_some() || state.clipboard().changed();
                if !changed {
                    continue;
                }

                let state = Arc::clone(&state);
                // The pasteboard read, the AEAD seal and the SQLite write are
                // all blocking. Running them on a worker keeps the reactor free
                // for the IPC server — and reaching it costs six thread wakeups,
                // which is why an idle clipboard stops short of here.
                let worker_pending = Arc::clone(&pending);
                match tokio::task::spawn_blocking(move || tick_slot(&state, &worker_pending)).await {
                    Ok(
                        CaptureOutcome::NoCapture
                        | CaptureOutcome::Stored
                        | CaptureOutcome::PolicyCancelled
                        | CaptureOutcome::InputRejected,
                    ) => {}
                    Ok(CaptureOutcome::Retried) => {
                        warn!("capture tick will retry transient storage failure");
                        #[cfg(test)]
                        test_busy_completed();
                    }
                    Ok(CaptureOutcome::Failed(error)) => {
                        warn!(error = ?error, "capture tick failed");
                    }
                    Ok(CaptureOutcome::AuthorityUnavailable) => {
                        warn!("accepted capture lost its settings authority");
                        accepted_failure.get_or_insert_with(|| anyhow::anyhow!(
                            "the accepted clipboard capture lost its settings authority before shutdown",
                        ));
                    }
                    // Manifest 01 I-36: a failed tick is logged, never fatal.
                    Err(error) => {
                        error!(error = %error, "capture task did not complete");
                        // Joining settles the worker before inspecting its slot.
                        // A panicked accepted payload has uncertain commit status
                        // and must never be replayed or called policy-cancelled.
                        if pending.lock().unwrap_or_else(|e| e.into_inner()).take().is_some() {
                            accepted_failure.get_or_insert_with(|| anyhow::Error::new(error).context(
                                "the accepted clipboard capture panicked before shutdown",
                            ));
                        }
                    }
                }
            }
        }
    };

    state.set_capture_running(false);
    info!("clipboard capture stopped");
    result
}

/// One poll. Returns `Ok(())` when there was nothing to capture.
enum CaptureOutcome {
    NoCapture,
    Retried,
    Stored,
    PolicyCancelled,
    InputRejected,
    Failed(IngestError),
    AuthorityUnavailable,
}
struct PendingCapture {
    capture: crate::clipboard::Capture,
    created_at: i64,
    privacy_epoch: u64,
}

fn tick_slot(state: &AppState, slot: &Mutex<Option<PendingCapture>>) -> CaptureOutcome {
    let mut slot = slot.lock().unwrap_or_else(|e| e.into_inner());
    tick(state, &mut slot)
}
fn drain_pending(state: &AppState, slot: &Mutex<Option<PendingCapture>>) -> CaptureOutcome {
    let mut slot = slot.lock().unwrap_or_else(|e| e.into_inner());
    #[cfg(test)]
    test_worker_gate(TestWorkerPhase::Drain);
    tick(state, &mut slot)
}
fn tick(state: &AppState, slot: &mut Option<PendingCapture>) -> CaptureOutcome {
    state
        .settings
        .with_capture_authority(|settings, privacy_epoch| {
            if slot.is_none() {
                // Drop the clipboard guard before storage and publication. The
                // settings authority remains held for this entire attempt.
                let capture = state
                    .clipboard()
                    .poll_with_policy(crate::clipboard::CapturePolicy::new(&settings));
                let Some(mut capture) = capture else {
                    return CaptureOutcome::NoCapture;
                };
                // A deferred desktop file becomes an immutable owned payload
                // before it is accepted into Pending, under the same authority.
                if !crate::clipboard::CapturePolicy::new(&settings).allows_materialized(&capture) {
                    return CaptureOutcome::PolicyCancelled;
                }
                if let Err(reason) = normalize_file_capture(&mut capture, &settings) {
                    reject_file_input(reason);
                    return CaptureOutcome::InputRejected;
                }
                *slot = Some(PendingCapture {
                    capture,
                    created_at: copypaste_core::now_ms(),
                    privacy_epoch,
                });
            }
            persist_pending(state, settings, privacy_epoch, slot)
        })
        .unwrap_or_else(|| {
            if slot.take().is_some() {
                CaptureOutcome::AuthorityUnavailable
            } else {
                CaptureOutcome::NoCapture
            }
        })
}
fn persist_pending(
    state: &AppState,
    settings: copypaste_ipc::ConfigData,
    privacy_epoch: u64,
    slot: &mut Option<PendingCapture>,
) -> CaptureOutcome {
    let pending = slot.as_ref().unwrap();
    if pending.privacy_epoch != privacy_epoch
        || !crate::clipboard::CapturePolicy::new(&settings).allows_materialized(&pending.capture)
    {
        *slot = None;
        return CaptureOutcome::PolicyCancelled;
    }
    #[cfg(test)]
    if let Some(outcome) = test_persist_outcome() {
        return outcome;
    }
    match ingest_capture(state, &settings, &pending.capture, pending.created_at) {
        Ok(Ingested::Stored(item)) => {
            if pending.capture.content_type == copypaste_ipc::content_type::FILE {
                info!("file capture stored");
            } else {
                debug!(id = %item.id, content_type = %item.content_type, "captured clipboard item");
            }
            // Wakes the watchers and pulls both sync loops to their floor, so a
            // copy here shows up over there in seconds rather than at whatever
            // interval the loops had drifted to. `note_capture` rather than
            // `note_local_change` because this is the one caller that knows the
            // change was a *copy*, which is what a client needs to decide
            // whether to notify (parity finding 18).
            announce_capture(state, item.created_at, &item.id, true);
            *slot = None;
            CaptureOutcome::Stored
        }
        Ok(Ingested::Duplicate(item)) => {
            if pending.capture.content_type == copypaste_ipc::content_type::FILE {
                info!("file capture deduplicated");
            } else {
                debug!(id = %item.id, "capture deduplicated against a recent item");
            }
            announce_capture(state, item.created_at, &item.id, false);
            *slot = None;
            CaptureOutcome::Stored
        }
        // An empty clipboard is not a failure, and there is nothing to store.
        Err(IngestError::Empty) => {
            *slot = None;
            CaptureOutcome::PolicyCancelled
        }
        // Over the size cap the user set. Reported once, at debug, rather than
        // as a tick failure: it is a decision they made, not a fault.
        Err(IngestError::TooLarge) => {
            debug!("clipboard item is over the configured size limit; not captured");
            *slot = None;
            CaptureOutcome::PolicyCancelled
        }
        Err(error) if retryable_storage_error(&error) => CaptureOutcome::Retried,
        Err(error) => {
            *slot = None;
            CaptureOutcome::Failed(error)
        }
    }
}

fn retryable_storage_error(error: &IngestError) -> bool {
    match error {
        IngestError::Storage(copypaste_core::StoreError::Sqlite(
            rusqlite::Error::SqliteFailure(error, _),
        )) => matches!(
            error.code,
            rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked
        ),
        IngestError::Storage(copypaste_core::StoreError::File(error)) => matches!(
            error.kind(),
            std::io::ErrorKind::Interrupted
                | std::io::ErrorKind::WouldBlock
                | std::io::ErrorKind::TimedOut
        ),
        _ => false,
    }
}

#[cfg(test)]
fn test_persist_outcome() -> Option<CaptureOutcome> {
    test_worker_gate(TestWorkerPhase::Persist);
    match *TEST_PERSIST_MODE.lock().unwrap_or_else(|e| e.into_inner()) {
        Some(TestPersistMode::Busy) => Some(CaptureOutcome::Retried),
        Some(TestPersistMode::Failed) => Some(CaptureOutcome::Failed(IngestError::Storage(
            copypaste_core::StoreError::InvalidKey,
        ))),
        Some(TestPersistMode::Panic) => panic!("test-only blocking ingest panic"),
        None => None,
    }
}

fn announce_capture(state: &AppState, created_at: i64, item_id: &str, saved: bool) {
    state.note_capture(created_at, item_id);
    crate::notify::on_capture(state, saved);
}

/// `settings` is the caller's snapshot, not a second read.
///
/// A `set_config` landing between the pasteboard read and the ingest let an
/// item pass `CapturePolicy`'s limit and then meet a different one here. One
/// snapshot per capture makes the gate and the ingest the same decision.
pub(crate) fn ingest_capture(
    state: &AppState,
    settings: &copypaste_ipc::ConfigData,
    capture: impl Borrow<crate::clipboard::Capture>,
    created_at: i64,
) -> Result<Ingested, IngestError> {
    let capture = capture.borrow();
    let payload_metadata = capture_metadata(capture);
    match (
        capture.content_type.as_str(),
        capture.binary_content.as_deref(),
        capture.file_path.as_ref(),
        capture.file_metadata.as_ref(),
    ) {
        (content_type, None, None, None)
            if crate::clipboard::format::supports(content_type)
                && copypaste_ipc::content_type::is_text(content_type) =>
        {
            copypaste_core::ingest::ingest_into_with_capture_source_metadata_with_current_retention(
                &state.store,
                &state.keyring,
                &capture.content,
                &capture.content_type,
                created_at,
                capture.app_bundle_id.as_deref(),
                capture.app_name.as_deref(),
                payload_metadata.as_ref(),
                settings,
                || state.settings.get().clone(),
            )
        }
        (content_type, Some(bytes), None, None)
            if capture.content.is_empty()
                && crate::clipboard::format::supports(content_type)
                && matches!(
                    copypaste_ipc::content_type::classify(content_type),
                    copypaste_ipc::ContentClass::Image
                ) =>
        {
            copypaste_core::ingest_binary_into_with_capture_source_metadata(
                &state.store,
                &state.keyring,
                bytes,
                content_type,
                created_at,
                capture.app_bundle_id.as_deref(),
                capture.app_name.as_deref(),
                payload_metadata.as_ref(),
                settings,
            )
        }
        (copypaste_ipc::content_type::FILE, Some(bytes), None, Some(metadata))
            if capture.content.is_empty() && !bytes.is_empty() && metadata.is_valid() =>
        {
            copypaste_core::ingest_binary_into_with_capture_source_metadata(
                &state.store,
                &state.keyring,
                bytes,
                copypaste_ipc::content_type::FILE,
                created_at,
                capture.app_bundle_id.as_deref(),
                capture.app_name.as_deref(),
                payload_metadata.as_ref(),
                settings,
            )
        }
        _ => Err(IngestError::Empty),
    }
}

fn capture_metadata(
    capture: &crate::clipboard::Capture,
) -> Option<copypaste_core::PayloadMetadata> {
    capture_metadata_with(capture, |app_id| {
        source_icon_cache().resolve_desktop(app_id)
    })
}

fn capture_metadata_with(
    capture: &crate::clipboard::Capture,
    resolver: impl FnOnce(&str) -> Option<copypaste_source_app::AppIcon>,
) -> Option<copypaste_core::PayloadMetadata> {
    let source_app_icon = capture
        .app_bundle_id
        .as_deref()
        .and_then(resolver)
        .and_then(|icon| {
            let png = STANDARD.decode(icon.png_base64).ok()?;
            copypaste_core::SourceAppIconMetadata::new(&png, icon.width, icon.height)
        });
    let metadata = copypaste_core::PayloadMetadata {
        file: capture.file_metadata.clone(),
        source_app_icon,
    };
    (metadata.file.is_some() || metadata.source_app_icon.is_some()).then_some(metadata)
}

fn reject_file_input(reason: crate::clipboard::file_capture::FileReadError) {
    warn!(message = reason.message());
}

fn normalize_file_capture(
    capture: &mut crate::clipboard::Capture,
    settings: &copypaste_ipc::ConfigData,
) -> Result<(), crate::clipboard::file_capture::FileReadError> {
    if let Some(path) = capture.file_path.as_ref() {
        let bytes = crate::clipboard::file_capture::read(
            path,
            settings.capture_limit_bytes(copypaste_ipc::content_type::FILE),
        )?;
        capture.binary_content = Some(bytes);
        capture.file_path = None;
        info!("file capture materialized");
    }
    Ok(())
}

pub fn ingest(
    state: &AppState,
    content: &str,
    content_type: &str,
) -> Result<Ingested, IngestError> {
    ingest_at(state, content, content_type, copypaste_core::now_ms())
}

/// [`ingest`] with the item's own timestamp, for an import.
///
/// A restored item keeps the moment it was originally captured, which is what
/// keeps a restored history in order and its ages honest. The dedup window is
/// applied around *that* stamp, not around now, so importing a file twice
/// collapses rather than doubling.
pub fn ingest_at(
    state: &AppState,
    content: &str,
    content_type: &str,
    created_at: i64,
) -> Result<Ingested, IngestError> {
    let settings = state.settings.get().clone();
    // A capture records no origin: `origin_device_id` is left empty and every
    // reader substitutes this device's id (`copypaste_core::origin_or`). The
    // alternative — stamping the id on every row — costs a column of repeated
    // UUIDs and an extra argument on a path that has no opinion about sync.
    copypaste_core::ingest::ingest_into_with_capture_source_with_current_retention(
        &state.store,
        &state.keyring,
        content,
        content_type,
        created_at,
        None,
        None,
        &settings,
        || state.settings.get().clone(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clipboard::windows_attribution::SourceApp;
    use crate::testutil::test_state;
    use copypaste_ipc::transport;
    use copypaste_ipc::{ErrorCode, Method, Request, Response, PROTOCOL_VERSION};
    use futures_util::StreamExt;
    use std::collections::VecDeque;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::io::AsyncWriteExt;
    use tokio_util::codec::{FramedRead, LinesCodec};

    struct TestPersistGuard {
        _serial: std::sync::MutexGuard<'static, ()>,
    }

    impl TestPersistGuard {
        fn set(mode: TestPersistMode) -> Self {
            let serial = TEST_PERSIST_SERIAL
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            *TEST_PERSIST_MODE.lock().unwrap_or_else(|e| e.into_inner()) = Some(mode);
            Self { _serial: serial }
        }

        fn busy_completed(&self) -> tokio::sync::oneshot::Receiver<()> {
            let (signal, completed) = tokio::sync::oneshot::channel();
            TEST_CAPTURE_PHASES
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .busy_completed = Some(signal);
            completed
        }

        fn gate(
            &self,
            phase: TestWorkerPhase,
        ) -> (
            tokio::sync::oneshot::Receiver<()>,
            std::sync::mpsc::Sender<()>,
        ) {
            let (entered, observer) = tokio::sync::oneshot::channel();
            let (release, worker) = std::sync::mpsc::channel();
            let gate = Some(TestWorkerGate {
                entered,
                release: worker,
            });
            let mut phases = TEST_CAPTURE_PHASES
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            match phase {
                TestWorkerPhase::Drain => phases.drain = gate,
                TestWorkerPhase::Persist => phases.persist = gate,
            }
            (observer, release)
        }
    }

    impl Drop for TestPersistGuard {
        fn drop(&mut self) {
            *TEST_PERSIST_MODE.lock().unwrap_or_else(|e| e.into_inner()) = None;
            let mut phases = TEST_CAPTURE_PHASES
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            phases.busy_completed = None;
            phases.drain = None;
            phases.persist = None;
        }
    }

    #[test]
    fn only_typed_transient_storage_errors_keep_a_capture_pending() {
        for code in [rusqlite::ffi::SQLITE_BUSY, rusqlite::ffi::SQLITE_LOCKED] {
            assert!(retryable_storage_error(&IngestError::Storage(
                copypaste_core::StoreError::Sqlite(rusqlite::Error::SqliteFailure(
                    rusqlite::ffi::Error::new(code),
                    None,
                )),
            )));
        }
        assert!(!retryable_storage_error(&IngestError::Storage(
            copypaste_core::StoreError::Sqlite(rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CONSTRAINT),
                None,
            )),
        )));
        for kind in [
            std::io::ErrorKind::Interrupted,
            std::io::ErrorKind::WouldBlock,
            std::io::ErrorKind::TimedOut,
        ] {
            assert!(retryable_storage_error(&IngestError::Storage(
                copypaste_core::StoreError::File(std::io::Error::from(kind)),
            )));
        }
        assert!(!retryable_storage_error(&IngestError::Storage(
            copypaste_core::StoreError::InvalidKey,
        )));
    }

    async fn shutdown_policy_cancels_pending_capture(
        name: &str,
        capture: crate::clipboard::Capture,
        patch: copypaste_ipc::ConfigPatch,
    ) {
        let polls = Arc::new(AtomicUsize::new(0));
        let (state, _dir) = crate::testutil::test_state_with_clipboard(
            name,
            Box::new(QueuedCapture {
                values: VecDeque::from([capture, captured("B", None)]),
                polls: Arc::clone(&polls),
            }),
        );
        let mut events = state.subscribe();
        let guard = TestPersistGuard::set(TestPersistMode::Busy);
        let busy_completed = guard.busy_completed();
        let task = tokio::spawn(run(Arc::clone(&state), state.shutdown_rx()));
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_millis(state.settings.get().poll_interval_ms)).await;
        busy_completed
            .await
            .expect("accepted capture completed its Busy attempt");
        state
            .settings
            .apply(&state.meta, &patch)
            .expect("apply policy");
        state.request_shutdown();
        tokio::time::advance(Duration::from_millis(state.settings.get().poll_interval_ms)).await;

        task.await
            .expect("capture task did not panic")
            .expect("policy cancellation is a clean terminal outcome");
        assert_eq!(
            polls.load(Ordering::SeqCst),
            1,
            "the newer capture was polled"
        );
        assert_eq!(
            state.store.count().unwrap(),
            0,
            "cancelled capture was stored"
        );
        assert!(matches!(
            events.try_recv(),
            Err(tokio::sync::broadcast::error::TryRecvError::Empty)
        ));
        drop(guard);
    }

    #[tokio::test(start_paused = true)]
    async fn shutdown_retry_cancels_a_pending_capture_when_private_mode_turns_on() {
        shutdown_policy_cancels_pending_capture(
            "pending-private-mode",
            captured("A", None),
            copypaste_ipc::ConfigPatch {
                private_mode: Some(true),
                ..Default::default()
            },
        )
        .await;
    }

    #[tokio::test(start_paused = true)]
    async fn shutdown_retry_cancels_a_pending_capture_when_its_app_is_excluded() {
        let app = SourceApp {
            id: "com.example.capture".into(),
            name: "Capture App".into(),
        };
        shutdown_policy_cancels_pending_capture(
            "pending-excluded-app",
            captured("A", Some(app)),
            copypaste_ipc::ConfigPatch {
                excluded_app_bundle_ids: Some(vec!["com.example.capture".into()]),
                ..Default::default()
            },
        )
        .await;
    }

    #[tokio::test(start_paused = true)]
    async fn shutdown_retry_cancels_a_pending_capture_when_exclusions_lack_attribution() {
        shutdown_policy_cancels_pending_capture(
            "pending-unknown-app",
            captured("A", None),
            copypaste_ipc::ConfigPatch {
                excluded_app_bundle_ids: Some(vec!["com.example.capture".into()]),
                ..Default::default()
            },
        )
        .await;
    }

    #[tokio::test(start_paused = true)]
    async fn shutdown_retry_cancels_a_pending_capture_above_the_new_text_limit() {
        let content = "A".repeat(copypaste_ipc::MIN_TEXT_SIZE_BYTES as usize + 1);
        shutdown_policy_cancels_pending_capture(
            "pending-text-limit",
            captured(&content, None),
            copypaste_ipc::ConfigPatch {
                max_text_size_bytes: Some(copypaste_ipc::MIN_TEXT_SIZE_BYTES),
                ..Default::default()
            },
        )
        .await;
    }

    fn captured(content: &str, app: Option<SourceApp>) -> crate::clipboard::Capture {
        crate::clipboard::Capture {
            content: content.to_string(),
            binary_content: None,
            file_path: None,
            file_metadata: None,
            content_type: copypaste_ipc::content_type::TEXT.to_string(),
            app_bundle_id: app.as_ref().map(|app| app.id.clone()),
            app_name: app.map(|app| app.name),
            source_policy: crate::clipboard::SourcePolicyEvidence::Legacy,
        }
    }

    struct QueuedCapture {
        values: VecDeque<crate::clipboard::Capture>,
        polls: Arc<AtomicUsize>,
    }
    impl crate::clipboard::ClipboardSource for QueuedCapture {
        fn poll(&mut self) -> Option<crate::clipboard::Capture> {
            self.polls.fetch_add(1, Ordering::SeqCst);
            self.values.pop_front()
        }
        fn poll_with_policy(
            &mut self,
            _: crate::clipboard::CapturePolicy<'_>,
        ) -> Option<crate::clipboard::Capture> {
            self.poll()
        }
        fn set_contents(&mut self, _: &str) -> anyhow::Result<()> {
            Ok(())
        }
        fn backend_name(&self) -> &'static str {
            "fake-queued"
        }
    }

    #[tokio::test(start_paused = true)]
    async fn shutdown_retries_busy_capture_past_the_soft_budget_without_polling_newer_value() {
        let polls = Arc::new(AtomicUsize::new(0));
        let (state, _dir) = crate::testutil::test_state_with_clipboard(
            "busy-drain",
            Box::new(QueuedCapture {
                values: VecDeque::from([captured("A", None), captured("B", None)]),
                polls: Arc::clone(&polls),
            }),
        );
        let endpoint = _dir.path().join("daemon.sock");
        let listener = crate::server::bind(&endpoint).expect("bind");
        let server = tokio::spawn(crate::server::run(
            listener,
            Arc::clone(&state),
            state.shutdown_rx(),
        ));
        let guard = TestPersistGuard::set(TestPersistMode::Busy);
        let busy_completed = guard.busy_completed();
        let task = tokio::spawn(run(Arc::clone(&state), state.shutdown_rx()));
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_millis(state.settings.get().poll_interval_ms)).await;
        busy_completed
            .await
            .expect("accepted capture completed its Busy attempt");
        state.request_shutdown();
        tokio::time::advance(crate::shutdown::TEARDOWN_BUDGET + Duration::from_secs(1)).await;
        assert_eq!(polls.load(Ordering::SeqCst), 1);
        assert_eq!(state.store.count().unwrap(), 0);
        assert!(!task.is_finished());
        assert!(
            crate::server::bind(&endpoint).is_err(),
            "the daemon released its endpoint before the busy capture settled"
        );
        let stream = transport::connect(&endpoint)
            .await
            .expect("draining endpoint");
        let (reader, mut writer) = stream.into_split();
        let request = Request {
            id: 91,
            protocol_version: PROTOCOL_VERSION,
            method: Method::Add {
                content: "must not enter during drain".into(),
            },
        };
        writer
            .write_all(serde_json::to_string(&request).unwrap().as_bytes())
            .await
            .expect("request");
        writer.write_all(b"\n").await.expect("frame end");
        writer.flush().await.expect("flush");
        let mut lines = FramedRead::new(reader, LinesCodec::new());
        let response: Response = serde_json::from_str(
            &lines
                .next()
                .await
                .expect("response frame")
                .expect("valid response frame"),
        )
        .expect("typed response");
        assert_eq!(response.id, 91);
        assert_eq!(response.error_code, Some(ErrorCode::NotReady));
        *TEST_PERSIST_MODE.lock().unwrap_or_else(|e| e.into_inner()) = None;
        tokio::time::advance(Duration::from_millis(state.settings.get().poll_interval_ms)).await;
        task.await.unwrap().unwrap();
        state.wait_for_admitted_requests().await;
        state.release_drain_listener();
        server.await.expect("listener task must not panic");
        drop(guard);
        assert_eq!(crate::testutil::contents(&state), ["A"]);
    }

    #[tokio::test(start_paused = true)]
    async fn shutdown_reports_a_permanent_failure_for_an_accepted_capture() {
        let polls = Arc::new(AtomicUsize::new(0));
        let (state, _dir) = crate::testutil::test_state_with_clipboard(
            "permanent-drain-failure",
            Box::new(QueuedCapture {
                values: VecDeque::from([captured("A", None)]),
                polls: Arc::clone(&polls),
            }),
        );
        let guard = TestPersistGuard::set(TestPersistMode::Busy);
        let busy_completed = guard.busy_completed();
        let (drain_entered, release_drain) = guard.gate(TestWorkerPhase::Drain);
        let task = tokio::spawn(run(Arc::clone(&state), state.shutdown_rx()));
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_millis(state.settings.get().poll_interval_ms)).await;
        busy_completed
            .await
            .expect("accepted capture completed its Busy attempt");
        state.request_shutdown();
        drain_entered
            .await
            .expect("actual shutdown drain worker entered");
        *TEST_PERSIST_MODE.lock().unwrap_or_else(|e| e.into_inner()) =
            Some(TestPersistMode::Failed);
        release_drain
            .send(())
            .expect("release shutdown drain fault");

        let error = task
            .await
            .expect("capture task did not panic")
            .expect_err("shutdown must fail when its accepted capture cannot persist");
        assert!(error.to_string().contains("could not be persisted"));
        assert_eq!(polls.load(Ordering::SeqCst), 1);
        assert_eq!(state.store.count().unwrap(), 0);
        drop(guard);
    }

    #[tokio::test(start_paused = true)]
    async fn shutdown_reports_a_blocking_panic_for_an_accepted_capture() {
        let (state, _dir) = crate::testutil::test_state_with_clipboard(
            "panic-drain-failure",
            Box::new(QueuedCapture {
                values: VecDeque::from([captured("A", None)]),
                polls: Arc::new(AtomicUsize::new(0)),
            }),
        );
        let guard = TestPersistGuard::set(TestPersistMode::Busy);
        let busy_completed = guard.busy_completed();
        let (drain_entered, release_drain) = guard.gate(TestWorkerPhase::Drain);
        let task = tokio::spawn(run(Arc::clone(&state), state.shutdown_rx()));
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_millis(state.settings.get().poll_interval_ms)).await;
        busy_completed
            .await
            .expect("accepted capture completed its Busy attempt");
        state.request_shutdown();
        drain_entered
            .await
            .expect("actual shutdown drain worker entered");
        *TEST_PERSIST_MODE.lock().unwrap_or_else(|e| e.into_inner()) = Some(TestPersistMode::Panic);
        release_drain
            .send(())
            .expect("release shutdown drain fault");

        let error = task
            .await
            .expect("capture task did not panic")
            .expect_err("shutdown must fail when its blocking drain panics");
        assert!(error.to_string().contains("panicked during shutdown"));
        drop(guard);
    }

    #[tokio::test(start_paused = true)]
    async fn shutdown_retains_an_already_running_ticks_accepted_panic() {
        let polls = Arc::new(AtomicUsize::new(0));
        let (state, _dir) = crate::testutil::test_state_with_clipboard(
            "accepted-tick-panic",
            Box::new(QueuedCapture {
                values: VecDeque::from([captured("A", None), captured("B", None)]),
                polls: polls.clone(),
            }),
        );
        let mut events = state.subscribe();
        let guard = TestPersistGuard::set(TestPersistMode::Panic);
        let (persist_entered, release_tick) = guard.gate(TestWorkerPhase::Persist);
        let task = tokio::spawn(run(Arc::clone(&state), state.shutdown_rx()));
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_millis(state.settings.get().poll_interval_ms)).await;
        persist_entered
            .await
            .expect("ordinary tick accepted Pending before shutdown");
        assert!(state.capture_running());
        state.request_shutdown();
        release_tick
            .send(())
            .expect("release the already-running ordinary tick");

        let error = task
            .await
            .expect("capture loop must not panic")
            .expect_err("accepted ordinary tick panic must make shutdown fail");
        assert!(error.to_string().contains("panicked before shutdown"));
        assert!(error
            .downcast_ref::<tokio::task::JoinError>()
            .unwrap()
            .is_panic());
        assert!(!state.capture_running());
        assert_eq!(polls.load(Ordering::SeqCst), 1);
        assert_eq!(state.store.count().unwrap(), 0);
        assert!(events.try_recv().is_err());
        assert!(state.settings.with_capture_authority(|_, _| ()).is_none());
        drop(guard);
    }

    #[test]
    fn accepted_pending_authority_refusal_is_a_terminal_failure_without_replay() {
        let polls = Arc::new(AtomicUsize::new(0));
        let (state, _dir) = crate::testutil::test_state_with_clipboard(
            "pending-authority-refusal",
            Box::new(QueuedCapture {
                values: VecDeque::from([captured("A", None), captured("B", None)]),
                polls: polls.clone(),
            }),
        );
        let guard = TestPersistGuard::set(TestPersistMode::Busy);
        let mut events = state.subscribe();
        let mut pending = None;
        assert!(matches!(
            tick(&state, &mut pending),
            CaptureOutcome::Retried
        ));
        let worker = state.clone();
        assert!(std::thread::spawn(move || worker
            .settings
            .with_capture_authority(|_, _| panic!("test-only authority poison")))
        .join()
        .is_err());
        assert!(matches!(
            tick(&state, &mut pending),
            CaptureOutcome::AuthorityUnavailable
        ));
        assert!(pending.is_none());
        assert!(matches!(
            tick(&state, &mut pending),
            CaptureOutcome::NoCapture
        ));
        assert_eq!(polls.load(Ordering::SeqCst), 1);
        assert_eq!(state.store.count().unwrap(), 0);
        assert!(events.try_recv().is_err());
        drop(guard);
    }

    #[tokio::test(start_paused = true)]
    async fn shutdown_reports_unavailable_authority_for_an_accepted_capture() {
        let polls = Arc::new(AtomicUsize::new(0));
        let (state, _dir) = crate::testutil::test_state_with_clipboard(
            "pending-authority-shutdown",
            Box::new(QueuedCapture {
                values: VecDeque::from([captured("A", None), captured("B", None)]),
                polls: polls.clone(),
            }),
        );
        let mut events = state.subscribe();
        let guard = TestPersistGuard::set(TestPersistMode::Busy);
        let busy_completed = guard.busy_completed();
        let (drain_entered, release_drain) = guard.gate(TestWorkerPhase::Drain);
        let task = tokio::spawn(run(Arc::clone(&state), state.shutdown_rx()));
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_millis(state.settings.get().poll_interval_ms)).await;
        busy_completed
            .await
            .expect("accepted capture completed its Busy attempt");
        state.request_shutdown();
        drain_entered
            .await
            .expect("actual shutdown drain worker entered");
        let worker = state.clone();
        assert!(std::thread::spawn(move || worker
            .settings
            .with_capture_authority(|_, _| panic!("test-only authority poison")))
        .join()
        .is_err());
        release_drain
            .send(())
            .expect("release unavailable-authority drain");
        let error = task
            .await
            .expect("capture loop must not panic")
            .expect_err("accepted capture authority refusal must make shutdown fail");
        assert!(error
            .to_string()
            .contains("lost its settings authority during shutdown"));
        assert!(!state.capture_running());
        assert_eq!(polls.load(Ordering::SeqCst), 1);
        assert_eq!(state.store.count().unwrap(), 0);
        assert!(events.try_recv().is_err());
        drop(guard);
    }

    #[tokio::test(start_paused = true)]
    async fn a_pre_acceptance_tick_panic_keeps_the_loop_alive_and_authority_closed() {
        struct PanickingSource {
            entered: Option<tokio::sync::oneshot::Sender<()>>,
            release: std::sync::mpsc::Receiver<()>,
            polls: Arc<AtomicUsize>,
        }
        impl crate::clipboard::ClipboardSource for PanickingSource {
            fn poll(&mut self) -> Option<crate::clipboard::Capture> {
                self.polls.fetch_add(1, Ordering::SeqCst);
                self.entered.take().unwrap().send(()).unwrap();
                self.release.recv().unwrap();
                panic!("test-only source panic before acceptance");
            }
            fn poll_with_policy(
                &mut self,
                _: crate::clipboard::CapturePolicy<'_>,
            ) -> Option<crate::clipboard::Capture> {
                self.poll()
            }
            fn set_contents(&mut self, _: &str) -> anyhow::Result<()> {
                Ok(())
            }
            fn backend_name(&self) -> &'static str {
                "fake-pre-acceptance-panic"
            }
        }
        let _serial = TEST_PERSIST_SERIAL
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let polls = Arc::new(AtomicUsize::new(0));
        let (entered, observer) = tokio::sync::oneshot::channel();
        let (release, worker) = std::sync::mpsc::channel();
        let (state, _dir) = crate::testutil::test_state_with_clipboard(
            "pre-acceptance-panic",
            Box::new(PanickingSource {
                entered: Some(entered),
                release: worker,
                polls: polls.clone(),
            }),
        );
        let task = tokio::spawn(run(Arc::clone(&state), state.shutdown_rx()));
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_millis(state.settings.get().poll_interval_ms)).await;
        observer
            .await
            .expect("ordinary tick entered source before acceptance");
        state.request_shutdown();
        release.send(()).expect("release pre-acceptance panic");
        task.await
            .expect("capture loop must not panic")
            .expect("pre-acceptance failure remains a logged tick failure");
        assert!(!state.capture_running());
        assert!(matches!(tick(&state, &mut None), CaptureOutcome::NoCapture));
        assert_eq!(polls.load(Ordering::SeqCst), 1);
        assert_eq!(state.store.count().unwrap(), 0);
    }

    #[test]
    fn image_capture_values_are_stored_as_binary_without_search_text() {
        let (state, _dir) = test_state("image-capture");
        let bytes = vec![0x89, b'P', b'N', b'G', 7];
        let stored = ingest_capture(
            &state,
            &state.settings.get(),
            crate::clipboard::Capture {
                content: String::new(),
                binary_content: Some(bytes.clone()),
                file_path: None,
                file_metadata: None,
                content_type: copypaste_ipc::content_type::IMAGE_PNG.to_string(),
                app_bundle_id: None,
                app_name: None,
                source_policy: crate::clipboard::SourcePolicyEvidence::Legacy,
            },
            copypaste_core::now_ms(),
        )
        .unwrap()
        .into_item();
        assert_eq!(stored.content_type, copypaste_ipc::content_type::IMAGE_PNG);
        assert_eq!(state.store.search("PNG", 10).unwrap(), Vec::new());
        let opened = copypaste_core::open_binary(
            &stored.content_ciphertext,
            &state.keyring.item_key(),
            &stored.id,
        )
        .unwrap();
        assert_eq!(opened.as_slice(), bytes.as_slice());
    }

    #[test]
    fn file_capture_reads_one_bounded_local_file_and_persists_its_path() {
        let (state, dir) = test_state("file-capture");
        let path = dir.path().join("fixture.bin");
        let bytes = b"synthetic file fixture".to_vec();
        std::fs::write(&path, &bytes).unwrap();
        let metadata = copypaste_core::FileMetadata::with_source_reference(
            "fixture.bin",
            "application/octet-stream",
            path.to_string_lossy(),
        )
        .unwrap();

        let mut capture = crate::clipboard::Capture {
            content: String::new(),
            binary_content: None,
            file_path: Some(path),
            file_metadata: Some(metadata.clone()),
            content_type: copypaste_ipc::content_type::FILE.to_string(),
            app_bundle_id: None,
            app_name: None,
            source_policy: crate::clipboard::SourcePolicyEvidence::Legacy,
        };
        normalize_file_capture(&mut capture, &state.settings.get()).unwrap();
        let stored = ingest_capture(
            &state,
            &state.settings.get(),
            &capture,
            copypaste_core::now_ms(),
        )
        .unwrap()
        .into_item();

        assert_eq!(stored.content_type, copypaste_ipc::content_type::FILE);
        assert_eq!(
            stored
                .payload_metadata
                .as_deref()
                .and_then(|value| copypaste_core::PayloadMetadata::from_json(
                    value,
                    copypaste_ipc::content_type::FILE,
                ))
                .and_then(|metadata| metadata.file),
            Some(metadata)
        );
        assert!(state.store.search("fixture", 10).unwrap().is_empty());
        let opened = copypaste_core::open_binary(
            &stored.content_ciphertext,
            &state.keyring.item_key(),
            &stored.id,
        )
        .unwrap();
        assert_eq!(opened.as_slice(), bytes.as_slice());
    }

    #[test]
    fn file_capture_rejects_a_file_above_its_configured_limit() {
        let (state, dir) = test_state("oversized-file-capture");
        let settings = copypaste_ipc::ConfigData {
            max_file_size_bytes: copypaste_ipc::MIN_FILE_SIZE_BYTES,
            ..Default::default()
        };
        let path = dir.path().join("oversized.bin");
        std::fs::write(
            &path,
            vec![0; copypaste_ipc::MIN_FILE_SIZE_BYTES as usize + 1],
        )
        .unwrap();
        let metadata =
            copypaste_core::FileMetadata::new("oversized.bin", "application/octet-stream").unwrap();

        let mut capture = crate::clipboard::Capture {
            content: String::new(),
            binary_content: None,
            file_path: Some(path),
            file_metadata: Some(metadata),
            content_type: copypaste_ipc::content_type::FILE.to_string(),
            app_bundle_id: None,
            app_name: None,
            source_policy: crate::clipboard::SourcePolicyEvidence::Legacy,
        };
        assert_eq!(
            normalize_file_capture(&mut capture, &settings),
            Err(crate::clipboard::file_capture::FileReadError::TooLarge)
        );
        assert_eq!(state.store.count().unwrap(), 0);
    }

    #[test]
    fn rich_text_and_html_captures_use_the_text_ingest_path() {
        let (state, _dir) = test_state("rich-text-capture");
        for (content, content_type) in [
            ("{\\rtf1 synthetic}", copypaste_ipc::content_type::RICH_TEXT),
            ("<p>synthetic</p>", copypaste_ipc::content_type::HTML),
        ] {
            let stored = ingest_capture(
                &state,
                &state.settings.get(),
                crate::clipboard::Capture {
                    content: content.to_string(),
                    binary_content: None,
                    file_path: None,
                    file_metadata: None,
                    content_type: content_type.to_string(),
                    app_bundle_id: None,
                    app_name: None,
                    source_policy: crate::clipboard::SourcePolicyEvidence::Legacy,
                },
                copypaste_core::now_ms(),
            )
            .unwrap()
            .into_item();
            assert_eq!(stored.content_type, content_type);
        }
        assert_eq!(state.store.search("synthetic", 10).unwrap().len(), 2);
    }

    #[test]
    fn capture_keeps_its_admission_snapshot_when_retention_reads_live_settings() {
        let (state, _dir) = test_state("capture-ingress-snapshot");
        let snapshot = state.settings.get().clone();
        state
            .settings
            .apply(
                &state.meta,
                &copypaste_ipc::ConfigPatch {
                    max_text_size_bytes: Some(copypaste_ipc::MIN_TEXT_SIZE_BYTES),
                    ..Default::default()
                },
            )
            .unwrap();
        let content = "x".repeat(copypaste_ipc::MIN_TEXT_SIZE_BYTES as usize + 1);

        assert!(ingest_capture(
            &state,
            &snapshot,
            captured(&content, None),
            copypaste_core::now_ms(),
        )
        .is_ok());
    }

    struct OnceCapture {
        inner: Option<crate::clipboard::Capture>,
    }

    impl crate::clipboard::ClipboardSource for OnceCapture {
        fn poll(&mut self) -> Option<crate::clipboard::Capture> {
            self.inner.take()
        }

        fn poll_with_policy(
            &mut self,
            _policy: crate::clipboard::CapturePolicy<'_>,
        ) -> Option<crate::clipboard::Capture> {
            self.poll()
        }

        fn set_contents(&mut self, _text: &str) -> anyhow::Result<()> {
            Ok(())
        }

        fn backend_name(&self) -> &'static str {
            "fake-memory"
        }
    }

    #[test]
    fn a_recopy_wakes_the_ui_and_sync_like_a_fresh_capture() {
        // `persist_pending` has test-only fault injection shared by the async
        // shutdown tests above. Keep this normal-path assertion out of that
        // injected scope, or parallel test execution can turn its recopy into
        // a synthetic retry.
        let _serial = TEST_PERSIST_SERIAL
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let content = "the same clipping again";
        let (state, _dir) = crate::testutil::test_state_with_clipboard(
            "recopy-wake",
            Box::new(OnceCapture {
                inner: Some(captured(content, None)),
            }),
        );
        // Seed a strictly older stamp; consecutive writes may share one millisecond.
        let first = ingest_at(
            &state,
            content,
            copypaste_ipc::content_type::TEXT,
            copypaste_core::now_ms().saturating_sub(1),
        )
        .unwrap()
        .into_item();
        assert_eq!(state.store.count().unwrap(), 1);

        let mut events = state.subscribe();
        let mut pending = None;
        assert!(matches!(tick(&state, &mut pending), CaptureOutcome::Stored));

        let after = state.store.get(&first.id).unwrap().unwrap();
        assert!(
            after.created_at > first.created_at,
            "the recopy never reached ingest"
        );
        assert_eq!(state.store.count().unwrap(), 1);

        let event = events
            .try_recv()
            .expect("a recopy must publish a capture event");
        assert!(
            event.captured,
            "UI watchers and notify_on_copy stay asleep on recopy"
        );
        assert_eq!(event.item_count, 1);
    }
    #[test]
    fn pending_busy_retains_payload_evidence_and_timestamp_without_repolling() {
        let polls = Arc::new(AtomicUsize::new(0));
        let (state, _dir) = crate::testutil::test_state_with_clipboard(
            "frozen-pending",
            Box::new(QueuedCapture {
                values: VecDeque::from([captured("frozen", None), captured("newer", None)]),
                polls: polls.clone(),
            }),
        );
        let guard = TestPersistGuard::set(TestPersistMode::Busy);
        let mut slot = None;
        assert!(matches!(tick(&state, &mut slot), CaptureOutcome::Retried));
        let frozen = slot.as_ref().unwrap().capture.clone();
        let created_at = slot.as_ref().unwrap().created_at;
        assert!(matches!(tick(&state, &mut slot), CaptureOutcome::Retried));
        assert_eq!(slot.as_ref().unwrap().capture, frozen);
        assert_eq!(slot.as_ref().unwrap().created_at, created_at);
        assert_eq!(polls.load(Ordering::SeqCst), 1);
        *TEST_PERSIST_MODE.lock().unwrap_or_else(|e| e.into_inner()) = None;
        assert!(matches!(tick(&state, &mut slot), CaptureOutcome::Stored));
        assert_eq!(crate::testutil::contents(&state), ["frozen"]);
        assert_eq!(polls.load(Ordering::SeqCst), 1);
        drop(guard);
    }

    fn deferred_file(path: &std::path::Path) -> crate::clipboard::Capture {
        crate::clipboard::Capture {
            content: String::new(),
            binary_content: None,
            file_path: Some(path.to_owned()),
            file_metadata: Some(
                copypaste_core::FileMetadata::with_source_reference(
                    path.file_name().unwrap().to_str().unwrap(),
                    "application/octet-stream",
                    path.to_str().unwrap(),
                )
                .unwrap(),
            ),
            content_type: copypaste_ipc::content_type::FILE.into(),
            // Invalid package syntax keeps this fake's icon resolution off native APIs.
            app_bundle_id: Some("SyntheticOwner".into()),
            app_name: Some("Synthetic owner".into()),
            source_policy: crate::clipboard::SourcePolicyEvidence::Legacy,
        }
    }

    #[test]
    fn file_pending_freezes_bytes_metadata_evidence_and_time_across_busy_mutation_and_deletion() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("private-file-token");
        let original = b"private payload token".to_vec();
        std::fs::write(&path, &original).unwrap();
        let polls = Arc::new(AtomicUsize::new(0));
        let (state, _store_dir) = crate::testutil::test_state_with_clipboard(
            "fake-frozen-file",
            Box::new(QueuedCapture {
                values: VecDeque::from([deferred_file(&path)]),
                polls: polls.clone(),
            }),
        );
        let mut events = state.subscribe();
        let guard = TestPersistGuard::set(TestPersistMode::Busy);
        crate::clipboard::file_capture::reset_test_open_count();
        let mut slot = None;
        assert!(matches!(tick(&state, &mut slot), CaptureOutcome::Retried));
        let frozen = slot.as_ref().unwrap().capture.clone();
        let time = slot.as_ref().unwrap().created_at;
        assert_eq!(frozen.binary_content.as_deref(), Some(original.as_slice()));
        assert!(frozen.file_path.is_none());
        assert!(events.try_recv().is_err());
        std::fs::write(&path, b"replacement token").unwrap();
        assert!(matches!(tick(&state, &mut slot), CaptureOutcome::Retried));
        assert_eq!(slot.as_ref().unwrap().capture, frozen);
        assert_eq!(slot.as_ref().unwrap().created_at, time);
        std::fs::remove_file(&path).unwrap();
        *TEST_PERSIST_MODE.lock().unwrap_or_else(|e| e.into_inner()) = None;
        assert!(matches!(tick(&state, &mut slot), CaptureOutcome::Stored));
        assert_eq!(polls.load(Ordering::SeqCst), 1);
        assert_eq!(crate::clipboard::file_capture::test_open_count(), 1);
        assert!(slot.is_none());
        assert!(events.try_recv().unwrap().captured);
        let stored = state
            .store
            .get(&copypaste_core::binary_item_id(&original))
            .unwrap()
            .unwrap();
        assert_eq!(stored.created_at, time);
        assert_eq!(stored.app_bundle_id.as_deref(), Some("SyntheticOwner"));
        assert_eq!(
            copypaste_core::open_binary(
                &stored.content_ciphertext,
                &state.keyring.item_key(),
                &stored.id
            )
            .unwrap()
            .as_slice(),
            original
        );
        assert_eq!(
            stored
                .payload_metadata
                .as_deref()
                .and_then(|m| copypaste_core::PayloadMetadata::from_json(
                    m,
                    copypaste_ipc::content_type::FILE
                ))
                .and_then(|m| m.file),
            frozen.file_metadata
        );
        drop(guard);
    }
    #[test]
    fn file_capture_logs_fixed_outcomes_without_locator_payload_or_identity() {
        let _serial = TEST_PERSIST_SERIAL
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("secret-locator-token");
        std::fs::write(&path, b"secret-payload-token").unwrap();
        let mut value = deferred_file(&path);
        value.app_bundle_id = Some("secret-app-token".into());
        value.app_name = Some("secret-app-name-token".into());
        let (state, _store_dir) = crate::testutil::test_state_with_clipboard(
            "fake-safe-file-log",
            Box::new(OnceCapture { inner: Some(value) }),
        );
        let (_, captured) = copypaste_runtime_log::test_support::capture(|| {
            assert!(matches!(tick(&state, &mut None), CaptureOutcome::Stored));
            tracing::info!(
                private_field = "secret-structured-token",
                "formatter field-drop control"
            );
            tracing::debug!("secret-debug-token");
        });
        assert!(!captured.contains("secret-structured-token"));
        assert!(!captured.contains("secret-debug-token"));
        assert_eq!(captured.matches("file capture materialized").count(), 1);
        assert_eq!(captured.matches("file capture stored").count(), 1);
        for token in [
            "secret-locator-token",
            "secret-payload-token",
            "secret-app-token",
            "secret-app-name-token",
            path.to_str().unwrap(),
        ] {
            assert!(!captured.contains(token));
        }
        let (state, _store_dir) = crate::testutil::test_state_with_clipboard(
            "fake-safe-file-reject-log",
            Box::new(OnceCapture {
                inner: Some(deferred_file(&dir.path().join("secret-missing-token"))),
            }),
        );
        let (_, rejected) = copypaste_runtime_log::test_support::capture(|| {
            assert!(matches!(
                tick(&state, &mut None),
                CaptureOutcome::InputRejected
            ));
            assert!(matches!(tick(&state, &mut None), CaptureOutcome::NoCapture));
        });
        assert_eq!(rejected.matches("file capture input rejected").count(), 1);
        assert!(rejected.contains("file capture input rejected: source not found"));
        assert!(!rejected.contains("secret-missing-token"));
    }

    #[test]
    fn every_reader_rejection_survives_actual_default_runtime_formatter() {
        use crate::clipboard::file_capture::FileReadError::*;
        let reasons = [
            OpenPermissionDenied,
            OpenNotFound,
            OpenFailed,
            StatFailed,
            NonRegular,
            TooLarge,
            UnsupportedEmptyFile,
            AllocationFailed,
            ReadPermissionDenied,
            ReadFailed,
            SourceChanged,
        ];
        let (_, log) = copypaste_runtime_log::test_support::capture(|| {
            for reason in reasons {
                reject_file_input(reason);
            }
            tracing::warn!(
                error = "secret-localized-os-error",
                filename = "secret-filename",
                url = "file:///secret-url",
                bytes = "secret-payload",
                "formatter boundary control"
            );
        });
        assert_eq!(
            log.matches("file capture input rejected:").count(),
            reasons.len()
        );
        let mut distinct = std::collections::HashSet::new();
        for reason in reasons {
            let message = reason.message();
            assert!(distinct.insert(message));
            assert_eq!(
                log.lines().filter(|line| line.ends_with(message)).count(),
                1
            );
        }
        for token in [
            "secret-localized-os-error",
            "secret-filename",
            "secret-url",
            "secret-payload",
        ] {
            assert!(!log.contains(token));
        }
        assert!(log.contains("formatter boundary control"));
    }
    #[test]
    fn default_runtime_log_materializes_once_across_busy_and_distinguishes_duplicate_commit() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("secret-retry-file");
        std::fs::write(&path, b"secret-retry-bytes").unwrap();
        let (state, _store_dir) = crate::testutil::test_state_with_clipboard(
            "fake-file-log-retry",
            Box::new(QueuedCapture {
                values: VecDeque::from([deferred_file(&path), deferred_file(&path)]),
                polls: Arc::new(AtomicUsize::new(0)),
            }),
        );
        let guard = TestPersistGuard::set(TestPersistMode::Busy);
        let mut slot = None;
        let (_, log) = copypaste_runtime_log::test_support::capture(|| {
            assert!(matches!(tick(&state, &mut slot), CaptureOutcome::Retried));
            assert!(matches!(tick(&state, &mut slot), CaptureOutcome::Retried));
            *TEST_PERSIST_MODE.lock().unwrap_or_else(|e| e.into_inner()) = None;
            assert!(matches!(tick(&state, &mut slot), CaptureOutcome::Stored));
        });
        assert_eq!(log.matches("file capture materialized").count(), 1);
        assert_eq!(log.matches("file capture stored").count(), 1);
        assert!(!log.contains("file capture deduplicated"));
        let (_, duplicate) = copypaste_runtime_log::test_support::capture(|| {
            assert!(matches!(tick(&state, &mut slot), CaptureOutcome::Stored));
        });
        assert_eq!(duplicate.matches("file capture materialized").count(), 1);
        assert_eq!(duplicate.matches("file capture deduplicated").count(), 1);
        assert!(!duplicate.contains("file capture stored"));
        for output in [&log, &duplicate] {
            assert!(!output.contains("secret-retry-file"));
            assert!(!output.contains("secret-retry-bytes"));
        }
        drop(guard);
    }

    #[test]
    fn admitted_file_read_commit_and_announcement_finish_before_settings_response() {
        struct BarrierFile {
            entered: std::sync::mpsc::Sender<()>,
            release: std::sync::mpsc::Receiver<()>,
            path: std::path::PathBuf,
        }
        impl crate::clipboard::ClipboardSource for BarrierFile {
            fn poll(&mut self) -> Option<crate::clipboard::Capture> {
                self.entered.send(()).unwrap();
                self.release.recv().unwrap();
                Some(deferred_file(&self.path))
            }
            fn poll_with_policy(
                &mut self,
                _: crate::clipboard::CapturePolicy<'_>,
            ) -> Option<crate::clipboard::Capture> {
                self.poll()
            }
            fn set_contents(&mut self, _: &str) -> anyhow::Result<()> {
                Ok(())
            }
            fn backend_name(&self) -> &'static str {
                "fake-file-barrier"
            }
        }
        let _serial = TEST_PERSIST_SERIAL
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        for narrow in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("file");
            std::fs::write(&path, b"file bytes").unwrap();
            let (entered_tx, entered_rx) = std::sync::mpsc::channel();
            let (release_tx, release_rx) = std::sync::mpsc::channel();
            let (state, _store_dir) = crate::testutil::test_state_with_clipboard(
                "fake-file-settings-order",
                Box::new(BarrierFile {
                    entered: entered_tx,
                    release: release_rx,
                    path,
                }),
            );
            let mut events = state.subscribe();
            let worker = state.clone();
            let capture = std::thread::spawn(move || {
                crate::clipboard::file_capture::reset_test_open_count();
                let result = tick(&worker, &mut None);
                (result, crate::clipboard::file_capture::test_open_count())
            });
            entered_rx.recv().unwrap();
            let (started_tx, started_rx) = std::sync::mpsc::channel();
            let (response_tx, response_rx) = std::sync::mpsc::channel();
            let worker = state.clone();
            let setter = std::thread::spawn(move || {
                started_tx.send(()).unwrap();
                let method = if narrow {
                    Method::SetPrivateMode { enabled: true }
                } else {
                    Method::SetConfig {
                        patch: copypaste_ipc::ConfigPatch {
                            private_mode: Some(true),
                            ..Default::default()
                        },
                    }
                };
                response_tx
                    .send(crate::server::dispatch::dispatch_store(&worker, 1, method))
                    .unwrap();
            });
            started_rx.recv().unwrap();
            assert!(response_rx.try_recv().is_err());
            assert!(state.settings.transition_is_in_progress());
            release_tx.send(()).unwrap();
            let (outcome, opens) = capture.join().unwrap();
            assert!(matches!(outcome, CaptureOutcome::Stored));
            assert_eq!(opens, 1);
            assert!(events.try_recv().unwrap().captured);
            assert!(response_rx.recv().unwrap().ok);
            setter.join().unwrap();
            assert_eq!(state.store.count().unwrap(), 1);
        }
    }

    #[test]
    fn file_pending_private_mode_aba_cancels_without_announcement() {
        for narrow_route in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("file");
            std::fs::write(&path, b"frozen").unwrap();
            let (state, _store_dir) = crate::testutil::test_state_with_clipboard(
                "fake-file-aba",
                Box::new(OnceCapture {
                    inner: Some(deferred_file(&path)),
                }),
            );
            let guard = TestPersistGuard::set(TestPersistMode::Busy);
            let mut slot = None;
            let mut events = state.subscribe();
            assert!(matches!(tick(&state, &mut slot), CaptureOutcome::Retried));
            std::fs::remove_file(&path).unwrap();
            for enabled in [true, false] {
                let method = if narrow_route {
                    Method::SetPrivateMode { enabled }
                } else {
                    Method::SetConfig {
                        patch: copypaste_ipc::ConfigPatch {
                            private_mode: Some(enabled),
                            ..Default::default()
                        },
                    }
                };
                assert!(crate::server::dispatch::dispatch_store(&state, 1, method).ok);
            }
            *TEST_PERSIST_MODE.lock().unwrap_or_else(|e| e.into_inner()) = None;
            assert!(matches!(
                tick(&state, &mut slot),
                CaptureOutcome::PolicyCancelled
            ));
            assert!(slot.is_none());
            assert_eq!(state.store.count().unwrap(), 0);
            while let Ok(event) = events.try_recv() {
                assert!(!event.captured);
            }
            drop(guard);
        }
    }
    #[test]
    fn denied_deferred_file_never_materializes_and_input_failure_is_terminal() {
        let _serial = TEST_PERSIST_SERIAL
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let dir = tempfile::tempdir().unwrap();
        let absent = dir.path().join("absent");
        for deny in [true, false] {
            let (state, _store_dir) = crate::testutil::test_state_with_clipboard(
                "fake-file-rejected",
                Box::new(OnceCapture {
                    inner: Some(deferred_file(&absent)),
                }),
            );
            if deny {
                assert!(
                    crate::server::dispatch::dispatch_store(
                        &state,
                        1,
                        Method::SetPrivateMode { enabled: true }
                    )
                    .ok
                );
            }
            crate::clipboard::file_capture::reset_test_open_count();
            let mut slot = None;
            let outcome = tick(&state, &mut slot);
            assert!(if deny {
                matches!(outcome, CaptureOutcome::PolicyCancelled)
            } else {
                matches!(outcome, CaptureOutcome::InputRejected)
            });
            assert_eq!(
                crate::clipboard::file_capture::test_open_count(),
                usize::from(!deny)
            );
            assert!(slot.is_none());
            assert_eq!(state.store.count().unwrap(), 0);
            assert!(matches!(tick(&state, &mut slot), CaptureOutcome::NoCapture));
        }
    }

    #[test]
    fn both_config_routes_revoke_pending_after_private_mode_round_trip() {
        for narrow_route in [false, true] {
            let polls = Arc::new(AtomicUsize::new(0));
            let (state, _dir) = crate::testutil::test_state_with_clipboard(
                "epoch-round-trip",
                Box::new(QueuedCapture {
                    values: VecDeque::from([captured("frozen", None)]),
                    polls: polls.clone(),
                }),
            );
            let guard = TestPersistGuard::set(TestPersistMode::Busy);
            let mut slot = None;
            assert!(matches!(tick(&state, &mut slot), CaptureOutcome::Retried));
            for enabled in [true, false] {
                let method = if narrow_route {
                    Method::SetPrivateMode { enabled }
                } else {
                    Method::SetConfig {
                        patch: copypaste_ipc::ConfigPatch {
                            private_mode: Some(enabled),
                            ..Default::default()
                        },
                    }
                };
                assert!(crate::server::dispatch::dispatch_store(&state, 1, method).ok);
            }
            *TEST_PERSIST_MODE.lock().unwrap_or_else(|e| e.into_inner()) = None;
            assert!(matches!(
                tick(&state, &mut slot),
                CaptureOutcome::PolicyCancelled
            ));
            assert!(slot.is_none());
            assert_eq!(state.store.count().unwrap(), 0);
            assert_eq!(polls.load(Ordering::SeqCst), 1);
            drop(guard);
        }
    }

    #[test]
    fn exclusion_round_trip_revokes_pending_even_when_current_policy_allows() {
        let (state, _dir) = crate::testutil::test_state_with_clipboard(
            "exclusion-round-trip",
            Box::new(QueuedCapture {
                values: VecDeque::from([captured("frozen", None)]),
                polls: Arc::new(AtomicUsize::new(0)),
            }),
        );
        let guard = TestPersistGuard::set(TestPersistMode::Busy);
        let mut slot = None;
        assert!(matches!(tick(&state, &mut slot), CaptureOutcome::Retried));
        for excluded in [vec!["Safari".into()], vec![]] {
            assert!(
                crate::server::dispatch::dispatch_store(
                    &state,
                    1,
                    Method::SetConfig {
                        patch: copypaste_ipc::ConfigPatch {
                            excluded_app_bundle_ids: Some(excluded),
                            ..Default::default()
                        }
                    }
                )
                .ok
            );
        }
        *TEST_PERSIST_MODE.lock().unwrap_or_else(|e| e.into_inner()) = None;
        assert!(matches!(
            tick(&state, &mut slot),
            CaptureOutcome::PolicyCancelled
        ));
        assert_eq!(state.store.count().unwrap(), 0);
        drop(guard);
    }

    struct BarrierCapture {
        entered: std::sync::mpsc::Sender<()>,
        release: std::sync::mpsc::Receiver<()>,
    }
    impl crate::clipboard::ClipboardSource for BarrierCapture {
        fn poll(&mut self) -> Option<crate::clipboard::Capture> {
            self.entered.send(()).unwrap();
            self.release.recv().unwrap();
            Some(captured("ordered", None))
        }
        fn poll_with_policy(
            &mut self,
            _: crate::clipboard::CapturePolicy<'_>,
        ) -> Option<crate::clipboard::Capture> {
            self.poll()
        }
        fn set_contents(&mut self, _: &str) -> anyhow::Result<()> {
            Ok(())
        }
        fn backend_name(&self) -> &'static str {
            "fake-barrier"
        }
    }
    #[test]
    fn capture_announcement_finishes_before_later_settings_response_on_both_routes() {
        let _serial = TEST_PERSIST_SERIAL
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        for narrow_route in [false, true] {
            let (entered_tx, entered_rx) = std::sync::mpsc::channel();
            let (release_tx, release_rx) = std::sync::mpsc::channel();
            let (state, _dir) = crate::testutil::test_state_with_clipboard(
                "capture-authority",
                Box::new(BarrierCapture {
                    entered: entered_tx,
                    release: release_rx,
                }),
            );
            let mut events = state.subscribe();
            let worker_state = state.clone();
            let capture = std::thread::spawn(move || tick(&worker_state, &mut None));
            entered_rx.recv().unwrap();
            let (started_tx, started_rx) = std::sync::mpsc::channel();
            let (response_tx, response_rx) = std::sync::mpsc::channel();
            let settings_state = state.clone();
            let setter = std::thread::spawn(move || {
                started_tx.send(()).unwrap();
                let method = if narrow_route {
                    Method::SetPrivateMode { enabled: true }
                } else {
                    Method::SetConfig {
                        patch: copypaste_ipc::ConfigPatch {
                            private_mode: Some(true),
                            ..Default::default()
                        },
                    }
                };
                let response = crate::server::dispatch::dispatch_store(&settings_state, 1, method);
                response_tx.send(response).unwrap();
            });
            started_rx.recv().unwrap();
            assert!(response_rx.try_recv().is_err());
            assert!(state.settings.transition_is_in_progress());
            release_tx.send(()).unwrap();
            assert!(matches!(capture.join().unwrap(), CaptureOutcome::Stored));
            assert!(response_rx.recv().unwrap().ok);
            setter.join().unwrap();
            assert_eq!(crate::testutil::contents(&state), ["ordered"]);
            assert!(events.try_recv().unwrap().captured);
        }
    }
}
