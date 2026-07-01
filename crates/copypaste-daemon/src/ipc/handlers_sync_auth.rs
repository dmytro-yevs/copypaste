//! Cloud auth (password / sign-in / sign-out) IPC verbs (split from
//! handlers_sync.rs, ADR-017 daemon-ipc track, CopyPaste-vp63.18).
use super::*;

impl IpcServer {
    // ── nq39: dedicated store_cloud_password verb ──────────────────
    //
    // Stores the Supabase GoTrue account password WITHOUT routing it
    // through set_config and WITHOUT persisting it to config.json.
    //
    // On macOS: writes to the macOS Keychain via the existing
    // `keychain::store_supabase_password_to_keychain` helper (same
    // logic as the set_config path).
    //
    // On non-macOS: no Keychain is available; the password is held
    // in the in-memory slot (`self.in_memory_cloud_password`) for the
    // daemon's lifetime and is never written to config.json.  The
    // caller receives `persisted: false` as a signal that the
    // password will be lost on restart.
    pub(crate) async fn handle_store_cloud_password(&self, req: Request) -> Response {
        // nq39: parse only the `password` field we care about.
        // Use a local struct so the daemon does not need to depend on
        // `copypaste-ipc` (that crate is for clients — CLI and UI).
        #[derive(serde::Deserialize)]
        struct StoreCloudPasswordParams {
            password: String,
        }
        let params: StoreCloudPasswordParams = match serde_json::from_value(req.params.clone()) {
            Ok(p) => p,
            Err(e) => {
                return Response::err_with_code(
                    req.id,
                    ERR_CODE_INVALID_ARGUMENT,
                    format!("invalid store_cloud_password params: {e}"),
                )
            }
        };

        if params.password.trim().is_empty() {
            return Response::err_with_code(
                req.id,
                ERR_CODE_INVALID_ARGUMENT,
                "password must not be empty",
            );
        }

        // Attempt Keychain write (macOS real path) via the blocking
        // thread pool — Security-framework calls must not block the
        // async executor.
        let password_for_task = params.password.clone();
        let join = tokio::task::spawn_blocking(move || {
            crate::keychain::store_supabase_password_to_keychain(&password_for_task)
        })
        .await;

        match join {
            Ok(Ok(())) => {
                // Keychain write succeeded (macOS) or was a no-op
                // (ephemeral-key bypass).  Verify the round-trip to
                // distinguish real persistence from the bypass.
                let persisted = crate::keychain::read_supabase_password_from_keychain().as_deref()
                    == Some(params.password.trim());
                tracing::info!(
                    persisted,
                    "store_cloud_password: keychain write {}",
                    if persisted {
                        "persisted"
                    } else {
                        "bypassed (ephemeral/non-macOS)"
                    }
                );
                // On non-macOS (or ephemeral bypass): hold in-memory
                // so cloud code can still read it this session.
                #[cfg(not(target_os = "macos"))]
                if !persisted {
                    if let Ok(mut guard) = self.in_memory_cloud_password.lock() {
                        *guard = Some(zeroize::Zeroizing::new(params.password.clone()));
                    }
                }
                Response::ok(req.id, serde_json::json!({ "persisted": persisted }))
            }
            Ok(Err(e)) => {
                // Keychain write failed (non-macOS KeychainError::Unsupported
                // or a real macOS Keychain error).  Store in-memory as a
                // best-effort fallback; warn caller via `persisted: false`.
                tracing::warn!(
                    error = %e,
                    "store_cloud_password: keychain write failed; \
                     holding password in-memory only (will be lost on restart)"
                );
                #[cfg(not(target_os = "macos"))]
                if let Ok(mut guard) = self.in_memory_cloud_password.lock() {
                    *guard = Some(zeroize::Zeroizing::new(params.password.clone()));
                }
                Response::ok(req.id, serde_json::json!({ "persisted": false }))
            }
            Err(e) => Response::err_with_code(
                req.id,
                ERR_CODE_INTERNAL_ERROR,
                format!("store_cloud_password blocking task panicked: {e}"),
            ),
        }
    }

    // ── Cloud auth ─────────────────────────────────────────────────
    //
    // `cloud_sign_in`: resolve GoTrue credentials via the same path
    // `start_cloud` uses at daemon startup, then flip `cloud_signed_in`
    // to reflect the real auth state. This fixes CopyPaste-i5b where
    // the flag was never set from the IPC (UI-driven) sign-in path —
    // only the env-var startup path set it.
    #[cfg(feature = "cloud-sync")]
    pub(crate) async fn handle_cloud_sign_in(&self, req: Request) -> Response {
        use crate::cloud::CloudConfig;
        let cfg = match CloudConfig::from_env() {
            Some(c) => c,
            None => {
                return Response::err_with_code(
                    req.id,
                    ERR_CODE_INVALID_ARGUMENT,
                    "cloud-sync not configured: set supabase_url and supabase_anon_key \
                     (via set_config or SUPABASE_URL / SUPABASE_ANON_KEY env vars)",
                );
            }
        };
        // Attempt GoTrue sign-in (or fall through to anon key when no
        // email/password is configured — mirrors resolve_bearer_with_client).
        let auth = copypaste_supabase::auth::AuthClient::new(&cfg.supabase_url, &cfg.anon_key);
        let sign_in_result = match (cfg.email.as_deref(), cfg.password.as_deref()) {
            (Some(email), Some(password)) if !email.is_empty() && !password.is_empty() => {
                auth.sign_in(email, password).await.map(|_| ())
            }
            // No email/password → anon key is the bearer; sign-in
            // succeeds trivially (the key itself is the credential).
            _ => Ok(()),
        };
        match sign_in_result {
            Ok(()) => {
                // CopyPaste-i5b fix: set the shared flag so
                // get_sync_status reports the real authenticated state.
                self.cloud_signed_in.store(true, Ordering::SeqCst);
                tracing::info!("cloud_sign_in: signed in; cloud_signed_in = true");
                Response::ok(req.id, serde_json::json!({"signed_in": true}))
            }
            Err(e) => {
                self.cloud_signed_in.store(false, Ordering::SeqCst);
                tracing::warn!("cloud_sign_in: sign-in failed ({e}); cloud_signed_in = false");
                Response::err_with_code(
                    req.id,
                    ERR_CODE_AUTH_FAILED,
                    format!("sign-in failed: {e}"),
                )
            }
        }
    }

    /// `cloud_sign_out`: unconditionally clear `cloud_signed_in` so
    /// `get_sync_status` immediately reflects the signed-out state.
    #[cfg(feature = "cloud-sync")]
    pub(crate) async fn handle_cloud_sign_out(&self, req: Request) -> Response {
        // CopyPaste-i5b fix: clear the flag on explicit sign-out so
        // get_sync_status stops reporting signed_in = true after logout.
        self.cloud_signed_in.store(false, Ordering::SeqCst);

        // CopyPaste-crh3.100: make sign-out PERSISTENT. Previously only
        // the in-memory flag was cleared, so CloudConfig::from_env
        // re-resolved the Keychain password on the next daemon start and
        // silently re-authenticated — the user stayed signed in across a
        // restart despite signing out. Delete the Keychain Supabase
        // password AND clear the persisted email/password from
        // config.json so credential resolution finds nothing. The
        // Supabase project URL + anon key are deliberately KEPT so the
        // user can sign back in without re-entering project settings.
        if let Err(e) = crate::keychain::delete_supabase_password_from_keychain() {
            tracing::warn!(
                error = %e,
                "cloud_sign_out: failed to delete the Keychain Supabase password"
            );
        }
        let mut cfg = read_config();
        cfg.supabase_email = None;
        cfg.supabase_password = None;
        if let Err(e) = write_config(&cfg) {
            tracing::warn!(
                error = %e,
                "cloud_sign_out: failed to clear persisted Supabase credentials"
            );
        }
        tracing::info!(
            "cloud_sign_out: cloud_signed_in = false; Keychain + persisted \
             Supabase credentials cleared"
        );
        Response::ok(req.id, serde_json::json!({"signed_in": false}))
    }
}
