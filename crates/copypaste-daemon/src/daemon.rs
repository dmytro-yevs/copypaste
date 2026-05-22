use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::interval;
use copypaste_core::{
    AppConfig, Database, DeviceKeypair,
    encrypt_item, insert_item, upsert_fts, ClipboardItem,
    encode_image, chunks_to_blob,
    detect,
};
use crate::{clipboard::{ClipboardContent, ClipboardMonitor}, ipc::IpcServer, paths};

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
        Ok(Some(ClipboardContent::Text(text))) => {
            handle_text(text, db, local_key, config).await;
        }
        Ok(Some(ClipboardContent::Image(raw_bytes))) => {
            handle_image(raw_bytes, db, local_key, config).await;
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
) {
    let is_sensitive = detect(&text).is_some();

    let (nonce, ciphertext) = encrypt_item(text.as_bytes(), local_key);
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
            tracing::debug!("stored text item id={} sensitive={}", item.id, is_sensitive);
            if let Err(e) = upsert_fts(&db_guard, &item.id, &text) {
                tracing::warn!("fts index failed for id={}: {e}", item.id);
            }
            prune_history(&db_guard, config);
        }
        Err(e) => tracing::warn!("failed to store text item: {e}"),
    }
}

async fn handle_image(
    raw_bytes: Vec<u8>,
    db: &Arc<Mutex<Database>>,
    local_key: &[u8; 32],
    config: &AppConfig,
) {
    // Derive a stable file_id from the raw bytes hash (first 16 bytes of SHA-256).
    // This gives a deterministic ID for deduplication without storing plaintext.
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    raw_bytes.hash(&mut hasher);
    let hash64 = hasher.finish();
    let mut file_id = [0u8; 16];
    file_id[..8].copy_from_slice(&hash64.to_be_bytes());
    // XOR with timestamp to ensure uniqueness across same-content pastes
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64;
    file_id[8..].copy_from_slice(&ts.to_be_bytes());

    match encode_image(&raw_bytes, local_key, &file_id) {
        Ok((meta, chunks)) => {
            let blob = chunks_to_blob(&chunks);
            let meta_json = format!(
                r#"{{"width":{},"height":{},"original_size":{},"chunk_count":{},"file_id":{:?}}}"#,
                meta.width, meta.height, meta.original_size, meta.chunk_count,
                meta.file_id
            );
            let item = ClipboardItem::new_image(blob, meta_json, 0);
            tracing::debug!(
                "image encoded: {}x{} px, {} chunks, original_size={}",
                meta.width, meta.height, meta.chunk_count, meta.original_size
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
                }
                Err(e) => tracing::warn!("failed to store image item: {e}"),
            }
        }
        Err(e) => {
            tracing::warn!("image encode failed (skipping): {e}");
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
            tracing::debug!("pruned {} items over history_limit={}", excess, config.history_limit);
        }
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
