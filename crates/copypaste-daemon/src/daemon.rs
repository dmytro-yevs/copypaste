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

    tracing::info!("clipboard monitor started");

    #[cfg(target_os = "macos")]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut sigterm = signal(SignalKind::terminate())?;
        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    handle_tick(&mut monitor, &db, &local_key, &config).await;
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
