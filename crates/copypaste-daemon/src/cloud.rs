//! Cloud sync orchestrator for Supabase.
//!
//! Enabled at runtime when `SUPABASE_URL` and `SUPABASE_ANON_KEY` environment
//! variables are set (regardless of whether the `cloud-sync` Cargo feature is
//! compiled in — the feature gate controls whether the `reqwest` dep is present).
//!
//! Two background tasks are spawned:
//! - **push_loop**: receives new [`ClipboardItem`]s from a broadcast channel and
//!   POSTs them to `POST /rest/v1/clipboard_items`.
//! - **realtime_loop**: polls `GET /rest/v1/clipboard_items?order=wall_time.desc&limit=20`
//!   every 10 seconds and inserts any unknown items into the local DB.
//!   (Full WebSocket realtime requires the separate `copypaste-supabase` crate;
//!   this implementation uses polling so the daemon compiles without extra deps.)

use std::sync::Arc;
use tokio::sync::Mutex;

use copypaste_core::{ClipboardItem, Database};

// ── CloudConfig ───────────────────────────────────────────────────────────────

/// Runtime configuration read from environment variables.
#[derive(Debug, Clone)]
pub struct CloudConfig {
    /// Supabase project base URL, e.g. `https://abc.supabase.co`.
    pub supabase_url: String,
    /// Supabase anonymous/public API key.
    pub anon_key: String,
}

impl CloudConfig {
    /// Returns `Some(config)` if both `SUPABASE_URL` and `SUPABASE_ANON_KEY`
    /// are set in the environment; otherwise `None`.
    pub fn from_env() -> Option<Self> {
        let supabase_url = std::env::var("SUPABASE_URL").ok()?;
        let anon_key = std::env::var("SUPABASE_ANON_KEY").ok()?;
        Some(Self {
            supabase_url: supabase_url.trim_end_matches('/').to_owned(),
            anon_key,
        })
    }
}

// ── CloudHandle ───────────────────────────────────────────────────────────────

/// Handle returned by [`start_cloud`].  Drop it to abandon the background tasks
/// (they will exit when the shutdown channel is signalled).
pub struct CloudHandle {
    shutdown_tx: tokio::sync::oneshot::Sender<()>,
}

impl CloudHandle {
    /// Signal both background tasks to stop.
    pub fn shutdown(self) {
        let _ = self.shutdown_tx.send(());
    }
}

// ── Public entry point ────────────────────────────────────────────────────────

/// Start the cloud-sync background tasks.
///
/// # Arguments
/// - `config` — Supabase credentials.
/// - `db` — shared local database (used by the realtime/poll loop to insert remote items).
/// - `new_item_rx` — broadcast receiver; every locally created item is pushed to Supabase.
///
/// Returns a [`CloudHandle`] that can be used to stop the tasks.
pub async fn start_cloud(
    config: CloudConfig,
    db: Arc<Mutex<Database>>,
    new_item_rx: tokio::sync::broadcast::Receiver<ClipboardItem>,
) -> anyhow::Result<CloudHandle> {
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    // We need two copies of the shutdown signal — use a shared Notify.
    let shutdown = Arc::new(tokio::sync::Notify::new());

    // Wire the oneshot into the Notify so both loops see the signal.
    let notify_clone = shutdown.clone();
    tokio::spawn(async move {
        let _ = shutdown_rx.await;
        notify_clone.notify_waiters();
    });

    // Try to obtain a bearer token (email/password auth or fall back to anon key).
    let bearer = resolve_bearer(&config).await;

    // Task A: push new local items to Supabase REST.
    let push_config = config.clone();
    let push_bearer = bearer.clone();
    let push_shutdown = shutdown.clone();
    tokio::spawn(push_loop(push_config, push_bearer, new_item_rx, push_shutdown));

    // Task B: poll Supabase REST for remote items and insert unknown ones locally.
    let poll_config = config.clone();
    let poll_bearer = bearer.clone();
    let poll_shutdown = shutdown.clone();
    tokio::spawn(realtime_loop(poll_config, poll_bearer, db, poll_shutdown));

    tracing::info!("cloud-sync started (url={})", config.supabase_url);
    Ok(CloudHandle { shutdown_tx })
}

// ── Bearer token resolution ───────────────────────────────────────────────────

/// Attempt email/password sign-in; fall back to the anonymous key as bearer.
async fn resolve_bearer(config: &CloudConfig) -> String {
    if let (Ok(email), Ok(password)) = (
        std::env::var("SUPABASE_EMAIL"),
        std::env::var("SUPABASE_PASSWORD"),
    ) {
        match sign_in_with_password(config, &email, &password).await {
            Ok(token) => {
                tracing::info!("cloud-sync: signed in as {email}");
                return token;
            }
            Err(e) => {
                tracing::warn!("cloud-sync: email/password sign-in failed ({e}); using anon key");
            }
        }
    }
    config.anon_key.clone()
}

/// POST `/auth/v1/token?grant_type=password` and return the `access_token`.
async fn sign_in_with_password(
    config: &CloudConfig,
    email: &str,
    password: &str,
) -> anyhow::Result<String> {
    let client = reqwest::Client::new();
    let url = format!("{}/auth/v1/token?grant_type=password", config.supabase_url);

    let resp = client
        .post(&url)
        .header("apikey", &config.anon_key)
        .json(&serde_json::json!({ "email": email, "password": password }))
        .send()
        .await?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("auth failed ({status}): {body}");
    }

    let json: serde_json::Value = resp.json().await?;
    let token = json["access_token"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("no access_token in auth response"))?
        .to_owned();
    Ok(token)
}

// ── Push loop ─────────────────────────────────────────────────────────────────

/// Receive locally created items from the broadcast channel and POST them to
/// `POST /rest/v1/clipboard_items`.
async fn push_loop(
    config: CloudConfig,
    bearer: String,
    mut rx: tokio::sync::broadcast::Receiver<ClipboardItem>,
    shutdown: Arc<tokio::sync::Notify>,
) {
    let client = reqwest::Client::new();
    let rest_url = format!("{}/rest/v1/clipboard_items", config.supabase_url);

    loop {
        tokio::select! {
            result = rx.recv() => {
                match result {
                    Ok(item) => {
                        if let Err(e) = push_item(&client, &rest_url, &config.anon_key, &bearer, &item).await {
                            tracing::warn!("cloud-sync push failed for id={}: {e}", item.id);
                        } else {
                            tracing::debug!("cloud-sync pushed id={}", item.id);
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("cloud-sync push_loop: lagged by {n} items");
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        tracing::info!("cloud-sync push_loop: channel closed, exiting");
                        break;
                    }
                }
            }
            _ = shutdown.notified() => {
                tracing::info!("cloud-sync push_loop: shutdown received");
                break;
            }
        }
    }
}

/// Serialise a [`ClipboardItem`] and POST it to the Supabase REST endpoint.
async fn push_item(
    client: &reqwest::Client,
    url: &str,
    anon_key: &str,
    bearer: &str,
    item: &ClipboardItem,
) -> anyhow::Result<()> {
    let body = clipboard_item_to_json(item);

    let resp = client
        .post(url)
        .header("apikey", anon_key)
        .header("Authorization", format!("Bearer {bearer}"))
        .header("Content-Type", "application/json")
        .header("Prefer", "return=minimal")
        .json(&body)
        .send()
        .await?;

    let status = resp.status();
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        anyhow::bail!("REST POST failed ({status}): {text}");
    }
    Ok(())
}

// ── Realtime / poll loop ──────────────────────────────────────────────────────

/// Poll Supabase REST every 10 s for recent items from other devices and insert
/// any that are not already in the local database.
async fn realtime_loop(
    config: CloudConfig,
    bearer: String,
    db: Arc<Mutex<Database>>,
    shutdown: Arc<tokio::sync::Notify>,
) {
    let client = reqwest::Client::new();
    let poll_url = format!(
        "{}/rest/v1/clipboard_items?order=wall_time.desc&limit=20",
        config.supabase_url
    );
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(10));

    loop {
        tokio::select! {
            _ = interval.tick() => {
                match fetch_remote_items(&client, &poll_url, &config.anon_key, &bearer).await {
                    Err(e) => tracing::warn!("cloud-sync poll failed: {e}"),
                    Ok(remote_items) => {
                        let db_guard = db.lock().await;
                        for item in remote_items {
                            match exists_item(&db_guard, &item.id) {
                                Ok(true) => {} // already local
                                Ok(false) => {
                                    if let Err(e) = copypaste_core::insert_item(&db_guard, &item) {
                                        tracing::warn!(
                                            "cloud-sync: failed to insert remote id={}: {e}",
                                            item.id
                                        );
                                    } else {
                                        tracing::info!("cloud-sync: synced remote id={}", item.id);
                                    }
                                }
                                Err(e) => {
                                    tracing::warn!("cloud-sync: exists_item error for id={}: {e}", item.id);
                                }
                            }
                        }
                    }
                }
            }
            _ = shutdown.notified() => {
                tracing::info!("cloud-sync realtime_loop: shutdown received");
                break;
            }
        }
    }
}

/// `GET /rest/v1/clipboard_items` and deserialise the response into a `Vec<ClipboardItem>`.
async fn fetch_remote_items(
    client: &reqwest::Client,
    url: &str,
    anon_key: &str,
    bearer: &str,
) -> anyhow::Result<Vec<ClipboardItem>> {
    let resp = client
        .get(url)
        .header("apikey", anon_key)
        .header("Authorization", format!("Bearer {bearer}"))
        .send()
        .await?;

    let status = resp.status();
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        anyhow::bail!("REST GET failed ({status}): {text}");
    }

    let rows: Vec<serde_json::Value> = resp.json().await?;
    let items = rows
        .into_iter()
        .filter_map(|v| json_to_clipboard_item(&v))
        .collect();
    Ok(items)
}

// ── Helper: exists_item ───────────────────────────────────────────────────────

/// Return `true` when a row with the given `id` already exists locally.
pub fn exists_item(db: &Database, id: &str) -> Result<bool, anyhow::Error> {
    let count: i64 = db
        .conn()
        .query_row(
            "SELECT COUNT(1) FROM clipboard_items WHERE id = ?1",
            rusqlite::params![id],
            |row| row.get(0),
        )
        .map_err(|e| anyhow::anyhow!("exists_item query: {e}"))?;
    Ok(count > 0)
}

// ── JSON serialisation helpers ────────────────────────────────────────────────

/// Convert a [`ClipboardItem`] to the JSON shape expected by the Supabase REST API.
/// Binary fields (`content`, `content_nonce`) are base64-encoded.
fn clipboard_item_to_json(item: &ClipboardItem) -> serde_json::Value {
    use base64::Engine as _;
    let b64 = base64::engine::general_purpose::STANDARD;

    serde_json::json!({
        "id":            item.id,
        "item_id":       item.item_id,
        "content_type":  item.content_type,
        "content":       item.content.as_deref().map(|b| b64.encode(b)),
        "content_nonce": item.content_nonce.as_deref().map(|b| b64.encode(b)),
        "blob_ref":      item.blob_ref,
        "is_sensitive":  item.is_sensitive,
        "is_synced":     item.is_synced,
        "lamport_ts":    item.lamport_ts,
        "wall_time":     item.wall_time,
        "expires_at":    item.expires_at,
        "app_bundle_id": item.app_bundle_id,
    })
}

/// Attempt to deserialise a Supabase REST row into a [`ClipboardItem`].
/// Returns `None` if required fields are missing.
fn json_to_clipboard_item(v: &serde_json::Value) -> Option<ClipboardItem> {
    use base64::Engine as _;
    let b64 = base64::engine::general_purpose::STANDARD;

    let id = v["id"].as_str()?.to_owned();
    let item_id = v["item_id"].as_str().unwrap_or(&id).to_owned();
    let content_type = v["content_type"].as_str().unwrap_or("text").to_owned();

    let content = v["content"]
        .as_str()
        .and_then(|s| b64.decode(s).ok());
    let content_nonce = v["content_nonce"]
        .as_str()
        .and_then(|s| b64.decode(s).ok());

    let blob_ref = v["blob_ref"].as_str().map(str::to_owned);
    let is_sensitive = v["is_sensitive"].as_bool().unwrap_or(false);
    let is_synced = v["is_synced"].as_bool().unwrap_or(true);
    let lamport_ts = v["lamport_ts"].as_i64().unwrap_or(0);
    let wall_time = v["wall_time"].as_i64().unwrap_or(0);
    let expires_at = v["expires_at"].as_i64();
    let app_bundle_id = v["app_bundle_id"].as_str().map(str::to_owned);

    Some(ClipboardItem {
        id,
        item_id,
        content_type,
        content,
        content_nonce,
        blob_ref,
        is_sensitive,
        is_synced,
        lamport_ts,
        wall_time,
        expires_at,
        app_bundle_id,
        content_hash: None,
    })
}
