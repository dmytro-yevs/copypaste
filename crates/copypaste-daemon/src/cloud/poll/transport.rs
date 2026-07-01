use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

use copypaste_supabase::auth::AuthClient;

use super::super::auth::refresh_bearer;
use super::super::config::CloudConfig;
use super::super::push::{parse_retry_after_secs, PUSH_INITIAL_BACKOFF, PUSH_MAX_BACKOFF};

/// Outcome of a single `fetch_remote_rows` attempt.
///
/// Mirrors the push-side `PushOutcome`: the poll path needs to distinguish
/// "bearer expired" (refresh-and-retry), "rate-limited" (sleep Retry-After),
/// and every other failure (log + wait for the next tick).
pub(crate) enum FetchOutcome {
    /// 2xx — rows decoded successfully.
    Ok(Vec<serde_json::Value>),
    /// 401 — bearer expired or invalid. Caller should refresh and retry once.
    Unauthorized,
    /// 429 — rate-limited. `Option<Duration>` carries the `Retry-After` value
    /// (seconds form) when the server provided one.  Caller should sleep that
    /// duration (or a bounded backoff) before retrying rather than waiting the
    /// full poll interval, which would ignore the server's guidance.
    /// [P1 audit fix: poll 429 Retry-After handling]
    RateLimited(Option<Duration>),
    /// Any other failure (network, 5xx, non-401/429 4xx, JSON decode). The
    /// message is for logging only; retrying immediately will not help, so the
    /// caller just waits for the next poll tick.
    Failed(String),
}

/// `GET /rest/v1/clipboard_items` and return the raw JSON rows.
///
/// The caller is responsible for extracting and decrypting `payload_ct`.
///
/// A 401 is surfaced as [`FetchOutcome::Unauthorized`] (not folded into the
/// generic error) so the poll loop can refresh the bearer and retry — without
/// this, an expired GoTrue token permanently stalls *downloads* even though
/// uploads keep working (the push path already refreshes on 401).
pub(crate) async fn fetch_remote_rows(
    client: &reqwest::Client,
    url: &str,
    anon_key: &str,
    bearer: &str,
) -> FetchOutcome {
    let resp = match client
        .get(url)
        .header("apikey", anon_key)
        .header("Authorization", format!("Bearer {bearer}"))
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => return FetchOutcome::Failed(format!("send: {e}")),
    };

    let status = resp.status();
    if status.as_u16() == 401 {
        return FetchOutcome::Unauthorized;
    }
    // [P1 audit fix] Surface 429 as a distinct outcome so the caller can sleep
    // the Retry-After duration instead of folding it into a generic Failed and
    // waiting the full poll interval, which ignores the server's guidance.
    if status.as_u16() == 429 {
        let retry_after = parse_retry_after_secs(resp.headers());
        return FetchOutcome::RateLimited(retry_after);
    }
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        return FetchOutcome::Failed(format!("REST GET failed ({status}): {text}"));
    }

    match resp.json::<Vec<serde_json::Value>>().await {
        Ok(rows) => FetchOutcome::Ok(rows),
        Err(e) => FetchOutcome::Failed(format!("decode rows: {e}")),
    }
}

/// Fetch rows, transparently refreshing the shared bearer on a single 401.
///
/// This is the poll-side counterpart of the `Unauthorized` arm in
/// `push_item_with_retries`: the `refreshed` single-shot guard guarantees we
/// refresh-and-retry at most once per call, so a refresh that itself yields a
/// still-401 token cannot spin into an infinite loop — the second 401 falls
/// through to `FetchOutcome::Unauthorized` and is reported as an error.
pub(crate) async fn fetch_remote_rows_with_refresh(
    client: &reqwest::Client,
    url: &str,
    config: &CloudConfig,
    bearer: &Arc<RwLock<String>>,
    cloud_signed_in: &Arc<std::sync::atomic::AtomicBool>,
    auth: &AuthClient,
) -> Result<Vec<serde_json::Value>, String> {
    let mut refreshed = false;
    // Single-shot guard: honour Retry-After at most once per call so a
    // misbehaving server returning permanent 429 cannot pin this loop.
    let mut honoured_rate_limit_once = false;
    loop {
        let token = bearer.read().await.clone();
        match fetch_remote_rows(client, url, &config.anon_key, &token).await {
            FetchOutcome::Ok(rows) => return Ok(rows),
            FetchOutcome::Unauthorized if !refreshed => {
                refreshed = true;
                tracing::info!("cloud-sync poll got 401; refreshing bearer and retrying once");
                match refresh_bearer(config, cloud_signed_in, auth).await {
                    Ok(new_token) => {
                        *bearer.write().await = new_token;
                    }
                    Err(e) => return Err(format!("401 refresh failed: {e}")),
                }
                // Loop again with the refreshed token.
                continue;
            }
            FetchOutcome::Unauthorized => {
                return Err("401 Unauthorized (already refreshed once)".into());
            }
            // [P1 audit fix] Sleep Retry-After (or a bounded backoff) before
            // retrying rather than folding 429 into Failed and waiting the full
            // poll interval, which ignores the server's rate-limit guidance.
            FetchOutcome::RateLimited(retry_after) if !honoured_rate_limit_once => {
                honoured_rate_limit_once = true;
                let delay = retry_after
                    .unwrap_or(PUSH_INITIAL_BACKOFF)
                    .min(PUSH_MAX_BACKOFF);
                tracing::warn!(
                    "cloud-sync poll got 429; sleeping {:?} before retry (Retry-After: {:?})",
                    delay,
                    retry_after,
                );
                tokio::time::sleep(delay).await;
                continue;
            }
            FetchOutcome::RateLimited(_) => {
                return Err("429 Too Many Requests (already retried after Retry-After)".into());
            }
            FetchOutcome::Failed(msg) => return Err(msg),
        }
    }
}
