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
use std::fs::File;
use std::io::Read;
use std::path::Path;
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
                            | CaptureOutcome::PolicyCancelled,
                        ) => {}
                        Ok(CaptureOutcome::Failed(error)) => {
                            break 'capture Err(anyhow::Error::new(error).context(
                                "the accepted clipboard capture could not be persisted during shutdown",
                            ));
                        }
                        Err(error) => {
                            break 'capture Err(anyhow::Error::new(error).context(
                                "the accepted clipboard capture panicked during shutdown",
                            ));
                        }
                    }
                    if pending.lock().unwrap_or_else(|e| e.into_inner()).is_some() {
                        let interval_ms = state.settings.get().poll_interval_ms;
                        tokio::time::sleep(Duration::from_millis(interval_ms)).await;
                    }
                }
                break Ok(());
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
                let pending = Arc::clone(&pending);
                match tokio::task::spawn_blocking(move || tick_slot(&state, &pending)).await {
                    Ok(
                        CaptureOutcome::NoCapture
                        | CaptureOutcome::Stored
                        | CaptureOutcome::PolicyCancelled,
                    ) => {}
                    Ok(CaptureOutcome::Retried) => warn!("capture tick will retry transient storage failure"),
                    Ok(CaptureOutcome::Failed(error)) => warn!(error = ?error, "capture tick failed"),
                    // Manifest 01 I-36: a failed tick is logged, never fatal.
                    Err(e) => error!(error = %e, "capture task did not complete"),
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
    Failed(IngestError),
}
struct PendingCapture {
    capture: crate::clipboard::Capture,
    created_at: i64,
    settings: copypaste_ipc::ConfigData,
}

fn tick_slot(state: &AppState, slot: &Mutex<Option<PendingCapture>>) -> CaptureOutcome {
    let mut slot = slot.lock().unwrap_or_else(|e| e.into_inner());
    tick(state, &mut slot)
}
fn drain_pending(state: &AppState, slot: &Mutex<Option<PendingCapture>>) -> CaptureOutcome {
    let mut slot = slot.lock().unwrap_or_else(|e| e.into_inner());
    tick(state, &mut slot)
}
fn tick(state: &AppState, slot: &mut Option<PendingCapture>) -> CaptureOutcome {
    // The guard is taken for the pasteboard read alone and dropped before the
    // ingest, so an in-flight `copy` waits on one accessor call, not on a
    // database write.
    if slot.is_some() {
        return persist_pending(state, state.settings.get().clone(), slot);
    }
    let settings = state.settings.get().clone();
    let capture = state
        .clipboard()
        .poll_with_policy(crate::clipboard::CapturePolicy::new(&settings));
    let Some(capture) = capture else {
        return CaptureOutcome::NoCapture;
    };
    *slot = Some(PendingCapture {
        capture,
        created_at: copypaste_core::now_ms(),
        settings: settings.clone(),
    });
    persist_pending(state, settings, slot)
}
fn persist_pending(
    state: &AppState,
    settings: copypaste_ipc::ConfigData,
    slot: &mut Option<PendingCapture>,
) -> CaptureOutcome {
    let pending = slot.as_ref().unwrap();
    if !crate::clipboard::CapturePolicy::new(&settings).allows_materialized(&pending.capture) {
        *slot = None;
        return CaptureOutcome::PolicyCancelled;
    }
    #[cfg(test)]
    if let Some(outcome) = test_persist_outcome() {
        return outcome;
    }
    match ingest_capture(
        state,
        &pending.settings,
        &pending.capture,
        pending.created_at,
    ) {
        Ok(Ingested::Stored(item)) => {
            debug!(id = %item.id, content_type = %item.content_type, "captured clipboard item");
            // Wakes the watchers and pulls both sync loops to their floor, so a
            // copy here shows up over there in seconds rather than at whatever
            // interval the loops had drifted to. `note_capture` rather than
            // `note_local_change` because this is the one caller that knows the
            // change was a *copy*, which is what a client needs to decide
            // whether to notify (parity finding 18).
            announce_capture(state, item.created_at, true);
            *slot = None;
            CaptureOutcome::Stored
        }
        Ok(Ingested::Duplicate(item)) => {
            debug!(id = %item.id, "capture deduplicated against a recent item");
            announce_capture(state, item.created_at, false);
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
    match *TEST_PERSIST_MODE.lock().unwrap_or_else(|e| e.into_inner()) {
        Some(TestPersistMode::Busy) => Some(CaptureOutcome::Retried),
        Some(TestPersistMode::Failed) => Some(CaptureOutcome::Failed(IngestError::Storage(
            copypaste_core::StoreError::InvalidKey,
        ))),
        Some(TestPersistMode::Panic) => panic!("test-only blocking ingest panic"),
        None => None,
    }
}

fn announce_capture(state: &AppState, created_at: i64, saved: bool) {
    state.note_capture(created_at);
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
        (copypaste_ipc::content_type::FILE, None, Some(path), Some(metadata))
            if capture.content.is_empty() && metadata.is_valid() =>
        {
            let bytes = read_file_capture(
                path,
                settings.capture_limit_bytes(copypaste_ipc::content_type::FILE),
            )?;
            copypaste_core::ingest_binary_into_with_capture_source_metadata(
                &state.store,
                &state.keyring,
                &bytes,
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

fn read_file_capture(path: &Path, cap: u64) -> Result<Vec<u8>, IngestError> {
    let file = File::open(path).map_err(|_| IngestError::Empty)?;
    let metadata = file.metadata().map_err(|_| IngestError::Empty)?;
    if !metadata.is_file() {
        return Err(IngestError::Empty);
    }
    let len = metadata.len();
    if len > cap {
        return Err(IngestError::TooLarge);
    }
    let capacity = usize::try_from(len).map_err(|_| IngestError::TooLarge)?;
    let mut bytes = Vec::with_capacity(capacity);
    file.take(cap.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| IngestError::Empty)?;
    if bytes.len() as u64 > cap {
        return Err(IngestError::TooLarge);
    }
    (!bytes.is_empty())
        .then_some(bytes)
        .ok_or(IngestError::Empty)
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
    }

    impl Drop for TestPersistGuard {
        fn drop(&mut self) {
            *TEST_PERSIST_MODE.lock().unwrap_or_else(|e| e.into_inner()) = None;
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
        let (polled_tx, polled_rx) = tokio::sync::oneshot::channel();
        let (state, _dir) = crate::testutil::test_state_with_clipboard(
            name,
            Box::new(QueuedCapture {
                values: VecDeque::from([capture, captured("B", None)]),
                polls: Arc::clone(&polls),
                polled: Some(polled_tx),
            }),
        );
        let mut events = state.subscribe();
        let guard = TestPersistGuard::set(TestPersistMode::Busy);
        let task = tokio::spawn(run(Arc::clone(&state), state.shutdown_rx()));
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_millis(state.settings.get().poll_interval_ms)).await;
        polled_rx.await.expect("capture source was polled");
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
        }
    }

    struct QueuedCapture {
        values: VecDeque<crate::clipboard::Capture>,
        polls: Arc<AtomicUsize>,
        polled: Option<tokio::sync::oneshot::Sender<()>>,
    }
    impl crate::clipboard::ClipboardSource for QueuedCapture {
        fn poll(&mut self) -> Option<crate::clipboard::Capture> {
            self.polls.fetch_add(1, Ordering::SeqCst);
            if let Some(polled) = self.polled.take() {
                let _ = polled.send(());
            }
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
            "queued"
        }
    }

    #[tokio::test(start_paused = true)]
    async fn shutdown_retries_busy_capture_past_the_soft_budget_without_polling_newer_value() {
        let polls = Arc::new(AtomicUsize::new(0));
        let (polled_tx, polled_rx) = tokio::sync::oneshot::channel();
        let (state, _dir) = crate::testutil::test_state_with_clipboard(
            "busy-drain",
            Box::new(QueuedCapture {
                values: VecDeque::from([captured("A", None), captured("B", None)]),
                polls: Arc::clone(&polls),
                polled: Some(polled_tx),
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
        let task = tokio::spawn(run(Arc::clone(&state), state.shutdown_rx()));
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_millis(state.settings.get().poll_interval_ms)).await;
        polled_rx.await.expect("capture source was polled");
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
        let (polled_tx, polled_rx) = tokio::sync::oneshot::channel();
        let (state, _dir) = crate::testutil::test_state_with_clipboard(
            "permanent-drain-failure",
            Box::new(QueuedCapture {
                values: VecDeque::from([captured("A", None)]),
                polls: Arc::clone(&polls),
                polled: Some(polled_tx),
            }),
        );
        let guard = TestPersistGuard::set(TestPersistMode::Busy);
        let task = tokio::spawn(run(Arc::clone(&state), state.shutdown_rx()));
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_millis(state.settings.get().poll_interval_ms)).await;
        polled_rx.await.expect("capture source was polled");
        state.request_shutdown();
        *TEST_PERSIST_MODE.lock().unwrap_or_else(|e| e.into_inner()) =
            Some(TestPersistMode::Failed);
        tokio::time::advance(Duration::from_millis(state.settings.get().poll_interval_ms)).await;

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
        let (polled_tx, polled_rx) = tokio::sync::oneshot::channel();
        let (state, _dir) = crate::testutil::test_state_with_clipboard(
            "panic-drain-failure",
            Box::new(QueuedCapture {
                values: VecDeque::from([captured("A", None)]),
                polls: Arc::new(AtomicUsize::new(0)),
                polled: Some(polled_tx),
            }),
        );
        let guard = TestPersistGuard::set(TestPersistMode::Busy);
        let task = tokio::spawn(run(Arc::clone(&state), state.shutdown_rx()));
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_millis(state.settings.get().poll_interval_ms)).await;
        polled_rx.await.expect("capture source was polled");
        state.request_shutdown();
        *TEST_PERSIST_MODE.lock().unwrap_or_else(|e| e.into_inner()) = Some(TestPersistMode::Panic);
        tokio::time::advance(Duration::from_millis(state.settings.get().poll_interval_ms)).await;

        let error = task
            .await
            .expect("capture task did not panic")
            .expect_err("shutdown must fail when its blocking drain panics");
        assert!(error.to_string().contains("panicked during shutdown"));
        drop(guard);
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

        let stored = ingest_capture(
            &state,
            &state.settings.get(),
            crate::clipboard::Capture {
                content: String::new(),
                binary_content: None,
                file_path: Some(path),
                file_metadata: Some(metadata.clone()),
                content_type: copypaste_ipc::content_type::FILE.to_string(),
                app_bundle_id: None,
                app_name: None,
            },
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

        let result = ingest_capture(
            &state,
            &settings,
            crate::clipboard::Capture {
                content: String::new(),
                binary_content: None,
                file_path: Some(path),
                file_metadata: Some(metadata),
                content_type: copypaste_ipc::content_type::FILE.to_string(),
                app_bundle_id: None,
                app_name: None,
            },
            copypaste_core::now_ms(),
        );
        assert!(matches!(result, Err(IngestError::TooLarge)));
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
        let first = ingest(&state, content, copypaste_ipc::content_type::TEXT)
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
}
