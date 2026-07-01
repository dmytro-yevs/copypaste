//! Cloud/relay sync IPC dispatch facade (split from ipc god-module, ra15.1;
//! further split into handlers_sync_{auth,keys,status}.rs per ADR-017
//! daemon-ipc track, CopyPaste-vp63.18). Verb bodies now live in the sibling
//! submodules as `handle_<verb>` methods on `IpcServer`; this file keeps
//! only the `dispatch_sync` match (including the cfg-gated `not_implemented`
//! stub arms) and the chain-of-responsibility link to `dispatch_status`.
use super::*;

impl IpcServer {
    pub(crate) async fn dispatch_sync(&self, req: Request) -> Response {
        match req.method.as_str() {
            "store_cloud_password" => self.handle_store_cloud_password(req).await,

            #[cfg(feature = "cloud-sync")]
            "cloud_sign_in" => self.handle_cloud_sign_in(req).await,
            #[cfg(feature = "cloud-sync")]
            "cloud_sign_out" => self.handle_cloud_sign_out(req).await,
            // When cloud-sync is not compiled in, cloud_sign_in / cloud_sign_out
            // are not available. Return not_implemented so clients see a
            // machine-readable error_code rather than "method not found".
            #[cfg(not(feature = "cloud-sync"))]
            "cloud_sign_in" | "cloud_sign_out" => Response::not_implemented(req.id, "cloud-sync"),

            #[cfg(feature = "cloud-sync")]
            "set_sync_passphrase" => self.handle_set_sync_passphrase(req).await,

            #[cfg(any(feature = "cloud-sync", feature = "relay-sync"))]
            "rotate_sync_key" => self.handle_rotate_sync_key(req).await,

            #[cfg(any(feature = "cloud-sync", feature = "relay-sync"))]
            "revoke_and_rotate" => self.handle_revoke_and_rotate(req).await,

            #[cfg(feature = "cloud-sync")]
            "get_sync_status" => self.handle_get_sync_status(req).await,

            #[cfg(feature = "cloud-sync")]
            "cloud_test_connection" => self.handle_cloud_test_connection(req).await,

            // When cloud-sync is not compiled in, return not_implemented for
            // Supabase-specific methods so the UI gets a machine-readable code
            // rather than "method not found".
            #[cfg(not(feature = "cloud-sync"))]
            "set_sync_passphrase" | "get_sync_status" | "cloud_test_connection" => {
                Response::not_implemented(req.id, "cloud-sync")
            }

            // rotate_sync_key and revoke_and_rotate are available when EITHER
            // cloud-sync OR relay-sync is compiled in (widened from cloud-sync
            // only — CopyPaste-gbo). When neither is active, report
            // not_implemented rather than "method not found" so callers can
            // distinguish "feature off" from "unknown method".
            #[cfg(not(any(feature = "cloud-sync", feature = "relay-sync")))]
            "rotate_sync_key" | "revoke_and_rotate" => {
                Response::not_implemented(req.id, "cloud-sync or relay-sync")
            }
            _ => self.dispatch_status(req).await,
        }
    }
}
