//! The capture loop, and the daemon's wrapper around the shared ingest path.
//!
//! The pipeline itself is [`copypaste_core::ingest_into`], re-exported here so
//! this crate's callers name it where they always did. It lives in the core
//! because Android links the core in-process and cannot depend on this crate,
//! which is a binary with no `lib` target — and a second ingest is the defect
//! v1 shipped, where the IPC path forgot the dedup probe.
//!
//! Manifest 01's data-loss rules that this file is responsible for:
//!
//! * **I-36** — no failure inside the pipeline may kill the poll loop. Every
//!   tick result is logged and the loop continues.
//! * Nothing acknowledges a capture without having stored it: the tick awaits
//!   the ingest before it returns, and shutdown is observed between ticks, not
//!   inside one.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::watch;
use tracing::{debug, error, info, warn};

pub use copypaste_core::{ingest_into, IngestError, Ingested};

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
/// live setting: v1 had hot-reload for it and the parity audit records losing
/// that as a consequence of losing config altogether.
pub async fn run(state: Arc<AppState>, mut shutdown: watch::Receiver<bool>) {
    state.set_capture_running(true);
    info!(
        backend = state.backend_name(),
        interval_ms = state.settings.get().poll_interval_ms,
        "clipboard capture started"
    );

    loop {
        let wait = Duration::from_millis(state.settings.get().poll_interval_ms);
        tokio::select! {
            _ = shutdown.changed() => break,
            // `sleep` rather than a ticker: a late tick must not cause a burst
            // of catch-up ticks — the clipboard has no backlog to drain, only a
            // current value — and the wait is recomputed each time round.
            _ = tokio::time::sleep(wait) => {
                let state = Arc::clone(&state);
                // The pasteboard read, the AEAD seal and the SQLite write are
                // all blocking. Running them on a worker keeps the reactor free
                // for the IPC server.
                match tokio::task::spawn_blocking(move || tick(&state)).await {
                    Ok(Ok(())) => {}
                    // Manifest 01 I-36: a failed tick is logged, never fatal.
                    Ok(Err(e)) => warn!(error = ?e, "capture tick failed"),
                    Err(e) => error!(error = %e, "capture task did not complete"),
                }
            }
        }
    }

    state.set_capture_running(false);
    info!("clipboard capture stopped");
}

/// Delete detected secrets whose TTL has elapsed.
///
/// Best-effort, and deliberately never fatal to a tick: the sweep deletes user
/// data, so a failure must leave the data alone and retry, not stop capture
/// (CLAUDE.md rule 4). `0` disables it, and that is the default until a user
/// asks for it — `copypaste_ipc::ConfigData::sensitive_ttl_secs` records why,
/// and what would justify turning it back on out of the box.
fn sweep_sensitive_items(state: &AppState) {
    let ttl = Duration::from_secs(state.settings.get().sensitive_ttl_secs);
    let removed = copypaste_core::sensitive::sweep_sensitive(
        &state.store,
        &state.detector,
        &state.keyring.item_key(),
        ttl,
        copypaste_core::now_ms(),
    );
    match removed {
        Ok(0) => {}
        // `note_sensitive_swept` rather than `note_local_change`: this is the
        // one history change nobody asked for, and a client cannot say so on an
        // event that only reports that the count moved.
        Ok(removed) => state.note_sensitive_swept(u32::try_from(removed).unwrap_or(u32::MAX)),
        Err(e) => warn!(error = ?e, "the sensitive-item sweep failed"),
    }
}

/// One poll. Returns `Ok(())` when there was nothing to capture.
fn tick(state: &AppState) -> Result<(), IngestError> {
    // The guard is taken for the pasteboard read alone and dropped before the
    // ingest, so an in-flight `copy` waits on one accessor call, not on a
    // database write.
    let capture = state.clipboard().poll();
    let Some(capture) = capture else {
        sweep_sensitive_items(state);
        return Ok(());
    };

    // The auto-wipe sweep rides the poll loop rather than owning a timer:
    // `sweep_sensitive` short-circuits on a cheap "is there anything wipeable"
    // probe, and a TTL measured to the nearest poll interval is exactly as
    // precise as the capture that started it.
    sweep_sensitive_items(state);

    match ingest(state, &capture.content, &capture.content_type) {
        Ok(Ingested::Stored(item)) => {
            debug!(id = %item.id, content_type = %item.content_type, "captured clipboard item");
            // Wakes the watchers and pulls both sync loops to their floor, so a
            // copy here shows up over there in seconds rather than at whatever
            // interval the loops had drifted to. `note_capture` rather than
            // `note_local_change` because this is the one caller that knows the
            // change was a *copy*, which is what a client needs to decide
            // whether to notify (parity finding 18).
            state.note_capture();
            crate::notify::on_capture(state);
            Ok(())
        }
        Ok(Ingested::Duplicate(item)) => {
            debug!(id = %item.id, "capture deduplicated against a recent item");
            Ok(())
        }
        // An empty clipboard is not a failure, and there is nothing to store.
        Err(IngestError::Empty) => Ok(()),
        // Over the size cap the user set. Reported once, at debug, rather than
        // as a tick failure: it is a decision they made, not a fault.
        Err(IngestError::TooLarge) => {
            debug!("clipboard item is over the configured size limit; not captured");
            Ok(())
        }
        Err(e) => Err(e),
    }
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
    ingest_into(
        &state.store,
        &state.detector,
        &state.keyring,
        content,
        content_type,
        created_at,
        &settings,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::test_state;

    /// The auto-wipe is the only thing in the daemon that deletes an item the
    /// user never asked it to, and until the count rode the event there was no
    /// way to tell them it had happened — which is the whole reason
    /// `sensitive_ttl_secs` ships at `0`.
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

    /// Every other history change reports `swept: 0`, so a client can branch on
    /// it without asking what kind of change it was.
    #[test]
    fn an_ordinary_change_carries_no_swept_count() {
        let (state, _dir) = test_state("alpha");
        let mut events = state.subscribe();
        state.note_local_change();
        assert_eq!(events.try_recv().expect("an event").swept, 0);
    }
}
