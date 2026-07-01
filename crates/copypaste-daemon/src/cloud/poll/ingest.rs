use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

use copypaste_core::storage::items::soft_delete_item;
use copypaste_core::{
    decrypt_from_cloud, exists_item_by_item_id, get_item_by_item_id, insert_item, insert_tombstone,
    prune_to_cap, Database, SyncKey,
};
use copypaste_supabase::auth::AuthClient;
use copypaste_sync::merge::{remote_wins, RemoteMeta};

use crate::sync_common::{build_local_item, decode_payload_ct, replace_cloud_item_by_item_id};

use super::super::config::CloudConfig;
use super::cursor::{build_poll_url, save_poll_watermark, PollCursor};
use super::transport::fetch_remote_rows_with_refresh;
use super::POLL_BATCH_SIZE;

/// Outcome of ingesting a single row from a poll batch (see [`ingest_row`]).
enum RowOutcome {
    /// The row had no usable `id`/`item_id` and was skipped entirely — the
    /// batch cursor must NOT advance for it.
    Unparseable,
    /// The row was read successfully (the batch cursor should advance to
    /// `(wall, id)`) whether or not it resulted in a local write. `synced`
    /// indicates whether a local DB write happened for this row.
    Ingested { wall: i64, id: String, synced: bool },
}

/// Ingest a single raw JSON row from a poll batch: dedup/LWW-resolve against
/// any existing local row (keyed on the cross-device `item_id`), apply the
/// tombstone fast-path for a soft-deleted remote row, or decode/decrypt/
/// re-encrypt/write a live item.
///
/// Extracted from `poll_once`'s per-row loop body (CopyPaste-vp63.28) so the
/// deeply nested dedup/tombstone/decrypt pipeline has a name and a return
/// value instead of being inlined in a `for` loop that also owns batch-cursor
/// bookkeeping. Behavior is unchanged: every `continue` in the original loop
/// maps 1:1 to a `RowOutcome` variant here, and the caller applies the exact
/// same `batch_max`/`synced` bookkeeping the inline loop used to.
fn ingest_row(
    row: &serde_json::Value,
    db_guard: &Database,
    sync_key: &SyncKey,
    local_key: &Arc<zeroize::Zeroizing<[u8; 32]>>,
) -> RowOutcome {
    let Some(id) = row["id"].as_str() else {
        return RowOutcome::Unparseable;
    };
    let Some(item_id) = row["item_id"].as_str() else {
        return RowOutcome::Unparseable;
    };
    let row_wall = row["wall_time"].as_i64().unwrap_or(0);

    // LWW dedup keyed on the cross-device `item_id` (NOT the per-row
    // `id`, which differs across devices for the same logical item). If
    // the item is already present locally, route it through an LWW
    // resolve instead of inserting a duplicate or unconditionally
    // dropping it: a strictly-newer remote `lamport_ts` must win so a
    // cloud edit propagates, while an older/equal one is skipped.
    let existing = match get_item_by_item_id(db_guard, item_id) {
        Ok(row) => row,
        Err(e) => {
            tracing::warn!("cloud-sync: get_item_by_item_id error for item_id={item_id}: {e}");
            return RowOutcome::Ingested {
                wall: row_wall,
                id: id.to_owned(),
                synced: false,
            };
        }
    };
    // Decode the remote total-order sort keys up front so the LWW
    // decision and the tombstone paths share one source of truth.
    let remote_lamport = row["lamport_ts"].as_i64().unwrap_or(0);
    let remote_origin = row["device_id"].as_str().unwrap_or("");
    let remote_deleted = row["deleted"].as_bool().unwrap_or(false);

    let preserved_pk = if let Some(local) = existing.as_ref() {
        // CopyPaste-ayvs: use the SAME total order as P2P (lamport ->
        // wall_time -> origin_device_id) instead of the old bare
        // `remote_lamport <= local -> keep`, which on EQUAL lamport
        // always kept local and never converged across transports.
        let wins = remote_wins(
            local.lamport_ts,
            local.wall_time,
            &local.origin_device_id,
            &RemoteMeta {
                lamport_ts: remote_lamport,
                wall_time: row_wall,
                origin_device_id: remote_origin,
            },
        );
        if !wins {
            // Local copy wins LWW — skip.
            return RowOutcome::Ingested {
                wall: row_wall,
                id: id.to_owned(),
                synced: false,
            };
        }
        // Remote wins LWW: replace in place, preserving the local PK so
        // FTS / copy_item / pins keep pointing at the same row.
        Some(local.id.clone())
    } else {
        // Defensive: also honour a same-`id` row that somehow lacks the
        // matching item_id (legacy rows) so we never double-insert.
        match exists_item_by_item_id(db_guard, item_id) {
            Ok(true) => {
                return RowOutcome::Ingested {
                    wall: row_wall,
                    id: id.to_owned(),
                    synced: false,
                }
            }
            Ok(false) => None,
            Err(e) => {
                tracing::warn!(
                    "cloud-sync: exists_item_by_item_id error for item_id={item_id}: {e}"
                );
                return RowOutcome::Ingested {
                    wall: row_wall,
                    id: id.to_owned(),
                    synced: false,
                };
            }
        }
    };

    // ── Tombstone fast-path ──────────────────────────────────────────
    // If the remote row carries `deleted = true` the remote device has
    // soft-deleted this item. Apply the deletion locally as a tombstone
    // (soft-delete: wipe content, set deleted=1, propagate via LWW) so
    // the item cannot resurrect on this device or re-broadcast incorrectly.
    // The cursor still advances (batch_max was updated by the caller) so
    // tombstones are never re-requested.
    if remote_deleted {
        let remote_wall = row_wall;
        let mut synced = false;
        if let Some(local_pk) = preserved_pk.as_ref() {
            match soft_delete_item(db_guard, local_pk, remote_lamport, remote_wall) {
                Ok(n) if n > 0 => {
                    synced = true;
                    tracing::info!(
                        "cloud-sync poll_once: applied tombstone for \
                         item_id={item_id} (soft-deleted {n} local row(s))"
                    );
                }
                Ok(_) => {
                    tracing::debug!(
                        "cloud-sync poll_once: tombstone for item_id={item_id} \
                         but row was already absent locally"
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        "cloud-sync poll_once: soft_delete_item failed for \
                         item_id={item_id}: {e}"
                    );
                }
            }
        } else {
            // CopyPaste-bfiu: the item is UNKNOWN locally (delete arrived
            // before the create). Persist a tombstone row so a later
            // out-of-order create loses LWW instead of resurrecting it.
            match insert_tombstone(
                db_guard,
                item_id,
                item_id,
                remote_lamport,
                remote_wall,
                remote_origin,
            ) {
                Ok(_) => {
                    synced = true;
                    tracing::info!(
                        "cloud-sync poll_once: inserted tombstone for unknown \
                         item_id={item_id} (delete-before-create)"
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        "cloud-sync poll_once: insert_tombstone failed for \
                         item_id={item_id}: {e}"
                    );
                }
            }
        }
        // Either soft-deleted / tombstoned / already absent — skip decode.
        return RowOutcome::Ingested {
            wall: row_wall,
            id: id.to_owned(),
            synced,
        };
    }

    // Decode payload_ct (base64 → bytes).
    let payload_ct_b64 = match row["payload_ct"].as_str() {
        Some(s) => s,
        None => {
            tracing::warn!("cloud-sync: row id={id} missing payload_ct; skipping");
            return RowOutcome::Ingested {
                wall: row_wall,
                id: id.to_owned(),
                synced: false,
            };
        }
    };
    let blob = match decode_payload_ct(payload_ct_b64) {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!("cloud-sync: payload_ct decode failed for id={id}: {e}; skipping");
            return RowOutcome::Ingested {
                wall: row_wall,
                id: id.to_owned(),
                synced: false,
            };
        }
    };

    // Decrypt with the single per-account sync key (AAD = item_id +
    // schema v5). On failure: skip, warn, NEVER log the blob or key. A
    // failure means a wrong passphrase/account or a tampered blob.
    let plaintext = match decrypt_from_cloud(sync_key, item_id, &blob) {
        Ok(p) => p,
        Err(e) => {
            // Never log plaintext or the key.
            tracing::warn!(
                "cloud-sync: decrypt failed for id={id} \
                 (wrong passphrase or tampered blob): {e}; skipping"
            );
            return RowOutcome::Ingested {
                wall: row_wall,
                id: id.to_owned(),
                synced: false,
            };
        }
    };

    // Re-encrypt with local key (v2 HKDF path).
    // [P2 audit fix] warn on missing/unexpected field values so
    // silent fallbacks are diagnosable without changing control flow.
    let content_type = row["content_type"]
        .as_str()
        .unwrap_or_else(|| {
            tracing::warn!(
                "cloud-sync poll_once: id={id} missing content_type; defaulting to \"text\""
            );
            "text"
        })
        .to_owned();
    let lamport_ts = row["lamport_ts"].as_i64().unwrap_or_else(|| {
        tracing::warn!("cloud-sync poll_once: id={id} missing lamport_ts; defaulting to 0");
        0
    });
    let wall_time = row_wall;
    let expires_at = row["expires_at"].as_i64();
    let app_bundle_id = row["app_bundle_id"].as_str().map(str::to_owned);
    let origin_device_id =
        row["device_id"]
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| {
                tracing::warn!(
                    "cloud-sync poll_once: id={id} missing device_id; defaulting to empty"
                );
                String::new()
            });

    // Read cloud pin state. These are sourced from the real columns now
    // (schema v10+), so the previous OR-merge workaround is replaced by
    // direct use of the authoritative cloud values.
    let cloud_pinned = row["pinned"].as_bool().unwrap_or(false);
    let cloud_pin_order = row["pin_order"].as_f64();

    let mut local_item = match build_local_item(
        id,
        item_id,
        &content_type,
        &plaintext,
        lamport_ts,
        wall_time,
        expires_at,
        app_bundle_id,
        origin_device_id,
        local_key,
    ) {
        Ok(i) => i,
        Err(e) => {
            tracing::warn!("cloud-sync: local re-encrypt failed for id={id}: {e}; skipping");
            return RowOutcome::Ingested {
                wall: row_wall,
                id: id.to_owned(),
                synced: false,
            };
        }
    };

    // For an LWW replace, preserve the existing local row's primary key
    // so FTS / copy_item / pins keep pointing at the same row (do NOT
    // adopt the remote's `id`).
    if let Some(pk) = preserved_pk.as_ref() {
        local_item.id = pk.clone();
    }

    // Apply cloud pin state. The cloud columns are now authoritative:
    // a pin/unpin on the originating device is propagated here.
    // If the cloud row pre-dates the pin columns (both absent/null) we
    // fall back to preserving the existing local state so a pinned item
    // does not lose its pin-exemption on a schema-skew roundtrip.
    let cloud_carries_pin = row.get("pinned").is_some();
    if cloud_carries_pin {
        local_item.pinned = cloud_pinned;
        local_item.pin_order = cloud_pin_order;
    } else if let Some(local) = existing.as_ref() {
        // Legacy row (no pin columns) — preserve existing local state.
        local_item.pinned = local_item.pinned || local.pinned;
        if local_item.pin_order.is_none() {
            local_item.pin_order = local.pin_order;
        }
    }

    let write_res = if preserved_pk.is_some() {
        // Replace the prior version atomically (delete by item_id +
        // re-insert with the preserved PK). Cloud items are text-only
        // here, so no FTS plaintext is threaded through; the FTS rewrite
        // happens lazily on read paths that already rebuild it.
        replace_cloud_item_by_item_id(db_guard, &local_item)
    } else {
        insert_item(db_guard, &local_item).map_err(anyhow::Error::from)
    };
    let synced = match write_res {
        Ok(()) => {
            tracing::info!(
                "cloud-sync: synced remote item_id={} (id={})",
                local_item.item_id,
                local_item.id
            );
            true
        }
        Err(e) => {
            tracing::warn!(
                "cloud-sync: failed to store remote item_id={}: {e}",
                local_item.item_id
            );
            false
        }
    };
    RowOutcome::Ingested {
        wall: row_wall,
        id: id.to_owned(),
        synced,
    }
}

/// Execute a single poll round and return the (possibly advanced) cursor.
///
/// 1. Build the poll URL with a `(wall_time, id)` keyset cursor ordered
///    `wall_time.asc, id.asc` so PostgREST returns the OLDEST `limit` rows after
///    everything ingested so far (forward pagination). The compound cursor
///    prevents the same-millisecond-burst data loss the old `wall_time`-only
///    `gt` cursor suffered (see [`super::cursor::build_poll_url`]).
/// 2. For each row, dedup/LWW by the cross-device `item_id` via [`ingest_row`]:
///    a brand-new item is inserted; an item already present locally is routed
///    through an LWW resolve (newer `lamport_ts` wins) and, on a win, replaced
///    in place while the local primary key is preserved.
/// 3. Advance the cursor to the `(wall_time, id)` of the last row seen in the
///    batch (including de-duped / undecryptable rows, so they are never
///    re-requested) and persist the wall component so a restart resumes forward.
///
/// On a fetch error the cursor is returned unchanged so the next tick retries
/// the same window.
// poll_once parameters: client, config, bearer, db, cursor, signed_in,
// ingest_tx, and last_sync_ms — each an independent runtime slice.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn poll_once(
    client: &reqwest::Client,
    config: &CloudConfig,
    bearer: &Arc<RwLock<String>>,
    db: &Arc<Mutex<Database>>,
    local_key: &Arc<zeroize::Zeroizing<[u8; 32]>>,
    last_sync_ms: &Arc<std::sync::atomic::AtomicI64>,
    cloud_signed_in: &Arc<std::sync::atomic::AtomicBool>,
    auth: &AuthClient,
    key_bytes: &[u8; 32],
    cursor: PollCursor,
    // Retention limit threaded from `AppConfig` so a long-offline device
    // converges to the cap after backfill instead of materialising unbounded rows.
    storage_quota_bytes: u64,
) -> (PollCursor, usize) {
    // Compile-time guard: POLL_SELECT_QS embeds a numeric `limit=` that MUST
    // match POLL_BATCH_SIZE. If this assert fires, update the limit= in
    // POLL_SELECT_QS to match POLL_BATCH_SIZE.
    const _: () = assert!(
        POLL_BATCH_SIZE == 20,
        "POLL_SELECT_QS limit= must match POLL_BATCH_SIZE"
    );

    let poll_url = build_poll_url(&config.supabase_url, cursor.wall, &cursor.id);

    let rows = match fetch_remote_rows_with_refresh(
        client,
        &poll_url,
        config,
        bearer,
        cloud_signed_in,
        auth,
    )
    .await
    {
        Ok(rows) => rows,
        Err(e) => {
            tracing::warn!("cloud-sync poll failed: {e}");
            return (cursor, 0);
        }
    };
    // Track raw row count BEFORE blocking processing for burst-drain detection.
    let batch_len = rows.len();

    // Decrypt + re-encrypt + insert in a blocking task so the async executor is
    // not blocked by rusqlite IO. We snapshot the single per-account key bytes
    // (non-secret from the perspective of the blocking thread, but never logged)
    // and move them into the closure to be zeroized after use.
    let db_arc = db.clone();
    let local_key_clone = local_key.clone();
    let mut sync_key_bytes: [u8; 32] = *key_bytes;
    let start_cursor = cursor.clone();
    let join = tokio::task::spawn_blocking(move || {
        let db_guard = db_arc.blocking_lock();
        // Reconstruct the SyncKey once (not per row). `SyncKey` is
        // `ZeroizeOnDrop`, so it is scrubbed when it drops at the end of the
        // closure; the raw `sync_key_bytes` array is zeroized explicitly below.
        let sync_key = SyncKey::from_bytes(sync_key_bytes);
        let mut synced_count = 0u32;
        // Highest `(wall_time, id)` observed in this batch — used to advance the
        // forward cursor even for rows that were de-duped or failed to decrypt,
        // so we never re-request them on the next tick. Ordering matches the
        // query's `(wall_time, id)` sort.
        let mut batch_max: (i64, String) = (start_cursor.wall, start_cursor.id.clone());
        for row in rows {
            match ingest_row(&row, &db_guard, &sync_key, &local_key_clone) {
                RowOutcome::Unparseable => continue,
                RowOutcome::Ingested { wall, id, synced } => {
                    // Advance the batch cursor for EVERY row we can read —
                    // including ones we skip (already present, undecryptable)
                    // — so the next poll's keyset filter does not re-request
                    // them.
                    if (wall, id.clone()) > batch_max {
                        batch_max = (wall, id);
                    }
                    if synced {
                        synced_count += 1;
                    }
                }
            }
        }
        // Zero the snapshot key bytes before the closure exits. `sync_key`
        // (the SyncKey) zeroizes itself on drop; scrub the raw array too.
        zeroize::Zeroize::zeroize(&mut sync_key_bytes);
        // ── Backfill safety: enforce local retention cap after ingest ─────────
        //
        // After writing all rows from this batch, prune oldest UNPINNED items so
        // the local DB stays within the configured byte cap. This prevents a
        // long-offline device from materialising thousands of cloud rows
        // unbounded on reconnect (each poll tick adds up to 20 rows).
        //
        // Count-based (`history_limit`) pruning was removed: `prune_to_cap`
        // against `storage_quota_bytes` is the single authoritative retention
        // policy.
        //
        // The cloud watermark (persisted below) tracks the highest cloud row
        // seen and is stored in the `settings` table — completely independent of
        // the `clipboard_items` rows we are pruning here. Evicting old local rows
        // does NOT move the watermark backwards: next tick the cursor still
        // advances from the cloud side. Cloud still holds the older items; only
        // the local cache is capped.
        if synced_count > 0 {
            // Byte cap: window-function prune via core API (takes i64 max_bytes).
            // `storage_quota_bytes` is u64 from AppConfig; saturating cast to i64
            // keeps the value in range (i64::MAX ≈ 9.2 EB, far beyond any real quota).
            let max_bytes = storage_quota_bytes.min(i64::MAX as u64) as i64;
            match prune_to_cap(&db_guard, max_bytes) {
                Ok(0) => {}
                Ok(n) => tracing::debug!(
                    "cloud-sync poll_once: byte-pruned {n} rows after batch ingest \
                     (quota_bytes={storage_quota_bytes})"
                ),
                Err(e) => tracing::warn!("cloud-sync poll_once: prune_to_cap failed: {e}"),
            }
        }

        // Persist the advanced wall watermark inside the same DB lock so it
        // survives a restart. Return the full `(wall, id)` cursor the async loop
        // should use going forward.
        let new_wall = batch_max.0;
        if new_wall > start_cursor.wall {
            if let Err(e) = save_poll_watermark(&db_guard, new_wall) {
                tracing::warn!("cloud-sync: failed to persist poll watermark {new_wall}: {e}");
            }
        }
        let new_cursor = PollCursor {
            wall: batch_max.0,
            id: batch_max.1,
        };
        (synced_count, new_cursor)
    });

    match join.await {
        Ok((synced_count, new_cursor)) => {
            if synced_count > 0 {
                // Record the wall-clock time of the last successful sync.
                let now_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as i64;
                last_sync_ms.store(now_ms, Ordering::Relaxed);
            }
            // Advance the in-memory cursor so the next tick's URL keyset-filters
            // past everything we just saw. `new_cursor` is monotonically ≥ the
            // start cursor (batch_max seeds from it), so it never regresses.
            (new_cursor, batch_len)
        }
        Err(e) => {
            tracing::warn!("cloud-sync: insert worker panicked or was cancelled: {e}");
            (cursor, 0)
        }
    }
}
