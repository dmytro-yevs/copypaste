//! The capture loop, and the single ingest path every new item goes through.
//!
//! [`ingest`] is shared by the clipboard poll loop and by the `add` IPC method
//! on purpose. v1 had two ingest paths that drifted: the IPC one forgot the
//! dedup probe, so `copypaste add` could insert a row the poll loop would have
//! collapsed. One function, two callers.
//!
//! Manifest 01's data-loss rules that this file is responsible for:
//!
//! * **I-33** — a dedup lookup failure falls through to the insert. Storing a
//!   duplicate is recoverable; dropping a capture is not.
//! * **I-36** — no failure inside the pipeline may kill the poll loop. Every
//!   tick result is logged and the loop continues.
//! * Nothing acknowledges a capture without having stored it: the tick awaits
//!   the ingest before it returns, and shutdown is observed between ticks, not
//!   inside one.

use std::sync::Arc;
use std::time::Duration;

use copypaste_core::{CryptoError, NewItem, StoreError, StoredItem};
use tokio::sync::watch;
use tracing::{debug, error, info, warn};

use crate::AppState;

// Manifest 01 §4's 500 ms perceived-instant default now lives in
// `copypaste_ipc::ConfigData::default`, because it is a setting rather than a
// constant. Below ~100 ms the poll loop's own cost becomes visible; above ~5 s
// bursts become the norm rather than the exception, which is what the bounds in
// `copypaste_ipc::config` encode.

/// What a successful ingest did.
#[derive(Debug)]
pub enum Ingested {
    /// A new row.
    Stored(StoredItem),
    /// The same content was already stored inside the dedup window; this is the
    /// existing row.
    Duplicate(StoredItem),
}

impl Ingested {
    pub fn into_item(self) -> StoredItem {
        match self {
            Ingested::Stored(item) | Ingested::Duplicate(item) => item,
        }
    }
}

/// Ingest failures.
///
/// The `Display` text is deliberately free of detail: these strings can end up
/// in an IPC error, and the underlying `StoreError` may carry a database path,
/// which discloses the local username (CLAUDE.md rule 4). The full error is
/// kept as the `source` and goes to the local log, never to a client.
#[derive(Debug, thiserror::Error)]
pub enum IngestError {
    #[error("clipboard content was empty")]
    Empty,
    #[error("the item is larger than the configured size limit")]
    TooLarge,
    #[error("the item could not be encrypted")]
    Crypto(#[from] CryptoError),
    #[error("the item could not be stored")]
    Storage(#[from] StoreError),
}

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
/// asks for it.
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
            // interval the loops had drifted to.
            state.note_local_change();
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

/// Detect, encrypt, deduplicate, store — the one path into the database.
///
/// A thin shim over [`ingest_into`]: everything except recording the item's
/// origin is expressed in `copypaste-core` types, so that function can move into
/// `copypaste-core` unchanged. See the module header for why that move matters.
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

/// The ingest path itself, in `copypaste-core` terms only.
///
/// **Written to be moved into `copypaste-core`.** Android links the core
/// in-process and cannot reach this crate, which is a binary with no `lib`
/// target — and re-typing it there is how v1 got two ingest paths that drifted
/// (see the module header). Nothing below mentions the daemon, so the move is a
/// cut-and-paste plus a re-export, taking [`Ingested`], [`IngestError`],
/// [`MAX_HISTORY_ITEMS`] and their tests with it.
#[allow(clippy::too_many_arguments)]
pub fn ingest_into(
    store: &copypaste_core::Store,
    detector: &copypaste_core::Detector,
    keyring: &copypaste_core::Keyring,
    content: &str,
    content_type: &str,
    created_at: i64,
    settings: &copypaste_ipc::ConfigData,
) -> Result<Ingested, IngestError> {
    if content.trim().is_empty() {
        return Err(IngestError::Empty);
    }
    // Refusing a capture is data loss, so the cap has to be a *user's* number
    // rather than a compiled-in one — and the refusal is reported, not silent.
    if content.len() as u64 > settings.max_item_bytes {
        return Err(IngestError::TooLarge);
    }

    let hash = content_hash(content.as_bytes());

    // Manifest 01 I-33: a probe failure must not abort the ingest. Falling
    // through costs at most a duplicate row; returning here costs the capture.
    // `find_recent_by_hash` takes an absolute epoch-ms cutoff, not a duration.
    // Passing the window width itself would compare every row's timestamp
    // against 60000 ms after 1970 and match the entire history, silently
    // collapsing all repeats of a value into the first one ever stored.
    let cutoff_ms = created_at - dedup_window_ms(settings);
    let recent = match store.find_recent_by_hash(&hash, cutoff_ms) {
        Ok(found) => found,
        Err(e) => {
            warn!(error = ?e, "dedup probe failed; storing the item anyway");
            None
        }
    };
    if let Some(row) = recent {
        return Ok(Ingested::Duplicate(row));
    }

    let is_sensitive = detector.is_sensitive(content);

    // The AEAD binds the item id as associated data (manifest 02: "AAD must
    // bind item identity"), and `decrypt` is handed `StoredItem::id` on the way
    // back out. The id therefore has to be chosen *before* the seal — it cannot
    // be assigned by the insert, or the AAD written and the AAD read differ and
    // every row fails authentication on every later read. That is why `NewItem`
    // carries `id` rather than the store minting one.
    let item_id = uuid::Uuid::new_v4().to_string();
    let key = keyring.item_key();
    let (nonce, ciphertext) = copypaste_core::encrypt(content.as_bytes(), &key, &item_id)?;

    let stored = store.insert(NewItem {
        id: item_id,
        content_ciphertext: ciphertext,
        nonce,
        content_type: content_type.to_string(),
        content_hash: hash,
        is_sensitive,
        // CLAUDE.md rule 4 / manifest 03 ADR-015: a sensitive item never
        // reaches the search index. This is the write-time layer of that rule;
        // `server::search` enforces it again at read time.
        search_text: if is_sensitive {
            None
        } else {
            Some(content.to_string())
        },
        created_at,
    })?;

    // Best-effort, and deliberately after the insert: the item is already
    // durable, and a failed sweep must never turn a stored capture into a lost
    // one.
    if let Err(e) = store.evict_over_cap(u64::from(settings.history_limit)) {
        warn!(error = ?e, "history cap eviction failed");
    }
    // Age-based retention, disabled by the `0` sentinel. Best-effort and after
    // the insert, for the same reason the cap sweep is.
    if settings.retention_days > 0 {
        let cutoff = created_at - i64::from(settings.retention_days) * 86_400_000;
        if let Err(e) = store.evict_older_than(cutoff) {
            warn!(error = ?e, "age-based retention failed");
        }
    }

    Ok(Ingested::Stored(stored))
}

/// Hex SHA-256 of the raw pre-encryption bytes.
///
/// Manifest 03 §3.7 (`CopyPaste-y4v1`): the **full** 64-character lowercase
/// digest. An earlier daemon helper truncated it to 16 bytes, which weakened
/// second-preimage resistance for nothing.
///
/// The digest comes from `copypaste-core` rather than a second `sha2` call
/// site here: the storage layer already hashes for its dedup index, and two
/// hash helpers is exactly how v1 ended up with two definitions of the same
/// content identity.
fn content_hash(bytes: &[u8]) -> String {
    copypaste_core::storage::compute_content_hash(bytes)
}

/// The dedup window, in milliseconds.
///
/// `copypaste_core::storage::DEDUP_WINDOW_MS` is the default this setting
/// starts at; the setting is what is in force. Two definitions of "the same
/// thing twice" is exactly the duplication rule 1 is about, so the constant is
/// referenced only in `ConfigData::default`.
fn dedup_window_ms(settings: &copypaste_ipc::ConfigData) -> i64 {
    i64::from(settings.dedup_window_secs) * 1_000
}
