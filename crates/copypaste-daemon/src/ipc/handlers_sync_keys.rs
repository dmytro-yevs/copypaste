//! Sync-key lifecycle IPC verbs — passphrase set / rotate / revoke+rotate
//! (split from handlers_sync.rs, ADR-017 daemon-ipc track, CopyPaste-vp63.18).
//!
//! SECURITY: this module derives, rotates and installs the shared cloud/relay
//! sync key. Key derivation/rotation semantics MUST move verbatim — do not
//! alter the single-derivation, no-back-compat semantics (per commit
//! 63b5d7f per-account cloud key) or the `SyncKey::random` vs
//! passphrase-derived choice (ADR-017 review checkpoint).
use super::*;

impl IpcServer {
    // ── cloud-sync IPC methods ──────────────────────────────────────
    //
    // `set_sync_passphrase` and `get_sync_status` are the UI-facing
    // surface for the cross-device shared encryption key. Both are
    // compiled in only when the `cloud-sync` Cargo feature is active.
    #[cfg(feature = "cloud-sync")]
    pub(crate) async fn handle_set_sync_passphrase(&self, req: Request) -> Response {
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

        // The single per-account sync key REQUIRES the Supabase account
        // id: it is the salt input that makes the key per-account and
        // defeats cross-user precompute. Cloud sync only runs when signed
        // in, so the account id is present in the normal flow; if it is
        // absent, fail cleanly rather than deriving a key no other device
        // of the account can reproduce.
        let account_id = match self.require_cloud_account_id() {
            Ok(id) => id,
            Err(msg) => return Response::err_with_code(req.id, ERR_CODE_AUTH_FAILED, msg),
        };

        // Derive the sync key via Argon2id with the per-account salt
        // (intentionally slow — one-time cost on passphrase entry, not
        // per-item). This single key is shared by the cloud (Supabase) and
        // relay paths.
        let new_key = match derive_sync_key(&passphrase, &account_id) {
            Ok(k) => k,
            Err(e) => {
                tracing::warn!("set_sync_passphrase: key derivation failed: {e}");
                return Response::err(req.id, format!("key derivation failed: {e}"));
            }
        };

        // Persist via the SAME backend the device key uses (0600 file
        // store on unsigned installs, Keychain otherwise) and swap the
        // live slot so the cloud + relay loops pick it up immediately.
        // The key bytes are never logged.
        self.persist_and_install_sync_key(new_key).await;
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
    pub(crate) async fn handle_rotate_sync_key(&self, req: Request) -> Response {
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

        // The single per-account sync key REQUIRES the Supabase account id
        // (see `set_sync_passphrase`). Fail cleanly if absent.
        let account_id = match self.require_cloud_account_id() {
            Ok(id) => id,
            Err(msg) => return Response::err_with_code(req.id, ERR_CODE_AUTH_FAILED, msg),
        };
        let new_key = match derive_sync_key(&passphrase, &account_id) {
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
                                    let old_k = copypaste_core::SyncKey::from_bytes(old_bytes);
                                    let new_k = copypaste_core::SyncKey::from_bytes(new_key_bytes);
                                    let raw = base64::engine::general_purpose::STANDARD
                                        .decode(old_ct_b64)
                                        .map_err(|e| format!("base64: {e}"))?;
                                    let plain = decrypt_from_cloud(&old_k, item_id, &raw)
                                        .map_err(|e| format!("decrypt: {e}"))?;
                                    let new_blob = encrypt_for_cloud(&new_k, item_id, &plain)
                                        .map_err(|e| format!("re-encrypt: {e}"))?;
                                    Ok(base64::engine::general_purpose::STANDARD.encode(&new_blob))
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
    pub(crate) async fn handle_revoke_and_rotate(&self, req: Request) -> Response {
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
        // Derive the new key FIRST so a bad passphrase / missing account
        // fails before we mutate any revocation state. The single
        // per-account sync key REQUIRES the Supabase account id (see
        // `set_sync_passphrase`).
        let account_id = match self.require_cloud_account_id() {
            Ok(id) => id,
            Err(msg) => return Response::err_with_code(req.id, ERR_CODE_AUTH_FAILED, msg),
        };
        let new_key = match derive_sync_key(&passphrase, &account_id) {
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
}
