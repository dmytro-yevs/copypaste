use std::collections::VecDeque;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};

use copypaste_core::{ClipboardItem, Database, SyncKey};
use copypaste_supabase::auth::AuthClient;

use crate::sync_common::SYNC_HTTP_TIMEOUT;
use crate::sync_in_flight::SyncInFlightGuard;

use super::super::backlog::{run_backlog_sweep, run_tombstone_backlog_sweep};
use super::super::config::CloudConfig;
use super::prepare::prepare_and_enqueue_item;
use super::queue::{enqueue_for_retry, mark_item_synced};
use super::transport::push_item_with_retries;
use super::MUTATION_QUEUE_DRAIN_INTERVAL;
use super::PUSH_INITIAL_BACKOFF;

/// Attempt to push a single dequeued item: apply the bandwidth throttle, POST
/// it via [`push_item_with_retries`], then record the outcome.
///
/// On success, marks the row synced (`mark_item_synced`) and stamps
/// `last_sync_ms`. On failure, re-enqueues the item (with its already-computed
/// ciphertext) so the next drain pass retries it. This is the "attempt push →
/// record outcome" pipeline that used to be copy-pasted twice in `push_loop`
/// (once for the retry-queue drain, once for a freshly-enqueued broadcast
/// item) — the two call sites differ only in their logging message and
/// whether they `continue` the outer loop, so those two details stay at the
/// call site while this helper owns the shared throttle/push/record logic.
#[allow(clippy::too_many_arguments)]
async fn attempt_push_and_record(
    client: &reqwest::Client,
    rest_url: &str,
    config: &CloudConfig,
    bearer: &Arc<RwLock<String>>,
    cloud_signed_in: &Arc<std::sync::atomic::AtomicBool>,
    auth: &AuthClient,
    db: &Arc<Mutex<Database>>,
    last_sync_ms: &Arc<std::sync::atomic::AtomicI64>,
    sync_in_flight: &std::sync::Arc<std::sync::atomic::AtomicBool>,
    core_config: &Arc<std::sync::RwLock<copypaste_core::AppConfig>>,
    bw_bucket: &mut crate::bandwidth::TokenBucket,
    retry_queue: &mut VecDeque<(ClipboardItem, Option<String>)>,
    item: ClipboardItem,
    payload_ct_b64: Option<String>,
) -> Result<(), String> {
    // crh3.107: pace the outbound upload (0 = unlimited, no sleep).
    {
        let kbps = core_config
            .read()
            .map(|g| g.max_bandwidth_kbps)
            .unwrap_or(0);
        bw_bucket.set_rate_kbps(kbps);
        let byte_count = payload_ct_b64.as_deref().map_or(0, |s| s.len()) as u64;
        let delay = bw_bucket.acquire(byte_count);
        if !delay.is_zero() {
            tracing::debug!(
                "cloud-sync push_loop: bandwidth throttle {delay:?} for id={} ({byte_count} B)",
                item.id,
            );
            tokio::time::sleep(delay).await;
        }
    }
    // CopyPaste-1jms.22: arm the in-flight guard for this push round-trip so
    // get_sync_status emits SyncBadgeState::Syncing.
    let _push_guard = SyncInFlightGuard::new(std::sync::Arc::clone(sync_in_flight));
    match push_item_with_retries(
        client,
        rest_url,
        config,
        bearer,
        &item,
        payload_ct_b64.as_deref(),
        Some(cloud_signed_in),
        auth,
    )
    .await
    {
        Ok(()) => {
            // Fix CLOUD-IS_SYNCED: mark the row synced so restart backlog
            // sweeps don't re-upload it.
            mark_item_synced(db, &item.item_id).await;
            // Fix #33: stamp last_sync_ms on every successful push so
            // get_sync_status returns a non-null timestamp even when no
            // remote items were polled.
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64;
            last_sync_ms.store(now_ms, Ordering::Relaxed);
            Ok(())
        }
        Err(e) => {
            // payload_ct_b64 is Option<String> — pass it directly back to the queue.
            enqueue_for_retry(retry_queue, item, payload_ct_b64);
            Err(e)
        }
    }
}

/// Receive locally created items from the broadcast channel and POST them to
/// `POST /rest/v1/clipboard_items`.
///
/// Wave 2.7 hardening:
/// - **#19 disconnect/reconnect**: items that fail to push are appended to an
///   in-memory retry queue (bounded by [`super::PUSH_RETRY_QUEUE_CAP`]). The queue is
///   drained between fresh broadcast receives, so when connectivity returns we
///   flush backlog before accepting new work.
/// - **#20 401 refresh**: `push_item_with_retries` refreshes the shared bearer
///   token on a 401 and retries the request once.
/// - **#21 429 Retry-After**: the helper honours `Retry-After` (seconds form)
///   and otherwise applies bounded exponential backoff (1s → 30s).
///
/// Fix CLOUD-BACKLOG #33: on startup the loop loads ALL existing local items
/// (not yet synced, i.e. `is_synced = 0`) and enqueues them in the retry queue
/// so the existing history uploads to Supabase, not only future captures.
/// `last_sync_ms` is now stamped after every successful push (not just polls)
/// so the UI sees a non-null `last_sync_ms` even when there are no new remote
/// items to poll.
// `pub(in super::super)`: visible to `cloud` — this fn moved one directory
// level deeper (into `cloud::push::loop_task`), so it needs one extra `super`
// to reach the same `cloud`-wide audience the flat `push.rs` file exposed;
// consumed by `cloud::lifecycle::start_cloud`.
#[allow(clippy::too_many_arguments)]
pub(in super::super) async fn push_loop(
    config: CloudConfig,
    bearer: Arc<RwLock<String>>,
    mut rx: tokio::sync::broadcast::Receiver<ClipboardItem>,
    shutdown: Arc<tokio::sync::Notify>,
    sync_key: Arc<Mutex<Option<SyncKey>>>,
    local_key: Arc<zeroize::Zeroizing<[u8; 32]>>,
    db: Arc<Mutex<Database>>,
    last_sync_ms: Arc<std::sync::atomic::AtomicI64>,
    cloud_signed_in: Arc<std::sync::atomic::AtomicBool>,
    auth: Arc<AuthClient>,
    // Live core config for hot-reload of sync_on_wifi_only (A-SET-2).
    core_config: Arc<std::sync::RwLock<copypaste_core::AppConfig>>,
    // CopyPaste-1jms.22: shared in-flight flag for SyncBadgeState::Syncing.
    sync_in_flight: std::sync::Arc<std::sync::atomic::AtomicBool>,
) {
    // P1: set a per-request timeout so a stalled Supabase endpoint cannot hang
    // this loop indefinitely. See `sync_common::SYNC_HTTP_TIMEOUT` for rationale.
    //
    // CopyPaste-16vr: propagate builder error rather than falling back to a
    // no-timeout client (which would be worse than the builder failure itself).
    // TLS cert-store load cannot fail on macOS/Linux in normal operation.
    let client = reqwest::Client::builder()
        .timeout(SYNC_HTTP_TIMEOUT)
        .build()
        .expect("reqwest Client::builder should not fail on supported platforms");
    let rest_url = format!("{}/rest/v1/clipboard_items", config.supabase_url);
    // Track whether we've already warned about a missing sync key so we don't
    // spam the log on every item in a burst.
    let mut warned_no_key = false;
    // crh3.107: one token bucket per push loop (not shared; no lock needed).
    // Starts unlimited; rate is updated from the live config on each item so
    // hot-reload of max_bandwidth_kbps takes effect without a restart.
    let mut bw_bucket = crate::bandwidth::TokenBucket::new(0);

    // In-memory retry queue: (item, pre-computed payload_ct_b64).
    // The cloud ciphertext is stored alongside the item so re-encryption
    // does NOT happen on each retry attempt — the same blob is re-sent until
    // it succeeds or is evicted by the capacity cap.
    let mut retry_queue: VecDeque<(ClipboardItem, Option<String>)> = VecDeque::new();

    // ── Startup backlog push (fix #33) ────────────────────────────────────────
    // Load all syncable items that have not yet been synced (`is_synced = 0`)
    // and queue them; the main loop below drains the retry queue before
    // accepting new broadcast items, so existing history flows to Supabase
    // first, in chronological order.
    //
    // BUG C2: if no sync passphrase is set at startup the sweep is a no-op here,
    // but we re-run it inside the loop on the first None→Some key transition
    // (see `prev_key_present` below), so the "start daemon, then enter
    // passphrase" flow no longer strands the existing history.
    let key_present_at_start = {
        // Sweep + re-encrypt the startup backlog under the single per-account
        // sync key so existing history uploads under the same key new captures use.
        let key_snapshot: Option<[u8; 32]> =
            super::super::snapshot_cloud_key_bytes(&sync_key).await;
        match key_snapshot {
            Some(key_bytes) => {
                run_backlog_sweep(&db, &local_key, &key_bytes, &mut retry_queue).await;
                run_tombstone_backlog_sweep(&db, &mut retry_queue).await;
                true
            }
            None => {
                tracing::debug!(
                    "cloud-sync backlog: no sync passphrase set at startup — \
                     skipping backlog pre-load (will re-sweep when a passphrase is set)"
                );
                false
            }
        }
    };

    // BUG C2: track sync-key presence across iterations so we can detect a
    // None→Some transition (passphrase entered after startup) and run the
    // backlog sweep exactly once on that edge, rather than every tick.
    let mut prev_key_present = key_present_at_start;

    // CopyPaste-1t38: periodic drain interval.
    //
    // When the retry queue is non-empty the main loop's `select!` (which reads
    // from `rx`) is never reached.  Broadcast-channel items sent during this
    // period (pin, delete, new clipboard captures) accumulate in the ring buffer
    // and are silently dropped when the buffer fills (Lagged).  A periodic
    // interval ensures we drain `rx` into the retry queue even during a
    // sustained cloud outage by adding `rx.recv()` as an additional arm in the
    // retry-failure backoff select.
    let mut mutation_drain_tick = tokio::time::interval(MUTATION_QUEUE_DRAIN_INTERVAL);
    // Skip the first tick (fires immediately on creation) so we don't
    // spuriously drain on the very first loop iteration.
    mutation_drain_tick.tick().await;

    // Audit-concurrency HIGH #1 — `broadcast::Receiver::recv` is documented
    // cancellation-safe. We park each item in the retry queue immediately upon
    // receipt (before any network await), so if `shutdown.notified()` fires
    // between dequeue and push the item is visible in the retry-queue log and
    // not silently dropped.
    loop {
        // BUG C2: detect a None→Some sync-key transition (passphrase entered
        // after the daemon started) and run the backlog sweep ONCE on that edge.
        // Without this, history captured before the passphrase was set never
        // uploads until each item is re-copied. We snapshot the key bytes under
        // the lock, then release it before the (awaiting) sweep.
        let key_now: Option<[u8; 32]> = super::super::snapshot_cloud_key_bytes(&sync_key).await;
        let key_present_now = key_now.is_some();
        if key_present_now && !prev_key_present {
            if let Some(key_bytes) = key_now.as_ref() {
                tracing::info!(
                    "cloud-sync: sync passphrase became available after startup — \
                     running backlog sweep once"
                );
                run_backlog_sweep(&db, &local_key, key_bytes, &mut retry_queue).await;
                run_tombstone_backlog_sweep(&db, &mut retry_queue).await;
            }
        }
        // Update the edge tracker every tick (covers both →true and →false) so
        // a later None→Some flip (e.g. clear then re-enter) sweeps again, but a
        // steady Some state never re-sweeps.
        prev_key_present = key_present_now;

        // tke7 (PG-30): hot-reload master sync gate for cloud push.
        let sync_enabled = core_config.read().map(|g| g.sync_enabled).unwrap_or(true);
        if !sync_enabled {
            tracing::debug!("cloud-sync push_loop: sync_enabled=false; skipping this push cycle");
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(10)) => {}
                _ = shutdown.notified() => { break; }
            }
            continue;
        }

        // A-SET-2 hot-reload: read sync_on_wifi_only from the live config on
        // every iteration so a runtime change via set_config takes effect
        // immediately without a daemon restart.  Items remain in the retry
        // queue and new broadcasts continue to accumulate; they'll be pushed
        // once Wi-Fi is restored.
        let sync_on_wifi_only = core_config
            .read()
            .map(|g| g.sync_on_wifi_only)
            .unwrap_or(false);
        if sync_on_wifi_only
            && !tokio::task::spawn_blocking(crate::platform::is_on_wifi)
                .await
                .unwrap_or(true)
        {
            tracing::debug!(
                "cloud-sync push_loop: sync_on_wifi_only=true and not on Wi-Fi; \
                 sleeping 10s before retry"
            );
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(10)) => {}
                _ = shutdown.notified() => { break; }
            }
            continue;
        }

        // Drain the retry queue first — if we made progress on backlog before
        // touching new items, recovery is observable and old items are not
        // perpetually starved by a steady stream of new work.
        if let Some((item, payload_ct_b64)) = retry_queue.pop_front() {
            // Capture the id before the item is moved into the shared
            // attempt/record helper — `RowId` is `Clone + Display`, not `Copy`.
            let id = item.id.clone();
            match attempt_push_and_record(
                &client,
                &rest_url,
                &config,
                &bearer,
                &cloud_signed_in,
                &auth,
                &db,
                &last_sync_ms,
                &sync_in_flight,
                &core_config,
                &mut bw_bucket,
                &mut retry_queue,
                item,
                payload_ct_b64,
            )
            .await
            {
                Ok(()) => {
                    tracing::info!(
                        "cloud-sync flushed queued id={} (retry queue drained one)",
                        id
                    );
                    continue;
                }
                Err(e) => {
                    tracing::warn!(
                        "cloud-sync still failing for id={} ({e}); re-queuing (queue_len={})",
                        id,
                        retry_queue.len(),
                    );
                    // CopyPaste-1t38: yield and also drain any pending broadcast
                    // items so pin/delete/new-capture events sent while the retry
                    // loop is busy don't age out of the broadcast ring buffer.
                    tokio::select! {
                        _ = tokio::time::sleep(PUSH_INITIAL_BACKOFF) => {}
                        _ = shutdown.notified() => {
                            tracing::info!(
                                "cloud-sync push_loop: shutdown received during retry drain ({} queued items not flushed)",
                                retry_queue.len(),
                            );
                            return;
                        }
                        result = rx.recv() => {
                            // A new item arrived while we were backing off.
                            // Enqueue it immediately so it doesn't sit in the
                            // ring buffer and risk being dropped under Lagged.
                            match result {
                                Ok(incoming) => {
                                    prepare_and_enqueue_item(
                                        incoming,
                                        &sync_key,
                                        &local_key,
                                        &mut retry_queue,
                                        &mut warned_no_key,
                                    )
                                    .await;
                                }
                                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                                    tracing::warn!(
                                        "cloud-sync push_loop: lagged by {n} items during retry drain"
                                    );
                                }
                                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                                    tracing::info!(
                                        "cloud-sync push_loop: channel closed during retry drain \
                                         ({} queued items not flushed)",
                                        retry_queue.len(),
                                    );
                                    return;
                                }
                            }
                        }
                        _ = mutation_drain_tick.tick() => {
                            // Periodic drain: pull any queued broadcast items
                            // into the retry queue using try_recv so we don't
                            // block on an empty channel.
                            loop {
                                match rx.try_recv() {
                                    Ok(incoming) => {
                                        prepare_and_enqueue_item(
                                            incoming,
                                            &sync_key,
                                            &local_key,
                                            &mut retry_queue,
                                            &mut warned_no_key,
                                        )
                                        .await;
                                    }
                                    Err(tokio::sync::broadcast::error::TryRecvError::Empty) => {
                                        break;
                                    }
                                    Err(
                                        tokio::sync::broadcast::error::TryRecvError::Lagged(n),
                                    ) => {
                                        tracing::warn!(
                                            "cloud-sync push_loop: lagged by {n} items \
                                             during periodic drain"
                                        );
                                        // Continue draining after lag.
                                    }
                                    Err(
                                        tokio::sync::broadcast::error::TryRecvError::Closed,
                                    ) => {
                                        tracing::info!(
                                            "cloud-sync push_loop: channel closed during \
                                             periodic drain ({} queued items not flushed)",
                                            retry_queue.len(),
                                        );
                                        return;
                                    }
                                }
                            }
                        }
                    }
                    continue;
                }
            }
        }

        tokio::select! {
            // biased: prefer shutdown over receive so a burst of incoming items
            // cannot starve teardown.
            biased;
            _ = shutdown.notified() => {
                tracing::info!(
                    "cloud-sync push_loop: shutdown received ({} queued items not flushed)",
                    retry_queue.len(),
                );
                break;
            }
            result = rx.recv() => {
                match result {
                    Ok(item) => {
                        // Encrypt and enqueue using the shared helper.  The helper
                        // handles P1-1 (sensitive skip), the no-key fast path,
                        // CopyPaste-z1xt (blocking decrypt), and cloud re-encrypt.
                        let enqueued = prepare_and_enqueue_item(
                            item,
                            &sync_key,
                            &local_key,
                            &mut retry_queue,
                            &mut warned_no_key,
                        )
                        .await;
                        if enqueued {
                            // Try to push the newly-enqueued item immediately.
                            // Park it first (already in retry_queue) then pop
                            // from the front so older queued items drain first.
                            if let Some((item, payload_ct_b64)) = retry_queue.pop_front() {
                                let id = item.id.clone();
                                match attempt_push_and_record(
                                    &client,
                                    &rest_url,
                                    &config,
                                    &bearer,
                                    &cloud_signed_in,
                                    &auth,
                                    &db,
                                    &last_sync_ms,
                                    &sync_in_flight,
                                    &core_config,
                                    &mut bw_bucket,
                                    &mut retry_queue,
                                    item,
                                    payload_ct_b64,
                                )
                                .await
                                {
                                    Ok(()) => {
                                        tracing::info!("cloud-sync pushed id={}", id);
                                    }
                                    Err(e) => {
                                        tracing::warn!(
                                            "cloud-sync push failed for id={}: {e}; queuing for retry",
                                            id
                                        );
                                    }
                                }
                            }
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("cloud-sync push_loop: lagged by {n} items");
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        tracing::info!(
                            "cloud-sync push_loop: channel closed, exiting (dropping {} queued items)",
                            retry_queue.len(),
                        );
                        break;
                    }
                }
            }
        }
    }
}
