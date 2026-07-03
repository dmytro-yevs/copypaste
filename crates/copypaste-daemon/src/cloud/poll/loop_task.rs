use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};

use copypaste_core::{Database, SyncKey};
use copypaste_supabase::auth::AuthClient;

use crate::sync_common::SYNC_HTTP_TIMEOUT;
use crate::sync_in_flight::SyncInFlightGuard;

use super::super::config::CloudConfig;
use super::cursor::{load_poll_watermark, PollCursor};
use super::ingest::poll_once;
use super::{POLL_BATCH_SIZE, POLL_INTERVAL_WS_CONNECTED, POLL_INTERVAL_WS_FALLBACK};

// Realtime loop handles: config, bearer, db, shutdown, notifier, signed_in,
// and ingest_tx — each is an independent runtime dependency with no natural
// grouping short of a new private struct.
// `pub(in super::super)`: visible to `cloud` — this fn moved one directory
// level deeper (into `cloud::poll::loop_task`), so it needs one extra `super`
// to reach the same `cloud`-wide audience the flat `poll.rs` file exposed;
// consumed by `cloud::lifecycle::start_cloud`.
#[allow(clippy::too_many_arguments)]
pub(in super::super) async fn realtime_loop(
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
                let (sync_on_wifi_only, storage_quota_bytes, max_decoded_image_mb) = {
                    let defaults = copypaste_core::AppConfig::default();
                    core_config
                        .read()
                        .map(|g| {
                            (
                                g.sync_on_wifi_only,
                                g.storage_quota_bytes,
                                g.max_decoded_image_mb,
                            )
                        })
                        .unwrap_or((
                            false,
                            defaults.storage_quota_bytes,
                            defaults.max_decoded_image_mb,
                        ))
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


                // If no sync key is set, skip with a one-time warning. Otherwise
                // snapshot the single per-account key bytes for this poll round.
                let Some(key_bytes) = super::super::snapshot_cloud_key_bytes(&sync_key).await else {
                    if !warned_no_key {
                        tracing::warn!(
                            "cloud-sync poll: no sync passphrase set — \
                             skipping download (call set_sync_passphrase first)"
                        );
                        warned_no_key = true;
                    }
                    continue;
                };
                warned_no_key = false;

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
                        max_decoded_image_mb,
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
