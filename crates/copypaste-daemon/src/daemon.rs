use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::interval;
use sha2::{Sha256, Digest};
use copypaste_core::{
    AppConfig, Database, DeviceKeypair,
    encrypt_item, insert_item, upsert_fts, ClipboardItem,
    detect, find_recent_by_hash,
};
use crate::{clipboard::ClipboardMonitor, paths};
#[cfg(unix)]
use crate::ipc::IpcServer;

/// 60 seconds expressed in milliseconds — duplicate window for content dedup.
const DEDUP_WINDOW_MS: i64 = 60_000;

pub async fn run() -> anyhow::Result<()> {
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
        Database::open(&db_path, &local_key)
            .map_err(|e| anyhow::anyhow!("Database: {e}"))?
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

    let mut monitor = ClipboardMonitor::new(config.max_text_size_bytes);
    let mut ticker = interval(Duration::from_millis(config.poll_interval_ms));
    let mut cleanup_ticks: u64 = 0;
    // In-memory cache of the last stored content hash — allows skipping the DB
    // query for consecutive identical clipboard contents (fast path).
    let mut last_hash: Option<String> = None;
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
            tokio::select! {
                _ = ticker.tick() => {
                    handle_tick(&mut monitor, &db, &local_key, &config, &private_mode, &mut last_hash).await;
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
                _ = sigterm.recv() => {
                    tracing::info!("SIGTERM received, shutting down");
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
                    handle_tick(&mut monitor, &db, &local_key, &config, &private_mode, &mut last_hash).await;
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
    last_hash: &mut Option<String>,
) {
    // Skip recording when private/pause mode is active
    if private_mode.load(Ordering::Relaxed) {
        // Still poll to advance the change-count so we don't replay on resume
        let _ = monitor.poll();
        tracing::debug!("private mode active: skipping clipboard recording");
        return;
    }

    match monitor.poll() {
        Ok(Some(content)) => {
            let bytes = content.as_bytes();
            let text = std::str::from_utf8(bytes).unwrap_or("");

            // --- Content deduplication (SHA-256) ---
            let hash_bytes = Sha256::digest(bytes);
            let hash_hex = hex::encode(hash_bytes);

            // Fast path: same hash as the very last stored item — skip immediately.
            if last_hash.as_deref() == Some(hash_hex.as_str()) {
                tracing::trace!("dedup(fast): skipping identical clipboard content");
                return;
            }

            // Slow path: query DB for a matching hash within the last 60 seconds.
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as i64;
            {
                let db_guard = db.lock().await;
                match find_recent_by_hash(&db_guard, &hash_hex, now_ms, DEDUP_WINDOW_MS) {
                    Ok(Some(existing_id)) => {
                        tracing::debug!(
                            "dedup(db): skipping duplicate, existing id={} hash={}",
                            existing_id,
                            &hash_hex[..8],
                        );
                        // Update fast-path cache so next tick is O(1).
                        *last_hash = Some(hash_hex);
                        return;
                    }
                    Ok(None) => {} // Not a duplicate — proceed with insert.
                    Err(e) => tracing::warn!("dedup query error: {e}"),
                }
            }
            // --- End deduplication ---

            let is_sensitive = detect(text).is_some();

            let (nonce, ciphertext) = encrypt_item(bytes, local_key);
            let mut item = ClipboardItem::new_text(ciphertext, nonce.to_vec(), 0);
            item.is_sensitive = is_sensitive;
            item.content_hash = Some(hash_hex.clone());

            if is_sensitive {
                item.expires_at = Some(
                    now_ms + (config.sensitive_ttl_local_secs as i64 * 1000),
                );
            }

            let db_guard = db.lock().await;
            match insert_item(&db_guard, &item) {
                Ok(_) => {
                    // Update the in-memory fast-path cache.
                    *last_hash = Some(hash_hex);

                    tracing::debug!(
                        "stored item id={} sensitive={}",
                        item.id,
                        is_sensitive
                    );
                    // Index plaintext for FTS5 before encryption is discarded
                    if item.content_type == "text" {
                        if let Err(e) = upsert_fts(&db_guard, &item.id, text) {
                            tracing::warn!("fts index failed for id={}: {e}", item.id);
                        }
                    } else if let Err(e) = upsert_fts(&db_guard, &item.id, "") {
                        tracing::warn!("fts empty index failed for id={}: {e}", item.id);
                    }
                    // Prune oldest items if over history_limit
                    let total = copypaste_core::count_items(&db_guard).unwrap_or(0) as usize;
                    if total > config.history_limit {
                        let excess = total - config.history_limit;
                        if let Ok(oldest) = copypaste_core::get_page(&db_guard, excess, config.history_limit) {
                            for old in &oldest {
                                let _ = copypaste_core::delete_item(&db_guard, &old.id);
                            }
                            tracing::debug!("pruned {} items over history_limit={}", excess, config.history_limit);
                        }
                    }
                }
                Err(e) => tracing::warn!("failed to store item: {e}"),
            }
        }
        Ok(None) => {}
        Err(e) => tracing::warn!("clipboard poll error: {e}"),
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
