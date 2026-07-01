//! Pairing / unpair / revoke IPC dispatch facade (split from ipc god-module,
//! ra15.1; further split into handlers_pairing_{sas,revoke,password,qr}.rs
//! per ADR-017 daemon-ipc track, CopyPaste-vp63.16). Verb bodies now live in
//! the sibling submodules as `handle_<verb>` methods on `IpcServer`; this
//! file keeps only the verb → handler dispatch table and the
//! chain-of-responsibility link to `dispatch_transfer`.
use super::*;

impl IpcServer {
    pub(crate) async fn dispatch_pairing(&self, req: Request) -> Response {
        match req.method.as_str() {
            "pair_with_discovered" => self.handle_pair_with_discovered(req).await,
            "pair_get_sas" => self.handle_pair_get_sas(req).await,
            "pair_confirm_sas" => self.handle_pair_confirm_sas(req).await,
            "pair_abort" => self.handle_pair_abort(req).await,

            "pair_peer" => self.handle_pair_peer(req).await,
            "unpair_peer" => self.handle_unpair_peer(req).await,
            "revoke_peer" => self.handle_revoke_peer(req).await,
            "revoke_all_peers" => self.handle_revoke_all_peers(req).await,

            "pair_peer_with_password" => self.handle_pair_peer_with_password(req).await,
            "pair_accept_password" => self.handle_pair_accept_password(req).await,
            "pair_accept_finish" => self.handle_pair_accept_finish(req).await,

            "pair_generate_qr" => self.handle_pair_generate_qr(req).await,
            "pair_accept_qr" => self.handle_pair_accept_qr(req).await,

            _ => self.dispatch_transfer(req).await,
        }
    }
}
