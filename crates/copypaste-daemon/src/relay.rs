//! Relay-as-database sync client (sync path #2 of 3) — daemon side.
//!
//! This is the producer/consumer that makes the **relay-as-database** path work
//! end-to-end, independent of P2P and Supabase. It is gated behind the
//! `relay-sync` cargo feature and is active at runtime iff `config.relay_url`
//! is set.
//!
//! # Architecture — shared-account inbox
//!
//! ALL of an account's devices use ONE relay inbox `device_id`, derived
//! deterministically from the shared sync key
//! ([`copypaste_core::derive_relay_inbox_id`]). Every device co-registers that
//! id with the relay (each gets an INDEPENDENT auth token — R1a), then pushes to
//! and subscribes to it. The relay only ever sees opaque ciphertext + the opaque
//! inbox id.
//!
//! # Pipeline (mirrors [`crate::cloud`])
//!
//! - **register:** `POST {relay_url}/devices` `{device_id, device_name,
//!   public_key_b64}` → 201 `{auth_token}`. Token cached in a `0600` file. On a
//!   401 during push/pull the token is dropped and re-registered.
//! - **push:** subscribe the `new_item_tx` broadcast; for each local item reuse
//!   [`sync_common::decrypt_item_plaintext`] →
//!   [`sync_common::wrap_and_check_cloud_upload_plaintext`] →
//!   `encrypt_for_cloud(sync_key, item_id, ...)` (the SAME blob the Supabase path
//!   produces) → build the envelope → `POST {relay_url}/devices/{inbox}/items`.
//! - **receive:** poll `GET {relay_url}/devices/{inbox}/items?since=&since_id=`,
//!   decode each item's envelope, `decrypt_from_cloud`, then reuse
//!   [`sync_common::build_local_item`] + [`copypaste_core::insert_item`] with the
//!   exact LWW + quota-prune the Supabase poll path uses. A `(wall_time, id)`
//!   watermark is held in memory across ticks.
//! - **self-echo:** the daemon both pushes to and subscribes to the same inbox,
//!   so a row it pushed comes back on the next pull. LWW dedup on `item_id`
//!   makes that a no-op (the local copy has an equal `lamport_ts`, so it is
//!   skipped) — confirmed by the receive path's `<=` LWW guard.
//!
//! # Security
//! - The inbox id is SECRET-derived (HKDF of the sync key) — NEVER logged.
//! - The auth token is a credential — NEVER logged; persisted `0600`.
//! - The relay sees only ciphertext; plaintext/key bytes are never logged.
//! - All HTTP is async (reqwest) — the tokio runtime is never blocked; the only
//!   blocking work (SQLCipher writes, AEAD) runs in `spawn_blocking`.

use std::path::PathBuf;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use base64::Engine as _;
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, Notify};

use copypaste_core::{
    decrypt_from_cloud, derive_relay_inbox_id, derive_relay_public_key, encrypt_for_cloud,
    exists_item_by_item_id, get_item_by_item_id, insert_item, prune_to_cap, AppConfig,
    ClipboardItem, Database, SyncKey,
};

use crate::sync_common::{
    build_local_item, decode_payload_ct, decrypt_item_plaintext, replace_cloud_item_by_item_id,
    wrap_and_check_cloud_upload_plaintext,
};

// ── Tuning ──────────────────────────────────────────────────────────────────

/// Per-request HTTP timeout. A stalled relay must not hang a loop forever.
const HTTP_TIMEOUT: Duration = Duration::from_secs(30);

/// Poll interval for the receive loop (the relay also offers SSE; polling is the
/// portable backstop and matches the at-least-once contract). Kept tight so
/// cross-device latency is low without hammering the relay.
const POLL_INTERVAL: Duration = Duration::from_secs(5);

/// Max items requested per pull tick. When a batch comes back full we re-poll
/// immediately (burst drain) rather than waiting a full interval.
const PULL_LIMIT: usize = 50;

/// Filename of the cached relay auth token inside the app data dir.
const RELAY_TOKEN_FILE: &str = "relay_token";

// ── Wire envelope ─────────────────────────────────────────────────────────────

/// The decoded `content_b64` envelope. `content_b64` (on the relay wire) is
/// `base64(JSON(this struct))`; `ct_b64` inside it is
/// `base64(encrypt_for_cloud(sync_key, item_id, wrapped_plaintext))` — the SAME
/// blob the Supabase path stores. This is the exact shape the Android SSE
/// receiver already decodes.
#[derive(Debug, Serialize, Deserialize)]
struct RelayEnvelope {
    item_id: String,
    lamport_ts: i64,
    ct_b64: String,
}

/// Relay register request body.
#[derive(Debug, Serialize)]
struct RegisterBody {
    device_id: String,
    device_name: String,
    public_key_b64: String,
}

/// Relay register response (we only need the token).
#[derive(Debug, Deserialize)]
struct RegisterResp {
    auth_token: String,
}

/// Relay push request body.
#[derive(Debug, Serialize)]
struct PushBody {
    content_type: String,
    content_b64: String,
    wall_time: u64,
}

/// One element of the pull response array.
#[derive(Debug, Deserialize)]
struct PullItem {
    id: i64,
    content_type: String,
    content_b64: String,
    wall_time: u64,
}

// ── Errors ──────────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum RelayError {
    /// The configured `relay_url` is not a usable HTTPS (or loopback in tests) URL.
    #[error("relay_url is not a valid https URL")]
    InvalidUrl,
    /// Network / transport failure talking to the relay.
    #[error("relay request failed: {0}")]
    Transport(String),
    /// Relay returned an unexpected non-success status.
    #[error("relay returned status {0}")]
    Status(u16),
    /// Could not resolve the inbox id (no sync key set).
    #[error("no sync passphrase set — relay sync inactive")]
    NoSyncKey,
}

// ── Handle ──────────────────────────────────────────────────────────────────

/// Handle to the running relay orchestrator. Drop (or call [`shutdown`]) to stop
/// the push and receive loops.
///
/// [`shutdown`]: RelayHandle::shutdown
pub struct RelayHandle {
    shutdown: Arc<Notify>,
}

impl RelayHandle {
    /// Signal both loops to stop. Idempotent.
    pub fn shutdown(self) {
        self.shutdown.notify_waiters();
    }
}

impl Drop for RelayHandle {
    fn drop(&mut self) {
        self.shutdown.notify_waiters();
    }
}

// ── Token cache (0600 file) ─────────────────────────────────────────────────

/// Path to the cached relay token file (sibling of the device-key files).
fn token_path() -> Option<PathBuf> {
    crate::paths::try_app_support_dir()
        .ok()
        .map(|d| d.join(RELAY_TOKEN_FILE))
}

/// Load a previously-cached relay auth token, if any. Never errors hard — a
/// missing/unreadable token just means "re-register".
fn load_cached_token() -> Option<String> {
    let path = token_path()?;
    let raw = std::fs::read_to_string(&path).ok()?;
    let t = raw.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_owned())
    }
}

/// Persist the relay auth token to a `0600` file. Best-effort: a failure is
/// logged (without the token) and the token is still used in-memory for this run.
fn store_cached_token(token: &str) {
    let Some(path) = token_path() else {
        tracing::warn!("relay-sync: cannot resolve data dir to cache token");
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Err(e) = write_token_0600(&path, token) {
        tracing::warn!(error = %e, "relay-sync: failed to cache relay token (continuing in-memory)");
    }
}

/// Write `token` to `path` with `0600` perms via a temp-file + rename so a
/// reader never sees a partial or world-readable file.
fn write_token_0600(path: &std::path::Path, token: &str) -> std::io::Result<()> {
    use std::io::Write as _;
    let dir = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    let tmp = dir.join(format!(".{RELAY_TOKEN_FILE}.tmp"));
    let mut f = std::fs::File::create(&tmp)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        f.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    }
    f.write_all(token.as_bytes())?;
    f.sync_all()?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

// ── URL validation ────────────────────────────────────────────────────────────

/// Accept `https://...`; in tests also accept loopback `http://` so the mock
/// relay can be exercised. Mirrors `cloud::is_https_url`'s posture.
fn is_relay_url_ok(s: &str) -> bool {
    let lower = s.to_ascii_lowercase();
    if let Some(rest) = lower.strip_prefix("https://") {
        return rest
            .chars()
            .next()
            .is_some_and(|c| c != '/' && !c.is_whitespace());
    }
    #[cfg(test)]
    {
        if let Some(rest) = lower.strip_prefix("http://") {
            let host = rest.split(['/', ':']).next().unwrap_or_default();
            return matches!(host, "127.0.0.1" | "localhost" | "[::1]" | "::1");
        }
    }
    false
}

// ── Sync-key snapshot helper ────────────────────────────────────────────────

/// Snapshot the live sync-key bytes (the `SyncKey` itself is not `Send` across
/// some boundaries, and we never hold the lock across an await). Returns `None`
/// when no passphrase is set.
async fn snapshot_sync_key(sync_key: &Arc<Mutex<Option<SyncKey>>>) -> Option<[u8; 32]> {
    let guard = sync_key.lock().await;
    guard.as_ref().map(|k| *k.as_bytes())
}

// ── Register ────────────────────────────────────────────────────────────────

/// Register (or co-register) this device's shared-account inbox with the relay
/// and return a fresh auth token. The inbox id + public key are derived from
/// `sync_key_bytes` (SECRET-derived — never logged).
async fn register(
    client: &reqwest::Client,
    relay_url: &str,
    sync_key_bytes: &[u8; 32],
    device_name: &str,
) -> Result<String, RelayError> {
    let inbox_id = derive_relay_inbox_id(sync_key_bytes);
    let pubkey = derive_relay_public_key(sync_key_bytes);
    let public_key_b64 = base64::engine::general_purpose::STANDARD.encode(pubkey);

    let body = RegisterBody {
        device_id: inbox_id,
        device_name: device_name.to_owned(),
        public_key_b64,
    };
    let url = format!("{relay_url}/devices");
    let resp = client
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| RelayError::Transport(e.to_string()))?;
    let status = resp.status();
    // R1a: a fresh register always returns 201 with a new independent token,
    // whether or not the id was already co-registered by another device.
    if status.as_u16() != 201 {
        return Err(RelayError::Status(status.as_u16()));
    }
    let parsed: RegisterResp = resp
        .json()
        .await
        .map_err(|e| RelayError::Transport(format!("decode register response: {e}")))?;
    tracing::info!("relay-sync: registered shared inbox with relay (token cached)");
    Ok(parsed.auth_token)
}

/// Ensure we hold a valid token: return the cached one if present, else register
/// and cache a fresh one.
async fn ensure_token(
    client: &reqwest::Client,
    relay_url: &str,
    sync_key_bytes: &[u8; 32],
    device_name: &str,
    cached: &mut Option<String>,
) -> Result<String, RelayError> {
    if let Some(t) = cached.as_ref() {
        return Ok(t.clone());
    }
    let token = register(client, relay_url, sync_key_bytes, device_name).await?;
    store_cached_token(&token);
    *cached = Some(token.clone());
    Ok(token)
}

// ── Envelope build ────────────────────────────────────────────────────────────

/// Build the relay `content_b64` for one item: encrypt the wrapped plaintext for
/// the cloud (sync key + item_id AAD), wrap it in the JSON envelope, base64 it.
///
/// Returns `Ok(None)` when the item should be skipped (e.g. oversized, decrypt
/// failure) — never logs plaintext.
fn build_content_b64(
    item: &ClipboardItem,
    local_key: &zeroize::Zeroizing<[u8; 32]>,
    sync_key: &SyncKey,
) -> Option<String> {
    let plaintext = match decrypt_item_plaintext(item, local_key) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!("relay-sync: decrypt id={} failed: {e}; skipping", item.id);
            return None;
        }
    };
    let wrapped = match wrap_and_check_cloud_upload_plaintext(item, plaintext) {
        Ok(w) => w,
        Err(e) => {
            tracing::warn!("relay-sync: skip id={}: {e}", item.id);
            return None;
        }
    };
    let blob = match encrypt_for_cloud(sync_key, &item.item_id, &wrapped) {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!(
                "relay-sync: cloud encrypt id={} failed: {e}; skipping",
                item.id
            );
            return None;
        }
    };
    let ct_b64 = base64::engine::general_purpose::STANDARD.encode(&blob);
    let envelope = RelayEnvelope {
        item_id: item.item_id.clone(),
        lamport_ts: item.lamport_ts,
        ct_b64,
    };
    let json = match serde_json::to_vec(&envelope) {
        Ok(j) => j,
        Err(e) => {
            tracing::warn!(
                "relay-sync: envelope encode id={} failed: {e}; skipping",
                item.id
            );
            return None;
        }
    };
    Some(base64::engine::general_purpose::STANDARD.encode(json))
}

// ── Push ──────────────────────────────────────────────────────────────────────

/// Push one item's content to the shared inbox. Returns `Ok(true)` on 201,
/// `Ok(false)` on 401 (caller should drop the token + re-register), `Err` on a
/// transient/other failure.
async fn push_item(
    client: &reqwest::Client,
    relay_url: &str,
    inbox_id: &str,
    token: &str,
    content_type: &str,
    content_b64: String,
    wall_time: u64,
) -> Result<bool, RelayError> {
    let url = format!("{relay_url}/devices/{inbox_id}/items");
    let body = PushBody {
        content_type: content_type.to_owned(),
        content_b64,
        wall_time,
    };
    let resp = client
        .post(&url)
        .bearer_auth(token)
        .json(&body)
        .send()
        .await
        .map_err(|e| RelayError::Transport(e.to_string()))?;
    let status = resp.status();
    if status.as_u16() == 201 {
        return Ok(true);
    }
    if status.as_u16() == 401 {
        return Ok(false);
    }
    Err(RelayError::Status(status.as_u16()))
}

/// The push loop: a 3rd subscriber on `new_item_tx` (alongside cloud + sync_orch).
#[allow(clippy::too_many_arguments)]
async fn push_loop(
    client: reqwest::Client,
    relay_url: String,
    device_name: String,
    mut rx: tokio::sync::broadcast::Receiver<ClipboardItem>,
    shutdown: Arc<Notify>,
    sync_key: Arc<Mutex<Option<SyncKey>>>,
    local_key: Arc<zeroize::Zeroizing<[u8; 32]>>,
    last_sync_ms: Arc<AtomicI64>,
) {
    let mut cached_token = load_cached_token();
    let mut warned_no_key = false;

    loop {
        tokio::select! {
            biased;
            _ = shutdown.notified() => {
                tracing::info!("relay-sync push_loop: shutdown");
                break;
            }
            result = rx.recv() => {
                let item = match result {
                    Ok(i) => i,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    // Lagged: we missed some items under a burst. They will be
                    // re-fetched by peers via their own poll; nothing to do.
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("relay-sync push_loop: lagged {n} items");
                        continue;
                    }
                };

                // Snapshot the sync key; skip (one-time warn) if no passphrase set.
                let key_bytes = match snapshot_sync_key(&sync_key).await {
                    Some(b) => {
                        warned_no_key = false;
                        b
                    }
                    None => {
                        if !warned_no_key {
                            tracing::warn!(
                                "relay-sync push_loop: no sync passphrase set — skipping upload"
                            );
                            warned_no_key = true;
                        }
                        continue;
                    }
                };

                let inbox_id = derive_relay_inbox_id(&key_bytes);
                let sk = SyncKey::from_bytes(key_bytes);
                let Some(content_b64) = build_content_b64(&item, &local_key, &sk) else {
                    continue;
                };
                let wall_time = item.wall_time.max(0) as u64;

                // Ensure token, push, and on 401 re-register once.
                if let Err(e) = push_with_reauth(
                    &client,
                    &relay_url,
                    &inbox_id,
                    &key_bytes,
                    &device_name,
                    &item.content_type,
                    content_b64,
                    wall_time,
                    &mut cached_token,
                )
                .await
                {
                    tracing::warn!("relay-sync push_loop: push id={} failed: {e}", item.id);
                } else {
                    let now_ms = now_ms();
                    last_sync_ms.store(now_ms, Ordering::Relaxed);
                }
            }
        }
    }
}

/// Push with one re-auth retry: ensure a token, push; on 401 drop the token,
/// re-register, and push once more.
#[allow(clippy::too_many_arguments)]
async fn push_with_reauth(
    client: &reqwest::Client,
    relay_url: &str,
    inbox_id: &str,
    sync_key_bytes: &[u8; 32],
    device_name: &str,
    content_type: &str,
    content_b64: String,
    wall_time: u64,
    cached_token: &mut Option<String>,
) -> Result<(), RelayError> {
    let token = ensure_token(client, relay_url, sync_key_bytes, device_name, cached_token).await?;
    match push_item(
        client,
        relay_url,
        inbox_id,
        &token,
        content_type,
        content_b64.clone(),
        wall_time,
    )
    .await
    {
        Ok(true) => Ok(()),
        Ok(false) => {
            // 401: token stale. Drop it, re-register, retry once.
            tracing::info!("relay-sync: push got 401; re-registering and retrying once");
            *cached_token = None;
            let token =
                ensure_token(client, relay_url, sync_key_bytes, device_name, cached_token).await?;
            match push_item(
                client,
                relay_url,
                inbox_id,
                &token,
                content_type,
                content_b64,
                wall_time,
            )
            .await
            {
                Ok(true) => Ok(()),
                Ok(false) => Err(RelayError::Status(401)),
                Err(e) => Err(e),
            }
        }
        Err(e) => Err(e),
    }
}

// ── Receive ─────────────────────────────────────────────────────────────────

/// `(wall_time, id)` keyset watermark so pagination is deterministic even within
/// one millisecond and a restart resumes forward (held in memory; the relay
/// inbox itself is the durable store).
#[derive(Clone, Copy, Default)]
struct Watermark {
    wall: u64,
    id: i64,
}

/// Pull one page from the inbox past the watermark. Returns the raw items and
/// whether a 401 was seen (caller re-registers).
async fn pull_page(
    client: &reqwest::Client,
    relay_url: &str,
    inbox_id: &str,
    token: &str,
    wm: Watermark,
) -> Result<Vec<PullItem>, RelayError> {
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

/// Ingest one pulled page into the local DB on a blocking thread (SQLCipher +
/// AEAD). Returns the advanced watermark and how many rows were stored.
///
/// LWW + quota-prune are byte-for-byte the Supabase poll path: dedup on
/// `item_id`, a strictly-newer remote `lamport_ts` replaces in place (preserving
/// the local PK + pin state), an older/equal one is skipped (this is also what
/// makes our OWN pushed rows a no-op when they echo back — self-echo dedup).
fn ingest_page_blocking(
    db: &Database,
    local_key: &zeroize::Zeroizing<[u8; 32]>,
    sync_key_bytes: &[u8; 32],
    page: &[PullItem],
    start: Watermark,
    storage_quota_bytes: u64,
) -> (Watermark, u32) {
    let mut wm = start;
    let mut stored = 0u32;
    let sk = SyncKey::from_bytes(*sync_key_bytes);

    for row in page {
        // Advance the watermark for EVERY readable row (even skipped ones) so the
        // next page does not re-request them.
        if (row.wall_time, row.id) > (wm.wall, wm.id) {
            wm = Watermark {
                wall: row.wall_time,
                id: row.id,
            };
        }

        // Decode the envelope: base64 → JSON → ct_b64 → ciphertext.
        let env_bytes = match base64::engine::general_purpose::STANDARD.decode(&row.content_b64) {
            Ok(b) => b,
            Err(e) => {
                tracing::warn!(
                    "relay-sync: id={} content_b64 decode failed: {e}; skipping",
                    row.id
                );
                continue;
            }
        };
        let env: RelayEnvelope = match serde_json::from_slice(&env_bytes) {
            Ok(e) => e,
            Err(e) => {
                tracing::warn!(
                    "relay-sync: id={} envelope parse failed: {e}; skipping",
                    row.id
                );
                continue;
            }
        };
        let blob = match decode_payload_ct(&env.ct_b64) {
            Ok(b) => b,
            Err(e) => {
                tracing::warn!(
                    "relay-sync: id={} ct_b64 decode failed: {e}; skipping",
                    row.id
                );
                continue;
            }
        };

        // LWW dedup on the cross-device item_id.
        let existing = match get_item_by_item_id(db, &env.item_id) {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("relay-sync: get_item_by_item_id error: {e}; skipping");
                continue;
            }
        };
        let preserved_pk = if let Some(local) = existing.as_ref() {
            if env.lamport_ts <= local.lamport_ts {
                // Local is newer-or-equal — keep it (this is the self-echo no-op
                // for a row we pushed, and the LWW loser for a remote edit).
                continue;
            }
            Some(local.id.clone())
        } else {
            match exists_item_by_item_id(db, &env.item_id) {
                Ok(true) => continue,
                Ok(false) => None,
                Err(e) => {
                    tracing::warn!("relay-sync: exists_item_by_item_id error: {e}; skipping");
                    continue;
                }
            }
        };

        // Decrypt with the sync key (AAD = item_id + cloud schema v5).
        let plaintext = match decrypt_from_cloud(&sk, &env.item_id, &blob) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(
                    "relay-sync: decrypt_from_cloud failed for item_id (wrong passphrase or \
                     tampered blob): {e}; skipping"
                );
                continue;
            }
        };

        let mut local_item = match build_local_item(
            // Use the cross-device item_id as the local PK seed when this is a
            // fresh insert; build_local_item sets `id` from this first arg.
            &env.item_id,
            &env.item_id,
            &row.content_type,
            &plaintext,
            env.lamport_ts,
            row.wall_time as i64,
            None,
            None,
            String::new(),
            local_key,
        ) {
            Ok(i) => i,
            Err(e) => {
                tracing::warn!("relay-sync: build_local_item failed: {e}; skipping");
                continue;
            }
        };

        // LWW replace preserves the prior local row's PK + pin state.
        if let Some(pk) = preserved_pk.as_ref() {
            local_item.id = pk.clone();
        }
        if let Some(local) = existing.as_ref() {
            local_item.pinned = local_item.pinned || local.pinned;
            if local_item.pin_order.is_none() {
                local_item.pin_order = local.pin_order;
            }
        }

        let write_res = if preserved_pk.is_some() {
            replace_cloud_item_by_item_id(db, &local_item)
        } else {
            insert_item(db, &local_item).map_err(anyhow::Error::from)
        };
        match write_res {
            Ok(()) => {
                stored += 1;
                tracing::info!("relay-sync: ingested remote item (id={})", local_item.id);
            }
            Err(e) => tracing::warn!("relay-sync: store failed: {e}"),
        }
    }

    // Byte-cap prune after ingest (long-offline backfill safety) — same policy
    // as the Supabase poll path.
    if stored > 0 {
        let max_bytes = storage_quota_bytes.min(i64::MAX as u64) as i64;
        match prune_to_cap(db, max_bytes) {
            Ok(0) => {}
            Ok(n) => tracing::debug!("relay-sync: byte-pruned {n} rows after ingest"),
            Err(e) => tracing::warn!("relay-sync: prune_to_cap failed: {e}"),
        }
    }

    (wm, stored)
}

/// The receive loop: poll the shared inbox, ingest new items via the LWW path,
/// advance the watermark.
#[allow(clippy::too_many_arguments)]
async fn receive_loop(
    client: reqwest::Client,
    relay_url: String,
    device_name: String,
    shutdown: Arc<Notify>,
    db: Arc<Mutex<Database>>,
    sync_key: Arc<Mutex<Option<SyncKey>>>,
    local_key: Arc<zeroize::Zeroizing<[u8; 32]>>,
    last_sync_ms: Arc<AtomicI64>,
    core_config: Arc<std::sync::RwLock<AppConfig>>,
) {
    let mut cached_token = load_cached_token();
    let mut wm = Watermark::default();
    let mut warned_no_key = false;

    loop {
        // Wait an interval, but wake early on shutdown.
        tokio::select! {
            biased;
            _ = shutdown.notified() => {
                tracing::info!("relay-sync receive_loop: shutdown");
                break;
            }
            _ = tokio::time::sleep(POLL_INTERVAL) => {}
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
        let inbox_id = derive_relay_inbox_id(&key_bytes);

        let token = match ensure_token(
            &client,
            &relay_url,
            &key_bytes,
            &device_name,
            &mut cached_token,
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
                    wm = new_wm;
                    if stored > 0 {
                        last_sync_ms.store(now_ms(), Ordering::Relaxed);
                    }
                }
                Err(e) => {
                    tracing::warn!("relay-sync receive_loop: ingest task panicked: {e}");
                    break;
                }
            }
            if page_len < PULL_LIMIT {
                break;
            }
        }
    }
}

// ── Public entry point ────────────────────────────────────────────────────────

/// Start the relay orchestrator: a push loop (subscribes `new_item_rx`) and a
/// receive loop (polls the shared inbox). Active iff `relay_url` is a valid URL.
///
/// `device_name` is the human-readable name presented at registration (1..=64).
#[allow(clippy::too_many_arguments)]
pub fn start_relay(
    client: reqwest::Client,
    relay_url: String,
    device_name: String,
    db: Arc<Mutex<Database>>,
    new_item_rx: tokio::sync::broadcast::Receiver<ClipboardItem>,
    sync_key: Arc<Mutex<Option<SyncKey>>>,
    local_key: Arc<zeroize::Zeroizing<[u8; 32]>>,
    last_sync_ms: Arc<AtomicI64>,
    core_config: Arc<std::sync::RwLock<AppConfig>>,
) -> Result<RelayHandle, RelayError> {
    let relay_url = relay_url.trim_end_matches('/').to_owned();
    if !is_relay_url_ok(&relay_url) {
        return Err(RelayError::InvalidUrl);
    }
    let shutdown = Arc::new(Notify::new());

    // Truncate the device name to the relay's 1..=64 contract defensively.
    let device_name = {
        let t = device_name.trim();
        let t = if t.is_empty() { "copypaste" } else { t };
        t.chars().take(64).collect::<String>()
    };

    tokio::spawn(push_loop(
        client.clone(),
        relay_url.clone(),
        device_name.clone(),
        new_item_rx,
        shutdown.clone(),
        sync_key.clone(),
        local_key.clone(),
        last_sync_ms.clone(),
    ));
    tokio::spawn(receive_loop(
        client,
        relay_url,
        device_name,
        shutdown.clone(),
        db,
        sync_key,
        local_key,
        last_sync_ms,
        core_config,
    ));

    tracing::info!("relay-sync: orchestrator started");
    Ok(RelayHandle { shutdown })
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use copypaste_core::{derive_sync_key, ITEM_KEY_VERSION_CURRENT};

    fn skey(p: &str) -> [u8; 32] {
        *derive_sync_key(p).expect("derive").as_bytes()
    }

    fn test_client() -> reqwest::Client {
        reqwest::Client::builder()
            .timeout(HTTP_TIMEOUT)
            .build()
            .expect("client")
    }

    /// derive_relay_inbox_id determinism (daemon-side sanity; core also tests it).
    #[test]
    fn inbox_id_is_deterministic() {
        let k = skey("relay-determinism-pass");
        assert_eq!(derive_relay_inbox_id(&k), derive_relay_inbox_id(&k));
    }

    /// register parses a 201 + auth_token. Uses the mockito 0.31 global server
    /// (`mockito::mock` + `mockito::server_url`), so it is `#[serial]`.
    #[tokio::test]
    #[serial_test::serial]
    async fn register_parses_201_auth_token() {
        let k = skey("register-test-pass");
        let inbox = derive_relay_inbox_id(&k);
        let m = mockito::mock("POST", "/devices")
            .with_status(201)
            .with_header("content-type", "application/json")
            .with_body(format!(
                r#"{{"device_id":"{inbox}","auth_token":"deadbeefdeadbeefdeadbeefdeadbeef","expires_at":"2027-01-01T00:00:00Z"}}"#
            ))
            .create();

        let token = register(&test_client(), &mockito::server_url(), &k, "test-device")
            .await
            .expect("register ok");
        assert_eq!(token, "deadbeefdeadbeefdeadbeefdeadbeef");
        m.assert();
    }

    /// push body shape: content_type / content_b64 / wall_time + bearer, 201 → Ok(true).
    #[tokio::test]
    #[serial_test::serial]
    async fn push_item_sends_expected_body() {
        let k = skey("push-body-pass");
        let inbox = derive_relay_inbox_id(&k);
        let path = format!("/devices/{inbox}/items");
        let m = mockito::mock("POST", path.as_str())
            .match_header("authorization", "Bearer tok123")
            .match_body(mockito::Matcher::JsonString(
                r#"{"content_type":"text","content_b64":"Zm9v","wall_time":42}"#.to_owned(),
            ))
            .with_status(201)
            .with_body(r#"{"id":7}"#)
            .create();

        let ok = push_item(
            &test_client(),
            &mockito::server_url(),
            &inbox,
            "tok123",
            "text",
            "Zm9v".to_owned(),
            42,
        )
        .await
        .expect("push ok");
        assert!(ok);
        m.assert();
    }

    /// push 401 → Ok(false) (caller re-registers).
    #[tokio::test]
    #[serial_test::serial]
    async fn push_item_401_signals_reauth() {
        let k = skey("push-401-pass");
        let inbox = derive_relay_inbox_id(&k);
        let path = format!("/devices/{inbox}/items");
        let _m = mockito::mock("POST", path.as_str())
            .with_status(401)
            .create();
        let ok = push_item(
            &test_client(),
            &mockito::server_url(),
            &inbox,
            "stale",
            "text",
            "Zm9v".to_owned(),
            1,
        )
        .await
        .expect("push returns Ok(false) on 401");
        assert!(!ok);
    }

    /// pull_page parses an items array and an empty array; watermark query is
    /// formed correctly (smoke).
    #[tokio::test]
    #[serial_test::serial]
    async fn pull_page_parses_items() {
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
            Watermark::default(),
        )
        .await
        .expect("pull ok");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, 3);
        assert_eq!(items[0].wall_time, 99);
    }

    /// Envelope round-trip: build_content_b64 → decode (base64 → JSON → ct_b64 →
    /// decrypt_from_cloud) recovers the original plaintext, proving the relay
    /// envelope carries the SAME blob shape the Supabase path produces.
    #[test]
    fn envelope_round_trips_through_cloud_crypto() {
        let local_key = zeroize::Zeroizing::new([7u8; 32]);
        let sync_key = SyncKey::from_bytes(skey("envelope-roundtrip-pass"));

        // Build a text item encrypted under the local key (mirrors capture).
        let plaintext = b"hello relay world";
        let item = make_local_text_item("item-rt-1", plaintext, &local_key, 5, 1000);

        let content_b64 =
            build_content_b64(&item, &local_key, &sync_key).expect("build content_b64");

        // Decode the envelope exactly as the receiver does.
        let env_bytes = base64::engine::general_purpose::STANDARD
            .decode(&content_b64)
            .expect("b64 decode envelope");
        let env: RelayEnvelope = serde_json::from_slice(&env_bytes).expect("parse envelope");
        assert_eq!(env.item_id, "item-rt-1");
        assert_eq!(env.lamport_ts, 5);
        let blob = decode_payload_ct(&env.ct_b64).expect("decode ct_b64");
        let recovered = decrypt_from_cloud(&sync_key, &env.item_id, &blob).expect("decrypt");
        assert_eq!(recovered, plaintext);
    }

    /// receive ingests a relay item via insert_item with LWW, and a re-pull of
    /// the SAME item (self-echo / equal lamport) is a no-op. Watermark advances.
    #[test]
    fn ingest_inserts_then_dedups_with_lww() {
        let db = open_mem_db();
        let local_key = zeroize::Zeroizing::new([9u8; 32]);
        let sync_bytes = skey("ingest-lww-pass");
        let sync_key = SyncKey::from_bytes(sync_bytes);

        // Build a wire item by encrypting a text payload through the cloud crypto.
        let plaintext = b"ingest me";
        let item_id = "item-ingest-1";
        let pull = make_pull_item(1, item_id, plaintext, &sync_key, 10, 2000);

        let g = db.blocking_lock();
        let (wm1, stored1) = ingest_page_blocking(
            &g,
            &local_key,
            &sync_bytes,
            std::slice::from_ref(&pull),
            Watermark::default(),
            u64::MAX,
        );
        assert_eq!(stored1, 1, "first ingest inserts the row");
        assert_eq!(wm1.wall, 2000);
        assert_eq!(wm1.id, 1);
        // The row is present and decodes through the production path.
        let got = get_item_by_item_id(&g, item_id)
            .expect("query")
            .expect("row present");
        assert_eq!(got.lamport_ts, 10);

        // Re-pull the same item (equal lamport) → LWW no-op (self-echo dedup).
        let pull2 = make_pull_item(2, item_id, plaintext, &sync_key, 10, 2001);
        let (wm2, stored2) = ingest_page_blocking(
            &g,
            &local_key,
            &sync_bytes,
            std::slice::from_ref(&pull2),
            wm1,
            u64::MAX,
        );
        assert_eq!(stored2, 0, "equal-lamport echo is a no-op");
        // Watermark still advances past the seen row so we don't re-fetch it.
        assert_eq!(wm2.wall, 2001);
        assert_eq!(wm2.id, 2);

        // A strictly-newer lamport for the same item_id wins LWW (replace).
        let pull3 = make_pull_item(3, item_id, b"edited", &sync_key, 11, 2002);
        let (_wm3, stored3) = ingest_page_blocking(
            &g,
            &local_key,
            &sync_bytes,
            std::slice::from_ref(&pull3),
            wm2,
            u64::MAX,
        );
        assert_eq!(stored3, 1, "newer lamport replaces in place");
    }

    // ── test helpers ──────────────────────────────────────────────────────────

    fn open_mem_db() -> Arc<Mutex<Database>> {
        let db = Database::open_in_memory().expect("open in-memory db");
        Arc::new(Mutex::new(db))
    }

    /// Build a locally-stored text ClipboardItem (v2 key path) so the upload
    /// pipeline's `decrypt_item_plaintext` can read it back.
    fn make_local_text_item(
        item_id: &str,
        plaintext: &[u8],
        local_key: &zeroize::Zeroizing<[u8; 32]>,
        lamport_ts: i64,
        wall_time: i64,
    ) -> ClipboardItem {
        use copypaste_core::{
            build_item_aad_v2, derive_v2, encrypt_item_with_aad, AAD_SCHEMA_VERSION_V4,
        };
        let v1: [u8; 32] = **local_key;
        let v2 = derive_v2(&v1);
        let aad = build_item_aad_v2(
            item_id,
            AAD_SCHEMA_VERSION_V4,
            ITEM_KEY_VERSION_CURRENT as u32,
        );
        let (nonce, ct) = encrypt_item_with_aad(plaintext, &v2, &aad).expect("encrypt");
        ClipboardItem {
            deleted: false,
            id: item_id.to_owned(),
            item_id: item_id.to_owned(),
            content_type: "text".to_owned(),
            content: Some(ct),
            content_nonce: Some(nonce.to_vec()),
            blob_ref: None,
            is_sensitive: false,
            is_synced: false,
            lamport_ts,
            wall_time,
            expires_at: None,
            app_bundle_id: None,
            content_hash: None,
            origin_device_id: "dev-local".to_owned(),
            key_version: ITEM_KEY_VERSION_CURRENT as u8,
            pinned: false,
            pin_order: None,
            thumb: None,
        }
    }

    /// Build a relay `PullItem` carrying a text payload encrypted for the cloud.
    fn make_pull_item(
        id: i64,
        item_id: &str,
        plaintext: &[u8],
        sync_key: &SyncKey,
        lamport_ts: i64,
        wall_time: u64,
    ) -> PullItem {
        let blob = encrypt_for_cloud(sync_key, item_id, plaintext).expect("cloud encrypt");
        let ct_b64 = base64::engine::general_purpose::STANDARD.encode(&blob);
        let env = RelayEnvelope {
            item_id: item_id.to_owned(),
            lamport_ts,
            ct_b64,
        };
        let content_b64 = base64::engine::general_purpose::STANDARD
            .encode(serde_json::to_vec(&env).expect("env json"));
        PullItem {
            id,
            content_type: "text".to_owned(),
            content_b64,
            wall_time,
        }
    }
}
