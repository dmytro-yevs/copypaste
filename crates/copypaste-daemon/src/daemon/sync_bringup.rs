//! Sync-orchestrator + cloud-sync + relay-sync bring-up. Extracted from
//! `run_with_quit_flag` (CopyPaste-vp63.12).
//!
//! Three independent startup steps, run in this order by the caller:
//! [`spawn_sync_orch`] (always), [`start_cloud_sync`] (feature `cloud-sync`),
//! [`start_relay_sync`] (feature `relay-sync`). Cloud and relay are ADDITIVE,
//! INDEPENDENT transports (see `relay.rs` § "Multi-transport topology") — both
//! run when both are configured.

// NOTE: `AppConfig` and `AtomicBool` are deliberately NOT imported at module
// level and instead fully-qualified (`copypaste_core::AppConfig`,
// `std::sync::atomic::AtomicBool`) at their one use site below. Both symbols
// are only referenced inside `start_cloud_sync`/`start_relay_sync`, which are
// gated on the (default-on, but not universally-on) `cloud-sync`/`relay-sync`
// features; a plain `use` would become an unused-import warning under
// `-D warnings` on a `--no-default-features` build.
use copypaste_core::{ClipboardItem, Database};
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, Mutex};
use tokio_util::sync::CancellationToken;

/// Spawn the sync orchestrator task.
///
/// The orchestrator owns the bridge between the local clipboard broadcast
/// channel and the peer transport(s). It is ALWAYS spawned — even when P2P is
/// disabled — because the inbound side may still receive items from the
/// cloud-sync path.
///
/// CopyPaste-vp63.12: hoisted verbatim from `run_with_quit_flag`. Each
/// argument is a distinct daemon-lifecycle handle the orchestrator's own
/// bring-up needs (device identity, storage, sync channels, crypto, config,
/// shutdown) — mirrors the rationale `sync_orch::run`'s own doc comment gives
/// for its argument list one layer down.
#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_sync_orch(
    db: Arc<Mutex<Database>>,
    new_item_tx: &broadcast::Sender<ClipboardItem>,
    sync_incoming_rx: mpsc::Receiver<copypaste_sync::WireItem>,
    sync_outbound_tx: mpsc::Sender<copypaste_sync::WireItem>,
    local_device_id: String,
    sync_crypto: Option<crate::sync_orch::SyncCrypto>,
    sync_quota_bytes: i64,
    sync_auto_apply: Option<crate::sync_orch::AutoApplyCtx>,
    shutdown_token: CancellationToken,
) -> tokio::task::JoinHandle<()> {
    let sync_db = db.clone();
    let sync_rx = new_item_tx.subscribe();
    tokio::spawn(async move {
        if let Err(e) = crate::sync_orch::run(
            sync_db,
            sync_rx,
            sync_incoming_rx,
            sync_outbound_tx,
            local_device_id,
            sync_crypto,
            sync_quota_bytes,
            sync_auto_apply,
            shutdown_token,
        )
        .await
        {
            tracing::warn!("sync orchestrator exited with error: {e}");
        }
    })
}

/// Start optional cloud-sync if credentials are present and sync is enabled.
///
/// Returns `None` (no orchestrator started) when `sync_enabled_at_start` is
/// false, `SUPABASE_URL` is unset, or `start_cloud` itself fails.
#[cfg(feature = "cloud-sync")]
#[allow(clippy::too_many_arguments)]
pub(crate) async fn start_cloud_sync(
    db: Arc<Mutex<Database>>,
    new_item_tx: &broadcast::Sender<ClipboardItem>,
    cloud_sync_key: Arc<Mutex<Option<copypaste_core::SyncKey>>>,
    cloud_last_sync_ms: Arc<std::sync::atomic::AtomicI64>,
    local_key_arc: &Arc<zeroize::Zeroizing<[u8; 32]>>,
    cloud_signed_in: Arc<std::sync::atomic::AtomicBool>,
    core_config_arc: Arc<std::sync::RwLock<copypaste_core::AppConfig>>,
    sync_in_flight: Arc<std::sync::atomic::AtomicBool>,
    cloud_account_id_slot: Arc<std::sync::Mutex<Option<String>>>,
    sync_enabled_at_start: bool,
) -> Option<crate::cloud::CloudHandle> {
    use crate::cloud::{start_cloud, CloudConfig};
    if !sync_enabled_at_start {
        tracing::info!("cloud-sync: sync_enabled=false — not starting cloud orchestrator");
        None
    } else if let Some(cloud_cfg) = CloudConfig::from_env() {
        tracing::info!("cloud-sync: SUPABASE_URL found, starting cloud orchestrator");
        // Subscribe a new receiver from the existing sender.
        let rx = new_item_tx.subscribe();
        match start_cloud(
            cloud_cfg,
            db,
            rx,
            cloud_sync_key,
            cloud_last_sync_ms,
            local_key_arc.clone(),
            cloud_signed_in,
            core_config_arc,
            sync_in_flight,
        )
        .await
        {
            Ok(handle) => {
                tracing::info!("cloud-sync: orchestrator started");
                // CopyPaste-1jms.34: publish the canonical account id into
                // the shared slot. The IpcServer's `get_sync_status` handler
                // holds the same Arc and reads through it on every request,
                // so this one write at startup is sufficient.
                *cloud_account_id_slot
                    .lock()
                    .unwrap_or_else(|p| p.into_inner()) = handle.cloud_account_id.clone();
                Some(handle)
            }
            Err(e) => {
                tracing::warn!("cloud-sync: failed to start ({e}); continuing without sync");
                None
            }
        }
    } else {
        tracing::debug!("cloud-sync: SUPABASE_URL not set, skipping");
        None
    }
}

/// Start the relay-as-database sync path iff `relay_url` is configured and
/// sync is enabled. Writes the started handle (or `None`) into
/// `relay_handle_slot` — the IPC `set_config` handler reads/replaces the same
/// Arc so it can shut the relay down at runtime (CopyPaste-44rq.67).
///
/// TOPOLOGY (dtq3): relay and Supabase are ADDITIVE, INDEPENDENT transports.
/// When both are configured, this and `start_cloud_sync` BOTH run; consumer-
/// side dedup (`remote_wins` / `ingest_page_blocking`) makes a double delivery
/// a no-op, so no mutual-exclusion gate is needed here. See `relay.rs` §
/// "Multi-transport topology" for the full contract.
#[cfg(feature = "relay-sync")]
#[allow(clippy::too_many_arguments)]
pub(crate) async fn start_relay_sync(
    core_config_arc: Arc<std::sync::RwLock<copypaste_core::AppConfig>>,
    sync_enabled_at_start: bool,
    local_device_id: &str,
    db: Arc<Mutex<Database>>,
    new_item_tx: &broadcast::Sender<ClipboardItem>,
    cloud_sync_key: Arc<Mutex<Option<copypaste_core::SyncKey>>>,
    local_key_arc: &Arc<zeroize::Zeroizing<[u8; 32]>>,
    cloud_last_sync_ms: Arc<std::sync::atomic::AtomicI64>,
    // Computed by the caller (cfg(unix) vs cfg(not(unix))) so this parameter
    // stays a plain, platform-independent `Option` — mirrors the
    // `sync_auto_apply` construction pattern already used for
    // `spawn_sync_orch`'s caller.
    relay_auto_apply_cc: Option<Arc<std::sync::atomic::AtomicI64>>,
    sync_in_flight: Arc<std::sync::atomic::AtomicBool>,
    relay_handle_slot: Arc<Mutex<Option<crate::relay::RelayHandle>>>,
) {
    let relay_url = core_config_arc
        .read()
        .ok()
        .and_then(|c| c.relay_url.clone());
    let started = if !sync_enabled_at_start {
        tracing::info!("relay-sync: sync_enabled=false — not starting relay orchestrator");
        None
    } else if let Some(relay_url) = relay_url {
        tracing::info!("relay-sync: relay_url configured, starting relay orchestrator");
        // CopyPaste-16vr: the previous fallback was `reqwest::Client::new()`
        // which has no request timeout — a stalled relay endpoint would
        // block the sync loop forever. The builder can fail (e.g. when the
        // platform TLS stack is unavailable). The fallback also applies the
        // timeout: a second builder call with identical settings is tried;
        // if that also fails, SYNC_HTTP_TIMEOUT is applied via
        // `tokio::time::timeout` at the call sites in relay.rs.
        let client = reqwest::Client::builder()
            .timeout(crate::sync_common::SYNC_HTTP_TIMEOUT)
            .build()
            .unwrap_or_else(|_| {
                // Re-attempt: building with only `.timeout()` set cannot
                // fail on any supported platform. `expect` is justified: if
                // even this minimal builder fails there is a fundamental
                // platform issue and daemon startup should abort rather than
                // run without timeouts.
                reqwest::Client::builder()
                    .timeout(crate::sync_common::SYNC_HTTP_TIMEOUT)
                    .build()
                    .expect(
                        "reqwest Client::builder().timeout().build() \
                         must succeed — platform TLS unavailable",
                    )
            });

        match crate::relay::start_relay(
            client,
            relay_url,
            super::resolve_device_name(),
            local_device_id.to_owned(),
            db,
            new_item_tx.subscribe(),
            cloud_sync_key,
            local_key_arc.clone(),
            cloud_last_sync_ms,
            core_config_arc,
            relay_auto_apply_cc,
            sync_in_flight,
        ) {
            Ok(handle) => {
                tracing::info!("relay-sync: orchestrator started");
                Some(handle)
            }
            Err(e) => {
                tracing::warn!("relay-sync: failed to start ({e}); continuing without relay");
                None
            }
        }
    } else {
        tracing::debug!("relay-sync: relay_url not set, skipping");
        None
    };
    // Publish into the shared slot the IPC server holds a clone of.
    *relay_handle_slot.lock().await = started;
}
