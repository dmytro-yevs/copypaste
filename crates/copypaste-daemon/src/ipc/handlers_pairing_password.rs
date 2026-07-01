//! Password-based PAKE pairing IPC verbs (split from handlers_pairing.rs,
//! ADR-017 daemon-ipc track, CopyPaste-vp63.16).
//!
//! SECURITY: `handle_pair_accept_finish` contains the mandatory initiator
//! confirmation-tag verification. The constant-time compare
//! (`subtle::ConstantTimeEq` / `received.as_slice().ct_eq(&expected)`) MUST
//! stay constant-time — moved verbatim from handlers_pairing.rs.
use super::*;

impl IpcServer {
    /// W2.4 — PAKE-based password pairing (initiator side).
    ///
    /// Two-step protocol over IPC:
    ///   step="initiate": validates inputs, creates PakeInitiator,
    ///     stores session in pake_sessions, returns {session_id, message1_b64}.
    ///   step="finish": looks up PakeInitiator by session_id, completes
    ///     handshake with server's message2, stores peer, returns
    ///     {ok: true, message3_b64}.
    pub(crate) async fn handle_pair_peer_with_password(&self, req: Request) -> Response {
        use base64::Engine as _;
        let b64 = base64::engine::general_purpose::STANDARD;

        let peer_fingerprint = match extract_str_param(
            &req.params,
            req.id.clone(),
            "peer_fingerprint",
            "missing peer_fingerprint",
        ) {
            Ok(s) => s,
            Err(resp) => return resp,
        };

        if !is_valid_fingerprint(&peer_fingerprint) {
            return Response::err_with_code(
                req.id,
                ERR_CODE_INVALID_ARGUMENT,
                format!("invalid peer_fingerprint format: {peer_fingerprint}"),
            );
        }

        let step = req
            .params
            .get("step")
            .and_then(|v| v.as_str())
            .unwrap_or("initiate")
            .to_string();

        match step.as_str() {
            "initiate" => {
                let password = match extract_str_param(
                    &req.params,
                    req.id.clone(),
                    "password",
                    "missing password",
                ) {
                    Ok(s) => s,
                    Err(resp) => return resp,
                };

                if password.chars().count() < 6 {
                    return Response::err_with_code(
                        req.id,
                        ERR_CODE_INVALID_ARGUMENT,
                        "password must be at least 6 characters",
                    );
                }

                let (initiator, msg1_bytes) = match PakeInitiator::new(&password) {
                    Ok(pair) => pair,
                    Err(e) => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INTERNAL_ERROR,
                            format!("PAKE init failed: {e}"),
                        )
                    }
                };

                let session_id = uuid::Uuid::new_v4().to_string();
                let msg1_b64 = b64.encode(&msg1_bytes);

                if let Err(msg) = self
                    .insert_pake_session(
                        session_id.clone(),
                        PakeSession::Initiator(Box::new(initiator)),
                    )
                    .await
                {
                    return Response::err_with_code(req.id, ERR_CODE_INTERNAL_ERROR, msg);
                }

                Response::ok(
                    req.id,
                    serde_json::json!({
                        "session_id": session_id,
                        "message1_b64": msg1_b64,
                    }),
                )
            }

            "finish" => {
                let session_id = match extract_str_param(
                    &req.params,
                    req.id.clone(),
                    "session_id",
                    "missing session_id for step=finish",
                ) {
                    Ok(s) => s,
                    Err(resp) => return resp,
                };
                let msg2_b64 = match extract_str_param(
                    &req.params,
                    req.id.clone(),
                    "message2_b64",
                    "missing message2_b64 for step=finish",
                ) {
                    Ok(s) => s,
                    Err(resp) => return resp,
                };

                let msg2_bytes = match b64.decode(&msg2_b64) {
                    Ok(b) => b,
                    Err(e) => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INVALID_ARGUMENT,
                            format!("invalid base64 in message2_b64: {e}"),
                        )
                    }
                };

                // Extract and consume the initiator session.
                let initiator = {
                    let mut sessions = self.pake_sessions.lock().await;
                    match sessions.remove(&session_id) {
                        Some(StampedPakeSession {
                            session: PakeSession::Initiator(i),
                            ..
                        }) => *i,
                        Some(other) => {
                            // Wrong session type — put it back and error.
                            let key = session_id.clone();
                            sessions.insert(key, other);
                            return Response::err_with_code(
                                req.id,
                                ERR_CODE_INVALID_ARGUMENT,
                                "session_id refers to a responder session, not initiator",
                            );
                        }
                        None => {
                            return Response::err_with_code(
                                req.id,
                                ERR_CODE_INVALID_ARGUMENT,
                                format!("unknown session_id: {session_id}"),
                            )
                        }
                    }
                };

                // S3 (CopyPaste-4ca): consume the SessionKey to derive a
                // cert-fingerprint-bound confirmation tag.
                //
                // The IPC path has no shared TLS channel between the two
                // devices, so RFC 5705 `export_keying_material` is not
                // available.  Instead we bind the SessionKey to the pair
                // of cert fingerprints (own + peer) that mTLS already
                // pins.  A relay/MitM that uses different certs will have
                // a different fingerprint pair → different binder →
                // different bound_key → confirmation tags that will not
                // match on the responder side → handshake aborted.
                //
                // Residual gap: the binder is built from the fingerprints
                // the UI supplies.  A MitM that can forge BOTH fingerprints
                // in the UI channel AND intercept/substitute PAKE messages
                // would still succeed.  Full RFC 5705 binding (over a
                // shared TLS exporter) is not achievable on this path
                // without a protocol change; that gap is tracked in bd
                // issue CopyPaste-4ca notes.
                let (session_key, msg3_bytes) = match initiator.finish(&msg2_bytes) {
                    Ok(pair) => pair,
                    Err(e) => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_AUTH_FAILED,
                            format!("PAKE finish failed: {e}"),
                        )
                    }
                };

                // Derive the cert-binder from both fingerprints and bind
                // the session key to it.  `own_fp` may be `None` in tests
                // without a cert; fall back to a zero binder in that case
                // (still binds the session key, just weakly — production
                // daemons always have a cert fingerprint).
                let own_fp = self.cert_fingerprint.clone().unwrap_or_default();
                let binder = Self::pake_cert_binder(&own_fp, &peer_fingerprint);
                let bound_key = session_key.bind_to_tls_channel(&binder);
                let initiator_tag = channel_confirmation_tag(&bound_key, ConfirmRole::Initiator);
                let initiator_confirm_b64 = b64.encode(initiator_tag);

                let msg3_b64 = b64.encode(&msg3_bytes);

                // Store the paired peer on the initiator side (no PasswordFile).
                let added_at = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();

                match load_peers() {
                    Ok(mut peers) => {
                        // Only add if not already present — normalise
                        // both sides so colon-hex vs bare-hex match.
                        let fp_c = canonical_fingerprint(&peer_fingerprint);
                        let already = peers.iter().any(|p| {
                            p.get("fingerprint")
                                .and_then(|v| v.as_str())
                                .map(|f| canonical_fingerprint(f) == fp_c)
                                .unwrap_or(false)
                        });
                        if !already {
                            peers.push(serde_json::json!({
                                "fingerprint": peer_fingerprint,
                                "added_at": added_at,
                            }));
                            if let Err(e) = save_peers(&peers) {
                                return Response::err(req.id, format!("failed to save peers: {e}"));
                            }
                        }
                    }
                    Err(e) => return Response::err(req.id, format!("failed to load peers: {e}")),
                }

                // Feed the newly-paired peer into the live allowlist so
                // the mTLS accept loop honours it without a restart.
                self.register_live_peer(&peer_fingerprint);

                Response::ok(
                    req.id,
                    serde_json::json!({
                        "ok": true,
                        "message3_b64": msg3_b64,
                        // S3: initiator confirmation tag — responder must
                        // verify this in pair_accept_finish to prove both
                        // sides share the same SessionKey + cert binder.
                        "initiator_confirm_b64": initiator_confirm_b64,
                    }),
                )
            }

            other => Response::err_with_code(
                req.id,
                ERR_CODE_INVALID_ARGUMENT,
                format!("unknown step '{other}'; expected 'initiate' or 'finish'"),
            ),
        }
    }

    /// c4q2.20: pair_accept_password (password-based PAKE responder) is
    /// stubbed not_implemented. The password-pairing flow was removed as a
    /// security concern (CopyPaste-c4q2.20) — use QR pairing
    /// (pair_generate_qr / pair_accept_qr) instead.
    pub(crate) async fn handle_pair_accept_password(&self, req: Request) -> Response {
        Response::err_with_code(
            req.id,
            ERR_CODE_NOT_IMPLEMENTED,
            "pair_accept_password is disabled — use QR pairing (pair_generate_qr / pair_accept_qr) (c4q2.20)",
        )
    }

    /// W2.4 — PAKE responder finish: receives message3 from initiator,
    /// completes handshake, persists peer + PasswordFile.
    /// Params: {session_id, message3_b64, peer_fingerprint}
    /// Response: {ok: true}
    pub(crate) async fn handle_pair_accept_finish(&self, req: Request) -> Response {
        use base64::Engine as _;
        let b64 = base64::engine::general_purpose::STANDARD;

        let session_id = match extract_str_param(
            &req.params,
            req.id.clone(),
            "session_id",
            "missing session_id",
        ) {
            Ok(s) => s,
            Err(resp) => return resp,
        };
        let msg3_b64 = match extract_str_param(
            &req.params,
            req.id.clone(),
            "message3_b64",
            "missing message3_b64",
        ) {
            Ok(s) => s,
            Err(resp) => return resp,
        };

        let msg3_bytes = match b64.decode(&msg3_b64) {
            Ok(b) => b,
            Err(e) => {
                return Response::err_with_code(
                    req.id,
                    ERR_CODE_INVALID_ARGUMENT,
                    format!("invalid base64 in message3_b64: {e}"),
                )
            }
        };

        // CopyPaste-j8dr: the initiator's confirmation tag is now
        // MANDATORY. An absent tag is rejected with AUTH_FAILED so that
        // a relay stripping the field, or an older initiator that never
        // sent one, cannot complete the handshake without mutual
        // confirmation. This closes the backwards-compatibility escape
        // hatch that was left open in the original S3 implementation.
        let initiator_confirm_b64 = match req
            .params
            .get("initiator_confirm_b64")
            .and_then(|v| v.as_str())
        {
            Some(s) => s.to_string(),
            None => {
                return Response::err_with_code(
                    req.id,
                    ERR_CODE_AUTH_FAILED,
                    "missing initiator_confirm_b64 — confirm tag is required",
                )
            }
        };

        // Extract and consume the responder session.
        let (responder, password_file, peer_fingerprint) = {
            let mut sessions = self.pake_sessions.lock().await;
            match sessions.remove(&session_id) {
                Some(StampedPakeSession {
                    session:
                        PakeSession::Responder {
                            responder,
                            password_file,
                            peer_fingerprint,
                        },
                    ..
                }) => (*responder, password_file, peer_fingerprint),
                Some(other) => {
                    let key = session_id.clone();
                    sessions.insert(key, other);
                    return Response::err_with_code(
                        req.id,
                        ERR_CODE_INVALID_ARGUMENT,
                        "session_id refers to an initiator session, not responder",
                    );
                }
                None => {
                    return Response::err_with_code(
                        req.id,
                        ERR_CODE_INVALID_ARGUMENT,
                        format!("unknown session_id: {session_id}"),
                    )
                }
            }
        };

        // S3 (CopyPaste-4ca): finalize the handshake and consume the
        // SessionKey.  Bind it to the cert-fingerprint binder so a
        // relay/MitM using a different cert pair will derive a different
        // bound_key and therefore produce mismatching confirmation tags.
        let session_key = match responder.finish(&msg3_bytes) {
            Ok(sk) => sk,
            Err(e) => {
                return Response::err_with_code(
                    req.id,
                    ERR_CODE_AUTH_FAILED,
                    format!("PAKE accept_finish failed: {e}"),
                );
            }
        };

        let own_fp = self.cert_fingerprint.clone().unwrap_or_default();
        // On the responder side: own_fp is responder's fp, peer_fp is
        // initiator's fp — same canonical (sorted) binder as the other end.
        let binder = Self::pake_cert_binder(&own_fp, &peer_fingerprint);
        let bound_key = session_key.bind_to_tls_channel(&binder);

        // Verify the initiator's confirmation tag (mandatory).
        {
            use subtle::ConstantTimeEq as _;
            let received = match b64.decode(&initiator_confirm_b64) {
                Ok(bytes) => bytes,
                Err(e) => {
                    return Response::err_with_code(
                        req.id,
                        ERR_CODE_INVALID_ARGUMENT,
                        format!("invalid base64 in initiator_confirm_b64: {e}"),
                    )
                }
            };
            if received.len() != CONFIRM_TAG_LEN {
                return Response::err_with_code(
                    req.id,
                    ERR_CODE_AUTH_FAILED,
                    format!(
                        "initiator_confirm_b64 wrong length: expected {CONFIRM_TAG_LEN}, got {}",
                        received.len()
                    ),
                );
            }
            let expected = channel_confirmation_tag(&bound_key, ConfirmRole::Initiator);
            // Constant-time compare — subtle::ConstantTimeEq on slices.
            let ok: bool = received.as_slice().ct_eq(&expected).into();
            if !ok {
                return Response::err_with_code(
                    req.id,
                    ERR_CODE_AUTH_FAILED,
                    "PAKE confirmation tag mismatch (possible relay MitM)",
                );
            }
        }

        // Derive and return the responder's confirmation tag so the
        // initiator can optionally verify it (future extension).
        let responder_tag = channel_confirmation_tag(&bound_key, ConfirmRole::Responder);
        let responder_confirm_b64 = b64.encode(responder_tag);

        // Persist the peer with the PasswordFile blob on the responder side.
        // CopyPaste-5lm: encrypt at rest with XChaCha20-Poly1305 under the
        // daemon's local key. The ciphertext (`password_file_enc`) replaces
        // the former plaintext base64 field (`password_file_b64`).
        let fp_c = canonical_fingerprint(&peer_fingerprint);
        let password_file_enc =
            match encrypt_pake_password_file(&password_file.serialized, &fp_c, &self.local_key) {
                Ok(enc) => enc,
                Err(e) => {
                    return Response::err(
                        req.id,
                        format!("failed to encrypt PasswordFile for storage: {e}"),
                    )
                }
            };
        let added_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        match load_peers() {
            Ok(mut peers) => {
                // Normalise both sides so colon-hex vs bare-hex match
                // (CopyPaste-qvn: raw string compare missed cross-format).
                let already = peers.iter().any(|p| {
                    p.get("fingerprint")
                        .and_then(|v| v.as_str())
                        .map(|f| canonical_fingerprint(f) == fp_c)
                        .unwrap_or(false)
                });
                if !already {
                    peers.push(serde_json::json!({
                        "fingerprint": peer_fingerprint,
                        // password_file_enc: encrypted-at-rest blob.
                        // password_file_b64 is NOT written — new records
                        // always use the encrypted form.
                        "password_file_enc": password_file_enc,
                        "added_at": added_at,
                    }));
                } else {
                    // Update existing peer with the new encrypted PasswordFile.
                    // Also clear any legacy password_file_b64 field.
                    for p in peers.iter_mut() {
                        if p.get("fingerprint")
                            .and_then(|v| v.as_str())
                            .map(|f| canonical_fingerprint(f) == fp_c)
                            .unwrap_or(false)
                        {
                            p["password_file_enc"] =
                                serde_json::Value::String(password_file_enc.clone());
                            // Remove legacy plaintext field if present.
                            if let Some(obj) = p.as_object_mut() {
                                obj.remove("password_file_b64");
                            }
                            break;
                        }
                    }
                }
                if let Err(e) = save_peers(&peers) {
                    return Response::err(req.id, format!("failed to save peers: {e}"));
                }
            }
            Err(e) => return Response::err(req.id, format!("failed to load peers: {e}")),
        }

        // Feed the newly-paired peer into the live allowlist so the
        // mTLS accept loop honours it without a restart.
        self.register_live_peer(&peer_fingerprint);

        Response::ok(
            req.id,
            serde_json::json!({
                "ok": true,
                // S3: responder confirmation tag — the initiator may
                // optionally verify this to prove the responder holds the
                // same SessionKey + cert binder.
                "responder_confirm_b64": responder_confirm_b64,
            }),
        )
    }
}
