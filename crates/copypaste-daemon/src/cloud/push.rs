use std::collections::VecDeque;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};

use copypaste_core::{encrypt_for_cloud, ClipboardItem, Database, SyncKey};
use copypaste_supabase::auth::AuthClient;

use crate::sync_common::{
    decrypt_item_plaintext_blocking, wrap_and_check_cloud_upload_plaintext, SYNC_HTTP_TIMEOUT,
};

use super::auth::refresh_bearer;
use super::backlog::{run_backlog_sweep, run_tombstone_backlog_sweep};
use super::config::CloudConfig;
use super::ingest::clipboard_item_to_json;

// ── Push reliability tuning (Wave 2.7 edge #19/#20/#21) ───────────────────────

/// Maximum number of items the in-memory retry queue will hold before it starts
/// dropping the oldest entries. Bounded so a sustained outage cannot exhaust
/// daemon memory.
pub(crate) const PUSH_RETRY_QUEUE_CAP: usize = 1024;

/// Maximum delay between retry attempts for transient push failures.
pub(super) const PUSH_MAX_BACKOFF: Duration = Duration::from_secs(30);

/// Initial delay between retry attempts. Doubles on each failure up to
/// `PUSH_MAX_BACKOFF`.
pub(super) const PUSH_INITIAL_BACKOFF: Duration = Duration::from_secs(1);

/// How often the push loop drains pending broadcast-channel items into the
/// retry queue when the retry queue is non-empty (CopyPaste-1t38).
///
/// Without a periodic drain, pin/delete/new-item events sent to `new_item_tx`
/// while the loop is busy draining a failed retry queue accumulate in the
/// broadcast ring buffer. If the ring buffer fills (default capacity: 16), the
/// oldest events are silently dropped (Lagged). A 10-second drain interval
/// ensures that mutations are picked up even during a sustained cloud outage.
pub(crate) const MUTATION_QUEUE_DRAIN_INTERVAL: Duration = Duration::from_secs(10);

// ── Push loop helpers ────────────────────────────────────────────────────────

/// Decrypt a clipboard item's local ciphertext, re-encrypt it under the
/// current cloud sync key, and append the result to the retry queue.
///
/// Returns `true` when the item was enqueued, `false` when it was skipped
/// (sensitive item, no sync key, decrypt error, encrypt error).
///
/// This is extracted from the `push_loop` main `select!` branch so the SAME
/// processing pipeline can be reused by the periodic drain path
/// (CopyPaste-1t38): when the retry queue is non-empty and a new broadcast
/// item arrives during the retry-backoff sleep, we must enqueue it
/// immediately rather than let it age in the broadcast ring buffer.
///
/// # Safety / reentrancy
///
/// The caller holds no locks when calling this function — the function takes
/// and immediately releases the `sync_key` lock twice (once for the fast
/// no-key check, once for re-encryption). The intermediate plaintext is
/// zeroized at the end of `decrypt_item_plaintext_blocking`.
#[allow(clippy::too_many_arguments)]
pub(super) async fn prepare_and_enqueue_item(
    item: ClipboardItem,
    sync_key: &Arc<Mutex<Option<SyncKey>>>,
    local_key: &Arc<zeroize::Zeroizing<[u8; 32]>>,
    retry_queue: &mut VecDeque<(ClipboardItem, Option<String>)>,
    warned_no_key: &mut bool,
) -> bool {
    // P1-1: sensitive items are NEVER uploaded.
    if item.is_sensitive {
        tracing::debug!(
            "cloud-sync push_loop: skipping sensitive id={} (never uploaded)",
            item.id
        );
        return false;
    }
    // CopyPaste-e89n: tombstone items (soft-deleted) carry no content —
    // push them directly without decrypt/re-encrypt. The server stores
    // `deleted=true, payload_ct=NULL` so receiving devices apply the deletion.
    if item.deleted {
        enqueue_for_retry(retry_queue, item, None);
        return true;
    }
    // Fast no-key skip: if no sync passphrase is set there is nothing to
    // upload. Drop the guard immediately so the lock is not held across the
    // await below.
    {
        let key_guard = sync_key.lock().await;
        if key_guard.is_none() {
            if !*warned_no_key {
                tracing::warn!(
                    "cloud-sync push_loop: no sync passphrase set — \
                     skipping upload (call set_sync_passphrase first)"
                );
                *warned_no_key = true;
            }
            return false;
        }
    }
    // Decrypt on the blocking pool (CPU-bound, potentially multi-MB).
    let (item_back, decrypt_res) =
        decrypt_item_plaintext_blocking(item, zeroize::Zeroizing::new(***local_key)).await;
    let item = match item_back {
        Some(it) => it,
        None => {
            tracing::warn!("cloud-sync push_loop: decrypt task failed; skipping");
            return false;
        }
    };
    let plaintext = match decrypt_res {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(
                "cloud-sync push_loop: failed to decrypt id={} for re-encryption: {e}; skipping",
                item.id
            );
            return false;
        }
    };
    // Re-encrypt for cloud under the current sync key.
    let payload_ct_b64 = {
        let key_guard = sync_key.lock().await;
        match &*key_guard {
            None => {
                if !*warned_no_key {
                    tracing::warn!(
                        "cloud-sync push_loop: no sync passphrase set — \
                         skipping upload (call set_sync_passphrase first)"
                    );
                    *warned_no_key = true;
                }
                return false;
            }
            Some(key) => {
                let cloud_plaintext = match wrap_and_check_cloud_upload_plaintext(&item, plaintext)
                {
                    Ok(p) => p,
                    Err(e) => {
                        tracing::warn!("cloud-sync push_loop: skipping id={}: {e}", item.id);
                        return false;
                    }
                };
                match encrypt_for_cloud(key, &item.item_id, &cloud_plaintext) {
                    Ok(blob) => {
                        use base64::Engine as _;
                        base64::engine::general_purpose::STANDARD.encode(&blob)
                    }
                    Err(e) => {
                        tracing::warn!(
                            "cloud-sync push_loop: cloud encrypt failed for id={}: {e}; skipping",
                            item.id
                        );
                        return false;
                    }
                }
            }
        }
    };
    *warned_no_key = false;
    enqueue_for_retry(retry_queue, item, Some(payload_ct_b64));
    true
}

// ── Push loop ─────────────────────────────────────────────────────────────────

/// Receive locally created items from the broadcast channel and POST them to
/// `POST /rest/v1/clipboard_items`.
///
/// Wave 2.7 hardening:
/// - **#19 disconnect/reconnect**: items that fail to push are appended to an
///   in-memory retry queue (bounded by [`PUSH_RETRY_QUEUE_CAP`]). The queue is
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
#[allow(clippy::too_many_arguments)]
pub(super) async fn push_loop(
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
        let key_snapshot: Option<Vec<u8>> = {
            let guard = sync_key.lock().await;
            guard.as_ref().map(|k| k.as_bytes().to_vec())
        };
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
        let key_now: Option<Vec<u8>> = {
            let guard = sync_key.lock().await;
            guard.as_ref().map(|k| k.as_bytes().to_vec())
        };
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
            match push_item_with_retries(
                &client,
                &rest_url,
                &config,
                &bearer,
                &item,
                payload_ct_b64.as_deref(),
                Some(&cloud_signed_in),
                &auth,
            )
            .await
            {
                Ok(()) => {
                    tracing::info!(
                        "cloud-sync flushed queued id={} (retry queue drained one)",
                        item.id
                    );
                    // Fix CLOUD-IS_SYNCED: mark the row synced so restart
                    // backlog sweeps don't re-upload it.
                    mark_item_synced(&db, &item.item_id).await;
                    // Fix #33: stamp last_sync_ms on every successful push so
                    // get_sync_status returns a non-null timestamp even when
                    // no remote items were polled.
                    let now_ms = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as i64;
                    last_sync_ms.store(now_ms, Ordering::Relaxed);
                    continue;
                }
                Err(e) => {
                    tracing::warn!(
                        "cloud-sync still failing for id={} ({e}); re-queuing (queue_len={})",
                        item.id,
                        retry_queue.len() + 1,
                    );
                    // payload_ct_b64 is Option<String> — pass it directly back to the queue.
                    enqueue_for_retry(&mut retry_queue, item, payload_ct_b64);
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
                                match push_item_with_retries(
                                    &client,
                                    &rest_url,
                                    &config,
                                    &bearer,
                                    &item,
                                    payload_ct_b64.as_deref(),
                                    Some(&cloud_signed_in),
                                    &auth,
                                )
                                .await
                                {
                                    Ok(()) => {
                                        tracing::info!("cloud-sync pushed id={}", item.id);
                                        // Fix CLOUD-IS_SYNCED: mark the row synced.
                                        mark_item_synced(&db, &item.item_id).await;
                                        // Fix #33: update last_sync_ms on every successful push.
                                        let now_ms = std::time::SystemTime::now()
                                            .duration_since(std::time::UNIX_EPOCH)
                                            .unwrap_or_default()
                                            .as_millis() as i64;
                                        last_sync_ms.store(now_ms, Ordering::Relaxed);
                                    }
                                    Err(e) => {
                                        tracing::warn!(
                                            "cloud-sync push failed for id={}: {e}; queuing for retry",
                                            item.id
                                        );
                                        // payload_ct_b64 is Option<String> — pass directly back.
                                        enqueue_for_retry(&mut retry_queue, item, payload_ct_b64);
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

/// Append `(item, payload_ct_b64)` to the retry queue, evicting the oldest
/// entry when the queue is at capacity. Bounded so a long outage cannot exhaust
/// memory.
pub(crate) fn enqueue_for_retry(
    queue: &mut VecDeque<(ClipboardItem, Option<String>)>,
    item: ClipboardItem,
    payload_ct_b64: Option<String>,
) {
    if queue.len() >= PUSH_RETRY_QUEUE_CAP {
        if let Some((dropped, _)) = queue.pop_front() {
            tracing::warn!(
                "cloud-sync retry queue at cap ({}); dropping oldest id={}",
                PUSH_RETRY_QUEUE_CAP,
                dropped.id,
            );
        }
    }
    queue.push_back((item, payload_ct_b64));
}

/// Mark a row as successfully uploaded by setting `is_synced = 1`.
///
/// Fix CLOUD-IS_SYNCED: without this, `is_synced` stayed 0 forever, causing
/// the startup backlog sweep (`WHERE is_synced = 0`) to re-upload the entire
/// history on every daemon restart. Best-effort: a failed UPDATE is logged and
/// not retried — the row will simply appear in the next backlog sweep, which is
/// harmless (the server deduplicates by primary key).
async fn mark_item_synced(db: &Arc<Mutex<Database>>, item_id: &str) {
    let db_arc = db.clone();
    let id_owned = item_id.to_owned();
    // Run on the blocking pool — rusqlite is synchronous.
    let result = tokio::task::spawn_blocking(move || {
        let db = db_arc.blocking_lock();
        db.conn()
            .execute(
                "UPDATE clipboard_items SET is_synced = 1 WHERE item_id = ?1",
                rusqlite::params![id_owned],
            )
            .map_err(|e| e.to_string())
    })
    .await;
    match result {
        Ok(Ok(rows)) => {
            if rows == 0 {
                // Row may have been deleted between push and update — benign.
                tracing::debug!("mark_item_synced: no row updated for item_id={item_id}");
            }
        }
        Ok(Err(e)) => {
            tracing::warn!("mark_item_synced: UPDATE failed for item_id={item_id}: {e}");
        }
        Err(e) => {
            tracing::warn!("mark_item_synced: blocking task panicked for item_id={item_id}: {e}");
        }
    }
}

/// Outcome of a single push attempt.
#[derive(Debug)]
enum PushOutcome {
    /// 2xx — accepted by the server.
    Ok,
    /// 401 — bearer expired or invalid. Caller should refresh and retry once.
    Unauthorized,
    /// 429 — rate-limited. The `Option<Duration>` carries the `Retry-After`
    /// value if the server provided one (in seconds form).
    RateLimited(Option<Duration>),
    /// Network or 5xx error. Transient; caller should back off and requeue.
    Transient(String),
    /// 4xx other than 401/429 — request is malformed or rejected for a reason
    /// retrying will not fix. Caller should give up on this item.
    Permanent(String),
}

/// One push attempt, surfacing structured outcomes so the caller can decide
/// between refresh, backoff, and abort.
///
/// `payload_ct_b64` is the base64-encoded cloud ciphertext (nonce||ciphertext)
/// produced by `encrypt_for_cloud`. It is pre-computed by the push loop so
/// re-encryption only happens once even when the attempt is retried. For
/// tombstone rows (`item.deleted == true`) this is `None` — the server stores
/// `payload_ct = NULL` and receiving devices apply a soft-delete.
async fn push_item_once(
    client: &reqwest::Client,
    url: &str,
    anon_key: &str,
    bearer: &str,
    item: &ClipboardItem,
    // `None` for tombstone rows (item.deleted == true); `Some(b64)` for live items.
    payload_ct_b64: Option<&str>,
) -> PushOutcome {
    let body = clipboard_item_to_json(item, payload_ct_b64);

    let resp = match client
        .post(url)
        .header("apikey", anon_key)
        .header("Authorization", format!("Bearer {bearer}"))
        .header("Content-Type", "application/json")
        .header("Prefer", "return=minimal")
        .json(&body)
        .send()
        .await
    {
        Ok(r) => r,
        // Network / DNS / TLS / connection-refused → transient.
        Err(e) => return PushOutcome::Transient(format!("send: {e}")),
    };

    let status = resp.status();
    if status.is_success() {
        return PushOutcome::Ok;
    }
    if status.as_u16() == 401 {
        return PushOutcome::Unauthorized;
    }
    if status.as_u16() == 429 {
        let retry_after = parse_retry_after_secs(resp.headers());
        return PushOutcome::RateLimited(retry_after);
    }
    let text = resp.text().await.unwrap_or_default();
    if status.is_server_error() {
        return PushOutcome::Transient(format!("{status}: {text}"));
    }
    PushOutcome::Permanent(format!("{status}: {text}"))
}

/// Parse the HTTP `Retry-After` header in its delta-seconds form. We
/// deliberately do NOT support the HTTP-date variant — Supabase emits the
/// integer-seconds form and supporting both pulls in a date-parsing dep for
/// no operator benefit.
pub(crate) fn parse_retry_after_secs(headers: &reqwest::header::HeaderMap) -> Option<Duration> {
    headers
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.trim().parse::<u64>().ok())
        .map(Duration::from_secs)
}

/// Compose the per-item push pipeline:
/// - try once;
/// - on `Unauthorized` → refresh the shared bearer (Wave 2.7 #20) and retry
///   exactly once;
/// - on `RateLimited(Some(d))` → honour `Retry-After` and retry once
///   (Wave 2.7 #21);
/// - on `Transient` → exponential backoff between attempts, capped at
///   `PUSH_MAX_BACKOFF`;
/// - on `Permanent` → abort and surface the error.
///
/// Returns `Ok(())` on 2xx, `Err(msg)` for permanent failures or after the
/// transient-retry budget is exhausted. Callers (the push loop) then decide
/// whether to requeue.
///
/// `cloud_signed_in` is the shared auth-state flag (BUG 2). When the 401 path
/// refreshes the bearer, a successful refresh keeps it `true` and a failed
/// refresh flips it `false`. `None` is accepted for callers/tests that do not
/// track auth state.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn push_item_with_retries(
    client: &reqwest::Client,
    url: &str,
    config: &CloudConfig,
    bearer: &Arc<RwLock<String>>,
    item: &ClipboardItem,
    // `None` for tombstone rows (item.deleted == true); `Some(b64)` for live items.
    payload_ct_b64: Option<&str>,
    cloud_signed_in: Option<&Arc<std::sync::atomic::AtomicBool>>,
    auth: &AuthClient,
) -> Result<(), String> {
    // A throwaway flag for the `None` case so `refresh_bearer` always has a
    // target to write — its write is then simply ignored by the caller.
    let scratch_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let signed_in = cloud_signed_in.unwrap_or(&scratch_flag);
    let mut backoff = PUSH_INITIAL_BACKOFF;
    // Hard cap on attempts to avoid hot loops even if every attempt comes back
    // as `Transient(_)`. The loop body sleeps between attempts so the worst-case
    // duration is bounded by the sum of backoffs.
    let max_transient_attempts: u8 = 4;
    let mut transient_attempts: u8 = 0;
    // `Unauthorized` may only trigger ONE refresh-and-retry per item to
    // avoid an infinite loop if the refresh itself returns a still-401 token.
    let mut refreshed_once = false;
    // Same single-shot guard for `Retry-After` so a misconfigured server
    // returning permanent 429 cannot pin us forever.
    let mut honoured_retry_after_once = false;

    loop {
        let token = bearer.read().await.clone();
        match push_item_once(client, url, &config.anon_key, &token, item, payload_ct_b64).await {
            PushOutcome::Ok => return Ok(()),

            PushOutcome::Unauthorized if !refreshed_once => {
                refreshed_once = true;
                tracing::info!("cloud-sync got 401; refreshing bearer and retrying once");
                match refresh_bearer(config, signed_in, auth).await {
                    Ok(new_token) => {
                        *bearer.write().await = new_token;
                    }
                    Err(e) => {
                        return Err(format!("401 refresh failed: {e}"));
                    }
                }
                // Loop again with the refreshed token.
                continue;
            }
            PushOutcome::Unauthorized => {
                return Err("401 Unauthorized (already refreshed once)".into());
            }

            PushOutcome::RateLimited(retry_after) if !honoured_retry_after_once => {
                honoured_retry_after_once = true;
                let delay = retry_after.unwrap_or(backoff).min(PUSH_MAX_BACKOFF);
                tracing::warn!(
                    "cloud-sync got 429; sleeping {:?} before retry (Retry-After: {:?})",
                    delay,
                    retry_after,
                );
                tokio::time::sleep(delay).await;
                continue;
            }
            PushOutcome::RateLimited(_) => {
                return Err("429 Too Many Requests (already retried after Retry-After)".into());
            }

            PushOutcome::Transient(msg) => {
                transient_attempts += 1;
                if transient_attempts >= max_transient_attempts {
                    return Err(format!(
                        "transient failure budget exhausted after {transient_attempts} attempts: {msg}"
                    ));
                }
                tracing::warn!(
                    "cloud-sync transient failure ({msg}); backing off {:?} (attempt {}/{})",
                    backoff,
                    transient_attempts,
                    max_transient_attempts,
                );
                tokio::time::sleep(backoff).await;
                backoff = (backoff * 2).min(PUSH_MAX_BACKOFF);
                continue;
            }

            PushOutcome::Permanent(msg) => return Err(msg),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use copypaste_core::ClipboardItem;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    fn text_item(lamport: i64) -> ClipboardItem {
        // new_text defaults is_sensitive=false, deleted=false.
        ClipboardItem::new_text(vec![1, 2, 3], vec![0u8; 24], lamport)
    }

    /// CopyPaste-20yw / P1-1: a SENSITIVE item must never be enqueued for cloud
    /// upload — `prepare_and_enqueue_item` returns false and leaves the retry
    /// queue empty, BEFORE any key/decrypt/network work. This is a real guard
    /// test: the positive control below proves removing the guard would let the
    /// item through (the previous coverage was a tautology elsewhere).
    #[tokio::test]
    async fn cloud_push_skips_sensitive_item() {
        let sync_key = Arc::new(Mutex::new(None));
        let local_key = Arc::new(zeroize::Zeroizing::new([0u8; 32]));
        let mut queue = VecDeque::new();
        let mut warned = false;

        let mut item = text_item(1);
        item.is_sensitive = true;
        // Even a sensitive TOMBSTONE must be skipped (the sensitive guard runs
        // before the deleted fast-path).
        item.deleted = true;

        let enqueued =
            prepare_and_enqueue_item(item, &sync_key, &local_key, &mut queue, &mut warned).await;

        assert!(!enqueued, "sensitive item must not be enqueued");
        assert!(
            queue.is_empty(),
            "sensitive item must not enter the retry queue"
        );
    }

    /// Positive control: a NON-sensitive tombstone (deleted) item is enqueued
    /// directly (no key/crypto needed). This proves the zero above is the
    /// sensitive guard at work, not a broken setup — and that removing the guard
    /// would make the sensitive tombstone above take this same path and fail the
    /// assertion.
    #[tokio::test]
    async fn cloud_push_enqueues_non_sensitive_tombstone() {
        let sync_key = Arc::new(Mutex::new(None));
        let local_key = Arc::new(zeroize::Zeroizing::new([0u8; 32]));
        let mut queue = VecDeque::new();
        let mut warned = false;

        let mut item = text_item(2);
        item.is_sensitive = false;
        item.deleted = true;

        let enqueued =
            prepare_and_enqueue_item(item, &sync_key, &local_key, &mut queue, &mut warned).await;

        assert!(enqueued, "non-sensitive tombstone must be enqueued");
        assert_eq!(queue.len(), 1, "tombstone must enter the retry queue");
    }
}
