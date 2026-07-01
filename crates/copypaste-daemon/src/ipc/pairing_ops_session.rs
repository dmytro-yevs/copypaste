//! PAKE session bookkeeping + own-sync-addr helpers (split from pairing_ops.rs,
//! ADR-017 daemon-ipc track, CopyPaste-vp63.17).
use super::*;

impl IpcServer {
    /// Insert a PAKE session under `session_id`, first evicting stale and
    /// excess sessions (fix/p2p-c-review #1 — DoS).
    ///
    /// Eviction policy, applied on every insert:
    /// 1. Drop any session older than [`PAKE_SESSION_TTL`].
    /// 2. If still at/above [`MAX_PAKE_SESSIONS`], reject the new session with
    ///    `Err` so the caller can surface a clear error instead of growing the
    ///    map without bound.
    ///
    /// On success returns `Ok(())` with the timestamped session stored.
    pub(crate) async fn insert_pake_session(
        &self,
        session_id: String,
        session: PakeSession,
    ) -> Result<(), &'static str> {
        let now = std::time::Instant::now();
        let mut sessions = self.pake_sessions.lock().await;

        // 1. Evict stale sessions (TTL).
        sessions.retain(|_, s| now.duration_since(s.created_at) < PAKE_SESSION_TTL);

        // 2. Enforce the hard cap. Reuse of an existing id (should not happen —
        //    ids are fresh UUIDs) overwrites in place and does not grow the map.
        if !sessions.contains_key(&session_id) && sessions.len() >= MAX_PAKE_SESSIONS {
            tracing::warn!(
                live = sessions.len(),
                cap = MAX_PAKE_SESSIONS,
                "rejecting new PAKE session: live-session cap reached"
            );
            return Err("too many in-flight pairing sessions; try again shortly");
        }

        sessions.insert(
            session_id,
            StampedPakeSession {
                session,
                created_at: now,
            },
        );
        Ok(())
    }

    /// Register a freshly-paired peer in the live mTLS allowlist so the accept
    /// loop honours it immediately, with no daemon restart (fix/p2p-c-review #2).
    ///
    /// `peer_fingerprint` is the user-facing colon-hex form; it is normalised
    /// to the canonical lowercase, colon-free hex the transport compares
    /// against. We go through [`copypaste_p2p::transport::PairedPeers::rotate_peer`] (rather than `add`)
    /// so the S10 cert-rotation grace path is exercised on the same code path
    /// used for re-pairing; for a first-time pair `old == new`, which `rotate`
    /// treats as a plain add (no superseded entry — nothing to grace).
    ///
    /// No-op when P2P is disabled (`p2p_peers == None`): the PAKE handler has
    /// already persisted the peer to `peers.json`, which `start_p2p` loads on
    /// the next run.
    pub(crate) fn register_live_peer(&self, peer_fingerprint: &str) {
        if let Some(ref peers) = self.p2p_peers {
            let canonical = canonical_fingerprint(peer_fingerprint);
            peers.rotate_peer(&canonical, canonical.clone(), peer_fingerprint);
            tracing::info!(
                fingerprint = %peer_fingerprint,
                "registered paired peer in live P2P allowlist"
            );
        }
    }

    /// This daemon's own P2P sync-listener address (`host:port`), if `start_p2p`
    /// has bound it. Sent in-band over the bootstrap channel so the peer can
    /// persist it for the Phase 3 connector. Returns an empty string when the
    /// port is not yet known (P2P disabled or not yet bound) — the bootstrap
    /// wire tolerates an empty address frame.
    pub(crate) fn own_sync_addr(&self) -> String {
        self.p2p_sync_addr
            .lock()
            .map(|slot| slot.clone().unwrap_or_default())
            .unwrap_or_else(|poisoned| poisoned.into_inner().clone().unwrap_or_default())
    }

    /// Like [`Self::own_sync_addr`] but waits (bounded by `timeout`) for the P2P sync
    /// listener to populate the slot before returning. A pairing initiated
    /// immediately after daemon startup can otherwise race the listener bind and
    /// advertise an EMPTY address, leaving the peer with no reachable sync addr
    /// (it then persists `address: null` and must fall back to unreliable
    /// loopback mDNS). Reading late (after metadata collection) shrank that
    /// window but did not close it on slow/loaded CI runners — hence the flaky
    /// `pairing_persists_..._on_both_sides` failures. Polling the slot makes the
    /// advertised address deterministic: it returns as soon as the listener
    /// binds, and on timeout returns the (still-empty) string for the same
    /// graceful mDNS-fallback degradation as before.
    pub(crate) async fn await_own_sync_addr(&self, timeout: std::time::Duration) -> String {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            let addr = self.own_sync_addr();
            if !addr.is_empty() || tokio::time::Instant::now() >= deadline {
                return addr;
            }
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
    }

    /// Derive a 32-byte channel-binding token from the two cert fingerprints
    /// involved in an IPC-path PAKE handshake.
    ///
    /// # Security rationale (S3 — IPC pairing path)
    ///
    /// The IPC password-pairing path (`pair_peer_with_password` /
    /// `pair_accept_password` / `pair_accept_finish`) relays PAKE messages
    /// through the UI rather than over a shared TLS connection, so an RFC 5705
    /// `export_keying_material` binder is not available. The next-best binding
    /// is the pair of cert fingerprints the two sides have already agreed to
    /// pin: each device knows its own cert fingerprint and the peer fingerprint
    /// supplied by the UI.
    ///
    /// A relay/MitM that substitutes its own cert will have a different
    /// fingerprint pair → a different binder → a different channel-bound key →
    /// confirmation tags that will not match → the handshake is aborted.
    ///
    /// The binder is the SHA-256 of `min_fp || max_fp` (lexicographic order on
    /// the raw bytes, so both ends produce the same value regardless of which
    /// end calls this function). Domain-separated from the session-key
    /// derivation by the surrounding `SessionKey::bind_to_tls_channel` HKDF
    /// info string, which differs from `derive_xchacha_key`'s info string.
    pub(crate) fn pake_cert_binder(fp_a: &str, fp_b: &str) -> [u8; 32] {
        use sha2::{Digest, Sha256};
        // Canonical order: lexicographic on the UTF-8 bytes so both sides
        // produce the same binder regardless of which is "own" vs "peer".
        let (lo, hi) = if fp_a.as_bytes() <= fp_b.as_bytes() {
            (fp_a.as_bytes(), fp_b.as_bytes())
        } else {
            (fp_b.as_bytes(), fp_a.as_bytes())
        };
        let mut h = Sha256::new();
        h.update(b"copypaste/p2p/ipc-cert-binder/v1\x00");
        h.update(lo);
        h.update(b"\x00");
        h.update(hi);
        h.finalize().into()
    }
}
