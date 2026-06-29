// l07l: AtomicI64/Ordering are only exercised by the macOS pasteboard
// change-count path; allow them unused on non-macOS so -D warnings stays green.
#[cfg_attr(not(target_os = "macos"), allow(unused_imports))]
use anyhow::Context as _; // CopyPaste-crh3.90
use std::sync::atomic::AtomicI64;
use std::sync::Arc;

use tokio::sync::Mutex;

use copypaste_core::{
    is_sensitive_for_autowipe, prune_to_cap, ClipboardItem, Database, MigrationState,
};
use copypaste_sync::{
    merge::{resolve, wire_to_local, MergeOutcome},
    protocol::WireItem,
};
use tracing::{debug, warn};

use super::pasteboard::apply_to_pasteboard_if_fresh;
use super::poison::is_poison_wire;
use super::rekey::{rekey_inbound, SyncCrypto};

/// Context passed to [`merge_incoming_with_crypto`] for Universal Clipboard.
/// Re-exported here so callers only need `use super::merge::AutoApplyCtx`
/// (the actual definition lives in `rekey.rs`).
pub use super::rekey::AutoApplyCtx;

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
/// (`content_nonce == None`, see `rekey_outbound`), the wire blob is
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

    // A staged item ready for the CPU re-key step (runs with the DB lock
    // released so PNG decode+re-encode does not stall other DB writers).
    struct PendingRekey {
        wire: WireItem,
        local_pk: Option<String>,
        exists: bool,
        wall_time: i64,
        content_type: String,
    }

    let result = tokio::task::spawn_blocking(move || {
        // ── Phase 1: LWW resolution + tombstone writes (DB lock held) ────────
        //
        // We acquire the lock, resolve LWW for every item, and immediately write
        // tombstones (they need no CPU re-key work).  Non-tombstone winners are
        // collected into `pending` so the CPU-heavy re-key step (image
        // decode+re-encode inside `rekey_inbound`) can run AFTER we release the
        // lock.  Holding the DB mutex across PNG decode was the primary cause of
        // the "tiny image blocks all DB writers" stall (Fix A).
        let (upserted_from_tombstones, pending): (usize, Vec<PendingRekey>) = {
            // Acquire the std-compatible blocking lock INSIDE the blocking closure.
            // This keeps the tokio executor free while we hold the lock and run
            // synchronous rusqlite calls (HIGH fix #2).
            let db_guard = db.blocking_lock();

            let mut tombstone_count = 0usize;
            let mut pending: Vec<PendingRekey> = Vec::with_capacity(items.len());

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
                                tombstone_count += 1;
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
                                tombstone_count += 1;
                            }
                            Err(e) => {
                                warn!(item_id = %wire.item_id, "sync_orch: insert_tombstone failed: {e}");
                            }
                        }
                    }
                    continue;
                }

                // Collect non-tombstone LWW winner for Phase 2 (re-key outside lock).
                pending.push(PendingRekey {
                    wall_time: wire.wall_time,
                    content_type: wire.content_type.clone(),
                    wire,
                    local_pk,
                    exists,
                });
            }
            // `db_guard` drops here — lock released before Phase 2 re-key work.
            (tombstone_count, pending)
        };

        // ── Phase 2: crypto re-key (no DB lock held) ─────────────────────────
        //
        // `rekey_inbound` for blobs calls `rewrap_inbound_blob` which runs a
        // full PNG pixel-decode + re-encode inside `encode_image_with_limit`.
        // This is the hot path for images and MUST run without holding the DB
        // mutex so other DB readers/writers are not stalled (Fix A).
        struct ReadyToInsert {
            to_insert: ClipboardItem,
            fts_plaintext: Option<Vec<u8>>,
            exists: bool,
            wall_time: i64,
            content_type: String,
        }

        let ready: Vec<ReadyToInsert> = pending
            .into_iter()
            .filter_map(|p| {
                let PendingRekey {
                    wire,
                    local_pk,
                    exists,
                    wall_time,
                    content_type,
                } = p;

                // P2P Phase 3: unwrap the shared-key payload into a row encrypted
                // under this device's own local key, recovering the plaintext for
                // FTS.  Returns the row to insert plus the decrypted plaintext
                // (when text) to index.
                let (mut to_insert, fts_plaintext) = match crypto_owned.as_ref() {
                    Some(c) => match rekey_inbound(c, wire) {
                        Ok(pair) => pair,
                        Err(w) => {
                            // Guard: if the item looks sync-key-wrapped but we
                            // couldn't decrypt it (shared key missing or wrong),
                            // the wire item has no content_nonce (and for
                            // file/image also no blob_ref).  Storing it verbatim
                            // creates a "poison row" that consumers reject with
                            // "missing content_nonce" / "missing blob_ref
                            // metadata". Skip it — the peer will re-send on the
                            // next catch-up cycle once the key is available.
                            // (CopyPaste-jww / CopyPaste-5y4)
                            if is_poison_wire(&w) {
                                warn!(
                                    item_id = %w.item_id,
                                    content_type = %w.content_type,
                                    "sync_orch: inbound item has no content_nonce/blob_ref \
                                     (sync-key-wrapped but undecryptable) — skipping to avoid \
                                     poison row (CopyPaste-jww/5y4)"
                                );
                                return None;
                            }
                            // Not sync-key-wrapped (or undecryptable): store
                            // verbatim.
                            (wire_to_local(*w), None)
                        }
                    },
                    None => (wire_to_local(wire), None),
                };

                // Preserve the local primary key on replace so FTS / copy_item /
                // pins (all keyed on `id`) keep pointing at the same row after the
                // update.  `wire_to_local` copies `wire.id` (the peer's PK) into
                // `to_insert.id`; we overwrite it here with the local row's PK
                // when one exists.
                if let Some(pk) = local_pk {
                    to_insert.id = pk;
                }

                // `wire_to_local` now propagates `pinned` and `pin_order` directly
                // from the wire (see merge.rs), so pin/unpin/reorder broadcasts
                // converge via normal LWW TakeRemote.  We intentionally trust the
                // wire's values here instead of OR-merging with the local state:
                // the IPC handlers bump lamport_ts before broadcasting, so the
                // wire wins LWW only when it is causally later — which is exactly
                // when its pin state should take effect.

                // CopyPaste-kcf fix: run SensitiveDetector on the decrypted
                // plaintext so inbound items get the same auto-wipe TTL as
                // locally-captured ones.  Previously `wire_to_local` always set
                // `is_sensitive = false`, meaning a password or API key synced
                // from another device bypassed TTL cleanup.  We reuse the same
                // `is_sensitive_for_autowipe` the local capture path uses
                // (daemon.rs line ~1587) — no new heuristics.  Only runs when
                // `fts_plaintext` is Some (i.e. rekey_inbound succeeded and
                // decrypted a text item); verbatim/image/file rows are left as-is
                // because we have no plaintext to inspect.
                if let Some(ref pt) = fts_plaintext {
                    if let Ok(text) = std::str::from_utf8(pt) {
                        to_insert.is_sensitive = is_sensitive_for_autowipe(text);
                    }
                }

                Some(ReadyToInsert {
                    to_insert,
                    fts_plaintext,
                    exists,
                    wall_time,
                    content_type,
                })
            })
            .collect();

        // ── Phase 3: DB writes + prune + auto-apply (DB lock re-acquired) ────
        //
        // Re-acquire the lock only for the INSERT and follow-up DB work.  The
        // expensive re-key (image decode/re-encode) is already done in Phase 2.
        let db_guard = db.blocking_lock();

        let mut upserted = upserted_from_tombstones;
        let mut apply_candidate: Option<(i64, Vec<u8>, String)> = None;

        for item in ready {
            let ReadyToInsert {
                to_insert,
                fts_plaintext,
                exists,
                wall_time: wire_wall_time,
                content_type: wire_content_type,
            } = item;

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
    .context("sync_orch: merge blocking task panicked")?;

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
