//! Durable paired-peer persistence (split from pairing_ops.rs, ADR-017
//! daemon-ipc track, CopyPaste-vp63.17).
use super::*;

impl IpcServer {
    /// Durably persist a freshly-paired peer to `peers.json` (P2P Phase 2), in
    /// addition to the in-memory allowlist registration.
    ///
    /// `peer_fp_canonical` is the canonical (colon-free, lowercase) cert
    /// fingerprint the bootstrap channel reports; it is stored in the
    /// user-facing colon-hex form so the rest of the IPC peers surface
    /// (`list_peers`, revoke, etc.) and `load_persisted_peers_into` round-trip
    /// it consistently. `peer_sync_addr` is the peer's P2P sync-listener address
    /// learned in-band, stored so the Phase 3 connector can dial it directly
    /// (loopback mDNS filters 127.0.0.1 and is unreliable).
    ///
    /// Idempotent: if a record with the same fingerprint already exists it is
    /// replaced (address/name refreshed) rather than duplicated. Failures are
    /// logged and swallowed — pairing already succeeded in memory, and a persist
    /// failure must not turn a successful pair into an IPC error.
    ///
    /// A free function (not a `&self` method) so the detached bootstrap-responder
    /// task can call it after `self` has been moved/borrowed away.
    ///
    /// `pub(crate)` so the LAN/SAS Phase 2 standing responder in `p2p.rs` reuses
    /// the IDENTICAL persistence logic as the QR path.
    /// Durably persist a freshly-paired peer to `peers.json`, then refresh the
    /// in-memory sync-key cache.
    ///
    /// CopyPaste-ww5q: the file I/O (`load_peers` + `save_peers` which calls
    /// `fsync`) and the `reload_sync_key` disk read are all synchronous and must
    /// NOT run on an async worker thread.  We pre-compute the CPU-only
    /// `sync_key_b64` derivation (HKDF + base64) on the calling async thread
    /// before the move into `spawn_blocking`, where the blocking disk work
    /// actually executes.  All string data is cloned before the move; the
    /// `SyncCrypto` is `Clone + Send` and is moved in as well.
    pub(crate) async fn persist_paired_peer(
        peer_fp_canonical: &str,
        peer_sync_addr: &str,
        session_key: &copypaste_p2p::pake::SessionKey,
        peer_meta: &copypaste_p2p::bootstrap::PeerMeta,
        sync_crypto: Option<&crate::sync_orch::SyncCrypto>,
    ) {
        // Derive the shared content sync key on the async thread (pure CPU: HKDF
        // + base64 encode, no I/O).  `SessionKey` is not Clone, so we must
        // extract the derived bytes before moving into spawn_blocking.
        let sync_key_b64 = Some(Self::derive_peer_sync_key_b64(session_key));

        // Clone all borrowed data so it can be moved into the blocking thread.
        let peer_fp_canonical = peer_fp_canonical.to_string();
        let peer_sync_addr = peer_sync_addr.to_string();
        let peer_meta = peer_meta.clone();
        let sync_crypto = sync_crypto.cloned();

        let join = tokio::task::spawn_blocking(move || {
            let display = display_fingerprint(&peer_fp_canonical);
            let added_at = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            let address = if peer_sync_addr.is_empty() {
                None
            } else {
                Some(peer_sync_addr.clone())
            };

            let path = peers_file_path();
            let mut peers = crate::peers::load_peers(&path);
            // Preserve any existing first/last-sync stamps across a re-pair so the
            // "first sync" history is not reset when the peer is re-paired.
            let (prior_first_sync, prior_last_sync) = peers
                .iter()
                .find(|p| canonical_fingerprint(&p.fingerprint) == peer_fp_canonical)
                .map(|p| (p.first_sync_at, p.last_sync_at))
                .unwrap_or((None, None));
            // Drop any prior record for the same peer (canonical compare) so a
            // re-pair refreshes the address/name instead of duplicating the entry.
            peers.retain(|p| canonical_fingerprint(&p.fingerprint) != peer_fp_canonical);
            // Populate `name` from the in-band device name received over the
            // bootstrap channel. Falls back to empty string when not provided
            // (e.g. discovery-initiated pairs that predate the device_name field).
            // TODO: carry device_name in PeerMeta for discovery-initiated pairs
            // (requires a BOOTSTRAP_PROTO_VERSION bump + re-pair).
            let name = peer_meta.device_name.clone().unwrap_or_default();
            peers.push(crate::peers::PairedDevice {
                fingerprint: display,
                name,
                added_at,
                address,
                sync_key_b64,
                model: peer_meta.model.clone(),
                os_version: peer_meta.os_version.clone(),
                app_version: peer_meta.app_version.clone(),
                local_ip: peer_meta.local_ip.clone(),
                public_ip: peer_meta.public_ip.clone(),
                // CopyPaste-yw2k: persist the peer's non-secret Supabase account
                // identity so list_peers can surface it and the UI can detect
                // cross-account mismatches at render time (not a token/key).
                supabase_account_id: peer_meta.supabase_account_id.clone(),
                first_sync_at: prior_first_sync,
                last_sync_at: prior_last_sync,
                // password_file_b64 / password_file_enc are only populated on the
                // RESPONDER side by pair_accept_finish; persist_paired_peer is called
                // from the INITIATOR path and the QR-responder bootstrap task — neither
                // holds the PasswordFile blob here.  Both fields default to None;
                // pair_accept_finish writes password_file_enc (encrypted) separately.
                password_file_b64: None,
                password_file_enc: None,
            });

            match crate::peers::save_peers(&path, &peers) {
                Ok(()) => {
                    tracing::info!(
                        fingerprint = %peer_fp_canonical,
                        addr = %peer_sync_addr,
                        "persisted paired peer to peers.json"
                    );
                    // H8: refresh the in-memory sync-key cache so the running
                    // orchestrator picks up the new shared key without a restart.
                    // reload_sync_key reads peers.json (disk I/O), so it belongs
                    // here in the blocking thread.
                    if let Some(ref crypto) = sync_crypto {
                        crypto.reload_sync_key();
                    }
                }
                Err(e) => tracing::warn!(
                    fingerprint = %peer_fp_canonical,
                    "failed to persist paired peer to peers.json: {e}"
                ),
            }
        });

        if let Err(e) = join.await {
            tracing::warn!("persist_paired_peer blocking task panicked: {e}");
        }
    }
}
