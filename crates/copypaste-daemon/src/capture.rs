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
        Ok(_) => state.note_local_change(),
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

/// [`copypaste_core::ingest_into`], plus the one thing the core cannot do:
/// record which device captured the item.
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
    let outcome = ingest_into(
        &state.store,
        &state.detector,
        &state.keyring,
        content,
        content_type,
        created_at,
        &settings,
    )?;

    // This device captured it, so this device is its origin — the one thing a
    // sync session needs about an item that the store has no column for. Read
    // as advisory on the way out (`meta::local_version` treats an absent row as
    // "captured here"), so a failure here costs nothing but is still worth
    // reporting. It stays in the daemon because the origin table is the
    // daemon's, not the store's.
    if let Ingested::Stored(item) = &outcome {
        if let Err(e) = state.meta.record_origin(&item.id, state.meta.device_id()) {
            warn!(error = ?e, "could not record the origin of a captured item");
        }
    }
    Ok(outcome)
}
