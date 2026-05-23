#[cfg(unix)]
use crate::ipc::IpcServer;
use crate::{
    clipboard::{ClipboardContent, ClipboardMonitor},
    p2p, paths,
};
use copypaste_core::{
    chunks_to_blob, detect, encode_image, encrypt_item, insert_item, upsert_fts, AppConfig,
    ClipboardItem, Database, DeviceKeypair,
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

    let local_key = load_local_key();
    tracing::info!("local encryption key ready");

    let db_path = paths::db_path();
    let db = Arc::new(Mutex::new(
        Database::open(&db_path, &local_key).map_err(|e| anyhow::anyhow!("Database: {e}"))?,
    ));
    tracing::info!("database opened at {}", db_path.display());

    // Shared private-mode flag: when true, the clipboard monitor skips recording.
    // This is set/cleared via the IPC `set_private_mode` command.
    let private_mode = Arc::new(AtomicBool::new(false));

    #[cfg(unix)]
    let socket_path = paths::socket_path();
    #[cfg(unix)]
    {
        let ipc_db = db.clone();
        let ipc_private_mode = private_mode.clone();
        let socket_clone = socket_path.clone();
        tokio::spawn(async move {
            let server = IpcServer::new(ipc_db, ipc_private_mode);
            if let Err(e) = server.serve(&socket_clone).await {
                tracing::error!("IPC server error: {e}");
            }
        });
    }

    // Broadcast channel: carries newly-inserted clipboard items to any
    // subscriber (P2P sync, cloud-sync, future extensions). Capacity 64 — lagging
    // receivers drop oldest items and log a warning.
    let (new_item_tx, _new_item_rx) = broadcast::channel::<ClipboardItem>(64);

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
    let sync_device_id = uuid::Uuid::new_v4().to_string();
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
                    if sensitive_cleanup_ticks >= (5_000 / config.poll_interval_ms.max(1)) {
                        sensitive_cleanup_ticks = 0;
                        let db_guard = db.lock().await;
                        let now_ms = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_millis() as i64;
                        match copypaste_core::delete_sensitive_expired(&db_guard, now_ms, sensitive_ttl_ms) {
                            Ok(n) if n > 0 => tracing::info!("sensitive TTL cleanup: wiped {n} sensitive items"),
                            Ok(_) => {}
                            Err(e) => tracing::warn!("sensitive TTL cleanup error: {e}"),
                        }
                    }

                    // General expires_at TTL: run every 60 seconds.
                    if cleanup_ticks >= (60_000 / config.poll_interval_ms.max(1)) {
                        cleanup_ticks = 0;
                        let db_guard = db.lock().await;
                        let now_ms = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
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
        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    handle_tick(&mut monitor, &db, &local_key, &config, &private_mode, &new_item_tx).await;
                    cleanup_ticks += 1;
                    sensitive_cleanup_ticks += 1;

                    // Sensitive item TTL: run every 5 seconds.
                    if sensitive_cleanup_ticks >= (5_000 / config.poll_interval_ms.max(1)) {
                        sensitive_cleanup_ticks = 0;
                        let db_guard = db.lock().await;
                        let now_ms = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_millis() as i64;
                        match copypaste_core::delete_sensitive_expired(&db_guard, now_ms, sensitive_ttl_ms) {
                            Ok(n) if n > 0 => tracing::info!("sensitive TTL cleanup: wiped {n} sensitive items"),
                            Ok(_) => {}
                            Err(e) => tracing::warn!("sensitive TTL cleanup error: {e}"),
                        }
                    }

                    // General expires_at TTL: run every 60 seconds.
                    if cleanup_ticks >= (60_000 / config.poll_interval_ms.max(1)) {
                        cleanup_ticks = 0;
                        let db_guard = db.lock().await;
                        let now_ms = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
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
            if let Some(item) = handle_text(text, db, local_key, config).await {
                // Broadcast to P2P + cloud-sync subscribers (and any future consumer).
                // A send error only means there are no active receivers —
                // that is normal when both P2P and cloud-sync are disabled.
                let _ = new_item_tx.send(item);
            }
        }
        Ok(Some(ClipboardContent::Image(raw_bytes))) => {
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

    // NOTE: encrypt_item now returns Result (crypto wave). One-line unblock
    // so the daemon crate compiles for clipboard test verification; the
    // crypto wave owner should fold this into their handler-error pass.
    let (nonce, ciphertext) = match encrypt_item(text.as_bytes(), local_key) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!("encrypt_item failed for text: {e}");
            return None;
        }
    };
    let mut item = ClipboardItem::new_text(ciphertext, nonce.to_vec(), 0);
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
            tracing::debug!("stored text item id={} sensitive={}", item.id, is_sensitive);
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
                    tracing::debug!("stored image item id={}", item.id);
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
        if let Ok(oldest) = copypaste_core::get_page(db, excess, config.history_limit) {
            for old in &oldest {
                let _ = copypaste_core::delete_item(db, &old.id);
            }
            tracing::debug!(
                "pruned {} items over history_limit={}",
                excess,
                config.history_limit
            );
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
