//! QR / network bootstrap pairing flows (split from pairing_ops.rs, ADR-017
//! daemon-ipc track, CopyPaste-vp63.17). Split further from
//! pairing_ops_flows.rs (over the 500-line ceiling) into discovered vs QR.
use super::*;

impl IpcServer {
    /// Spawn the responder side of the P2P Phase 1 bootstrap PAKE handshake.
    ///
    /// The `responder` already owns the bound, TLS-wrapped ephemeral listener
    /// whose address was advertised in the QR's `addr_hint`. This accepts ONE
    /// inbound connection within the pairing window and runs the PAKE responder
    /// over the TLS stream. On success the peer's cert fingerprint (learned over
    /// the same channel) is registered in the live mTLS allowlist so subsequent
    /// pinned mTLS sessions are accepted without a daemon restart.
    ///
    /// Runs detached: pairing is driven by the scanning device dialling in, so
    /// there is nothing for the IPC caller to await here. PAKE failure (wrong
    /// token, MitM, timeout) only logs — no peer is registered.
    ///
    /// Race-fix (CopyPaste-7mf): returns the `JoinHandle` so the caller can store
    /// it in `self.pending_bootstrap`. `list_peers` awaits that handle (with a
    /// short timeout) before reading `peers.json`, ensuring that a
    /// `pair_generate_qr` → (initiator scans) → `list_peers` sequence on the
    /// responder side always sees the freshly-persisted peer.
    ///
    /// Empty-address fix: `own_sync_addr` is now read from the slot INSIDE the
    /// spawned task, after `DeviceMeta::collect` completes but before
    /// `responder.run()`. This gives the P2P subsystem maximum time to bind its
    /// listener and populate the slot (it does so on startup, before any pairing
    /// request arrives in practice). If the slot is still empty at that point the
    /// record stores `address: null` and the connector falls back to mDNS — the
    /// same graceful degradation as before, but without over-capturing a stale
    /// empty string from before the P2P listener was ready.
    pub(crate) fn spawn_bootstrap_responder(
        &self,
        responder: copypaste_p2p::bootstrap::BootstrapResponder,
        password: String,
    ) -> tokio::task::JoinHandle<()> {
        let peers = self.p2p_peers.clone();
        // Clone the addr slot Arc so the task can read it after device metadata
        // is collected — giving the P2P listener maximum time to populate it.
        // (Empty-address fix: previously own_sync_addr() was called here, before
        // the async work inside the task, so a racing listener start would still
        // produce an empty address. Reading from the Arc inside the task is later
        // and avoids that window.)
        let own_sync_addr_slot = self.p2p_sync_addr.clone();
        // B1: clone the public-IP cache Arc before the move so the detached task
        // can read our current STUN-discovered global IP to advertise in-band.
        let public_ip_cache = self.cached_public_ip.clone();
        // "QR fully provisions all sync": clone the sync_key Arc so the detached
        // task can BUILD our provisioning to advertise and APPLY the peer's.
        #[cfg(feature = "cloud-sync")]
        let sync_key = self.sync_key.clone();
        // H8: clone before the move so the spawned task can call reload_sync_key
        // after persist_paired_peer writes peers.json.
        let spawn_sync_crypto = self.p2p_sync_crypto.clone();
        let own_device_id = self.local_device_id.clone();
        // CopyPaste-yw2k: clone the account-id Arc before the move so the
        // spawned task can read the non-secret identity to advertise in-band.
        let cloud_account_id_arc = self.cloud_account_id.clone();
        tokio::spawn(async move {
            // CopyPaste-yw2k: read the non-secret local Supabase account id
            // inside the task (after the Arc was cloned before the spawn).
            let own_supabase_account_id: Option<String> = cloud_account_id_arc
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .clone();
            // P2P Phase 4: collect our own device metadata to advertise in-band.
            // DeviceMeta::collect spawns child processes (up to ~2 s), so run it
            // off the async worker. Falls back to empty metadata on join error.
            let own_public_ip = public_ip_cache.read().await.clone();
            let own_meta = tokio::task::spawn_blocking(move || {
                Self::collect_own_peer_meta(own_public_ip, own_device_id, own_supabase_account_id)
            })
            .await
            .unwrap_or_default();
            // Wait (bounded) for the P2P listener to populate the slot so the
            // responder never advertises an empty address (mirrors the initiator's
            // `await_own_sync_addr`; fixes flaky pairing_persists_..._on_both_sides
            // on slow CI runners). Resolves immediately once bound; on timeout
            // returns the still-empty string (graceful mDNS fallback).
            let own_sync_addr = {
                let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
                loop {
                    let addr = own_sync_addr_slot
                        .lock()
                        .map(|slot| slot.clone().unwrap_or_default())
                        .unwrap_or_else(|p| p.into_inner().clone().unwrap_or_default());
                    if !addr.is_empty() || tokio::time::Instant::now() >= deadline {
                        break addr;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(25)).await;
                }
            };
            // Build our SyncProvisioning to advertise (None without cloud-sync).
            #[cfg(feature = "cloud-sync")]
            let own_provisioning = Self::build_local_provisioning_from(&sync_key).await;
            #[cfg(not(feature = "cloud-sync"))]
            let own_provisioning: Option<copypaste_p2p::bootstrap::SyncProvisioning> = None;
            match responder
                .run(&password, &own_sync_addr, &own_meta, own_provisioning)
                .await
            {
                Ok(outcome) => {
                    tracing::info!(
                        peer_fingerprint = %outcome.peer_fingerprint,
                        peer_sync_addr = %outcome.peer_sync_addr,
                        "bootstrap PAKE responder completed over network channel"
                    );
                    // Register the freshly-paired peer in the live allowlist.
                    // The bootstrap channel reports the canonical (colon-free)
                    // hex fingerprint; `rotate_peer` upserts it as active.
                    if let Some(peers) = peers {
                        peers.rotate_peer(
                            &outcome.peer_fingerprint,
                            outcome.peer_fingerprint.to_string(),
                            String::new(),
                        );
                    }
                    // P2P Phase 2: durably persist the peer (fingerprint +
                    // sync-listener address) so it survives a restart and the
                    // Phase 3 connector can dial it directly. Phase 4: also
                    // persist the peer's advertised device metadata.
                    let peer_meta = copypaste_p2p::bootstrap::PeerMeta {
                        model: outcome.peer_model.clone(),
                        os_version: outcome.peer_os.clone(),
                        app_version: outcome.peer_app_version.clone(),
                        local_ip: outcome.peer_local_ip.clone(),
                        device_name: outcome.peer_device_name.clone(),
                        public_ip: outcome.peer_public_ip.clone(),
                        device_id: outcome.peer_device_id.clone(),
                        // CopyPaste-yw2k: carry the peer's non-secret account identity.
                        supabase_account_id: outcome.peer_supabase_account_id.clone(),
                    };
                    // Persist is the last observable side-effect of the bootstrap
                    // task. `list_peers` awaits `pending_bootstrap` (stored by
                    // `pair_generate_qr`) before reading peers.json, so callers
                    // see a consistent view once this JoinHandle completes.
                    Self::persist_paired_peer(
                        &outcome.peer_fingerprint,
                        &outcome.peer_sync_addr,
                        &outcome.session_key,
                        &peer_meta,
                        spawn_sync_crypto.as_ref(),
                    )
                    .await;
                    // "QR fully provisions all sync": apply any sync config the
                    // scanning peer advertised that we currently lack.
                    #[cfg(feature = "cloud-sync")]
                    if let Some(prov) = outcome.peer_provisioning {
                        Self::apply_peer_provisioning_to(&sync_key, prov).await;
                    }
                }
                Err(e) => {
                    tracing::warn!("bootstrap PAKE responder failed: {e}");
                }
            }
        })
    }

    /// Initiator side of the P2P Phase 1 network pairing flow.
    ///
    /// Decodes the scanned `qr`, derives the PAKE password from its token,
    /// resolves the responder's `host:port` (QR `addr_hint` primary; mDNS
    /// `resolve_peer` fallback), dials the unauthenticated bootstrap TLS channel,
    /// and runs the PAKE initiator over it. On success the responder's cert
    /// fingerprint is registered in the live mTLS allowlist.
    ///
    /// Returns the IPC `Response` directly (this is the whole handler for the
    /// network branch of `pair_accept_qr`).
    pub(crate) async fn pair_accept_qr_network(&self, req_id: String, qr: &str) -> Response {
        // We must have our own cert to present on the bootstrap channel so the
        // responder learns the fingerprint it will later pin.
        let cert = match self.p2p_cert.as_ref() {
            Some(c) => Arc::clone(c),
            None => {
                return Response::err_with_code(
                    req_id,
                    ERR_CODE_INVALID_ARGUMENT,
                    "P2P is disabled (set COPYPASTE_P2P=1): cannot accept a pairing QR \
                     over the network without an mTLS certificate",
                )
            }
        };

        // Accept both the wrapped cppair://pair?p=… deep-link form (emitted by
        // pair_generate_qr / Android for external scanners) and a bare CPPAIR2
        // string (back-compat). strip_deeplink is a no-op on the bare form.
        let bare = copypaste_core::strip_deeplink(qr);
        let payload = match copypaste_core::PairingPayload::decode(&bare) {
            Ok(p) => p,
            Err(e) => {
                return Response::err_with_code(
                    req_id,
                    ERR_CODE_INVALID_ARGUMENT,
                    format!("failed to decode pairing QR: {e}"),
                )
            }
        };

        let password = payload.token.to_pake_password();

        // Resolve the responder's address: addr_hint is primary; fall back to
        // mDNS resolution by device_id when it is empty (best-effort — loopback
        // mDNS is unreliable, see discovery::resolve_peer).
        let addr = match self.resolve_pairing_addr(&payload) {
            Ok(addr) => addr,
            Err(msg) => return Response::err_with_code(req_id, ERR_CODE_INVALID_ARGUMENT, msg),
        };

        let (cert_der, key_der) = (cert.0.clone(), cert.1.clone());
        // B1: our own STUN-discovered global IP, advertised in-band so the peer
        // can show it. None if unresolved/disabled.
        let own_public_ip = self.cached_public_ip.read().await.clone();
        let own_device_id = self.local_device_id.clone();
        // CopyPaste-yw2k: read the non-secret local Supabase account id to
        // advertise in-band so the peer can detect cross-account mismatches.
        let own_supabase_account_id: Option<String> = self
            .cloud_account_id
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        // P2P Phase 4: collect our own device metadata to advertise in-band.
        // DeviceMeta::collect spawns child processes (up to ~2 s), so run it off
        // the async worker; empty metadata on join error.
        let own_meta = tokio::task::spawn_blocking(move || {
            Self::collect_own_peer_meta(own_public_ip, own_device_id, own_supabase_account_id)
        })
        .await
        .unwrap_or_default();
        // "QR fully provisions all sync": advertise our Supabase/relay config +
        // derived sync key over the authenticated tunnel (None if unconfigured).
        let own_provisioning = self.build_local_provisioning().await;
        // Our own P2P sync-listener address, sent in-band so the responder can
        // persist it for its Phase 3 connector. Wait (bounded) for the listener
        // to bind so we never advertise an empty address when pairing is
        // initiated right after startup (flaky `pairing_persists_..._on_both_sides`:
        // A recorded B with no address). Resolves immediately once bound.
        let own_sync_addr = self
            .await_own_sync_addr(std::time::Duration::from_secs(5))
            .await;
        match copypaste_p2p::bootstrap::run_initiator(
            addr,
            cert_der,
            key_der,
            &password,
            &own_sync_addr,
            &own_meta,
            own_provisioning,
        )
        .await
        {
            Ok(outcome) => {
                tracing::info!(
                    peer_fingerprint = %outcome.peer_fingerprint,
                    peer_sync_addr = %outcome.peer_sync_addr,
                    "bootstrap PAKE initiator completed over network channel"
                );
                if let Some(ref peers) = self.p2p_peers {
                    peers.rotate_peer(
                        &outcome.peer_fingerprint,
                        outcome.peer_fingerprint.to_string(),
                        String::new(),
                    );
                }
                // P2P Phase 2: durably persist the peer (fingerprint + the
                // sync-listener address it advertised) for restart-survival and
                // the Phase 3 outbound connector. Phase 4: also persist the
                // peer's advertised device metadata.
                let peer_meta = copypaste_p2p::bootstrap::PeerMeta {
                    model: outcome.peer_model.clone(),
                    os_version: outcome.peer_os.clone(),
                    app_version: outcome.peer_app_version.clone(),
                    local_ip: outcome.peer_local_ip.clone(),
                    device_name: outcome.peer_device_name.clone(),
                    public_ip: outcome.peer_public_ip.clone(),
                    device_id: outcome.peer_device_id.clone(),
                    // CopyPaste-yw2k: carry the peer's non-secret account identity.
                    supabase_account_id: outcome.peer_supabase_account_id.clone(),
                };
                Self::persist_paired_peer(
                    &outcome.peer_fingerprint,
                    &outcome.peer_sync_addr,
                    &outcome.session_key,
                    &peer_meta,
                    self.p2p_sync_crypto.as_ref(),
                )
                .await;
                // "QR fully provisions all sync": apply any sync config the
                // responder advertised that we currently lack.
                if let Some(prov) = outcome.peer_provisioning {
                    self.apply_peer_provisioning(prov).await;
                }
                Response::ok(
                    req_id,
                    serde_json::json!({
                        "ok": true,
                        "peer_fingerprint": outcome.peer_fingerprint.to_string(),
                    }),
                )
            }
            Err(e) => Response::err_with_code(
                req_id,
                ERR_CODE_AUTH_FAILED,
                format!("network PAKE pairing failed: {e}"),
            ),
        }
    }

    /// Resolve the responder's socket address for the initiator bootstrap dial.
    ///
    /// Uses the QR `addr_hint` when present; otherwise falls back to mDNS
    /// `resolve_peer` keyed by the QR's `device_id`. Returns a human-readable
    /// error string when neither yields a usable address.
    pub(crate) fn resolve_pairing_addr(
        &self,
        payload: &copypaste_core::PairingPayload,
    ) -> Result<std::net::SocketAddr, String> {
        if !payload.addr_hint.is_empty() {
            return payload
                .addr_hint
                .parse::<std::net::SocketAddr>()
                .map_err(|e| format!("invalid addr_hint '{}': {e}", payload.addr_hint));
        }

        // mDNS fallback (best-effort).
        let discovery = self
            .discovery
            .as_ref()
            .ok_or_else(|| "QR has no addr_hint and mDNS discovery is unavailable".to_string())?;
        let peer = discovery
            .resolve_peer(&payload.device_id)
            .ok_or_else(|| "QR has no addr_hint and the peer was not found via mDNS".to_string())?;
        let ip = peer
            .ip_addrs
            .first()
            .ok_or_else(|| "mDNS-resolved peer has no IP address".to_string())?;
        Ok(std::net::SocketAddr::new(*ip, peer.port))
    }
}
