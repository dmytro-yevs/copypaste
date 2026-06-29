//! Pairing / unpair / revoke IPC handlers (split from ipc god-module, ra15.1).
use super::*;

impl IpcServer {
    pub(crate) async fn dispatch_pairing(&self, req: Request) -> Response {
        match req.method.as_str() {
            // LAN/SAS Phase 2: begin a discovery-initiated SAS pairing as the
            // INITIATOR. Resolves the peer's bootstrap port (`bport`) from the
            // shared discovery snapshot, generates an EPHEMERAL random PAKE
            // password (the SAS — derived from the post-PAKE bound_key — is the
            // real authenticator; the password is sent in-clear inside the
            // bootstrap TLS), and runs `run_initiator_with_confirm` with a
            // callback wired into the pairing state machine.
            "pair_with_discovered" => {
                let device_id = match req.params.get("device_id").and_then(|v| v.as_str()) {
                    Some(s) => s.to_string(),
                    None => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INVALID_ARGUMENT,
                            "missing param: device_id",
                        )
                    }
                };
                self.pair_with_discovered(req.id.clone(), &device_id).await
            }

            // LAN/SAS Phase 2: poll the pairing state machine. Returns the
            // current state plus the SAS + role when awaiting confirmation.
            // Also surfaces whatever peer metadata is known at this point:
            //   • peer_device_name  — mDNS advertised name (initiator path)
            //   • peer_ip_addrs     — resolved IP addresses (initiator path)
            //   • peer_fingerprint  — cert fingerprint = mDNS device_id (initiator path)
            // These are all Optional — absent on the responder path (inbound
            // connection, no prior mDNS resolution) and gracefully omitted by
            // the UI. Model/OS/version are NOT surfaced here: the PAKE metadata
            // extension happens AFTER the SAS confirm step; they appear in the
            // final `pair_with_discovered` response once both sides accept.
            "pair_get_sas" => {
                let state = self.pairing.snapshot();
                let mut body = serde_json::json!({ "state": state.as_str() });
                if let Some(sas) = state.sas() {
                    body["sas"] = serde_json::Value::String(sas.to_string());
                }
                if let Some(role) = state.role() {
                    body["role"] = serde_json::Value::String(role.as_str().to_string());
                }
                if let Some(snap) = state.peer_snapshot() {
                    if let Some(ref name) = snap.device_name {
                        body["peer_device_name"] = serde_json::Value::String(name.clone());
                    }
                    if !snap.ip_addrs.is_empty() {
                        body["peer_ip_addrs"] = serde_json::Value::Array(
                            snap.ip_addrs
                                .iter()
                                .map(|a| serde_json::Value::String(a.clone()))
                                .collect(),
                        );
                    }
                    if let Some(ref fp) = snap.fingerprint {
                        body["peer_fingerprint"] = serde_json::Value::String(fp.clone());
                    }
                }
                Response::ok(req.id, body)
            }

            // LAN/SAS Phase 2: deliver the local user's accept/reject decision
            // into the in-flight handshake's confirm callback. The pairing
            // succeeds (keys trusted + persisted) only when BOTH sides accept.
            "pair_confirm_sas" => {
                let accept = match req.params.get("accept").and_then(|v| v.as_bool()) {
                    Some(b) => b,
                    None => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INVALID_ARGUMENT,
                            "missing or non-boolean param: accept",
                        )
                    }
                };
                let delivered = self.pairing.deliver_decision(accept);
                if !delivered {
                    return Response::err_with_code(
                        req.id,
                        ERR_CODE_INVALID_ARGUMENT,
                        "no pairing is awaiting SAS confirmation",
                    );
                }
                Response::ok(
                    req.id,
                    serde_json::json!({ "ok": true, "accepted": accept }),
                )
            }

            // LAN/SAS Phase 2: abort an in-flight pairing. Dropping the confirm
            // channel resolves the handshake's await as a rejection so the
            // session key drops/zeroizes; the machine moves to `aborted`.
            "pair_abort" => {
                self.pairing.abort();
                Response::ok(req.id, serde_json::json!({ "ok": true }))
            }

            // CopyPaste-3n9h: `pair_peer` previously trusted a peer and
            // registered it in the live mTLS allowlist WITHOUT any
            // authentication (no PAKE, no SAS). A caller that knew a peer's
            // TLS fingerprint could add it as trusted with no proof of identity.
            //
            // The unauthenticated path is now DISABLED. All pairing MUST go
            // through the authenticated paths:
            //   • QR / password: `pair_peer_with_password` + `pair_accept_finish`
            //   • LAN/SAS discovery: `pair_with_discovered` + `pair_confirm_sas`
            //
            // This handler is retained (not removed) so old CLI versions
            // receive an explicit error instead of "unknown method", which
            // makes the upgrade path diagnosable.
            "pair_peer" => Response::err_with_code(
                req.id,
                ERR_CODE_NOT_IMPLEMENTED,
                "pair_peer is disabled: use pair_peer_with_password (QR/password) \
                 or pair_with_discovered (LAN/SAS) for authenticated pairing",
            ),

            "unpair_peer" => {
                let fingerprint = match req.params.get("fingerprint").and_then(|v| v.as_str()) {
                    Some(s) => s.to_string(),
                    None => return Response::err(req.id, "missing param: fingerprint"),
                };

                match load_peers() {
                    Ok(mut peers) => {
                        let before_len = peers.len();
                        let fp_canonical = canonical_fingerprint(&fingerprint);
                        // Gap A: capture the peer's last-known dial address +
                        // display name BEFORE removing the record, so a durable
                        // pending-unpair can be delivered if the peer is offline.
                        let (peer_addr, peer_name) = peers
                            .iter()
                            .find(|p| {
                                p.get("fingerprint")
                                    .and_then(|v| v.as_str())
                                    .map(|f| canonical_fingerprint(f) == fp_canonical)
                                    .unwrap_or(false)
                            })
                            .map(|p| {
                                (
                                    p.get("address")
                                        .and_then(|v| v.as_str())
                                        .map(|s| s.to_string()),
                                    p.get("name")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("")
                                        .to_string(),
                                )
                            })
                            .unwrap_or((None, String::new()));
                        peers.retain(|p| {
                            p.get("fingerprint")
                                .and_then(|v| v.as_str())
                                .map(|f| canonical_fingerprint(f) != fp_canonical)
                                .unwrap_or(true)
                        });
                        let removed = peers.len() < before_len;

                        match save_peers(&peers) {
                            Ok(_) => {
                                // AB-9: unpair must also evict the live in-memory
                                // mTLS allowlist (mirrors what revoke_peer does),
                                // otherwise an existing mTLS session survives until
                                // the next daemon restart. Normalise to canonical
                                // lowercase hex (strip colons) to match
                                // PairedPeers' key format.
                                if let Some(ref peers) = self.p2p_peers {
                                    peers.remove(&canonical_fingerprint(&fingerprint));
                                }
                                // Mutual unpair: best-effort signal the peer if
                                // it is currently connected over P2P.
                                send_unpair_signal_if_connected(
                                    &self.live_peer_sinks,
                                    &canonical_fingerprint(&fingerprint),
                                );
                                // Gap A: queue a DURABLE pending-unpair so the
                                // connector can deliver the Unpair frame on the
                                // peer's next reconnect even if it was offline now.
                                if removed {
                                    queue_unpair_for_offline_delivery(
                                        &fingerprint,
                                        peer_addr.as_deref(),
                                        &peer_name,
                                    );
                                }
                                Response::ok(
                                    req.id,
                                    serde_json::json!({ "ok": true, "removed": removed }),
                                )
                            }
                            Err(e) => Response::err(req.id, format!("failed to save peers: {e}")),
                        }
                    }
                    Err(e) => Response::err(req.id, format!("failed to load peers: {e}")),
                }
            }

            // T4 (v0.3) — manual peer revocation. Atomic with respect to the
            // user: a single click both (a) removes the peer from the local
            // JSON peer store so future sync attempts won't re-discover the
            // device by name, and (b) writes a row to the SQLite
            // `revoked_devices` audit table. The v1.0 cryptographic
            // revocation protocol will later consume that table to broadcast
            // revocation markers. For v0.3 the audit row is the only durable
            // record — mTLS rejection on unknown fingerprint is what blocks
            // the revoked peer from continuing to sync.
            "revoke_peer" => {
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

                // Capture the peer's display name *before* deleting so the
                // audit row preserves the human-readable label. Falls back
                // to an empty string if the peer wasn't in the store
                // (revoking an unknown fingerprint is allowed — useful when
                // the local peer list is out of sync with reality).
                let (removed, captured_name, captured_addr) = match load_peers() {
                    Ok(mut peers) => {
                        let before_len = peers.len();
                        let fp_canonical = canonical_fingerprint(&fingerprint);
                        let matched = peers.iter().find(|p| {
                            p.get("fingerprint")
                                .and_then(|v| v.as_str())
                                .map(|f| canonical_fingerprint(f) == fp_canonical)
                                .unwrap_or(false)
                        });
                        let name = matched
                            .and_then(|p| p.get("name"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        // Gap A: capture the last-known dial address before delete.
                        let addr = matched
                            .and_then(|p| p.get("address"))
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());

                        peers.retain(|p| {
                            p.get("fingerprint")
                                .and_then(|v| v.as_str())
                                .map(|f| canonical_fingerprint(f) != fp_canonical)
                                .unwrap_or(true)
                        });
                        if let Err(e) = save_peers(&peers) {
                            return Response::err(req.id, format!("failed to save peers: {e}"));
                        }
                        (peers.len() < before_len, name, addr)
                    }
                    Err(e) => return Response::err(req.id, format!("failed to load peers: {e}")),
                };

                // Write the audit row. Done on the blocking thread pool
                // because rusqlite is sync; the mutex is held only for the
                // duration of the two short statements inside
                // `revoke_device`.
                let db_arc = self.db.clone();
                let fp_for_db = fingerprint.clone();
                let name_for_db = captured_name.clone();
                let join = tokio::task::spawn_blocking(move || {
                    let db = db_arc.blocking_lock();
                    revoke_device(db.conn(), &fp_for_db, &name_for_db)
                })
                .await;

                match join {
                    Ok(Ok(revoked_at)) => {
                        // Fix CRITICAL #1: remove the peer from the live in-memory
                        // mTLS allowlist so the revoked peer's existing (or new)
                        // mTLS session is rejected immediately — without waiting
                        // for a daemon restart. Normalise to canonical lowercase
                        // hex (strip colons) to match PairedPeers' key format.
                        if let Some(ref peers) = self.p2p_peers {
                            peers.remove(&canonical_fingerprint(&fingerprint));
                        }
                        // Mutual unpair: best-effort signal the peer if it is
                        // currently connected over P2P.
                        send_unpair_signal_if_connected(
                            &self.live_peer_sinks,
                            &canonical_fingerprint(&fingerprint),
                        );
                        // Gap A: durable pending-unpair for offline delivery.
                        if removed {
                            queue_unpair_for_offline_delivery(
                                &fingerprint,
                                captured_addr.as_deref(),
                                &captured_name,
                            );
                        }
                        // FIX (CopyPaste-gbo): when cloud-sync or relay-sync is
                        // compiled in AND a sync key is currently installed,
                        // automatically rotate it to a fresh random key so the
                        // revoked device is ALSO cut off from cloud/relay sync —
                        // without requiring a passphrase from the user.
                        //
                        // Security rationale: the revoked device holds the OLD
                        // shared sync key and can use it to decrypt items
                        // encrypted under that key (XChaCha20-Poly1305 auth tags
                        // only reject ciphertexts produced under a DIFFERENT key).
                        // Rotating to a fresh random key means:
                        //   • all items produced AFTER revocation are encrypted
                        //     under the new key — the revoked device cannot
                        //     decrypt them (auth-tag rejection);
                        //   • the relay inbox id (HKDF of the sync key) diverges,
                        //     so the revoked device's inbox token is now stale.
                        //
                        // Distribution: remaining paired devices must re-provision
                        // (re-scan the pairing QR or accept the next P2P
                        // bootstrap push) to receive the new key.  This is the
                        // same requirement as `revoke_and_rotate`, but WITHOUT
                        // manual passphrase entry.
                        //
                        // When no sync key is currently installed (sync not yet
                        // configured), the rotation is skipped — there is nothing
                        // to rotate — and the response reflects that.
                        #[cfg(any(feature = "cloud-sync", feature = "relay-sync"))]
                        {
                            let key_was_active = self.sync_key.lock().await.is_some();
                            if key_was_active {
                                let new_key = SyncKey::random();
                                self.persist_and_install_sync_key(new_key).await;
                                tracing::info!(
                                    fingerprint = %fingerprint,
                                    "revoke_peer: P2P revoked + sync key auto-rotated (random); \
                                     remaining devices must re-provision to keep syncing",
                                );
                                Response::ok(
                                    req.id,
                                    serde_json::json!({
                                        "ok": true,
                                        "removed": removed,
                                        "revoked_at": revoked_at,
                                        "fingerprint": fingerprint,
                                        "sync_key_rotated": true,
                                    }),
                                )
                            } else {
                                // No sync key installed — P2P-only revocation is
                                // the complete action; nothing to rotate.
                                tracing::info!(
                                    fingerprint = %fingerprint,
                                    "revoke_peer: P2P-only revocation (no sync key installed); \
                                     cloud/relay sync was not active",
                                );
                                Response::ok(
                                    req.id,
                                    serde_json::json!({
                                        "ok": true,
                                        "removed": removed,
                                        "revoked_at": revoked_at,
                                        "fingerprint": fingerprint,
                                        "sync_key_rotated": false,
                                    }),
                                )
                            }
                        }
                        #[cfg(not(any(feature = "cloud-sync", feature = "relay-sync")))]
                        // P2P-only build: mTLS denylist is sufficient revocation.
                        Response::ok(
                            req.id,
                            serde_json::json!({
                                "ok": true,
                                "removed": removed,
                                "revoked_at": revoked_at,
                                "fingerprint": fingerprint,
                            }),
                        )
                    }
                    Ok(Err(e)) => Response::err_with_code(
                        req.id,
                        ERR_CODE_INTERNAL_ERROR,
                        format!("failed to record revocation: {e}"),
                    ),
                    Err(e) => Response::err_with_code(
                        req.id,
                        ERR_CODE_INTERNAL_ERROR,
                        format!("revoke task join error: {e}"),
                    ),
                }
            }

            // T5.x — revoke ALL paired peers in one call (Settings →
            // "Reset pairings"). Clears the local JSON peer store and writes
            // a `revoked_devices` audit row for each peer, reusing the same
            // single-peer `revoke_device` primitive. An empty store is a
            // success returning `{revoked: 0}` rather than an error.
            "revoke_all_peers" => {
                // Snapshot the current peers (fingerprint + display name)
                // before clearing the store so we can write audit rows.
                let peers = match load_peers() {
                    Ok(p) => p,
                    Err(e) => return Response::err(req.id, format!("failed to load peers: {e}")),
                };
                let captured: Vec<(String, String)> = peers
                    .iter()
                    .filter_map(|p| {
                        let fp = p.get("fingerprint").and_then(|v| v.as_str())?.to_string();
                        let name = p
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        Some((fp, name))
                    })
                    .collect();
                // Gap A: capture last-known dial addresses alongside fingerprints
                // so each revoked peer gets a durable pending-unpair record.
                let captured_addrs: Vec<Option<String>> = peers
                    .iter()
                    .filter_map(|p| {
                        p.get("fingerprint").and_then(|v| v.as_str())?;
                        Some(
                            p.get("address")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string()),
                        )
                    })
                    .collect();

                // Write every audit row in a single transaction FIRST, and only
                // clear the JSON peer store once that transaction has durably
                // committed. The previous order (clear store → loop inserting
                // audit rows, swallowing per-row errors) could leave the store
                // empty with audit rows missing on a partial failure, with the
                // loss only logged. With this order a failure leaves *both*
                // stores untouched so the caller can safely retry.
                let db_arc = self.db.clone();
                let captured_for_db = captured.clone();
                let join = tokio::task::spawn_blocking(move || {
                    let db = db_arc.blocking_lock();
                    revoke_devices(db.conn(), &captured_for_db)
                })
                .await;

                let revoked_at = match join {
                    Ok(Ok(ts)) => ts,
                    Ok(Err(e)) => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INTERNAL_ERROR,
                            format!("failed to record revocations: {e}"),
                        )
                    }
                    Err(e) => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INTERNAL_ERROR,
                            format!("revoke_all task join error: {e}"),
                        )
                    }
                };

                // Audit log committed — now clear the local peer store. If this
                // fails the audit rows are already durable (idempotent on a
                // retry via the UPSERT), so we surface the error rather than
                // silently leaving stale peers behind.
                if let Err(e) = save_peers(&[]) {
                    return Response::err_with_code(
                        req.id,
                        ERR_CODE_INTERNAL_ERROR,
                        format!("revocations recorded but failed to clear peers: {e}"),
                    );
                }

                // Fix CRITICAL #1: evict every revoked peer from the live mTLS
                // allowlist so their sessions are rejected immediately without
                // a daemon restart. Normalise each fingerprint to canonical
                // lowercase hex (strip colons) to match PairedPeers' key format.
                if let Some(ref peers) = self.p2p_peers {
                    for (fp, _) in &captured {
                        peers.remove(&canonical_fingerprint(fp));
                    }
                }

                // Mutual unpair: signal every currently-connected peer.
                for (fp, _) in &captured {
                    send_unpair_signal_if_connected(
                        &self.live_peer_sinks,
                        &canonical_fingerprint(fp),
                    );
                }

                // Gap A: durable pending-unpair for every revoked peer so the
                // signal reaches peers that were offline at reset time.
                for ((fp, name), addr) in captured.iter().zip(captured_addrs.iter()) {
                    queue_unpair_for_offline_delivery(fp, addr.as_deref(), name);
                }

                Response::ok(
                    req.id,
                    serde_json::json!({
                        "ok": true,
                        "revoked": captured.len(),
                        "cleared": captured.len(),
                        "revoked_at": revoked_at,
                    }),
                )
            }

            // W2.4 — PAKE-based password pairing (initiator side).
            //
            // Two-step protocol over IPC:
            //   step="initiate": validates inputs, creates PakeInitiator,
            //     stores session in pake_sessions, returns {session_id, message1_b64}.
            //   step="finish": looks up PakeInitiator by session_id, completes
            //     handshake with server's message2, stores peer, returns
            //     {ok: true, message3_b64}.
            "pair_peer_with_password" => {
                use base64::Engine as _;
                let b64 = base64::engine::general_purpose::STANDARD;

                let peer_fingerprint =
                    match req.params.get("peer_fingerprint").and_then(|v| v.as_str()) {
                        Some(s) => s.to_string(),
                        None => {
                            return Response::err_with_code(
                                req.id,
                                ERR_CODE_INVALID_ARGUMENT,
                                "missing peer_fingerprint",
                            )
                        }
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
                        let password = match req.params.get("password").and_then(|v| v.as_str()) {
                            Some(s) => s.to_string(),
                            None => {
                                return Response::err_with_code(
                                    req.id,
                                    ERR_CODE_INVALID_ARGUMENT,
                                    "missing password",
                                )
                            }
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
                        let session_id = match req.params.get("session_id").and_then(|v| v.as_str())
                        {
                            Some(s) => s.to_string(),
                            None => {
                                return Response::err_with_code(
                                    req.id,
                                    ERR_CODE_INVALID_ARGUMENT,
                                    "missing session_id for step=finish",
                                )
                            }
                        };
                        let msg2_b64 = match req.params.get("message2_b64").and_then(|v| v.as_str())
                        {
                            Some(s) => s.to_string(),
                            None => {
                                return Response::err_with_code(
                                    req.id,
                                    ERR_CODE_INVALID_ARGUMENT,
                                    "missing message2_b64 for step=finish",
                                )
                            }
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
                        let initiator_tag =
                            channel_confirmation_tag(&bound_key, ConfirmRole::Initiator);
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
                                        return Response::err(
                                            req.id,
                                            format!("failed to save peers: {e}"),
                                        );
                                    }
                                }
                            }
                            Err(e) => {
                                return Response::err(req.id, format!("failed to load peers: {e}"))
                            }
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

            // c4q2.20: pair_accept_password (password-based PAKE responder) is
            // stubbed not_implemented. The password-pairing flow was removed as a
            // security concern (CopyPaste-c4q2.20) — use QR pairing
            // (pair_generate_qr / pair_accept_qr) instead.
            "pair_accept_password" => Response::err_with_code(
                req.id,
                ERR_CODE_NOT_IMPLEMENTED,
                "pair_accept_password is disabled — use QR pairing (pair_generate_qr / pair_accept_qr) (c4q2.20)",
            ),

            // W2.4 — PAKE responder finish: receives message3 from initiator,
            // completes handshake, persists peer + PasswordFile.
            // Params: {session_id, message3_b64, peer_fingerprint}
            // Response: {ok: true}
            "pair_accept_finish" => {
                use base64::Engine as _;
                let b64 = base64::engine::general_purpose::STANDARD;

                let session_id = match req.params.get("session_id").and_then(|v| v.as_str()) {
                    Some(s) => s.to_string(),
                    None => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INVALID_ARGUMENT,
                            "missing session_id",
                        )
                    }
                };
                let msg3_b64 = match req.params.get("message3_b64").and_then(|v| v.as_str()) {
                    Some(s) => s.to_string(),
                    None => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INVALID_ARGUMENT,
                            "missing message3_b64",
                        )
                    }
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
                let password_file_enc = match encrypt_pake_password_file(
                    &password_file.serialized,
                    &fp_c,
                    &self.local_key,
                ) {
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

            // ----------------------------------------------------------------
            // QR pairing — displaying side. Generate a fresh pairing token,
            // store it for the matching `pair_accept_qr` step, and return a
            // single-line QR payload (the `copypaste-core::PairingPayload`
            // wire form) the *other* device scans. The token is the PAKE
            // password; the scanner derives it from the QR and drives the
            // existing `pair_peer_with_password` initiator flow. No new crypto:
            // QR is purely a transport for the token + this device's
            // fingerprint. See `copypaste_core::crypto::pairing_qr`.
            //
            // Request params: {} (device identity is taken from daemon state).
            // Response data: { "qr": "CPPAIR2...", "expires_in_secs": <u64> }
            // ----------------------------------------------------------------
            "pair_generate_qr" => {
                // CRITICAL-1 fix: the QR must carry the live mTLS **certificate**
                // fingerprint (the value the scanner pins and the mTLS verifier
                // compares — `PeerTransport::fingerprint` / `fingerprint_of`),
                // NOT the device-key fingerprint (`keychain::own_fingerprint`).
                // The QR payload already documents this field as the cert
                // fingerprint (see `copypaste_core::crypto::pairing_qr`), so the
                // payload format/version is unchanged — only the value sourced
                // here was wrong, making cert-pinning unable to ever match.
                //
                // No cert exists when P2P is disabled; refuse rather than
                // advertise a fingerprint that cannot authenticate the channel.
                let fingerprint = match self.cert_fingerprint.as_ref() {
                    Some(fp) => fp.clone(),
                    None => {
                        return Response::err(
                            req.id,
                            "P2P is disabled (set COPYPASTE_P2P=1): cannot generate a \
                             pairing QR without an mTLS certificate to advertise",
                        )
                    }
                };

                // Device name mirrors the P2P subsystem's source (HOSTNAME /
                // COMPUTERNAME, falling back to "CopyPaste") so the scanning
                // device shows a consistent label.
                let device_name = crate::daemon::resolve_device_name();

                // device_id must be a valid UUID: CPPAIR2 encodes it as 16 raw
                // bytes (base64url), and the decoder rejects any other length.
                // Use the stable daemon UUID when available; fall back to a
                // fresh v4 UUID (informational only — peer pinning uses the
                // fingerprint, not device_id).
                let device_id = self
                    .local_device_id
                    .clone()
                    .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

                // Generate the single-use pairing token up front so the same
                // value feeds (a) the QR the scanner reads, (b) the legacy IPC
                // PAKE path's stored token, and (c) the bootstrap responder's
                // PAKE password — all derived from one token.
                let token = copypaste_core::PairingToken::generate();
                let password = token.to_pake_password();

                // P2P Phase 1: spawn an ephemeral, *unauthenticated* bootstrap
                // TLS listener and advertise its `host:port` in the QR's
                // `addr_hint`. The initiator dials it and the responder side of
                // the PAKE handshake runs over that TLS stream (PAKE provides
                // the mutual auth from the shared QR secret; the channel is
                // unpinned because neither side knows the other's cert yet).
                //
                // When P2P is disabled / the cert is absent we leave `addr_hint`
                // empty and fall back to the legacy IPC-relayed PAKE path.
                let addr_hint = if let Some(cert) = self.p2p_cert.clone() {
                    let (cert_der, key_der) = (cert.0.clone(), cert.1.clone());
                    match copypaste_p2p::bootstrap::BootstrapResponder::bind(cert_der, key_der)
                        .await
                    {
                        Ok(responder) => match responder.local_addr() {
                            Ok(local) => {
                                // The listener binds 0.0.0.0, so it's reachable on
                                // every interface — but the QR must carry one
                                // concrete host. A loopback hint (127.0.0.1) is
                                // unreachable from another device/emulator, so we
                                // advertise a real LAN-routable host via the shared
                                // `advertise_sync_addr` policy (same selection the
                                // in-band sync-listener address uses), falling back
                                // to 127.0.0.1 only when no LAN interface exists so
                                // same-host (and loopback-test) pairing still works.
                                let hint =
                                    copypaste_p2p::interfaces::advertise_sync_addr(local.port())
                                        .to_string();
                                // Race-fix (CopyPaste-7mf): store the handle so
                                // `list_peers` can await it before reading peers.json.
                                let handle =
                                    self.spawn_bootstrap_responder(responder, password.clone());
                                *self.pending_bootstrap.lock().await = Some(handle);
                                hint
                            }
                            Err(e) => {
                                tracing::warn!(
                                    "bootstrap listener local_addr failed ({e}); \
                                     falling back to mDNS-only addr_hint"
                                );
                                String::new()
                            }
                        },
                        Err(e) => {
                            tracing::warn!(
                                "bootstrap listener bind failed ({e}); \
                                 falling back to mDNS-only addr_hint"
                            );
                            String::new()
                        }
                    }
                } else {
                    String::new()
                };

                // H4: embed relay + Supabase config into the QR as the optional
                // 6th provisioning field so the scanning device (Android) can
                // configure cloud/relay sync automatically at scan time — before
                // the P2P bootstrap tunnel is established (covers off-LAN case
                // where the P2P handshake may not complete).
                //
                // These are all non-secret values: relay_url is a plain HTTP
                // base URL; supabase_url + supabase_anon_key are the publishable
                // Supabase connection params, intentionally public per Supabase
                // documentation. No long-term secrets are embedded in the QR.
                let qr_provisioning = {
                    let app_cfg = read_config();
                    let relay_url = app_cfg.relay_url.clone();
                    let supabase_url = std::env::var("SUPABASE_URL").ok().or(app_cfg.supabase_url);
                    let supabase_anon_key = std::env::var("SUPABASE_ANON_KEY")
                        .ok()
                        .or(app_cfg.supabase_anon_key);
                    let prov = copypaste_core::QrProvisioning {
                        relay_url,
                        supabase_url,
                        supabase_anon_key,
                    };
                    if prov.is_empty() {
                        None
                    } else {
                        Some(prov)
                    }
                };

                // Build the payload directly from the pre-generated token so the
                // QR, the stored token, and the bootstrap password all agree.
                let payload = copypaste_core::PairingPayload {
                    fingerprint,
                    token,
                    device_id,
                    device_name,
                    addr_hint,
                    provisioning: qr_provisioning,
                };

                // Wrap the bare CPPAIR2 payload in the cppair://pair?p= deep-link
                // URI so external scanners (Google Lens, the system camera) treat
                // the QR as an actionable link and offer "open in app". The
                // in-app scanner and Android manifest deep-link both strip the
                // wrapper before decoding (see copypaste_core::strip_deeplink).
                let qr = payload.encode_deeplink();

                // Store the token (replacing any prior active QR) so the legacy
                // IPC `pair_accept_qr` path can re-derive the same PAKE password.
                {
                    let mut slot = self.pending_qr_token.lock().await;
                    *slot = Some((payload.token, std::time::Instant::now()));
                }

                Response::ok(
                    req.id,
                    serde_json::json!({
                        "qr": qr,
                        "expires_in_secs": PAKE_SESSION_TTL.as_secs(),
                    }),
                )
            }

            // ----------------------------------------------------------------
            // QR pairing — displaying side, accept step. The scanning device
            // (initiator) has derived the PAKE password from the QR token and
            // sent `message1`. We look up the stored token, re-derive the same
            // password, register a PasswordFile and respond exactly as
            // `pair_accept_password` does — but without the user typing the
            // password (it came from the QR we generated). The follow-up
            // `pair_accept_finish` step is unchanged.
            //
            // Request params: { "message1_b64", "peer_fingerprint" }
            // Response data:  { "session_id", "message2_b64" }
            // ----------------------------------------------------------------
            "pair_accept_qr" => {
                use base64::Engine as _;
                let b64 = base64::engine::general_purpose::STANDARD;

                // ── P2P Phase 1: network bootstrap path ─────────────────────
                // When the caller supplies the scanned `qr` string (rather than
                // a relayed `message1_b64`), this daemon is the *initiator*: it
                // decodes the QR, dials the responder's `addr_hint` over the
                // unauthenticated bootstrap TLS channel, and runs the full PAKE
                // initiator handshake over the network. PAKE provides mutual auth
                // from the shared QR secret; the channel is unpinned. On success
                // the responder's cert fingerprint (learned over the channel) is
                // registered in the live mTLS allowlist.
                if let Some(qr) = req.params.get("qr").and_then(|v| v.as_str()) {
                    let qr = qr.to_string();
                    return self.pair_accept_qr_network(req.id.clone(), &qr).await;
                }

                let message1_b64 = match req.params.get("message1_b64").and_then(|v| v.as_str()) {
                    Some(s) => s.to_string(),
                    None => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INVALID_ARGUMENT,
                            "missing message1_b64",
                        )
                    }
                };
                let peer_fingerprint =
                    match req.params.get("peer_fingerprint").and_then(|v| v.as_str()) {
                        Some(s) => s.to_string(),
                        None => {
                            return Response::err_with_code(
                                req.id,
                                ERR_CODE_INVALID_ARGUMENT,
                                "missing peer_fingerprint",
                            )
                        }
                    };

                if !is_valid_fingerprint(&peer_fingerprint) {
                    return Response::err_with_code(
                        req.id,
                        ERR_CODE_INVALID_ARGUMENT,
                        format!("invalid peer_fingerprint format: {peer_fingerprint}"),
                    );
                }

                // Retrieve the active QR token, enforcing the TTL. Take it out
                // so a stale/expired token cannot linger.
                let password = {
                    let mut slot = self.pending_qr_token.lock().await;
                    match slot.take() {
                        Some((token, issued)) if issued.elapsed() < PAKE_SESSION_TTL => {
                            token.to_pake_password()
                        }
                        Some(_) => {
                            return Response::err_with_code(
                                req.id,
                                ERR_CODE_INVALID_ARGUMENT,
                                "QR pairing token expired; regenerate the code",
                            )
                        }
                        None => {
                            return Response::err_with_code(
                                req.id,
                                ERR_CODE_INVALID_ARGUMENT,
                                "no active QR pairing token; generate a code first",
                            )
                        }
                    }
                };

                let msg1_bytes = match b64.decode(&message1_b64) {
                    Ok(b) => b,
                    Err(e) => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INVALID_ARGUMENT,
                            format!("invalid base64 in message1_b64: {e}"),
                        )
                    }
                };

                let password_file = match copypaste_p2p::pake::PasswordFile::register(&password) {
                    Ok(pf) => pf,
                    Err(e) => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INTERNAL_ERROR,
                            format!("PasswordFile::register failed: {e}"),
                        )
                    }
                };

                let (responder, msg2_bytes) =
                    match PakeResponder::respond(&password_file, &msg1_bytes) {
                        Ok(pair) => pair,
                        Err(e) => {
                            return Response::err_with_code(
                                req.id,
                                ERR_CODE_AUTH_FAILED,
                                format!("PAKE respond failed: {e}"),
                            )
                        }
                    };

                let session_id = uuid::Uuid::new_v4().to_string();
                let msg2_b64 = b64.encode(&msg2_bytes);

                if let Err(msg) = self
                    .insert_pake_session(
                        session_id.clone(),
                        PakeSession::Responder {
                            responder: Box::new(responder),
                            password_file,
                            peer_fingerprint,
                        },
                    )
                    .await
                {
                    return Response::err_with_code(req.id, ERR_CODE_INTERNAL_ERROR, msg);
                }

                Response::ok(
                    req.id,
                    serde_json::json!({
                        "session_id": session_id,
                        "message2_b64": msg2_b64,
                    }),
                )
            }

            _ => self.dispatch_transfer(req).await,
        }
    }
}
