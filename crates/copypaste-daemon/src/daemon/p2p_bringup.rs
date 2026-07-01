//! P2P subsystem bring-up: identity/config resolution (enable flag, paired-peers
//! allowlist, mDNS discovery service, mTLS identity) and the full subsystem
//! start (catch-up provider, `start_p2p`, shared-slot population, peer-event
//! bridge). Extracted from `run_with_quit_flag` (CopyPaste-vp63.12).
//!
//! `resolve_p2p_identity` runs BEFORE the IPC server is constructed (the IPC
//! server needs clones of `p2p_peers`/`p2p_cert`/`p2p_discovery`/
//! `cert_fingerprint_display`); `start_p2p_subsystem` runs AFTER it (it needs
//! the IPC-server-produced shared slots). Both stay in this module so the P2P
//! identity/config computation lives next to the code that consumes it.

use crate::sync_orch;
use crate::{p2p, paths};
use copypaste_core::{AppConfig, ClipboardItem, Database};
use copypaste_telemetry::{report_and_log, OsTag, ReportableError};
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, Mutex};
use tokio_util::sync::CancellationToken;

/// P2P identity/config resolved once at startup, before the IPC server and
/// [`start_p2p_subsystem`] both need it.
pub(crate) struct P2pIdentity {
    pub(crate) p2p_enabled: bool,
    pub(crate) lan_visibility_at_start: bool,
    pub(crate) p2p_peers: Option<copypaste_p2p::transport::PairedPeers>,
    pub(crate) p2p_discovery: Option<Arc<copypaste_p2p::discovery::DiscoveryService>>,
    pub(crate) p2p_cert: Option<copypaste_p2p::cert::SelfSignedCert>,
    pub(crate) cert_fingerprint_display: Option<String>,
}

/// Resolve whether P2P is enabled, then (when enabled) build the paired-peers
/// allowlist, the shared `DiscoveryService`, and the mTLS identity cert — all
/// BEFORE the IPC server and `start_p2p` are constructed, since both share
/// these exact instances (fix/p2p-c-review #2, LAN/SAS Phase 0, CRITICAL-1).
pub(crate) fn resolve_p2p_identity(local_device_id: &str) -> P2pIdentity {
    // A-SET-4: env var is the override (the app always spawns with COPYPASTE_P2P=1
    // when it wants P2P enabled); when env var is absent, fall back to the
    // persisted IPC config's p2p_enabled so the user's UI toggle takes effect.
    // The IPC AppConfig (config.json) owns p2p_enabled; the core AppConfig
    // (config.toml) owns limits. Read the IPC config here just for this field.
    let p2p_enabled = match std::env::var("COPYPASTE_P2P").as_deref() {
        Ok("1") => true,
        Ok("0") => false,
        // Item 6: single source of truth — delegate to the public accessor so
        // daemon.rs and any future caller always agree on the read path.
        _ => crate::ipc::p2p_enabled_from_config(),
    };
    // lan_visibility is persisted in config.toml (overlaid by update_core_config
    // on set_config). Read it here so start_p2p can gate mDNS at startup.
    let lan_visibility_at_start = {
        let core =
            copypaste_core::AppConfig::load(&crate::paths::config_path()).unwrap_or_default();
        core.lan_visibility
    };
    // fix/p2p-c-review #2: when P2P is enabled, the IPC PAKE handlers and the
    // mTLS transport must share ONE live `PairedPeers` allowlist so a peer
    // paired at runtime is accepted by the accept loop without a restart.
    // `None` when P2P is disabled — IPC pairing then only persists to peers.json.
    let p2p_peers: Option<copypaste_p2p::transport::PairedPeers> = if p2p_enabled {
        Some(copypaste_p2p::transport::PairedPeers::new())
    } else {
        None
    };

    // LAN/SAS Phase 0: construct ONE DiscoveryService here, before both the IPC
    // server and `start_p2p`, so both share the same instance. `None` when P2P
    // is disabled — discovery makes no sense without the P2P stack.
    let p2p_discovery: Option<Arc<copypaste_p2p::discovery::DiscoveryService>> = if p2p_enabled {
        Some(Arc::new(copypaste_p2p::discovery::DiscoveryService::new()))
    } else {
        None
    };

    // CRITICAL-1: generate the mTLS self-signed cert ONCE, here, before both the
    // IPC server and `start_p2p`. The IPC pairing handlers advertise this cert's
    // fingerprint and `start_p2p` makes the transport present the SAME cert — so
    // a scanning/pairing peer pins exactly the value the mTLS verifier compares.
    // `None` when P2P is disabled: no transport runs, so there is no cert to
    // advertise; the pairing IPC handlers then return a clear error.
    let p2p_cert: Option<copypaste_p2p::cert::SelfSignedCert> = if p2p_enabled {
        // P2P-DURABILITY: persist the mTLS identity so the fingerprint peers
        // pin at pairing time is STABLE across daemon restarts.
        //
        // EXCEPTION: tests/dev set COPYPASTE_EPHEMERAL_KEY=1 to keep each
        // instance isolated (no shared on-disk identity), so honour that by
        // falling back to an ephemeral `generate()`.
        let ephemeral = std::env::var("COPYPASTE_EPHEMERAL_KEY").as_deref() == Ok("1");
        let result = if ephemeral {
            copypaste_p2p::cert::SelfSignedCert::generate(local_device_id)
                .map_err(|e| std::io::Error::other(format!("cert generate: {e}")))
        } else {
            copypaste_p2p::cert::SelfSignedCert::load_or_create(
                &paths::p2p_identity_path(),
                local_device_id,
            )
        };
        match result {
            Ok(cert) => Some(cert),
            Err(e) => {
                tracing::warn!(
                    "mTLS cert load/generate failed ({e}); pairing disabled this session"
                );
                None
            }
        }
    } else {
        None
    };
    // Colon-hex (user-facing) form of the cert fingerprint for the pairing
    // surface; `display_fingerprint` round-trips back to `fingerprint_of` via
    // `canonical_fingerprint` at the mTLS boundary.
    let cert_fingerprint_display: Option<String> = p2p_cert
        .as_ref()
        .map(|c| crate::ipc::display_fingerprint(&c.fingerprint()));

    P2pIdentity {
        p2p_enabled,
        lan_visibility_at_start,
        p2p_peers,
        p2p_discovery,
        p2p_cert,
        cert_fingerprint_display,
    }
}

/// Start the P2P subsystem when P2P is enabled (`p2p_peers`/`p2p_cert`/
/// `p2p_discovery` are all `Some`); returns `None` (and drops `sync_outbound_rx`)
/// when P2P is disabled, or when `start_p2p` itself fails.
///
/// CopyPaste-vp63.12: this is the P2P subsystem start step of
/// `run_with_quit_flag`, hoisted verbatim. Each argument is a distinct shared
/// handle produced by an earlier bring-up step (P2P identity resolution, the
/// IPC server construction) that `start_p2p` or its wiring needs; bundling
/// them into a context struct would add indirection without reducing the
/// genuine fan-in from the upstream subsystems (identity, IPC slots, sync
/// channels, cloud, core config) — the same rationale `p2p::start_p2p` itself
/// documents for its own argument list.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn start_p2p_subsystem(
    p2p_peers: Option<copypaste_p2p::transport::PairedPeers>,
    p2p_cert: Option<copypaste_p2p::cert::SelfSignedCert>,
    p2p_discovery: Option<Arc<copypaste_p2p::discovery::DiscoveryService>>,
    local_device_id: &str,
    lan_visibility_at_start: bool,
    db: Arc<Mutex<Database>>,
    local_key_arc: &Arc<zeroize::Zeroizing<[u8; 32]>>,
    new_item_tx: &broadcast::Sender<ClipboardItem>,
    sync_incoming_tx: mpsc::Sender<copypaste_sync::WireItem>,
    sync_outbound_rx: mpsc::Receiver<copypaste_sync::WireItem>,
    pairing_coordinator: Arc<crate::pairing_sm::PairingCoordinator>,
    p2p_sync_addr_slot: Arc<std::sync::Mutex<Option<String>>>,
    public_ip_cache: Arc<tokio::sync::RwLock<Option<String>>>,
    sync_crypto: Option<sync_orch::SyncCrypto>,
    core_config_arc: Arc<std::sync::RwLock<AppConfig>>,
    // Feature-independent type (`Mutex<Option<String>>`) computed by the
    // caller so this parameter list stays uniform across cloud-sync on/off
    // builds; the caller passes `None` when the `cloud-sync` feature is
    // disabled, matching the original `#[cfg(not(feature = "cloud-sync"))]`
    // arm at the `p2p::start_p2p` call site verbatim.
    cloud_account_id_for_p2p: Option<Arc<std::sync::Mutex<Option<String>>>>,
    live_sinks_slot: Arc<std::sync::Mutex<Option<crate::p2p::LivePeerSinks>>>,
    live_rtt_ms_slot: Arc<std::sync::Mutex<Option<crate::p2p::PeerRttMs>>>,
    p2p_shutdown_token_slot: Arc<std::sync::Mutex<Option<CancellationToken>>>,
    peer_event_queue: Arc<
        std::sync::Mutex<std::collections::VecDeque<crate::ipc::PeerEventRecord>>,
    >,
    reporter: &dyn copypaste_telemetry::ErrorReporter,
) -> Option<p2p::P2pHandle> {
    // Start the P2P subsystem when p2p_enabled is true (resolved via
    // `resolve_p2p_identity`). The live allowlist, cert, and shared
    // DiscoveryService must all be present: the cert is the identity the
    // transport presents and that pairing advertises, and the DiscoveryService
    // must be the SAME Arc handed to the IPC server so list_discovered sees
    // live peers (LAN/SAS Phase 0, CRITICAL-1).
    if let (Some(p2p_peers), Some(p2p_cert), Some(p2p_disc)) = (p2p_peers, p2p_cert, p2p_discovery)
    {
        // Reuse the persistent device_id loaded by the caller (parsing it back
        // to Uuid is cheap).
        let device_id =
            uuid::Uuid::parse_str(local_device_id).unwrap_or_else(|_| uuid::Uuid::new_v4());
        let device_name = super::resolve_device_name();

        let p2p_config = p2p::P2pConfig {
            listen_port: 0,
            device_name,
            enabled: true,
            lan_visibility: lan_visibility_at_start,
        };

        // P2P Phase 3 (sync-on-connect catch-up): build a provider that
        // replays the current local history — already re-keyed under the
        // shared sync key, exactly like normal outbound — into each peer the
        // instant a link is established. Without this, an item produced
        // before the link came up is never delivered (fanout is
        // fire-and-forget to currently-connected sinks). Uses the same
        // `SyncCrypto` construction as the orchestrator.
        // CopyPaste-716: the closure takes the connecting peer's fingerprint
        // so `catchup_items` uses that peer's specific pairwise sync key
        // rather than the first cached key for all peers.
        let catchup: p2p::CatchupProvider = {
            let catchup_db = db.clone();
            let catchup_device_id = local_device_id.to_owned();
            let catchup_seed: [u8; 32] = ***local_key_arc;
            Arc::new(move |peer_fingerprint: &str| {
                let crypto =
                    sync_orch::SyncCrypto::new(catchup_seed, crate::ipc::peers_file_path());
                // The closure is `Fn` (sync) but the DB sits behind a tokio
                // Mutex; `block_in_place` + `blocking_lock` safely acquires
                // it on the multi-thread runtime without blocking the worker.
                //
                // Fix B (P2P image perf): split the catch-up into two phases so
                // the DB lock is held ONLY for the sequential read, not for the
                // CPU-heavy per-image re-key (chunk-decrypt + shared-key
                // re-encrypt).
                //   Phase 1: acquire lock, read all raw pages, release lock.
                //   Phase 2: re-key items (CPU, no DB lock).
                let fp = peer_fingerprint.to_owned();

                // Pre-flight: if the connecting peer has no sync key nothing is
                // decryptable, so skip both phases entirely (fast path).
                if crypto.sync_key_for_peer(&fp).is_none() {
                    return Vec::new();
                }

                // Phase 1: read raw items (DB lock held only here).
                let raw = tokio::task::block_in_place(|| {
                    let db = catchup_db.blocking_lock();
                    sync_orch::catchup_read_raw(&db, &catchup_device_id)
                });

                // Phase 2: re-key outside the DB lock (CPU work).
                sync_orch::rekey_catchup_items(raw, &crypto, &fp)
            })
        };

        // Hand the SAME live allowlist already shared with the IPC server
        // (fix/p2p-c-review #2), the SAME cert whose fingerprint the IPC
        // pairing handlers advertise (CRITICAL-1), and the SAME
        // DiscoveryService handed to the IPC server so list_discovered sees
        // live peers (LAN/SAS Phase 0). `start_p2p` seeds the allowlist
        // from peers.json.
        match p2p::start_p2p(
            p2p_config,
            db.clone(),
            device_id,
            (**local_key_arc).clone(),
            p2p_cert,
            p2p_peers,
            new_item_tx.subscribe(),
            sync_incoming_tx.clone(),
            sync_outbound_rx,
            catchup,
            p2p_disc,
            // LAN/SAS Phase 2: the SAME pairing coordinator the IPC server
            // exposes, and the SAME sync-addr slot, so the standing
            // discovery-pairing responder routes its SAS through the shared
            // state machine and advertises a routable sync address in-band.
            Arc::clone(&pairing_coordinator),
            Arc::clone(&p2p_sync_addr_slot),
            // B1: the SAME public-IP cache the IPC server reads and the STUN
            // refresh task writes, so the standing LAN/SAS responder advertises
            // our own global IP in-band exactly like the IPC pairing paths.
            Arc::clone(&public_ip_cache),
            // CopyPaste-1w7 (H8 fix): share a SyncCrypto clone with the
            // standing responder so it can call reload_sync_key after a
            // successful button-pair.
            sync_crypto.clone(),
            // CopyPaste-7ub: the shared live core config so the P2P outbound
            // fanout honours sync_on_wifi_only (same Arc the IPC server hot-reloads).
            core_config_arc.clone(),
            // CopyPaste-yw2k: non-secret Supabase account identity slot so the
            // standing LAN/SAS responder can include it in PeerMeta in-band.
            cloud_account_id_for_p2p,
        )
        .await
        {
            Ok(handle) => {
                tracing::info!(port = handle.actual_port, "P2P subsystem running");
                // P2P Phase 2: publish this daemon's now-bound sync-listener
                // address into the shared slot the IPC pairing handlers read.
                //
                // The listener binds `0.0.0.0:actual_port`, so it is reachable
                // on every interface — but the address we ADVERTISE to a peer
                // (sent in-band during pairing and persisted into the peer's
                // `peers.json`) must be a concrete LAN-routable host, never
                // `127.0.0.1`. `advertise_sync_addr` selects a real LAN address
                // via the same interface filter the QR `addr_hint` uses,
                // falling back to loopback only when no LAN interface exists.
                #[cfg(unix)]
                {
                    let addr = copypaste_p2p::interfaces::advertise_sync_addr(handle.actual_port)
                        .to_string();
                    tracing::info!(
                        sync_addr = %addr,
                        "P2P advertising LAN-routable sync-listener address"
                    );
                    let mut slot = p2p_sync_addr_slot
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    *slot = Some(addr);
                }
                {
                    // Populate the single shared slot used for both online-status
                    // (list_peers) and mutual-unpair signalling (unpair/revoke).
                    // live_sinks and peer_sinks on P2pHandle are Arc clones of the
                    // same underlying map; we write live_sinks here.
                    let mut slot = live_sinks_slot
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    *slot = Some(Arc::clone(&handle.live_sinks));
                }
                {
                    // Populate the RTT slot so list_peers can include latency_ms.
                    let mut slot = live_rtt_ms_slot
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    *slot = Some(Arc::clone(&handle.peer_rtt_ms));
                }
                {
                    // Populate the P2P shutdown token slot so rescan_discovered
                    // can cancel the mDNS browse task on P2P shutdown
                    // (CopyPaste-fbxj). Mirrors the live_sinks_slot pattern.
                    let mut slot = p2p_shutdown_token_slot
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    *slot = Some(handle.shutdown_token.clone());
                }
                {
                    // Subscribe to peer connect/disconnect events and relay
                    // them into the IPC queue so `poll_peer_events` callers
                    // (e.g. the Tauri event bridge) can push live presence
                    // updates to the UI without waiting for the 10 s poll.
                    let mut event_rx = handle.peer_event_tx.subscribe();
                    let event_queue = Arc::clone(&peer_event_queue);
                    let event_shutdown = handle.shutdown_token.clone();
                    tokio::spawn(async move {
                        loop {
                            tokio::select! {
                                biased;
                                _ = event_shutdown.cancelled() => { break; }
                                recv = event_rx.recv() => {
                                    match recv {
                                        Ok(ev) => {
                                            let record = match &ev {
                                                crate::p2p::PeerEvent::Connected { fingerprint } => {
                                                    crate::ipc::PeerEventRecord {
                                                        kind: "connected",
                                                        fingerprint: fingerprint.to_string(),
                                                    }
                                                }
                                                crate::p2p::PeerEvent::Disconnected { fingerprint } => {
                                                    crate::ipc::PeerEventRecord {
                                                        kind: "disconnected",
                                                        fingerprint: fingerprint.to_string(),
                                                    }
                                                }
                                            };
                                            let mut q = event_queue
                                                .lock()
                                                .unwrap_or_else(|p| p.into_inner());
                                            // Cap the queue to avoid unbounded growth
                                            // when no consumer is draining it.
                                            if q.len() >= crate::ipc::PEER_EVENT_QUEUE_CAP {
                                                q.pop_front();
                                            }
                                            q.push_back(record);
                                        }
                                        // Broadcast lagged: receiver fell behind.
                                        // The channel stays open — log and continue
                                        // so live-presence push survives event bursts
                                        // (network flaps etc.).
                                        Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                                            tracing::warn!(
                                                skipped = n,
                                                "P2P event bridge lagged; skipped {n} events"
                                            );
                                            continue;
                                        }
                                        // The sender dropped (P2P shutdown) — exit.
                                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                                    }
                                }
                            }
                        }
                    });
                }
                Some(handle)
            }
            Err(e) => {
                tracing::warn!("Failed to start P2P subsystem: {e}");
                // CopyPaste-9fb6: P2P startup failure is recoverable (daemon
                // continues without sync) but actionable for diagnostics.
                report_and_log(
                    reporter,
                    ReportableError::new(
                        env!("CARGO_PKG_NAME"),
                        env!("CARGO_PKG_VERSION"),
                        "p2p.startup_failed",
                        OsTag::current(),
                    ),
                );
                None
            }
        }
    } else {
        tracing::debug!(
            "P2P disabled (via COPYPASTE_P2P=0 or persisted p2p_enabled=false in config.json; \
                 set COPYPASTE_P2P=1 or toggle in Settings to enable)"
        );
        // Drop sync_outbound_rx — no consumer. sync_orch will log debug
        // on each outbound send (harmless: closed receiver just means no
        // peers are connected).
        drop(sync_outbound_rx);
        None
    }
}
