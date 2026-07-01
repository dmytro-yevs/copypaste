//! Discovered-peer (mDNS/SAS) pairing flow (split from pairing_ops.rs,
//! ADR-017 daemon-ipc track, CopyPaste-vp63.17). Split further from
//! pairing_ops_flows.rs (over the 500-line ceiling) into discovered vs QR.
use super::*;

impl IpcServer {
    /// LAN/SAS Phase 2 — INITIATOR side of discovery-initiated SAS pairing.
    ///
    /// Resolves the discovered peer (`device_id`) to its bootstrap socket
    /// address via the shared [`DiscoveryService`](copypaste_p2p::discovery::DiscoveryService)
    /// (using the v2 `bport` TXT key), generates an EPHEMERAL random PAKE
    /// password, and runs [`run_initiator_with_confirm`](copypaste_p2p::bootstrap::run_initiator_with_confirm).
    ///
    /// ## Why an in-clear ephemeral password is safe here
    /// The discovery path has NO pre-shared secret, so the bootstrap TLS channel
    /// is run with a throwaway random password. Authentication is provided
    /// ENTIRELY by the human SAS comparison: the SAS is derived from the
    /// post-PAKE, post-channel-binding `bound_key`, so a man-in-the-middle that
    /// substitutes its own password per leg produces a DIFFERENT SAS per leg and
    /// the two users see mismatched codes. Both sides must ACCEPT (frame 10a)
    /// before any key is trusted; on reject/abort/timeout the session key is
    /// dropped/zeroized and NOTHING is persisted (no `rotate_peer`).
    ///
    /// The `confirm` callback transitions the state machine to `awaiting_sas`
    /// and awaits the `oneshot` that `pair_confirm_sas`/`pair_abort` fire. On a
    /// both-accept success this reuses the SAME `rotate_peer` +
    /// `persist_paired_peer` as the QR path so the steady-state link is
    /// identical (mutual fingerprint-pinned mTLS).
    pub(crate) async fn pair_with_discovered(&self, req_id: String, device_id: &str) -> Response {
        let cert = match self.p2p_cert.as_ref() {
            Some(c) => Arc::clone(c),
            None => {
                return Response::err_with_code(
                    req_id,
                    ERR_CODE_INVALID_ARGUMENT,
                    "P2P is disabled (set COPYPASTE_P2P=1): cannot pair over the network",
                )
            }
        };
        let discovery = match self.discovery.as_ref() {
            Some(d) => d,
            None => {
                return Response::err_with_code(
                    req_id,
                    ERR_CODE_INVALID_ARGUMENT,
                    "discovery not available (P2P disabled)",
                )
            }
        };

        // Resolve the peer's bootstrap listener address from the live snapshot.
        let peer = match discovery.resolve_peer(device_id) {
            Some(p) => p,
            None => {
                return Response::err_with_code(
                    req_id,
                    ERR_CODE_NOT_FOUND,
                    format!("device not currently discoverable: {device_id}"),
                )
            }
        };
        let bport =
            match peer.bport {
                Some(p) => p,
                None => return Response::err_with_code(
                    req_id,
                    ERR_CODE_INVALID_ARGUMENT,
                    "peer does not advertise a bootstrap port (v1 peer): SAS pairing unsupported",
                ),
            };
        // Prefer an IPv4 address (broadest compatibility); fall back to the
        // first address of any family. `ip_addrs` is sorted IPv4-first.
        let ip = match peer
            .ip_addrs
            .iter()
            .find(|a| a.is_ipv4())
            .or_else(|| peer.ip_addrs.first())
        {
            Some(ip) => *ip,
            None => {
                return Response::err_with_code(
                    req_id,
                    ERR_CODE_NOT_FOUND,
                    "peer has no resolved IP address",
                )
            }
        };
        let addr = std::net::SocketAddr::new(ip, bport);

        // Build the peer snapshot from the mDNS PeerInfo resolved above.
        // This is available immediately (pre-handshake) and is the richest
        // source of peer identity data at `pair_get_sas` poll time. The PAKE
        // metadata exchange (model/OS/version) happens AFTER the SAS confirm
        // step and is surfaced in the final `pair_with_discovered` response.
        let peer_snapshot = crate::pairing_sm::PeerSnapshot {
            device_name: if peer.device_name.is_empty() {
                None
            } else {
                Some(peer.device_name.clone())
            },
            ip_addrs: peer.ip_addrs.iter().map(|a| a.to_string()).collect(),
            // device_id IS the cert fingerprint (hex SHA-256); use it directly
            // so the UI can show the fingerprint before the TLS handshake.
            fingerprint: if peer.device_id.is_empty() {
                None
            } else {
                Some(peer.device_id.clone())
            },
        };

        // Claim the single-active-pairing slot. A concurrent request is rejected
        // with a rate-limited error (one pairing at a time, v0.6 simplicity).
        if !self.pairing.try_begin(
            crate::pairing_sm::PairingRole::Initiator,
            peer_snapshot.clone(),
        ) {
            return Response::err_with_code(
                req_id,
                ERR_CODE_RATE_LIMITED,
                "another pairing is already in progress",
            );
        }

        // Discovery (QR-less) path: a FIXED, well-known, NON-SECRET PAKE password
        // shared by every initiator/responder. opaque-ke is asymmetric, so a
        // per-side random password would fail `ClientLogin::finish` at frame 7
        // before any SAS is derived. The human SAS compare authenticates, not the
        // password — see `copypaste_p2p::DISCOVERY_PAIRING_PASSWORD`. (QR pairing
        // keeps its token-derived password; this only affects discovery.)
        let password = copypaste_p2p::DISCOVERY_PAIRING_PASSWORD.to_string();
        let (cert_der, key_der) = (cert.0.clone(), cert.1.clone());
        // B1: our own STUN-discovered global IP, read from the shared cache and
        // advertised in-band so the peer can show it. None if STUN unresolved or
        // collection is disabled. Reuses the daemon's single STUN source.
        let own_public_ip = self.cached_public_ip.read().await.clone();
        let own_device_id = self.local_device_id.clone();
        // CopyPaste-yw2k: read the local Supabase account identity (non-secret)
        // to advertise it in-band so the peer can detect cross-account mismatches.
        let own_supabase_account_id: Option<String> = self
            .cloud_account_id
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        let own_meta = tokio::task::spawn_blocking(move || {
            Self::collect_own_peer_meta(own_public_ip, own_device_id, own_supabase_account_id)
        })
        .await
        .unwrap_or_default();
        // "QR fully provisions all sync": advertise our Supabase/relay config +
        // derived sync key over the authenticated tunnel (None if unconfigured).
        let own_provisioning = self.build_local_provisioning().await;
        // Wait (bounded) for our own P2P sync-listener to bind so we never
        // advertise an empty address. See the QR `pair_accept` path / the
        // `await_own_sync_addr` doc. Resolves immediately once bound.
        let own_sync_addr = self
            .await_own_sync_addr(std::time::Duration::from_secs(5))
            .await;

        let coordinator = Arc::clone(&self.pairing);
        // The confirm callback runs AFTER frame 9 (PAKE + channel binding), when
        // the SAS is known and identical on both honest endpoints. It moves the
        // SM to `awaiting_sas` and awaits the user's decision (or the dropped
        // sender on abort, which it maps to a rejection).
        let confirm = move |sas: &str, peer_fp: &str| {
            let coordinator = Arc::clone(&coordinator);
            let sas = sas.to_string();
            // Forward the already-captured peer snapshot so `pair_get_sas` polls
            // surface the mDNS identity while the user is reading the SAS code.
            // CopyPaste-n3bc: override with the verified TLS peer fingerprint.
            let mut snap = peer_snapshot.clone();
            snap.fingerprint = Some(peer_fp.to_string());
            async move {
                let rx = coordinator.enter_awaiting_sas(
                    sas,
                    crate::pairing_sm::PairingRole::Initiator,
                    snap,
                );
                // SAS_CONFIRM_TIMEOUT bounds the human decision; a dropped sender
                // (abort) or elapsed timeout both yield a rejection.
                match tokio::time::timeout(crate::pairing_sm::SAS_CONFIRM_TIMEOUT, rx).await {
                    Ok(Ok(accept)) => accept,
                    // Sender dropped (pair_abort) or timed out → reject.
                    _ => false,
                }
            }
        };

        let result = copypaste_p2p::bootstrap::run_initiator_with_confirm(
            addr,
            cert_der,
            key_der,
            &password,
            &own_sync_addr,
            &own_meta,
            own_provisioning,
            confirm,
        )
        .await;

        match result {
            Ok(outcome) => {
                tracing::info!(
                    peer_fingerprint = %outcome.peer_fingerprint,
                    "discovery SAS pairing completed (both sides accepted)"
                );
                // Both sides accepted: trust + persist exactly like the QR path.
                if let Some(ref peers) = self.p2p_peers {
                    peers.rotate_peer(
                        &outcome.peer_fingerprint,
                        outcome.peer_fingerprint.to_string(),
                        String::new(),
                    );
                }
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
                // "QR fully provisions all sync": apply any sync config the peer
                // advertised that we currently lack (never overwrites existing).
                if let Some(prov) = outcome.peer_provisioning {
                    self.apply_peer_provisioning(prov).await;
                }
                self.pairing
                    .finish(crate::pairing_sm::PairingState::Confirmed);
                let resp = Response::ok(
                    req_id,
                    serde_json::json!({
                        "ok": true,
                        "peer_fingerprint": outcome.peer_fingerprint.to_string(),
                    }),
                );
                // BUG A1: the terminal outcome is returned synchronously to the
                // UI in `resp`, so the brief observable-window concern does not
                // apply on this initiator path. Reset the SM to `Idle` so a
                // SUBSEQUENT `pair_with_discovered` is not refused as
                // rate-limited (the SM requires `is_idle()` for `try_begin`).
                self.pairing.reset();
                resp
            }
            Err(e) => {
                // Reject / mismatch / timeout / network error → NO persist, NO
                // rotate_peer; the session key already dropped/zeroized inside
                // the bootstrap function. Record a terminal state unless the SM
                // was already moved to a terminal state by `pair_abort`.
                let snapshot = self.pairing.snapshot();
                if !snapshot.is_terminal() {
                    self.pairing
                        .finish(crate::pairing_sm::PairingState::Rejected);
                }
                tracing::warn!("discovery SAS pairing failed: {e}");
                // HB-4: a raw TCP connect failure ("Connection refused", host
                // unreachable, timeout) means the peer's bootstrap responder is
                // not listening — almost always because the device is already
                // paired (so it no longer advertises) or its Devices/pairing
                // screen is closed. Map that to a friendly message instead of the
                // raw os-error; genuine PAKE/SAS failures keep the auth message.
                let lower = e.to_string().to_ascii_lowercase();
                let is_connect_failure = lower.contains("connection refused")
                    || lower.contains("connect")
                    || lower.contains("unreachable")
                    || lower.contains("timed out")
                    || lower.contains("timeout")
                    || lower.contains("os error 61")
                    || lower.contains("os error 111");
                let (code, message) = if is_connect_failure {
                    (
                        ERR_CODE_NOT_FOUND,
                        "device not reachable (already paired or its screen is closed)".to_string(),
                    )
                } else {
                    (
                        ERR_CODE_AUTH_FAILED,
                        format!("discovery SAS pairing failed: {e}"),
                    )
                };
                let resp = Response::err_with_code(req_id, code, message);
                // BUG A1: reset the SM to `Idle` on EVERY failure return path that
                // reached here after `try_begin` succeeded, so the next pairing
                // attempt is not refused as rate-limited. The terminal outcome is
                // already returned synchronously to the UI in `resp` above.
                self.pairing.reset();
                resp
            }
        }
    }
}
