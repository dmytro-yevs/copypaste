//! Sync orchestrator — wires `copypaste-sync` into the daemon.
//!
//! Responsibilities:
//!
//! 1. Subscribe to the daemon's local `new_item_tx` broadcast channel and
//!    convert each freshly-inserted [`ClipboardItem`] into a [`WireItem`],
//!    forwarding it on `outbound_tx` for the transport layer to deliver.
//! 2. Consume incoming [`WireItem`]s pushed by the transport layer via
//!    `incoming_rx` and merge them into the local SQLite database using the
//!    Last-Write-Wins rules defined in `copypaste-sync::merge`.
//!
//! ## Why channels instead of a Transport trait?
//!
//! The actual peer transports (mTLS-over-TCP from `copypaste-p2p`, the
//! Supabase relay from `cloud.rs`, or a future WebRTC channel) live in
//! sibling modules. We expose two `tokio::sync` channels — outbound and
//! inbound — so the orchestrator stays pure I/O-free merge logic and the
//! tests remain hermetic. The transport layer owns the network side and just
//! forwards bytes through these channels.

use std::path::PathBuf;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use tokio::sync::{broadcast, mpsc, Mutex};

/// Page size used by [`catchup_items`] when iterating local history to build
/// the catch-up set. Keeping pages small avoids materialising thousands of
/// structs at once and keeps peak heap usage proportional to this constant
/// rather than to the total item count.
const CATCHUP_PAGE_SIZE: usize = 500;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use copypaste_core::{
    build_item_aad_v2, decrypt_from_cloud, decrypt_item_by_version, derive_v2,
    encode_image_with_limit, encrypt_for_cloud, encrypt_item_with_aad, is_sensitive_for_autowipe,
    prune_to_cap, ClipboardItem, Database, MigrationState, SyncKey, AAD_SCHEMA_VERSION_V4,
    NONCE_SIZE,
};
use copypaste_sync::{
    merge::{local_to_wire_owned, resolve, wire_to_local, MergeOutcome},
    protocol::WireItem,
};

/// Context passed to [`merge_incoming_with_crypto`] to enable the
/// Universal Clipboard auto-apply feature: when a genuinely fresh remote
/// item wins the LWW merge, write its decrypted plaintext directly to
/// NSPasteboard so it is ready to paste immediately.
///
/// The `self_write_change_count` is the **same** `Arc<AtomicI64>` the
/// [`ClipboardMonitor`](crate::clipboard::ClipboardMonitor) checks on every
/// poll tick.  Writing to NSPasteboard increments the system changeCount;
/// we stamp the new changeCount into this atomic before the monitor's next
/// tick so the poller recognises the write as ours and skips re-capturing it
/// (loop prevention — identical to the mechanism used by the `copy_item` IPC
/// handler).
pub struct AutoApplyCtx {
    /// Shared self-write sentinel for the pasteboard poller.
    pub self_write_change_count: Arc<AtomicI64>,
    /// This device's local encryption key (v1 seed).  Needed to decrypt image
    /// chunks for NSPasteboard writes.
    pub local_key: Arc<zeroize::Zeroizing<[u8; 32]>>,
    /// Live daemon config.  The `auto_apply_synced_clip` flag is read here on
    /// every merge so toggling it via `set_config` takes effect immediately.
    pub core_config: Arc<std::sync::RwLock<copypaste_core::AppConfig>>,
}

/// Cross-device content-key context for the sync orchestrator (P2P Phase 3).
///
/// Items are stored at rest encrypted under this device's *per-device*
/// local-storage key, so the on-wire ciphertext is undecryptable by any other
/// device. To make a synced item readable on a paired peer we re-key it through
/// a **shared content sync key** established at pairing (derived deterministically
/// from the PAKE session key — both peers hold the identical key):
///
/// * **outbound** — decrypt the row's ciphertext with the local key, then
///   re-encrypt the plaintext under the **per-peer** sync key (K_AB for peer B,
///   K_AC for peer C, etc. — [`encrypt_for_cloud`], XChaCha20-Poly1305 + per-item-id
///   AAD). The wire item carries that blob with `content_nonce = None` (the cloud
///   blob is self-framed: it prefixes its own 24-byte nonce).
/// * **inbound** — decrypt the wire blob with the shared sync key, then
///   re-encrypt the plaintext under THIS device's local v2 key before storing,
///   and index the plaintext into FTS so search + previews work for synced rows.
///
/// When no shared key is available (P2P disabled, or a legacy peer record with
/// no `sync_key_b64`) the orchestrator falls back to the legacy behaviour:
/// outgoing items ship their raw at-rest ciphertext (undecryptable on the peer,
/// exactly as before Phase 3) and incoming items are stored verbatim.
///
/// ## Key model (CopyPaste-716 fix)
///
/// Keys are **per-peer pairwise**: K_AB (shared between A and B) differs from
/// K_AC (shared between A and C). The previous implementation cached only the
/// FIRST peer's key and used it for all fanout targets, so peer C received a
/// blob encrypted under K_AB — which it could not decrypt (silent sync failure).
///
/// The fix: `cached_peer_keys` is a `HashMap<fingerprint, [u8; 32]>` populated
/// from **all** paired peers in `peers.json`. `sync_key_for_peer(fp)` does a
/// O(1) map lookup; the outbound fanout path calls it once per peer and
/// re-encrypts independently.
///
/// ## Caching (H8 perf fix, preserved)
///
/// The key map is wrapped in `Arc<Mutex<…>>` so all `SyncCrypto` clones
/// (including the temporary copy inside `merge_incoming_with_crypto::spawn_blocking`)
/// share the same backing store. `reload_sync_key` refreshes the entire map
/// atomically — visible to every live clone immediately.
#[derive(Clone)]
pub struct SyncCrypto {
    /// This device's v1 local-storage key (the raw seed from `load_local_key`).
    v1_key: [u8; 32],
    /// This device's v2 local-storage key (`derive_v2(seed)`).
    /// Item 5: wrapped in `Zeroizing` so the key bytes are scrubbed on drop.
    v2_key: zeroize::Zeroizing<[u8; 32]>,
    /// Path to `peers.json`. Only read during construction and `reload_sync_key`
    /// — NOT on every crypto operation (H8 fix).
    peers_path: PathBuf,
    /// Per-peer sync key cache (CopyPaste-716 fix).
    ///
    /// Maps canonical peer fingerprint → 32-byte pairwise sync key bytes.
    /// Populated from ALL paired peers in `peers.json` (not just the first).
    /// Shared via `Arc` so every `SyncCrypto` clone observes the same map.
    /// Updated atomically by `reload_sync_key` after any pairing write.
    cached_peer_keys: std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, [u8; 32]>>>,
}

impl SyncCrypto {
    /// Build a crypto context from the device's local-storage seed and the
    /// `peers.json` path. Eagerly loads the shared sync key from `peers.json`
    /// so the hot-path `sync_key_for_peer()` never touches the filesystem.
    pub fn new(local_seed: [u8; 32], peers_path: PathBuf) -> Self {
        let cached = Self::load_keys_from_peers(&peers_path);
        Self {
            v1_key: local_seed,
            v2_key: derive_v2(&local_seed),
            cached_peer_keys: std::sync::Arc::new(std::sync::Mutex::new(cached)),
            peers_path,
        }
    }

    /// Read `peers.json` once and return a map of canonical fingerprint →
    /// 32-byte sync key for every paired peer that has a valid `sync_key_b64`.
    ///
    /// CopyPaste-716: previously this returned only the FIRST peer's key via
    /// `find_map`, causing all fanout targets beyond the first peer to receive
    /// a blob encrypted under the wrong key. Now returns ALL peers' keys.
    ///
    /// The map key is the **canonical** (colon-free lowercase hex) fingerprint —
    /// the same form used by the mTLS transport as `DeviceFingerprint` in
    /// `peer_sinks`. `peers.json` stores colon-hex (e.g. `"aa:bb:cc"`); we
    /// normalise via `canonical_fingerprint` so lookups by `DeviceFingerprint`
    /// always hit (CopyPaste-716 secondary fix).
    fn load_keys_from_peers(peers_path: &std::path::Path) -> std::collections::HashMap<String, [u8; 32]> {
        use base64::Engine as _;
        crate::peers::load_peers(peers_path)
            .into_iter()
            .filter_map(|dev| {
                let b64 = dev.sync_key_b64.as_deref()?;
                let bytes = base64::engine::general_purpose::STANDARD.decode(b64).ok()?;
                let key = <[u8; 32]>::try_from(bytes.as_slice()).ok()?;
                // Normalise to canonical (colon-free lowercase) so lookups by
                // DeviceFingerprint (the mTLS transport's canonical form) hit.
                let canonical = crate::ipc::canonical_fingerprint(&dev.fingerprint);
                Some((canonical, key))
            })
            .collect()
    }

    /// Return the sync key for a specific peer fingerprint.
    ///
    /// This is an O(1) map read (no file I/O — H8 preserved). Call
    /// `reload_sync_key` after any pairing write to refresh all peers' keys.
    ///
    /// The `fingerprint` parameter may be in either colon-hex (`aa:bb:cc`) or
    /// canonical colon-free lowercase form — this function normalises before
    /// lookup so both call sites (tests with colon-hex, production fanout with
    /// canonical DeviceFingerprint) work correctly.
    ///
    /// Returns `None` when the peer has no sync key (legacy peer record or
    /// no pairing yet).
    pub fn sync_key_for_peer(&self, fingerprint: &str) -> Option<SyncKey> {
        let canonical = crate::ipc::canonical_fingerprint(fingerprint);
        let guard = self
            .cached_peer_keys
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        guard.get(canonical.as_str()).copied().map(SyncKey::from_bytes)
    }

    /// Return ANY available shared content sync key (if any peer has one).
    ///
    /// **Outbound use only** — used by `rekey_outbound` / `rekey_blob_outbound`
    /// for the legacy single-peer fallback path.  For the inbound path use
    /// [`all_sync_keys`] (CopyPaste-kw2) to avoid the arbitrary-first-entry
    /// bias that breaks 3+ device topologies.
    ///
    /// This is an O(1) memory read — no file I/O (H8 fix).
    fn shared_sync_key(&self) -> Option<SyncKey> {
        let guard = self
            .cached_peer_keys
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        guard.values().next().copied().map(SyncKey::from_bytes)
    }

    /// Return ALL cached pairwise sync keys as a `Vec<SyncKey>`.
    ///
    /// Used on the **inbound** path (CopyPaste-kw2 fix): because the mTLS
    /// authenticated sender fingerprint is dropped before items reach the
    /// merge path, we cannot look up the exact pairwise key by fingerprint.
    /// Instead we try every registered peer key until AEAD decryption
    /// succeeds — the authentication tag guarantees only the correct key
    /// accepts the ciphertext, so this is both correct and safe.
    ///
    /// In the common 2-device case there is exactly one key and the cost is
    /// identical to the previous `values().next()` path. In a 3+-device
    /// topology each sender encrypts under the pairwise key shared with
    /// THIS device, so at most one entry in the vec will ever succeed.
    fn all_sync_keys(&self) -> Vec<SyncKey> {
        let guard = self
            .cached_peer_keys
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        guard.values().copied().map(SyncKey::from_bytes).collect()
    }

    /// Re-read `peers.json` and update the in-memory per-peer key map. Call
    /// this once after any write to `peers.json` (pairing completion, revoke)
    /// so the orchestrator picks up new/changed keys without a daemon restart.
    ///
    /// Because `cached_peer_keys` is an `Arc`, this update is visible to
    /// every `SyncCrypto` clone (including ones moved into `spawn_blocking`
    /// closures) immediately.
    pub fn reload_sync_key(&self) {
        let new_keys = Self::load_keys_from_peers(&self.peers_path);
        let mut guard = self
            .cached_peer_keys
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        *guard = new_keys;
    }

    /// Returns `true` if the in-memory per-peer key map contains at least one
    /// entry.
    ///
    /// Only available in test builds so production code cannot accidentally
    /// depend on the cache state as a signal (reload_sync_key is the contract).
    #[cfg(test)]
    pub fn has_cached_sync_key(&self) -> bool {
        !self
            .cached_peer_keys
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .is_empty()
    }
}

/// Run the sync orchestrator until both upstream channels close or `shutdown`
/// is cancelled.
///
/// * `db` — shared handle to the local SQLite store.
/// * `new_item_rx` — broadcast receiver from `daemon::run`; carries items
///   produced by the local clipboard monitor.
/// * `incoming_rx` — `mpsc` receiver fed by the transport layer with items
///   received from remote peers.
/// * `outbound_tx` — `mpsc` sender drained by the transport layer to push
///   locally-produced items to peers. A closed receiver is logged and
///   ignored — peers may simply not be connected.
/// * `device_id` — UUID stamped as `origin_device_id` on outgoing items.
/// * `storage_quota_bytes` — byte cap passed to `prune_to_cap` after each
///   successful P2P merge so the local DB stays bounded (mirrors the cloud path).
/// * `auto_apply` — when `Some`, enables the Universal Clipboard feature: a
///   genuinely fresh incoming item (newer than the current local clipboard) is
///   written to NSPasteboard immediately after merge, with the self-write guard
///   armed to prevent re-capture by the poller.
/// * `shutdown` — D2: token cancelled by the daemon on SIGINT/SIGTERM so the
///   orchestrator exits promptly instead of waiting for channels to drain.
///
/// Returns `Ok(())` once both channels close or `shutdown` fires.
// `run` takes: db, new_item_rx, incoming_rx, outbound_tx, device_id, crypto,
// storage_quota_bytes, auto_apply, and shutdown — each a distinct runtime
// dependency; no struct without pulling daemon internals into copypaste-sync.
#[allow(clippy::too_many_arguments)]
pub async fn run(
    db: Arc<Mutex<Database>>,
    mut new_item_rx: broadcast::Receiver<ClipboardItem>,
    mut incoming_rx: mpsc::Receiver<WireItem>,
    outbound_tx: mpsc::Sender<WireItem>,
    device_id: String,
    crypto: Option<SyncCrypto>,
    storage_quota_bytes: i64,
    auto_apply: Option<AutoApplyCtx>,
    shutdown: CancellationToken,
) -> anyhow::Result<()> {
    info!(%device_id, has_crypto = crypto.is_some(), "sync orchestrator started");

    let mut local_closed = false;
    let mut incoming_closed = false;

    while !(local_closed && incoming_closed) {
        tokio::select! {
            // D2: exit promptly on daemon-wide shutdown signal.
            _ = shutdown.cancelled() => {
                info!("sync orchestrator: shutdown signal received, stopping");
                break;
            }
            // Local clipboard → forward to transport for fan-out.
            local = new_item_rx.recv(), if !local_closed => {
                match local {
                    Ok(item) => {
                        // CopyPaste-ux2i: `item` is owned here and unused after the
                        // wire item is built, so move its content blobs instead of
                        // cloning them.
                        let wire = local_to_wire_owned(item, &device_id);
                        // CopyPaste-716: per-peer re-keying now happens in the
                        // transport's fanout_to_peers (p2p.rs) so each peer
                        // receives a blob encrypted under its own pairwise sync
                        // key. Sending the raw at-rest wire here is safe because
                        // the outbound_loop holds a SyncCrypto and re-encrypts
                        // once per peer at send time. When crypto is None (P2P
                        // disabled) the raw ciphertext is forwarded as before.
                        debug!(item_id = %wire.id, "sync_orch: forwarding local item to transport");
                        if let Err(e) = outbound_tx.send(wire).await {
                            // No transport listening — normal when P2P/cloud disabled.
                            debug!("sync_orch: outbound channel closed: {e}");
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        warn!("sync_orch: broadcast lagged by {n} items");
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        info!("sync_orch: local channel closed");
                        local_closed = true;
                    }
                }
            }
            // Incoming peer item → LWW merge into DB.
            incoming = incoming_rx.recv(), if !incoming_closed => {
                match incoming {
                    Some(wire) => {
                        if let Err(e) = merge_incoming_with_crypto(
                            &db,
                            vec![wire],
                            crypto.as_ref(),
                            storage_quota_bytes,
                            auto_apply.as_ref(),
                        ).await {
                            warn!("sync_orch: merge_incoming failed: {e}");
                        }
                    }
                    None => {
                        info!("sync_orch: incoming channel closed");
                        incoming_closed = true;
                    }
                }
            }
        }
    }

    info!("sync orchestrator stopped");
    Ok(())
}

/// Apply LWW conflict resolution and persist any items that should win.
///
/// For each incoming [`WireItem`]:
///
/// * If the local row is missing, insert the wire version (marked synced).
/// * If the local row exists, [`resolve`] picks the winner; on `TakeRemote`
///   we delete the stale local row and insert the wire version.
///
/// Returns the number of rows that were actually upserted (i.e. winners
/// that replaced or supplemented local state). The orchestrator itself
/// ignores the count — it is exposed for tests and telemetry.
///
/// Uses `AppConfig::default().storage_quota_bytes` for the byte cap. Prefer
/// [`merge_incoming_with_crypto`] when the live quota is available.
pub async fn merge_incoming(
    db: &Arc<Mutex<Database>>,
    items: Vec<WireItem>,
) -> anyhow::Result<usize> {
    let quota = copypaste_core::AppConfig::default().storage_quota_bytes as i64;
    merge_incoming_with_crypto(db, items, None, quota, None).await
}

/// Crypto-aware variant of [`merge_incoming`] (P2P Phase 3).
///
/// When `crypto` is `Some` and an incoming item is sync-key-wrapped
/// (`content_nonce == None`, see [`rekey_outbound`]), the wire blob is
/// decrypted with the shared sync key and re-encrypted under THIS device's
/// local v2 key before storage, and the plaintext is indexed into FTS so the
/// synced row is searchable / previewable. Items that are not sync-key-wrapped
/// (legacy peers, image chunk blobs) are stored verbatim, exactly as the
/// pre-Phase-3 path did.
///
/// **Fix HIGH-2:** the entire merge body (get_item_by_item_id, resolve,
/// replace_item_atomic, prune_to_cap) is wrapped in `tokio::task::spawn_blocking`
/// so the synchronous rusqlite calls and shared_sync_key disk I/O do not block
/// an async executor worker. The tokio Mutex is acquired INSIDE the blocking
/// closure using `blocking_lock()`, mirroring `handle_text`/`handle_image`.
///
/// **Fix HIGH-3:** after a successful merge `prune_to_cap` is called with
/// `storage_quota_bytes` so the P2P inbound path enforces the same local DB
/// size cap the cloud path already enforces.
///
/// **Universal Clipboard:** when `auto_apply` is `Some` and the winning item
/// is *strictly newer* than the current local latest (wall_time comparison),
/// the decrypted plaintext is written to NSPasteboard so it is immediately
/// ready to paste. The self-write changeCount sentinel is stamped before/after
/// the write so the poller skips re-capturing the write. Only text and image
/// are auto-applied; files are skipped (noted in the log).
pub async fn merge_incoming_with_crypto(
    db: &Arc<Mutex<Database>>,
    items: Vec<WireItem>,
    crypto: Option<&SyncCrypto>,
    storage_quota_bytes: i64,
    auto_apply: Option<&AutoApplyCtx>,
) -> anyhow::Result<usize> {
    if items.is_empty() {
        return Ok(0);
    }

    // Clone what the blocking closure needs so it can be moved in:
    // - `Arc<Mutex<Database>>` is cheap to clone (reference count bump).
    // - `SyncCrypto` is `Clone` (derives it).
    let db = db.clone();
    let crypto_owned: Option<SyncCrypto> = crypto.cloned();
    // Clone the auto-apply Arcs so the blocking closure can move them in.
    // Type alias avoids the `clippy::type_complexity` lint on this local binding.
    type AutoApplyTuple = (
        Arc<AtomicI64>,
        Arc<zeroize::Zeroizing<[u8; 32]>>,
        Arc<std::sync::RwLock<copypaste_core::AppConfig>>,
    );
    let auto_apply_owned: Option<AutoApplyTuple> = auto_apply.map(|ctx| {
        (
            ctx.self_write_change_count.clone(),
            ctx.local_key.clone(),
            ctx.core_config.clone(),
        )
    });

    let result = tokio::task::spawn_blocking(move || {
        // Acquire the std-compatible blocking lock INSIDE the blocking closure.
        // This keeps the tokio executor free while we hold the lock and run
        // synchronous rusqlite calls (HIGH fix #2).
        let db_guard = db.blocking_lock();

        let mut upserted = 0usize;
        // Candidate for auto-apply: (wall_time, plaintext_bytes, content_type).
        // We track the highest-wall_time winner across the batch so only the
        // single freshest item is applied to NSPasteboard even during a burst.
        let mut apply_candidate: Option<(i64, Vec<u8>, String)> = None;

        for mut wire in items {
            // P0 security/correctness: clamp any negative lamport_ts / wall_time
            // before processing. A hostile or buggy peer can send lamport_ts = -1
            // which, when cast to u64 for the Lamport clock, becomes u64::MAX and
            // wins every LWW comparison forever. Clamping to 0 at ingest makes
            // the item a low-priority candidate that local items will override.
            wire.clamp_timestamps();

            // B1 FIX: look up by the STABLE cross-device `item_id` (the CRDT
            // identity), NOT `wire.id` (the peer's per-row primary key which is a
            // fresh UUID on every device and therefore never matches the local row).
            // Using `wire.id` caused the lookup to always return None, so the code
            // treated every incoming item as new and tried to INSERT with the peer's
            // PK — hitting the `idx_clipboard_item_id` UNIQUE constraint when the
            // item already existed locally, silently dropping the update.
            // Mirrors the cloud path (`cloud.rs`: `get_item_by_item_id`).
            let existing = match copypaste_core::get_item_by_item_id(&db_guard, &wire.item_id) {
                Ok(row) => row,
                Err(e) => {
                    warn!(item_id = %wire.item_id, "sync_orch: get_item_by_item_id failed: {e}");
                    continue;
                }
            };
            // Capture the local primary key before moving `existing` into resolve.
            // On TakeRemote we patch `to_insert.id` so FTS / copy_item / pins that
            // are keyed on the local `id` keep pointing at the same row — mirroring
            // the cloud path's `preserved_pk` pattern.
            let local_pk: Option<String> = existing.as_ref().map(|r| r.id.clone());
            let exists = existing.is_some();
            let take_remote = match existing.as_ref() {
                Some(local) => matches!(resolve(local, &wire), MergeOutcome::TakeRemote),
                None => true,
            };

            if !take_remote {
                debug!(item_id = %wire.item_id, "sync_orch: LWW kept local");
                continue;
            }
            // Tombstone fast-path: when the winning wire item is a soft-delete
            // (deleted=true), apply it locally without going through the full
            // rekey + replace path.
            //   • Row exists locally  → soft-delete it (wipe content, set deleted=1).
            //   • Row does not exist  → insert a tombstone row (CopyPaste-bfiu)
            //     so a later out-of-order create loses LWW instead of
            //     resurrecting the item (delete-before-create race).
            if wire.deleted {
                if exists {
                    let local_id = local_pk
                        .as_deref()
                        // SAFETY: `exists` is true only when `local_pk` is Some —
                        // it is set from `existing.as_ref().map(|r| r.id.clone())`.
                        .unwrap_or("");
                    match copypaste_core::storage::items::soft_delete_item(
                        &db_guard,
                        local_id,
                        wire.lamport_ts,
                        wire.wall_time,
                    ) {
                        Ok(_) => {
                            debug!(item_id = %wire.item_id, "sync_orch: applied inbound tombstone");
                            upserted += 1;
                        }
                        Err(e) => {
                            warn!(item_id = %wire.item_id, "sync_orch: soft_delete_item failed: {e}");
                        }
                    }
                } else {
                    // CopyPaste-bfiu: persist a tombstone for the unknown item so
                    // a create that arrives after the delete (out-of-order over
                    // P2P) is LWW-rejected and the item stays deleted. Honors the
                    // soft_delete "an inbound delete cannot resurrect" contract.
                    match copypaste_core::insert_tombstone(
                        &db_guard,
                        &wire.item_id,
                        &wire.item_id,
                        wire.lamport_ts,
                        wire.wall_time,
                        &wire.origin_device_id,
                    ) {
                        Ok(_) => {
                            debug!(item_id = %wire.item_id, "sync_orch: inserted tombstone for unknown item (delete-before-create)");
                            upserted += 1;
                        }
                        Err(e) => {
                            warn!(item_id = %wire.item_id, "sync_orch: insert_tombstone failed: {e}");
                        }
                    }
                }
                continue;
            }

            // Capture wall_time before wire is consumed by rekey_inbound.
            let wire_wall_time = wire.wall_time;
            let wire_content_type = wire.content_type.clone();

            // P2P Phase 3: unwrap the shared-key payload into a row encrypted under
            // this device's own local key, recovering the plaintext for FTS. Returns
            // the row to insert plus the decrypted plaintext (when text) to index.
            let (mut to_insert, fts_plaintext) = match crypto_owned.as_ref() {
                Some(c) => match rekey_inbound(c, wire) {
                    Ok(pair) => pair,
                    Err(w) => {
                        // Guard: if the item looks sync-key-wrapped but we
                        // couldn't decrypt it (shared key missing or wrong),
                        // the wire item has no content_nonce (and for
                        // file/image also no blob_ref).  Storing it verbatim
                        // creates a "poison row" that consumers reject with
                        // "missing content_nonce" / "missing blob_ref metadata".
                        // Skip it — the peer will re-send on the next catch-up
                        // cycle once the key is available.
                        // (CopyPaste-jww / CopyPaste-5y4)
                        if is_poison_wire(&w) {
                            warn!(
                                item_id = %w.item_id,
                                content_type = %w.content_type,
                                "sync_orch: inbound item has no content_nonce/blob_ref \
                                 (sync-key-wrapped but undecryptable) — skipping to avoid \
                                 poison row (CopyPaste-jww/5y4)"
                            );
                            continue;
                        }
                        // Not sync-key-wrapped (or undecryptable): store verbatim.
                        (wire_to_local(*w), None)
                    }
                },
                None => (wire_to_local(wire), None),
            };

            // Preserve the local primary key on replace so FTS / copy_item / pins
            // (all keyed on `id`) keep pointing at the same row after the update.
            // `wire_to_local` copies `wire.id` (the peer's PK) into `to_insert.id`;
            // we overwrite it here with the local row's PK when one exists.
            if let Some(pk) = local_pk {
                to_insert.id = pk;
            }

            // `wire_to_local` now propagates `pinned` and `pin_order` directly from
            // the wire (see merge.rs), so pin/unpin/reorder broadcasts converge via
            // normal LWW TakeRemote.  We intentionally trust the wire's values here
            // instead of OR-merging with the local state: the IPC handlers bump
            // lamport_ts before broadcasting, so the wire wins LWW only when it is
            // causally later — which is exactly when its pin state should take effect.

            // CopyPaste-kcf fix: run SensitiveDetector on the decrypted plaintext
            // so inbound items get the same auto-wipe TTL as locally-captured ones.
            // Previously `wire_to_local` always set `is_sensitive = false`, meaning
            // a password or API key synced from another device bypassed TTL cleanup.
            // We reuse the same `is_sensitive_for_autowipe` the local capture path
            // uses (daemon.rs line ~1587) — no new heuristics.
            // Only runs when `fts_plaintext` is Some (i.e. rekey_inbound succeeded
            // and decrypted a text item); verbatim/image/file rows are left as-is
            // because we have no plaintext to inspect.
            if let Some(ref pt) = fts_plaintext {
                if let Ok(text) = std::str::from_utf8(pt) {
                    to_insert.is_sensitive = is_sensitive_for_autowipe(text);
                }
            }

            // M1: make the delete-then-insert (plus FTS) ATOMIC. The previous code
            // ran `delete_item` then a separate `insert_item`; if the insert failed
            // the row was lost. We wrap delete + insert + FTS in a single
            // transaction so a failed insert rolls back the delete and leaves the
            // old row (and its FTS entry) intact. Mirrors `insert_item_with_fts`'s
            // `unchecked_transaction` approach (we can't reuse it directly because
            // it does plain INSERT with dedup-on-conflict rather than replace).
            let fts_text = fts_plaintext
                .clone()
                .and_then(|pt| String::from_utf8(pt).ok());
            match replace_item_atomic(&db_guard, exists, &to_insert, fts_text.as_deref()) {
                Ok(()) => {
                    debug!(item_id = %to_insert.item_id, "sync_orch: upserted incoming item");
                    upserted += 1;
                    // Track the highest-wall_time winner for potential auto-apply.
                    // `fts_plaintext` holds the already-decrypted text bytes; for
                    // image/file we recover the plaintext separately in
                    // `apply_to_pasteboard_if_fresh`.  Only update the candidate
                    // when this item is strictly newer than the current best.
                    if auto_apply_owned.is_some() {
                        let plaintext_opt = match wire_content_type.as_str() {
                            "text" => fts_plaintext.clone(),
                            // Image plaintext must be recovered from the stored
                            // row's chunks; pass a sentinel so the caller knows to
                            // do the decode step.  We use an empty vec as the
                            // marker here — the actual decode happens in
                            // `apply_to_pasteboard_if_fresh`.
                            "image" => Some(Vec::new()),
                            // Files: skip (deferred — file-URL pasteboard write
                            // needs a temp-file round-trip; not safe to do in
                            // the blocking DB closure).
                            _ => None,
                        };
                        if let Some(pt) = plaintext_opt {
                            let better = apply_candidate
                                .as_ref()
                                .is_none_or(|(best_wt, _, _)| wire_wall_time > *best_wt);
                            if better {
                                apply_candidate = Some((wire_wall_time, pt, wire_content_type));
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!(item_id = %to_insert.item_id, "sync_orch: atomic replace failed: {e}")
                }
            }
        }

        // Fix HIGH-3: enforce storage cap after P2P merge, mirroring the cloud
        // path (cloud.rs prune_to_cap call after poll_once). Without this the
        // local DB grew unboundedly when items arrived via P2P.
        if upserted > 0 {
            match prune_to_cap(&db_guard, storage_quota_bytes) {
                Ok(0) => {}
                Ok(n) => debug!("sync_orch: prune_to_cap removed {n} rows after P2P merge"),
                Err(e) => warn!("sync_orch: prune_to_cap failed after P2P merge: {e}"),
            }
        }

        // Universal Clipboard auto-apply: write the single freshest winner to
        // NSPasteboard, but ONLY when it is strictly newer than the current local
        // latest wall_time (prevents historical backfill from overwriting the user's
        // current clipboard on reconnect).
        if let (
            Some((candidate_wt, plaintext, content_type)),
            Some((swcc, local_key, core_config)),
        ) = (apply_candidate, auto_apply_owned)
        {
            // Check feature flag — allows live toggle via set_config.
            let enabled = core_config
                .read()
                .map(|cfg| cfg.auto_apply_synced_clip)
                .unwrap_or(true); // safe default: on

            if enabled {
                // Query the current local latest wall_time to decide whether this
                // is a genuinely fresh remote copy or historical catch-up.
                let local_latest_wt: i64 = db_guard
                    .conn()
                    .query_row(
                        "SELECT COALESCE(MAX(wall_time), 0) FROM clipboard_items \
                         WHERE origin_device_id = ''  OR 1=1",
                        [],
                        |r| r.get(0),
                    )
                    .unwrap_or(0);
                // Use the actual max wall_time across ALL rows (local or synced)
                // to detect whether this candidate is the newest thing we know of.
                let global_max_wt: i64 = db_guard
                    .conn()
                    .query_row(
                        "SELECT COALESCE(MAX(wall_time), 0) FROM clipboard_items",
                        [],
                        |r| r.get(0),
                    )
                    .unwrap_or(0);
                let _ = local_latest_wt; // used via global_max_wt path below

                if candidate_wt >= global_max_wt {
                    // This item is the newest in the DB — apply it.
                    apply_to_pasteboard_if_fresh(
                        &db_guard,
                        &content_type,
                        plaintext,
                        &local_key,
                        &swcc,
                    );
                } else {
                    debug!(
                        candidate_wt,
                        global_max_wt,
                        "sync_orch: auto-apply skipped — not the newest item (historical backfill)"
                    );
                }
            }
        }

        upserted
    })
    .await
    .map_err(|e| anyhow::anyhow!("sync_orch: merge blocking task panicked: {e}"))?;

    Ok(result)
}

/// Atomically replace (or insert) a clipboard row and its FTS index for the
/// sync merge path (sync M1).
///
/// Runs DELETE (when `existed`) + INSERT + FTS rewrite inside one
/// `unchecked_transaction`, so a failed insert rolls the whole thing back and
/// the prior row survives intact. Unlike `insert_item` / `insert_item_with_fts`
/// in core (plain INSERT, dedup-on-conflict), this path is a true replace keyed
/// on the cross-device `item_id` (the CRDT identity), which is what LWW
/// `TakeRemote` requires. The caller preserves the existing local row's primary
/// key on `item.id`, so the DELETE-by-item_id + INSERT keeps the same `id` and
/// the FTS rewrite below (keyed on `item.id`) stays consistent.
///
/// `fts_text` is the already-decrypted plaintext to index; `None`/empty skips
/// FTS (e.g. verbatim or image rows). The stored `key_version` is taken from
/// `item.key_version` rather than hardcoded to ITEM_KEY_VERSION_CURRENT so that
/// a verbatim (non-rewrapped) incoming row with key_version=1 is stored as v1
/// and can be decrypted by the existing v1 path, instead of being stamped v2
/// (which would make it permanently undecryptable — auth-tag mismatch).
fn replace_item_atomic(
    db: &Database,
    existed: bool,
    item: &ClipboardItem,
    fts_text: Option<&str>,
) -> anyhow::Result<()> {
    use rusqlite::params;

    // Honour the same write gate the core `insert_item` enforces: while the v4
    // key-version sweep is running, reject writes so a key_version=2 row can't
    // corrupt the cursor-based resume.
    if matches!(db.migration_state()?, MigrationState::InProgress { .. }) {
        anyhow::bail!("sync_orch: refusing write while v4 migration is in progress");
    }

    let tx = db.conn().unchecked_transaction()?;
    if existed {
        // Delete the prior version by its cross-device `item_id` (the row's
        // local PK is preserved on `item.id`, so the subsequent INSERT reuses
        // the same `id`). Deleting by `item_id` also defends the UNIQUE
        // `idx_clipboard_item_id` index from a conflict on re-insert.
        tx.execute(
            "DELETE FROM clipboard_items WHERE item_id = ?1",
            params![item.item_id],
        )?;
    }
    tx.execute(
        "INSERT INTO clipboard_items
         (id, item_id, content_type, content, content_nonce, blob_ref,
          is_sensitive, is_synced, lamport_ts, wall_time, expires_at, app_bundle_id,
          content_hash, origin_device_id, key_version, pinned, pin_order, deleted)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18)",
        params![
            item.id,
            item.item_id,
            item.content_type,
            item.content,
            item.content_nonce,
            item.blob_ref,
            item.is_sensitive as i64,
            item.is_synced as i64,
            item.lamport_ts,
            item.wall_time,
            item.expires_at,
            item.app_bundle_id,
            item.content_hash,
            item.origin_device_id,
            // Use item.key_version (set by rekey_inbound=2 or wire_to_local=wire.key_version)
            // rather than the hardcoded ITEM_KEY_VERSION_CURRENT. A verbatim legacy
            // key_version=1 row would be stamped v2 here but its ciphertext is still
            // v1-encrypted → permanent auth-tag failure on every subsequent decrypt.
            item.key_version as i64,
            item.pinned as i64,
            // pin_order: the wire now carries pin_order directly via wire_to_local,
            // so this correctly reflects the sender's pinned ordering.
            item.pin_order,
            // deleted: wire_to_local propagates this from the WireItem; for
            // non-tombstone items this is always false (tombstones are handled
            // by the soft_delete_item fast-path above and never reach here).
            item.deleted as i64,
        ],
    )?;
    if let Some(text) = fts_text {
        if !text.is_empty() {
            tx.execute("DELETE FROM clipboard_fts WHERE id = ?1", params![item.id])?;
            tx.execute(
                "INSERT INTO clipboard_fts(id, content_text) VALUES (?1, ?2)",
                params![item.id, text],
            )?;
        }
    }
    tx.commit()?;
    Ok(())
}

/// Build the set of local items to push to a specific peer that has just
/// connected (P2P Phase 3 "sync on connect" / catch-up).
///
/// Fanout is fire-and-forget to *currently* connected sinks, so an item
/// captured/imported before the mTLS link came up would otherwise never reach
/// the peer (and the both-sides-dial race makes the exact connect instant
/// non-deterministic). When a connection is established we therefore replay the
/// full local history to it once: each row is converted to a wire item and
/// re-keyed under the **per-peer** sync key for `peer_fingerprint` so only
/// the target peer can decrypt it. LWW on the receiver makes the replay
/// idempotent (already-present items lose or no-op).
///
/// CopyPaste-716: the previous signature had no `peer_fingerprint` parameter
/// and used `shared_sync_key()` (the first peer's key), so on 3+ device
/// topologies peers B and C both received catch-up blobs encrypted under K_AB.
/// Peer C (holding K_AC) could never decrypt them — silent sync failure.
/// Now each catch-up call passes the connecting peer's fingerprint and uses
/// that peer's specific pairwise key.
///
/// Returns an empty vec when the peer has no sync key (nothing decryptable to
/// send) or the DB read fails — catch-up is best-effort.
pub fn catchup_items(
    db: &Database,
    device_id: &str,
    crypto: &SyncCrypto,
    peer_fingerprint: &str,
) -> Vec<WireItem> {
    // Pre-flight: only bother paginating if the connecting peer has a sync key.
    // H8 fix preserved: uses the in-memory cache — no peers.json disk read.
    if crypto.sync_key_for_peer(peer_fingerprint).is_none() {
        return Vec::new();
    }

    let mut out = Vec::new();
    let mut offset: usize = 0;
    loop {
        let page: Vec<ClipboardItem> = match copypaste_core::get_page(db, CATCHUP_PAGE_SIZE, offset)
        {
            Ok(rows) => rows,
            Err(e) => {
                warn!("sync_orch: catchup get_page (offset={offset}) failed: {e}");
                break;
            }
        };
        let page_len = page.len();
        // CopyPaste-ux2i: `page` is a locally-owned Vec consumed once; move each
        // item's content blob into the wire item instead of cloning it.
        for item in page {
            let mut wire = local_to_wire_owned(item, device_id);
            // Re-key under this peer's pairwise key (CopyPaste-716).
            // Only forward items we could actually re-key — a
            // still-locally-encrypted (NotApplicable) or failed payload is useless
            // — or worse, undecryptable — to the peer (sync H2).
            if rekey_outbound_for_peer(crypto, peer_fingerprint, &mut wire)
                == RekeyOutcome::Rewrapped
            {
                out.push(wire);
            }
        }
        if page_len < CATCHUP_PAGE_SIZE {
            break; // last page
        }
        offset += CATCHUP_PAGE_SIZE;
    }
    out
}

/// Outcome of an attempt to re-key an outgoing item under the shared sync key.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum RekeyOutcome {
    /// The payload was successfully re-wrapped under the shared sync key — the
    /// wire item is decryptable by the paired peer and safe to forward.
    Rewrapped,
    /// The item is not a re-key candidate (non-text, no content/nonce, or no
    /// shared key is available). The wire item is left unchanged and follows
    /// the legacy path — it may carry raw at-rest ciphertext (a no-crypto /
    /// legacy peer expects that) or be an image chunk handled elsewhere.
    NotApplicable,
    /// A shared key WAS available and the item WAS a candidate, but re-keying
    /// failed (wrong nonce length, local-decrypt, or shared-encrypt error). The
    /// wire item still carries raw at-rest ciphertext that the peer can never
    /// decrypt — the caller MUST drop it rather than forward a permanently
    /// undecryptable row (sync H2).
    Failed,
}

/// Maximum reassembled image/file plaintext we will re-key onto the wire.
///
/// The P2P transport frames at 16 MiB (`transport.rs`) and the cloud relay
/// caps the request body, so an oversized blob would either be rejected by the
/// transport or land undecryptable on the peer. We enforce the ceiling here so
/// the item is *dropped with a warning* rather than silently corrupting sync.
///
/// Ceiling layering (one blob, four caps — see `defaults.rs::MAX_FILE_SIZE_BYTES`):
///   * STORABLE = 100 MiB — `copypaste_core::MAX_FILE_BYTES`, library hard cap on
///     a locally-stored file item. `max_file_size_bytes` is clamped to this.
///   * SYNC     =   8 MiB — *this* const: the largest plaintext re-keyed onto the
///     wire. Items 8–100 MiB are kept LOCALLY but skipped for sync (warned).
///   * P2P frame =  16 MiB — transport framing cap.
///   * Relay body = 10 MiB — relay request-body cap.
///
/// So a file can be storable yet un-syncable: local storage and sync are
/// deliberately decoupled, and the UI tells the user where the sync line sits.
pub(crate) const SYNC_MAX_BLOB_BYTES: usize = 8 * 1024 * 1024;

/// Reassemble an image/file item's at-rest chunk blob back into plaintext.
///
/// The chunks were encrypted under this device's LOCAL v1 seed (`crypto.v1_key`)
/// with the 16-byte `file_id` as AEAD AAD — exactly as `daemon::handle_image`
/// (and the file pipeline) writes them. We parse `file_id` out of the
/// `blob_ref` meta JSON (shared parser for both image and file), deserialize the
/// chunks, and decode:
///   * image → [`copypaste_core::decode_image`] (PNG bytes)
///   * file  → [`copypaste_core::decode_file`] (verbatim bytes)
///
/// Returns the recovered plaintext, or `None` on any parse/decrypt failure
/// (logged). Callers map `None` to `RekeyOutcome::Failed` so a corrupt local
/// row is dropped, never forwarded.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn recover_blob_plaintext(crypto: &SyncCrypto, wire: &WireItem) -> Option<Vec<u8>> {
    let meta_json = wire.blob_ref.as_deref()?;
    let file_id = match crate::ipc::parse_image_file_id(meta_json) {
        Ok(id) => id,
        Err(e) => {
            warn!(item_id = %wire.item_id, "sync_orch: blob meta parse failed: {e}");
            return None;
        }
    };
    let content = wire.content.as_deref()?;
    let chunks = match copypaste_core::chunks_from_blob(content) {
        Ok(c) => c,
        Err(e) => {
            warn!(item_id = %wire.item_id, "sync_orch: chunks_from_blob failed: {e}");
            return None;
        }
    };
    // Dispatch on the wire item's key_version: v1 rows use the raw local
    // key (v1 seed); v2 rows use derive_v2(seed). After the writer fix
    // (handle_image / handle_file now use derive_v2), all freshly-captured
    // rows are kv=2. Legacy rows stamped kv=2 but encrypted with v1 (the
    // mislabeled rows) are repaired by repair_mislabeled_kv2_blob_rows at
    // startup, so by the time sync runs all kv=2 rows are truly v2.
    let blob_key: &[u8; 32] = if wire.key_version == 1 {
        &crypto.v1_key
    } else {
        &crypto.v2_key
    };
    let decoded = if wire.content_type == "image" {
        copypaste_core::decode_image(&chunks, blob_key, &file_id).map_err(|e| e.to_string())
    } else {
        copypaste_core::decode_file(&chunks, blob_key, &file_id).map_err(|e| e.to_string())
    };
    match decoded {
        Ok(pt) => Some(pt),
        Err(e) => {
            warn!(item_id = %wire.item_id, "sync_orch: blob decode failed: {e}");
            None
        }
    }
}

/// Re-key an image/file wire item onto the shared sync key.
///
/// Reassembles the at-rest blob to plaintext ([`recover_blob_plaintext`]),
/// enforces [`SYNC_MAX_BLOB_BYTES`], then replaces `content` with a single
/// shared-key-wrapped blob (`encrypt_for_cloud`, same call the text arm uses),
/// clears `content_nonce` (the unwrap marker) and `blob_ref`, and keeps
/// `content_type`. Mirrors the text arm's `Failed`/`NotApplicable` contract.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
/// Inner implementation for blob (image/file) outbound re-keying under an
/// explicit `SyncKey` (CopyPaste-716: key now passed in by caller rather than
/// fetched from the first-peer-only cache).
fn rekey_blob_outbound_with_key(
    crypto: &SyncCrypto,
    shared: &SyncKey,
    wire: &mut WireItem,
) -> RekeyOutcome {
    // A shared key IS present: from here any failure is `Failed` (drop), never
    // a silent forward of an undecryptable at-rest blob (sync H2).
    let Some(plaintext) = recover_blob_plaintext(crypto, wire) else {
        return RekeyOutcome::Failed;
    };
    if plaintext.len() > SYNC_MAX_BLOB_BYTES {
        warn!(
            item_id = %wire.item_id,
            size = plaintext.len(),
            max = SYNC_MAX_BLOB_BYTES,
            "sync_orch: blob exceeds sync ceiling, dropping (not forwarded)"
        );
        return RekeyOutcome::Failed;
    }
    match encrypt_for_cloud(shared, &wire.item_id, &plaintext) {
        Ok(blob) => {
            wire.content = Some(blob);
            // Self-framed blob → no item-level nonce; `None` is the receiver's
            // sync-key-wrapped marker.
            wire.content_nonce = None;
            // For file items: stash filename + mime into the dedicated wire
            // fields BEFORE clearing blob_ref, so the receiver can reconstruct
            // the local file meta JSON with the correct identity. blob_ref
            // itself must not travel (it is a local at-rest artefact; the
            // receiver rebuilds it from recovered plaintext + these fields).
            if wire.content_type == "file" {
                if let Some((fname, fmime)) =
                    wire.blob_ref.as_deref().and_then(parse_file_name_mime)
                {
                    wire.file_name = Some(fname);
                    wire.mime = Some(fmime);
                }
            }
            wire.blob_ref = None;
            RekeyOutcome::Rewrapped
        }
        Err(e) => {
            warn!(item_id = %wire.item_id, "sync_orch: blob shared-encrypt failed: {e}");
            RekeyOutcome::Failed
        }
    }
}

fn rekey_blob_outbound(crypto: &SyncCrypto, wire: &mut WireItem) -> RekeyOutcome {
    let Some(shared) = crypto.shared_sync_key() else {
        return RekeyOutcome::NotApplicable;
    };
    rekey_blob_outbound_with_key(crypto, &shared, wire)
}

/// Re-encrypt an outgoing item's payload under the shared content sync key so a
/// paired peer can decrypt it (P2P Phase 3).
///
/// Decrypts the row's at-rest ciphertext with this device's local key (by
/// `key_version`), then re-encrypts the plaintext under the shared sync key via
/// [`encrypt_for_cloud`] (XChaCha20-Poly1305, AAD bound to `item_id`). The
/// resulting self-framed blob (its own 24-byte nonce prefix + ciphertext+tag)
/// is placed in `wire.content` and `wire.content_nonce` is cleared to `None`,
/// which the receiver uses as the "sync-key-wrapped" marker.
///
/// Returns [`RekeyOutcome`]:
/// * [`RekeyOutcome::Rewrapped`] — payload re-wrapped, safe to forward.
/// * [`RekeyOutcome::NotApplicable`] — non-text, no content/nonce, or no shared
///   key: the wire item is left UNCHANGED and follows the legacy path.
/// * [`RekeyOutcome::Failed`] — a shared key was present but the crypto step
///   failed; the wire still carries raw at-rest ciphertext the peer cannot
///   decrypt, so the caller must DROP it (sync H2).
///
/// Image and file items are re-keyed by reassembling the at-rest chunk blob
/// into plaintext (decoded with the LOCAL v1 seed + `file_id` AAD), then
/// re-wrapping that whole plaintext under the shared sync key — identical wire
/// shape to text (`content_nonce = None`, `blob_ref = None`, `content_type`
/// preserved). See [`recover_blob_plaintext`] / [`rekey_blob_outbound`].
fn rekey_outbound(crypto: &SyncCrypto, wire: &mut WireItem) -> RekeyOutcome {
    if wire.content_type == "image" || wire.content_type == "file" {
        return rekey_blob_outbound(crypto, wire);
    }
    if wire.content_type != "text" {
        return RekeyOutcome::NotApplicable;
    }
    let Some(shared) = crypto.shared_sync_key() else {
        return RekeyOutcome::NotApplicable;
    };
    rekey_outbound_text_with_key(crypto, &shared, wire)
}

/// Inner text re-key under an explicit `SyncKey` (CopyPaste-716: per-peer key).
///
/// Decrypts the at-rest ciphertext under `crypto`'s local key, then
/// re-encrypts under `peer_key`. Caller is responsible for passing the correct
/// per-peer key (via [`SyncCrypto::sync_key_for_peer`]).
fn rekey_outbound_text_with_key(
    crypto: &SyncCrypto,
    peer_key: &SyncKey,
    wire: &mut WireItem,
) -> RekeyOutcome {
    let (Some(ciphertext), Some(nonce_vec)) = (wire.content.as_ref(), wire.content_nonce.as_ref())
    else {
        return RekeyOutcome::NotApplicable;
    };
    // From here on a shared key IS present and the item IS a re-key candidate,
    // so any failure must surface as `Failed` (drop), never silent forward.
    let mut nonce = [0u8; NONCE_SIZE];
    if nonce_vec.len() != NONCE_SIZE {
        warn!(item_id = %wire.item_id, "sync_orch: rekey_outbound wrong nonce length, dropping");
        return RekeyOutcome::Failed;
    }
    nonce.copy_from_slice(nonce_vec);

    let plaintext = match decrypt_item_by_version(
        wire.key_version,
        &crypto.v1_key,
        &crypto.v2_key,
        &wire.item_id,
        &nonce,
        ciphertext,
    ) {
        Ok(pt) => pt,
        Err(e) => {
            warn!(item_id = %wire.item_id, "sync_orch: rekey_outbound local-decrypt failed: {e}");
            return RekeyOutcome::Failed;
        }
    };

    match encrypt_for_cloud(peer_key, &wire.item_id, &plaintext) {
        Ok(blob) => {
            wire.content = Some(blob);
            // The cloud blob is self-framed (nonce prefix), so there is no
            // separate item-level nonce. `None` is the receiver's unwrap marker.
            wire.content_nonce = None;
            RekeyOutcome::Rewrapped
        }
        Err(e) => {
            warn!(item_id = %wire.item_id, "sync_orch: rekey_outbound shared-encrypt failed: {e}");
            RekeyOutcome::Failed
        }
    }
}

/// Re-encrypt an outgoing item under the pairwise sync key for `peer_fingerprint`.
///
/// CopyPaste-716: this is the correct per-peer fanout call. Unlike
/// [`rekey_outbound`] (which uses the first cached key for legacy/catchup
/// compatibility), this function looks up the sync key specific to
/// `peer_fingerprint` from the per-peer cache. The caller (fanout + catchup
/// paths) must clone the `WireItem` before calling so each peer gets its own
/// independently-encrypted copy.
///
/// Returns [`RekeyOutcome`]:
/// * [`RekeyOutcome::Rewrapped`] — payload re-wrapped under the peer's key.
/// * [`RekeyOutcome::NotApplicable`] — peer has no sync key, or item type is
///   not re-keyable (non-text/image/file). Wire item is left unchanged.
/// * [`RekeyOutcome::Failed`] — key present but crypto failed; caller must drop.
pub(crate) fn rekey_outbound_for_peer(
    crypto: &SyncCrypto,
    peer_fingerprint: &str,
    wire: &mut WireItem,
) -> RekeyOutcome {
    let Some(peer_key) = crypto.sync_key_for_peer(peer_fingerprint) else {
        return RekeyOutcome::NotApplicable;
    };
    if wire.content_type == "image" || wire.content_type == "file" {
        return rekey_blob_outbound_with_key(crypto, &peer_key, wire);
    }
    if wire.content_type != "text" {
        return RekeyOutcome::NotApplicable;
    }
    rekey_outbound_text_with_key(crypto, &peer_key, wire)
}

/// Inverse of [`rekey_outbound`]: turn a sync-key-wrapped incoming wire item
/// into a [`ClipboardItem`] encrypted under THIS device's local v2 key, plus
/// the recovered plaintext (for FTS indexing).
///
/// Returns `Err(wire)` (handing the item back unchanged) when the item is not
/// sync-key-wrapped or cannot be decrypted, so the caller can fall back to
/// storing it verbatim.
// `WireItem` is ~232 bytes, so a bare `Result<_, WireItem>` trips
// clippy::result_large_err. We box the rarely-taken error payload (the
// hand-back-unchanged path) to keep the common Ok variant small.
#[allow(clippy::result_large_err)]
fn rekey_inbound(
    crypto: &SyncCrypto,
    wire: WireItem,
) -> Result<(ClipboardItem, Option<Vec<u8>>), Box<WireItem>> {
    // Marker: a sync-key-wrapped payload carries content but no nonce.
    let is_blob = wire.content_type == "image" || wire.content_type == "file";
    if (wire.content_type != "text" && !is_blob)
        || wire.content_nonce.is_some()
        || wire.content.is_none()
    {
        return Err(Box::new(wire));
    }

    // CopyPaste-kw2 fix: try ALL registered peer keys instead of the arbitrary
    // first entry in the HashMap.  In a 3+-device topology the authenticated
    // mTLS sender fingerprint is dropped before items reach the merge path, so
    // we cannot look up the pairwise key by fingerprint here.  AEAD guarantees
    // that only the correct key (K_sender_this_device) produces a valid tag —
    // trying every key until one succeeds is correct, safe, and O(n) in the
    // number of paired peers (typically 1-3).
    let peer_keys = crypto.all_sync_keys();
    if peer_keys.is_empty() {
        return Err(Box::new(wire));
    }

    if is_blob {
        // For blobs try each key; pass ownership of wire only to the first
        // attempt, hand it back on failure, and on the final failure return.
        let mut wire_box = Box::new(wire);
        for key in &peer_keys {
            match rewrap_inbound_blob(crypto, *wire_box, key) {
                Ok(pair) => return Ok(pair),
                Err(w) => {
                    wire_box = w;
                }
            }
        }
        return Err(wire_box);
    }

    let blob = match wire.content.as_ref() {
        Some(b) => b.clone(),
        None => return Err(Box::new(wire)),
    };

    // Try each pairwise key until AEAD decryption succeeds (CopyPaste-kw2).
    let plaintext = {
        let mut found: Option<Vec<u8>> = None;
        for key in &peer_keys {
            match decrypt_from_cloud(key, &wire.item_id, &blob) {
                Ok(pt) => {
                    found = Some(pt);
                    break;
                }
                Err(_) => continue,
            }
        }
        match found {
            Some(pt) => pt,
            None => {
                warn!(item_id = %wire.item_id, "sync_orch: rekey_inbound: all peer keys failed to decrypt (tried {})", peer_keys.len());
                return Err(Box::new(wire));
            }
        }
    };

    // Re-encrypt under this device's local v2 key + v4 AAD so the stored row is
    // readable by the production read path (`decrypt_item_by_version` at v2).
    let aad = build_item_aad_v2(&wire.item_id, AAD_SCHEMA_VERSION_V4, 2);
    let (nonce, ciphertext) = match encrypt_item_with_aad(&plaintext, &crypto.v2_key, &aad) {
        Ok(out) => out,
        Err(e) => {
            warn!(item_id = %wire.item_id, "sync_orch: rekey_inbound local-encrypt failed: {e}");
            return Err(Box::new(wire));
        }
    };

    let mut local = wire_to_local(wire);
    local.content = Some(ciphertext);
    local.content_nonce = Some(nonce.to_vec());
    local.key_version = 2;
    Ok((local, Some(plaintext)))
}

/// Inverse of [`rekey_blob_outbound`]: unwrap a sync-key-wrapped image/file
/// payload and re-chunk it under THIS device's local v1 seed so the stored row
/// reads back through the production image/file decode path.
///
/// 1. `decrypt_from_cloud(shared, item_id, content)` → plaintext (the original
///    PNG / file bytes).
/// 2. Re-derive `file_id` deterministically from the plaintext content hash so
///    the AEAD AAD matches on both devices and item_id/dedup converge.
/// 3. Re-encode under `crypto.v1_key` (image → [`encode_image_with_limit`],
///    file → [`encode_file`]) → `chunks_to_blob` → `local.content`; rebuild the
///    meta JSON; set `blob_ref`, `content_type`, `key_version = 1` (chunks are
///    v1-keyed). `fts_plaintext = None` (blobs are not FTS-indexed).
///
/// Returns `Err(wire)` (hand back unchanged) on any failure so the caller can
/// fall back to verbatim storage.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
#[allow(clippy::result_large_err)]
fn rewrap_inbound_blob(
    crypto: &SyncCrypto,
    wire: WireItem,
    shared: &SyncKey,
) -> Result<(ClipboardItem, Option<Vec<u8>>), Box<WireItem>> {
    // F2: decrypt borrows the at-rest blob in place — no `.clone()` of the
    // (potentially multi-MiB) ciphertext. We still hand `wire` back intact on
    // either failure path so the caller's verbatim-storage fallback keeps the
    // original `content`. The borrow of `wire.content` ends before each
    // `Err(Box::new(wire))` move (NLL), so returning `wire` is sound.
    let plaintext = match wire.content.as_deref() {
        Some(blob) => match decrypt_from_cloud(shared, &wire.item_id, blob) {
            Ok(pt) => pt,
            Err(e) => {
                warn!(item_id = %wire.item_id, "sync_orch: inbound blob shared-decrypt failed: {e}");
                return Err(Box::new(wire));
            }
        },
        None => return Err(Box::new(wire)),
    };

    // Re-derive file_id deterministically from the recovered bytes (same hash
    // the sender used at capture) so item_id and dedup converge across devices.
    let file_id = crate::clipboard::image_content_hash(&plaintext);

    let (chunks_blob, meta_json) = if wire.content_type == "image" {
        match encode_image_with_limit(
            &plaintext,
            &crypto.v1_key,
            &file_id,
            copypaste_core::MAX_IMAGE_BYTES,
            copypaste_core::config::MAX_DECODED_IMAGE_MB,
        ) {
            Ok((meta, chunks)) => {
                let blob = match copypaste_core::chunks_to_blob(&chunks) {
                    Ok(b) => b,
                    Err(e) => {
                        warn!(item_id = %wire.item_id, "sync_orch: inbound image chunks_to_blob failed: {e}");
                        return Err(Box::new(wire));
                    }
                };
                // No thumbnail is synced (regenerated on demand); record a
                // distinct thumb_file_id with zero dims so the meta shape stays
                // consistent and get_item_thumbnail returns the null sentinel.
                let thumb_file_id = crate::clipboard::image_thumb_file_id(&file_id);
                let meta_json =
                    crate::clipboard::build_image_meta_json(&meta, &thumb_file_id, 0, 0);
                (blob, meta_json)
            }
            Err(e) => {
                warn!(item_id = %wire.item_id, "sync_orch: inbound image re-encode failed: {e}");
                return Err(Box::new(wire));
            }
        }
    } else {
        // File: re-chunk verbatim. Prefer the dedicated wire fields
        // (file_name / mime) stamped by `rekey_blob_outbound`; fall back to
        // parsing blob_ref (pre-21b peers or direct non-rekey paths) and
        // finally to neutral defaults when neither is available.
        let (filename, mime) = if wire.file_name.is_some() || wire.mime.is_some() {
            (
                wire.file_name.clone().unwrap_or_else(|| "file".to_string()),
                wire.mime
                    .clone()
                    .unwrap_or_else(|| "application/octet-stream".to_string()),
            )
        } else {
            wire.blob_ref
                .as_deref()
                .and_then(parse_file_name_mime)
                .unwrap_or_else(|| ("file".to_string(), "application/octet-stream".to_string()))
        };
        // B3: this is the INBOUND re-chunk path; the configured per-device
        // capture knob (`max_file_size_bytes`) is NOT threaded this deep (doing
        // so would change `run`'s signature and its daemon.rs call site, which is
        // out of scope here). Using `MAX_FILE_BYTES` is now coherent regardless:
        // `clamp_values` caps the user knob AT `MAX_FILE_BYTES`, so the storable
        // ceiling and this bound are the same number — we accept any item a peer
        // could legitimately have stored, never more.
        match copypaste_core::encode_file(
            &plaintext,
            &filename,
            &mime,
            &crypto.v1_key,
            &file_id,
            copypaste_core::MAX_FILE_BYTES,
        ) {
            Ok((meta, chunks)) => {
                let blob = match copypaste_core::chunks_to_blob(&chunks) {
                    Ok(b) => b,
                    Err(e) => {
                        warn!(item_id = %wire.item_id, "sync_orch: inbound file chunks_to_blob failed: {e}");
                        return Err(Box::new(wire));
                    }
                };
                let meta_json = crate::clipboard::build_file_meta_json(&meta);
                (blob, meta_json)
            }
            Err(e) => {
                warn!(item_id = %wire.item_id, "sync_orch: inbound file re-encode failed: {e}");
                return Err(Box::new(wire));
            }
        }
    };

    let mut local = wire_to_local(wire);
    local.content = Some(chunks_blob);
    local.content_nonce = None;
    local.blob_ref = Some(meta_json);
    // Chunk content is keyed by the LOCAL v1 seed + file_id AAD, NOT the v2
    // item-AAD scheme — the image/file read paths decode with v1.
    local.key_version = 1;
    Ok((local, None))
}

/// Parse `filename` / `mime` out of a file `blob_ref` meta JSON (the shape
/// produced by `clipboard::build_file_meta_json`). Returns `None` if either
/// field is absent so the caller can fall back to neutral defaults.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn parse_file_name_mime(meta_json: &str) -> Option<(String, String)> {
    let value: serde_json::Value = serde_json::from_str(meta_json).ok()?;
    let filename = value.get("filename")?.as_str()?.to_string();
    let mime = value.get("mime")?.as_str()?.to_string();
    Some((filename, mime))
}

/// Write decrypted plaintext for a synced item directly to NSPasteboard.
///
/// Called from [`merge_incoming_with_crypto`] after determining that the
/// incoming item is the freshest thing in the DB and `auto_apply_synced_clip`
/// is enabled.
///
/// # Loop prevention
///
/// The self-write guard works identically to the `copy_item` IPC handler:
/// 1. Read the *current* NSPasteboard `changeCount` (pre-write).
/// 2. Pre-stamp the expected post-write value (`current + 2`) into
///    `self_write_change_count` **before** calling `clearContents` /
///    `setString_forType`, so no poll arriving between the write and the stamp
///    can slip through with a stale sentinel.
/// 3. After the write, overwrite with the *actual* new `changeCount` so a
///    macOS increment that differs from our prediction is handled correctly.
/// 4. On any failure reset the sentinel to `-1` to avoid permanent suppression.
///
/// # Content types
///
/// * `text` — writes `NSPasteboardTypeString`.  `plaintext` is the raw UTF-8
///   bytes returned by [`rekey_inbound`].
/// * `image` — `plaintext` is an empty-vec sentinel (set in the merge loop
///   because the full PNG was not re-materialised there).  We re-decode it
///   here from the stored chunks in the DB row.
/// * `file` — **skipped** (a temp-file round-trip is required; deferred).
///   Logged at DEBUG.
///
/// All Cocoa calls are wrapped in `autoreleasepool` to prevent Objective-C
/// object leaks on this tokio blocking thread (mirrors `clipboard.rs::poll` and
/// `ipc.rs::write_to_pasteboard`).
#[cfg_attr(not(target_os = "macos"), allow(unused_variables))]
fn apply_to_pasteboard_if_fresh(
    db: &Database,
    content_type: &str,
    plaintext: Vec<u8>,
    local_key: &zeroize::Zeroizing<[u8; 32]>,
    self_write_change_count: &Arc<AtomicI64>,
) {
    #[cfg(target_os = "macos")]
    {
        use objc2_app_kit::NSPasteboard;

        match content_type {
            "text" => {
                let text = match std::str::from_utf8(&plaintext) {
                    Ok(s) => s,
                    Err(e) => {
                        warn!("sync_orch: auto-apply text is not UTF-8: {e}");
                        return;
                    }
                };
                objc2::rc::autoreleasepool(|_pool| {
                    use objc2_app_kit::NSPasteboardTypeString;
                    use objc2_foundation::NSString;

                    // Pre-stamp expected changeCount (clearContents +1, setString +1).
                    let pre = unsafe { NSPasteboard::generalPasteboard().changeCount() } as i64;
                    self_write_change_count.store(pre + 2, Ordering::Release);

                    let ok = unsafe {
                        let pb = NSPasteboard::generalPasteboard();
                        pb.clearContents();
                        let ns_str = NSString::from_str(text);
                        pb.setString_forType(&ns_str, NSPasteboardTypeString)
                    };
                    if ok {
                        // Post-stamp with the actual changeCount.
                        let actual =
                            unsafe { NSPasteboard::generalPasteboard().changeCount() } as i64;
                        self_write_change_count.store(actual, Ordering::Release);
                        debug!(
                            change_count = actual,
                            "sync_orch: auto-applied synced text to NSPasteboard"
                        );
                    } else {
                        // Reset sentinel so the monitor is not permanently suppressed.
                        self_write_change_count.store(-1, Ordering::Release);
                        warn!("sync_orch: auto-apply text: NSPasteboard setString:forType: returned false");
                    }
                });
            }
            "image" => {
                // `plaintext` is an empty-vec sentinel from the merge loop
                // (the PNG was not re-materialised there to avoid a second
                // decode pass).  Recover the PNG from the most-recent image
                // row in the DB — it was just inserted/updated by this merge.
                let png_bytes = recover_latest_image_png(db, local_key);
                let png_bytes = match png_bytes {
                    Some(b) => b,
                    None => {
                        warn!("sync_orch: auto-apply image: could not recover PNG from DB");
                        return;
                    }
                };
                objc2::rc::autoreleasepool(|_pool| {
                    use objc2_foundation::{NSData, NSString};

                    let pre = unsafe { NSPasteboard::generalPasteboard().changeCount() } as i64;
                    self_write_change_count.store(pre + 2, Ordering::Release);

                    let ok = unsafe {
                        let pb = NSPasteboard::generalPasteboard();
                        pb.clearContents();
                        let type_str = NSString::from_str("public.png");
                        let data = NSData::with_bytes(&png_bytes);
                        pb.setData_forType(Some(&data), &type_str)
                    };
                    if ok {
                        let actual =
                            unsafe { NSPasteboard::generalPasteboard().changeCount() } as i64;
                        self_write_change_count.store(actual, Ordering::Release);
                        debug!(
                            change_count = actual,
                            "sync_orch: auto-applied synced image to NSPasteboard"
                        );
                    } else {
                        self_write_change_count.store(-1, Ordering::Release);
                        warn!("sync_orch: auto-apply image: NSPasteboard setData:forType: returned false");
                    }
                });
            }
            "file" => {
                // Files require writing bytes to a temp file and placing its
                // file-URL on the pasteboard — deferred to a future iteration.
                debug!("sync_orch: auto-apply skipped for file item (not yet supported)");
            }
            other => {
                debug!(
                    content_type = other,
                    "sync_orch: auto-apply skipped for unknown content_type"
                );
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        // Non-macOS: no NSPasteboard. No-op — called only on macOS in production.
        debug!(content_type, "sync_orch: auto-apply skipped (not macOS)");
    }
}

/// Recover PNG bytes for the most-recently-inserted image row from the DB.
///
/// Used by [`apply_to_pasteboard_if_fresh`] for the image auto-apply path.
/// Reads the newest image row's chunk blob + blob_ref, decodes with `local_key`
/// (v1 seed, the chunk-encryption key), and returns the raw PNG bytes.
/// Returns `None` on any parse/decrypt failure (logged at DEBUG).
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn recover_latest_image_png(
    db: &Database,
    local_key: &zeroize::Zeroizing<[u8; 32]>,
) -> Option<Vec<u8>> {
    use copypaste_core::{chunks_from_blob, decode_image};

    // Fetch the most recent image row (just inserted by this merge).
    let (content, blob_ref): (Vec<u8>, String) = db
        .conn()
        .query_row(
            "SELECT content, blob_ref FROM clipboard_items \
             WHERE content_type = 'image' AND content IS NOT NULL AND blob_ref IS NOT NULL \
             ORDER BY wall_time DESC, id DESC LIMIT 1",
            [],
            |r| Ok((r.get::<_, Vec<u8>>(0)?, r.get::<_, String>(1)?)),
        )
        .ok()?;

    let file_id = crate::ipc::parse_image_file_id(&blob_ref)
        .map_err(|e| {
            debug!("sync_orch: auto-apply image: blob_ref parse failed: {e}");
        })
        .ok()?;

    let chunks = chunks_from_blob(&content)
        .map_err(|e| {
            debug!("sync_orch: auto-apply image: chunks_from_blob failed: {e}");
        })
        .ok()?;

    decode_image(&chunks, local_key, &file_id)
        .map_err(|e| {
            debug!("sync_orch: auto-apply image: decode_image failed: {e}");
        })
        .ok()
}

// ── Poison-row guard (CopyPaste-jww / CopyPaste-5y4) ─────────────────────────

/// Returns `true` when a [`WireItem`] would become a poison row if stored
/// verbatim — i.e. when `rekey_inbound` failed because the shared sync key is
/// missing or wrong and the item was sync-key-wrapped.
///
/// A sync-key-wrapped item has `content` (the wrapped blob) but the sender
/// strips `content_nonce` (which is the "no local-nonce" sentinel on the wire)
/// and for file/image items also strips `blob_ref`. Storing such an item means
/// consumers will see a row with ciphertext they cannot decrypt AND no nonce /
/// no blob reference — causing "missing content_nonce" or "missing blob_ref
/// metadata" errors on every read.
///
/// The check is intentionally conservative:
/// * `text` is poison when `content` is present and `content_nonce` is absent.
/// * `file` / `image` are poison when `content` is present, `content_nonce` is
///   absent, AND `blob_ref` is also absent.  A file item that arrived via the
///   large-blob path carries `blob_ref` even without a nonce — that is a
///   legitimate row and must not be discarded.
pub fn is_poison_wire(w: &WireItem) -> bool {
    if w.content.is_none() {
        // No ciphertext at all (tombstone or empty) — not a poison row.
        return false;
    }
    match w.content_type.as_str() {
        "text" => w.content_nonce.is_none(),
        "file" | "image" => w.content_nonce.is_none() && w.blob_ref.is_none(),
        // Unknown content types: be conservative, do not treat as poison.
        _ => false,
    }
}

/// Delete all poison rows from `clipboard_items` and return the count removed.
///
/// A poison row is any row that was stored verbatim from a sync-key-wrapped
/// wire item (i.e. `rekey_inbound` failed) and therefore lacks the fields
/// consumers need to decrypt it:
/// * `content_type = 'text'` with `content IS NOT NULL` and `content_nonce IS NULL`
/// * `content_type IN ('file', 'image')` with `content IS NOT NULL`,
///   `content_nonce IS NULL`, and `blob_ref IS NULL`
///
/// Safe to call at startup on every restart — idempotent.  The affected peers
/// will re-send the items on their next catch-up cycle (sync is idempotent).
///
/// Returns `Err` only on SQLite failures; a zero-row result is `Ok(0)`.
pub fn sweep_poison_rows(db: &Database) -> Result<usize, anyhow::Error> {
    let n = db.conn().execute(
        "DELETE FROM clipboard_items \
         WHERE (content_type = 'text' \
                AND content IS NOT NULL \
                AND content_nonce IS NULL) \
            OR (content_type IN ('file', 'image') \
                AND content IS NOT NULL \
                AND content_nonce IS NULL \
                AND blob_ref IS NULL)",
        [],
    )?;
    if n > 0 {
        warn!(
            swept = n,
            "sync_orch: swept {n} poison row(s) \
             (sync-key-wrapped items stored without content_nonce/blob_ref \
             — peers will re-send on next connect) (CopyPaste-jww/5y4)"
        );
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use copypaste_core::insert_item;

    fn make_db() -> Arc<Mutex<Database>> {
        Arc::new(Mutex::new(
            Database::open_in_memory().expect("in-memory DB must open"),
        ))
    }

    fn make_wire(id: &str, lamport: i64, content: u8) -> WireItem {
        WireItem {
            deleted: false,
            pinned: false,
            pin_order: None,
            id: id.to_string(),
            item_id: format!("{id}-iid"),
            content_type: "text".to_string(),
            content: Some(vec![content]),
            content_nonce: Some(vec![0u8; 24]),
            blob_ref: None,
            is_sensitive: false,
            lamport_ts: lamport,
            wall_time: 1_700_000_000_000 + lamport,
            expires_at: None,
            app_bundle_id: None,
            origin_device_id: "remote-device".to_string(),
            key_version: 2,
            file_name: None,
            mime: None,
        }
    }

    /// P2P Phase 3 (cross-device readability): an item encrypted at rest under
    /// device A's per-device local key must, after `rekey_outbound` (shared key
    /// re-wrap) → `rekey_inbound` (shared key unwrap + re-wrap under device B's
    /// local key), be readable on B via the production read path
    /// (`decrypt_item_by_version`). A and B have DIFFERENT local seeds — the
    /// whole point of the shared sync key.
    #[test]
    fn rekey_round_trip_makes_item_readable_on_peer_with_different_local_key() {
        use base64::Engine as _;
        use copypaste_core::{
            build_item_aad_v2, derive_v2, encrypt_item_with_aad, AAD_SCHEMA_VERSION_V4,
        };
        use tempfile::tempdir;

        // Two distinct devices, two distinct local seeds.
        let seed_a = [0x11u8; 32];
        let seed_b = [0x22u8; 32];
        assert_ne!(seed_a, seed_b);

        // The shared content sync key both sides persisted at pairing (same 32
        // bytes on both — here a fixed test value).
        let shared = [0x33u8; 32];
        let shared_b64 = base64::engine::general_purpose::STANDARD.encode(shared);

        let item_id = "iid-rekey-001".to_string();
        let plaintext = b"the answer is 42 and it travelled over real P2P";

        // SENDER A: item is stored encrypted under A's v2 local key (exactly as
        // a freshly-captured text item is).
        let a_v2 = derive_v2(&seed_a);
        let aad_a = build_item_aad_v2(&item_id, AAD_SCHEMA_VERSION_V4, 2);
        let (nonce_a, ct_a) =
            encrypt_item_with_aad(plaintext, &a_v2, &aad_a).expect("A local encrypt");

        // A's wire item carries A's at-rest ciphertext + nonce.
        let mut wire = WireItem {
            deleted: false,
            pinned: false,
            pin_order: None,
            id: "row-1".to_string(),
            item_id: item_id.clone(),
            content_type: "text".to_string(),
            content: Some(ct_a),
            content_nonce: Some(nonce_a.to_vec()),
            blob_ref: None,
            is_sensitive: false,
            lamport_ts: 7,
            wall_time: 1_700_000_000_000,
            expires_at: None,
            app_bundle_id: None,
            origin_device_id: "device-A".to_string(),
            key_version: 2,
            file_name: None,
            mime: None,
        };

        // A's peers.json holds the shared key (peer = B).
        let dir_a = tempdir().unwrap();
        let peers_a = dir_a.path().join("peers.json");
        std::fs::write(
            &peers_a,
            format!(
                r#"[{{"fingerprint":"bb:bb","added_at":1,"address":"127.0.0.1:9","sync_key_b64":"{shared_b64}"}}]"#
            ),
        )
        .unwrap();
        let crypto_a = SyncCrypto::new(seed_a, peers_a);

        // OUTBOUND: re-key under the shared sync key.
        rekey_outbound(&crypto_a, &mut wire);
        assert!(
            wire.content_nonce.is_none(),
            "sync-key-wrapped payload must clear the item nonce (self-framed blob)"
        );
        // The wire content is no longer A's at-rest ciphertext.
        assert_ne!(wire.content.as_deref(), Some(plaintext.as_slice()));

        // RECEIVER B: different local seed, same shared key in its peers.json.
        let dir_b = tempdir().unwrap();
        let peers_b = dir_b.path().join("peers.json");
        std::fs::write(
            &peers_b,
            format!(
                r#"[{{"fingerprint":"aa:aa","added_at":1,"address":"127.0.0.1:9","sync_key_b64":"{shared_b64}"}}]"#
            ),
        )
        .unwrap();
        let crypto_b = SyncCrypto::new(seed_b, peers_b);

        // INBOUND: unwrap with shared key, re-wrap under B's local v2 key.
        let (stored, recovered_pt) =
            rekey_inbound(&crypto_b, wire).expect("B must unwrap the sync-key-wrapped item");
        assert_eq!(
            recovered_pt.as_deref(),
            Some(plaintext.as_slice()),
            "B recovers A's original plaintext"
        );
        assert_eq!(stored.key_version, 2);

        // B's production read path: decrypt the stored row with B's keys.
        let b_v2 = derive_v2(&seed_b);
        let stored_nonce = stored.content_nonce.expect("nonce");
        let mut narr = [0u8; NONCE_SIZE];
        narr.copy_from_slice(&stored_nonce);
        let read_back = decrypt_item_by_version(
            stored.key_version,
            &seed_b, // v1
            &b_v2,   // v2
            &stored.item_id,
            &narr,
            &stored.content.expect("content"),
        )
        .expect("B read path must decrypt the stored synced item");
        assert_eq!(
            read_back, plaintext,
            "synced item reads back as A's original plaintext on B"
        );
    }

    /// v0.6 image sync: an image stored at rest under device A's local v1 seed
    /// (the chunk-encryption key handle, exactly as `handle_image` uses) must,
    /// after `rekey_outbound` (image arm — reassembles plaintext, re-wraps under
    /// the shared key) → `rekey_inbound` (re-chunks under device B's local key),
    /// decode back to the ORIGINAL PNG bytes on B, with a re-derived
    /// file_id/item_id that converges with A's (deterministic dedup).
    #[test]
    fn image_rekey_round_trip_decodes_back_to_original_png_on_peer() {
        use base64::Engine as _;
        use copypaste_core::{
            chunks_from_blob, chunks_to_blob, decode_image, encode_image_with_limit,
        };
        use tempfile::tempdir;

        let seed_a = [0x11u8; 32];
        let seed_b = [0x22u8; 32];
        let shared = [0x33u8; 32];
        let shared_b64 = base64::engine::general_purpose::STANDARD.encode(shared);

        // A real tiny PNG (2x2 RGB). Built via the image crate so encode/decode
        // is exercised end-to-end.
        let png = {
            use image::ImageEncoder;
            let mut buf = Vec::new();
            let raw = vec![0xFFu8; 2 * 2 * 3];
            image::codecs::png::PngEncoder::new(&mut buf)
                .write_image(&raw, 2, 2, image::ExtendedColorType::Rgb8)
                .expect("encode test png");
            buf
        };

        // SENDER A: store the image under A's LOCAL v1 seed (handle_image path).
        let file_id_a = crate::clipboard::image_content_hash(&png);
        let (_meta_a, chunks_a) =
            encode_image_with_limit(&png, &seed_a, &file_id_a, 0, 256).expect("A encode image");
        let blob_a = chunks_to_blob(&chunks_a).expect("A blob");
        let item_id = uuid::Uuid::from_bytes(file_id_a).to_string();
        let meta_json_a = format!(r#"{{"file_id":{file_id_a:?}}}"#);

        let mut wire = WireItem {
            deleted: false,
            pinned: false,
            pin_order: None,
            id: "img-row".to_string(),
            item_id: item_id.clone(),
            content_type: "image".to_string(),
            content: Some(blob_a),
            content_nonce: Some(vec![0u8; 24]),
            blob_ref: Some(meta_json_a),
            is_sensitive: false,
            lamport_ts: 9,
            wall_time: 1_700_000_000_000,
            expires_at: None,
            app_bundle_id: None,
            origin_device_id: "device-A".to_string(),
            key_version: 1,
            file_name: None,
            mime: None,
        };

        let dir_a = tempdir().unwrap();
        let peers_a = dir_a.path().join("peers.json");
        std::fs::write(
            &peers_a,
            format!(
                r#"[{{"fingerprint":"bb:bb","added_at":1,"address":"127.0.0.1:9","sync_key_b64":"{shared_b64}"}}]"#
            ),
        )
        .unwrap();
        let crypto_a = SyncCrypto::new(seed_a, peers_a);

        // OUTBOUND image arm: reassemble + re-wrap under shared key.
        assert_eq!(
            rekey_outbound(&crypto_a, &mut wire),
            RekeyOutcome::Rewrapped
        );
        assert!(wire.content_nonce.is_none(), "wrapped blob clears nonce");
        assert!(wire.blob_ref.is_none(), "blob_ref dropped on the wire");
        assert_eq!(wire.content_type, "image");

        // RECEIVER B: different local seed, same shared key.
        let dir_b = tempdir().unwrap();
        let peers_b = dir_b.path().join("peers.json");
        std::fs::write(
            &peers_b,
            format!(
                r#"[{{"fingerprint":"aa:aa","added_at":1,"address":"127.0.0.1:9","sync_key_b64":"{shared_b64}"}}]"#
            ),
        )
        .unwrap();
        let crypto_b = SyncCrypto::new(seed_b, peers_b);

        let (stored, fts) = rekey_inbound(&crypto_b, wire).expect("B must unwrap the synced image");
        assert!(fts.is_none(), "images carry no FTS plaintext");
        assert_eq!(stored.content_type, "image");
        assert_eq!(
            stored.item_id, item_id,
            "item_id must converge across devices"
        );

        // B decodes its re-chunked blob under B's local seed → original PNG.
        let meta_b = stored.blob_ref.expect("B meta json");
        let file_id_b = crate::ipc::parse_image_file_id(&meta_b).expect("parse B file_id");
        assert_eq!(file_id_b, file_id_a, "file_id must converge");
        let b_chunks =
            chunks_from_blob(&stored.content.expect("B content")).expect("B chunks_from_blob");
        let recovered = decode_image(&b_chunks, &seed_b, &file_id_b).expect("B decode image");
        assert_eq!(recovered, png, "B recovers A's exact PNG bytes");
    }

    /// v0.6 file sync: same as the image round-trip but for an arbitrary file
    /// blob (`content_type = "file"`), chunked verbatim (no decode/re-encode).
    #[test]
    fn file_rekey_round_trip_decodes_back_to_original_bytes_on_peer() {
        use base64::Engine as _;
        use copypaste_core::{chunks_from_blob, chunks_to_blob, decode_file, encode_file};
        use tempfile::tempdir;

        let seed_a = [0x44u8; 32];
        let seed_b = [0x55u8; 32];
        let shared = [0x66u8; 32];
        let shared_b64 = base64::engine::general_purpose::STANDARD.encode(shared);

        let raw = b"arbitrary file bytes \x00\x01\x02 over P2P sync".to_vec();

        let file_id_a = crate::clipboard::image_content_hash(&raw);
        let (meta_a, chunks_a) = encode_file(
            &raw,
            "notes.bin",
            "application/octet-stream",
            &seed_a,
            &file_id_a,
            0,
        )
        .expect("A encode file");
        let blob_a = chunks_to_blob(&chunks_a).expect("A blob");
        let item_id = uuid::Uuid::from_bytes(file_id_a).to_string();
        let meta_json_a = crate::clipboard::build_file_meta_json(&meta_a);

        let mut wire = WireItem {
            deleted: false,
            pinned: false,
            pin_order: None,
            id: "file-row".to_string(),
            item_id: item_id.clone(),
            content_type: "file".to_string(),
            content: Some(blob_a),
            content_nonce: Some(vec![0u8; 24]),
            blob_ref: Some(meta_json_a),
            is_sensitive: false,
            lamport_ts: 11,
            wall_time: 1_700_000_000_000,
            expires_at: None,
            app_bundle_id: None,
            origin_device_id: "device-A".to_string(),
            key_version: 1,
            file_name: None,
            mime: None,
        };

        let dir_a = tempdir().unwrap();
        let peers_a = dir_a.path().join("peers.json");
        std::fs::write(
            &peers_a,
            format!(
                r#"[{{"fingerprint":"bb:bb","added_at":1,"address":"127.0.0.1:9","sync_key_b64":"{shared_b64}"}}]"#
            ),
        )
        .unwrap();
        let crypto_a = SyncCrypto::new(seed_a, peers_a);

        assert_eq!(
            rekey_outbound(&crypto_a, &mut wire),
            RekeyOutcome::Rewrapped
        );
        assert!(wire.content_nonce.is_none());
        assert!(
            wire.blob_ref.is_none(),
            "blob_ref must be cleared on the wire"
        );
        assert_eq!(wire.content_type, "file");
        // #21b: filename + mime must be stamped on the wire before blob_ref is cleared.
        assert_eq!(
            wire.file_name.as_deref(),
            Some("notes.bin"),
            "rekey_blob_outbound must stamp file_name onto the wire"
        );
        assert_eq!(
            wire.mime.as_deref(),
            Some("application/octet-stream"),
            "rekey_blob_outbound must stamp mime onto the wire"
        );

        let dir_b = tempdir().unwrap();
        let peers_b = dir_b.path().join("peers.json");
        std::fs::write(
            &peers_b,
            format!(
                r#"[{{"fingerprint":"aa:aa","added_at":1,"address":"127.0.0.1:9","sync_key_b64":"{shared_b64}"}}]"#
            ),
        )
        .unwrap();
        let crypto_b = SyncCrypto::new(seed_b, peers_b);

        let (stored, fts) = rekey_inbound(&crypto_b, wire).expect("B must unwrap the synced file");
        assert!(fts.is_none());
        assert_eq!(stored.content_type, "file");
        assert_eq!(stored.item_id, item_id, "item_id converges");

        let meta_b = stored.blob_ref.expect("B meta json");
        // #21b: verify that filename + mime survive the full rekey round-trip
        // (outbound stamps wire fields; inbound reads them to rebuild file meta).
        let (recovered_name, recovered_mime) =
            parse_file_name_mime(&meta_b).expect("B meta must carry filename+mime");
        assert_eq!(
            recovered_name, "notes.bin",
            "filename must survive outbound→wire→inbound"
        );
        assert_eq!(
            recovered_mime, "application/octet-stream",
            "mime must survive outbound→wire→inbound"
        );
        let file_id_b = crate::ipc::parse_image_file_id(&meta_b).expect("parse B file_id");
        assert_eq!(file_id_b, file_id_a);
        let b_chunks =
            chunks_from_blob(&stored.content.expect("B content")).expect("B chunks_from_blob");
        let recovered = decode_file(&b_chunks, &seed_b, &file_id_b).expect("B decode file");
        assert_eq!(recovered, raw, "B recovers A's exact file bytes");
    }

    /// CopyPaste-716: 3-device topology (A paired with B and C under DIFFERENT
    /// pairwise keys) must produce per-peer ciphertext blobs.
    ///
    /// Before the fix: `rekey_outbound` used `shared_sync_key()` (first peer
    /// only) so fanout to C produced a K_AB-encrypted blob that C could never
    /// decrypt. After the fix: `rekey_outbound_for_peer` uses the per-peer key
    /// from the HashMap cache, so each peer gets a blob it can actually decrypt.
    ///
    /// This test simulates device A sending to peers B and C:
    /// - K_AB = [0x33; 32], K_AC = [0x44; 32] (distinct pairwise keys)
    /// - Fanout to B must produce a blob decryptable under K_AB (not K_AC)
    /// - Fanout to C must produce a blob decryptable under K_AC (not K_AB)
    #[test]
    fn three_device_fanout_uses_per_peer_key_not_first_peer_key() {
        use base64::Engine as _;
        use copypaste_core::{
            build_item_aad_v2, derive_v2, decrypt_from_cloud, encrypt_item_with_aad,
            AAD_SCHEMA_VERSION_V4,
        };
        use tempfile::tempdir;

        // Device A's local seed.
        let seed_a = [0x11u8; 32];

        // Two distinct pairwise keys: K_AB (A↔B) and K_AC (A↔C).
        let k_ab: [u8; 32] = [0x33u8; 32];
        let k_ac: [u8; 32] = [0x44u8; 32];
        assert_ne!(k_ab, k_ac, "test requires distinct per-peer keys");

        let k_ab_b64 = base64::engine::general_purpose::STANDARD.encode(k_ab);
        let k_ac_b64 = base64::engine::general_purpose::STANDARD.encode(k_ac);

        // Peer fingerprints (as stored in peers.json / used as DeviceFingerprint).
        let fp_b = "bb:bb";
        let fp_c = "cc:cc";

        // A's peers.json: both B and C, each with their own pairwise key.
        let dir_a = tempdir().unwrap();
        let peers_a = dir_a.path().join("peers.json");
        std::fs::write(
            &peers_a,
            format!(
                r#"[
                    {{"fingerprint":"{fp_b}","added_at":1,"address":"127.0.0.1:9","sync_key_b64":"{k_ab_b64}"}},
                    {{"fingerprint":"{fp_c}","added_at":1,"address":"127.0.0.1:8","sync_key_b64":"{k_ac_b64}"}}
                ]"#
            ),
        )
        .unwrap();
        let crypto_a = SyncCrypto::new(seed_a, peers_a);

        // Confirm both keys loaded.
        assert!(
            crypto_a.sync_key_for_peer(fp_b).is_some(),
            "A must have K_AB for peer B"
        );
        assert!(
            crypto_a.sync_key_for_peer(fp_c).is_some(),
            "A must have K_AC for peer C"
        );

        // Prepare a plaintext item and encrypt it under A's local v2 key
        // (exactly as the daemon stores a captured clipboard item).
        let item_id = "iid-716-three-device".to_string();
        let plaintext = b"three-device sync test payload";
        let a_v2 = derive_v2(&seed_a);
        let aad_a = build_item_aad_v2(&item_id, AAD_SCHEMA_VERSION_V4, 2);
        let (nonce_a, ct_a) =
            encrypt_item_with_aad(plaintext, &a_v2, &aad_a).expect("A local encrypt");

        // Build the wire item (at-rest ciphertext from A's local storage).
        let wire_template = WireItem {
            deleted: false,
            pinned: false,
            pin_order: None,
            id: "row-716".to_string(),
            item_id: item_id.clone(),
            content_type: "text".to_string(),
            content: Some(ct_a),
            content_nonce: Some(nonce_a.to_vec()),
            blob_ref: None,
            is_sensitive: false,
            lamport_ts: 1,
            wall_time: 1_700_000_000_000,
            expires_at: None,
            app_bundle_id: None,
            origin_device_id: "device-A".to_string(),
            key_version: 2,
            file_name: None,
            mime: None,
        };

        // ── Fanout to peer B (should use K_AB) ───────────────────────────────
        let mut wire_for_b = wire_template.clone();
        let outcome_b = rekey_outbound_for_peer(&crypto_a, fp_b, &mut wire_for_b);
        assert_eq!(
            outcome_b,
            RekeyOutcome::Rewrapped,
            "fanout to B must succeed (K_AB present)"
        );
        assert!(
            wire_for_b.content_nonce.is_none(),
            "sync-key-wrapped payload clears item nonce"
        );
        let blob_b = wire_for_b.content.as_ref().unwrap().clone();

        // ── Fanout to peer C (should use K_AC) ───────────────────────────────
        let mut wire_for_c = wire_template.clone();
        let outcome_c = rekey_outbound_for_peer(&crypto_a, fp_c, &mut wire_for_c);
        assert_eq!(
            outcome_c,
            RekeyOutcome::Rewrapped,
            "fanout to C must succeed (K_AC present)"
        );
        let blob_c = wire_for_c.content.as_ref().unwrap().clone();

        // ── Verify: B's blob decrypts under K_AB ─────────────────────────────
        let key_b = copypaste_core::SyncKey::from_bytes(k_ab);
        let decrypted_b = decrypt_from_cloud(&key_b, &item_id, &blob_b)
            .expect("blob_b must decrypt under K_AB");
        assert_eq!(
            decrypted_b, plaintext,
            "B recovers A's original plaintext from its blob"
        );

        // ── Verify: C's blob decrypts under K_AC ─────────────────────────────
        let key_c = copypaste_core::SyncKey::from_bytes(k_ac);
        let decrypted_c = decrypt_from_cloud(&key_c, &item_id, &blob_c)
            .expect("blob_c must decrypt under K_AC");
        assert_eq!(
            decrypted_c, plaintext,
            "C recovers A's original plaintext from its blob"
        );

        // ── Key isolation: B's blob must NOT decrypt under K_AC ──────────────
        // (This is what was silently broken before the fix: fanout used K_AB for
        // all peers, so C received blob_b which it cannot decrypt.)
        let result_wrong = decrypt_from_cloud(&key_c, &item_id, &blob_b);
        assert!(
            result_wrong.is_err(),
            "blob_b (encrypted under K_AB) must NOT decrypt under K_AC — \
             this would be the CopyPaste-716 bug if it succeeded"
        );

        // ── Key isolation: C's blob must NOT decrypt under K_AB ──────────────
        let result_wrong2 = decrypt_from_cloud(&key_b, &item_id, &blob_c);
        assert!(
            result_wrong2.is_err(),
            "blob_c (encrypted under K_AC) must NOT decrypt under K_AB"
        );

        // ── Blobs differ (per-peer encryption with fresh nonces) ─────────────
        assert_ne!(
            blob_b, blob_c,
            "each peer must receive a distinct (independently encrypted) blob"
        );
    }

    /// CopyPaste-716: catchup_items must use the connecting peer's pairwise
    /// key, not the first peer's key. With 2+ peers, the catch-up set for peer
    /// C must decrypt under K_AC only, not K_AB.
    ///
    /// This is the equivalent catch-up path test for the fanout fix above.
    #[tokio::test]
    async fn catchup_items_uses_per_peer_key_not_first_peer_key() {
        use base64::Engine as _;
        use copypaste_core::{
            build_item_aad_v2, derive_v2, decrypt_from_cloud, encrypt_item_with_aad,
            insert_item, AAD_SCHEMA_VERSION_V4,
        };
        use tempfile::tempdir;

        let seed_a = [0x11u8; 32];
        let k_ab: [u8; 32] = [0x33u8; 32];
        let k_ac: [u8; 32] = [0x44u8; 32];
        let k_ab_b64 = base64::engine::general_purpose::STANDARD.encode(k_ab);
        let k_ac_b64 = base64::engine::general_purpose::STANDARD.encode(k_ac);
        let fp_b = "bb:bb";
        let fp_c = "cc:cc";

        let dir_a = tempdir().unwrap();
        let peers_a = dir_a.path().join("peers.json");
        std::fs::write(
            &peers_a,
            format!(
                r#"[
                    {{"fingerprint":"{fp_b}","added_at":1,"address":"127.0.0.1:9","sync_key_b64":"{k_ab_b64}"}},
                    {{"fingerprint":"{fp_c}","added_at":1,"address":"127.0.0.1:8","sync_key_b64":"{k_ac_b64}"}}
                ]"#
            ),
        )
        .unwrap();
        let crypto_a = SyncCrypto::new(seed_a, peers_a);

        // Insert one text item into the DB encrypted under A's v2 key.
        let db = make_db();
        let item_id = "catchup-716-item".to_string();
        let plaintext = b"catchup per-peer key test";
        let a_v2 = derive_v2(&seed_a);
        let aad_a = build_item_aad_v2(&item_id, AAD_SCHEMA_VERSION_V4, 2);
        let (nonce_a, ct_a) =
            encrypt_item_with_aad(plaintext, &a_v2, &aad_a).expect("A local encrypt");

        let mut local = copypaste_core::ClipboardItem::new_text(ct_a, nonce_a.to_vec(), 1);
        local.item_id = item_id.clone();
        {
            let g = db.lock().await;
            insert_item(&g, &local).unwrap();
        }

        let db_guard = db.lock().await;
        // Catch-up for peer B: items must be encrypted under K_AB.
        let items_for_b = catchup_items(&db_guard, "device-A", &crypto_a, fp_b);
        assert_eq!(items_for_b.len(), 1, "catch-up for B must contain our item");
        let blob_b = items_for_b[0].content.as_ref().unwrap().clone();
        let key_b = copypaste_core::SyncKey::from_bytes(k_ab);
        let dec_b = decrypt_from_cloud(&key_b, &item_id, &blob_b)
            .expect("B's catch-up blob must decrypt under K_AB");
        assert_eq!(dec_b, plaintext, "B recovers original plaintext from catch-up");

        // Catch-up for peer C: items must be encrypted under K_AC.
        let items_for_c = catchup_items(&db_guard, "device-A", &crypto_a, fp_c);
        assert_eq!(items_for_c.len(), 1, "catch-up for C must contain our item");
        let blob_c = items_for_c[0].content.as_ref().unwrap().clone();
        let key_c = copypaste_core::SyncKey::from_bytes(k_ac);
        let dec_c = decrypt_from_cloud(&key_c, &item_id, &blob_c)
            .expect("C's catch-up blob must decrypt under K_AC");
        assert_eq!(dec_c, plaintext, "C recovers original plaintext from catch-up");

        // Key isolation: C's catch-up blob must NOT decrypt under K_AB.
        assert!(
            decrypt_from_cloud(&key_b, &item_id, &blob_c).is_err(),
            "C's catch-up blob (K_AC) must not decrypt under K_AB — \
             this would be the CopyPaste-716 bug if it succeeded"
        );
    }

    /// W2.2: an incoming WireItem from the transport must be persisted to the
    /// local DB via the LWW merge path.
    #[tokio::test]
    async fn sync_orch_inserts_incoming_wire_item() {
        let db = make_db();

        let (_local_tx, local_rx) = broadcast::channel::<ClipboardItem>(8);
        let (incoming_tx, incoming_rx) = mpsc::channel::<WireItem>(8);
        let (outbound_tx, _outbound_rx) = mpsc::channel::<WireItem>(8);

        let db_for_task = db.clone();
        let shutdown = CancellationToken::new();
        let handle = tokio::spawn(async move {
            run(
                db_for_task,
                local_rx,
                incoming_rx,
                outbound_tx,
                "local-device".to_string(),
                None,
                500_000_000, // storage_quota_bytes: 500 MB (test default)
                None,        // auto_apply: disabled in tests (no NSPasteboard)
                shutdown,
            )
            .await
            .expect("orchestrator must finish cleanly");
        });

        // Push one wire item from the "transport".
        let wire = make_wire("new-item", 5, 0xAB);
        incoming_tx.send(wire).await.expect("send incoming");

        // Let the orchestrator merge.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Drop senders so the orchestrator exits.
        drop(incoming_tx);
        drop(_local_tx);
        handle.await.expect("task join");

        let db_guard = db.lock().await;
        let rows = copypaste_core::get_page(&*db_guard, 10, 0).expect("get_page");
        assert_eq!(rows.len(), 1, "incoming item must be persisted");
        assert_eq!(rows[0].id, "new-item");
        assert!(rows[0].is_synced, "item from peer must be marked synced");
        assert_eq!(rows[0].lamport_ts, 5);
    }

    /// W2.2: a locally-produced item arriving on the broadcast channel must
    /// be forwarded to the transport's outbound channel.
    #[tokio::test]
    async fn sync_orch_broadcasts_local_item() {
        let db = make_db();

        let (local_tx, local_rx) = broadcast::channel::<ClipboardItem>(8);
        let (_incoming_tx, incoming_rx) = mpsc::channel::<WireItem>(8);
        let (outbound_tx, mut outbound_rx) = mpsc::channel::<WireItem>(8);

        let db_for_task = db.clone();
        let shutdown = CancellationToken::new();
        let handle = tokio::spawn(async move {
            run(
                db_for_task,
                local_rx,
                incoming_rx,
                outbound_tx,
                "local-device".to_string(),
                None,
                500_000_000, // storage_quota_bytes: 500 MB (test default)
                None,        // auto_apply: disabled in tests (no NSPasteboard)
                shutdown,
            )
            .await
            .expect("orchestrator must finish cleanly");
        });

        // Push a local item through the broadcast channel.
        let item = ClipboardItem::new_text(vec![0xCC, 0xDD], vec![0u8; 24], 9);
        let item_id = item.id.clone();
        local_tx.send(item).expect("broadcast send");

        // Receive on the transport side.
        let received =
            tokio::time::timeout(std::time::Duration::from_millis(200), outbound_rx.recv())
                .await
                .expect("must receive within 200ms")
                .expect("outbound channel must yield item");

        assert_eq!(received.id, item_id, "wire id must match local id");
        assert_eq!(
            received.origin_device_id, "local-device",
            "origin_device_id must be stamped by the orchestrator"
        );
        assert_eq!(received.lamport_ts, 9);

        // Tear down and join.
        drop(local_tx);
        drop(_incoming_tx);
        handle.await.expect("task join");
    }

    /// Op-propagation (v0.6.1): the wire now carries authoritative `pinned` /
    /// `pin_order`, so on LWW TakeRemote the winning wire's pin state is applied
    /// directly — this is what lets an explicit pin / UNPIN / reorder on one
    /// device converge onto the others. (The previous semantic OR-merged the
    /// local pin and was kept only because the wire had no pin field; that
    /// would silently swallow a remote unpin, which the "sync all operations"
    /// requirement forbids.)
    ///
    /// Here a locally-pinned row receives a newer remote that is unpinned →
    /// the row must become unpinned (remote unpin propagated). The lookup uses
    /// `wire.id`, so the local row's `id` must equal `wire.id` for TakeRemote.
    #[tokio::test]
    async fn merge_incoming_takeremote_propagates_wire_pin_state() {
        let db = make_db();
        // Local row pinned=true; lamport 3 < wire lamport 9 → TakeRemote fires.
        let mut local = ClipboardItem::new_text(vec![0x11], vec![0u8; 24], 3);
        local.id = "shared-id".to_string();
        local.item_id = "shared-id-iid".to_string();
        local.pinned = true;
        {
            let g = db.lock().await;
            insert_item(&g, &local).unwrap();
            let stored = copypaste_core::get_item_by_id(&*g, "shared-id")
                .unwrap()
                .unwrap();
            assert!(stored.pinned, "setup: local row must be pinned");
        }

        // Newer remote, unpinned (make_wire sets pinned=false, pin_order=None).
        let wire = make_wire("shared-id", 9, 0xFF);
        assert_eq!(wire.id, "shared-id");
        assert!(!wire.pinned, "setup: incoming wire is unpinned");

        let upserted = merge_incoming(&db, vec![wire]).await.unwrap();
        assert_eq!(upserted, 1, "newer remote must win LWW");

        let g = db.lock().await;
        let rows = copypaste_core::get_page(&*g, 10, 0).unwrap();
        assert_eq!(rows.len(), 1, "must remain ONE row");
        assert!(
            !rows[0].pinned,
            "remote unpin must propagate on TakeRemote — got pinned={}",
            rows[0].pinned
        );
    }

    /// LWW: a stale wire item (lower lamport) must NOT overwrite the local row.
    /// Identity is matched on the cross-device `item_id`, so the local row and
    /// the wire item share `item_id = "shared-iid"` (the local `id` is distinct,
    /// as it always is across devices).
    #[tokio::test]
    async fn merge_incoming_keeps_local_on_older_remote() {
        let db = make_db();
        // Pre-insert a local row with a higher lamport clock. Its `item_id`
        // matches the incoming wire's so they are recognised as the SAME item.
        let mut local = ClipboardItem::new_text(vec![0x11], vec![0u8; 24], 50);
        local.id = "shared".to_string();
        local.item_id = "shared-iid".to_string();
        {
            let g = db.lock().await;
            insert_item(&g, &local).unwrap();
        }

        let wire = make_wire("shared", 5, 0xFF); // older; item_id = "shared-iid"
        let upserted = merge_incoming(&db, vec![wire]).await.unwrap();
        assert_eq!(upserted, 0, "older remote must lose LWW");

        let g = db.lock().await;
        let rows = copypaste_core::get_page(&*g, 10, 0).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].content, Some(vec![0x11]), "local payload preserved");
    }

    /// CopyPaste-bfiu: a P2P tombstone for an UNKNOWN item_id (delete arrives
    /// before the create, out-of-order over P2P) must persist a tombstone row so
    /// a later lower-lamport create is LWW-rejected and the item stays deleted —
    /// instead of the old behaviour that silently skipped the tombstone and let
    /// the create resurrect the item.
    #[tokio::test]
    async fn merge_incoming_delete_before_create_does_not_resurrect() {
        let db = make_db();

        // 1) DELETE arrives first for an item we have never seen (lamport 20).
        let mut tomb = make_wire("ghost", 20, 0x00);
        tomb.item_id = "ghost-iid".to_string();
        tomb.deleted = true;
        tomb.content = None;
        tomb.content_nonce = None;
        let upserted = merge_incoming(&db, vec![tomb]).await.unwrap();
        assert_eq!(upserted, 1, "tombstone for unknown item must be persisted");

        {
            let g = db.lock().await;
            let row = copypaste_core::get_item_by_item_id(&g, "ghost-iid")
                .unwrap()
                .expect("tombstone row must exist");
            assert!(row.deleted, "unknown-item tombstone must persist as deleted");
            // The user-facing list must not show it.
            assert!(
                copypaste_core::get_page(&*g, 10, 0).unwrap().is_empty(),
                "tombstone must not appear in the history list"
            );
        }

        // 2) CREATE arrives later with a LOWER lamport (10 < 20) → loses LWW.
        let mut create = make_wire("ghost", 10, 0xAB);
        create.item_id = "ghost-iid".to_string();
        let upserted2 = merge_incoming(&db, vec![create]).await.unwrap();
        assert_eq!(upserted2, 0, "late lower-lamport create must NOT resurrect");

        let g = db.lock().await;
        let row = copypaste_core::get_item_by_item_id(&g, "ghost-iid")
            .unwrap()
            .expect("row still present");
        assert!(row.deleted, "item must stay deleted after the racing create");
        assert!(
            copypaste_core::get_page(&*g, 10, 0).unwrap().is_empty(),
            "item must remain hidden from history"
        );
    }

    /// CRDT identity + local-PK preservation: a TakeRemote (newer lamport) for
    /// an item already present locally under a DIFFERENT row `id` must replace
    /// the content in place while preserving the local primary key — so FTS,
    /// `copy_item`, and pins (all keyed on `id`) keep pointing at the same row.
    #[tokio::test]
    async fn merge_incoming_replaces_by_item_id_preserving_local_pk() {
        let db = make_db();
        // Local row: PK "local-pk", item_id "X", lamport 5.
        let mut local = ClipboardItem::new_text(vec![0x11], vec![0u8; 24], 5);
        local.id = "local-pk".to_string();
        local.item_id = "X".to_string();
        {
            let g = db.lock().await;
            insert_item(&g, &local).unwrap();
        }

        // Incoming wire: peer's own PK "peer-pk", SAME item_id "X", newer
        // lamport 9, different content.
        let mut wire = make_wire("peer-pk", 9, 0xFF);
        wire.item_id = "X".to_string();

        let upserted = merge_incoming(&db, vec![wire]).await.unwrap();
        assert_eq!(upserted, 1, "newer remote must win LWW");

        let g = db.lock().await;
        let rows = copypaste_core::get_page(&*g, 10, 0).unwrap();
        assert_eq!(
            rows.len(),
            1,
            "must remain ONE row (replace, not duplicate)"
        );
        assert_eq!(
            rows[0].id, "local-pk",
            "local primary key must be preserved"
        );
        assert_eq!(rows[0].item_id, "X");
        assert_eq!(rows[0].lamport_ts, 9, "remote (newer) lamport stored");
        assert_eq!(rows[0].content, Some(vec![0xFF]), "remote content stored");
        // The peer's row id must NOT have been adopted.
        assert!(
            copypaste_core::get_item_by_id(&*g, "peer-pk")
                .unwrap()
                .is_none(),
            "peer's row id must not leak into local storage"
        );
    }

    /// CopyPaste-kw2: 3-device topology — inbound item from peer C (encrypted
    /// under K_AC) must be decrypted correctly even when K_AB is the first
    /// entry in the peer key map.
    ///
    /// Before the fix: `rekey_inbound` called `shared_sync_key()` which returned
    /// `values().next()` — an arbitrary HashMap entry.  If K_AB was first, the
    /// item from C (encrypted under K_AC) failed to decrypt and was silently
    /// dropped.
    ///
    /// After the fix: `all_sync_keys()` returns all keys; `rekey_inbound` tries
    /// each until AEAD succeeds, so K_AC is always found regardless of iteration
    /// order.
    #[test]
    fn rekey_inbound_3_device_tries_all_keys() {
        use base64::Engine as _;
        use copypaste_core::{decrypt_from_cloud, encrypt_for_cloud};
        use tempfile::tempdir;

        // Distinct pairwise keys: K_AB and K_AC.
        let k_ab: [u8; 32] = [0x33u8; 32];
        let k_ac: [u8; 32] = [0x44u8; 32];
        assert_ne!(k_ab, k_ac);
        let k_ab_b64 = base64::engine::general_purpose::STANDARD.encode(k_ab);
        let k_ac_b64 = base64::engine::general_purpose::STANDARD.encode(k_ac);

        let fp_b = "bb:bb";
        let fp_c = "cc:cc";
        let seed_b_recv = [0x22u8; 32]; // Device B's local seed (the receiver in this test)

        // Device B's peers.json: both A and C, each with their own pairwise key.
        // B shares K_AB with A and K_BC with C — but for this test B receives
        // a blob from C encrypted under K_AC (the key A and C share).
        // We simulate a direct A→B case: B holds K_AB (to reach A) and K_AC (wrong).
        // The payload we produce is encrypted under K_AC and B should find it by
        // trying all keys.
        //
        // To make the scenario concrete: pretend this device IS A and receives a
        // blob from C (encrypted under K_AC).  A's peer map has K_AB (for B) first
        // and K_AC (for C) second.  The fix ensures K_AC is tried.
        let dir = tempdir().unwrap();
        let peers_path = dir.path().join("peers.json");
        std::fs::write(
            &peers_path,
            format!(
                r#"[
                    {{"fingerprint":"{fp_b}","added_at":1,"address":"127.0.0.1:9","sync_key_b64":"{k_ab_b64}"}},
                    {{"fingerprint":"{fp_c}","added_at":1,"address":"127.0.0.1:8","sync_key_b64":"{k_ac_b64}"}}
                ]"#
            ),
        )
        .unwrap();
        let crypto = SyncCrypto::new(seed_b_recv, peers_path);

        // Build a wire item encrypted under K_AC (as if sent by peer C).
        let item_id = "kw2-test-item".to_string();
        let plaintext = b"secret payload from peer C";
        let key_ac = copypaste_core::SyncKey::from_bytes(k_ac);
        let blob_ac = encrypt_for_cloud(&key_ac, &item_id, plaintext).expect("encrypt under K_AC");

        let wire = WireItem {
            deleted: false,
            pinned: false,
            pin_order: None,
            id: "kw2-row".to_string(),
            item_id: item_id.clone(),
            content_type: "text".to_string(),
            content: Some(blob_ac),
            content_nonce: None, // sync-key-wrapped (no local nonce)
            blob_ref: None,
            is_sensitive: false,
            lamport_ts: 1,
            wall_time: 1_700_000_000_000,
            expires_at: None,
            app_bundle_id: None,
            origin_device_id: "device-C".to_string(),
            key_version: 2,
            file_name: None,
            mime: None,
        };

        // rekey_inbound must succeed even if K_AB is iterated before K_AC.
        let (stored, fts_plaintext) =
            rekey_inbound(&crypto, wire).expect("must decrypt under K_AC (CopyPaste-kw2 fix)");

        assert_eq!(
            fts_plaintext.as_deref(),
            Some(plaintext.as_slice()),
            "recovered plaintext must match original"
        );
        // The stored row must be re-encrypted under this device's v2 key.
        assert!(
            stored.content_nonce.is_some(),
            "stored row must have a local nonce after rekey"
        );
        assert_eq!(stored.key_version, 2, "stored row must be keyed at v2");

        // Sanity: the wrong key (K_AB) alone would have failed.
        let key_ab = copypaste_core::SyncKey::from_bytes(k_ab);
        let blob_from_stored = stored.content.as_ref().unwrap();
        // The stored ciphertext is under the device's v2 key, not K_AB — just
        // confirm K_AB cannot decrypt the original blob (key isolation).
        // We reconstruct the original blob to check isolation.
        let original_blob = encrypt_for_cloud(&key_ac, &item_id, plaintext).unwrap();
        assert!(
            decrypt_from_cloud(&key_ab, &item_id, &original_blob).is_err(),
            "K_AB must not decrypt a blob encrypted under K_AC — key isolation"
        );
        let _ = blob_from_stored; // silence unused warning
    }

    /// CopyPaste-kcf: inbound items must have `is_sensitive` set from the
    /// decrypted plaintext, so cross-device sensitive items get the auto-wipe TTL.
    ///
    /// Before the fix: `wire_to_local` always set `is_sensitive = false`, so a
    /// password or API key copied on device A and synced to B was stored with
    /// `is_sensitive = false` on B — bypassing the auto-wipe TTL entirely.
    ///
    /// After the fix: `merge_incoming_with_crypto` runs `is_sensitive_for_autowipe`
    /// on the recovered plaintext and stores the correct value.
    #[tokio::test]
    async fn rekey_inbound_sets_is_sensitive_from_plaintext() {
        use base64::Engine as _;
        use copypaste_core::encrypt_for_cloud;
        use tempfile::tempdir;

        // A single pairwise key between two devices.
        let k_shared: [u8; 32] = [0x55u8; 32];
        let k_shared_b64 = base64::engine::general_purpose::STANDARD.encode(k_shared);
        let fp_a = "aa:aa";
        let seed_b = [0x22u8; 32];

        let dir = tempdir().unwrap();
        let peers_path = dir.path().join("peers.json");
        std::fs::write(
            &peers_path,
            format!(
                r#"[{{"fingerprint":"{fp_a}","added_at":1,"address":"127.0.0.1:9","sync_key_b64":"{k_shared_b64}"}}]"#
            ),
        )
        .unwrap();
        let crypto = SyncCrypto::new(seed_b, peers_path);

        let db = make_db();
        let key_sync = copypaste_core::SyncKey::from_bytes(k_shared);

        // ── Test 1: sensitive plaintext (AWS key) → is_sensitive = true ──────
        let item_id_sensitive = "kcf-sensitive".to_string();
        // A real AWS access key triggers the detector at confidence 0.99.
        let sensitive_plaintext = "AKIAIOSFODNN7EXAMPLE";
        let blob_sensitive =
            encrypt_for_cloud(&key_sync, &item_id_sensitive, sensitive_plaintext.as_bytes())
                .expect("encrypt sensitive");

        let wire_sensitive = WireItem {
            deleted: false,
            pinned: false,
            pin_order: None,
            id: "kcf-row-sens".to_string(),
            item_id: item_id_sensitive.clone(),
            content_type: "text".to_string(),
            content: Some(blob_sensitive),
            content_nonce: None, // sync-key-wrapped
            blob_ref: None,
            is_sensitive: false, // sender's flag — must be overridden by receiver
            lamport_ts: 1,
            wall_time: 1_700_000_000_001,
            expires_at: None,
            app_bundle_id: None,
            origin_device_id: "device-A".to_string(),
            key_version: 2,
            file_name: None,
            mime: None,
        };

        let quota = copypaste_core::AppConfig::default().storage_quota_bytes as i64;
        merge_incoming_with_crypto(&db, vec![wire_sensitive], Some(&crypto), quota, None)
            .await
            .expect("merge sensitive item");

        {
            let g = db.lock().await;
            let rows = copypaste_core::get_page(&*g, 10, 0).expect("get_page");
            assert_eq!(rows.len(), 1, "sensitive item must be stored");
            assert!(
                rows[0].is_sensitive,
                "inbound sensitive item must have is_sensitive=true (CopyPaste-kcf fix); got false"
            );
        }

        // ── Test 2: non-sensitive plaintext → is_sensitive = false ───────────
        let item_id_plain = "kcf-plain".to_string();
        let plain_text = "hello world, nothing secret here";
        let blob_plain =
            encrypt_for_cloud(&key_sync, &item_id_plain, plain_text.as_bytes())
                .expect("encrypt plain");

        let wire_plain = WireItem {
            deleted: false,
            pinned: false,
            pin_order: None,
            id: "kcf-row-plain".to_string(),
            item_id: item_id_plain.clone(),
            content_type: "text".to_string(),
            content: Some(blob_plain),
            content_nonce: None,
            blob_ref: None,
            is_sensitive: false,
            lamport_ts: 2,
            wall_time: 1_700_000_000_002,
            expires_at: None,
            app_bundle_id: None,
            origin_device_id: "device-A".to_string(),
            key_version: 2,
            file_name: None,
            mime: None,
        };

        merge_incoming_with_crypto(&db, vec![wire_plain], Some(&crypto), quota, None)
            .await
            .expect("merge plain item");

        {
            let g = db.lock().await;
            let rows = copypaste_core::get_page(&*g, 10, 0).expect("get_page");
            assert_eq!(rows.len(), 2, "both items must be stored");
            // The plain item should have is_sensitive=false.
            let plain_row = rows
                .iter()
                .find(|r| r.item_id == item_id_plain)
                .expect("plain item must be in DB");
            assert!(
                !plain_row.is_sensitive,
                "non-sensitive inbound item must have is_sensitive=false"
            );
        }
    }
}
