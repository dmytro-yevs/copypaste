//! The capture loop, and the daemon's wrapper around the shared ingest path.
//!
//! The pipeline itself is [`copypaste_core::ingest_into`], re-exported here so
//! this crate's callers name it where they always did. It lives in the core
//! because Android links the core in-process and cannot depend on this crate,
//! which is a binary with no `lib` target. A second ingest path could bypass
//! dedup, secret detection, or retention.
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
use std::time::{Duration, Instant};

use tokio::sync::watch;
use tracing::{debug, error, info, warn};

pub use copypaste_core::{IngestError, Ingested};

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

    let mut last_sweep = Instant::now();
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
                // The sweep used to ride every poll. It cannot any more — an
                // unchanged clipboard no longer reaches `tick` — so it gets a
                // cadence of its own, and none at all while it is switched off.
                let sweep_due = state.settings.get().sensitive_ttl_secs > 0
                    && last_sweep.elapsed() >= SWEEP_INTERVAL;

                // Bound to a local so the clipboard guard is released before
                // the handoff below rather than held across it.
                let changed = pending.lock().unwrap_or_else(|e| e.into_inner()).is_some() || state.clipboard().changed();
                if !changed && !sweep_due {
                    continue;
                }
                if sweep_due {
                    last_sweep = Instant::now();
                }

                let state = Arc::clone(&state);
                // The pasteboard read, the AEAD seal and the SQLite write are
                // all blocking. Running them on a worker keeps the reactor free
                // for the IPC server — and reaching it costs six thread wakeups,
                // which is why an idle clipboard stops short of here.
                let pending = Arc::clone(&pending);
                match tokio::task::spawn_blocking(move || tick_slot(&state, sweep_due, &pending)).await {
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

/// Delete detected secrets whose TTL has elapsed.
///
/// Best-effort, and deliberately never fatal to a tick: the sweep deletes user
/// data, so a failure must leave the data alone and retry, not stop capture
/// (AGENTS.md rule 4). `0` disables it. The shipped default is 30 seconds.
fn sweep_sensitive_items(state: &AppState) {
    let (mutation_started, removed) = state.store.with_retention(|| {
        let ttl = Duration::from_secs(state.settings.get().sensitive_ttl_secs);
        // Capture before wipe stamps so the upload floor cannot land above the
        // tombstone `created_at` (decrypt/judge can cross a millisecond).
        let mutation_started = copypaste_core::now_ms();
        let removed = copypaste_core::sensitive::sweep_sensitive(
            &state.store,
            &state.detector,
            &state.keyring.item_key(),
            ttl,
            mutation_started,
        );
        (mutation_started, removed)
    });
    match removed {
        Ok(0) => {}
        // `note_sensitive_swept` rather than `note_local_change`: this is the
        // one history change nobody asked for, and a client cannot say so on an
        // event that only reports that the count moved.
        Ok(removed) => {
            crate::cloud::note_version_written(state, mutation_started);
            state.note_sensitive_swept(u32::try_from(removed).unwrap_or(u32::MAX));
        }
        Err(e) => warn!(error = ?e, "the sensitive-item sweep failed"),
    }
}

/// How often the sensitive-item sweep runs while it is switched on. A TTL is
/// measured in seconds at least, so it does not need the poll interval.
const SWEEP_INTERVAL: Duration = Duration::from_secs(1);

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

fn tick_slot(
    state: &AppState,
    sweep_due: bool,
    slot: &Mutex<Option<PendingCapture>>,
) -> CaptureOutcome {
    let mut slot = slot.lock().unwrap_or_else(|e| e.into_inner());
    tick(state, sweep_due, &mut slot)
}
fn drain_pending(state: &AppState, slot: &Mutex<Option<PendingCapture>>) -> CaptureOutcome {
    let mut slot = slot.lock().unwrap_or_else(|e| e.into_inner());
    tick(state, false, &mut slot)
}
fn tick(state: &AppState, sweep_due: bool, slot: &mut Option<PendingCapture>) -> CaptureOutcome {
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
    if sweep_due {
        sweep_sensitive_items(state);
    }
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
    if crate::clipboard::format::preferred([capture.content_type.as_str()])
        != Some(copypaste_ipc::content_type::TEXT)
        || capture.binary_content.is_some()
        || capture.file_path.is_some()
        || capture.file_metadata.is_some()
    {
        return Err(IngestError::Empty);
    }
    let sensitive_floor = capture
        .app_bundle_id
        .as_deref()
        .is_some_and(crate::clipboard::is_password_manager_app);
    copypaste_core::ingest::ingest_into_with_capture_source_with_current_retention(
        &state.store,
        &state.detector,
        &state.keyring,
        &capture.content,
        &capture.content_type,
        created_at,
        sensitive_floor,
        capture.app_bundle_id.as_deref(),
        capture.app_name.as_deref(),
        settings,
        || state.settings.get().clone(),
    )
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
        &state.detector,
        &state.keyring,
        content,
        content_type,
        created_at,
        false,
        None,
        None,
        &settings,
        || state.settings.get().clone(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clipboard::windows_attribution::{Attribution, SourceApp};
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

    /// DMY-158, the whole path and not one predicate of it.
    ///
    /// Two clipboard changes a third of a second apart, the second written by a
    /// credential store. The attribution decision, the sensitivity floor, the
    /// search index and the set a sync session advertises live in four files;
    /// this is the test that runs them as one. Before the fix the second change
    /// inherited the first application's identity, so the floor never applied
    /// and the password was both searchable and syncable — no assertion on
    /// `is_password_manager_app` alone can see that.
    #[test]
    fn a_rapid_second_capture_from_a_credential_store_reaches_neither_search_nor_sync() {
        let (state, _dir) = test_state("credential-store-burst");
        let mut attribution = Attribution::default();

        // Resolved exactly as the Windows backend resolves them: once per
        // clipboard change, inside what used to be one 750 ms cache window.
        let ordinary =
            attribution.for_change(41, || SourceApp::from_image_path(r"C:\Windows\notepad.exe"));
        let credential = attribution.for_change(42, || {
            SourceApp::from_image_path(r"C:\Users\ann\AppData\Local\1Password\app\8\1Password.exe")
        });
        let now = copypaste_core::now_ms();
        let notes = ingest_capture(
            &state,
            &state.settings.get(),
            captured("thursday agenda notes", ordinary),
            now,
        )
        .expect("the ordinary capture is stored")
        .into_item();
        let secret = ingest_capture(
            &state,
            &state.settings.get(),
            captured("correct horse battery staple", credential),
            now + 1,
        )
        .expect("the credential capture is stored")
        .into_item();

        assert!(
            secret.is_sensitive,
            "a credential store's copy was stored as ordinary text"
        );

        // The index answers for the ordinary row, so an empty result for the
        // secret is the gate and not a broken query.
        let found: Vec<String> = state
            .store
            .search("thursday", 10)
            .expect("search")
            .into_iter()
            .map(|item| item.id)
            .collect();
        assert_eq!(found, vec![notes.id.clone()]);
        assert!(
            state
                .store
                .search("battery", 10)
                .expect("search")
                .is_empty(),
            "the password reached full-text search"
        );

        // What a sync session advertises: sensitive rows are not in it, and a
        // peer cannot ask for what was never offered.
        let advertised: Vec<String> = state
            .store
            .summaries_since(0, None, 100)
            .expect("summaries")
            .into_iter()
            .map(|version| version.id)
            .collect();
        assert!(advertised.contains(&notes.id));
        assert!(
            !advertised.contains(&secret.id),
            "the password was offered to a sync session"
        );
    }

    /// DMY-158 failure-before: when the attribution cache shares one identity
    /// across two changes (the TTL bug), a credential store's password passes
    /// the sensitivity floor, reaches full-text search and is offered to sync.
    #[test]
    fn shared_attribution_lets_a_credential_stores_copy_into_search_and_sync() {
        let (state, _dir) = test_state("shared-attribution-vuln");
        let mut attribution = Attribution::default();

        let ordinary =
            attribution.for_change(41, || SourceApp::from_image_path(r"C:\Windows\notepad.exe"));
        // Same sequence number: the cache answers from the first resolution,
        // so the credential store is attributed to Notepad.
        let misattributed = attribution.for_change(41, || unreachable!("the cache must answer"));
        assert_eq!(
            misattributed.as_ref().map(|a| a.id.as_str()),
            Some("notepad.exe"),
            "the cache must return the first identity"
        );

        let now = copypaste_core::now_ms();
        let notes = ingest_capture(
            &state,
            &state.settings.get(),
            captured("thursday agenda notes", ordinary),
            now,
        )
        .unwrap()
        .into_item();
        let wrong = ingest_capture(
            &state,
            &state.settings.get(),
            captured("correct horse battery staple", misattributed),
            now + 1,
        )
        .unwrap()
        .into_item();

        assert!(
            !wrong.is_sensitive,
            "misattributed to Notepad, the secret was not flagged"
        );
        assert!(
            !state.store.search("battery", 10).unwrap().is_empty(),
            "the password reached full-text search under the wrong identity"
        );
        let advertised: Vec<String> = state
            .store
            .summaries_since(0, None, 100)
            .expect("summaries")
            .into_iter()
            .map(|version| version.id)
            .collect();
        assert!(
            advertised.contains(&wrong.id),
            "the password must leak to sync under shared attribution"
        );
        assert!(
            advertised.contains(&notes.id),
            "the ordinary item must still be advertised"
        );
    }

    /// DMY-158: the two writers resolve within 750 ms, which is the window
    /// the old TTL cache would have collapsed. Measured to confirm the fix
    /// does not regress even with per-change resolution.
    #[test]
    fn two_writer_attribution_is_measured_within_750ms() {
        let (state, _dir) = test_state("timed-two-writer");
        let mut attribution = Attribution::default();

        let started = std::time::Instant::now();
        let ordinary =
            attribution.for_change(51, || SourceApp::from_image_path(r"C:\Windows\notepad.exe"));
        let credential = attribution.for_change(52, || {
            SourceApp::from_image_path(r"C:\Users\ann\AppData\Local\1Password\app\8\1Password.exe")
        });
        let elapsed = started.elapsed();

        assert!(
            elapsed.as_millis() < 750,
            "two resolutions took {}ms; must fit in the old 750ms window",
            elapsed.as_millis()
        );
        let ordinary = ordinary.expect("first writer");
        let credential = credential.expect("second writer");
        assert_eq!(ordinary.id, "notepad.exe");
        assert_eq!(credential.id, "1password.exe");
        assert_ne!(ordinary.id, credential.id, "distinct writers");

        let now = copypaste_core::now_ms();
        let notes = ingest_capture(
            &state,
            &state.settings.get(),
            captured("meeting agenda", Some(ordinary)),
            now,
        )
        .expect("ordinary")
        .into_item();
        let secret = ingest_capture(
            &state,
            &state.settings.get(),
            captured("correct horse battery staple", Some(credential)),
            now + 1,
        )
        .expect("credential")
        .into_item();

        assert!(secret.is_sensitive);
        assert!(!notes.is_sensitive);
        assert!(state.store.search("battery", 10).unwrap().is_empty());
        let advertised: Vec<String> = state
            .store
            .summaries_since(0, None, 100)
            .unwrap()
            .into_iter()
            .map(|v| v.id)
            .collect();
        assert!(!advertised.contains(&secret.id));
        assert!(advertised.contains(&notes.id));
    }

    #[test]
    fn non_text_capture_values_are_not_ingested() {
        let (state, _dir) = test_state("text-capture-only");
        let result = ingest_capture(
            &state,
            &state.settings.get(),
            crate::clipboard::Capture {
                content: String::new(),
                binary_content: Some(vec![1, 2, 3]),
                file_path: None,
                file_metadata: None,
                content_type: copypaste_ipc::content_type::IMAGE_PNG.to_string(),
                app_bundle_id: None,
                app_name: None,
            },
            copypaste_core::now_ms(),
        );
        assert!(matches!(result, Err(IngestError::Empty)));
        assert_eq!(state.store.count().unwrap(), 0);
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

    /// `sensitive_ttl_secs` ships at 30 seconds; this test turns it on
    /// explicitly so the fixture does not depend on the default.
    #[test]
    fn a_sweep_reports_how_many_secrets_it_deleted() {
        let (state, _dir) = test_state("alpha");
        state
            .settings
            .apply(
                &state.meta,
                &copypaste_ipc::ConfigPatch {
                    sensitive_ttl_secs: Some(30),
                    ..Default::default()
                },
            )
            .unwrap();
        // Captured long enough ago to be past a 30-second deadline.
        let old = copypaste_core::now_ms() - 10 * 60 * 1000;
        ingest_at(&state, "AKIAIOSFODNN7EXAMPLE", "text", old).unwrap();

        let mut events = state.subscribe();
        sweep_sensitive_items(&state);

        let event = events.try_recv().expect("the sweep publishes an event");
        assert_eq!(event.swept, 1);
        assert_eq!(event.item_count, 0);
        assert!(!event.captured);
    }

    /// The sweep used to ride every poll. Now that an unchanged clipboard
    /// never reaches `tick`, the loop decides when it is due — and a `tick`
    /// that swept regardless would put the database back on every poll.
    #[test]
    fn only_a_due_tick_sweeps() {
        let (state, _dir) = test_state("alpha");
        state
            .settings
            .apply(
                &state.meta,
                &copypaste_ipc::ConfigPatch {
                    sensitive_ttl_secs: Some(30),
                    ..Default::default()
                },
            )
            .unwrap();
        let old = copypaste_core::now_ms() - 10 * 60 * 1000;
        ingest_at(&state, "AKIAIOSFODNN7EXAMPLE", "text", old).unwrap();

        let mut pending = None;
        assert!(matches!(
            tick(&state, false, &mut pending),
            CaptureOutcome::NoCapture
        ));
        assert_eq!(
            state.store.count().unwrap(),
            1,
            "a tick that was not due swept"
        );

        assert!(matches!(
            tick(&state, true, &mut pending),
            CaptureOutcome::NoCapture
        ));
        assert_eq!(state.store.count().unwrap(), 0, "a due tick did not sweep");
    }

    /// A sweep that removed nothing must stay silent, or a client would post a
    /// "deleted" notice on every poll tick.
    #[test]
    fn a_sweep_with_nothing_to_delete_publishes_nothing() {
        let (state, _dir) = test_state("alpha");
        ingest(&state, "an ordinary clipping", "text").unwrap();
        let mut events = state.subscribe();
        sweep_sensitive_items(&state);
        assert!(events.try_recv().is_err());
    }

    #[test]
    fn a_disabled_sensitive_ttl_keeps_an_expired_secret() {
        let (state, _dir) = test_state("ttl-off");
        state
            .settings
            .apply(
                &state.meta,
                &copypaste_ipc::ConfigPatch {
                    sensitive_ttl_secs: Some(0),
                    ..Default::default()
                },
            )
            .unwrap();
        let item = ingest_at(
            &state,
            "AKIAIOSFODNN7EXAMPLE",
            copypaste_ipc::content_type::TEXT,
            copypaste_core::now_ms().saturating_sub(120_000),
        )
        .unwrap()
        .into_item();

        sweep_sensitive_items(&state);
        assert!(state.store.get(&item.id).unwrap().is_some());
    }

    #[test]
    fn a_sensitive_sweep_pulls_the_cloud_upload_floor_back() {
        let (state, _dir) = test_state("alpha");
        state
            .settings
            .apply(
                &state.meta,
                &copypaste_ipc::ConfigPatch {
                    sensitive_ttl_secs: Some(30),
                    ..Default::default()
                },
            )
            .unwrap();
        let old = copypaste_core::now_ms() - 10 * 60 * 1000;
        ingest_at(&state, "AKIAIOSFODNN7EXAMPLE", "text", old).unwrap();
        let ahead = copypaste_core::now_ms().saturating_add(60_000);
        state
            .meta
            .set_state_ms(crate::cloud::KEY_UPLOAD_FLOOR, ahead)
            .unwrap();

        sweep_sensitive_items(&state);

        let floor = state.meta.state_ms(crate::cloud::KEY_UPLOAD_FLOOR).unwrap();
        assert!(floor < ahead, "the sweep left the upload floor ahead");
        assert!(
            state
                .store
                .versions_since(floor, 100)
                .unwrap()
                .iter()
                .any(|row| row.deleted),
            "the wipe tombstone was not offered"
        );
    }

    /// Every other history change reports `swept: 0`, so a client can branch on
    /// it without asking what kind of change it was.
    #[test]
    fn an_ordinary_change_carries_no_swept_count() {
        let (state, _dir) = test_state("alpha");
        let mut events = state.subscribe();
        state.note_local_change();
        assert_eq!(events.try_recv().expect("an event").swept, 0);
    }

    #[test]
    fn detected_sensitive_capture_never_reaches_fts_or_sync() {
        let (state, _dir) = test_state("sensitive-capture");
        let stored = ingest(
            &state,
            "AKIAIOSFODNN7EXAMPLE",
            copypaste_ipc::content_type::TEXT,
        )
        .unwrap()
        .into_item();
        assert!(stored.is_sensitive);
        assert!(state.store.search("generated", 10).unwrap().is_empty());
        assert!(
            state
                .store
                .versions_since(i64::MIN, 100)
                .unwrap()
                .is_empty(),
            "sensitive rows never enter sync"
        );
    }

    #[test]
    fn credential_store_origin_is_persisted_and_forces_sensitive() {
        let (state, _dir) = test_state("credential-store-origin");
        let stored = ingest_capture(
            &state,
            &state.settings.get(),
            crate::clipboard::Capture {
                content: "xK9mQ3nR7pT2vW5".to_string(),
                binary_content: None,
                file_path: None,
                file_metadata: None,
                content_type: copypaste_ipc::content_type::TEXT.to_string(),
                app_bundle_id: Some("COM.1Password.Desktop".to_string()),
                app_name: Some("1Password".to_string()),
            },
            copypaste_core::now_ms(),
        )
        .unwrap()
        .into_item();
        assert!(stored.is_sensitive);
        assert_eq!(
            stored.app_bundle_id.as_deref(),
            Some("COM.1Password.Desktop")
        );
        assert_eq!(stored.app_name.as_deref(), Some("1Password"));
        assert!(state
            .store
            .search("xK9mQ3nR7pT2vW5", 10)
            .unwrap()
            .is_empty());
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
        assert!(matches!(
            tick(&state, false, &mut pending),
            CaptureOutcome::Stored
        ));

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
        assert_eq!(event.swept, 0);
    }

    #[test]
    fn password_manager_recopy_promotes_an_existing_duplicate_to_sensitive() {
        let (state, _dir) = test_state("credential-store-duplicate");
        let content = "ordinary-looking clipboard value";
        let first = ingest(&state, content, copypaste_ipc::content_type::TEXT)
            .unwrap()
            .into_item();
        assert!(!first.is_sensitive);
        assert_eq!(state.store.search("ordinary-looking", 10).unwrap().len(), 1);

        let duplicate = ingest_capture(
            &state,
            &state.settings.get(),
            crate::clipboard::Capture {
                content: content.to_string(),
                binary_content: None,
                file_path: None,
                file_metadata: None,
                content_type: copypaste_ipc::content_type::TEXT.to_string(),
                app_bundle_id: Some("com.1password.desktop".to_string()),
                app_name: Some("1Password".to_string()),
            },
            copypaste_core::now_ms(),
        )
        .unwrap()
        .into_item();

        assert_eq!(duplicate.id, first.id);
        assert!(duplicate.is_sensitive);
        assert!(state
            .store
            .search("ordinary-looking", 10)
            .unwrap()
            .is_empty());
        assert!(state.store.versions_since(i64::MIN, 10).unwrap().is_empty());
    }
}
