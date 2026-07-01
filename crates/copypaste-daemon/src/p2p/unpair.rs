//! Peer eviction (receive-side) and revocation session teardown (send-side).

use copypaste_p2p::transport::{DeviceFingerprint, PairedPeers};
use copypaste_sync::protocol::{ControlMsg, PeerFrame};

use super::PeerSinks;

/// Send a `ControlMsg::Unpair` notification to the revoked peer and block
/// further outbound data delivery by removing the peer's sender from `peer_sinks`.
///
/// # Security — revocation lane (CopyPaste-1jms.8 + CopyPaste-qw1k)
///
/// This is the **send side** of revocation-triggered session teardown. Two
/// things must happen when a peer is locally revoked:
///
/// 1. **Notify the peer** (`ControlMsg::Unpair` frame) so it learns of the
///    revocation immediately over the still-open channel (CopyPaste-1jms.8).
///    Best-effort via `try_send` — if the sink is full or closed the peer will
///    learn at the next handshake via mTLS rejection instead.
///
/// 2. **Block further outbound data** by removing the sender from `peer_sinks`
///    (CopyPaste-qw1k). The fanout loop snapshots `peer_sinks` on every item;
///    once this entry is gone, no new clipboard data is delivered to the revoked
///    peer. Additionally, when the peer's `run_peer_connection_framed` pump
///    delivers the Unpair frame, the peer (running the same pump) calls
///    `evict_peer_local` + `return`, dropping its `Framed` stream and sending
///    TCP FIN. Our pump's `framed.next()` then returns `None`/EOF, and the pump
///    exits within one RTT — completing the session teardown (CopyPaste-qw1k).
///
/// **Non-cooperative peers**: if the peer ignores the Unpair frame and keeps
/// the connection open, the ping-loop dead-connection detection (CopyPaste-8i3q)
/// will eventually evict the stale sink and close the TLS stream when a Pong is
/// not received within `PING_PONG_TIMEOUT`. New handshakes from the revoked peer
/// are always rejected because the mTLS allowlist entry was already removed by
/// the caller before this function is invoked.
///
/// **Constraint**: `ipc.rs` currently calls `send_unpair_signal_if_connected`
/// which only does step 1 (fire-and-forget `try_send`) but NOT step 2 (removes
/// the sink from `peer_sinks`). As a result, the fanout loop can still deliver
/// items to the revoked peer between the `try_send` and the TCP FIN. Callers
/// should replace that call with this function for full revocation semantics.
/// The `ipc.rs` call site is tracked as a cross-file follow-up (ipc.rs is
/// owned by a different lane and cannot be edited here).
///
/// `peer_sinks` is the same `Arc<Mutex<HashMap<…>>>` exposed on `P2pHandle`
/// as both `live_sinks` and `peer_sinks` (they point to the same underlying map).
///
/// Returns `true` if a live session entry was found and removed; `false` if the
/// peer had no active sink (was already disconnected or never connected).
pub async fn send_unpair_and_close_session(peer_sinks: &PeerSinks, canonical_fp: &str) -> bool {
    let mut sinks = peer_sinks.lock().await;
    match sinks.remove(canonical_fp) {
        None => {
            // No live session entry — peer was offline or already disconnected.
            // The durable pending-unpair queue in `pending_unpair.json` handles
            // offline delivery via the connector loop (Gap A).
            tracing::debug!(
                peer = %canonical_fp,
                "send_unpair_and_close_session: no live sink — peer was offline"
            );
            false
        }
        Some(tx) => {
            // Step 1 (CopyPaste-1jms.8): notify the peer BEFORE dropping tx so
            // the Unpair frame is queued into the channel while the pump is still
            // draining it. `try_send` is non-blocking; a full or already-closed
            // channel is silently ignored (the peer learns at next mTLS handshake
            // rejection instead — acceptable for a misbehaving or lagged peer).
            let _ = tx.try_send(PeerFrame::Control(ControlMsg::Unpair));
            tracing::info!(
                peer = %canonical_fp,
                "send_unpair_and_close_session: queued Unpair notification; sink removed from fanout"
            );
            // Step 2 (CopyPaste-qw1k): tx drops here (end of match arm). The
            // `peer_sinks` map entry is gone, so no new fanout items reach this
            // peer. The pump will send the queued Unpair to the peer; the peer
            // then closes its connection (cooperative), our framed.next() returns
            // EOF, and the pump exits — completing the TCP-level session teardown
            // within one RTT.
            true
        }
    }
}

/// Evict a peer from the local persistent store and live allowlist on receipt
/// of an authenticated unpair signal.
///
/// This is the **receive side** of mutual unpair.  `peer_fp` is the mTLS
/// certificate fingerprint that the TLS transport verified before any data was
/// exchanged — it is the only input used for the eviction, so a misbehaving
/// peer cannot cause another peer's record to be removed.
///
/// Best-effort: file-system or parse failures are logged but do not return an
/// error — the calling connection task exits regardless, ensuring the local
/// mTLS transport will refuse further reconnects from this peer once the
/// allowlist entry is gone.
///
/// `live_peers` is the daemon's live, interior-mutable mTLS allowlist. When
/// supplied, the peer is ALSO removed from it (Gap B fix) so the stale mTLS
/// allowlist entry is gone immediately — without waiting for a daemon restart.
/// Passing `None` (as the unit tests do for the file-only path) skips that step.
pub(super) fn evict_peer_local(peer_fp: &str, live_peers: Option<&PairedPeers>) {
    let peers_path = crate::ipc::peers_file_path();
    let mut peers = crate::peers::load_peers(&peers_path);
    let before = peers.len();
    // Normalise stored colon-hex fingerprints before comparing, because the
    // P2P layer reports colon-free hex (the canonical form used here).
    let canonical_target = peer_fp.to_ascii_lowercase();
    peers.retain(|p| crate::ipc::canonical_fingerprint(&p.fingerprint) != canonical_target);
    let removed = peers.len() < before;
    if let Err(e) = crate::peers::save_peers(&peers_path, &peers) {
        tracing::warn!(
            peer = %peer_fp,
            "evict_peer_local: failed to save peers.json after unpair signal: {e}"
        );
    } else if removed {
        tracing::info!(peer = %peer_fp, "evict_peer_local: peer removed from peers.json");
    }

    // Gap B fix: the persisted file alone is not enough — the live mTLS
    // allowlist (`PairedPeers`, shared with the transport's cert verifier) must
    // ALSO drop this fingerprint, or the unpaired peer keeps being accepted on
    // every subsequent handshake until the daemon restarts.
    if let Some(live) = live_peers {
        live.remove(&canonical_target);
        tracing::info!(
            peer = %peer_fp,
            "evict_peer_local: peer removed from live PairedPeers allowlist"
        );
    }
}

/// Stamp first/last sync timestamps for a freshly-established peer connection.
///
/// Called ONCE per established connection (both the accept and connector paths),
/// right after the sync-on-connect catch-up. This per-connection cadence is the
/// throttle: `peers.json` is rewritten when a link comes up, never per synced
/// item, so there is no write amplification under a busy stream.
///
/// `peer_fp` is the verified mTLS certificate fingerprint (colon-free hex);
/// [`crate::peers::touch_sync_times`] canonicalises it against the colon-hex
/// form stored in `peers.json`. A missing peer record or a write failure only
/// logs at `debug` — sync-time stamping is best-effort and must never disrupt
/// the live connection.
pub(super) fn stamp_peer_sync(peers_path: &std::path::Path, peer_fp: &DeviceFingerprint) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    if let Err(e) = crate::peers::touch_sync_times(peers_path, peer_fp, now) {
        tracing::debug!(%peer_fp, "failed to stamp peer sync times: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    // ── Mutual unpair ─────────────────────────────────────────────────────────

    /// Gap B (pure unit): `evict_peer_local` with a live `PairedPeers` supplied
    /// must remove the fingerprint from BOTH `peers.json` and the live allowlist.
    #[test]
    fn gap_b_evict_peer_local_unit() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("peers.json");
        crate::peers::save_peers(
            &path,
            &[crate::peers::PairedDevice {
                fingerprint: "aa:bb:cc".to_string(),
                name: "Alice".to_string(),
                added_at: 1_000,
                address: Some("10.0.0.1:1111".to_string()),
                sync_key_b64: None,
                model: None,
                os_version: None,
                app_version: None,
                local_ip: None,
                public_ip: None,
                first_sync_at: None,
                last_sync_at: None,
                password_file_b64: None,
                password_file_enc: None,
                supabase_account_id: None,
            }],
        )
        .unwrap();

        let live = PairedPeers::new();
        live.add("aabbcc", "Alice");
        assert!(
            live.is_known("aabbcc"),
            "precondition: live allowlist has Alice"
        );

        let env_lock = crate::TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let prev = std::env::var_os("COPYPASTE_CONFIG_DIR");
        // SAFETY: serialised via TEST_ENV_LOCK.
        unsafe {
            std::env::set_var("COPYPASTE_CONFIG_DIR", tmp.path());
        }

        evict_peer_local("aabbcc", Some(&live));

        unsafe {
            match prev {
                Some(v) => std::env::set_var("COPYPASTE_CONFIG_DIR", v),
                None => std::env::remove_var("COPYPASTE_CONFIG_DIR"),
            }
        }
        drop(env_lock);

        // File: Alice removed.
        let loaded = crate::peers::load_peers(&path);
        assert!(
            loaded.is_empty(),
            "Gap B: peers.json must no longer contain Alice"
        );
        // Live allowlist: Alice removed.
        assert!(
            !live.is_known("aabbcc"),
            "Gap B: live PairedPeers must no longer contain Alice"
        );
    }

    /// `evict_peer_local` removes the matching peer from `peers.json` and
    /// leaves all other records intact.  The eviction is keyed to the
    /// mTLS-authenticated fingerprint (canonical, colon-free hex); the function
    /// must not touch any other record even when the stored form uses colon-hex.
    #[test]
    fn evict_peer_local_removes_only_the_authenticated_peer() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("peers.json");

        // Store two peers in colon-hex form (the standard peers.json format).
        crate::peers::save_peers(
            &path,
            &[
                crate::peers::PairedDevice {
                    fingerprint: "aa:bb:cc".to_string(),
                    name: "Alice".to_string(),
                    added_at: 1_000,
                    address: Some("10.0.0.1:1111".to_string()),
                    sync_key_b64: None,
                    model: None,
                    os_version: None,
                    app_version: None,
                    local_ip: None,
                    public_ip: None,
                    first_sync_at: None,
                    last_sync_at: None,
                    password_file_b64: None,
                    password_file_enc: None,
                    supabase_account_id: None,
                },
                crate::peers::PairedDevice {
                    fingerprint: "dd:ee:ff".to_string(),
                    name: "Bob".to_string(),
                    added_at: 2_000,
                    address: None,
                    sync_key_b64: None,
                    model: None,
                    os_version: None,
                    app_version: None,
                    local_ip: None,
                    public_ip: None,
                    first_sync_at: None,
                    last_sync_at: None,
                    password_file_b64: None,
                    password_file_enc: None,
                    supabase_account_id: None,
                },
            ],
        )
        .unwrap();

        // Set up the env so `evict_peer_local` resolves to our temp dir.
        let env_lock = crate::TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let prev = std::env::var_os("COPYPASTE_CONFIG_DIR");
        // SAFETY: serialised via TEST_ENV_LOCK.
        unsafe {
            std::env::set_var("COPYPASTE_CONFIG_DIR", tmp.path());
        }

        // Evict Alice using the canonical (colon-free) form of her fingerprint,
        // exactly as the mTLS layer would provide it.
        evict_peer_local("aabbcc", None);

        // Restore env before any assertions that might panic.
        unsafe {
            match prev {
                Some(v) => std::env::set_var("COPYPASTE_CONFIG_DIR", v),
                None => std::env::remove_var("COPYPASTE_CONFIG_DIR"),
            }
        }
        drop(env_lock);

        let loaded = crate::peers::load_peers(&path);
        assert_eq!(loaded.len(), 1, "Alice must have been removed");
        assert_eq!(
            loaded[0].name, "Bob",
            "Bob must remain untouched after Alice's eviction"
        );
    }

    /// Receiving an `Unpair` signal from a peer whose fingerprint does NOT
    /// match any stored record is a no-op: `peers.json` is unchanged and the
    /// call does not panic.
    #[test]
    fn evict_peer_local_unknown_fp_is_noop() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("peers.json");
        crate::peers::save_peers(
            &path,
            &[crate::peers::PairedDevice {
                fingerprint: "aa:bb:cc".to_string(),
                name: "Alice".to_string(),
                added_at: 1_000,
                address: None,
                sync_key_b64: None,
                model: None,
                os_version: None,
                app_version: None,
                local_ip: None,
                public_ip: None,
                first_sync_at: None,
                last_sync_at: None,
                password_file_b64: None,
                password_file_enc: None,
                supabase_account_id: None,
            }],
        )
        .unwrap();

        let env_lock = crate::TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let prev = std::env::var_os("COPYPASTE_CONFIG_DIR");
        unsafe {
            std::env::set_var("COPYPASTE_CONFIG_DIR", tmp.path());
        }

        // "deadbeef" has no stored record — must be a silent no-op.
        evict_peer_local("deadbeef", None);

        unsafe {
            match prev {
                Some(v) => std::env::set_var("COPYPASTE_CONFIG_DIR", v),
                None => std::env::remove_var("COPYPASTE_CONFIG_DIR"),
            }
        }
        drop(env_lock);

        let loaded = crate::peers::load_peers(&path);
        assert_eq!(loaded.len(), 1, "Alice must be untouched");
        assert_eq!(loaded[0].name, "Alice");
    }

    /// `send_unpair_and_close_session` returns `false` and is a no-op when the
    /// peer has no live session (already disconnected or offline).
    #[tokio::test]
    async fn send_unpair_and_close_session_noop_when_offline() {
        let peer_sinks: PeerSinks = Arc::new(Mutex::new(HashMap::new()));
        let result = send_unpair_and_close_session(&peer_sinks, "deadbeef").await;
        assert!(
            !result,
            "CopyPaste-qw1k: must return false when peer has no live session"
        );
        assert!(
            peer_sinks.lock().await.is_empty(),
            "map must remain empty after noop call"
        );
    }
}
