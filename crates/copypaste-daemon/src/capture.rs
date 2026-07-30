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

use copypaste_core::storage::DEDUP_WINDOW_MS;
use copypaste_core::{CryptoError, NewItem, StoreError, StoredItem};
use tokio::sync::watch;
use tokio::time::{interval, MissedTickBehavior};
use tracing::{debug, error, info, warn};

use crate::AppState;

/// Manifest 01 §4: 500 ms is the perceived-instant default. Below ~100 ms the
/// poll loop's own cost becomes visible; above ~5 s bursts become the norm
/// rather than the exception.
pub const POLL_INTERVAL: Duration = Duration::from_millis(500);

/// How much local history the daemon keeps, in live items.
///
/// `Store::evict_over_cap` counts rows, not bytes — v1's only bound was a
/// 10 GiB byte quota, so this number is a new decision rather than a ported
/// one. 10 000 is the top of the range the UI offers as a render limit
/// (manifest 06: `historyDisplayLimit`, default 1 000), so the daemon holding
/// more than the UI shows stays true at every setting below "Unlimited".
///
/// Pinned items are exempt inside the store (manifest 03 I9), and so is the
/// newest unpinned item — copying something huge must never make it vanish.
pub const MAX_HISTORY_ITEMS: u64 = 10_000;

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
    #[error("the item could not be encrypted")]
    Crypto(#[from] CryptoError),
    #[error("the item could not be stored")]
    Storage(#[from] StoreError),
}

/// Poll the clipboard until shutdown.
pub async fn run(state: Arc<AppState>, mut shutdown: watch::Receiver<bool>) {
    let mut ticker = interval(POLL_INTERVAL);
    // A late tick must not cause a burst of catch-up ticks: the clipboard has
    // no backlog to drain, only a current value.
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

    state.set_capture_running(true);
    info!(
        backend = state.backend_name(),
        interval_ms = POLL_INTERVAL.as_millis() as u64,
        "clipboard capture started"
    );

    loop {
        tokio::select! {
            _ = shutdown.changed() => break,
            _ = ticker.tick() => {
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

/// One poll. Returns `Ok(())` when there was nothing to capture.
fn tick(state: &AppState) -> Result<(), IngestError> {
    // The guard is taken for the pasteboard read alone and dropped before the
    // ingest, so an in-flight `copy` waits on one accessor call, not on a
    // database write.
    let capture = state.clipboard().poll();
    let Some(capture) = capture else {
        return Ok(());
    };

    match ingest(state, &capture.content, &capture.content_type) {
        Ok(Ingested::Stored(item)) => {
            debug!(id = %item.id, content_type = %item.content_type, "captured clipboard item");
            Ok(())
        }
        Ok(Ingested::Duplicate(item)) => {
            debug!(id = %item.id, "capture deduplicated against a recent item");
            Ok(())
        }
        // An empty clipboard is not a failure, and there is nothing to store.
        Err(IngestError::Empty) => Ok(()),
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
    let outcome = ingest_into(
        &state.store,
        &state.detector,
        &state.keyring,
        content,
        content_type,
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
pub fn ingest_into(
    store: &copypaste_core::Store,
    detector: &copypaste_core::Detector,
    keyring: &copypaste_core::Keyring,
    content: &str,
    content_type: &str,
) -> Result<Ingested, IngestError> {
    if content.trim().is_empty() {
        return Err(IngestError::Empty);
    }

    let hash = content_hash(content.as_bytes());

    // Manifest 01 I-33: a probe failure must not abort the ingest. Falling
    // through costs at most a duplicate row; returning here costs the capture.
    // `find_recent_by_hash` takes an absolute epoch-ms cutoff, not a duration.
    // Passing the window width itself would compare every row's timestamp
    // against 60000 ms after 1970 and match the entire history, silently
    // collapsing all repeats of a value into the first one ever stored.
    let cutoff_ms = copypaste_core::now_ms() - DEDUP_WINDOW_MS;
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
        created_at: copypaste_core::now_ms(),
    })?;

    // Best-effort, and deliberately after the insert: the item is already
    // durable, and a failed sweep must never turn a stored capture into a lost
    // one.
    if let Err(e) = store.evict_over_cap(MAX_HISTORY_ITEMS) {
        warn!(error = ?e, "history cap eviction failed");
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
