//! Private-mode + status IPC handlers (split from ipc god-module, ra15.1).
use super::*;

impl IpcServer {
    pub(crate) async fn dispatch_status(&self, req: Request) -> Response {
        match req.method.as_str() {
            "set_private_mode" => {
                let enabled = match req.params.get("enabled").and_then(|v| v.as_bool()) {
                    Some(b) => b,
                    // P2-8u2b: tag with ERR_CODE_INVALID_ARGUMENT so machine
                    // clients can classify the error.
                    None => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INVALID_ARGUMENT,
                            "missing param: enabled (bool)",
                        )
                    }
                };
                self.private_mode.store(enabled, Ordering::Relaxed);
                // CopyPaste-48k0: increment the epoch counter so any periodic
                // `status` or `get_private_mode` poll can detect the change
                // without needing a dedicated subscription.  The tray's one-shot
                // poller exits after startup; the epoch lets it (or any other
                // client) re-sync by comparing the epoch value across polls.
                let epoch = self
                    .private_mode_epoch
                    .fetch_add(1, Ordering::Relaxed)
                    .wrapping_add(1);
                // Persist so the setting survives a daemon restart (restored by
                // `daemon::load_private_mode` at startup). Best-effort: the
                // in-memory atomic above is authoritative for this process.
                crate::daemon::persist_private_mode(enabled);
                tracing::info!("private mode set to {enabled} (epoch={epoch})");
                Response::ok(
                    req.id,
                    serde_json::json!({"private_mode": enabled, "private_mode_epoch": epoch}),
                )
            }
            "get_private_mode" => {
                let enabled = self.private_mode.load(Ordering::Relaxed);
                // CopyPaste-48k0: include the epoch so callers can detect
                // changes since their last poll without a separate subscription.
                let epoch = self.private_mode_epoch.load(Ordering::Relaxed);
                Response::ok(
                    req.id,
                    serde_json::json!({"private_mode": enabled, "private_mode_epoch": epoch}),
                )
            }
            "status" => {
                let enabled = self.private_mode.load(Ordering::Relaxed);
                // CopyPaste-48k0: include the epoch in `status` so the UI's
                // periodic health-check poll can detect private-mode changes
                // without a dedicated subscription. A changed epoch → re-sync.
                let epoch = self.private_mode_epoch.load(Ordering::Relaxed);
                // In degraded startup the daemon is alive and the socket is
                // bound, but the backing DB is unavailable (e.g. the Keychain
                // SQLCipher key could not be read after a reinstall). Report
                // status="degraded" + a machine-readable reason + a flag so the
                // UI shows a recovery banner instead of treating the reachable
                // socket as "everything is fine". When healthy, `ready` is true
                // and `degraded_reason` is absent — unchanged shape for clients
                // that only read `status`/`private_mode`.
                // `build_version` + `pid` let a client (or a newer daemon doing
                // socket takeover) detect and evict a STALE predecessor after an
                // upgrade. Both are reported even in the degraded branch so the
                // stale check works without a healthy DB.
                let reason = self
                    .degraded_reason
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .clone();
                // CopyPaste-ruep: surface the X25519 device-key fingerprint
                // (SHA-256 of the public key bytes, lowercase hex) for operator
                // diagnostics.  This is informational only and distinct from the
                // mTLS cert fingerprint that pairing uses.
                let device_key_fingerprint = {
                    use sha2::{Digest as _, Sha256};
                    hex::encode(Sha256::digest(self.device_public_key.as_ref()))
                };
                match reason {
                    Some(reason) => Response::ok(
                        req.id,
                        serde_json::json!({
                            "status": "degraded",
                            "private_mode": enabled,
                            "private_mode_epoch": epoch,
                            "ready": false,
                            "degraded": true,
                            "degraded_reason": reason,
                            "build_version": BUILD_VERSION,
                            "pid": std::process::id(),
                            "device_key_fingerprint": device_key_fingerprint,
                        }),
                    ),
                    None => Response::ok(
                        req.id,
                        serde_json::json!({
                            "status": "running",
                            "private_mode": enabled,
                            "private_mode_epoch": epoch,
                            "ready": self.ready.load(Ordering::Relaxed),
                            "degraded": false,
                            "build_version": BUILD_VERSION,
                            "pid": std::process::id(),
                            "device_key_fingerprint": device_key_fingerprint,
                        }),
                    ),
                }
            }

            _ => self.dispatch_db(req).await,
        }
    }
}
