//! Unpair / revoke IPC verbs (split from handlers_pairing.rs, ADR-017
//! daemon-ipc track, CopyPaste-vp63.16).
use super::*;

impl IpcServer {
    /// CopyPaste-3n9h: `pair_peer` previously trusted a peer and
    /// registered it in the live mTLS allowlist WITHOUT any
    /// authentication (no PAKE, no SAS). A caller that knew a peer's
    /// TLS fingerprint could add it as trusted with no proof of identity.
    ///
    /// The unauthenticated path is now DISABLED. All pairing MUST go
    /// through the authenticated paths:
    ///   • QR / password: `pair_peer_with_password` + `pair_accept_finish`
    ///   • LAN/SAS discovery: `pair_with_discovered` + `pair_confirm_sas`
    ///
    /// This handler is retained (not removed) so old CLI versions
    /// receive an explicit error instead of "unknown method", which
    /// makes the upgrade path diagnosable.
    pub(crate) async fn handle_pair_peer(&self, req: Request) -> Response {
        Response::err_with_code(
            req.id,
            ERR_CODE_NOT_IMPLEMENTED,
            "pair_peer is disabled: use pair_peer_with_password (QR/password) \
             or pair_with_discovered (LAN/SAS) for authenticated pairing",
        )
    }

    pub(crate) async fn handle_unpair_peer(&self, req: Request) -> Response {
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

    /// T4 (v0.3) — manual peer revocation. Atomic with respect to the
    /// user: a single click both (a) removes the peer from the local
    /// JSON peer store so future sync attempts won't re-discover the
    /// device by name, and (b) writes a row to the SQLite
    /// `revoked_devices` audit table. The v1.0 cryptographic
    /// revocation protocol will later consume that table to broadcast
    /// revocation markers. For v0.3 the audit row is the only durable
    /// record — mTLS rejection on unknown fingerprint is what blocks
    /// the revoked peer from continuing to sync.
    pub(crate) async fn handle_revoke_peer(&self, req: Request) -> Response {
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

    /// T5.x — revoke ALL paired peers in one call (Settings →
    /// "Reset pairings"). Clears the local JSON peer store and writes
    /// a `revoked_devices` audit row for each peer, reusing the same
    /// single-peer `revoke_device` primitive. An empty store is a
    /// success returning `{revoked: 0}` rather than an error.
    pub(crate) async fn handle_revoke_all_peers(&self, req: Request) -> Response {
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
            send_unpair_signal_if_connected(&self.live_peer_sinks, &canonical_fingerprint(fp));
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
}
