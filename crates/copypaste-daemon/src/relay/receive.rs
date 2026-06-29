//! Relay receive path: pull pages from the inbox, ingest via LWW, advance the
//! watermark, drive the poll loop.

use std::sync::{
    atomic::{AtomicI64, Ordering},
    Arc,
};

use copypaste_core::{
    decrypt_from_cloud, exists_item_by_item_id, get_item_by_item_id, insert_item, insert_tombstone,
    prune_to_cap, soft_delete_item, AppConfig, Database, SyncKey,
};
// CopyPaste-ayvs: relay LWW now routes through the SAME total order the P2P and
// cloud paths use (lamport -> wall_time -> origin_device_id) so all transports
// converge identically.
use copypaste_sync::merge::{remote_wins, RemoteMeta};
use tokio::sync::{Mutex, Notify};

use crate::sync_common::{build_local_item, replace_cloud_item_by_item_id};
use crate::sync_in_flight::SyncInFlightGuard;

use super::pasteboard::{
    relay_apply_to_pasteboard, relay_fetch_auto_apply_candidate, relay_should_auto_apply,
    relay_should_skip_wifi,
};
use super::registration::{ensure_token, load_initial_token, snapshot_sync_key};
use super::types::{PullItem, RelayError};
use super::watermark::{load_watermark, save_watermark, Watermark};
use super::wire::decode_payload;

// ── Pull ─────────────────────────────────────────────────────────────────────

/// Pull one page from the inbox past the watermark. Returns the raw items and
/// whether a 401 was seen (caller re-registers).
pub(super) async fn pull_page(
    client: &reqwest::Client,
    relay_url: &str,
    inbox_id: &str,
    token: &str,
    wm: Watermark,
) -> Result<Vec<PullItem>, RelayError> {
    use super::super::relay::PULL_LIMIT;
    let url = format!(
        "{relay_url}/devices/{inbox_id}/items?since={}&since_id={}&limit={}",
        wm.wall, wm.id, PULL_LIMIT
    );
    let resp = client
        .get(&url)
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| RelayError::Transport(e.to_string()))?;
    let status = resp.status();
    if status.as_u16() == 401 {
        return Err(RelayError::Status(401));
    }
    if !status.is_success() {
        return Err(RelayError::Status(status.as_u16()));
    }
    resp.json::<Vec<PullItem>>()
        .await
        .map_err(|e| RelayError::Transport(format!("decode pull response: {e}")))
}

// ── Ingest ───────────────────────────────────────────────────────────────────

/// Ingest one pulled page into the local DB on a blocking thread (SQLCipher +
/// AEAD). Returns the advanced watermark and how many rows were stored.
///
/// LWW + quota-prune are byte-for-byte the Supabase poll path: dedup on
/// `item_id`, a strictly-newer remote `lamport_ts` replaces in place (preserving
/// the local PK + pin state), an older/equal one is skipped (this is also what
/// makes our OWN pushed rows a no-op when they echo back — self-echo dedup).
pub(super) fn ingest_page_blocking(
    db: &Database,
    local_key: &zeroize::Zeroizing<[u8; 32]>,
    sync_key_bytes: &[u8; 32],
    page: &[PullItem],
    start: Watermark,
    storage_quota_bytes: u64,
) -> (Watermark, u32) {
    let mut wm = start;
    let mut stored = 0u32;
    let sk = SyncKey::from_bytes(*sync_key_bytes);

    for row in page {
        // Advance the watermark for EVERY readable row (even skipped ones) so the
        // next page does not re-request them.
        if (row.wall_time, row.id) > (wm.wall, wm.id) {
            wm = Watermark {
                wall: row.wall_time,
                id: row.id,
            };
        }

        // CopyPaste-crh3.69: version-gated decode of EITHER wire format —
        // legacy V1 `base64(JSON{..,ct_b64})` (in-flight inbox items written by
        // older daemons) OR the new V2 single-base64 frame
        // `base64(0x01||u32_le(meta_len)||meta_json||raw_ct)`. Both funnel into
        // the same metadata + raw ciphertext shape.
        let env = match decode_payload(&row.content_b64) {
            Ok(d) => d,
            Err(e) => {
                tracing::warn!(
                    "relay-sync: id={} wire decode failed: {e}; skipping",
                    row.id
                );
                continue;
            }
        };
        let blob: &[u8] = &env.ct;

        // LWW dedup on the cross-device item_id.
        let existing = match get_item_by_item_id(db, &env.item_id) {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("relay-sync: get_item_by_item_id error: {e}; skipping");
                continue;
            }
        };
        // The envelope's wall_time is authoritative for LWW; fall back to the
        // relay row's wall_time when an older envelope omitted it (=> 0).
        let env_wall = if env.wall_time != 0 {
            env.wall_time
        } else {
            row.wall_time as i64
        };
        let preserved_pk = if let Some(local) = existing.as_ref() {
            // CopyPaste-ayvs: same total order as P2P/cloud (lamport ->
            // wall_time -> origin_device_id) instead of the old bare
            // `env.lamport_ts <= local -> keep`, which never converged on ties.
            let wins = remote_wins(
                local.lamport_ts,
                local.wall_time,
                &local.origin_device_id,
                &RemoteMeta {
                    lamport_ts: env.lamport_ts,
                    wall_time: env_wall,
                    origin_device_id: &env.origin_device_id,
                },
            );
            if !wins {
                // Local wins LWW — keep it (self-echo no-op + remote-edit loser).
                continue;
            }
            Some(local.id.clone())
        } else {
            match exists_item_by_item_id(db, &env.item_id) {
                Ok(true) => continue,
                Ok(false) => None,
                Err(e) => {
                    tracing::warn!("relay-sync: exists_item_by_item_id error: {e}; skipping");
                    continue;
                }
            }
        };

        // ── Tombstone fast-path (CopyPaste-cm0u / CopyPaste-bfiu) ─────────────
        // A delete envelope carries deleted=true and an empty ct_b64 (NULL
        // content). Apply it via the SAME soft_delete / insert_tombstone path as
        // P2P and cloud so deletes propagate over relay-only topologies, and a
        // delete that races ahead of the create still leaves a tombstone the
        // later create loses LWW against.
        if env.deleted {
            if let Some(local_pk) = preserved_pk.as_ref() {
                match soft_delete_item(db, local_pk, env.lamport_ts, env_wall) {
                    Ok(n) if n > 0 => {
                        stored += 1;
                        tracing::info!("relay-sync: applied tombstone (item known locally)");
                    }
                    Ok(_) => {}
                    Err(e) => tracing::warn!("relay-sync: soft_delete_item failed: {e}"),
                }
            } else {
                match insert_tombstone(
                    db,
                    &env.item_id,
                    &env.item_id,
                    env.lamport_ts,
                    env_wall,
                    &env.origin_device_id,
                ) {
                    Ok(_) => {
                        stored += 1;
                        tracing::info!(
                            "relay-sync: inserted tombstone for unknown item \
                             (delete-before-create)"
                        );
                    }
                    Err(e) => tracing::warn!("relay-sync: insert_tombstone failed: {e}"),
                }
            }
            continue;
        }

        // Decrypt with the sync key (AAD = item_id + cloud schema v5).
        let plaintext = match decrypt_from_cloud(&sk, &env.item_id, blob) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(
                    "relay-sync: decrypt_from_cloud failed for item_id (wrong passphrase or \
                     tampered blob): {e}; skipping"
                );
                continue;
            }
        };

        let mut local_item = match build_local_item(
            // Use the cross-device item_id as the local PK seed when this is a
            // fresh insert; build_local_item sets `id` from this first arg.
            &env.item_id,
            &env.item_id,
            &row.content_type,
            &plaintext,
            env.lamport_ts,
            env_wall,
            None,
            None,
            // CopyPaste-ayvs: preserve the sender's origin so future tie-breaks
            // on this device stay deterministic across hops.
            env.origin_device_id.clone(),
            local_key,
        ) {
            Ok(i) => i,
            Err(e) => {
                tracing::warn!("relay-sync: build_local_item failed: {e}; skipping");
                continue;
            }
        };

        // LWW replace preserves the prior local row's PK.
        if let Some(pk) = preserved_pk.as_ref() {
            local_item.id = pk.clone();
        }
        // CopyPaste-cm0u: the envelope's pin state is authoritative (it travels
        // with the item now). The pin LWW already won above (this is the
        // TakeRemote branch), so apply the sender's pinned/pin_order directly.
        local_item.pinned = env.pinned;
        local_item.pin_order = env.pin_order;

        let write_res = if preserved_pk.is_some() {
            replace_cloud_item_by_item_id(db, &local_item)
        } else {
            insert_item(db, &local_item).map_err(anyhow::Error::from)
        };
        match write_res {
            Ok(()) => {
                stored += 1;
                tracing::info!("relay-sync: ingested remote item (id={})", local_item.id);
            }
            Err(e) => tracing::warn!("relay-sync: store failed: {e}"),
        }
    }

    // Byte-cap prune after ingest (long-offline backfill safety) — same policy
    // as the Supabase poll path.
    if stored > 0 {
        let max_bytes = storage_quota_bytes.min(i64::MAX as u64) as i64;
        match prune_to_cap(db, max_bytes) {
            Ok(0) => {}
            Ok(n) => tracing::debug!("relay-sync: byte-pruned {n} rows after ingest"),
            Err(e) => tracing::warn!("relay-sync: prune_to_cap failed: {e}"),
        }
    }

    (wm, stored)
}

// ── Receive loop ─────────────────────────────────────────────────────────────

/// The receive loop: poll the shared inbox, ingest new items via the LWW path,
/// advance the watermark.
// All parameters are independent runtime slices (db, url, name, device_id,
// keys, shutdown, auto_apply_change_count) with no natural grouping for a
// private async fn.
#[allow(clippy::too_many_arguments)]
pub(super) async fn receive_loop(
    client: reqwest::Client,
    relay_url: String,
    device_name: String,
    device_id: String,
    shutdown: Arc<Notify>,
    db: Arc<Mutex<Database>>,
    sync_key: Arc<Mutex<Option<SyncKey>>>,
    local_key: Arc<zeroize::Zeroizing<[u8; 32]>>,
    last_sync_ms: Arc<AtomicI64>,
    core_config: Arc<std::sync::RwLock<AppConfig>>,
    // Shared self-write sentinel for the pasteboard poller.  When `Some`, the
    // relay auto-apply path stamps this atomic before/after each NSPasteboard
    // write so the `ClipboardMonitor` does not re-capture daemon-own writes
    // (loop prevention — mirrors the sync_orch / copy_item IPC guard).
    // `None` disables the pasteboard write (non-Unix, tests, callers that have
    // not wired the sentinel yet).
    auto_apply_change_count: Option<Arc<AtomicI64>>,
    // CopyPaste-1jms.22: shared in-flight flag for SyncBadgeState::Syncing.
    sync_in_flight: std::sync::Arc<std::sync::atomic::AtomicBool>,
) {
    use super::super::relay::{
        IDLE_EMPTY_POLL_THRESHOLD, IDLE_POLL_STEP, POLL_INTERVAL, POLL_INTERVAL_MAX,
    };
    use copypaste_core::derive_relay_inbox_id;

    let mut cached_token = load_initial_token(&local_key, &device_id);
    // CopyPaste-hf40 / CopyPaste-1jms.24: load the persisted watermark so a
    // daemon restart resumes from the last-seen (wall, id) cursor rather than
    // re-fetching all relay items from (0, 0).
    let mut wm = load_watermark();
    let mut warned_no_key = false;

    // CopyPaste-28br: adaptive idle back-off.
    //
    // When no items arrive for `IDLE_EMPTY_POLL_THRESHOLD` consecutive polls
    // the interval grows linearly (POLL_INTERVAL per step) up to
    // POLL_INTERVAL_MAX, reducing battery drain and relay load during idle
    // periods. A non-empty pull resets the counter and interval to their
    // minimum values so latency stays low when items are actually flowing.
    let mut consecutive_empty: u32 = 0;
    let mut current_interval = POLL_INTERVAL;

    loop {
        // Wait an interval, but wake early on shutdown.
        tokio::select! {
            biased;
            _ = shutdown.notified() => {
                tracing::info!("relay-sync receive_loop: shutdown");
                break;
            }
            _ = tokio::time::sleep(current_interval) => {}
        }

        let key_bytes = match snapshot_sync_key(&sync_key).await {
            Some(b) => {
                warned_no_key = false;
                b
            }
            None => {
                if !warned_no_key {
                    tracing::warn!("relay-sync receive_loop: no sync passphrase set — idle");
                    warned_no_key = true;
                }
                continue;
            }
        };

        // tke7 (PG-30): hot-reload master sync gate — checked on every poll tick.
        let sync_enabled = core_config.read().map(|g| g.sync_enabled).unwrap_or(true);
        if !sync_enabled {
            tracing::debug!("relay-sync receive_loop: sync_enabled=false; skipping poll this tick");
            continue;
        }

        // A-SET-2 hot-reload: check sync_on_wifi_only every tick so a runtime
        // set_config change takes effect without a daemon restart.  The
        // is_on_wifi check runs on a blocking thread (networksetup shell
        // invocation) so it does not stall the async executor.  Mirrors the
        // identical guard in cloud.rs poll loop.
        let (sync_on_wifi_only, auto_apply_synced_clip) = core_config
            .read()
            .map(|g| (g.sync_on_wifi_only, g.auto_apply_synced_clip))
            .unwrap_or((false, true));
        if sync_on_wifi_only {
            let on_wifi = tokio::task::spawn_blocking(crate::platform::is_on_wifi)
                .await
                .unwrap_or(true); // fail-open: assume Wi-Fi if detection errors
            if relay_should_skip_wifi(sync_on_wifi_only, on_wifi) {
                tracing::debug!(
                    "relay-sync receive_loop: sync_on_wifi_only=true and not on Wi-Fi; \
                     skipping this tick"
                );
                continue;
            }
        }
        // Shadow as a local bool so the ingest path can use it without holding
        // the RwLock guard across await points.
        let auto_apply_enabled = relay_should_auto_apply(auto_apply_synced_clip);

        let inbox_id = derive_relay_inbox_id(&key_bytes);

        let token = match ensure_token(
            &client,
            &relay_url,
            &key_bytes,
            &device_name,
            &mut cached_token,
            &local_key,
            &device_id,
        )
        .await
        {
            Ok(t) => t,
            Err(e) => {
                tracing::warn!("relay-sync receive_loop: register failed: {e}");
                continue;
            }
        };

        // Burst-drain: keep pulling while pages come back full.
        loop {
            // CopyPaste-1jms.22: arm in-flight guard for this relay pull
            // round-trip. Resets on drop (error, empty, or end of drain).
            let _relay_rx_guard = SyncInFlightGuard::new(std::sync::Arc::clone(&sync_in_flight));
            let page = match pull_page(&client, &relay_url, &inbox_id, &token, wm).await {
                Ok(p) => p,
                Err(RelayError::Status(401)) => {
                    tracing::info!("relay-sync receive_loop: 401; re-registering next tick");
                    cached_token = None;
                    break;
                }
                Err(e) => {
                    tracing::warn!("relay-sync receive_loop: pull failed: {e}");
                    break;
                }
            };
            if page.is_empty() {
                // CopyPaste-28br: empty poll — advance the idle counter and
                // grow the interval (linearly in IDLE_POLL_STEP increments,
                // capped at POLL_INTERVAL_MAX).
                consecutive_empty = consecutive_empty.saturating_add(1);
                if consecutive_empty >= IDLE_EMPTY_POLL_THRESHOLD {
                    // Each idle step adds one IDLE_POLL_STEP (60 s) so the
                    // first step already meets the ≥ 60 s acceptance criterion.
                    let steps = consecutive_empty.saturating_sub(IDLE_EMPTY_POLL_THRESHOLD) + 1;
                    current_interval = (IDLE_POLL_STEP * steps).min(POLL_INTERVAL_MAX);
                    tracing::debug!(
                        consecutive_empty,
                        current_interval_secs = current_interval.as_secs(),
                        "relay-sync receive_loop: idle back-off active"
                    );
                }
                break;
            }
            let page_len = page.len();

            let quota = core_config
                .read()
                .map(|g| g.storage_quota_bytes)
                .unwrap_or(u64::MAX);
            let db_arc = db.clone();
            let local_key_clone = local_key.clone();
            let join = tokio::task::spawn_blocking(move || {
                let guard = db_arc.blocking_lock();
                ingest_page_blocking(&guard, &local_key_clone, &key_bytes, &page, wm, quota)
            })
            .await;
            match join {
                Ok((new_wm, stored)) => {
                    let advanced =
                        new_wm.wall > wm.wall || (new_wm.wall == wm.wall && new_wm.id > wm.id);
                    wm = new_wm;
                    if advanced {
                        // CopyPaste-hf40 / CopyPaste-1jms.24: persist the
                        // advanced watermark so a daemon restart resumes
                        // from the last-seen cursor instead of zero.
                        // CopyPaste-crh3.79: save_watermark does write + fsync
                        // (sync_all), which can take 50-200ms on APFS/NFS. Run it
                        // on the blocking pool so this async receive worker is not
                        // parked; awaiting yields the worker (runs other tasks)
                        // while the fsync proceeds, and preserves save ordering.
                        let _ = tokio::task::spawn_blocking(move || save_watermark(wm)).await;
                    }
                    if stored > 0 {
                        // CopyPaste-28br: a non-empty batch — reset idle
                        // back-off so latency stays low while items are flowing.
                        consecutive_empty = 0;
                        current_interval = POLL_INTERVAL;
                        last_sync_ms.store(super::now_ms(), Ordering::Relaxed);
                        if auto_apply_enabled {
                            // CopyPaste-7ub: implement auto_apply_synced_clip on the
                            // relay receive path. Fetch the freshest stored text item,
                            // decrypt it, and write it to NSPasteboard — stamping the
                            // self-write sentinel so the ClipboardMonitor does NOT
                            // re-capture the write as a new local item (loop prevention).
                            //
                            // The pasteboard write is gated on `auto_apply_change_count`
                            // being Some (wired from daemon.rs via `start_relay`). When
                            // None (tests, non-Unix) the ingest is still recorded in
                            // last_sync_ms but no pasteboard write occurs.
                            if let Some(ref swcc) = auto_apply_change_count {
                                let db_arc2 = db.clone();
                                let lk2 = local_key.clone();
                                let swcc2 = swcc.clone();
                                let join2 = tokio::task::spawn_blocking(move || {
                                    let guard = db_arc2.blocking_lock();
                                    if let Some(cand) =
                                        relay_fetch_auto_apply_candidate(&guard, &lk2)
                                    {
                                        relay_apply_to_pasteboard(&cand, &swcc2);
                                    }
                                })
                                .await;
                                if let Err(e) = join2 {
                                    tracing::warn!(
                                        "relay-sync receive_loop: auto-apply task panicked: {e}"
                                    );
                                }
                            } else {
                                tracing::debug!(
                                    "relay-sync receive_loop: auto_apply_synced_clip=true \
                                     but change-count sentinel not wired; \
                                     {stored} relay item(s) stored (no pasteboard write)"
                                );
                            }
                        } else {
                            tracing::debug!(
                                "relay-sync receive_loop: auto_apply_synced_clip=false; \
                                 {stored} relay item(s) stored but NOT auto-applied to pasteboard"
                            );
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("relay-sync receive_loop: ingest task panicked: {e}");
                    break;
                }
            }
            let pull_limit: usize = super::super::relay::PULL_LIMIT;
            if page_len < pull_limit {
                break;
            }
        }
    }
}
