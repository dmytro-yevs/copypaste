//! QR pairing IPC verbs (split from handlers_pairing.rs, ADR-017 daemon-ipc
//! track, CopyPaste-vp63.16).
use super::*;

impl IpcServer {
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
    pub(crate) async fn handle_pair_generate_qr(&self, req: Request) -> Response {
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
            match copypaste_p2p::bootstrap::BootstrapResponder::bind(cert_der, key_der).await {
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
                        let hint = copypaste_p2p::interfaces::advertise_sync_addr(local.port())
                            .to_string();
                        // Race-fix (CopyPaste-7mf): store the handle so
                        // `list_peers` can await it before reading peers.json.
                        let handle = self.spawn_bootstrap_responder(responder, password.clone());
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
    pub(crate) async fn handle_pair_accept_qr(&self, req: Request) -> Response {
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
        let peer_fingerprint = match req.params.get("peer_fingerprint").and_then(|v| v.as_str()) {
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

        let (responder, msg2_bytes) = match PakeResponder::respond(&password_file, &msg1_bytes) {
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
}
