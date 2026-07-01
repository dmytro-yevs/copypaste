use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

use copypaste_core::ClipboardItem;
use copypaste_supabase::auth::AuthClient;

use super::super::auth::refresh_bearer;
use super::super::config::CloudConfig;
use super::super::ingest::clipboard_item_to_json;
use super::{PUSH_INITIAL_BACKOFF, PUSH_MAX_BACKOFF};

/// Outcome of a single push attempt.
#[derive(Debug)]
enum PushOutcome {
    /// 2xx — accepted by the server.
    Ok,
    /// 401 — bearer expired or invalid. Caller should refresh and retry once.
    Unauthorized,
    /// 429 — rate-limited. The `Option<Duration>` carries the `Retry-After`
    /// value if the server provided one (in seconds form).
    RateLimited(Option<Duration>),
    /// Network or 5xx error. Transient; caller should back off and requeue.
    Transient(String),
    /// 4xx other than 401/429 — request is malformed or rejected for a reason
    /// retrying will not fix. Caller should give up on this item.
    Permanent(String),
}

/// One push attempt, surfacing structured outcomes so the caller can decide
/// between refresh, backoff, and abort.
///
/// `payload_ct_b64` is the base64-encoded cloud ciphertext (nonce||ciphertext)
/// produced by `encrypt_for_cloud`. It is pre-computed by the push loop so
/// re-encryption only happens once even when the attempt is retried. For
/// tombstone rows (`item.deleted == true`) this is `None` — the server stores
/// `payload_ct = NULL` and receiving devices apply a soft-delete.
async fn push_item_once(
    client: &reqwest::Client,
    url: &str,
    anon_key: &str,
    bearer: &str,
    item: &ClipboardItem,
    // `None` for tombstone rows (item.deleted == true); `Some(b64)` for live items.
    payload_ct_b64: Option<&str>,
) -> PushOutcome {
    let body = clipboard_item_to_json(item, payload_ct_b64);

    let resp = match client
        .post(url)
        .header("apikey", anon_key)
        .header("Authorization", format!("Bearer {bearer}"))
        .header("Content-Type", "application/json")
        .header("Prefer", "return=minimal")
        .json(&body)
        .send()
        .await
    {
        Ok(r) => r,
        // Network / DNS / TLS / connection-refused → transient.
        Err(e) => return PushOutcome::Transient(format!("send: {e}")),
    };

    let status = resp.status();
    if status.is_success() {
        return PushOutcome::Ok;
    }
    if status.as_u16() == 401 {
        return PushOutcome::Unauthorized;
    }
    if status.as_u16() == 429 {
        let retry_after = parse_retry_after_secs(resp.headers());
        return PushOutcome::RateLimited(retry_after);
    }
    let text = resp.text().await.unwrap_or_default();
    if status.is_server_error() {
        return PushOutcome::Transient(format!("{status}: {text}"));
    }
    PushOutcome::Permanent(format!("{status}: {text}"))
}

/// Parse the HTTP `Retry-After` header in its delta-seconds form. We
/// deliberately do NOT support the HTTP-date variant — Supabase emits the
/// integer-seconds form and supporting both pulls in a date-parsing dep for
/// no operator benefit.
pub(crate) fn parse_retry_after_secs(headers: &reqwest::header::HeaderMap) -> Option<Duration> {
    headers
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.trim().parse::<u64>().ok())
        .map(Duration::from_secs)
}

/// Compose the per-item push pipeline:
/// - try once;
/// - on `Unauthorized` → refresh the shared bearer (Wave 2.7 #20) and retry
///   exactly once;
/// - on `RateLimited(Some(d))` → honour `Retry-After` and retry once
///   (Wave 2.7 #21);
/// - on `Transient` → exponential backoff between attempts, capped at
///   `PUSH_MAX_BACKOFF`;
/// - on `Permanent` → abort and surface the error.
///
/// Returns `Ok(())` on 2xx, `Err(msg)` for permanent failures or after the
/// transient-retry budget is exhausted. Callers (the push loop) then decide
/// whether to requeue.
///
/// `cloud_signed_in` is the shared auth-state flag (BUG 2). When the 401 path
/// refreshes the bearer, a successful refresh keeps it `true` and a failed
/// refresh flips it `false`. `None` is accepted for callers/tests that do not
/// track auth state.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn push_item_with_retries(
    client: &reqwest::Client,
    url: &str,
    config: &CloudConfig,
    bearer: &Arc<RwLock<String>>,
    item: &ClipboardItem,
    // `None` for tombstone rows (item.deleted == true); `Some(b64)` for live items.
    payload_ct_b64: Option<&str>,
    cloud_signed_in: Option<&Arc<std::sync::atomic::AtomicBool>>,
    auth: &AuthClient,
) -> Result<(), String> {
    // A throwaway flag for the `None` case so `refresh_bearer` always has a
    // target to write — its write is then simply ignored by the caller.
    let scratch_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let signed_in = cloud_signed_in.unwrap_or(&scratch_flag);
    let mut backoff = PUSH_INITIAL_BACKOFF;
    // Hard cap on attempts to avoid hot loops even if every attempt comes back
    // as `Transient(_)`. The loop body sleeps between attempts so the worst-case
    // duration is bounded by the sum of backoffs.
    let max_transient_attempts: u8 = 4;
    let mut transient_attempts: u8 = 0;
    // `Unauthorized` may only trigger ONE refresh-and-retry per item to
    // avoid an infinite loop if the refresh itself returns a still-401 token.
    let mut refreshed_once = false;
    // Same single-shot guard for `Retry-After` so a misconfigured server
    // returning permanent 429 cannot pin us forever.
    let mut honoured_retry_after_once = false;

    loop {
        let token = bearer.read().await.clone();
        match push_item_once(client, url, &config.anon_key, &token, item, payload_ct_b64).await {
            PushOutcome::Ok => return Ok(()),

            PushOutcome::Unauthorized if !refreshed_once => {
                refreshed_once = true;
                tracing::info!("cloud-sync got 401; refreshing bearer and retrying once");
                match refresh_bearer(config, signed_in, auth).await {
                    Ok(new_token) => {
                        *bearer.write().await = new_token;
                    }
                    Err(e) => {
                        return Err(format!("401 refresh failed: {e}"));
                    }
                }
                // Loop again with the refreshed token.
                continue;
            }
            PushOutcome::Unauthorized => {
                return Err("401 Unauthorized (already refreshed once)".into());
            }

            PushOutcome::RateLimited(retry_after) if !honoured_retry_after_once => {
                honoured_retry_after_once = true;
                let delay = retry_after.unwrap_or(backoff).min(PUSH_MAX_BACKOFF);
                tracing::warn!(
                    "cloud-sync got 429; sleeping {:?} before retry (Retry-After: {:?})",
                    delay,
                    retry_after,
                );
                tokio::time::sleep(delay).await;
                continue;
            }
            PushOutcome::RateLimited(_) => {
                return Err("429 Too Many Requests (already retried after Retry-After)".into());
            }

            PushOutcome::Transient(msg) => {
                transient_attempts += 1;
                if transient_attempts >= max_transient_attempts {
                    return Err(format!(
                        "transient failure budget exhausted after {transient_attempts} attempts: {msg}"
                    ));
                }
                tracing::warn!(
                    "cloud-sync transient failure ({msg}); backing off {:?} (attempt {}/{})",
                    backoff,
                    transient_attempts,
                    max_transient_attempts,
                );
                tokio::time::sleep(backoff).await;
                backoff = (backoff * 2).min(PUSH_MAX_BACKOFF);
                continue;
            }

            PushOutcome::Permanent(msg) => return Err(msg),
        }
    }
}
