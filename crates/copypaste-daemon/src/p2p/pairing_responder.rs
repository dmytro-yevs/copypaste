//! Standing discovery-pairing responder loop (LAN/SAS Phase 2).

use std::sync::Arc;
use std::time::Duration;

use tokio_util::sync::CancellationToken;

use copypaste_p2p::transport::PairedPeers;

/// Standing discovery-pairing responder loop (LAN/SAS Phase 2).
///
/// Re-binds the bootstrap listener on the advertised `bport` and accepts ONE
/// inbound SAS-pairing connection per iteration. Each accepted connection runs
/// [`run_with_confirm`](copypaste_p2p::bootstrap::BootstrapResponder::run_with_confirm),
/// routing the derived SAS through the SHARED [`PairingCoordinator`](crate::pairing_sm::PairingCoordinator)
/// so the LOCAL user confirms via the IPC `pair_get_sas` / `pair_confirm_sas`
/// surface exactly like the initiator.
///
/// ## Security
/// The initiator transmits an EPHEMERAL random password in-clear inside the
/// (unauthenticated) bootstrap TLS channel; that password is NOT a secret. The
/// human SAS comparison — derived from the post-PAKE, post-channel-binding
/// `bound_key` — is the SOLE authenticator. Both sides exchange frame-10a
/// ACCEPT/REJECT inside `run_with_confirm`; on reject/mismatch/timeout the
/// session key drops/zeroizes and NOTHING is persisted (no `rotate_peer`). Only
/// on a both-accept success do we `rotate_peer` + `persist_paired_peer`,
/// identical to the QR path, so steady-state remains mutual fingerprint-pinned
/// mTLS.
///
/// ## Single active pairing
/// We only begin (`try_begin`) when the coordinator is `Idle`; a connection that
/// arrives while another pairing (inbound or the IPC-initiated outbound) is in
/// flight is dropped immediately so there is never more than one pending SAS.
// CopyPaste-1w7: `sync_crypto` is the 9th parameter; allow the lint so the
// handle can be threaded through without introducing a new struct solely to
// satisfy the argument-count limit (matching the pattern used for `start_p2p`).
#[allow(clippy::too_many_arguments)]
pub(super) async fn standing_pairing_responder_loop(
    bport: u16,
    cert_der: Vec<u8>,
    key_der: Vec<u8>,
    peers: PairedPeers,
    pairing: Arc<crate::pairing_sm::PairingCoordinator>,
    own_sync_addr: Arc<std::sync::Mutex<Option<String>>>,
    // B1: shared public-IP cache (the daemon's single STUN source). Read each
    // iteration so our own current global IP is advertised in-band to the peer.
    public_ip_cache: Arc<tokio::sync::RwLock<Option<String>>>,
    // CopyPaste-1w7 (H8 fix): the daemon's shared SyncCrypto handle.  Passed
    // to `persist_paired_peer` so `reload_sync_key` runs after a successful
    // button-pair and the running orchestrator picks up the new shared key
    // without a daemon restart.  Matches the three IPC-initiated pairing
    // paths (SAS initiator, QR responder, QR initiator).
    sync_crypto: Option<crate::sync_orch::SyncCrypto>,
    // Our own stable device UUID, threaded in so the advertised PeerMeta
    // carries device_id and the peer can match clipboard origin_device_id to
    // a peer name without relying on the TLS cert fingerprint.
    local_device_id: Option<String>,
    // CopyPaste-yw2k: non-secret Supabase account identity to advertise
    // in-band so the peer can detect cross-account mismatches.
    cloud_account_id: Option<Arc<std::sync::Mutex<Option<String>>>>,
    shutdown: CancellationToken,
) {
    tracing::info!(bport, "LAN/SAS standing pairing responder running");

    // CopyPaste-1hw5: per-peer fingerprint token-bucket rate limiter.
    //
    // DISCOVERY_PAIRING_PASSWORD is a public constant (both sides must use it so
    // OPAQUE-KE can complete), meaning any LAN host can open connections cheaply.
    // Without throttling, a flooding attacker can:
    //   (a) exhaust CPU via repeated Argon2id invocations (one per PAKE round), AND
    //   (b) spam the local user with SAS-confirmation dialogs.
    //
    // Defence layers:
    // 1. Minimum inter-accept delay (`MIN_PAIRING_INTERVAL`): the loop is
    //    serial — at most one PAKE runs at a time. The mandatory sleep between
    //    iterations bounds Argon2id invocations to ≤ 1 per 2 s.
    // 2. Per-fingerprint token-bucket (`pairing_rate_limiter`): keyed on the
    //    TLS peer fingerprint *after* PAKE completes, limiting how often the
    //    same device can surface a SAS dialog. A device that completes PAKE
    //    faster than the budget allows is rejected before the confirm callback
    //    fires, so no SAS is shown.
    //
    // The SAS human-comparison step (confirm callback) still provides the real
    // authentication — these rate limits are additional hardening only.
    use copypaste_p2p::rate_limit::MdnsRateLimiter;
    // Mandatory minimum gap between consecutive accepts. Bounds Argon2id CPU.
    const MIN_PAIRING_INTERVAL: Duration = Duration::from_secs(2);
    // Wrap in Arc so the confirm closure (which is `move`) can share the limiter
    // across iterations without consuming it, while keeping Send + Sync.
    let pairing_rate_limiter = Arc::new(MdnsRateLimiter::new());
    let mut last_accept = std::time::Instant::now()
        .checked_sub(MIN_PAIRING_INTERVAL)
        .unwrap_or(std::time::Instant::now());

    loop {
        // BUG F1: exit promptly if shutdown was requested between iterations.
        if shutdown.is_cancelled() {
            tracing::info!("LAN/SAS standing pairing responder shutting down");
            break;
        }

        // CopyPaste-1hw5 layer 1: enforce a minimum inter-accept interval so
        // an attacker cannot run more than one Argon2id per MIN_PAIRING_INTERVAL.
        // Race the sleep against cancellation so shutdown is not delayed.
        let since_last = last_accept.elapsed();
        if since_last < MIN_PAIRING_INTERVAL {
            tokio::select! {
                biased;
                _ = shutdown.cancelled() => break,
                _ = tokio::time::sleep(MIN_PAIRING_INTERVAL - since_last) => {}
            }
        }

        // Re-bind the fixed bootstrap port for the next inbound pairing. A
        // listening socket is dropped (not connected) between iterations, so it
        // never enters TIME_WAIT and the re-bind succeeds immediately.
        let responder = match copypaste_p2p::bootstrap::BootstrapResponder::bind_on(
            bport,
            cert_der.clone(),
            key_der.clone(),
        )
        .await
        {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(bport, "LAN/SAS: re-bind bootstrap listener failed: {e}");
                // Brief backoff to avoid a hot loop if the port is wedged; race it
                // against cancellation so shutdown is not delayed by the sleep.
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(2)) => {}
                    _ = shutdown.cancelled() => break,
                }
                continue;
            }
        };

        // Resolve our own sync address + metadata for the in-band exchange.
        let own_addr = own_sync_addr
            .lock()
            .map(|s| s.clone().unwrap_or_default())
            .unwrap_or_else(|p| p.into_inner().clone().unwrap_or_default());
        let own_public_ip = public_ip_cache.read().await.clone();
        let own_device_id = local_device_id.clone();
        // CopyPaste-yw2k: read the non-secret Supabase account id to advertise.
        let own_supabase_account_id: Option<String> = cloud_account_id
            .as_ref()
            .and_then(|arc| arc.lock().unwrap_or_else(|p| p.into_inner()).clone());
        let own_meta = tokio::task::spawn_blocking(move || {
            crate::ipc::IpcServer::collect_own_peer_meta(
                own_public_ip,
                own_device_id,
                own_supabase_account_id,
            )
        })
        .await
        .unwrap_or_default();

        // Discovery (QR-less) path: the responder's OPAQUE `PasswordFile` MUST be
        // registered for the SAME password the initiator uses, because opaque-ke
        // is asymmetric — a per-side random password makes `ClientLogin::finish`
        // fail at frame 7 before any SAS is derived. So both ends use the FIXED,
        // well-known, NON-SECRET `copypaste_p2p::DISCOVERY_PAIRING_PASSWORD`; the
        // human SAS compare authenticates, not this value.
        let password = copypaste_p2p::DISCOVERY_PAIRING_PASSWORD.to_string();

        // CopyPaste-1hw5 layer 1: record the moment we accepted (bind succeeded
        // and we are about to start PAKE). This timestamps the accept so the
        // MIN_PAIRING_INTERVAL sleep at the top of the next iteration is accurate.
        last_accept = std::time::Instant::now();

        let coordinator = Arc::clone(&pairing);
        // CopyPaste-1hw5 layer 2: clone the Arc for this iteration's confirm
        // closure. The Arc keeps the shared limiter alive across iterations
        // while satisfying the `move` + `Send + Sync` bounds required by
        // `tokio::spawn` in the calling scope.
        let rl_for_confirm = Arc::clone(&pairing_rate_limiter);
        // Claim the single-active-pairing slot LAZILY inside the confirm
        // callback is too late (the handshake already ran); instead we gate at
        // the SAS step: the confirm callback only runs after frame 9, and we
        // refuse to surface a SAS if a pairing is already active.
        let confirm = move |sas: &str, peer_fp: &str| {
            let coordinator = Arc::clone(&coordinator);
            let rl = Arc::clone(&rl_for_confirm);
            let sas = sas.to_string();
            let peer_fp = peer_fp.to_string();
            // CopyPaste-n3bc: the verified inbound TLS peer fingerprint is now
            // threaded into the confirm callback — surface it in the responder
            // PeerSnapshot so pair_get_sas returns peer identity (was empty default).
            let snap = crate::pairing_sm::PeerSnapshot {
                fingerprint: Some(peer_fp.clone()),
                ..Default::default()
            };
            async move {
                // CopyPaste-1hw5 layer 2: per-fingerprint rate gate.
                // Check the budget BEFORE surfacing a SAS dialog so a peer
                // that completes PAKE repeatedly cannot flood the user with
                // confirm dialogs. The MdnsRateLimiter (reused from the mDNS
                // flood-defence) is keyed on the TLS peer fingerprint: a
                // device that exhausts its per-key budget is rejected here
                // without entering the coordinator, and no SAS is shown.
                if !rl.try_admit_key(&peer_fp) {
                    tracing::warn!(
                        peer_fp = %peer_fp,
                        "LAN/SAS: rate-limiting inbound pairing attempt \
                         (CopyPaste-1hw5: per-fingerprint budget exhausted)"
                    );
                    return false;
                }
                // Single active pairing: if the coordinator is busy, reject.
                if !coordinator.try_begin(crate::pairing_sm::PairingRole::Responder, snap.clone()) {
                    tracing::warn!("LAN/SAS: inbound pairing rejected — another pairing active");
                    return false;
                }
                let rx = coordinator.enter_awaiting_sas(
                    sas,
                    crate::pairing_sm::PairingRole::Responder,
                    snap,
                );
                match tokio::time::timeout(crate::pairing_sm::SAS_CONFIRM_TIMEOUT, rx).await {
                    Ok(Ok(accept)) => accept,
                    // Sender dropped (pair_abort) or timed out → reject.
                    _ => false,
                }
            }
        };

        // BUG F1: race the (potentially long, up to SAS_CONFIRM_TIMEOUT) inbound
        // handshake against cancellation. On shutdown we drop the responder
        // future — cancelling the in-flight handshake (the confirm await resolves
        // to a rejection, keys drop/zeroize) — and exit the loop.
        // "QR fully provisions all sync": this LAN/SAS *discovery* responder does
        // not advertise sync provisioning (it has no `sync_key` handle here, and
        // the feature is scoped to the QR pairing paths). Pass `None`; a future
        // wave can plumb the sync_key Arc through `start_p2p` to enable it on the
        // discovery path too. A peer's received provisioning is left unapplied on
        // this path for the same reason.
        let run_result = tokio::select! {
            r = responder.run_with_confirm(&password, &own_addr, &own_meta, None, confirm) => r,
            _ = shutdown.cancelled() => {
                tracing::info!("LAN/SAS standing pairing responder shutting down (mid-accept)");
                if pairing.snapshot().is_active() {
                    pairing.finish(crate::pairing_sm::PairingState::Aborted);
                }
                if pairing.snapshot().is_terminal() {
                    pairing.reset();
                }
                break;
            }
        };
        match run_result {
            Ok(outcome) => {
                tracing::info!(
                    peer_fingerprint = %outcome.peer_fingerprint,
                    "LAN/SAS inbound pairing completed (both sides accepted)"
                );
                peers.rotate_peer(
                    &outcome.peer_fingerprint,
                    outcome.peer_fingerprint.to_string(),
                    String::new(),
                );
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
                // CopyPaste-1w7 (H8 fix): pass the real SyncCrypto handle so
                // `persist_paired_peer` calls `reload_sync_key` after writing
                // `peers.json`.  This mirrors the three IPC-initiated pairing
                // paths (SAS initiator ipc.rs:2159, QR responder ipc.rs:2312,
                // QR initiator ipc.rs:2436) and ensures the running orchestrator
                // picks up the new shared key without a daemon restart.
                crate::ipc::IpcServer::persist_paired_peer(
                    &outcome.peer_fingerprint,
                    &outcome.peer_sync_addr,
                    &outcome.session_key,
                    &peer_meta,
                    sync_crypto.as_ref(),
                )
                .await;
                pairing.finish(crate::pairing_sm::PairingState::Confirmed);
            }
            Err(e) => {
                // Reject / mismatch / timeout / no inbound connection within the
                // accept window. NO persist, NO rotate_peer — the session key
                // already dropped/zeroized inside `run_with_confirm`. Only move
                // to a terminal state if we had actually begun a pairing (a bare
                // accept-timeout never claimed the coordinator).
                let snap = pairing.snapshot();
                if snap.is_active() {
                    pairing.finish(crate::pairing_sm::PairingState::Rejected);
                }
                tracing::debug!("LAN/SAS inbound pairing ended without success: {e}");
            }
        }

        // Reset to Idle so the next inbound (or IPC-initiated) pairing may begin.
        // The UI has a brief window to observe the terminal state via
        // `pair_get_sas` before this reset; v0.6 keeps it simple.
        if pairing.snapshot().is_terminal() {
            pairing.reset();
        }
    }
}

/// Bind a probe bootstrap listener on an OS-assigned port so `start_p2p` learns
/// the port before advertising it in the mDNS `bport` TXT key, then drop the
/// probe so [`standing_pairing_responder_loop`] can re-bind the SAME port for
/// its first accept. A listening socket is dropped (not connected) between
/// iterations, so it never enters TIME_WAIT and the immediate re-bind succeeds.
///
/// Best-effort: on any failure (bind or `local_addr`) returns `None`, in which
/// case the caller advertises mDNS as v1 (no `bport`) and discovery pairing is
/// simply unavailable on this instance — QR pairing is unaffected.
pub(super) async fn probe_bootstrap_port(cert_der: Vec<u8>, key_der: Vec<u8>) -> Option<u16> {
    match copypaste_p2p::bootstrap::BootstrapResponder::bind_on(0, cert_der, key_der).await {
        Ok(probe) => match probe.local_addr() {
            Ok(addr) => {
                // Drop the probe listener so the responder loop can re-bind
                // the same port for its first accept.
                let p = addr.port();
                drop(probe);
                Some(p)
            }
            Err(e) => {
                tracing::warn!("LAN/SAS: bootstrap listener local_addr failed: {e}");
                None
            }
        },
        Err(e) => {
            tracing::warn!("LAN/SAS: failed to bind bootstrap listener: {e}");
            None
        }
    }
}

/// Spawn [`standing_pairing_responder_loop`] when both `lan_visibility` is
/// enabled AND a bootstrap port was successfully probed. Thin glue extracted
/// from `start_p2p` (ADR-017, CopyPaste-vp63.2) — every clone below is
/// identical to what `start_p2p` built before spawning inline.
///
/// CopyPaste-1htb: gate on `lan_visibility`. When the user sets
/// lan_visibility=false the device must be fully invisible on the LAN — no
/// mDNS advertising AND no inbound pairing listener. The mTLS sync listener
/// (already-paired peers) continues to run because it requires a pre-shared
/// cert fingerprint and never surfaces a SAS dialog; only the unauthenticated
/// bootstrap bport is suppressed here.
#[allow(clippy::too_many_arguments)] // mirrors standing_pairing_responder_loop's own attribute
pub(super) fn spawn_standing_responder_if_visible(
    lan_visibility: bool,
    bootstrap_port: Option<u16>,
    bootstrap_cert_der: Vec<u8>,
    bootstrap_key_der: Vec<u8>,
    peers: PairedPeers,
    pairing: Arc<crate::pairing_sm::PairingCoordinator>,
    own_sync_addr: Arc<std::sync::Mutex<Option<String>>>,
    public_ip_cache: Arc<tokio::sync::RwLock<Option<String>>>,
    sync_crypto: Option<crate::sync_orch::SyncCrypto>,
    device_id: uuid::Uuid,
    cloud_account_id: Option<Arc<std::sync::Mutex<Option<String>>>>,
    shutdown_token: CancellationToken,
) {
    if lan_visibility {
        if let Some(bport) = bootstrap_port {
            let peers_for_responder = peers;
            let pairing_for_responder = pairing;
            let own_sync_addr_for_responder = own_sync_addr;
            let public_ip_cache_for_responder = public_ip_cache;
            let cert_der = bootstrap_cert_der;
            let key_der = bootstrap_key_der;
            let responder_shutdown = shutdown_token;
            // CopyPaste-1w7: clone the SyncCrypto handle (all clones share the
            // same Arc<Mutex<…>> backing store) so the responder can call
            // reload_sync_key after a successful button-pair without a restart.
            let sync_crypto_for_responder = sync_crypto;
            // Thread our own device UUID so the responder advertises it in-band,
            // allowing the peer to match clipboard origin_device_id to a name.
            let local_device_id_for_responder = Some(device_id.to_string());
            // CopyPaste-yw2k: clone the account-id arc so the responder can
            // include our supabase_account_id in PeerMeta (non-secret, not a token).
            let cloud_account_id_for_responder = cloud_account_id;
            tokio::spawn(async move {
                standing_pairing_responder_loop(
                    bport,
                    cert_der,
                    key_der,
                    peers_for_responder,
                    pairing_for_responder,
                    own_sync_addr_for_responder,
                    public_ip_cache_for_responder,
                    sync_crypto_for_responder,
                    local_device_id_for_responder,
                    cloud_account_id_for_responder,
                    responder_shutdown,
                )
                .await;
            });
        }
    } else {
        tracing::info!(
            "lan_visibility=false: bootstrap pairing listener suppressed \
             (CopyPaste-1htb: device fully invisible on LAN)"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// BUG F1 (verification follow-up): the `standing_pairing_responder_loop`
    /// must exit promptly on token cancel. It binds an ephemeral bootstrap port
    /// (`bport = 0`, a passive loopback TCP listener — no multicast) and then
    /// parks inside `run_with_confirm` awaiting an inbound pairing connection
    /// that never arrives, raced against cancellation. Cancelling must drop the
    /// in-flight accept future and break the loop. Fully hermetic.
    #[tokio::test(flavor = "multi_thread")]
    async fn cancellation_token_stops_standing_responder_loop() {
        let token = CancellationToken::new();
        let handle = {
            let cert = copypaste_p2p::cert::SelfSignedCert::generate("f1-responder").unwrap();
            let peers = PairedPeers::new();
            let pairing = Arc::new(crate::pairing_sm::PairingCoordinator::new());
            let own_sync_addr = Arc::new(std::sync::Mutex::new(Some("127.0.0.1:0".to_string())));
            let public_ip_cache = Arc::new(tokio::sync::RwLock::new(None));
            let token = token.clone();
            tokio::spawn(async move {
                standing_pairing_responder_loop(
                    0, // ephemeral bootstrap port — passive loopback listener
                    cert.cert_der,
                    cert.key_der,
                    peers,
                    pairing,
                    own_sync_addr,
                    public_ip_cache,
                    None, // sync_crypto — not needed for cancellation test
                    None, // local_device_id — not needed for cancellation test
                    None, // cloud_account_id — not needed for cancellation test
                    token,
                )
                .await;
            })
        };

        // Give the loop a moment to reach its `run_with_confirm` accept await,
        // then cancel; it must break out well within the bound.
        tokio::time::sleep(Duration::from_millis(100)).await;
        token.cancel();
        let joined = tokio::time::timeout(Duration::from_secs(5), handle).await;
        assert!(
            joined.is_ok(),
            "BUG F1: standing_pairing_responder_loop must exit promptly on token cancel"
        );
        joined.unwrap().unwrap();
    }

    // ── CopyPaste-1hw5: per-fingerprint rate limit in standing_pairing_responder_loop ──

    /// Verify that the per-fingerprint `MdnsRateLimiter` inside
    /// `standing_pairing_responder_loop` behaves correctly in isolation: a fresh
    /// fingerprint is admitted, and after the burst budget is exhausted the same
    /// fingerprint is rejected.
    ///
    /// This exercises the rate-limiting logic path (layer 2) without a real PAKE
    /// exchange — we test `MdnsRateLimiter` directly since the confirm closure in
    /// `standing_pairing_responder_loop` uses the same `try_admit_key` call.
    #[test]
    fn standing_responder_rate_limiter_admits_then_throttles() {
        use copypaste_p2p::rate_limit::{MdnsRateLimiter, BURST_CAPACITY};

        let rl = MdnsRateLimiter::new();
        let fp = "aa:bb:cc:dd:ee:ff:00:11:22:33";

        // A fresh fingerprint should be admitted up to the burst capacity.
        let mut admitted = 0u32;
        for _ in 0..BURST_CAPACITY {
            if rl.try_admit_key(fp) {
                admitted += 1;
            }
        }
        assert_eq!(
            admitted, BURST_CAPACITY,
            "fresh fingerprint should be admitted up to BURST_CAPACITY"
        );

        // Beyond burst: should be rejected (rate limited).
        let beyond = rl.try_admit_key(fp);
        assert!(
            !beyond,
            "CopyPaste-1hw5: fingerprint must be rejected after burst capacity exhausted"
        );
        assert!(rl.total_drops() > 0, "rate limiter must record the drop");
    }
}
