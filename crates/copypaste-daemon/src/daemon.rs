use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::interval;
use copypaste_core::{
    AppConfig, Database, DeviceKeypair,
    encrypt_item, insert_item, upsert_fts, ClipboardItem,
    detect,
};
use crate::{clipboard::ClipboardMonitor, ipc::IpcServer, paths};

/// Run the daemon until `Ctrl+C` / `SIGTERM` is received.
///
/// This is the entry point used on non-macOS platforms and in tests.
pub async fn run() -> anyhow::Result<()> {
    run_with_quit_flag(Arc::new(AtomicBool::new(false))).await
}

/// Run the daemon until `Ctrl+C`, `SIGTERM`, or `quit_flag` is set.
///
/// On macOS the tray icon sets `quit_flag` when the user clicks Quit.
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
        Database::open(&db_path, &local_key)
            .map_err(|e| anyhow::anyhow!("Database: {e}"))?
    ));
    tracing::info!("database opened at {}", db_path.display());

    let ipc_db = db.clone();
    let socket_path = paths::socket_path();
    let socket_clone = socket_path.clone();
    tokio::spawn(async move {
        let server = IpcServer::new(ipc_db);
        if let Err(e) = server.serve(&socket_clone).await {
            tracing::error!("IPC server error: {e}");
        }
    });

    let mut monitor = ClipboardMonitor::new(config.max_text_size_bytes);
    let mut ticker = interval(Duration::from_millis(config.poll_interval_ms));
    let mut cleanup_ticks: u64 = 0;

    tracing::info!("clipboard monitor started");

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
                    handle_tick(&mut monitor, &db, &local_key, &config).await;
                    cleanup_ticks += 1;
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
                    handle_tick(&mut monitor, &db, &local_key, &config).await;
                    cleanup_ticks += 1;
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

    let _ = std::fs::remove_file(&socket_path);
    tracing::info!("daemon stopped");
    Ok(())
}

async fn handle_tick(
    monitor: &mut ClipboardMonitor,
    db: &Arc<Mutex<Database>>,
    local_key: &[u8; 32],
    config: &AppConfig,
) {
    match monitor.poll() {
        Ok(Some(content)) => {
            let bytes = content.as_bytes();
            let text = std::str::from_utf8(bytes).unwrap_or("");
            let is_sensitive = detect(text).is_some();

            let (nonce, ciphertext) = encrypt_item(bytes, local_key);
            let mut item = ClipboardItem::new_text(ciphertext, nonce.to_vec(), 0);
            item.is_sensitive = is_sensitive;

            if is_sensitive {
                let now_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as i64;
                item.expires_at = Some(
                    now_ms + (config.sensitive_ttl_local_secs as i64 * 1000),
                );
            }

            let db_guard = db.lock().await;
            match insert_item(&db_guard, &item) {
                Ok(_) => {
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
