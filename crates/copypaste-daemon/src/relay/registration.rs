//! Device registration with the relay: PoP-authenticated register, ensure-token,
//! URL validation, and sync-key snapshot helper.

use std::sync::Arc;

use base64::Engine as _;
use copypaste_core::{
    derive_relay_inbox_id, derive_relay_public_key, derive_relay_registration_pop, SyncKey,
};
use tokio::sync::Mutex;

use super::token::{load_cached_token, store_cached_token};
use super::types::{RegisterBody, RegisterResp, RelayError};

// ── URL validation ────────────────────────────────────────────────────────────

/// Accept `https://...`; in tests also accept loopback `http://` so the mock
/// relay can be exercised. Mirrors `cloud::is_https_url`'s posture.
pub(super) fn is_relay_url_ok(s: &str) -> bool {
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
pub(super) async fn snapshot_sync_key(sync_key: &Arc<Mutex<Option<SyncKey>>>) -> Option<[u8; 32]> {
    let guard = sync_key.lock().await;
    guard.as_ref().map(|k| *k.as_bytes())
}

// ── Register ────────────────────────────────────────────────────────────────

/// Register (or co-register) this device's shared-account inbox with the relay
/// and return a fresh auth token. The inbox id + public key are derived from
/// `sync_key_bytes` (SECRET-derived — never logged).
pub(super) async fn register(
    client: &reqwest::Client,
    relay_url: &str,
    sync_key_bytes: &[u8; 32],
    device_name: &str,
) -> Result<String, RelayError> {
    let inbox_id = derive_relay_inbox_id(sync_key_bytes);
    let pubkey = derive_relay_public_key(sync_key_bytes);
    let public_key_b64 = base64::engine::general_purpose::STANDARD.encode(pubkey);

    // Proof-of-possession: HMAC-SHA256(sync_key, prefix || inbox_id).
    // Proves the registrant holds the sync key corresponding to the derived inbox id.
    // Fixes CopyPaste-n2l: the relay now rejects registrations without a valid PoP.
    let pop = derive_relay_registration_pop(sync_key_bytes, &inbox_id);
    let pop_b64 = base64::engine::general_purpose::STANDARD.encode(pop);

    let body = RegisterBody {
        device_id: inbox_id,
        device_name: device_name.to_owned(),
        public_key_b64,
        pop_b64,
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
///
/// `device_id` is the daemon's own stable device UUID, bound into the token
/// file's AEAD AAD via [`store_cached_token`].
pub(super) async fn ensure_token(
    client: &reqwest::Client,
    relay_url: &str,
    sync_key_bytes: &[u8; 32],
    device_name: &str,
    cached: &mut Option<String>,
    local_key: &zeroize::Zeroizing<[u8; 32]>,
    device_id: &str,
) -> Result<String, RelayError> {
    if let Some(t) = cached.as_ref() {
        return Ok(t.clone());
    }
    let token = register(client, relay_url, sync_key_bytes, device_name).await?;
    store_cached_token(&token, local_key, device_id);
    *cached = Some(token.clone());
    Ok(token)
}

/// Load the initial cached token at startup (thin wrapper for the push/receive
/// loops so they don't import from `token` directly).
///
/// `device_id` is the daemon's own stable device UUID — must match the id that
/// was used when the token was stored (see [`store_cached_token`]).
pub(super) fn load_initial_token(
    local_key: &zeroize::Zeroizing<[u8; 32]>,
    device_id: &str,
) -> Option<String> {
    load_cached_token(local_key, device_id)
}
