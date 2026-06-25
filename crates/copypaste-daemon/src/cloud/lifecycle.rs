use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

use copypaste_core::{ClipboardItem, Database, SyncKey};
use copypaste_supabase::auth::AuthClient;
use copypaste_supabase::RealtimeConfig;

use super::auth::resolve_bearer_with_client;
use super::config::{is_https_url, test_only_allows_local_http, CloudConfig, CloudError};
use super::handle::CloudHandle;
use super::poll::realtime_loop;
use super::push::push_loop;
use super::ws::ws_ingest_loop;

// ── Public entry point ────────────────────────────────────────────────────────

/// Start the cloud-sync background tasks.
///
/// # Arguments
/// - `config` — Supabase credentials.
/// - `db` — shared local database (used by the realtime/poll loop to insert remote items).
/// - `new_item_rx` — broadcast receiver; every locally created item is pushed to Supabase.
/// - `sync_key` — shared passphrase-derived cloud encryption key. When `None`,
///   upload and download are skipped with a one-time `warn!`.
/// - `last_sync_ms` — shared counter updated after each successful poll round.
///   Read by `get_sync_status` IPC to surface a timestamp to the UI.
/// - `local_key` — daemon's local XChaCha20-Poly1305 key, used to decrypt
///   locally-stored ciphertext before re-encrypting for the cloud.
/// - `cloud_signed_in` — shared flag published for the IPC `get_sync_status`
///   handler. Set `true` once a bearer is successfully resolved and `false` if
///   bearer resolution fails (BUG 2: the IPC layer previously hardcoded
///   `signed_in = supabase_configured`, so it kept reporting "signed in" even
///   after a `CloudError::AuthFailed` aborted cloud sync).
///
/// Returns a [`CloudHandle`] that can be used to stop the tasks.
#[allow(clippy::too_many_arguments)]
pub async fn start_cloud(
    config: CloudConfig,
    db: Arc<Mutex<Database>>,
    new_item_rx: tokio::sync::broadcast::Receiver<ClipboardItem>,
    sync_key: Arc<Mutex<Option<SyncKey>>>,
    last_sync_ms: Arc<std::sync::atomic::AtomicI64>,
    local_key: Arc<zeroize::Zeroizing<[u8; 32]>>,
    cloud_signed_in: Arc<std::sync::atomic::AtomicBool>,
    // Shared live core config. The push/poll loops read `sync_on_wifi_only`
    // and `storage_quota_bytes` on every tick so runtime changes via
    // `set_config` take effect without a daemon restart (A-SET-2).
    core_config: Arc<std::sync::RwLock<copypaste_core::AppConfig>>,
    // CopyPaste-1jms.22: shared in-flight flag for SyncBadgeState::Syncing.
    sync_in_flight: std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> anyhow::Result<CloudHandle> {
    // Defence-in-depth: re-validate the URL even though CloudConfig::new should
    // have rejected it already. Cheap, and protects callers that constructed
    // the struct directly (e.g. tests).
    //
    // TEST SEAM: under `#[cfg(test)]` only, a plain-`http://` URL whose host is
    // `127.0.0.1`/`localhost` is permitted so the orchestrator can be pointed at
    // an in-process mock PostgREST (see the `bytea_e2e` test module). PRODUCTION
    // builds (no `cfg(test)`) still require HTTPS for every URL — this branch is
    // compiled out entirely outside tests, so it cannot weaken the shipped binary.
    if !is_https_url(&config.supabase_url) && !test_only_allows_local_http(&config.supabase_url) {
        // Not an auth failure per se, but cloud sync is not running, so the UI
        // must not claim we are signed in.
        cloud_signed_in.store(false, Ordering::Relaxed);
        return Err(CloudError::InsecureUrl(config.supabase_url.clone()).into());
    }

    // Shared auth client: holds the GoTrue session (incl. refresh token) in its
    // in-memory store after sign-in, so the 401-refresh path can use the cheap
    // refresh-token grant instead of a full password sign-in.
    let auth_client = Arc::new(AuthClient::new(
        config.supabase_url.clone(),
        config.anon_key.clone(),
    ));

    // Resolve the bearer fail-closed: if email/password is configured and
    // sign-in fails, we abort cloud sync entirely instead of silently using
    // the anon key (which would downgrade scope without operator awareness).
    // Publish the real auth state either way (BUG 2). Use the shared
    // `auth_client` so the resulting session (incl. refresh token) is reusable
    // by the 401-refresh path's cheap refresh-token grant.
    let bearer_str = match resolve_bearer_with_client(&config, &auth_client).await {
        Ok(token) => {
            cloud_signed_in.store(true, Ordering::Relaxed);
            token
        }
        Err(e) => {
            cloud_signed_in.store(false, Ordering::Relaxed);
            return Err(e.into());
        }
    };
    // Shared, mutable bearer so the 401-refresh path (Wave 2.7 edge #20) can
    // swap in a fresh token without restarting the loops.
    let bearer: Arc<RwLock<String>> = Arc::new(RwLock::new(bearer_str));

    // [P1 audit fix] Wire spawn_auto_refresh so the token is proactively
    // refreshed before the ~1 h GoTrue expiry.
    //
    // Audit-concurrency MEDIUM: the auto-refresh loop has no cooperative
    // shutdown of its own, so we must NOT detach it with `let _ =` — that
    // leaked one immortal task (+ its `Arc<AuthClient>` and reqwest pool) per
    // cloud (re)start. Retain the JoinHandle in the CloudHandle and `.abort()`
    // it on shutdown/drop instead.
    let auth_refresh_handle = auth_client.clone().spawn_auto_refresh();

    // Extract the GoTrue user UUID from the session (populated by sign_in).
    // Used as the Realtime postgres_changes filter so the server pre-filters
    // rows by user_id before delivering events (P1 audit fix: realtime.rs ~235).
    let ws_user_id: Option<String> = auth_client.current_session().map(|s| s.user.id.clone());

    // CopyPaste-44rq.26: compute a canonical account identity token that
    // combines the Supabase project reference (parsed from the URL) and the
    // GoTrue user UUID.  Two devices must share the SAME token for their RLS
    // policies to let them see each other's rows.
    //
    // We log the project slug (non-secret) and emit a WARN when the user_id is
    // absent (= anon-key-only, which means RLS will reject all operations
    // anyway).  The IPC `get_sync_status` handler can expose this token once
    // the ipc.rs IPC/UI lane adds the field (see bd note on CopyPaste-44rq.26).
    let cloud_account_id: Option<String> = ws_user_id
        .as_deref()
        .map(|uid| copypaste_supabase::supabase_account_id(&config.supabase_url, uid));
    {
        let project_ref = copypaste_supabase::supabase_project_ref(&config.supabase_url)
            .unwrap_or_else(|| config.supabase_url.clone());
        match cloud_account_id {
            Some(ref id) => {
                // Log the project ref and a truncated (non-secret) account id.
                // The full user UUID is not PII but we avoid echoing the entire
                // token to reduce noise; the first 8 chars identify the project.
                tracing::info!(
                    project_ref = %project_ref,
                    account_id_prefix = %id.chars().take(16).collect::<String>(),
                    "cloud-sync: account identity established (CopyPaste-44rq.26)"
                );
            }
            None => {
                // No GoTrue session → anon-key-only; RLS rejects all operations.
                tracing::warn!(
                    project_ref = %project_ref,
                    "cloud-sync: no GoTrue user session — anon-key-only requests \
                     will be rejected by RLS; sign in with email/password \
                     (CopyPaste-44rq.26)"
                );
            }
        }
    }

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    // We need two copies of the shutdown signal — use a shared Notify.
    let shutdown = Arc::new(tokio::sync::Notify::new());

    // Wire the oneshot into the Notify so both loops see the signal.
    let notify_clone = shutdown.clone();
    tokio::spawn(async move {
        let _ = shutdown_rx.await;
        notify_clone.notify_waiters();
    });

    // v0.5.3: shared flag — `true` when the Realtime WebSocket channel is
    // subscribed and delivering events. The HTTP poll loop reads this flag to
    // decide its tick interval: slow (120 s) when WS is up (catch-up only),
    // full-speed (10 s) when WS is down or has never connected.
    let ws_connected = Arc::new(std::sync::atomic::AtomicBool::new(false));

    // Task A: push new local items to Supabase REST.
    // Also passes `db` (for startup backlog) and `last_sync_ms` (so every
    // successful push updates the timestamp, not only poll-side syncs).
    let push_config = config.clone();
    let push_bearer = bearer.clone();
    let push_shutdown = shutdown.clone();
    let push_sync_key = sync_key.clone();
    let push_local_key = local_key.clone();
    let push_db = db.clone();
    let push_last_sync_ms = last_sync_ms.clone();
    let push_signed_in = cloud_signed_in.clone();
    let push_auth = auth_client.clone();
    let push_core_config = core_config.clone();
    tokio::spawn(push_loop(
        push_config,
        push_bearer,
        new_item_rx,
        push_shutdown,
        push_sync_key,
        push_local_key,
        push_db,
        push_last_sync_ms,
        push_signed_in,
        push_auth,
        push_core_config,
        sync_in_flight.clone(),
    ));

    // Task B: poll Supabase REST for remote items and insert unknown ones locally.
    // When Task C (WS) is connected this runs at POLL_INTERVAL_WS_CONNECTED
    // (2 min catch-up); when WS is disconnected it falls back to
    // POLL_INTERVAL_WS_FALLBACK (10 s) as the sole download path.
    let poll_config = config.clone();
    let poll_bearer = bearer.clone();
    let poll_shutdown = shutdown.clone();
    let poll_sync_key = sync_key.clone();
    let poll_local_key = local_key.clone();
    let poll_last_sync_ms = last_sync_ms.clone();
    let poll_signed_in = cloud_signed_in.clone();
    let poll_auth = auth_client.clone();
    let poll_ws_connected = ws_connected.clone();
    // Share the live core config Arc with ws_ingest_loop so it reads the
    // current `storage_quota_bytes` on every prune (byte-only policy, hot-reload)
    // — mirroring realtime_loop.  Clone before core_config is moved below.
    let ws_core_config = core_config.clone();
    let poll_core_config = core_config;
    tokio::spawn(realtime_loop(
        poll_config,
        poll_bearer,
        db.clone(),
        poll_shutdown,
        poll_sync_key,
        poll_local_key,
        poll_last_sync_ms,
        poll_signed_in,
        poll_auth,
        poll_ws_connected,
        poll_core_config,
        sync_in_flight.clone(),
    ));

    // Task C: Supabase Realtime WebSocket — instant INSERT delivery.
    //
    // Builds a `RealtimeConfig` from the same credentials as the REST loops,
    // passing the authenticated bearer as `user_jwt` so the Realtime server
    // applies RLS and delivers only the signed-in user's rows.
    //
    // On connect: sets `ws_connected = true` → HTTP poll backs off to 120 s.
    // On disconnect / reconnect cycle: `ws_connected = false` during the gap →
    // HTTP poll automatically steps back up to 10 s so no items are missed.
    //
    // The Wi-Fi guard (`sync_on_wifi_only`) is NOT applied here because the
    // WebSocket connection is persistent; the poll loop already guards the
    // actual download work. A WS reconnect on cellular is cheap (a few bytes)
    // and avoids a stale `ws_connected = false` that would needlessly
    // accelerate polling.
    // [P0 audit fix] Build the RealtimeConfig with the live bearer Arc so
    // ws_ingest_loop can write the current token into config.user_jwt on every
    // reconnect, preventing stale-JWT permanent failure after ~1 h expiry.
    // [P1 audit fix] Also thread ws_user_id so the postgres_changes subscription
    // carries a server-side filter clause.
    let ws_jwt = bearer.read().await.clone();
    let ws_realtime_config = RealtimeConfig::with_jwt_and_user_id(
        config.supabase_url.clone(),
        config.anon_key.clone(),
        RealtimeConfig::DEFAULT_TOPIC,
        Some(ws_jwt),
        ws_user_id,
        true,
    );
    let ws_bearer = bearer.clone();
    let ws_sync_key = sync_key.clone();
    let ws_local_key = local_key.clone();
    let ws_db = db;
    let ws_last_sync_ms = last_sync_ms.clone();
    let ws_shutdown = shutdown.clone();
    let ws_connected_flag = ws_connected;
    tokio::spawn(ws_ingest_loop(
        ws_realtime_config,
        ws_bearer,
        ws_db,
        ws_sync_key,
        ws_local_key,
        ws_last_sync_ms,
        ws_shutdown,
        ws_connected_flag,
        ws_core_config,
        sync_in_flight,
    ));

    tracing::info!(
        "cloud-sync started (url={}, realtime=ws)",
        config.supabase_url
    );
    Ok(CloudHandle {
        shutdown_tx: Some(shutdown_tx),
        auth_refresh_handle: Some(auth_refresh_handle),
        cloud_account_id,
    })
}
