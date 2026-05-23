#[cfg(unix)]
use crate::ipc::IpcServer;
use crate::{
    clipboard::{ClipboardContent, ClipboardMonitor},
    p2p, paths,
};
use copypaste_core::{
    build_item_aad, chunks_to_blob, detect, encode_image, encrypt_item_with_aad, insert_item,
    upsert_fts, AppConfig, ClipboardItem, Database, DeviceKeypair, AAD_SCHEMA_VERSION,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, mpsc, Mutex};
use tokio::time::interval;

// Beta W2.2 (arch-1): sync orchestrator that wires `copypaste-sync` into the
// daemon. Declared at crate root in `lib.rs` (`pub mod sync_orch;`); we
// re-import it here for the local `sync_orch::run` call below.
use crate::sync_orch;

/// Run the daemon until `Ctrl+C` / `SIGTERM` is received.
///
/// This is the entry point used on non-macOS platforms and in tests.
pub async fn run() -> anyhow::Result<()> {
    run_with_quit_flag(Arc::new(AtomicBool::new(false))).await
}

/// Run the daemon until `Ctrl+C`, `SIGTERM`, or `quit_flag` is set.
///
/// On macOS the tray icon sets `quit_flag` when the user clicks Quit.
#[cfg_attr(not(target_os = "macos"), allow(unused_variables))]
pub async fn run_with_quit_flag(quit_flag: Arc<AtomicBool>) -> anyhow::Result<()> {
    let config = load_config();
    tracing::info!(
        "poll_interval={}ms history_limit={}",
        config.poll_interval_ms,
        config.history_limit
    );

    // v0.3 (THREAT-MODEL OI-4): upgrade the Keychain entry's ACL on first
    // launch after install/upgrade.  Idempotent + best-effort — a failure
    // here (e.g. user denied a Keychain prompt) must not block the daemon
    // because the entry is still usable, just with the legacy unrestricted
    // ACL.  The next launch retries automatically.
    #[cfg(target_os = "macos")]
    {
        match crate::keychain::acl::rotate_acl_to_current_install() {
            Ok(true) => tracing::info!("Keychain ACL rotated to current install"),
            Ok(false) => tracing::debug!("Keychain ACL already current"),
            Err(e) => tracing::warn!(
                error = %e,
                "Keychain ACL rotation failed — entry still usable with legacy ACL"
            ),
        }
    }

    let local_key = load_local_key();
    let local_key_arc: Arc<[u8; 32]> = Arc::new(local_key);
    tracing::info!("local encryption key ready");

    let db_path = paths::db_path();
    let db = Arc::new(Mutex::new(
        Database::open(&db_path, &local_key).map_err(|e| anyhow::anyhow!("Database: {e}"))?,
    ));
    tracing::info!("database opened at {}", db_path.display());

    // Device-keypair public bytes — passed into IpcServer so
    // `get_own_fingerprint` returns a stable cryptographic fingerprint
    // (audit HIGH #6: DefaultHasher(hostname,pid) changed every restart).
    // On non-macOS we don't have a keychain-backed keypair; use a zero
    // placeholder. Memory: Windows/Linux are cfg-frozen (macOS+Android only).
    #[cfg(target_os = "macos")]
    let device_public_key_arc: Arc<[u8; 32]> = {
        let kp = crate::keychain::load_or_create()
            .map_err(|e| anyhow::anyhow!("keychain load_or_create: {e}"))?;
        Arc::new(kp.public_key_bytes())
    };
    #[cfg(not(target_os = "macos"))]
    let device_public_key_arc: Arc<[u8; 32]> = Arc::new([0u8; 32]);

    // Shared private-mode flag: when true, the clipboard monitor skips recording.
    // This is set/cleared via the IPC `set_private_mode` command.
    let private_mode = Arc::new(AtomicBool::new(false));

    #[cfg(unix)]
    let socket_path = paths::socket_path();
    #[cfg(unix)]
    {
        let ipc_db = db.clone();
        let ipc_private_mode = private_mode.clone();
        let ipc_local_key = local_key_arc.clone();
        let ipc_device_pub = device_public_key_arc.clone();
        let socket_clone = socket_path.clone();
        tokio::spawn(async move {
            let server = IpcServer::new(ipc_db, ipc_private_mode, ipc_local_key, ipc_device_pub);
            if let Err(e) = server.serve(&socket_clone).await {
                tracing::error!("IPC server error: {e}");
            }
        });
    }

    // Broadcast channel: carries newly-inserted clipboard items to any
    // subscriber (P2P sync, cloud-sync, future extensions).
    //
    // Capacity 256 (bumped from 64 — audit HIGH #8). The earlier 64-slot
    // buffer was too small for clipboard bursts (e.g. a rapid `pbcopy` loop
    // or a P2P peer momentarily backpressured by network jitter): subscribers
    // would receive `RecvError::Lagged` and silently drop items.
    //
    // Subscriber loops (p2p::subscriber_loop, cloud orchestrator, sync_orch)
    // still need to log `Lagged(n)` themselves — owned by the subsystems that
    // hold the receivers, not this file.
    let (new_item_tx, _new_item_rx) = broadcast::channel::<ClipboardItem>(256);

    // Start the P2P subsystem when COPYPASTE_P2P=1 is set in the environment.
    let _p2p_handle: Option<p2p::P2pHandle> =
        if std::env::var("COPYPASTE_P2P").as_deref() == Ok("1") {
            // Persistent device_id: regenerating on every restart would break P2P
            // pairing and cloud peer recognition (arch LOW #24). Read from disk,
            // creating + writing a fresh UUID v4 on first run.
            let device_id = match load_or_create_device_id() {
                Ok(id) => id,
                Err(e) => {
                    tracing::warn!("device_id load/create failed ({e}); using ephemeral UUID");
                    uuid::Uuid::new_v4()
                }
            };
            let device_name = std::env::var("HOSTNAME")
                .or_else(|_| std::env::var("COMPUTERNAME"))
                .unwrap_or_else(|_| "CopyPaste".to_string());

            let p2p_config = p2p::P2pConfig {
                listen_port: 0,
                device_name,
                enabled: true,
            };

            match p2p::start_p2p(
                p2p_config,
                db.clone(),
                device_id,
                local_key,
                new_item_tx.subscribe(),
            )
            .await
            {
                Ok(handle) => {
                    tracing::info!(port = handle.actual_port, "P2P subsystem running");
                    Some(handle)
                }
                Err(e) => {
                    tracing::warn!("Failed to start P2P subsystem: {e}");
                    None
                }
            }
        } else {
            tracing::debug!("P2P disabled (set COPYPASTE_P2P=1 to enable)");
            None
        };

    // Beta W2.2 (arch-1): start the sync orchestrator.
    //
    // The orchestrator owns the bridge between the local clipboard broadcast
    // channel and the peer transport(s). We always spawn it — even when P2P
    // is disabled — because the inbound side may still receive items from the
    // cloud-sync path once that worker (W2.3) wires its incoming sender in.
    //
    // For now the outbound side has no live consumer (the P2P subsystem still
    // owns its own subscriber loop, see `p2p::subscriber_loop`). Sending into
    // a channel whose receiver is dropped is harmless: `outbound_tx.send`
    // returns `Err`, which the orchestrator logs at debug and continues.
    let (sync_outbound_tx, _sync_outbound_rx) = mpsc::channel::<copypaste_sync::WireItem>(64);
    let (_sync_incoming_tx, sync_incoming_rx) = mpsc::channel::<copypaste_sync::WireItem>(64);
    // Persistent sync_device_id (audit HIGH #13).
    //
    // Previously this was `Uuid::new_v4().to_string()` on every startup,
    // which broke sync orchestrator correlation: peers saw a brand-new
    // device on every restart and could not deduplicate items by origin.
    //
    // We reuse the same on-disk identifier the P2P branch already loads
    // via `load_or_create_device_id` so the daemon presents a single
    // stable identity across the local-clipboard / P2P / sync surfaces.
    let sync_device_id = match load_or_create_device_id() {
        Ok(id) => id.to_string(),
        Err(e) => {
            tracing::warn!(
                "sync_device_id load/create failed ({e}); falling back to ephemeral UUID — \
                 sync orchestrator will treat this run as a new device"
            );
            uuid::Uuid::new_v4().to_string()
        }
    };
    let sync_db = db.clone();
    let sync_rx = new_item_tx.subscribe();
    let _sync_handle = tokio::spawn(async move {
        if let Err(e) = sync_orch::run(
            sync_db,
            sync_rx,
            sync_incoming_rx,
            sync_outbound_tx,
            sync_device_id,
        )
        .await
        {
            tracing::warn!("sync orchestrator exited with error: {e}");
        }
    });
    // Keep the local channel endpoints alive across shutdown — dropping the
    // inbound sender would close the orchestrator's incoming side prematurely,
    // and dropping the outbound receiver would cause every local item to log
    // a debug message. The transport worker will own its own clones once
    // integration lands.
    let _keep_alive_sync_incoming = _sync_incoming_tx;
    let _keep_alive_sync_outbound = _sync_outbound_rx;

    // Start optional cloud-sync if credentials are present.
    #[cfg(feature = "cloud-sync")]
    let _cloud_handle = {
        use crate::cloud::{start_cloud, CloudConfig};
        if let Some(cloud_cfg) = CloudConfig::from_env() {
            tracing::info!("cloud-sync: SUPABASE_URL found, starting cloud orchestrator");
            // Subscribe a new receiver from the existing sender.
            let rx = new_item_tx.subscribe();
            match start_cloud(cloud_cfg, db.clone(), rx).await {
                Ok(handle) => {
                    tracing::info!("cloud-sync: orchestrator started");
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
    };

    let mut monitor = ClipboardMonitor::new(config.max_text_size_bytes);
    let mut ticker = interval(Duration::from_millis(config.poll_interval_ms));
    let mut cleanup_ticks: u64 = 0;
    // Sensitive TTL cleanup runs every 5 seconds; track elapsed ticks separately.
    let mut sensitive_cleanup_ticks: u64 = 0;
    let sensitive_ttl_ms = config.sensitive_ttl_secs as i64 * 1000;

    tracing::info!("clipboard monitor started");
    tracing::info!(
        "sensitive auto-wipe TTL: {}s ({}ms), checked every 5s",
        config.sensitive_ttl_secs,
        sensitive_ttl_ms,
    );

    #[cfg(target_os = "macos")]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut sigterm = signal(SignalKind::terminate())?;
        loop {
            // Check tray quit flag before blocking on select
            if quit_flag.load(Ordering::Relaxed) {
                tracing::info!("quit flag set, shutting down daemon");
                break;
            }
            tokio::select! {
                _ = ticker.tick() => {
                    handle_tick(&mut monitor, &db, &local_key, &config, &private_mode, &new_item_tx).await;
                    cleanup_ticks += 1;
                    sensitive_cleanup_ticks += 1;

                    // Sensitive item TTL: run every 5 seconds.
                    // `5_000 / poll_interval_ms` is integer-divided; for any
                    // `poll_interval_ms > 5000` the quotient is 0, which would
                    // make this branch fire every tick. Clamp the threshold to
                    // at least 1 so the cleanup runs (at most) every tick.
                    if sensitive_cleanup_ticks >= (5_000 / config.poll_interval_ms.max(1)).max(1) {
                        sensitive_cleanup_ticks = 0;
                        let db_guard = db.lock().await;
                        // `unwrap_or_default()` matches the pattern at ipc.rs:799
                        // — clock skew (system clock moved backwards past UNIX
                        // epoch) must not panic the daemon.
                        let now_ms = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as i64;
                        match copypaste_core::delete_sensitive_expired(&db_guard, now_ms, sensitive_ttl_ms) {
                            Ok(n) if n > 0 => tracing::info!("sensitive TTL cleanup: wiped {n} sensitive items"),
                            Ok(_) => {}
                            Err(e) => tracing::warn!("sensitive TTL cleanup error: {e}"),
                        }
                    }

                    // General expires_at TTL: run every 60 seconds. Same
                    // integer-division clamp as above.
                    if cleanup_ticks >= (60_000 / config.poll_interval_ms.max(1)).max(1) {
                        cleanup_ticks = 0;
                        let db_guard = db.lock().await;
                        let now_ms = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as i64;
                        match copypaste_core::delete_expired(&db_guard, now_ms) {
                            Ok(n) if n > 0 => tracing::info!("TTL cleanup: removed {n} expired items"),
                            Ok(_) => {}
                            Err(e) => tracing::warn!("TTL cleanup error: {e}"),
                        }
                    }
                }
                _ = tokio::signal::ctrl_c() => {
                    tracing::info!("SIGINT received, shutting down");
                    quit_flag.store(true, Ordering::Relaxed);
                    break;
                }
                _ = sigterm.recv() => {
                    tracing::info!("SIGTERM received, shutting down");
                    quit_flag.store(true, Ordering::Relaxed);
                    break;
                }
            }
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        // SIGTERM handling on non-macOS — previously only SIGINT was wired,
        // so launchd/systemd sending SIGTERM would terminate the process
        // without running our cleanup branch (sock file removal, log flush).
        #[cfg(unix)]
        let mut sigterm = {
            use tokio::signal::unix::{signal, SignalKind};
            signal(SignalKind::terminate())?
        };
        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    handle_tick(&mut monitor, &db, &local_key, &config, &private_mode, &new_item_tx).await;
                    cleanup_ticks += 1;
                    sensitive_cleanup_ticks += 1;

                    // Sensitive item TTL: run every 5 seconds.
                    if sensitive_cleanup_ticks >= (5_000 / config.poll_interval_ms.max(1)).max(1) {
                        sensitive_cleanup_ticks = 0;
                        let db_guard = db.lock().await;
                        let now_ms = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as i64;
                        match copypaste_core::delete_sensitive_expired(&db_guard, now_ms, sensitive_ttl_ms) {
                            Ok(n) if n > 0 => tracing::info!("sensitive TTL cleanup: wiped {n} sensitive items"),
                            Ok(_) => {}
                            Err(e) => tracing::warn!("sensitive TTL cleanup error: {e}"),
                        }
                    }

                    // General expires_at TTL: run every 60 seconds.
                    if cleanup_ticks >= (60_000 / config.poll_interval_ms.max(1)).max(1) {
                        cleanup_ticks = 0;
                        let db_guard = db.lock().await;
                        let now_ms = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as i64;
                        match copypaste_core::delete_expired(&db_guard, now_ms) {
                            Ok(n) if n > 0 => tracing::info!("TTL cleanup: removed {n} expired items"),
                            Ok(_) => {}
                            Err(e) => tracing::warn!("TTL cleanup error: {e}"),
                        }
                    }
                }
                _ = tokio::signal::ctrl_c() => {
                    tracing::info!("SIGINT received, shutting down");
                    break;
                }
                #[cfg(unix)]
                _ = sigterm.recv() => {
                    tracing::info!("SIGTERM received, shutting down");
                    break;
                }
            }
        }
    }

    #[cfg(unix)]
    let _ = std::fs::remove_file(&socket_path);
    tracing::info!("daemon stopped");
    Ok(())
}

#[tracing::instrument(skip_all, name = "clipboard_tick")]
async fn handle_tick(
    monitor: &mut ClipboardMonitor,
    db: &Arc<Mutex<Database>>,
    local_key: &[u8; 32],
    config: &AppConfig,
    private_mode: &Arc<AtomicBool>,
    new_item_tx: &broadcast::Sender<ClipboardItem>,
) {
    // Skip recording when private/pause mode is active
    if private_mode.load(Ordering::Relaxed) {
        // Still poll to advance the change-count so we don't replay on resume
        let _ = monitor.poll();
        tracing::debug!("private mode active: skipping clipboard recording");
        return;
    }

    match monitor.poll() {
        Ok(Some(ClipboardContent::Text(text))) => {
            // beta.5 Bug-1 visibility: log every capture at info level so
            // users can confirm from `daemon.out.log` that the pasteboard is
            // actually being read. Prior code only emitted `debug!` here
            // which the default `copypaste=info` filter dropped, leaving
            // operators unable to distinguish "no captures happening" from
            // "captures happening but UI not refreshing".
            tracing::info!(
                bytes = text.len(),
                "clipboard captured: text ({} bytes)",
                text.len()
            );
            if let Some(item) = handle_text(text, db, local_key, config).await {
                // Broadcast to P2P + cloud-sync subscribers (and any future consumer).
                // A send error only means there are no active receivers —
                // that is normal when both P2P and cloud-sync are disabled.
                let _ = new_item_tx.send(item);
            }
        }
        Ok(Some(ClipboardContent::Image(raw_bytes))) => {
            tracing::info!(
                bytes = raw_bytes.len(),
                "clipboard captured: image ({} bytes raw)",
                raw_bytes.len()
            );
            if let Some(item) = handle_image(raw_bytes, db, local_key, config).await {
                let _ = new_item_tx.send(item);
            }
        }
        Ok(Some(ClipboardContent::SkippedBatch(missed))) => {
            // Rapid clipboard burst — the monitor already logged the gap;
            // we just bump telemetry here and let the next poll capture
            // the now-current pasteboard value.
            tracing::warn!(
                missed,
                "clipboard rapid-burst: {} intermediate updates lost between polls",
                missed
            );
        }
        Ok(None) => {}
        Err(e) => tracing::warn!("clipboard poll error: {e}"),
    }
}

async fn handle_text(
    text: String,
    db: &Arc<Mutex<Database>>,
    local_key: &[u8; 32],
    config: &AppConfig,
) -> Option<ClipboardItem> {
    let is_sensitive = detect(&text).is_some();

    // v0.3: encrypt_item_with_aad binds ciphertext to (item_id, schema_version)
    // via AEAD AAD. Pre-generate item_id so the value baked into the AAD is the
    // same one persisted in the row — decryption later rebuilds AAD from the
    // stored item_id. The legacy empty-AAD fallback was removed in 1c55e57, so
    // a mismatch here would surface as `EncryptError::AuthFailed` on read.
    let item_id = uuid::Uuid::new_v4().to_string();
    let aad = build_item_aad(&item_id, AAD_SCHEMA_VERSION);
    let (nonce, ciphertext) = match encrypt_item_with_aad(text.as_bytes(), local_key, &aad) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!("encrypt_item_with_aad failed for text: {e}");
            return None;
        }
    };
    let mut item = ClipboardItem::new_text(ciphertext, nonce.to_vec(), 0);
    item.item_id = item_id;
    item.is_sensitive = is_sensitive;

    if is_sensitive {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;
        item.expires_at = Some(now_ms + (config.sensitive_ttl_local_secs as i64 * 1000));
    }

    let db_guard = db.lock().await;
    match insert_item(&db_guard, &item) {
        Ok(_) => {
            // beta.5 Bug-1 visibility: promoted from debug! to info! so users
            // can verify in `daemon.out.log` that captured items reach the DB.
            tracing::info!(
                id = %item.id,
                sensitive = is_sensitive,
                "stored text item id={} sensitive={}",
                item.id,
                is_sensitive
            );
            if let Err(e) = upsert_fts(&db_guard, &item.id, &text) {
                tracing::warn!("fts index failed for id={}: {e}", item.id);
            }
            prune_history(&db_guard, config);
            Some(item)
        }
        Err(e) => {
            tracing::warn!("failed to store text item: {e}");
            None
        }
    }
}

async fn handle_image(
    raw_bytes: Vec<u8>,
    db: &Arc<Mutex<Database>>,
    local_key: &[u8; 32],
    config: &AppConfig,
) -> Option<ClipboardItem> {
    // Derive a stable file_id from SHA-256(raw_bytes)[..16] — a 128-bit
    // collision-resistant content hash. This is deterministic so identical
    // images dedup naturally, and replaces the prior `DefaultHasher XOR
    // nanos` scheme (Wave 2.1 security LOW #19).
    let file_id = crate::clipboard::image_content_hash(&raw_bytes);

    match encode_image(&raw_bytes, local_key, &file_id) {
        Ok((meta, chunks)) => {
            let blob = chunks_to_blob(&chunks);
            let meta_json = format!(
                r#"{{"width":{},"height":{},"original_size":{},"chunk_count":{},"file_id":{:?}}}"#,
                meta.width, meta.height, meta.original_size, meta.chunk_count, meta.file_id
            );
            let item = ClipboardItem::new_image(blob, meta_json, 0);
            tracing::debug!(
                "image encoded: {}x{} px, {} chunks, original_size={}",
                meta.width,
                meta.height,
                meta.chunk_count,
                meta.original_size
            );

            let db_guard = db.lock().await;
            match insert_item(&db_guard, &item) {
                Ok(_) => {
                    // beta.5 Bug-1 visibility: promoted from debug! to info!.
                    tracing::info!(id = %item.id, "stored image item id={}", item.id);
                    // Images don't have searchable text; index empty string for FTS consistency.
                    if let Err(e) = upsert_fts(&db_guard, &item.id, "") {
                        tracing::warn!("fts empty index failed for image id={}: {e}", item.id);
                    }
                    prune_history(&db_guard, config);
                    Some(item)
                }
                Err(e) => {
                    tracing::warn!("failed to store image item: {e}");
                    None
                }
            }
        }
        Err(e) => {
            tracing::warn!("image encode failed (skipping): {e}");
            None
        }
    }
}

fn prune_history(db: &Database, config: &AppConfig) {
    let total = copypaste_core::count_items(db).unwrap_or(0) as usize;
    if total > config.history_limit {
        let excess = total - config.history_limit;
        // Direct SQL DELETE ordered by `wall_time ASC` — bulk-removes the
        // oldest rows in a single statement (audit HIGH #4). The previous
        // implementation went through `get_page` + per-row `delete_item`,
        // which was both N+1 and risked pruning the wrong page if the
        // pagination math drifted.
        //
        // TODO(v0.3): the schema does not yet carry a dedicated `pinned`
        // column — `pin_item` currently only clears `expires_at`, which is
        // indistinguishable from a never-expiring default row. Once a real
        // `pinned BOOLEAN` column lands, extend the WHERE clause with
        // `AND (pinned = 0 OR pinned IS NULL)` so explicitly pinned items
        // survive the prune.
        let res = db.conn().execute(
            "DELETE FROM clipboard_items WHERE id IN (
                SELECT id FROM clipboard_items
                ORDER BY wall_time ASC
                LIMIT ?1
            )",
            rusqlite::params![excess as i64],
        );
        match res {
            Ok(n) => tracing::debug!(
                "pruned {} of {} requested items over history_limit={}",
                n,
                excess,
                config.history_limit
            ),
            Err(e) => tracing::warn!("prune_history failed: {e}"),
        }
    }
}

#[tracing::instrument(name = "load_local_key")]
fn load_local_key() -> [u8; 32] {
    #[cfg(target_os = "macos")]
    {
        match crate::keychain::load_or_create() {
            Ok(kp) => {
                tracing::info!("device fingerprint={}", kp.fingerprint());
                kp.local_enc_key()
            }
            Err(e) => {
                tracing::warn!("Keychain unavailable ({e}), using ephemeral key");
                DeviceKeypair::generate().local_enc_key()
            }
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        // Keychain not available on non-macOS; use an ephemeral key for CI/Linux builds.
        // On production macOS this branch is never compiled in.
        tracing::warn!("Non-macOS platform: using ephemeral encryption key (data not persisted across restarts)");
        DeviceKeypair::generate().local_enc_key()
    }
}

#[tracing::instrument(name = "load_config")]
fn load_config() -> AppConfig {
    let path = paths::config_path();
    AppConfig::load(&path).unwrap_or_else(|_| {
        let cfg = AppConfig::default();
        if let Err(e) = cfg.save(&path) {
            tracing::warn!("could not save default config: {e}");
        }
        cfg
    })
}

/// Loads the persistent device_id from disk, creating it on first run.
///
/// Fixes arch LOW #24: previously the daemon regenerated a fresh UUID on
/// every restart, which broke P2P pairing and confused cloud peers. We now
/// persist a UUID v4 to `app_support_dir()/device_id` (or
/// `COPYPASTE_DEVICE_ID_PATH` when set) and chmod the file to `0o600` on
/// Unix so it is not world-readable.
///
/// On parse failure of an existing file we log + regenerate rather than
/// erroring — corrupt state should not block daemon startup.
#[tracing::instrument(name = "load_or_create_device_id")]
fn load_or_create_device_id() -> anyhow::Result<uuid::Uuid> {
    let path = paths::device_id_path()?;

    if let Ok(contents) = std::fs::read_to_string(&path) {
        let trimmed = contents.trim();
        match uuid::Uuid::parse_str(trimmed) {
            Ok(id) => {
                tracing::info!(device_id = %id, "loaded persistent device_id");
                return Ok(id);
            }
            Err(e) => {
                tracing::warn!(
                    path = %path.display(),
                    error = %e,
                    "device_id file unparsable, regenerating"
                );
            }
        }
    }

    // Ensure parent dir exists before writing.
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let id = uuid::Uuid::new_v4();
    std::fs::write(&path, id.to_string())?;

    // Restrict to owner-only on Unix.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o600);
        if let Err(e) = std::fs::set_permissions(&path, perms) {
            tracing::warn!(
                path = %path.display(),
                error = %e,
                "could not chmod device_id to 0600"
            );
        }
    }

    tracing::info!(device_id = %id, path = %path.display(), "created persistent device_id");
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// arch LOW #24 regression: the device_id must survive restarts.
    /// Two consecutive calls to `load_or_create_device_id` with the same
    /// backing file must return the same UUID.
    #[test]
    fn device_id_persists_across_restart() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("device_id");

        // SAFETY: env mutation is process-global. We use a unique tmpdir path
        // so parallel tests don't collide on the value, and we restore the
        // previous value after the test.
        let prev = std::env::var_os("COPYPASTE_DEVICE_ID_PATH");
        unsafe {
            std::env::set_var("COPYPASTE_DEVICE_ID_PATH", &path);
        }

        let first = load_or_create_device_id().expect("first call must succeed");
        assert!(
            path.exists(),
            "device_id file must be written on first call"
        );

        let second = load_or_create_device_id().expect("second call must succeed");

        // Restore env before assertions so a failure doesn't leak state.
        unsafe {
            match prev {
                Some(v) => std::env::set_var("COPYPASTE_DEVICE_ID_PATH", v),
                None => std::env::remove_var("COPYPASTE_DEVICE_ID_PATH"),
            }
        }

        assert_eq!(first, second, "device_id must persist across restarts");

        // On Unix the file must be 0o600.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600, "device_id file must be chmod 0600");
        }
    }
}
