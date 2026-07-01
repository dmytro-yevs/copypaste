//! LAN/SAS discovery-initiated pairing IPC verbs (split from
//! handlers_pairing.rs, ADR-017 daemon-ipc track, CopyPaste-vp63.16).
use super::*;

impl IpcServer {
    /// LAN/SAS Phase 2: begin a discovery-initiated SAS pairing as the
    /// INITIATOR. Resolves the peer's bootstrap port (`bport`) from the
    /// shared discovery snapshot, generates an EPHEMERAL random PAKE
    /// password (the SAS — derived from the post-PAKE bound_key — is the
    /// real authenticator; the password is sent in-clear inside the
    /// bootstrap TLS), and runs `run_initiator_with_confirm` with a
    /// callback wired into the pairing state machine.
    pub(crate) async fn handle_pair_with_discovered(&self, req: Request) -> Response {
        let device_id = match extract_str_param(
            &req.params,
            req.id.clone(),
            "device_id",
            "missing param: device_id",
        ) {
            Ok(s) => s,
            Err(resp) => return resp,
        };
        self.pair_with_discovered(req.id.clone(), &device_id).await
    }

    /// LAN/SAS Phase 2: poll the pairing state machine. Returns the
    /// current state plus the SAS + role when awaiting confirmation.
    /// Also surfaces whatever peer metadata is known at this point:
    ///   • peer_device_name  — mDNS advertised name (initiator path)
    ///   • peer_ip_addrs     — resolved IP addresses (initiator path)
    ///   • peer_fingerprint  — cert fingerprint = mDNS device_id (initiator path)
    /// These are all Optional — absent on the responder path (inbound
    /// connection, no prior mDNS resolution) and gracefully omitted by
    /// the UI. Model/OS/version are NOT surfaced here: the PAKE metadata
    /// extension happens AFTER the SAS confirm step; they appear in the
    /// final `pair_with_discovered` response once both sides accept.
    pub(crate) async fn handle_pair_get_sas(&self, req: Request) -> Response {
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

    /// LAN/SAS Phase 2: deliver the local user's accept/reject decision
    /// into the in-flight handshake's confirm callback. The pairing
    /// succeeds (keys trusted + persisted) only when BOTH sides accept.
    pub(crate) async fn handle_pair_confirm_sas(&self, req: Request) -> Response {
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

    /// LAN/SAS Phase 2: abort an in-flight pairing. Dropping the confirm
    /// channel resolves the handshake's await as a rejection so the
    /// session key drops/zeroizes; the machine moves to `aborted`.
    pub(crate) async fn handle_pair_abort(&self, req: Request) -> Response {
        self.pairing.abort();
        Response::ok(req.id, serde_json::json!({ "ok": true }))
    }
}
