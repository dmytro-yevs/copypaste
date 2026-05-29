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
use std::sync::Arc;

use tokio::sync::{broadcast, mpsc, Mutex};
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use copypaste_core::{
    build_item_aad_v2, decrypt_from_cloud, decrypt_item_by_version, derive_v2, encrypt_for_cloud,
    encrypt_item_with_aad, ClipboardItem, Database, MigrationState, SyncKey, AAD_SCHEMA_VERSION_V4,
    ITEM_KEY_VERSION_CURRENT, NONCE_SIZE,
};
use copypaste_sync::{
    merge::{local_to_wire, resolve, wire_to_local, MergeOutcome},
    protocol::WireItem,
};

/// Cross-device content-key context for the sync orchestrator (P2P Phase 3).
///
/// Items are stored at rest encrypted under this device's *per-device*
/// local-storage key, so the on-wire ciphertext is undecryptable by any other
/// device. To make a synced item readable on a paired peer we re-key it through
/// a **shared content sync key** established at pairing (derived deterministically
/// from the PAKE session key — both peers hold the identical key):
///
/// * **outbound** — decrypt the row's ciphertext with the local key, then
///   re-encrypt the plaintext under the shared sync key
///   (`encrypt_for_cloud`, XChaCha20-Poly1305 + per-item-id AAD). The wire
///   item carries that blob with `content_nonce = None` (the cloud blob is
///   self-framed: it prefixes its own 24-byte nonce).
/// * **inbound** — decrypt the wire blob with the shared sync key, then
///   re-encrypt the plaintext under THIS device's local v2 key before storing,
///   and index the plaintext into FTS so search + previews work for synced rows.
///
/// When no shared key is available (P2P disabled, or a legacy peer record with
/// no `sync_key_b64`) the orchestrator falls back to the legacy behaviour:
/// outgoing items ship their raw at-rest ciphertext (undecryptable on the peer,
/// exactly as before Phase 3) and incoming items are stored verbatim.
#[derive(Clone)]
pub struct SyncCrypto {
    /// This device's v1 local-storage key (the raw seed from `load_local_key`).
    v1_key: [u8; 32],
    /// This device's v2 local-storage key (`derive_v2(seed)`).
    v2_key: [u8; 32],
    /// Path to `peers.json`, re-read on each crypto operation so a peer paired
    /// at runtime contributes its shared sync key without a restart.
    peers_path: PathBuf,
}

impl SyncCrypto {
    /// Build a crypto context from the device's local-storage seed and the
    /// `peers.json` path.
    pub fn new(local_seed: [u8; 32], peers_path: PathBuf) -> Self {
        Self {
            v1_key: local_seed,
            v2_key: derive_v2(&local_seed),
            peers_path,
        }
    }

    /// Load the shared content sync key (if any) from `peers.json`.
    ///
    /// Returns the first peer record that carries a valid `sync_key_b64`. The
    /// supported topology is two paired devices sharing one key; with >2 devices
    /// a common group key would be required (deferred — see module notes).
    fn shared_sync_key(&self) -> Option<SyncKey> {
        use base64::Engine as _;
        let peers = crate::peers::load_peers(&self.peers_path);
        for dev in &peers {
            let Some(b64) = dev.sync_key_b64.as_deref() else {
                continue;
            };
            let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(b64) else {
                continue;
            };
            if let Ok(arr) = <[u8; 32]>::try_from(bytes.as_slice()) {
                return Some(SyncKey::from_bytes(arr));
            }
        }
        None
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
/// * `shutdown` — D2: token cancelled by the daemon on SIGINT/SIGTERM so the
///   orchestrator exits promptly instead of waiting for channels to drain.
///
/// Returns `Ok(())` once both channels close or `shutdown` fires.
pub async fn run(
    db: Arc<Mutex<Database>>,
    mut new_item_rx: broadcast::Receiver<ClipboardItem>,
    mut incoming_rx: mpsc::Receiver<WireItem>,
    outbound_tx: mpsc::Sender<WireItem>,
    device_id: String,
    crypto: Option<SyncCrypto>,
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
                        let mut wire = local_to_wire(&item, &device_id);
                        // P2P Phase 3: re-key the payload under the shared sync
                        // key so a paired peer can decrypt it. Falls back to the
                        // raw at-rest ciphertext when no shared key is available.
                        if let Some(ref crypto) = crypto {
                            // sync H2: if a shared key is present but re-keying
                            // fails, DROP the item — forwarding raw at-rest
                            // ciphertext would land a permanently-undecryptable
                            // row on the peer that it counts as synced.
                            if rekey_outbound(crypto, &mut wire) == RekeyOutcome::Failed {
                                warn!(
                                    item_id = %wire.id,
                                    "sync_orch: dropping local item — re-key failed (would be undecryptable on peer)"
                                );
                                continue;
                            }
                        }
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
                        if let Err(e) = merge_incoming_with_crypto(&db, vec![wire], crypto.as_ref()).await {
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
pub async fn merge_incoming(
    db: &Arc<Mutex<Database>>,
    items: Vec<WireItem>,
) -> anyhow::Result<usize> {
    merge_incoming_with_crypto(db, items, None).await
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
pub async fn merge_incoming_with_crypto(
    db: &Arc<Mutex<Database>>,
    items: Vec<WireItem>,
    crypto: Option<&SyncCrypto>,
) -> anyhow::Result<usize> {
    if items.is_empty() {
        return Ok(0);
    }

    let db_guard = db.lock().await;

    let mut upserted = 0usize;
    for wire in items {
        // M3: look up the specific row by id rather than snapshotting a capped
        // page. A `get_page(.., 10_000, 0)` snapshot misses rows past row 10k,
        // so an incoming update for such an item would be treated as new and
        // then lost on the PK conflict at insert time.
        let existing = match copypaste_core::get_item_by_id(&db_guard, &wire.id) {
            Ok(row) => row,
            Err(e) => {
                warn!(item_id = %wire.id, "sync_orch: get_item_by_id failed: {e}");
                continue;
            }
        };
        let exists = existing.is_some();
        let take_remote = match existing.as_ref() {
            Some(local) => matches!(resolve(local, &wire), MergeOutcome::TakeRemote),
            None => true,
        };

        if !take_remote {
            debug!(item_id = %wire.id, "sync_orch: LWW kept local");
            continue;
        }

        // P2P Phase 3: unwrap the shared-key payload into a row encrypted under
        // this device's own local key, recovering the plaintext for FTS. Returns
        // the row to insert plus the decrypted plaintext (when text) to index.
        let (to_insert, fts_plaintext) = match crypto {
            Some(c) => match rekey_inbound(c, wire) {
                Ok(pair) => pair,
                Err(w) => {
                    // Not sync-key-wrapped (or undecryptable): store verbatim.
                    (wire_to_local(*w), None)
                }
            },
            None => (wire_to_local(wire), None),
        };

        // M1: make the delete-then-insert (plus FTS) ATOMIC. The previous code
        // ran `delete_item` then a separate `insert_item`; if the insert failed
        // the row was lost. We wrap delete + insert + FTS in a single
        // transaction so a failed insert rolls back the delete and leaves the
        // old row (and its FTS entry) intact. Mirrors `insert_item_with_fts`'s
        // `unchecked_transaction` approach (we can't reuse it directly because
        // it does plain INSERT with dedup-on-conflict rather than replace).
        let fts_text = fts_plaintext.and_then(|pt| String::from_utf8(pt).ok());
        match replace_item_atomic(&db_guard, exists, &to_insert, fts_text.as_deref()) {
            Ok(()) => {
                debug!(item_id = %to_insert.id, "sync_orch: upserted incoming item");
                upserted += 1;
            }
            Err(e) => warn!(item_id = %to_insert.id, "sync_orch: atomic replace failed: {e}"),
        }
    }
    Ok(upserted)
}

/// Atomically replace (or insert) a clipboard row and its FTS index for the
/// sync merge path (sync M1).
///
/// Runs DELETE (when `existed`) + INSERT + FTS rewrite inside one
/// `unchecked_transaction`, so a failed insert rolls the whole thing back and
/// the prior row survives intact. Unlike `insert_item` / `insert_item_with_fts`
/// in core (plain INSERT, dedup-on-conflict), this path is a true replace keyed
/// on the primary key `id`, which is what LWW `TakeRemote` requires.
///
/// `fts_text` is the already-decrypted plaintext to index; `None`/empty skips
/// FTS (e.g. verbatim or image rows). The stored `key_version` mirrors what the
/// non-atomic path wrote (`insert_item` stamps the current item key version).
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
        tx.execute(
            "DELETE FROM clipboard_items WHERE id = ?1",
            params![item.id],
        )?;
    }
    tx.execute(
        "INSERT INTO clipboard_items
         (id, item_id, content_type, content, content_nonce, blob_ref,
          is_sensitive, is_synced, lamport_ts, wall_time, expires_at, app_bundle_id,
          content_hash, origin_device_id, key_version, pinned)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16)",
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
            ITEM_KEY_VERSION_CURRENT,
            item.pinned as i64,
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

/// Build the set of local items to push to a peer that has just connected
/// (P2P Phase 3 "sync on connect" / catch-up).
///
/// Fanout is fire-and-forget to *currently* connected sinks, so an item
/// captured/imported before the mTLS link came up would otherwise never reach
/// the peer (and the both-sides-dial race makes the exact connect instant
/// non-deterministic). When a connection is established we therefore replay the
/// full local history to it once: each row is converted to a wire item and
/// re-keyed under the shared sync key so the peer can decrypt it. LWW on the
/// receiver makes the replay idempotent (already-present items lose or no-op).
///
/// Returns an empty vec when there is no shared sync key (nothing decryptable to
/// send) or the DB read fails — catch-up is best-effort.
pub fn catchup_items(db: &Database, device_id: &str, crypto: &SyncCrypto) -> Vec<WireItem> {
    let local: Vec<ClipboardItem> = match copypaste_core::get_page(db, 10_000, 0) {
        Ok(rows) => rows,
        Err(e) => {
            warn!("sync_orch: catchup get_page failed: {e}");
            return Vec::new();
        }
    };
    let mut out = Vec::with_capacity(local.len());
    for item in &local {
        let mut wire = local_to_wire(item, device_id);
        // Only forward items we could actually re-key under the shared key; a
        // still-locally-encrypted (NotApplicable) or failed payload is useless
        // — or worse, undecryptable — to the peer (sync H2).
        if rekey_outbound(crypto, &mut wire) == RekeyOutcome::Rewrapped {
            out.push(wire);
        }
    }
    out
}

/// Outcome of an attempt to re-key an outgoing item under the shared sync key.
#[derive(Debug, PartialEq, Eq)]
enum RekeyOutcome {
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
/// Only text items are re-keyed; image chunk blobs use a separate per-chunk
/// scheme (deferred).
fn rekey_outbound(crypto: &SyncCrypto, wire: &mut WireItem) -> RekeyOutcome {
    if wire.content_type != "text" {
        return RekeyOutcome::NotApplicable;
    }
    let Some(shared) = crypto.shared_sync_key() else {
        return RekeyOutcome::NotApplicable;
    };
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

    match encrypt_for_cloud(&shared, &wire.item_id, &plaintext) {
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
    // Marker: a sync-key-wrapped text payload carries content but no nonce.
    if wire.content_type != "text" || wire.content_nonce.is_some() || wire.content.is_none() {
        return Err(Box::new(wire));
    }
    let Some(shared) = crypto.shared_sync_key() else {
        return Err(Box::new(wire));
    };
    let blob = match wire.content.as_ref() {
        Some(b) => b.clone(),
        None => return Err(Box::new(wire)),
    };
    let plaintext = match decrypt_from_cloud(&shared, &wire.item_id, &blob) {
        Ok(pt) => pt,
        Err(e) => {
            warn!(item_id = %wire.item_id, "sync_orch: rekey_inbound shared-decrypt failed: {e}");
            return Err(Box::new(wire));
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
        let rows = copypaste_core::get_page(&db_guard, 10, 0).expect("get_page");
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

    /// LWW: a stale wire item (lower lamport) must NOT overwrite the local row.
    #[tokio::test]
    async fn merge_incoming_keeps_local_on_older_remote() {
        let db = make_db();
        // Pre-insert a local row with a higher lamport clock.
        let mut local = ClipboardItem::new_text(vec![0x11], vec![0u8; 24], 50);
        local.id = "shared".to_string();
        {
            let g = db.lock().await;
            insert_item(&g, &local).unwrap();
        }

        let wire = make_wire("shared", 5, 0xFF); // older
        let upserted = merge_incoming(&db, vec![wire]).await.unwrap();
        assert_eq!(upserted, 0, "older remote must lose LWW");

        let g = db.lock().await;
        let rows = copypaste_core::get_page(&g, 10, 0).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].content, Some(vec![0x11]), "local payload preserved");
    }
}
