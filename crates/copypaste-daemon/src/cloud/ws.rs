use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};

use copypaste_core::storage::items::soft_delete_item;
use copypaste_core::{
    decrypt_from_cloud, exists_item_by_item_id, get_item_by_item_id, insert_item, insert_tombstone,
    prune_to_cap, Database, SyncKey,
};
use copypaste_supabase::protocol::ChangeType;
use copypaste_supabase::{RealtimeClient, RealtimeConfig};
use copypaste_sync::merge::{remote_wins, RemoteMeta};

use crate::sync_common::{build_local_item, decode_payload_ct, replace_cloud_item_by_item_id};

///
/// # Token refresh
///
/// The WS client reconnects with backoff on any disconnect.  When a reconnect
/// happens after a 401-style close (Supabase closes the WS for expired JWTs)
/// the existing `bearer` RwLock is read for the current token.  Token refresh
/// is handled by the push/poll loops' shared `AuthClient`; the WS loop simply
/// reads the latest value from `bearer` at each reconnect attempt.
///
/// # Shutdown
///
/// Listens on the shared `shutdown` Notify; calls `ClientHandle::shutdown`
/// which sends `phx_leave` + WebSocket Close before returning.
// ws_ingest_loop binds: realtime config, bearer, db, shutdown, ingest_tx,
// signed_in, and re-auth callback — independent slices across the cloud stack.
#[allow(clippy::too_many_arguments)]
pub(super) async fn ws_ingest_loop(
    config: RealtimeConfig,
    // [P0 audit fix] Shared bearer written by push/poll loops on 401-refresh.
    // Before each RealtimeClient::new we write the current token into
    // config.user_jwt so every reconnect carries the live JWT, not the
    // one captured at start_cloud time (~1 h before expiry kills the channel).
    bearer: Arc<RwLock<String>>,
    db: Arc<Mutex<Database>>,
    sync_key: Arc<Mutex<Option<SyncKey>>>,
    local_key: Arc<zeroize::Zeroizing<[u8; 32]>>,
    last_sync_ms: Arc<std::sync::atomic::AtomicI64>,
    shutdown: Arc<tokio::sync::Notify>,
    ws_connected: Arc<std::sync::atomic::AtomicBool>,
    // Live core config for hot-reload of the byte-only storage cap
    // (`storage_quota_bytes`).  Read on every prune so a runtime set_config
    // change takes effect without a restart — mirrors realtime_loop.
    core_config: Arc<std::sync::RwLock<copypaste_core::AppConfig>>,
) {
    loop {
        // Snapshot the current sync key.  If absent, back off and retry —
        // the WS events can't be decrypted without it anyway.
        let key_snapshot: Option<Vec<u8>> = {
            let guard = sync_key.lock().await;
            guard.as_ref().map(|k| k.as_bytes().to_vec())
        };
        if key_snapshot.is_none() {
            tracing::debug!("ws_ingest_loop: no sync passphrase set — waiting 30 s before retry");
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(30)) => {}
                _ = shutdown.notified() => {
                    tracing::info!("ws_ingest_loop: shutdown received (no sync key)");
                    return;
                }
            }
            continue;
        }
        let key_bytes = key_snapshot.expect("checked above");

        // [P0 audit fix] Refresh config.user_jwt from the shared bearer before
        // building the client so this reconnect uses the most-recent token.
        // The push/poll loops update `bearer` on every 401-refresh; without
        // this write the WS would reconnect with the original ~1 h JWT forever.
        {
            let current_token = bearer.read().await.clone();
            *config.user_jwt.write().await = current_token;
        }

        // Build a fresh client for this connection attempt.
        let (client, mut rx) = RealtimeClient::new(config.clone());

        let handle = match client.connect().await {
            Ok(h) => h,
            Err(e) => {
                tracing::warn!("ws_ingest_loop: connect failed: {e}; backing off 10 s");
                ws_connected.store(false, Ordering::Relaxed);
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(10)) => {}
                    _ = shutdown.notified() => {
                        tracing::info!("ws_ingest_loop: shutdown during connect backoff");
                        return;
                    }
                }
                continue;
            }
        };

        tracing::info!(
            "ws_ingest_loop: WebSocket socket open; awaiting Phoenix Channel join confirmation"
        );

        // Phase 3: gate ws_connected=true on the Phoenix Channel join being
        // confirmed (phx_reply ok), NOT merely on the TCP/WS socket opening.
        // Until the join is confirmed the channel is not yet subscribed and
        // will not deliver events, so backing the poll loop off to
        // POLL_INTERVAL_WS_CONNECTED before that point would open a window of
        // up to 60 s where clips could be missed.
        //
        // We wait with a 10 s timeout so a server that never replies to phx_join
        // (e.g. malformed credentials, network issue) still triggers a reconnect
        // rather than hanging indefinitely.  Shutdown is also handled.
        let joined_notify = handle.channel_joined();
        let join_confirmed = tokio::select! {
            biased;
            _ = shutdown.notified() => {
                tracing::info!("ws_ingest_loop: shutdown received while awaiting channel join");
                ws_connected.store(false, Ordering::Relaxed);
                handle.shutdown().await;
                return;
            }
            _ = tokio::time::sleep(Duration::from_secs(10)) => {
                tracing::warn!(
                    "ws_ingest_loop: timed out waiting for phx_reply ok (10 s); \
                     reconnecting without setting ws_connected"
                );
                false
            }
            _ = joined_notify.notified() => {
                true
            }
        };

        if join_confirmed {
            tracing::info!(
                "ws_ingest_loop: Phoenix Channel join confirmed; setting ws_connected=true"
            );
            ws_connected.store(true, Ordering::Relaxed);
        } else {
            // Join timed out — drop the handle (triggers shutdown via Drop) and retry.
            drop(handle);
            continue;
        }

        // Drain events until the channel closes (WS disconnect) or shutdown fires.
        loop {
            tokio::select! {
                biased;
                _ = shutdown.notified() => {
                    tracing::info!("ws_ingest_loop: shutdown received; closing WS");
                    ws_connected.store(false, Ordering::Relaxed);
                    handle.shutdown().await;
                    return;
                }
                maybe_event = rx.recv() => {
                    match maybe_event {
                        None => {
                            // Channel closed — WS disconnected.
                            tracing::warn!(
                                "ws_ingest_loop: event channel closed (WS disconnected); \
                                 setting ws_connected=false, will reconnect"
                            );
                            ws_connected.store(false, Ordering::Relaxed);
                            // Audit-concurrency HIGH: explicitly shut down the
                            // OLD client before the outer loop builds a fresh
                            // one. Without this, the previous `connection_loop`
                            // task kept its `running` flag set and reconnected
                            // independently — leaking one live client stack per
                            // disconnect. (`ClientHandle`'s `Drop` is the
                            // backstop, but we shut down explicitly here so the
                            // old socket/task tears down before the new connect
                            // rather than at an indeterminate later drop point.)
                            handle.shutdown().await;
                            break; // outer loop will reconnect
                        }
                        Some(event) => {
                            // Only ingest clipboard_items events (INSERT, UPDATE, DELETE).
                            // CopyPaste-44rq.66: previously only INSERT was handled; UPDATE
                            // (pin-state changes from another device) and DELETE (tombstones
                            // pushed by another device) were silently dropped.
                            if event.table != "clipboard_items" {
                                continue;
                            }
                            // DELETE events from Realtime carry old_record, not record.
                            // Route them through the tombstone path using old_record.
                            if event.change_type == ChangeType::Delete {
                                let old_row = event.old_record.as_ref().unwrap_or(&event.record);
                                let Some(item_id) = old_row["item_id"].as_str() else { continue };
                                let item_id_owned = item_id.to_owned();
                                let db_arc = db.clone();
                                let _join = tokio::task::spawn_blocking(move || {
                                    let db = db_arc.blocking_lock();
                                    // Try soft-delete first (item exists locally).
                                    // If the item is not found, it was either never
                                    // seen or already tombstoned — either is fine.
                                    let _ = db.conn().execute(
                                        "UPDATE clipboard_items SET deleted = 1, \
                                         content = NULL, content_nonce = NULL, thumb = NULL \
                                         WHERE item_id = ?1 AND deleted = 0",
                                        rusqlite::params![item_id_owned],
                                    );
                                })
                                .await;
                                continue;
                            }
                            // For INSERT and UPDATE, fall through to the existing
                            // decrypt → LWW → dedup → re-encrypt → insert path below.
                            if event.change_type != ChangeType::Insert
                                && event.change_type != ChangeType::Update
                            {
                                continue;
                            }

                            // Run the same decrypt → LWW → dedup → re-encrypt →
                            // insert → prune path as poll_once, but for a single
                            // row sourced from the WS event record.
                            let row = &event.record;
                            let Some(id) = row["id"].as_str() else { continue };
                            let Some(item_id) = row["item_id"].as_str() else { continue };

                            // Snapshot tombstone and pin state before the
                            // spawn_blocking move so the fields are owned.
                            let ws_deleted = row["deleted"].as_bool().unwrap_or(false);
                            let ws_pinned = row["pinned"].as_bool().unwrap_or(false);
                            let ws_pin_order = row["pin_order"].as_f64();
                            // Track whether the cloud row actually carries the pin
                            // columns (present → authoritative; absent → legacy
                            // schema, fall back to local-state preservation).
                            let ws_has_pin_col = row.get("pinned").is_some();

                            // Tombstone rows intentionally carry no payload.
                            // Only require payload_ct for live (non-deleted) items.
                            let blob_opt: Option<Vec<u8>> = if ws_deleted {
                                None
                            } else {
                                let Some(payload_ct_str) = row["payload_ct"].as_str() else {
                                    tracing::warn!(
                                        "ws_ingest_loop: INSERT event for id={id} missing \
                                         payload_ct; skipping"
                                    );
                                    continue;
                                };
                                match decode_payload_ct(payload_ct_str) {
                                    Ok(b) => Some(b),
                                    Err(e) => {
                                        tracing::warn!(
                                            "ws_ingest_loop: payload_ct decode failed \
                                             for id={id}: {e}; skipping"
                                        );
                                        continue;
                                    }
                                }
                            };

                            // Snapshot ingestion inputs (all cheap clones / copies).
                            let db_arc = db.clone();
                            let local_key_clone = local_key.clone();
                            let id_owned = id.to_owned();
                            let item_id_owned = item_id.to_owned();
                            // [P2 audit fix] warn on missing/unexpected field
                            // values so silent fallbacks are diagnosable.
                            let content_type = row["content_type"]
                                .as_str()
                                .unwrap_or_else(|| {
                                    tracing::warn!(
                                        "ws_ingest_loop: id={id} missing content_type; \
                                         defaulting to \"text\""
                                    );
                                    "text"
                                })
                                .to_owned();
                            let lamport_ts = row["lamport_ts"].as_i64().unwrap_or_else(|| {
                                tracing::warn!(
                                    "ws_ingest_loop: id={id} missing lamport_ts; defaulting to 0"
                                );
                                0
                            });
                            let wall_time = row["wall_time"].as_i64().unwrap_or_else(|| {
                                tracing::warn!(
                                    "ws_ingest_loop: id={id} missing wall_time; defaulting to 0"
                                );
                                0
                            });
                            let expires_at = row["expires_at"].as_i64();
                            let app_bundle_id =
                                row["app_bundle_id"].as_str().map(str::to_owned);
                            let origin_device_id = row["device_id"]
                                .as_str()
                                .map(str::to_owned)
                                .unwrap_or_else(|| {
                                    tracing::warn!(
                                        "ws_ingest_loop: id={id} missing device_id; \
                                         defaulting to empty"
                                    );
                                    String::new()
                                });

                            let mut key_arr = [0u8; 32];
                            key_arr.copy_from_slice(&key_bytes);

                            // Read the live byte cap out of the shared config and
                            // drop the std RwLock guard before the spawn_blocking
                            // move (the guard is !Send and must not cross the
                            // closure boundary).  Byte-only prune policy, hot-reload.
                            let storage_quota_bytes = {
                                let defaults = copypaste_core::AppConfig::default();
                                core_config
                                    .read()
                                    .map(|g| g.storage_quota_bytes)
                                    .unwrap_or(defaults.storage_quota_bytes)
                            };

                            // Decrypt + re-encrypt + insert on the blocking pool.
                            let result = tokio::task::spawn_blocking(move || {
                                let db_guard = db_arc.blocking_lock();

                                // LWW dedup: skip if item already present with
                                // equal-or-newer lamport_ts.
                                let existing =
                                    match get_item_by_item_id(&db_guard, &item_id_owned) {
                                        Ok(r) => r,
                                        Err(e) => {
                                            tracing::warn!(
                                                "ws_ingest_loop: get_item_by_item_id \
                                                 error for item_id={item_id_owned}: {e}"
                                            );
                                            return false;
                                        }
                                    };

                                let preserved_pk = if let Some(local) = existing.as_ref() {
                                    // CopyPaste-ayvs: use the SAME total order as
                                    // P2P (lamport -> wall_time ->
                                    // origin_device_id) so cloud-WS, cloud-poll,
                                    // and P2P converge identically. The old bare
                                    // `lamport_ts <= local -> skip` kept local on
                                    // every equal-lamport tie.
                                    let wins = remote_wins(
                                        local.lamport_ts,
                                        local.wall_time,
                                        &local.origin_device_id,
                                        &RemoteMeta {
                                            lamport_ts,
                                            wall_time,
                                            origin_device_id: &origin_device_id,
                                        },
                                    );
                                    if !wins {
                                        // Local wins LWW — skip.
                                        zeroize::Zeroize::zeroize(&mut key_arr);
                                        return false;
                                    }
                                    Some(local.id.clone())
                                } else {
                                    match exists_item_by_item_id(&db_guard, &item_id_owned) {
                                        Ok(true) => {
                                            zeroize::Zeroize::zeroize(&mut key_arr);
                                            return false;
                                        }
                                        Ok(false) => None,
                                        Err(e) => {
                                            tracing::warn!(
                                                "ws_ingest_loop: \
                                                 exists_item_by_item_id error for \
                                                 item_id={item_id_owned}: {e}"
                                            );
                                            return false;
                                        }
                                    }
                                };

                                // ── Tombstone fast-path ──────────────────────
                                // Tombstone rows carry deleted=true and no payload.
                                // Apply as a soft-delete so the item becomes a local
                                // tombstone (deleted=1, content wiped) with the remote
                                // lamport/wall_time, exactly matching the P2P path.
                                if ws_deleted {
                                    zeroize::Zeroize::zeroize(&mut key_arr);
                                    if let Some(local_pk) = preserved_pk.as_ref() {
                                        match soft_delete_item(
                                            &db_guard,
                                            local_pk,
                                            lamport_ts,
                                            wall_time,
                                        ) {
                                            Ok(n) if n > 0 => {
                                                tracing::info!(
                                                    "ws_ingest_loop: applied tombstone \
                                                     item_id={item_id_owned}"
                                                );
                                                return true;
                                            }
                                            Ok(_) => {}
                                            Err(e) => {
                                                tracing::warn!(
                                                    "ws_ingest_loop: soft_delete_item failed \
                                                     for item_id={item_id_owned}: {e}"
                                                );
                                            }
                                        }
                                    } else {
                                        // CopyPaste-bfiu: unknown item — the delete
                                        // raced ahead of the create. Persist a
                                        // tombstone so a later out-of-order create
                                        // loses LWW instead of resurrecting it.
                                        match insert_tombstone(
                                            &db_guard,
                                            &item_id_owned,
                                            &item_id_owned,
                                            lamport_ts,
                                            wall_time,
                                            &origin_device_id,
                                        ) {
                                            Ok(_) => {
                                                tracing::info!(
                                                    "ws_ingest_loop: inserted tombstone for \
                                                     unknown item_id={item_id_owned} \
                                                     (delete-before-create)"
                                                );
                                                return true;
                                            }
                                            Err(e) => {
                                                tracing::warn!(
                                                    "ws_ingest_loop: insert_tombstone failed \
                                                     for item_id={item_id_owned}: {e}"
                                                );
                                            }
                                        }
                                    }
                                    return false;
                                }

                                // Decrypt with sync key.
                                // `blob_opt` is always `Some` here — tombstones were
                                // handled above and `blob_opt` is `None` only for
                                // deleted rows (guarded by `ws_deleted`).
                                let blob = match blob_opt {
                                    Some(b) => b,
                                    None => {
                                        tracing::warn!(
                                            "ws_ingest_loop: no blob for non-tombstone \
                                             id={id_owned}; skipping"
                                        );
                                        zeroize::Zeroize::zeroize(&mut key_arr);
                                        return false;
                                    }
                                };
                                let tmp_key = SyncKey::from_bytes(key_arr);
                                let plaintext =
                                    match decrypt_from_cloud(&tmp_key, &item_id_owned, &blob) {
                                        Ok(p) => p,
                                        Err(e) => {
                                            tracing::warn!(
                                                "ws_ingest_loop: decrypt_from_cloud \
                                                 failed for id={id_owned}: {e}; skipping"
                                            );
                                            return false;
                                        }
                                    };

                                // Re-encrypt with local key.
                                let mut local_item = match build_local_item(
                                    &id_owned,
                                    &item_id_owned,
                                    &content_type,
                                    &plaintext,
                                    lamport_ts,
                                    wall_time,
                                    expires_at,
                                    app_bundle_id,
                                    origin_device_id,
                                    &local_key_clone,
                                ) {
                                    Ok(i) => i,
                                    Err(e) => {
                                        tracing::warn!(
                                            "ws_ingest_loop: local re-encrypt failed \
                                             for id={id_owned}: {e}; skipping"
                                        );
                                        return false;
                                    }
                                };

                                if let Some(pk) = preserved_pk.as_ref() {
                                    local_item.id = pk.clone();
                                }

                                // Apply cloud pin state. When the cloud row carries
                                // the pin columns (ws_has_pin_col) they are
                                // authoritative — use them directly. For legacy rows
                                // (schema-skew, no pin columns) fall back to the
                                // previous OR-merge so a pinned item does not lose
                                // its prune-exemption on an old-schema roundtrip.
                                if ws_has_pin_col {
                                    local_item.pinned = ws_pinned;
                                    local_item.pin_order = ws_pin_order;
                                } else if let Some(local) = existing.as_ref() {
                                    local_item.pinned = local_item.pinned || local.pinned;
                                    if local_item.pin_order.is_none() {
                                        local_item.pin_order = local.pin_order;
                                    }
                                }

                                let write_res = if preserved_pk.is_some() {
                                    replace_cloud_item_by_item_id(&db_guard, &local_item)
                                } else {
                                    insert_item(&db_guard, &local_item)
                                        .map_err(anyhow::Error::from)
                                };

                                match write_res {
                                    Ok(()) => {
                                        tracing::info!(
                                            "ws_ingest_loop: ingested INSERT \
                                             item_id={} (id={})",
                                            local_item.item_id,
                                            local_item.id
                                        );
                                        // Prune to the byte-only storage cap.
                                        // Count-based (`history_limit`) pruning was
                                        // removed: `prune_to_cap` against
                                        // `storage_quota_bytes` is the single
                                        // authoritative retention policy.
                                        let max_bytes =
                                            storage_quota_bytes.min(i64::MAX as u64) as i64;
                                        match prune_to_cap(&db_guard, max_bytes) {
                                            Ok(0) => {}
                                            Ok(n) => tracing::debug!(
                                                "ws_ingest_loop: byte-pruned {n} rows \
                                                 (quota_bytes={storage_quota_bytes})"
                                            ),
                                            Err(e) => tracing::warn!(
                                                "ws_ingest_loop: prune_to_cap failed: {e}"
                                            ),
                                        }
                                        true
                                    }
                                    Err(e) => {
                                        tracing::warn!(
                                            "ws_ingest_loop: failed to store \
                                             item_id={}: {e}",
                                            local_item.item_id
                                        );
                                        false
                                    }
                                }
                            })
                            .await;

                            if let Ok(true) = result {
                                let now_ms = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_millis() as i64;
                                last_sync_ms.store(now_ms, Ordering::Relaxed);
                            }
                        }
                    }
                }
            }
        }

        // Brief backoff before reconnecting so a flapping connection
        // doesn't spin the loop.  The WS client itself uses exponential
        // backoff internally, but that is for errors during a session;
        // this covers the outer reconnect loop after a clean disconnect.
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(5)) => {}
            _ = shutdown.notified() => {
                tracing::info!("ws_ingest_loop: shutdown during reconnect backoff");
                // Update config.user_jwt with latest bearer before the next
                // connect attempt — not needed here since we're shutting down.
                return;
            }
        }
    }
}
