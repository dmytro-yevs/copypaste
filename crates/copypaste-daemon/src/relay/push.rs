//! Relay push path: encrypt-and-upload one item, push loop, re-auth retry.

use std::sync::{
    atomic::{AtomicI64, Ordering},
    Arc,
};

use base64::Engine as _;
use copypaste_core::{derive_relay_inbox_id, encrypt_for_cloud, AppConfig, ClipboardItem, SyncKey};
use tokio::sync::{Mutex, Notify};

use crate::sync_common::{decrypt_item_plaintext, wrap_and_check_cloud_upload_plaintext};
use crate::sync_in_flight::SyncInFlightGuard;

use super::pasteboard::relay_should_skip_wifi;
use super::registration::{ensure_token, load_initial_token, snapshot_sync_key};
use super::types::{PushBody, RelayEnvelope, RelayError};

// ── Envelope build ────────────────────────────────────────────────────────────

/// Build the relay `content_b64` for one item: encrypt the wrapped plaintext for
/// the cloud (sync key + item_id AAD), wrap it in the JSON envelope, base64 it.
///
/// Returns `Ok(None)` when the item should be skipped (e.g. oversized, decrypt
/// failure) — never logs plaintext.
pub(super) fn build_content_b64(
    item: &ClipboardItem,
    local_key: &zeroize::Zeroizing<[u8; 32]>,
    sync_key: &SyncKey,
) -> Option<String> {
    // CopyPaste-cm0u: a tombstone has content = NULL — there is nothing to
    // decrypt. Emit a delete envelope (empty ct_b64, deleted=true) instead of
    // calling decrypt_item_plaintext on NULL (which Err'd and dropped the
    // delete, so deletes never propagated over relay-only topologies).
    let ct_b64 = if item.deleted {
        String::new()
    } else {
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
        base64::engine::general_purpose::STANDARD.encode(&blob)
    };
    let envelope = RelayEnvelope {
        item_id: item.item_id.clone(),
        lamport_ts: item.lamport_ts,
        ct_b64,
        // CopyPaste-cm0u: carry delete + pin state so they propagate over relay.
        deleted: item.deleted,
        pinned: item.pinned,
        pin_order: item.pin_order,
        // CopyPaste-ayvs: carry the LWW tie-break keys.
        wall_time: item.wall_time,
        origin_device_id: item.origin_device_id.clone(),
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
pub(super) async fn push_item(
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
// relay_url, device_name, device_id, sync_key, local_key, last_sync_ms, and
// shutdown are independent state slices — no natural grouping into a struct
// without adding indirection for a private-only function.
#[allow(clippy::too_many_arguments)]
pub(super) async fn push_loop(
    client: reqwest::Client,
    relay_url: String,
    device_name: String,
    device_id: String,
    mut rx: tokio::sync::broadcast::Receiver<ClipboardItem>,
    shutdown: Arc<Notify>,
    sync_key: Arc<Mutex<Option<SyncKey>>>,
    local_key: Arc<zeroize::Zeroizing<[u8; 32]>>,
    last_sync_ms: Arc<AtomicI64>,
    core_config: Arc<std::sync::RwLock<AppConfig>>,
    // CopyPaste-1jms.22: shared in-flight flag for SyncBadgeState::Syncing.
    sync_in_flight: std::sync::Arc<std::sync::atomic::AtomicBool>,
) {
    let mut cached_token = load_initial_token(&local_key, &device_id);
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

                // P1-1: honour the "sensitive items are NEVER uploaded" guarantee
                // (docs/relay-api.md:105). Drop the item before any crypto work so
                // ciphertext never enters the relay inbox.
                if item.is_sensitive {
                    tracing::debug!(
                        "relay-sync push_loop: skipping sensitive id={} (never uploaded)",
                        item.id
                    );
                    continue;
                }

                // tke7 (PG-30): hot-reload master sync gate.  When sync_enabled is
                // toggled off at runtime, drop outbound items immediately so no data
                // is uploaded.  The item is not re-queued — the user explicitly
                // disabled sync.
                let sync_enabled = core_config
                    .read()
                    .map(|g| g.sync_enabled)
                    .unwrap_or(true);
                if !sync_enabled {
                    tracing::debug!(
                        "relay-sync push_loop: sync_enabled=false; dropping outbound id={}",
                        item.id
                    );
                    continue;
                }

                // A-SET-2 hot-reload: read sync_on_wifi_only from the live config on
                // every incoming item so a runtime set_config change takes effect
                // immediately.  When the guard fires we skip this item; it will be
                // re-broadcast (or recovered via receive_loop) once Wi-Fi is available.
                let sync_on_wifi_only = core_config
                    .read()
                    .map(|g| g.sync_on_wifi_only)
                    .unwrap_or(false);
                if sync_on_wifi_only {
                    let on_wifi = tokio::task::spawn_blocking(crate::platform::is_on_wifi)
                        .await
                        .unwrap_or(true); // fail-open: if check errors, assume Wi-Fi
                    if relay_should_skip_wifi(sync_on_wifi_only, on_wifi) {
                        tracing::debug!(
                            "relay-sync push_loop: sync_on_wifi_only=true and not on Wi-Fi; \
                             skipping push for id={}",
                            item.id
                        );
                        continue;
                    }
                }

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
                // CopyPaste-z1xt: `build_content_b64` decrypts the local
                // ciphertext + re-encrypts for the relay (CPU-bound, possibly
                // multi-MB) — run it on the blocking thread pool instead of inline
                // on the async executor. Move `item` into the closure (no clone of
                // the heavy blob) and get it back so the rest of the loop can use
                // it. `SyncKey` is reconstructed inside from the Send `[u8; 32]`.
                let lk = local_key.clone();
                let (item, content_b64) = match tokio::task::spawn_blocking(move || {
                    let sk = SyncKey::from_bytes(key_bytes);
                    let out = build_content_b64(&item, &lk, &sk);
                    (item, out)
                })
                .await
                {
                    Ok(pair) => pair,
                    Err(e) => {
                        tracing::warn!("relay-sync push_loop: build task failed: {e}; skipping");
                        continue;
                    }
                };
                let Some(content_b64) = content_b64 else {
                    continue;
                };
                let wall_time = item.wall_time.max(0) as u64;

                // CopyPaste-1jms.22: arm in-flight guard for this relay push
                // round-trip. Resets on drop (error or success).
                let _relay_push_guard =
                    SyncInFlightGuard::new(std::sync::Arc::clone(&sync_in_flight));
                // Ensure token, push, and on 401 re-register once.
                if let Err(e) = push_with_reauth(
                    &client,
                    &relay_url,
                    &inbox_id,
                    &key_bytes,
                    &device_name,
                    &device_id,
                    &item.content_type,
                    content_b64,
                    wall_time,
                    &mut cached_token,
                    &local_key,
                )
                .await
                {
                    tracing::warn!("relay-sync push_loop: push id={} failed: {e}", item.id);
                } else {
                    let now_ms = super::now_ms();
                    last_sync_ms.store(now_ms, Ordering::Relaxed);
                }
            }
        }
    }
}

/// Push with one re-auth retry: ensure a token, push; on 401 drop the token,
/// re-register, and push once more.
// The relay protocol binds all of: client, url, inbox_id, sync_key_bytes,
// device_name, device_id, local_key, and last_sync_ms. No natural grouping
// without a new intermediate struct; count is justified by the protocol surface.
#[allow(clippy::too_many_arguments)]
pub(super) async fn push_with_reauth(
    client: &reqwest::Client,
    relay_url: &str,
    inbox_id: &str,
    sync_key_bytes: &[u8; 32],
    device_name: &str,
    device_id: &str,
    content_type: &str,
    content_b64: String,
    wall_time: u64,
    cached_token: &mut Option<String>,
    local_key: &zeroize::Zeroizing<[u8; 32]>,
) -> Result<(), RelayError> {
    let token = ensure_token(
        client,
        relay_url,
        sync_key_bytes,
        device_name,
        cached_token,
        local_key,
        device_id,
    )
    .await?;
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
            let token = ensure_token(
                client,
                relay_url,
                sync_key_bytes,
                device_name,
                cached_token,
                local_key,
                device_id,
            )
            .await?;
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
