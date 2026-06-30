//! Cloud/relay sync + passphrase/rotation IPC handlers (split from ipc god-module, ra15.1).
use super::*;

impl IpcServer {
    pub(crate) async fn dispatch_sync(&self, req: Request) -> Response {
        match req.method.as_str() {
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
            "store_cloud_password" => {
                // nq39: parse only the `password` field we care about.
                // Use a local struct so the daemon does not need to depend on
                // `copypaste-ipc` (that crate is for clients — CLI and UI).
                #[derive(serde::Deserialize)]
                struct StoreCloudPasswordParams {
                    password: String,
                }
                let params: StoreCloudPasswordParams =
                    match serde_json::from_value(req.params.clone()) {
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
                        let persisted = crate::keychain::read_supabase_password_from_keychain()
                            .as_deref()
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
            //
            // `cloud_sign_out`: unconditionally clear `cloud_signed_in` so
            // `get_sync_status` immediately reflects the signed-out state.
            #[cfg(feature = "cloud-sync")]
            "cloud_sign_in" => {
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
                let auth =
                    copypaste_supabase::auth::AuthClient::new(&cfg.supabase_url, &cfg.anon_key);
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
                        tracing::warn!(
                            "cloud_sign_in: sign-in failed ({e}); cloud_signed_in = false"
                        );
                        Response::err_with_code(
                            req.id,
                            ERR_CODE_AUTH_FAILED,
                            format!("sign-in failed: {e}"),
                        )
                    }
                }
            }
            #[cfg(feature = "cloud-sync")]
            "cloud_sign_out" => {
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
            // When cloud-sync is not compiled in, cloud_sign_in / cloud_sign_out
            // are not available. Return not_implemented so clients see a
            // machine-readable error_code rather than "method not found".
            #[cfg(not(feature = "cloud-sync"))]
            "cloud_sign_in" | "cloud_sign_out" => Response::not_implemented(req.id, "cloud-sync"),

            // ── cloud-sync IPC methods ──────────────────────────────────────
            //
            // `set_sync_passphrase` and `get_sync_status` are the UI-facing
            // surface for the cross-device shared encryption key. Both are
            // compiled in only when the `cloud-sync` Cargo feature is active.
            #[cfg(feature = "cloud-sync")]
            "set_sync_passphrase" => {
                let passphrase = match req.params.get("passphrase").and_then(|v| v.as_str()) {
                    Some(p) if !p.is_empty() => p.to_owned(),
                    _ => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INVALID_ARGUMENT,
                            "missing or empty param: passphrase",
                        )
                    }
                };

                // Derive the v1 (global-salt) sync key via Argon2id (intentionally
                // slow — one-time cost on passphrase entry, not per-item).
                //
                // CopyPaste-jdq5: the v1 key remains the SHARED key for relay /
                // P2P / cloud-read-fallback — it is byte-identical to the previous
                // `derive_sync_key`, so relay inbox addressing and all existing
                // ciphertexts are unchanged. The account-aware v2 per-account-salt
                // key is derived SEPARATELY below (`refresh_cloud_v2_key`) into a
                // dedicated cloud-only slot, so the relay key never changes.
                let new_key = match derive_sync_key_versioned(&passphrase, None) {
                    Ok(k) => k,
                    Err(e) => {
                        tracing::warn!("set_sync_passphrase: key derivation failed: {e}");
                        return Response::err(req.id, format!("key derivation failed: {e}"));
                    }
                };

                // Persist via the SAME backend the device key uses (0600 file
                // store on unsigned installs, Keychain otherwise) and swap the
                // live slot so the cloud loops pick it up immediately. This also
                // clears any stale v2 key. The key bytes are never logged.
                self.persist_and_install_sync_key(new_key).await;
                // CopyPaste-jdq5: derive + install the v2 per-account cloud key
                // when a Supabase account id is known. No-op (stays on v1) when not
                // signed in yet — the cutover then happens on the next passphrase
                // entry while signed in.
                self.refresh_cloud_v2_key(&passphrase).await;
                tracing::info!("set_sync_passphrase: sync key updated");
                Response::ok(req.id, serde_json::json!({"ok": true}))
            }

            // ── C-P0-4: honest cloud/relay device revocation ────────────────
            //
            // Revoking a peer (`revoke_peer`) only cuts off P2P (mTLS allowlist
            // + revoked_fingerprints denylist). It does NOT cut off cloud /
            // relay sync, because the revoked device still holds the shared sync
            // key — it can keep decrypting NEW cloud items and keeps addressing
            // the SAME relay inbox (the inbox id is HKDF of the sync key).
            //
            // The ONLY real cloud/relay revocation is ROTATING the sync key:
            //   * the old key can no longer decrypt items encrypted under the
            //     new key (XChaCha20-Poly1305 auth-tag rejection — see
            //     copypaste_core::sync_key);
            //   * the relay inbox id (HKDF of the sync key — see
            //     copypaste_core::relay::derive_relay_inbox_id) diverges, so the
            //     revoked device's saved token now addresses a DEAD inbox.
            //
            // `rotate_sync_key` accepts a NEW passphrase, derives a fresh key,
            // and installs it via the SAME persist + slot-swap path as
            // `set_sync_passphrase`. Remaining devices must re-provision (re-scan
            // the pairing QR or re-enter the new passphrase) to keep syncing.
            //
            // Available for BOTH cloud-sync (Supabase) and relay-sync: the relay
            // inbox id is HKDF of the sync key, so rotating it cuts off the
            // revoked device's relay access too.
            #[cfg(any(feature = "cloud-sync", feature = "relay-sync"))]
            "rotate_sync_key" => {
                let passphrase = match req.params.get("passphrase").and_then(|v| v.as_str()) {
                    Some(p) if !p.is_empty() => p.to_owned(),
                    _ => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INVALID_ARGUMENT,
                            "missing or empty param: passphrase",
                        )
                    }
                };

                // CopyPaste-wg4w: legacy v1 fallback (account id passed as None) —
                // see the detailed rationale in the `set_sync_passphrase` handler.
                let new_key = match derive_sync_key_versioned(&passphrase, None) {
                    Ok(k) => k,
                    Err(e) => {
                        tracing::warn!("rotate_sync_key: key derivation failed: {e}");
                        return Response::err(req.id, format!("key derivation failed: {e}"));
                    }
                };

                // CopyPaste-vvsf: snapshot old key bytes BEFORE installing the new
                // one so the re-encryption closure can decrypt existing Supabase rows.
                // The bytes are captured as a plain array (SyncKey is !Clone) and
                // used only inside the closure below.
                #[cfg(feature = "cloud-sync")]
                let old_key_bytes: Option<[u8; 32]> = {
                    let guard = self.sync_key.lock().await;
                    guard.as_ref().map(|k| *k.as_bytes())
                };
                #[cfg(not(feature = "cloud-sync"))]
                let _old_key_bytes: Option<[u8; 32]> = None;

                // Derive new key bytes BEFORE consuming new_key (persist_and_install
                // will move it into the Arc<Mutex<…>> slot).
                #[cfg(feature = "cloud-sync")]
                let new_key_bytes: [u8; 32] = *new_key.as_bytes();

                self.persist_and_install_sync_key(new_key).await;
                // CopyPaste-jdq5: re-derive the v2 per-account cloud key under the
                // NEW passphrase (cloud-sync only; `persist_and_install_sync_key`
                // already cleared the stale v2).
                #[cfg(feature = "cloud-sync")]
                self.refresh_cloud_v2_key(&passphrase).await;
                tracing::info!(
                    "rotate_sync_key: sync key rotated; relay inbox id will diverge and the old \
                     key can no longer decrypt new cloud items"
                );

                // CopyPaste-vvsf: re-encrypt all existing cloud rows under the new
                // key so devices provisioned with the new passphrase can still read
                // historic items.  This is a best-effort network call: we log results
                // but never fail the rotate_sync_key response because:
                //   * The key rotation itself (local Keychain + in-memory slot) has
                //     already succeeded and is the caller's primary intent.
                //   * A partial re-encryption (network failure mid-batch) is recoverable
                //     — the push loop will re-upload newly-captured items under the new
                //     key; the caller can retry rotate_sync_key to attempt re-encryption
                //     of remaining rows.
                #[cfg(feature = "cloud-sync")]
                {
                    use base64::Engine as _;
                    use copypaste_core::{decrypt_from_cloud, encrypt_for_cloud};
                    use copypaste_supabase::RestClient;

                    if let Some(old_bytes) = old_key_bytes {
                        match RestClient::from_env() {
                            Ok(rest_client) => {
                                let (ok, skipped, err) = rest_client
                                    .reencrypt_all_cloud_items(
                                        move |item_id, old_ct_b64| -> Result<String, String> {
                                            let old_k =
                                                copypaste_core::SyncKey::from_bytes(old_bytes);
                                            let new_k =
                                                copypaste_core::SyncKey::from_bytes(new_key_bytes);
                                            let raw = base64::engine::general_purpose::STANDARD
                                                .decode(old_ct_b64)
                                                .map_err(|e| format!("base64: {e}"))?;
                                            let plain = decrypt_from_cloud(&old_k, item_id, &raw)
                                                .map_err(|e| format!("decrypt: {e}"))?;
                                            let new_blob =
                                                encrypt_for_cloud(&new_k, item_id, &plain)
                                                    .map_err(|e| format!("re-encrypt: {e}"))?;
                                            Ok(base64::engine::general_purpose::STANDARD
                                                .encode(&new_blob))
                                        },
                                    )
                                    .await
                                    .unwrap_or((0, 0, 0));
                                tracing::info!(
                                    ok,
                                    skipped,
                                    err,
                                    "rotate_sync_key: cloud re-encryption complete \
                                     (CopyPaste-vvsf)"
                                );
                            }
                            Err(e) => {
                                // Access token not in env — cloud is managed by GoTrue
                                // inside start_cloud. Log a warning but do not fail the
                                // rotate (the key rotation is already done).
                                tracing::warn!(
                                    error = %e,
                                    "rotate_sync_key: RestClient not available from env — \
                                     existing cloud rows NOT re-encrypted (CopyPaste-vvsf); \
                                     the push loop will re-upload future items under the new key"
                                );
                            }
                        }
                    } else {
                        tracing::debug!(
                            "rotate_sync_key: no previous sync key — nothing to re-encrypt \
                             in cloud (CopyPaste-vvsf)"
                        );
                    }
                }

                Response::ok(req.id, serde_json::json!({"ok": true, "rotated": true}))
            }

            // C-P0-4: revoke a peer from P2P AND rotate the sync key in one call,
            // so the revoked device is cut off from cloud/relay sync too. Runs
            // the SAME body as `revoke_peer` (P2P allowlist eviction + audit
            // row), then derives & installs the new sync key. The new passphrase
            // is required; if it is missing/invalid we do NOT revoke (so the
            // caller can retry without a half-applied state).
            //
            // SECURITY (C-P0-4 / CopyPaste-gbo): previously gated only on
            // `cloud-sync`. Widened to `relay-sync` because the relay inbox id is
            // HKDF-derived from the sync key — without rotation a revoked device
            // retains its relay inbox address and the shared key to decrypt new
            // relay items. `revoke_peer` alone (P2P-only denylist) is NOT
            // sufficient revocation when relay-sync is active.
            #[cfg(any(feature = "cloud-sync", feature = "relay-sync"))]
            "revoke_and_rotate" => {
                let fingerprint = match req.params.get("fingerprint").and_then(|v| v.as_str()) {
                    Some(s) => s.to_string(),
                    None => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INVALID_ARGUMENT,
                            "missing param: fingerprint",
                        )
                    }
                };
                if !is_valid_fingerprint(&fingerprint) {
                    return Response::err_with_code(
                        req.id,
                        ERR_CODE_INVALID_ARGUMENT,
                        format!("invalid fingerprint format: {fingerprint}"),
                    );
                }
                let passphrase = match req.params.get("passphrase").and_then(|v| v.as_str()) {
                    Some(p) if !p.is_empty() => p.to_owned(),
                    _ => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INVALID_ARGUMENT,
                            "missing or empty param: passphrase",
                        )
                    }
                };
                // Derive the new key FIRST so a bad passphrase fails before we
                // mutate any revocation state.
                // CopyPaste-wg4w: legacy v1 fallback (account id passed as None) —
                // see the detailed rationale in the `set_sync_passphrase` handler.
                let new_key = match derive_sync_key_versioned(&passphrase, None) {
                    Ok(k) => k,
                    Err(e) => {
                        tracing::warn!("revoke_and_rotate: key derivation failed: {e}");
                        return Response::err(req.id, format!("key derivation failed: {e}"));
                    }
                };

                // ── Revoke (same as the `revoke_peer` body) ──
                let (removed, captured_name) = match load_peers() {
                    Ok(mut peers) => {
                        let before_len = peers.len();
                        // Normalise both sides so colon-hex display fingerprints
                        // and bare-hex canonical fingerprints both match
                        // (CopyPaste-qvn: raw string compare missed cross-format).
                        let fp_canonical = canonical_fingerprint(&fingerprint);
                        let name = peers
                            .iter()
                            .find(|p| {
                                p.get("fingerprint")
                                    .and_then(|v| v.as_str())
                                    .map(|f| canonical_fingerprint(f) == fp_canonical)
                                    .unwrap_or(false)
                            })
                            .and_then(|p| p.get("name"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        peers.retain(|p| {
                            p.get("fingerprint")
                                .and_then(|v| v.as_str())
                                .map(|f| canonical_fingerprint(f) != fp_canonical)
                                .unwrap_or(true)
                        });
                        if let Err(e) = save_peers(&peers) {
                            return Response::err(req.id, format!("failed to save peers: {e}"));
                        }
                        (peers.len() < before_len, name)
                    }
                    Err(e) => return Response::err(req.id, format!("failed to load peers: {e}")),
                };

                let db_arc = self.db.clone();
                let fp_for_db = fingerprint.clone();
                let name_for_db = captured_name.clone();
                let join = tokio::task::spawn_blocking(move || {
                    let db = db_arc.blocking_lock();
                    revoke_device(db.conn(), &fp_for_db, &name_for_db)
                })
                .await;

                let revoked_at = match join {
                    Ok(Ok(ts)) => {
                        // Evict from the live mTLS allowlist immediately.
                        if let Some(ref peers) = self.p2p_peers {
                            peers.remove(&canonical_fingerprint(&fingerprint));
                        }
                        ts
                    }
                    Ok(Err(e)) => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INTERNAL_ERROR,
                            format!("failed to record revocation: {e}"),
                        )
                    }
                    Err(e) => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INTERNAL_ERROR,
                            format!("revoke task join error: {e}"),
                        )
                    }
                };

                // ── Rotate the sync key (cuts off cloud/relay for the revoked
                // device; remaining devices must re-provision). ──
                self.persist_and_install_sync_key(new_key).await;
                // CopyPaste-jdq5: re-derive the v2 per-account cloud key under the
                // NEW passphrase (cloud-sync only).
                #[cfg(feature = "cloud-sync")]
                self.refresh_cloud_v2_key(&passphrase).await;
                tracing::info!(
                    "revoke_and_rotate: revoked peer and rotated sync key; remaining devices must \
                     re-provision to keep syncing"
                );
                Response::ok(
                    req.id,
                    serde_json::json!({
                        "ok": true,
                        "removed": removed,
                        "revoked_at": revoked_at,
                        "fingerprint": fingerprint,
                        "rotated": true,
                    }),
                )
            }

            #[cfg(feature = "cloud-sync")]
            "get_sync_status" => {
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
            "cloud_test_connection" => {
                let result = test_cloud_connection().await;
                Response::ok(req.id, result)
            }

            // When cloud-sync is not compiled in, return not_implemented for
            // Supabase-specific methods so the UI gets a machine-readable code
            // rather than "method not found".
            #[cfg(not(feature = "cloud-sync"))]
            "set_sync_passphrase" | "get_sync_status" | "cloud_test_connection" => {
                Response::not_implemented(req.id, "cloud-sync")
            }

            // rotate_sync_key and revoke_and_rotate are available when EITHER
            // cloud-sync OR relay-sync is compiled in (widened from cloud-sync
            // only — CopyPaste-gbo). When neither is active, report
            // not_implemented rather than "method not found" so callers can
            // distinguish "feature off" from "unknown method".
            #[cfg(not(any(feature = "cloud-sync", feature = "relay-sync")))]
            "rotate_sync_key" | "revoke_and_rotate" => {
                Response::not_implemented(req.id, "cloud-sync or relay-sync")
            }
            _ => self.dispatch_status(req).await,
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
