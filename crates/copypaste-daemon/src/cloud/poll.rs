use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};

use copypaste_core::storage::items::soft_delete_item;
use copypaste_core::{
    decrypt_from_cloud, exists_item_by_item_id, get_item_by_item_id, insert_item, insert_tombstone,
    prune_to_cap, Database, SyncKey,
};
use copypaste_supabase::auth::AuthClient;
use copypaste_sync::merge::{remote_wins, RemoteMeta};

use crate::sync_common::{
    build_local_item, decode_payload_ct, replace_cloud_item_by_item_id, SYNC_HTTP_TIMEOUT,
};
use crate::sync_cursor::CloudCursor;
use crate::sync_in_flight::SyncInFlightGuard;

use super::auth::refresh_bearer;
use super::config::CloudConfig;
use super::push::{parse_retry_after_secs, PUSH_INITIAL_BACKOFF, PUSH_MAX_BACKOFF};

// ── Realtime / poll-interval tuning (v0.5.3) ─────────────────────────────────

/// HTTP poll interval when the Realtime WebSocket is **connected** *and the
/// Phoenix Channel join has been confirmed* (`phx_reply ok`).
///
/// The WS delivers INSERT events instantly once the channel is subscribed, so
/// the poll loop runs only as a catch-up / missed-event safety net at a lower
/// frequency.  Lowered from 120 s → 60 s (Phase 3) to halve the worst-case
/// missed-event window while still keeping the HTTP load negligible compared
/// to full-speed fallback polling.
const POLL_INTERVAL_WS_CONNECTED: Duration = Duration::from_secs(60);

/// HTTP poll interval when the Realtime WebSocket is **disconnected** or
/// has never connected (original behaviour — full-speed polling as the sole
/// sync path).
const POLL_INTERVAL_WS_FALLBACK: Duration = Duration::from_secs(10);

/// Maximum number of rows fetched per poll tick.
///
/// When a batch comes back full (== POLL_BATCH_SIZE rows), the poll loop
/// immediately re-polls without waiting for the full interval (burst-drain).
/// This prevents a burst of simultaneous remote inserts from stalling at the
/// watermark for a full interval when the batch was exactly exhausted.
const POLL_BATCH_SIZE: usize = 20;

// ── Realtime / poll loop ──────────────────────────────────────────────────────

/// Poll Supabase REST every 10 s for recent items from other devices and insert
/// any that are not already in the local database.
///
/// Download path:
/// 1. `GET /rest/v1/clipboard_items` → raw JSON rows.
/// 2. For each row, base64-decode `payload_ct` → `decrypt_from_cloud(sync_key, item_id, blob)` → plaintext.
/// 3. Re-encrypt plaintext with the local key → local [`ClipboardItem`].
/// 4. Insert via `insert_item` (dedup by `id`).
///
/// If no sync key is set, the poll is skipped with a one-time warning.
/// If decryption fails (wrong passphrase, tampered blob), the row is skipped
/// and a `warn!` is emitted — we never crash, never log plaintext.
/// The settings-table key under which the download high-water-mark (max ingested
/// `wall_time`, in Unix ms) is persisted so a restart resumes forward pagination
/// instead of re-downloading the entire cloud history.
const POLL_WATERMARK_KEY: &str = "cloud_poll_watermark";

/// Forward-pagination cursor for the cloud poll loop.
///
/// `wall` is the Unix-ms wall_time of the last row ingested (the persisted
/// high-water-mark). `id` is that row's primary key, the secondary keyset
/// component used to page forward through rows that share the same `wall`
/// millisecond (see [`build_poll_url`]). `id` is empty on a cold start (only the
/// `wall` lower bound is applied) and is populated once a row is ingested.
// PartialEq: the burst-drain loop compares the post-poll cursor against the
// pre-poll snapshot to detect a no-advance stall (full batch but no usable
// keyset progress) and break rather than re-poll the same window forever.
//
// CopyPaste-w47w #3: the struct has moved to `crate::sync_cursor::CloudCursor`;
// `PollCursor` is a type alias kept for all existing call sites.
pub(crate) type PollCursor = CloudCursor;

/// Base poll query (no lower bound). The keyset cursor filter is appended by
/// [`build_poll_url`] when a watermark is known. Order is the **compound**
/// `(wall_time, id)` so pagination is deterministic even within one millisecond.
/// Base poll query string. The `limit=` value MUST match [`POLL_BATCH_SIZE`];
/// a compile-time assertion in `poll_once` enforces this.
const POLL_SELECT_QS: &str = "select=id,item_id,content_type,payload_ct,lamport_ts,wall_time,expires_at,app_bundle_id,device_id,deleted,pinned,pin_order&order=wall_time.asc,id.asc&limit=20";

/// Construct the poll URL for a single tick using a `(wall_time, id)` keyset
/// cursor.
///
/// WATERMARK BUG FIX: the previous query used a `wall_time`-only cursor
/// (`order=wall_time.asc&limit=20` + strict `wall_time=gt.<max>`). Because
/// `wall_time` is millisecond granularity, a burst of ≥ `limit` rows sharing the
/// SAME max millisecond was fatal: a tick fetched `limit` of them, advanced the
/// watermark to that millisecond, and the next tick's strict `gt` filtered out
/// the remaining same-millisecond rows FOREVER (silent download data loss).
///
/// The fix is a proper compound keyset cursor `(watermark_wall, watermark_id)`
/// ordered by `(wall_time, id)`: each tick requests rows strictly *after* the
/// `(wall_time, id)` pair of the last row ingested. Expressed in PostgREST:
///
/// ```text
/// or=(wall_time.gt.W, and(wall_time.eq.W, id.gt.ID))
/// ```
///
/// i.e. a later millisecond OR the same millisecond with a larger `id`. This
/// advances forward through same-millisecond rows by `id` instead of stalling,
/// so ≥20 rows sharing one wall_time are all eventually fetched, in order, with
/// no gaps. Forward (`asc`) direction is preserved. `watermark_id` is empty on a
/// fresh start (only a `wall_time` lower bound is used) or for a watermark
/// restored from the persisted `wall_time`-only setting.
pub(crate) fn build_poll_url(
    supabase_url: &str,
    watermark_wall: i64,
    watermark_id: &str,
) -> String {
    let base = format!("{supabase_url}/rest/v1/clipboard_items?{POLL_SELECT_QS}");
    if watermark_wall <= 0 {
        return base;
    }
    if watermark_id.is_empty() {
        // No id component yet (cold start from a persisted wall_time-only
        // watermark): use an inclusive `gte` so the boundary millisecond's rows
        // are (re-)offered; the per-row item_id dedup drops already-ingested
        // ones. Once a row is ingested the id component is populated and the
        // strict keyset below takes over.
        return format!("{base}&wall_time=gte.{watermark_wall}");
    }
    // Strict `(wall_time, id)` keyset: a later ms, OR the same ms with a larger
    // id. URL-encode the parens-bearing PostgREST `or=` expression's commas are
    // significant; reqwest will percent-encode the whole query value for us when
    // we pass it through the URL, but we build the canonical PostgREST syntax
    // here (matching the existing hand-built query strings in this module).
    format!(
        "{base}&or=(wall_time.gt.{watermark_wall},and(wall_time.eq.{watermark_wall},id.gt.{watermark_id}))"
    )
}

/// Seed the download watermark on startup from the larger of the persisted
/// `cloud_poll_watermark` setting and the local `MAX(wall_time)`. Either source
/// missing/unreadable contributes `0` (download from the beginning). Never
/// errors — a fresh DB or absent setting simply yields `0`.
pub(crate) fn load_poll_watermark(db: &Database) -> i64 {
    let persisted: i64 = db
        .conn()
        .query_row(
            "SELECT value FROM settings WHERE key = ?1",
            rusqlite::params![POLL_WATERMARK_KEY],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0);
    let local_max: i64 = db
        .conn()
        .query_row(
            "SELECT COALESCE(MAX(wall_time), 0) FROM clipboard_items",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);
    persisted.max(local_max)
}

/// Persist the download watermark into the `settings` table (upsert). Returns the
/// rusqlite error on failure so the caller can log it; the watermark also lives
/// in memory, so a persist failure only costs re-pagination after a restart.
pub(crate) fn save_poll_watermark(db: &Database, watermark: i64) -> rusqlite::Result<()> {
    db.conn().execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2) \
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        rusqlite::params![POLL_WATERMARK_KEY, watermark.to_string()],
    )?;
    Ok(())
}

// Realtime loop handles: config, bearer, db, shutdown, notifier, signed_in,
// and ingest_tx — each is an independent runtime dependency with no natural
// grouping short of a new private struct.
#[allow(clippy::too_many_arguments)]
pub(super) async fn realtime_loop(
    config: CloudConfig,
    bearer: Arc<RwLock<String>>,
    db: Arc<Mutex<Database>>,
    shutdown: Arc<tokio::sync::Notify>,
    sync_key: Arc<Mutex<Option<SyncKey>>>,
    local_key: Arc<zeroize::Zeroizing<[u8; 32]>>,
    last_sync_ms: Arc<std::sync::atomic::AtomicI64>,
    cloud_signed_in: Arc<std::sync::atomic::AtomicBool>,
    auth: Arc<AuthClient>,
    // Flag set by the WS task. When `true`, this loop uses the slow
    // POLL_INTERVAL_WS_CONNECTED (2 min) interval so the WS delivers
    // events instantly and HTTP is only a catch-up safety net.  When
    // `false` (WS down / never connected), the loop runs at
    // POLL_INTERVAL_WS_FALLBACK (10 s) as the sole download path.
    ws_connected: Arc<std::sync::atomic::AtomicBool>,
    // Live core config for hot-reload of sync_on_wifi_only and
    // storage_quota_bytes (A-SET-2).  Loops read on every tick so runtime
    // set_config changes take effect without a daemon restart.
    core_config: Arc<std::sync::RwLock<copypaste_core::AppConfig>>,
    // CopyPaste-1jms.22: shared in-flight flag for SyncBadgeState::Syncing.
    // Set true at the start of each poll_once round-trip via SyncInFlightGuard;
    // the guard's Drop resets it false on ALL exit paths (success, error, `?`).
    sync_in_flight: std::sync::Arc<std::sync::atomic::AtomicBool>,
) {
    // P1: same timeout as push_loop — see `sync_common::SYNC_HTTP_TIMEOUT`.
    //
    // CopyPaste-16vr: propagate builder error rather than falling back to a
    // no-timeout client. TLS cert-store load cannot fail on macOS/Linux.
    let client = reqwest::Client::builder()
        .timeout(SYNC_HTTP_TIMEOUT)
        .build()
        .expect("reqwest Client::builder should not fail on supported platforms");
    // Start at the fallback (full-speed) interval; the tick period is
    // updated dynamically before each sleep based on ws_connected.
    let mut interval = tokio::time::interval(POLL_INTERVAL_WS_FALLBACK);
    // Don't burst: if a poll round runs long (slow network, large batch) and we
    // miss one or more ticks, skip the backlog and resume on the next aligned
    // tick instead of firing the missed ticks back-to-back (the default `Burst`
    // behavior), which would hammer the relay/Supabase right after recovery.
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut warned_no_key = false;

    // BUG 1 fix — download high-water-mark.
    //
    // The previous poll URL was a fixed `order=wall_time.desc&limit=20` with NO
    // lower bound, so every tick re-fetched the same newest 20 rows: older
    // history never downloaded, and if more than 20 items arrived between ticks
    // the surplus was lost forever. We now track the maximum `wall_time` we have
    // ingested and append `&wall_time=gt.<watermark>` AND order `wall_time.asc`
    // so polling paginates strictly FORWARD from the watermark: each tick takes
    // the oldest `limit` rows above it (descending order would skip rows between
    // the watermark and the limit-th newest when >limit arrive per tick). The
    // column/filter syntax is the same one the Android client uses
    // — `wall_time=gt.$sinceWallTime`). The watermark is seeded on startup from
    // the larger of (a) the persisted `cloud_poll_watermark` setting and (b) the
    // local `MAX(wall_time)`, and is persisted again after each advance so a
    // daemon restart does not re-download the entire history.
    let mut cursor: PollCursor = {
        let db_arc = db.clone();
        let wall = tokio::task::spawn_blocking(move || {
            let db_guard = db_arc.blocking_lock();
            load_poll_watermark(&db_guard)
        })
        .await
        .unwrap_or(0);
        // The persisted watermark is wall_time-only, so the id component starts
        // empty and is populated as soon as the first row is ingested. Until
        // then `build_poll_url` uses an inclusive `gte.<wall>` so no boundary
        // millisecond row is skipped.
        PollCursor {
            wall,
            id: String::new(),
        }
    };
    tracing::info!(
        "cloud-sync poll: seeded download watermark wall_time={}",
        cursor.wall
    );

    loop {
        // Dynamic interval: slow down when the WebSocket is delivering events
        // instantly (2 min catch-up), run full-speed when WS is down (10 s).
        // We reset the interval BEFORE waiting so the new period takes effect
        // on the very next sleep, not after a stale tick fires.
        let tick_period = if ws_connected.load(Ordering::Relaxed) {
            POLL_INTERVAL_WS_CONNECTED
        } else {
            POLL_INTERVAL_WS_FALLBACK
        };
        if interval.period() != tick_period {
            interval = tokio::time::interval(tick_period);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            // Consume the immediate tick that a fresh interval fires on creation
            // so we don't poll twice in quick succession after a period change.
            interval.tick().await;
        }

        tokio::select! {
            _ = interval.tick() => {
                // tke7 (PG-30): hot-reload master sync gate for cloud poll.
                let sync_enabled_now = core_config
                    .read()
                    .map(|g| g.sync_enabled)
                    .unwrap_or(true);
                if !sync_enabled_now {
                    tracing::debug!(
                        "cloud-sync poll: sync_enabled=false; skipping this poll tick"
                    );
                    continue;
                }

                // A-SET-2 hot-reload: read sync_on_wifi_only live so a
                // runtime set_config change takes effect without a restart.
                // The is_on_wifi check runs on a blocking thread (networksetup
                // shell invocation) so it doesn't block the async executor.
                let (sync_on_wifi_only, storage_quota_bytes) = {
                    let defaults = copypaste_core::AppConfig::default();
                    core_config
                        .read()
                        .map(|g| (g.sync_on_wifi_only, g.storage_quota_bytes))
                        .unwrap_or((false, defaults.storage_quota_bytes))
                };
                if sync_on_wifi_only
                    && !tokio::task::spawn_blocking(crate::platform::is_on_wifi)
                        .await
                        .unwrap_or(true)
                {
                    tracing::debug!(
                        "cloud-sync poll: sync_on_wifi_only=true and not on Wi-Fi; \
                         skipping this tick"
                    );
                    continue;
                }


                // If no sync key is set, skip with a one-time warning.
                let key_snapshot: Option<Vec<u8>> = {
                    let guard = sync_key.lock().await;
                    guard.as_ref().map(|k| k.as_bytes().to_vec())
                };
                let key_bytes = match key_snapshot {
                    None => {
                        if !warned_no_key {
                            tracing::warn!(
                                "cloud-sync poll: no sync passphrase set — \
                                 skipping download (call set_sync_passphrase first)"
                            );
                            warned_no_key = true;
                        }
                        continue;
                    }
                    Some(b) => {
                        warned_no_key = false;
                        b
                    }
                };

                // One poll round: fetch rows newer than `watermark`, ingest them,
                // and advance/persist the watermark. Extracted into `poll_once`
                // so the forward-pagination contract (BUG 1) is unit-testable
                // without waiting on the 10s interval. `poll_once` internally uses
                // `fetch_remote_rows_with_refresh`, which performs the bl-cloud
                // refresh-token grant on a 401 (via `auth`) and updates
                // `cloud_signed_in`.
                //
                // Burst-drain: if the batch came back full (== POLL_BATCH_SIZE),
                // there may be more rows waiting — re-poll immediately rather than
                // waiting the full interval, so a multi-device burst of simultaneous
                // inserts is drained without a full 10-120 s delay per batch.
                loop {
                    // Snapshot the cursor before this poll so we can detect a
                    // stall: if a full batch's rows all lack a usable id/item_id,
                    // `batch_max`/`new_cursor` never advance past `start_cursor`
                    // and the keyset filter re-requests the exact same window
                    // forever. Break on no-advance below (defensive).
                    let start_cursor = cursor.clone();
                    // CopyPaste-1jms.22: arm the in-flight guard for this
                    // poll round-trip. The guard sets sync_in_flight=true and
                    // resets to false on Drop (all exit paths including `?`
                    // and burst-drain breaks), so the badge is Syncing only
                    // while the network exchange is actually in progress.
                    let _in_flight_guard =
                        SyncInFlightGuard::new(std::sync::Arc::clone(&sync_in_flight));
                    let (new_cursor, batch_size) = poll_once(
                        &client,
                        &config,
                        &bearer,
                        &db,
                        &local_key,
                        &last_sync_ms,
                        &cloud_signed_in,
                        &auth,
                        &key_bytes,
                        cursor,
                        storage_quota_bytes,
                    )
                    .await;
                    // Drop the guard before the burst-drain loop's bookkeeping
                    // so idle checks between polls do not show Syncing.
                    drop(_in_flight_guard);
                    cursor = new_cursor;
                    // Only keep draining if the batch was full AND shutdown hasn't fired.
                    // Check shutdown without blocking so we don't stall the drain loop.
                    if batch_size < POLL_BATCH_SIZE {
                        break;
                    }
                    // Defensive stall-guard: a full batch whose cursor did NOT
                    // advance means no row was usable for keyset progress, so
                    // re-polling would spin on the same window indefinitely. A
                    // genuine backlog always advances the cursor, so this only
                    // breaks the pathological no-progress case. Placed AFTER the
                    // partial-batch break so the normal path is untouched.
                    if cursor == start_cursor {
                        tracing::warn!(
                            "cloud-sync burst drain: full batch but cursor did not advance; \
                             breaking drain to avoid re-polling the same window"
                        );
                        break;
                    }
                    // Check shutdown between burst-drain ticks.
                    if matches!(
                        tokio::time::timeout(
                            Duration::from_millis(0),
                            shutdown.notified(),
                        )
                        .await,
                        Ok(())
                    ) {
                        tracing::info!(
                            "cloud-sync realtime_loop: shutdown during burst drain"
                        );
                        return;
                    }
                    tracing::debug!(
                        "cloud-sync burst drain: batch_size={batch_size} == POLL_BATCH_SIZE, re-polling immediately"
                    );
                }
            }
            _ = shutdown.notified() => {
                tracing::info!("cloud-sync realtime_loop: shutdown received");
                break;
            }
        }
    }
}

/// Execute a single poll round and return the (possibly advanced) cursor.
///
/// 1. Build the poll URL with a `(wall_time, id)` keyset cursor ordered
///    `wall_time.asc, id.asc` so PostgREST returns the OLDEST `limit` rows after
///    everything ingested so far (forward pagination). The compound cursor
///    prevents the same-millisecond-burst data loss the old `wall_time`-only
///    `gt` cursor suffered (see [`build_poll_url`]).
/// 2. For each row, dedup/LWW by the cross-device `item_id`: a brand-new item is
///    inserted; an item already present locally is routed through an LWW resolve
///    (newer `lamport_ts` wins) and, on a win, replaced in place while the local
///    primary key is preserved.
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
    key_bytes: &[u8],
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
    // not blocked by rusqlite IO. We snapshot the key bytes (non-secret from the
    // perspective of the blocking thread, but never logged).
    let db_arc = db.clone();
    let local_key_clone = local_key.clone();
    let mut key_arr = [0u8; 32];
    key_arr.copy_from_slice(key_bytes);
    let start_cursor = cursor.clone();
    let join = tokio::task::spawn_blocking(move || {
        let db_guard = db_arc.blocking_lock();
        let mut synced = 0u32;
        // Highest `(wall_time, id)` observed in this batch — used to advance the
        // forward cursor even for rows that were de-duped or failed to decrypt,
        // so we never re-request them on the next tick. Ordering matches the
        // query's `(wall_time, id)` sort.
        let mut batch_max: (i64, String) = (start_cursor.wall, start_cursor.id.clone());
        for row in rows {
            let Some(id) = row["id"].as_str() else {
                continue;
            };
            let Some(item_id) = row["item_id"].as_str() else {
                continue;
            };
            // Advance the batch cursor for EVERY row we can read — including ones
            // we skip below (already present, undecryptable) — so the next poll's
            // keyset filter does not re-request them.
            let row_wall = row["wall_time"].as_i64().unwrap_or(0);
            if (row_wall, id.to_owned()) > batch_max {
                batch_max = (row_wall, id.to_owned());
            }
            // LWW dedup keyed on the cross-device `item_id` (NOT the per-row
            // `id`, which differs across devices for the same logical item). If
            // the item is already present locally, route it through an LWW
            // resolve instead of inserting a duplicate or unconditionally
            // dropping it: a strictly-newer remote `lamport_ts` must win so a
            // cloud edit propagates, while an older/equal one is skipped.
            let existing = match get_item_by_item_id(&db_guard, item_id) {
                Ok(row) => row,
                Err(e) => {
                    tracing::warn!(
                        "cloud-sync: get_item_by_item_id error for item_id={item_id}: {e}"
                    );
                    continue;
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
                    continue;
                }
                // Remote wins LWW: replace in place, preserving the local PK so
                // FTS / copy_item / pins keep pointing at the same row.
                Some(local.id.clone())
            } else {
                // Defensive: also honour a same-`id` row that somehow lacks the
                // matching item_id (legacy rows) so we never double-insert.
                match exists_item_by_item_id(&db_guard, item_id) {
                    Ok(true) => continue,
                    Ok(false) => None,
                    Err(e) => {
                        tracing::warn!(
                            "cloud-sync: exists_item_by_item_id error for item_id={item_id}: {e}"
                        );
                        continue;
                    }
                }
            };

            // ── Tombstone fast-path ──────────────────────────────────────────
            // If the remote row carries `deleted = true` the remote device has
            // soft-deleted this item. Apply the deletion locally as a tombstone
            // (soft-delete: wipe content, set deleted=1, propagate via LWW) so
            // the item cannot resurrect on this device or re-broadcast incorrectly.
            // The cursor still advances (batch_max was updated above) so tombstones
            // are never re-requested.
            if remote_deleted {
                let remote_wall = row_wall;
                if let Some(local_pk) = preserved_pk.as_ref() {
                    match soft_delete_item(&db_guard, local_pk, remote_lamport, remote_wall) {
                        Ok(n) if n > 0 => {
                            synced += 1;
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
                        &db_guard,
                        item_id,
                        item_id,
                        remote_lamport,
                        remote_wall,
                        remote_origin,
                    ) {
                        Ok(_) => {
                            synced += 1;
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
                continue;
            }

            // Decode payload_ct (base64 → bytes).
            let payload_ct_b64 = match row["payload_ct"].as_str() {
                Some(s) => s,
                None => {
                    tracing::warn!("cloud-sync: row id={id} missing payload_ct; skipping");
                    continue;
                }
            };
            let blob = match decode_payload_ct(payload_ct_b64) {
                Ok(b) => b,
                Err(e) => {
                    tracing::warn!(
                        "cloud-sync: payload_ct decode failed for id={id}: {e}; skipping"
                    );
                    continue;
                }
            };

            // Decrypt with sync key (AAD = item_id + schema v5).
            // On failure: skip, warn, NEVER log the blob or key.
            //
            // We snapshot the sync key bytes before entering spawn_blocking
            // (SyncKey is not Send across the async boundary). Reconstruct a
            // temporary SyncKey via `from_bytes` so the canonical
            // `decrypt_from_cloud` code path is used — same AEAD parameters as
            // upload.
            let plaintext = {
                let tmp_key = SyncKey::from_bytes(key_arr);
                match decrypt_from_cloud(&tmp_key, item_id, &blob) {
                    Ok(p) => p,
                    Err(e) => {
                        // Never log plaintext or the key.
                        tracing::warn!(
                            "cloud-sync: decrypt_from_cloud failed for id={id} \
                             (wrong passphrase or tampered blob): {e}; skipping"
                        );
                        continue;
                    }
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
                &local_key_clone,
            ) {
                Ok(i) => i,
                Err(e) => {
                    tracing::warn!(
                        "cloud-sync: local re-encrypt failed for id={id}: {e}; skipping"
                    );
                    continue;
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
                replace_cloud_item_by_item_id(&db_guard, &local_item)
            } else {
                insert_item(&db_guard, &local_item).map_err(anyhow::Error::from)
            };
            match write_res {
                Ok(()) => {
                    synced += 1;
                    tracing::info!(
                        "cloud-sync: synced remote item_id={} (id={})",
                        local_item.item_id,
                        local_item.id
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        "cloud-sync: failed to store remote item_id={}: {e}",
                        local_item.item_id
                    );
                }
            }
        }
        // Zero the snapshot key bytes before the closure exits.
        zeroize::Zeroize::zeroize(&mut key_arr);
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
        if synced > 0 {
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
        (synced, new_cursor)
    });

    match join.await {
        Ok((synced, new_cursor)) => {
            if synced > 0 {
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

/// Outcome of a single `fetch_remote_rows` attempt.
///
/// Mirrors the push-side [`PushOutcome`]: the poll path needs to distinguish
/// "bearer expired" (refresh-and-retry), "rate-limited" (sleep Retry-After),
/// and every other failure (log + wait for the next tick).
pub(crate) enum FetchOutcome {
    /// 2xx — rows decoded successfully.
    Ok(Vec<serde_json::Value>),
    /// 401 — bearer expired or invalid. Caller should refresh and retry once.
    Unauthorized,
    /// 429 — rate-limited. `Option<Duration>` carries the `Retry-After` value
    /// (seconds form) when the server provided one.  Caller should sleep that
    /// duration (or a bounded backoff) before retrying rather than waiting the
    /// full poll interval, which would ignore the server's guidance.
    /// [P1 audit fix: poll 429 Retry-After handling]
    RateLimited(Option<Duration>),
    /// Any other failure (network, 5xx, non-401/429 4xx, JSON decode). The
    /// message is for logging only; retrying immediately will not help, so the
    /// caller just waits for the next poll tick.
    Failed(String),
}

/// `GET /rest/v1/clipboard_items` and return the raw JSON rows.
///
/// The caller is responsible for extracting and decrypting `payload_ct`.
///
/// A 401 is surfaced as [`FetchOutcome::Unauthorized`] (not folded into the
/// generic error) so the poll loop can refresh the bearer and retry — without
/// this, an expired GoTrue token permanently stalls *downloads* even though
/// uploads keep working (the push path already refreshes on 401).
pub(crate) async fn fetch_remote_rows(
    client: &reqwest::Client,
    url: &str,
    anon_key: &str,
    bearer: &str,
) -> FetchOutcome {
    let resp = match client
        .get(url)
        .header("apikey", anon_key)
        .header("Authorization", format!("Bearer {bearer}"))
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => return FetchOutcome::Failed(format!("send: {e}")),
    };

    let status = resp.status();
    if status.as_u16() == 401 {
        return FetchOutcome::Unauthorized;
    }
    // [P1 audit fix] Surface 429 as a distinct outcome so the caller can sleep
    // the Retry-After duration instead of folding it into a generic Failed and
    // waiting the full poll interval, which ignores the server's guidance.
    if status.as_u16() == 429 {
        let retry_after = parse_retry_after_secs(resp.headers());
        return FetchOutcome::RateLimited(retry_after);
    }
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        return FetchOutcome::Failed(format!("REST GET failed ({status}): {text}"));
    }

    match resp.json::<Vec<serde_json::Value>>().await {
        Ok(rows) => FetchOutcome::Ok(rows),
        Err(e) => FetchOutcome::Failed(format!("decode rows: {e}")),
    }
}

/// Fetch rows, transparently refreshing the shared bearer on a single 401.
///
/// This is the poll-side counterpart of the `Unauthorized` arm in
/// [`push_item_with_retries`]: the `refreshed` single-shot guard guarantees we
/// refresh-and-retry at most once per call, so a refresh that itself yields a
/// still-401 token cannot spin into an infinite loop — the second 401 falls
/// through to `FetchOutcome::Unauthorized` and is reported as an error.
pub(crate) async fn fetch_remote_rows_with_refresh(
    client: &reqwest::Client,
    url: &str,
    config: &CloudConfig,
    bearer: &Arc<RwLock<String>>,
    cloud_signed_in: &Arc<std::sync::atomic::AtomicBool>,
    auth: &AuthClient,
) -> Result<Vec<serde_json::Value>, String> {
    let mut refreshed = false;
    // Single-shot guard: honour Retry-After at most once per call so a
    // misbehaving server returning permanent 429 cannot pin this loop.
    let mut honoured_rate_limit_once = false;
    loop {
        let token = bearer.read().await.clone();
        match fetch_remote_rows(client, url, &config.anon_key, &token).await {
            FetchOutcome::Ok(rows) => return Ok(rows),
            FetchOutcome::Unauthorized if !refreshed => {
                refreshed = true;
                tracing::info!("cloud-sync poll got 401; refreshing bearer and retrying once");
                match refresh_bearer(config, cloud_signed_in, auth).await {
                    Ok(new_token) => {
                        *bearer.write().await = new_token;
                    }
                    Err(e) => return Err(format!("401 refresh failed: {e}")),
                }
                // Loop again with the refreshed token.
                continue;
            }
            FetchOutcome::Unauthorized => {
                return Err("401 Unauthorized (already refreshed once)".into());
            }
            // [P1 audit fix] Sleep Retry-After (or a bounded backoff) before
            // retrying rather than folding 429 into Failed and waiting the full
            // poll interval, which ignores the server's rate-limit guidance.
            FetchOutcome::RateLimited(retry_after) if !honoured_rate_limit_once => {
                honoured_rate_limit_once = true;
                let delay = retry_after
                    .unwrap_or(PUSH_INITIAL_BACKOFF)
                    .min(PUSH_MAX_BACKOFF);
                tracing::warn!(
                    "cloud-sync poll got 429; sleeping {:?} before retry (Retry-After: {:?})",
                    delay,
                    retry_after,
                );
                tokio::time::sleep(delay).await;
                continue;
            }
            FetchOutcome::RateLimited(_) => {
                return Err("429 Too Many Requests (already retried after Retry-After)".into());
            }
            FetchOutcome::Failed(msg) => return Err(msg),
        }
    }
}
