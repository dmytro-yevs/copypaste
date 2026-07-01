//! Discovery + SAS pairing (ABI 12 — Android parity for LAN discovery).
//!
//! The Android analog of the macOS daemon's discovery-pairing path. Drives the
//! SAME `copypaste_p2p` discovery (mDNS browse/advertise) + bootstrap PAKE stack
//! the desktop uses, wired to a POLLED state machine (UniFFI cannot pass an async
//! Rust callback). Kotlin starts discovery once, polls `list_discovered`, calls
//! `pair_with_discovered` to initiate, polls `pair_get_sas` for the SAS, then
//! confirms/aborts. The standing responder bound on `bport` makes the Android
//! device pairable FROM macOS. See `pairing.rs` for the full security contract.

use crate::{pairing, panic_boundary, CopypasteError};

use super::bootstrap::{build_android_peer_meta, confirmed_pairing_from, SyncProvisioning};
use super::runtime::runtime;

/// Start LAN discovery + the standing SAS-pairing responder. Idempotent: a
/// second call tears down and replaces the previous discovery/responder tasks
/// (restart-in-place after a roster / port change).
///
/// Advertises this device over mDNS with the v2 `bport` TXT key (so macOS peers
/// can dial it for SAS pairing) and browses for peers. ALSO binds a standing
/// `BootstrapResponder` on `bport` that accepts inbound discovery-pair
/// connections and runs `run_with_confirm` wired to the SAME coordinator with
/// the `Responder` role — this is what makes Android pairable FROM macOS.
///
/// `cert_der`/`key_der` are this device's mTLS identity (`generate_device_cert`);
/// `sync_port` is the P2P sync-listener port advertised in mDNS; `bport` is the
/// fixed TCP port the standing bootstrap responder binds (advertised so
/// initiators know where to dial). `key_der` is secret — the caller must zero
/// the ByteArray after the call and never log it.
///
/// Errors: [`CopypasteError::P2pError`] if the discovery registration, mDNS
/// daemon, or the standing responder bind fails.
#[allow(clippy::too_many_arguments)] // FFI contract: identity + ports + names.
pub fn start_discovery(
    device_id: String,
    device_name: String,
    sync_port: u16,
    bport: u16,
    cert_der: &[u8],
    key_der: &[u8],
    // HB-1a (ABI 14) / PG-28 (ABI 18): THIS device's own metadata, threaded
    // into the standing responder loop so a macOS-INITIATED discovery pair
    // records real Android info. `device_name` is already a param; the standing
    // responder reuses it for `PeerMeta.device_name`.
    device_model: Option<String>,
    os_version: Option<String>,
    app_version: Option<String>,
    local_ip: Option<String>,
    // ABI 18 (PG-28): STUN-derived WAN address. Kotlin collects it via
    // `StunUtils.queryPublicIp` before starting discovery. `None` when
    // `collect_public_ip` is false or STUN failed.
    public_ip: Option<String>,
) -> Result<(), CopypasteError> {
    panic_boundary::catch_result(|| {
        let rt = runtime()?;
        let cert_der = cert_der.to_vec();
        let key_der = key_der.to_vec();
        // Assemble the responder's PeerMeta once; reuse `device_name` (already a
        // param) for the friendly name field.
        let own_meta = build_android_peer_meta(
            Some(device_name.clone()),
            device_model,
            os_version,
            app_version,
            local_ip,
            public_ip,
        );

        // Build + register the discovery service (advertise with bport so we are
        // a v2 peer macOS can pair with) and start its browse task.
        let discovery = std::sync::Arc::new(copypaste_p2p::discovery::DiscoveryService::new());
        discovery
            .register_with_bport(sync_port, device_id.clone(), device_name.clone(), bport)
            .map_err(|e| pairing::p2p_err(format!("discovery register failed: {e}")))?;
        let discovery_for_start = std::sync::Arc::clone(&discovery);
        let browse_task = rt.spawn(async move {
            // `start` returns a JoinHandle for the internal browse loop; await it
            // so this task lives as long as discovery is running. A start error
            // just ends the task (discovery simply yields no peers).
            if let Ok(handle) = discovery_for_start.start().await {
                let _ = handle.await;
            }
        });

        // Spawn the standing bootstrap responder: re-bind `bport`, accept ONE
        // inbound discovery-pair connection per iteration, run `run_with_confirm`
        // wired to the SAME coordinator with the Responder role.
        let responder_task = rt.spawn(standing_responder_loop(bport, cert_der, key_der, own_meta));

        pairing::global().install(discovery, browse_task, responder_task);
        Ok(())
    })
}

/// The standing-responder accept loop (Responder role). Re-binds `bport` for
/// each inbound pairing attempt and runs the confirm-gated responder handshake
/// wired to the global coordinator. Never logs key/SAS bytes.
async fn standing_responder_loop(
    bport: u16,
    cert_der: Vec<u8>,
    key_der: Vec<u8>,
    // HB-1a (ABI 14): this device's own metadata, advertised to the macOS
    // initiator on every accepted pairing (was `PeerMeta::default()`).
    own_meta: copypaste_p2p::bootstrap::PeerMeta,
) {
    use copypaste_p2p::bootstrap::BootstrapResponder;

    let coordinator = std::sync::Arc::clone(&pairing::global().coordinator);
    loop {
        // Bind the fixed bport afresh each iteration. A *listening* socket that
        // is dropped (not connected) never enters TIME_WAIT, so re-binding the
        // same port succeeds immediately (mirrors the macOS standing responder).
        let responder =
            match BootstrapResponder::bind_on(bport, cert_der.clone(), key_der.clone()).await {
                Ok(r) => r,
                Err(_e) => {
                    // Bind failed (port busy / transient). Back off briefly and retry
                    // so a momentary conflict does not permanently disable inbound
                    // pairing. Never log the error verbatim (no secrets, but keep it
                    // quiet — this loop is hot).
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    continue;
                }
            };

        // Only accept an inbound pairing when idle (single active pairing). If a
        // pairing is already in flight, drop this responder and loop; the next
        // bind happens once the previous one finishes.
        if !coordinator.try_begin(pairing::PairingRole::Responder) {
            drop(responder);
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            continue;
        }

        let confirm_coord = std::sync::Arc::clone(&coordinator);
        // The discovery path uses a FIXED well-known PAKE password
        // (`DISCOVERY_PAIRING_PASSWORD`): opaque-ke is asymmetric, so both ends
        // must register the IDENTICAL password or the handshake fails at frame 7
        // before any SAS is derived. Authentication is ENTIRELY via the SAS
        // compare (see `pairing::DISCOVERY_PAIRING_PASSWORD` docs + plan
        // §"SAS design rationale"). The responder advertises no sync_addr here
        // (Android learns the peer's address from the inbound frames / discovery).
        let result = responder
            .run_with_confirm(
                pairing::DISCOVERY_PAIRING_PASSWORD,
                "",
                // HB-1a: advertise this Android device's real metadata.
                &own_meta,
                None,
                move |sas: &str, _peer_fp: &str| {
                    let coord = std::sync::Arc::clone(&confirm_coord);
                    let sas = sas.to_string();
                    async move {
                        // Park on the user's decision, bounded by the SAS window.
                        let rx = coord.enter_awaiting_sas(sas, pairing::PairingRole::Responder);
                        match tokio::time::timeout(pairing::SAS_CONFIRM_TIMEOUT, rx).await {
                            Ok(Ok(accept)) => accept,
                            // Timeout or sender dropped (abort) → reject.
                            _ => false,
                        }
                    }
                },
            )
            .await;

        match result {
            Ok(p) => {
                coordinator.finish(pairing::PairingState::Confirmed(confirmed_pairing_from(p)))
            }
            Err(_e) => {
                // A confirm-rejected SAS, a timeout, an abort, or a network/PAKE
                // failure all land here. Only move out of an active state — if
                // `pair_abort` already set Aborted, leave it. Keys drop/zeroize
                // (nothing persisted). Distinguish timeout vs reject is not
                // observable from the Err alone, so report Aborted unless the
                // coordinator already recorded a terminal state.
                if coordinator.snapshot().is_active() {
                    coordinator.finish(pairing::PairingState::Aborted);
                }
            }
        }
    }
}

/// Stop LAN discovery + the standing responder. Idempotent. Aborts the browse,
/// responder, and any in-flight initiator task and drops the discovery service
/// (releasing the mDNS socket). Any in-flight confirmation is aborted.
pub fn stop_discovery() -> Result<(), CopypasteError> {
    panic_boundary::catch_result(|| {
        pairing::global().stop();
        Ok(())
    })
}

/// Snapshot the peers currently discovered on the LAN. Despite its legacy name
/// (frozen for ABI 14), `paired_fingerprints` now carries the caller's set of
/// already-paired IP HOSTS (a peer's `local_ip` / sync-address host) — NOT cert
/// fingerprints.
///
/// HB-4: the mDNS `device_id` is a random UUID, not a cert fingerprint, so the
/// old fingerprint-compare against `device_id` NEVER matched and paired devices
/// kept showing "Pair". We now mark a peer `paired` when ANY of its resolved
/// `ip_addrs` is in the caller-supplied set. Returns an empty list when
/// discovery is not running.
pub fn list_discovered(
    paired_fingerprints: Vec<String>,
) -> Result<Vec<pairing::DiscoveredPeer>, CopypasteError> {
    panic_boundary::catch_result(|| {
        let Some(discovery) = pairing::global().discovery() else {
            return Ok(Vec::new());
        };
        // Param name is frozen at ABI 14; the values are paired IP hosts.
        let paired_ips: std::collections::HashSet<String> = paired_fingerprints
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect();
        let peers = discovery
            .peers()
            .into_iter()
            .map(|p| {
                let is_paired = p
                    .ip_addrs
                    .iter()
                    .any(|ip| paired_ips.contains(&ip.to_string()));
                pairing::DiscoveredPeer::from_peer_info(p, is_paired)
            })
            .collect();
        Ok(peers)
    })
}

/// Begin pairing (Initiator role) with a discovered peer. Resolves the peer's
/// `bport` + IPv4-first address from discovery, claims the coordinator, and
/// SPAWNS the bootstrap initiator on the shared runtime (does NOT block). Kotlin
/// then polls `pair_get_sas` for the SAS and calls `pair_confirm_sas`.
///
/// `cert_der`/`key_der` are this device's mTLS identity; `sync_addr` is this
/// device's own P2P sync-listener `host:port` (sent in-band); `local_provisioning`
/// is the OPTIONAL sync-account setup this device offers (typically `null` on
/// Android). Errors: [`CopypasteError::P2pError`] if the peer is unknown, lacks a
/// `bport` (v1 peer), advertises no address, or a pairing is already in flight.
#[allow(clippy::too_many_arguments)] // FFI contract: peer id + identity + 5 meta fields.
pub fn pair_with_discovered(
    device_id: String,
    cert_der: &[u8],
    key_der: &[u8],
    sync_addr: String,
    local_provisioning: Option<SyncProvisioning>,
    // HB-1a (ABI 14) / PG-28 (ABI 18): THIS device's own metadata, advertised
    // to the discovered peer during the initiator handshake.
    device_name: Option<String>,
    device_model: Option<String>,
    os_version: Option<String>,
    app_version: Option<String>,
    local_ip: Option<String>,
    // ABI 18 (PG-28): STUN-derived WAN address. Kotlin collects it via
    // `StunUtils.queryPublicIp` before calling this function. `None` when
    // `collect_public_ip` is false or STUN failed.
    public_ip: Option<String>,
) -> Result<(), CopypasteError> {
    panic_boundary::catch_result(|| {
        let rt = runtime()?;
        let global = pairing::global();

        let Some(discovery) = global.discovery() else {
            return Err(pairing::p2p_err("discovery is not running"));
        };
        let peer = discovery
            .resolve_peer(&device_id)
            .ok_or_else(|| pairing::p2p_err(format!("peer {device_id} not found in discovery")))?;
        if peer.bport.is_none() {
            return Err(pairing::p2p_err(
                "peer is a v1 build (no bport) and cannot SAS-pair",
            ));
        }
        let addr = pairing::ipv4_first_addr(&peer)
            .ok_or_else(|| pairing::p2p_err("peer advertised no routable address"))?;

        // Claim the machine (single active pairing). The standing responder uses
        // the same coordinator, so this also refuses while an inbound pairing is
        // in flight.
        if !global
            .coordinator
            .try_begin(pairing::PairingRole::Initiator)
        {
            return Err(pairing::p2p_err("a pairing is already in flight"));
        }

        let coordinator = std::sync::Arc::clone(&global.coordinator);
        let cert_der = cert_der.to_vec();
        let key_der = key_der.to_vec();
        let provisioning = local_provisioning.map(Into::into);
        // ABI 18: build PeerMeta including the STUN-derived public_ip.
        let own_meta = build_android_peer_meta(
            device_name,
            device_model,
            os_version,
            app_version,
            local_ip,
            public_ip,
        );

        let task = rt.spawn(async move {
            use copypaste_p2p::bootstrap::run_initiator_with_confirm;
            let confirm_coord = std::sync::Arc::clone(&coordinator);
            let result = run_initiator_with_confirm(
                addr,
                cert_der,
                key_der,
                // Discovery path: fixed well-known PAKE password; the SAS is the
                // real authenticator (see `pairing::DISCOVERY_PAIRING_PASSWORD`).
                pairing::DISCOVERY_PAIRING_PASSWORD,
                &sync_addr,
                // HB-1a: advertise this Android device's real metadata.
                &own_meta,
                provisioning,
                move |sas: &str, _peer_fp: &str| {
                    let coord = std::sync::Arc::clone(&confirm_coord);
                    let sas = sas.to_string();
                    async move {
                        let rx = coord.enter_awaiting_sas(sas, pairing::PairingRole::Initiator);
                        match tokio::time::timeout(pairing::SAS_CONFIRM_TIMEOUT, rx).await {
                            Ok(Ok(accept)) => accept,
                            _ => false,
                        }
                    }
                },
            )
            .await;

            match result {
                Ok(p) => {
                    coordinator.finish(pairing::PairingState::Confirmed(confirmed_pairing_from(p)))
                }
                Err(_e) => {
                    // Reject/timeout/abort/network failure: keys drop/zeroize,
                    // nothing persisted. Only move out of an active state so an
                    // explicit `pair_abort` (Aborted) is not clobbered.
                    if coordinator.snapshot().is_active() {
                        coordinator.finish(pairing::PairingState::Aborted);
                    }
                }
            }
        });
        global.set_initiator_task(task);
        Ok(())
    })
}

/// Poll the current pairing status. While active, `sas` + `role` are populated;
/// the `peer_*` outputs (incl. the 32-byte `session_key`) are populated ONLY
/// when `state == "confirmed"`. Kotlin persists those then calls `pair_reset`.
/// The `session_key` is secret — zero the ByteArray after KEK-wrapping it.
pub fn pair_get_sas() -> Result<pairing::PairStatus, CopypasteError> {
    panic_boundary::catch_result(|| {
        let state = pairing::global().coordinator.snapshot();
        Ok(pairing::PairStatus::from_state(&state))
    })
}

/// Deliver the local user's accept(`true`)/reject(`false`) SAS decision into the
/// in-flight handshake. A reject drops/zeroizes the session key (nothing
/// persisted). No-op (returns Ok) when no pairing is awaiting confirmation.
pub fn pair_confirm_sas(accept: bool) -> Result<(), CopypasteError> {
    panic_boundary::catch_result(|| {
        pairing::global().coordinator.deliver_decision(accept);
        Ok(())
    })
}

/// Abort the in-flight pairing: cancel the initiator task, drop the confirmation
/// channel (the handshake's confirm await resolves to a rejection → keys
/// drop/zeroize), and move the machine to `aborted`. Idempotent.
pub fn pair_abort() -> Result<(), CopypasteError> {
    panic_boundary::catch_result(|| {
        let global = pairing::global();
        global.abort_initiator();
        global.coordinator.abort();
        Ok(())
    })
}

/// Reset the pairing machine to `idle` (call after observing a terminal state so
/// a fresh pairing may begin). Also aborts any lingering initiator task.
pub fn pair_reset() -> Result<(), CopypasteError> {
    panic_boundary::catch_result(|| {
        let global = pairing::global();
        global.abort_initiator();
        global.coordinator.reset();
        Ok(())
    })
}
