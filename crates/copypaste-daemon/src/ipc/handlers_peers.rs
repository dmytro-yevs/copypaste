//! Device-info + peer-listing + discovery IPC handlers (split from ipc god-module, ra15.1).
use super::*;

impl IpcServer {
    pub(crate) async fn dispatch_peers(&self, req: Request) -> Response {
        match req.method.as_str() {
            // ------------------------------------------------------------------
            // P2P IPC methods
            // ------------------------------------------------------------------
            "get_own_fingerprint" => {
                // CRITICAL-1 fix: advertise the live mTLS **certificate**
                // fingerprint — the value peers pin and the mTLS verifier
                // compares (`PeerTransport::fingerprint` / `fingerprint_of`) —
                // NOT the device-key fingerprint
                // (`keychain::own_fingerprint`, SHA-256 of the X25519 public
                // key). The latter is never compared by the mTLS allowlist, so
                // pinning it could never authenticate a channel.
                //
                // When P2P is disabled there is no running transport and thus
                // no cert to advertise; return a clear error rather than a
                // fingerprint that cannot authenticate anything.
                match self.cert_fingerprint.as_ref() {
                    Some(fingerprint) => {
                        Response::ok(req.id, serde_json::json!({ "fingerprint": fingerprint }))
                    }
                    None => Response::err(
                        req.id,
                        "P2P is disabled (set COPYPASTE_P2P=1): no mTLS certificate \
                         to advertise for pairing",
                    ),
                }
            }

            // ----------------------------------------------------------------
            // `get_own_device_info` — rich identity for THIS device.
            //
            // Returns fingerprint (same as `get_own_fingerprint`) PLUS
            // human-readable metadata: device name, model, OS, app version,
            // and LAN IP.  All fields except `app_version` and `fingerprint`
            // are optional (`skip_serializing_if = "is_none"`) so older UI
            // versions that don't know about them still get a valid response.
            //
            // The fingerprint field is omitted when P2P is disabled — callers
            // must gracefully handle a `null` fingerprint (same contract as
            // `get_own_fingerprint`).
            //
            // CopyPaste-bps: previously called DeviceMeta::collect here on
            // every UI refresh, spawning scutil/sysctl/sw_vers (~6 s total).
            // Now reads the process-wide OnceLock cache that was warmed once at
            // daemon startup — no child-process spawn on the hot path.
            // ----------------------------------------------------------------
            "get_own_device_info" => {
                let fingerprint_val = self.cert_fingerprint.clone();
                // get_cached is wait-free after the startup warm; spawn_blocking
                // is kept for correctness on the unlikely cold path.
                let meta = tokio::task::spawn_blocking(|| {
                    crate::device_meta::get_cached(env!("CARGO_PKG_VERSION"))
                })
                .await
                .unwrap_or_else(|_| crate::device_meta::get_cached(env!("CARGO_PKG_VERSION")));
                // Read the cached public IP collected asynchronously on startup
                // (STUN, best-effort). `None` when disabled by config or when
                // the network query has not yet resolved / failed.
                let public_ip_val = self.cached_public_ip.read().await.clone();
                Response::ok(
                    req.id,
                    serde_json::json!({
                        "fingerprint": fingerprint_val,
                        "device_name": meta.device_name,
                        "device_model": meta.device_model,
                        "os_version": meta.os_version,
                        "app_version": meta.app_version,
                        "local_ip": meta.local_ip,
                        "public_ip": public_ip_val,
                    }),
                )
            }

            "list_peers" => {
                // Race-fix (CopyPaste-7mf): if the QR bootstrap responder task is
                // still in flight, await it (with a generous timeout) before reading
                // peers.json. This ensures that a responder-side caller doing
                // `pair_generate_qr` → (initiator scans) → `list_peers` always sees
                // the freshly-persisted peer rather than an empty list.
                // We take the handle out of the slot so we only wait once per
                // bootstrap session; subsequent list_peers calls on the same daemon
                // do not block (the slot is None again).
                {
                    let maybe_handle = self.pending_bootstrap.lock().await.take();
                    if let Some(handle) = maybe_handle {
                        // 5-second timeout — the bootstrap PAKE + file write should
                        // complete in well under 1 s on any real device. If it
                        // times out (task panicked / stuck) we proceed anyway so
                        // list_peers never stalls indefinitely.
                        let _ =
                            tokio::time::timeout(std::time::Duration::from_secs(5), handle).await;
                    }
                }
                match load_peers() {
                    Ok(peers) => {
                        let now_secs = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            // SAFETY: now() is always after UNIX_EPOCH on any
                            // supported platform (macOS, Linux, Android).
                            .unwrap_or_default()
                            .as_secs() as i64;

                        // SINGLE SOURCE OF TRUTH for online status: snapshot the
                        // live P2P peer-sinks map (set of fingerprints with a
                        // non-closed mpsc sender = currently-connected peers).
                        // Falls back to `last_sync_at` recency only when P2P is
                        // disabled (inner slot is None).
                        //
                        // The outer std::sync::Mutex is locked briefly to clone
                        // the inner Arc (no .await while holding it). The inner
                        // tokio Mutex is then locked with .await so we don't block
                        // the executor.
                        let live_fps: Option<std::collections::HashSet<String>> = {
                            // Clone the Arc while holding the std::sync lock, then
                            // drop the lock before awaiting.
                            let maybe_sinks_arc: Option<crate::p2p::LivePeerSinks> = {
                                let slot = self
                                    .live_peer_sinks
                                    .lock()
                                    .unwrap_or_else(|p| p.into_inner());
                                slot.as_ref().map(Arc::clone)
                            };
                            if let Some(sinks_arc) = maybe_sinks_arc {
                                let sinks = sinks_arc.lock().await;
                                Some(
                                    sinks
                                        .iter()
                                        .filter(|(_, tx)| !tx.is_closed())
                                        .map(|(fp, _)| fp.to_string())
                                        .collect(),
                                )
                            } else {
                                None
                            }
                        };

                        // Snapshot the RTT map (fingerprint → last RTT in ms).
                        // Same lazy-injection pattern as live_fps: None when P2P is
                        // disabled or not yet started.
                        let rtt_snapshot: Option<std::collections::HashMap<String, u32>> = {
                            let maybe_rtt_arc: Option<crate::p2p::PeerRttMs> = {
                                let slot = self
                                    .live_peer_rtt_ms
                                    .lock()
                                    .unwrap_or_else(|p| p.into_inner());
                                slot.as_ref().map(std::sync::Arc::clone)
                            };
                            if let Some(rtt_arc) = maybe_rtt_arc {
                                let rtt = rtt_arc.lock().await;
                                Some(rtt.iter().map(|(k, v)| (k.to_string(), *v)).collect())
                            } else {
                                None
                            }
                        };

                        // Snapshot the per-peer rekey-failure counter
                        // (CopyPaste-ptgcc). Gated on `live_fps.is_some()`
                        // like `rtt_snapshot` — the counter is only
                        // meaningful while P2P (and thus fanout) is running.
                        let rekey_snapshot: Option<std::collections::HashMap<String, u32>> =
                            live_fps.is_some().then(crate::p2p::rekey_failure_snapshot);

                        // CopyPaste-1jms.32: Determine which non-P2P transport is
                        // active for this daemon so offline peers can be labelled
                        // "Relay" or "Supabase" rather than a generic "Cloud".
                        //
                        // Relay and Supabase both use a shared inbox (no per-peer
                        // routing), so transport is inferred from daemon config:
                        //   relay running  → all non-P2P peers use "relay"
                        //   supabase signed-in (no relay) → "supabase"
                        //   neither  → None (unknown)
                        //
                        // P2P takes precedence and is determined per-peer below
                        // via `live_fps`.
                        #[cfg(feature = "relay-sync")]
                        let relay_active: bool = {
                            // Non-blocking check: try_lock returns None if the lock
                            // is momentarily held; we conservatively treat that as
                            // "relay running" (the common-path assumption is that
                            // relay IS active — a brief lock on set_config should
                            // not flip the chip to Unknown transiently).
                            self.relay_handle.try_lock().map_or(true, |g| g.is_some())
                        };
                        #[cfg(not(feature = "relay-sync"))]
                        let relay_active: bool = false;

                        #[cfg(feature = "cloud-sync")]
                        let supabase_active: bool = self.cloud_signed_in.load(Ordering::Relaxed);
                        #[cfg(not(feature = "cloud-sync"))]
                        let supabase_active: bool = false;

                        let enriched: Vec<serde_json::Value> = peers
                            .into_iter()
                            .map(|mut peer| {
                                // last_sync_at from the record (i64 or absent).
                                let last_sync_at: Option<i64> =
                                    peer.get("last_sync_at").and_then(|v| v.as_i64());

                                // last_seen_secs: seconds since the last successful
                                // sync, or -1 when we have no stamp at all.
                                let last_seen_secs: i64 = match last_sync_at {
                                    Some(t) => now_secs.saturating_sub(t),
                                    None => -1,
                                };

                                // Compute online from the authoritative source:
                                // 1. If live_fps is available (P2P running): peer is
                                //    online iff its canonical fingerprint has a live
                                //    non-closed sink in the connection table.
                                // 2. Fallback (P2P disabled): recent last_sync_at
                                //    within ONLINE_THRESHOLD_SECS.
                                let peer_fp_canonical = peer
                                    .get("fingerprint")
                                    .and_then(|v| v.as_str())
                                    .map(canonical_fingerprint)
                                    .unwrap_or_default();

                                // c4q2.21: delegate to extracted pure function.
                                let live_sink_opt = live_fps
                                    .as_ref()
                                    .map(|fps| fps.contains(&peer_fp_canonical));
                                let online =
                                    compute_peer_online(live_sink_opt, last_sync_at, now_secs);

                                // latency_ms: last measured RTT for this peer, in ms.
                                // Present only when P2P is running AND a ping-pong has
                                // completed at least once for this connection.
                                let latency_ms: Option<u32> = rtt_snapshot
                                    .as_ref()
                                    .and_then(|m| m.get(&peer_fp_canonical).copied());

                                // rekey_failures: current rekey-failure count
                                // for this peer (CopyPaste-ptgcc). Present
                                // only when P2P is running AND at least one
                                // failure has been recorded.
                                let rekey_failures: Option<u32> = rekey_snapshot
                                    .as_ref()
                                    .and_then(|m| m.get(&peer_fp_canonical).copied());

                                if let Some(obj) = peer.as_object_mut() {
                                    obj.insert(
                                        "online".to_string(),
                                        serde_json::Value::Bool(online),
                                    );
                                    obj.insert(
                                        "last_seen_secs".to_string(),
                                        serde_json::Value::Number(last_seen_secs.into()),
                                    );
                                    if let Some(ms) = latency_ms {
                                        obj.insert(
                                            "latency_ms".to_string(),
                                            serde_json::Value::Number(ms.into()),
                                        );
                                    }
                                    if let Some(count) = rekey_failures {
                                        obj.insert(
                                            "rekey_failures".to_string(),
                                            serde_json::Value::Number(count.into()),
                                        );
                                    }
                                    // CopyPaste-vypo: surface trust status honestly.
                                    // Every record in peers.json completed a PAKE
                                    // handshake (mutual key confirmation), so each
                                    // persisted peer is "verified".  We never store
                                    // unauthenticated (unverified) peers on disk —
                                    // the PAKE session is discarded on failure.
                                    // Hardcoding "Verified" was misleading because it
                                    // implied the label could vary; using the string
                                    // "verified" (lowercase, stable enum value) makes
                                    // the semantics explicit and leaves room for a
                                    // future "unverified" value for in-memory
                                    // discovered-but-not-yet-paired devices.
                                    obj.insert(
                                        "trust".to_string(),
                                        serde_json::Value::String("verified".to_string()),
                                    );

                                    // CopyPaste-1jms.32: surface per-peer transport.
                                    // Priority: P2P (live sink) > Relay > Supabase > None.
                                    let transport_str: Option<&'static str> =
                                        if live_sink_opt == Some(true) {
                                            // Live P2P connection for this peer.
                                            Some("p2p")
                                        } else if relay_active {
                                            // Relay is the active secondary transport.
                                            Some("relay")
                                        } else if supabase_active {
                                            // Supabase is the active secondary transport.
                                            Some("supabase")
                                        } else {
                                            // No transport is active or known.
                                            None
                                        };
                                    if let Some(t) = transport_str {
                                        obj.insert(
                                            "transport".to_string(),
                                            serde_json::Value::String(t.to_string()),
                                        );
                                    }
                                    // CopyPaste-5lm: never expose the PasswordFile blob
                                    // (encrypted or plaintext) over the IPC wire. The UI
                                    // has no need for this field; stripping it here means
                                    // a compromised IPC client cannot exfiltrate it.
                                    obj.remove("password_file_enc");
                                    obj.remove("password_file_b64");
                                }
                                peer
                            })
                            .collect();

                        Response::ok(req.id, serde_json::json!({ "peers": enriched }))
                    }
                    Err(e) => Response::err(req.id, format!("failed to load peers: {e}")),
                }
            }

            // Drain all pending peer connect/disconnect events and return them
            // as an array. Called by the Tauri event bridge every ~1 s so the
            // UI can update online presence dots without waiting for the next
            // full `list_peers` poll.
            //
            // Response: { events: [{ kind: "connected"|"disconnected",
            //                        fingerprint: "<hex>" }] }
            //
            // An empty `events` array is a valid response (no changes since the
            // last poll). This is a draining read — once returned, events are
            // removed from the queue.
            "poll_peer_events" => {
                let events: Vec<PeerEventRecord> = {
                    let mut q = self
                        .peer_event_queue
                        .lock()
                        .unwrap_or_else(|p| p.into_inner());
                    q.drain(..).collect()
                };
                Response::ok(req.id, serde_json::json!({ "events": events }))
            }

            // LAN/SAS Phase 0: return discovered peers (mDNS) cross-referenced
            // against peers.json to flag already-paired devices.
            //
            // Response: { devices: [{ device_id, device_name, ip_addrs, port,
            //                         bport, paired }] }
            // `paired` = true when canonical fingerprint matches peers.json.
            // `bport`  = null on v1 peers (UI disables "Pair" button).
            "list_discovered" => {
                let disc = match self.discovery.as_ref() {
                    Some(d) => d,
                    None => return Response::err(req.id, "discovery not available (P2P disabled)"),
                };

                // CopyPaste-vgpy: `paired_fingerprints()` (pairing.rs) cannot be
                // wired in here — the mDNS-discovered `PeerInfo` carries only
                // `device_id` (the peer's random per-install mDNS UUID,
                // advertised as the `did` TXT key) and never a TLS cert
                // fingerprint, so comparing it against a fingerprint set would
                // never match anything. The stable identity field that DOES
                // round-trip on both sides is `device_id`: `PairedDevice.device_id`
                // persists the SAME mDNS UUID, learned in-band at pairing time
                // (see `peers/model.rs`), and is already used for the identical
                // re-correlation problem in
                // `p2p::connector::discovery_resolve::refresh_peer_meta_from_discovery`.
                // We mirror that pattern here: prefer a `device_id` match (stable
                // across DHCP/IP changes) and fall back to the HB-4 IP-host
                // correlation for legacy peers.json records that predate
                // `device_id` persistence (`device_id: None`).
                //
                // Race-fix (CopyPaste-daq, sibling of CopyPaste-7mf): if the QR
                // bootstrap responder task is still in flight, await it (with a
                // timeout) before reading peers.json. Otherwise a just-paired
                // device's IP is absent from `paired_ips` and the Devices page
                // shows a spurious "Pair" prompt for an already-paired device.
                // Mirrors the identical await in the `list_peers` handler.
                {
                    let maybe_handle = self.pending_bootstrap.lock().await.take();
                    if let Some(handle) = maybe_handle {
                        let _ =
                            tokio::time::timeout(std::time::Duration::from_secs(5), handle).await;
                    }
                }
                let (paired_ips, paired_device_id_set): (
                    std::collections::HashSet<String>,
                    std::collections::HashSet<String>,
                ) = match load_peers() {
                    Ok(stored) => (paired_ip_hosts(&stored), paired_device_ids(&stored)),
                    // Non-fatal: treat as empty — we just won't mark any peer paired.
                    Err(e) => {
                        tracing::warn!("list_discovered: failed to load peers.json: {e}");
                        (
                            std::collections::HashSet::new(),
                            std::collections::HashSet::new(),
                        )
                    }
                };

                let devices: Vec<serde_json::Value> = disc
                    .peers()
                    .into_iter()
                    .map(|peer| {
                        let ip_strs: Vec<String> =
                            peer.ip_addrs.iter().map(|a| a.to_string()).collect();
                        // device_id match takes priority: it is stable across
                        // DHCP/IP changes. Fall back to IP correlation only when
                        // the discovered peer's device_id is unknown to us
                        // (empty / never learned).
                        let paired = (!peer.device_id.is_empty()
                            && paired_device_id_set.contains(&peer.device_id))
                            || ip_strs.iter().any(|ip| paired_ips.contains(ip));
                        serde_json::json!({
                            "device_id":   peer.device_id,
                            "device_name": peer.device_name,
                            "ip_addrs":    ip_strs,
                            "port":        peer.port,
                            // null when peer is v1 (no bport TXT key); UI
                            // disables "Pair" in that case.
                            "bport":       peer.bport,
                            "paired":      paired,
                        })
                    })
                    .collect();

                Response::ok(req.id, serde_json::json!({ "devices": devices }))
            }

            // HB-9: manual rescan. Restart the mDNS-SD browse in place
            // (`DiscoveryService::start` tears down the prior browse task/socket
            // first, then re-advertises + re-browses) and return the fresh peer
            // snapshot. Used by the UI "Refresh" button next to the discovered
            // list when passive polling hasn't surfaced a peer yet.
            //
            // Response: { devices: [...] } — same shape as `list_discovered`.
            "rescan_discovered" => {
                let disc = match self.discovery.as_ref() {
                    Some(d) => d,
                    None => return Response::err(req.id, "discovery not available (P2P disabled)"),
                };

                // CopyPaste-ydhw: abort any browse handle stored from a prior
                // rescan before starting a new one.  This prevents accumulation
                // of orphaned browse tasks across multiple UI "Refresh" presses.
                //
                // Note: `disc.start()` (below) also calls `shutdown_inner()`
                // which aborts the `DiscoveryService`-internal AbortHandle.
                // Aborting `prev_handle` here covers the JoinHandle we returned
                // from the *previous* rescan — the two mechanisms are complementary.
                {
                    let mut slot = self
                        .discovery_browse_handle
                        .lock()
                        .unwrap_or_else(|p| p.into_inner());
                    if let Some(prev_handle) = slot.take() {
                        prev_handle.abort();
                    }
                }

                // Restart-in-place re-browse.  `disc.start()` aborts the prior
                // browse via `shutdown_inner()`, which also aborts the JoinHandle
                // that `start_p2p`'s discovery task was select!-ing on — that
                // task then exits (see p2p.rs discovery task, CopyPaste-ydhw).
                // The IPC server takes over lifecycle ownership of the new browse
                // via `discovery_browse_handle`.
                //
                // If the P2P shutdown token is available (daemon.rs writes it
                // into `p2p_shutdown_token` after `start_p2p` returns), we wrap
                // the browse handle in a select! so it participates in graceful
                // shutdown.  When the token is absent (P2P disabled, or the slot
                // not yet wired by daemon.rs) we still store the handle so the
                // next rescan can abort it.
                match disc.start().await {
                    Ok(handle) => {
                        // Clone the shutdown token BEFORE locking browse_handle
                        // to avoid holding the mutex across an await.
                        let maybe_token: Option<CancellationToken> = {
                            self.p2p_shutdown_token
                                .lock()
                                .unwrap_or_else(|p| p.into_inner())
                                .clone()
                        };

                        let wrapper_handle = if let Some(token) = maybe_token {
                            // Wrap the browse handle with a cancellation select!
                            // so P2P shutdown aborts the browse task cleanly.
                            tokio::spawn(async move {
                                tokio::select! {
                                    _ = handle => {}
                                    _ = token.cancelled() => {
                                        tracing::debug!(
                                            "rescan_discovered browse task shut down by P2P shutdown token"
                                        );
                                    }
                                }
                            })
                        } else {
                            // No shutdown token yet — spawn a plain wrapper so
                            // dropping `wrapper_handle` does not abort the browse.
                            // The browse runs until the next rescan aborts it.
                            tokio::spawn(async move {
                                let _ = handle.await;
                            })
                        };

                        // Store the wrapper handle so the next rescan can abort it.
                        *self
                            .discovery_browse_handle
                            .lock()
                            .unwrap_or_else(|p| p.into_inner()) = Some(wrapper_handle);
                    }
                    Err(e) => {
                        return Response::err(req.id, format!("rescan failed to start: {e}"));
                    }
                }

                // CopyPaste-vgpy: device_id-correlate already-paired peers, with
                // HB-4 IP correlation as the legacy fallback (see `list_discovered`
                // for the full rationale — `paired_fingerprints()` does not apply
                // here because discovered peers carry no TLS cert fingerprint).
                let (paired_ips, paired_device_id_set): (
                    std::collections::HashSet<String>,
                    std::collections::HashSet<String>,
                ) = match load_peers() {
                    Ok(stored) => (paired_ip_hosts(&stored), paired_device_ids(&stored)),
                    Err(e) => {
                        tracing::warn!("rescan_discovered: failed to load peers.json: {e}");
                        (
                            std::collections::HashSet::new(),
                            std::collections::HashSet::new(),
                        )
                    }
                };

                let devices: Vec<serde_json::Value> = disc
                    .peers()
                    .into_iter()
                    .map(|peer| {
                        let ip_strs: Vec<String> =
                            peer.ip_addrs.iter().map(|a| a.to_string()).collect();
                        let paired = (!peer.device_id.is_empty()
                            && paired_device_id_set.contains(&peer.device_id))
                            || ip_strs.iter().any(|ip| paired_ips.contains(ip));
                        serde_json::json!({
                            "device_id":   peer.device_id,
                            "device_name": peer.device_name,
                            "ip_addrs":    ip_strs,
                            "port":        peer.port,
                            "bport":       peer.bport,
                            "paired":      paired,
                        })
                    })
                    .collect();

                Response::ok(req.id, serde_json::json!({ "devices": devices }))
            }

            _ => self.dispatch_pairing(req).await,
        }
    }
}

/// CopyPaste-vgpy: build the set of paired peers' stable mDNS `device_id`s
/// (the per-install UUID advertised as the `did` TXT key, persisted on
/// `PairedDevice.device_id` — see `peers/model.rs`), for correlating
/// mDNS-discovered peers against `peers.json` **by identity** instead of by
/// IP host.
///
/// This is the discovered-list analogue of
/// `p2p::connector::discovery_resolve::refresh_peer_meta_from_discovery`,
/// which already re-keys a *paired* peer's persisted address on this same
/// `device_id` after a DHCP/roaming IP change. `pairing::paired_fingerprints`
/// is NOT usable here: it is keyed on the peer's TLS **cert fingerprint**,
/// but the mDNS `PeerInfo` a discovered peer is built from carries no cert
/// fingerprint at all (only `device_id`, `device_name`, `ip_addrs`, `port`,
/// `bport`) — the fingerprint is learned only after an authenticated mTLS
/// handshake, which a not-yet-paired discovered peer has not undergone.
///
/// Records written before `device_id` persistence existed deserialize it to
/// `None` and are simply absent from the returned set; callers must keep the
/// [`paired_ip_hosts`] fallback for those legacy peers.
fn paired_device_ids(peers: &[serde_json::Value]) -> std::collections::HashSet<String> {
    peers
        .iter()
        .filter_map(|p| p.get("device_id").and_then(|v| v.as_str()))
        .filter(|id| !id.is_empty())
        .map(str::to_string)
        .collect()
}
