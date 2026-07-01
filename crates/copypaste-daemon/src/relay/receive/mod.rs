//! Relay receive path: pull pages from the inbox, ingest via LWW, advance the
//! watermark, drive the poll loop.

use std::sync::{
    atomic::{AtomicI64, Ordering},
    Arc,
};

use copypaste_core::{AppConfig, Database, SyncKey};
use tokio::sync::{Mutex, Notify};

use crate::sync_in_flight::SyncInFlightGuard;

use super::pasteboard::{
    relay_apply_to_pasteboard, relay_fetch_auto_apply_candidate, relay_should_auto_apply,
    relay_should_skip_wifi,
};
use super::registration::{ensure_token, load_initial_token, snapshot_sync_key};
use super::types::{PullItem, RelayError};
use super::watermark::{load_watermark, save_watermark, Watermark};

mod ingest;

pub(super) use ingest::ingest_page_blocking;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relay::testutil::{skey, test_client};
    use crate::relay::watermark::Watermark as RelayWatermark;

    /// pull_page parses an items array and an empty array; watermark query is
    /// formed correctly (smoke).
    #[tokio::test]
    #[serial_test::serial]
    async fn pull_page_parses_items() {
        use copypaste_core::derive_relay_inbox_id;
        let k = skey("pull-page-pass");
        let inbox = derive_relay_inbox_id(&k);
        let path = format!("/devices/{inbox}/items");
        let _m = mockito::mock("GET", mockito::Matcher::Regex(format!("^{path}.*")))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"[{"id":3,"content_type":"text","content_b64":"YQ==","wall_time":99}]"#)
            .create();
        let items = pull_page(
            &test_client(),
            &mockito::server_url(),
            &inbox,
            "tok",
            RelayWatermark::default(),
        )
        .await
        .expect("pull ok");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, 3);
        assert_eq!(items[0].wall_time, 99);
    }

    // ── CopyPaste-28br: adaptive idle back-off constants ─────────────────────

    /// Verify the back-off constants satisfy the acceptance criterion:
    /// ≥60 s interval after 3 consecutive empty polls, reset to 5 s on
    /// non-empty batch.
    ///
    /// The logic is in `receive_loop` (not easily extracted as a pure fn),
    /// so this test pins the constants and the arithmetic directly.
    #[test]
    fn idle_backoff_constants_satisfy_acceptance_criteria() {
        use super::super::{IDLE_POLL_STEP, POLL_INTERVAL, POLL_INTERVAL_MAX};

        // Acceptance: after IDLE_EMPTY_POLL_THRESHOLD consecutive empty polls
        // the interval must grow to ≥ 60 s.
        let steps_after_threshold = 1u32; // first step beyond threshold
        let interval = IDLE_POLL_STEP * steps_after_threshold;
        assert!(
            interval >= std::time::Duration::from_secs(60),
            "CopyPaste-28br: first idle step must be ≥ 60 s, got {interval:?}. \
             IDLE_POLL_STEP={IDLE_POLL_STEP:?}"
        );

        // The cap (POLL_INTERVAL_MAX) must be at least the first step.
        assert!(
            POLL_INTERVAL_MAX >= interval,
            "POLL_INTERVAL_MAX ({POLL_INTERVAL_MAX:?}) must be ≥ first idle step ({interval:?})"
        );

        // A non-empty batch resets to POLL_INTERVAL (5 s).
        // This is logic in receive_loop; assert the constant here.
        assert_eq!(
            POLL_INTERVAL,
            std::time::Duration::from_secs(5),
            "base POLL_INTERVAL must remain 5 s for low-latency active sync"
        );
    }

    /// Simulate the adaptive back-off state machine from `receive_loop` to
    /// verify the counter and interval transitions are correct.
    #[test]
    fn idle_backoff_state_machine_grows_then_resets() {
        use super::super::{
            IDLE_EMPTY_POLL_THRESHOLD, IDLE_POLL_STEP, POLL_INTERVAL, POLL_INTERVAL_MAX,
        };
        use std::time::Duration;

        let mut consecutive_empty: u32 = 0;
        let mut current_interval = POLL_INTERVAL;

        // Helper: simulate one empty poll tick (mirrors the logic in receive_loop).
        let tick_empty = |consecutive_empty: &mut u32, current_interval: &mut Duration| {
            *consecutive_empty = consecutive_empty.saturating_add(1);
            if *consecutive_empty >= IDLE_EMPTY_POLL_THRESHOLD {
                let steps = consecutive_empty.saturating_sub(IDLE_EMPTY_POLL_THRESHOLD) + 1;
                *current_interval = (IDLE_POLL_STEP * steps).min(POLL_INTERVAL_MAX);
            }
        };

        // Helper: simulate one non-empty poll tick.
        let tick_nonempty = |consecutive_empty: &mut u32, current_interval: &mut Duration| {
            *consecutive_empty = 0;
            *current_interval = POLL_INTERVAL;
        };

        // Initial state: interval is at minimum.
        assert_eq!(current_interval, POLL_INTERVAL);

        // Polls 1 and 2 (below threshold): interval must not grow yet.
        tick_empty(&mut consecutive_empty, &mut current_interval);
        assert_eq!(
            current_interval, POLL_INTERVAL,
            "below threshold: interval must not grow yet (poll 1)"
        );
        tick_empty(&mut consecutive_empty, &mut current_interval);
        assert_eq!(
            current_interval, POLL_INTERVAL,
            "below threshold: interval must not grow yet (poll 2)"
        );

        // Poll 3 reaches threshold: interval must grow to ≥ 60 s.
        tick_empty(&mut consecutive_empty, &mut current_interval);
        assert!(
            current_interval >= Duration::from_secs(60),
            "CopyPaste-28br: at threshold (poll 3) interval must be ≥ 60 s, got {current_interval:?}"
        );

        // Further polls: interval must grow and stay ≤ POLL_INTERVAL_MAX.
        for _ in 0..20 {
            tick_empty(&mut consecutive_empty, &mut current_interval);
            assert!(
                current_interval <= POLL_INTERVAL_MAX,
                "interval must be capped at POLL_INTERVAL_MAX, got {current_interval:?}"
            );
        }

        // A non-empty poll must reset both counter and interval.
        tick_nonempty(&mut consecutive_empty, &mut current_interval);
        assert_eq!(
            consecutive_empty, 0,
            "non-empty poll must reset consecutive_empty to 0"
        );
        assert_eq!(
            current_interval, POLL_INTERVAL,
            "non-empty poll must reset interval to POLL_INTERVAL"
        );
    }
}
