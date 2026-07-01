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
/// relay can be exercised.
///
/// Delegates to `crate::url_guard` — the single authoritative HTTPS gate shared
/// by both cloud-sync and relay-sync (g06m.32 #2).  Any security fix to the
/// gate logic now propagates to both paths automatically.
pub(super) fn is_relay_url_ok(s: &str) -> bool {
    crate::url_guard::is_https_url(s) || crate::url_guard::allows_loopback_http_in_tests(s)
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
    // CopyPaste-crh3.79: store_cached_token does write_token_0600 -> fsync
    // (sync_all), which can park a tokio worker for 50-200ms on APFS/NFS. Run it
    // on the blocking pool so this async path is not stalled.
    {
        let token_for_cache = token.clone();
        let lk = local_key.clone();
        let did = device_id.to_string();
        let _ =
            tokio::task::spawn_blocking(move || store_cached_token(&token_for_cache, &lk, &did))
                .await;
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relay::testutil::{skey, test_client};

    /// register parses a 201 + auth_token. Uses the mockito 0.31 global server
    /// (`mockito::mock` + `mockito::server_url`), so it is `#[serial]`.
    #[tokio::test]
    #[serial_test::serial]
    async fn register_parses_201_auth_token() {
        use copypaste_core::derive_relay_inbox_id;
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
}
