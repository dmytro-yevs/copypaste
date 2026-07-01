//! Sync status / connection-test IPC verbs + shared account-id guard (split
//! from handlers_sync.rs, ADR-017 daemon-ipc track, CopyPaste-vp63.18).
use super::*;

impl IpcServer {
    #[cfg(feature = "cloud-sync")]
    pub(crate) async fn handle_get_sync_status(&self, req: Request) -> Response {
        let passphrase_set = self.sync_key.lock().await.is_some();
        // Fix HIGH #3: read_config() does blocking fs I/O; move it to
        // the blocking thread pool so the async worker is not stalled.
        let app_cfg = match tokio::task::spawn_blocking(read_config).await {
            Ok(cfg) => cfg,
            Err(e) => {
                return Response::err_with_code(
                    req.id,
                    ERR_CODE_INTERNAL_ERROR,
                    format!("get_sync_status blocking task failed: {e}"),
                )
            }
        };
        let supabase_configured = app_cfg.supabase_url.is_some()
            && app_cfg.supabase_anon_key.is_some()
            || std::env::var("SUPABASE_URL").is_ok();
        // BUG 2 fix: report the REAL GoTrue auth state published by the
        // cloud loops, not the old `signed_in = supabase_configured`
        // placeholder. The flag is set `true` once `start_cloud` resolves
        // a bearer and `false` on a bearer-resolution / 401-refresh
        // failure, so the UI no longer claims "signed in" after a
        // `CloudError::AuthFailed` aborted cloud sync.
        let signed_in = self
            .cloud_signed_in
            .load(std::sync::atomic::Ordering::Relaxed);
        let raw_ts = self.last_sync_ms.load(std::sync::atomic::Ordering::Relaxed);
        let last_sync_ms_val: Option<i64> = if raw_ts > 0 { Some(raw_ts) } else { None };
        // B. Expose the non-secret Supabase URL and email so the UI can
        // show/prefill them. We do NOT expose the anon key, password, or
        // passphrase. Priority: env vars override AppConfig (same as
        // CloudConfig::from_env).
        let supabase_url_val: Option<String> = std::env::var("SUPABASE_URL")
            .ok()
            .or_else(|| app_cfg.supabase_url.clone());
        // M3 FIX: mask the email before sending over IPC so arbitrary
        // same-UID processes cannot harvest the full GoTrue address.
        // `a***@example.com` preserves the account-indicator the UI
        // needs (SettingsView shows "Signed in as …") without leaking
        // the full address. Mirrors `cloud::redact_email` — inlined
        // here because that helper is private to the cloud module.
        let email_val: Option<String> = std::env::var("SUPABASE_EMAIL")
            .ok()
            .or_else(|| app_cfg.supabase_email.clone())
            .map(|e| {
                // Show first char + *** + @domain; non-address input →
                // "<redacted>" (same contract as cloud::redact_email).
                match e.split_once('@') {
                    Some((local, domain)) if !local.is_empty() && !domain.is_empty() => {
                        let first = local.chars().next().unwrap_or('*');
                        if local.chars().count() <= 1 {
                            format!("*@{domain}")
                        } else {
                            format!("{first}***@{domain}")
                        }
                    }
                    _ => "<redacted>".to_string(),
                }
            });
        // CopyPaste-merc / CopyPaste-1jms.22: compute badge state once
        // here in the daemon so every consumer (macOS UI, Android)
        // renders the SAME canonical value. Use the `_with_inflight`
        // variant so `Syncing` (green pulse) is emitted while a sync
        // round-trip is actively in progress.
        // `supabase_url_val` is Some(url) when either the env var or
        // the config has a URL — use it as the "url_set" signal.
        let supabase_url_set = supabase_url_val.is_some();
        // Relaxed ordering: a brief window where the badge says "idle"
        // while a round-trip just started (or vice versa) is acceptable.
        // The badge is informational and is refreshed on every IPC poll.
        let in_flight = self
            .sync_in_flight
            .load(std::sync::atomic::Ordering::Relaxed);
        let badge_state = compute_sync_badge_state_with_inflight(
            passphrase_set,
            supabase_url_set,
            supabase_configured,
            signed_in,
            last_sync_ms_val,
            None, // use SystemTime::now() inside the helper
            in_flight,
        );
        let badge_state_json =
            serde_json::to_value(&badge_state).unwrap_or(serde_json::Value::Null);
        // CopyPaste-1jms.34: read the canonical account identity set by
        // `with_cloud_account_id` after `start_cloud` returned. A `None`
        // means cloud-sync is not active or the daemon is anon-key-only.
        // The Mutex hold is tiny (clone the String and drop immediately).
        let account_id_val: Option<String> = self
            .cloud_account_id
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        Response::ok(
            req.id,
            serde_json::json!({
                "passphrase_set": passphrase_set,
                "supabase_configured": supabase_configured,
                "signed_in": signed_in,
                "last_sync_ms": last_sync_ms_val,
                "supabase_url": supabase_url_val,
                "email": email_val,
                // Single source of truth for the badge colour on all platforms.
                // Optional for backward-compat: consumers that receive this field
                // MUST use it; consumers talking to older daemons may not see it
                // and may fall back to local derivation from the raw fields above.
                "badge_state": badge_state_json,
                // Non-secret stable account identity (CopyPaste-1jms.34).
                // Omitted from the wire when None (cloud off / anon-key-only).
                "supabase_account_id": account_id_val,
            }),
        )
    }

    // `cloud_test_connection` validates the configured Supabase
    // credentials end-to-end so the UI/CLI can give a precise, actionable
    // diagnostic instead of leaving the user to guess why sync is silent.
    // It performs a single cheap `GET /rest/v1/clipboard_items?limit=0`
    // with the anon key (+ optional email/password bearer) and classifies
    // the outcome (URL reachable? key valid? table present? RLS ok?).
    #[cfg(feature = "cloud-sync")]
    pub(crate) async fn handle_cloud_test_connection(&self, req: Request) -> Response {
        let result = test_cloud_connection().await;
        Response::ok(req.id, result)
    }

    /// Resolve the Supabase account id required by the single per-account sync-key
    /// derivation, or an `Err(message)` the caller surfaces as a clean
    /// `auth_failed` response.
    ///
    /// Invariant: the per-account salt that defeats cross-user precompute has no
    /// meaning without a stable account id, so every passphrase-entry / rotation
    /// path that DERIVES a key (`set_sync_passphrase`, `rotate_sync_key`,
    /// `revoke_and_rotate`) must hold one. Cloud sync only runs when signed in, so
    /// the account id (`copypaste_supabase::supabase_account_id`, set by
    /// `start_cloud` after a bearer resolves) is present in the normal flow; this
    /// returns a clear error instead of silently falling back to an account-free
    /// key that no other device of the account could reproduce.
    ///
    /// (Pairing-provisioned devices receive the raw key bytes directly and never
    /// call this — only the device that ENTERS the passphrase derives a key.)
    #[cfg(any(feature = "cloud-sync", feature = "relay-sync"))]
    pub(crate) fn require_cloud_account_id(&self) -> Result<String, String> {
        let account_id = self
            .cloud_account_id
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        match account_id {
            Some(id) if !id.is_empty() => Ok(id),
            _ => Err(
                "cloud sync requires sign-in: sign into your Supabase account before \
                      setting or rotating the sync passphrase (no account id is available for \
                      the per-account key derivation)"
                    .to_string(),
            ),
        }
    }
}

/// Probe the configured Supabase project and return a structured diagnostic.
///
/// This is what backs the `cloud_test_connection` IPC method (and `copypaste
/// cloud test`). It performs at most one authenticated round-trip:
/// `GET /rest/v1/clipboard_items?limit=0` with the anon key in `apikey` and an
/// `Authorization: Bearer` header (email/password token when configured, anon
/// key otherwise). The HTTP outcome is mapped to an actionable message so the
/// user learns *which* step is wrong (credentials missing, URL unreachable,
/// key invalid, table not provisioned, RLS misconfigured) rather than seeing
/// silent no-op sync.
///
/// The returned JSON shape is stable (consumed by the CLI/UI):
/// ```json
/// { "ok": bool, "configured": bool, "stage": "<step>", "message": "<human>" }
/// ```
/// `ok` is the single source of truth ("is cloud sync ready?"); `stage` and
/// `message` are for display. No secrets are ever included in the output.
#[cfg(feature = "cloud-sync")]
async fn test_cloud_connection() -> serde_json::Value {
    use crate::cloud::CloudConfig;

    // Resolve credentials the same way the daemon's cloud orchestrator does
    // (env vars first, then the persisted AppConfig the UI writes).
    let cfg = match CloudConfig::from_env() {
        Some(c) => c,
        None => {
            return serde_json::json!({
                "ok": false,
                "configured": false,
                "stage": "config",
                "message": "Supabase is not configured. Set the project URL and anon key \
                            (Settings → Sync, or `copypaste cloud setup`).",
            });
        }
    };

    // Mirror the daemon's HTTPS-only gate so the diagnostic matches what
    // start_cloud would actually accept.
    if !cfg
        .supabase_url
        .to_ascii_lowercase()
        .starts_with("https://")
    {
        return serde_json::json!({
            "ok": false,
            "configured": true,
            "stage": "url",
            "message": format!(
                "Supabase URL must use https:// (got {}). Cloud sync refuses plain http.",
                cfg.supabase_url
            ),
        });
    }

    // Bearer: prefer an email/password GoTrue token (authenticated scope, the
    // scope RLS expects), falling back to the anon key. Credentials come from
    // `CloudConfig` (env vars first, then the persisted `0600` config written by
    // `copypaste cloud setup`) — the same resolution the orchestrator uses. We
    // do NOT fail the whole probe if sign-in fails — we report it as the failing
    // stage so the user can fix credentials specifically.
    let (bearer, signed_in) = match (cfg.email.as_deref(), cfg.password.as_deref()) {
        (Some(email), Some(password)) if !email.is_empty() && !password.is_empty() => {
            let auth = copypaste_supabase::auth::AuthClient::new(&cfg.supabase_url, &cfg.anon_key);
            match auth.sign_in(email, password).await {
                Ok(session) => (session.access_token, true),
                Err(e) => {
                    return serde_json::json!({
                        "ok": false,
                        "configured": true,
                        "stage": "auth",
                        "message": format!(
                            "Sign-in failed for {email}: {e}. Re-check the email/password \
                             (run `copypaste cloud setup` again, or set SUPABASE_EMAIL / \
                             SUPABASE_PASSWORD), and that the user is confirmed."
                        ),
                    });
                }
            }
        }
        _ => (cfg.anon_key.clone(), false),
    };

    // One cheap REST round-trip. `limit=0` returns an empty array on success
    // without transferring any rows, so it is safe even on a large table.
    // CopyPaste-16vr: use a request timeout so a stalled endpoint cannot
    // block the IPC handler indefinitely. 30 s matches SYNC_HTTP_TIMEOUT.
    let url = format!("{}/rest/v1/clipboard_items?limit=0", cfg.supabase_url);
    let client = reqwest::Client::builder()
        .timeout(crate::sync_common::SYNC_HTTP_TIMEOUT)
        .build()
        .unwrap_or_else(|_| reqwest::Client::new()); // Client::new is fine here — timeout on send
    let resp = match client
        .get(&url)
        .header("apikey", &cfg.anon_key)
        .header("Authorization", format!("Bearer {bearer}"))
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            return serde_json::json!({
                "ok": false,
                "configured": true,
                "stage": "network",
                "message": format!(
                    "Could not reach {}: {e}. Check the URL and your network/proxy.",
                    cfg.supabase_url
                ),
            });
        }
    };

    let status = resp.status();
    let code = status.as_u16();
    if status.is_success() {
        let scope = if signed_in {
            "signed in (authenticated scope)"
        } else {
            "anon key (sign in for full scope)"
        };
        return serde_json::json!({
            "ok": true,
            "configured": true,
            "stage": "done",
            "message": format!("Connected to Supabase — table reachable, {scope}."),
        });
    }

    // Classify the common failure HTTP codes into actionable guidance.
    let body = resp.text().await.unwrap_or_default();
    let (stage, message) = match code {
        // 401 has two distinct root causes. When we already hold an
        // authenticated bearer (`signed_in`), the anon key itself must be
        // wrong/expired. When the probe used only the anon key (no sign-in),
        // the project's `authenticated`-only RLS rejects the request and the
        // fix is to supply email/password, not to re-copy the anon key.
        401 if signed_in => (
            "auth",
            "401 Unauthorized — the anon key is wrong or expired. Re-copy it from \
             Supabase → Project Settings → API."
                .to_string(),
        ),
        401 => (
            "auth",
            "401 Unauthorized — the request used the anon key with no signed-in \
             session, and the table's RLS grants only the `authenticated` role. \
             Provide email/password (run `copypaste cloud setup` and supply them, \
             or set SUPABASE_EMAIL / SUPABASE_PASSWORD) so the daemon authenticates."
                .to_string(),
        ),
        404 => (
            "schema",
            "404 Not Found — the clipboard_items table is missing. Run the \
             provisioning SQL: `copypaste cloud setup-sql` then paste it into the \
             Supabase SQL Editor."
                .to_string(),
        ),
        // PostgREST returns 400/406 with a 'relation does not exist' hint when
        // the table is absent under some configs; surface the body for clarity.
        400 | 406 => (
            "schema",
            format!(
                "{code} from PostgREST — the table may be missing or misconfigured. \
                 Run `copypaste cloud setup-sql`. Server said: {}",
                body.trim()
            ),
        ),
        403 => (
            "rls",
            "403 Forbidden — row-level security rejected the request. Re-run the RLS \
             part of `copypaste cloud setup-sql`."
                .to_string(),
        ),
        _ => (
            "http",
            format!("Unexpected HTTP {code} from Supabase: {}", body.trim()),
        ),
    };
    serde_json::json!({
        "ok": false,
        "configured": true,
        "stage": stage,
        "message": message,
    })
}
