//! `IpcServer` constructor + builder methods (split from ipc god-module, ra15.1).
use super::*;

impl IpcServer {
    pub fn new(
        db: Arc<Mutex<Database>>,
        private_mode: Arc<AtomicBool>,
        local_key: Arc<zeroize::Zeroizing<[u8; 32]>>,
        device_public_key: Arc<[u8; 32]>,
    ) -> Self {
        Self::new_with_ready(
            db,
            private_mode,
            local_key,
            device_public_key,
            Arc::new(AtomicBool::new(true)),
        )
    }

    /// Mark this server as serving a degraded startup (e.g. keychain-locked /
    /// db-unavailable). The reason is echoed in the `status` response so the UI
    /// can show a recovery banner. Pair this with `new_with_ready(.., false)`
    /// so DB-touching methods return `IPC_NOT_READY`.
    pub fn with_degraded_reason(self, reason: impl Into<String>) -> Self {
        // Poisoned mutex (a prior panic while holding the lock) is recovered:
        // the slot holds only a non-secret reason string.
        *self
            .degraded_reason
            .lock()
            .unwrap_or_else(|p| p.into_inner()) = Some(reason.into());
        self
    }

    /// Attach the live mTLS certificate fingerprint that pairing advertises.
    ///
    /// CRITICAL-1: this MUST be the fingerprint of the same cert the running
    /// `PeerTransport` presents (`display_fingerprint(transport.fingerprint())`)
    /// so a scanning/pairing peer pins a value the mTLS layer actually compares
    /// against. The daemon generates the cert once and hands the same cert to
    /// `start_p2p` and the colon-hex fingerprint here, guaranteeing they agree.
    pub fn with_cert_fingerprint(mut self, fingerprint: impl Into<String>) -> Self {
        self.cert_fingerprint = Some(fingerprint.into());
        self
    }

    /// Attach the stable per-device UUID so `history_page` can return it as
    /// `own_device_id`. The UI uses this to label locally-captured items as
    /// "This device" vs. items synced from a remote peer.
    pub fn with_local_device_id(mut self, id: impl Into<String>) -> Self {
        self.local_device_id = Some(id.into());
        self
    }

    /// Attach the live P2P paired-peer allowlist (fix/p2p-c-review #2).
    ///
    /// The daemon shares the same `PairedPeers` instance with the running mTLS
    /// transport; supplying it here lets the PAKE finish handlers register a
    /// freshly-paired peer in-memory so the accept loop honours it without a
    /// daemon restart.
    pub fn with_p2p_peers(mut self, peers: copypaste_p2p::transport::PairedPeers) -> Self {
        self.p2p_peers = Some(peers);
        self
    }

    /// Return the slot that daemon.rs writes `P2pHandle::live_sinks` into after
    /// `start_p2p` returns.
    ///
    /// Two consumers share this slot:
    /// - `list_peers` iterates it to compute the authoritative online flag from
    ///   live connection state rather than the stale mTLS-allowlist heuristic.
    /// - `unpair_peer` / `revoke_peer` / `revoke_all_peers` look up a specific
    ///   peer's sender and deliver a best-effort `ControlMsg::Unpair` signal.
    pub fn live_peer_sinks_slot(&self) -> Arc<std::sync::Mutex<Option<crate::p2p::LivePeerSinks>>> {
        Arc::clone(&self.live_peer_sinks)
    }

    /// Return the slot that daemon.rs writes `P2pHandle::peer_rtt_ms` into
    /// after `start_p2p` returns.  The `list_peers` handler reads from this
    /// to add `latency_ms` to each peer entry.
    pub fn live_peer_rtt_ms_slot(&self) -> Arc<std::sync::Mutex<Option<crate::p2p::PeerRttMs>>> {
        Arc::clone(&self.live_peer_rtt_ms)
    }

    /// Return the shared peer-event queue that `daemon.rs` enqueues into and
    /// the `poll_peer_events` IPC handler drains.
    pub fn peer_event_queue(
        &self,
    ) -> Arc<std::sync::Mutex<std::collections::VecDeque<PeerEventRecord>>> {
        Arc::clone(&self.peer_event_queue)
    }

    /// Return the slot that `daemon.rs` can write the P2P subsystem's
    /// `CancellationToken` into after `start_p2p` returns (CopyPaste-ydhw).
    ///
    /// When populated, `rescan_discovered` wraps the replacement mDNS-SD browse
    /// task in a `select!` that respects P2P shutdown, preventing the browse
    /// from outliving the P2P subsystem.  Follows the same lazy-injection
    /// pattern as [`live_peer_sinks_slot`](Self::live_peer_sinks_slot).
    ///
    /// `None` means P2P is disabled or `start_p2p` has not yet returned.
    pub fn p2p_shutdown_token_slot(&self) -> Arc<std::sync::Mutex<Option<CancellationToken>>> {
        Arc::clone(&self.p2p_shutdown_token)
    }

    /// Attach a clone of the running sync orchestrator's `SyncCrypto` context
    /// (H8 perf fix). Because `SyncCrypto` stores its cached sync key behind an
    /// `Arc<Mutex>`, this clone shares the SAME backing store as the
    /// orchestrator's copy; calling `reload_sync_key()` here after a pairing
    /// write propagates to the orchestrator without any channel or restart.
    pub fn with_p2p_sync_crypto(mut self, crypto: crate::sync_orch::SyncCrypto) -> Self {
        self.p2p_sync_crypto = Some(crypto);
        self
    }

    /// Attach the self-signed mTLS cert (DER) + key used to TLS-wrap the
    /// unauthenticated bootstrap pairing channel (P2P Phase 1).
    ///
    /// MUST be a clone of the exact cert `start_p2p`'s transport presents (and
    /// whose fingerprint `with_cert_fingerprint` advertises) so the fingerprints
    /// a peer learns over the bootstrap channel match what the pinned mTLS layer
    /// later compares.
    pub fn with_p2p_cert(mut self, cert_der: Vec<u8>, key_der: Vec<u8>) -> Self {
        self.p2p_cert = Some(Arc::new((cert_der, key_der)));
        self
    }

    /// Attach the mDNS discovery handle used as the QR-accept fallback when the
    /// QR carries no `addr_hint`.
    pub fn with_discovery(
        mut self,
        discovery: Arc<copypaste_p2p::discovery::DiscoveryService>,
    ) -> Self {
        self.discovery = Some(discovery);
        self
    }

    /// Return a clone of the shared discovery-pairing coordinator (LAN/SAS
    /// Phase 2).
    ///
    /// `start_p2p`'s standing discovery-pairing responder routes its SAS
    /// confirmation through the SAME coordinator the IPC handlers observe, so
    /// the responder user confirms via `pair_get_sas`/`pair_confirm_sas` exactly
    /// like the initiator. The daemon calls this before moving the server into
    /// its task and hands the clone to `start_p2p`.
    pub fn pairing_coordinator(&self) -> Arc<crate::pairing_sm::PairingCoordinator> {
        Arc::clone(&self.pairing)
    }

    /// Return a handle to the shared slot holding this daemon's own P2P
    /// sync-listener address (`host:port`).
    ///
    /// The IPC server is constructed before `start_p2p` binds its accept loop,
    /// so the OS-assigned port is not known yet. The daemon calls
    /// [`set_p2p_sync_addr`](Self::set_p2p_sync_addr) (via this same Arc) once
    /// `start_p2p` returns the bound port; the pairing handlers then read it and
    /// send it in-band over the bootstrap channel. Returning the Arc lets the
    /// daemon populate the slot after the server has been moved into its task.
    pub fn p2p_sync_addr_slot(&self) -> Arc<std::sync::Mutex<Option<String>>> {
        Arc::clone(&self.p2p_sync_addr)
    }

    /// Populate the shared slot with this daemon's bound P2P sync-listener
    /// address. Convenience wrapper over [`p2p_sync_addr_slot`](Self::p2p_sync_addr_slot)
    /// for callers that still hold the server (e.g. tests).
    ///
    /// A poisoned mutex (a prior panic while holding the lock) is recovered
    /// rather than propagated — the slot holds only a non-secret address string,
    /// so reusing it after a panic is safe and keeps pairing functional.
    pub fn set_p2p_sync_addr(&self, addr: impl Into<String>) {
        let mut slot = self
            .p2p_sync_addr
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *slot = Some(addr.into());
    }

    /// Wire up shared cloud-sync state created by the daemon before spawning
    /// the IPC server and `start_cloud`.
    ///
    /// By calling this the daemon guarantees both surfaces see the **same**
    /// `Arc`s: a `set_sync_passphrase` IPC call writes to the same
    /// `sync_key` `Mutex` that the cloud push/poll loops read from, and the
    /// cloud loops write to the same `last_sync_ms` counter that
    /// `get_sync_status` reads.
    #[cfg(any(feature = "cloud-sync", feature = "relay-sync"))]
    pub fn with_cloud_sync_state(
        mut self,
        sync_key: Arc<Mutex<Option<SyncKey>>>,
        last_sync_ms: Arc<std::sync::atomic::AtomicI64>,
        cloud_signed_in: Arc<AtomicBool>,
    ) -> Self {
        self.sync_key = sync_key;
        self.last_sync_ms = last_sync_ms;
        self.cloud_signed_in = cloud_signed_in;
        self
    }

    /// Wire the shared **v2 per-account-salt** cloud key slot (CopyPaste-jdq5).
    ///
    /// The daemon creates this `Arc<Mutex<Option<SyncKey>>>` (restoring any
    /// persisted v2 key) BEFORE spawning both the IPC server and `start_cloud`,
    /// then passes the SAME `Arc` to both so a `set_sync_passphrase` IPC call
    /// installs the freshly-derived v2 key into the exact slot the cloud loops
    /// read for dual-key dispatch.
    #[cfg(feature = "cloud-sync")]
    pub fn with_cloud_sync_key_v2(mut self, sync_key_v2: Arc<Mutex<Option<SyncKey>>>) -> Self {
        self.sync_key_v2 = sync_key_v2;
        self
    }

    /// Wire the canonical Supabase account-identity slot (CopyPaste-1jms.34).
    ///
    /// The caller creates a shared `Arc<Mutex<Option<String>>>` BEFORE spawning
    /// the server, passes it here, and then retains its own clone to write the
    /// value produced by `start_cloud` *after* the server has been spawned.
    /// The `get_sync_status` handler reads through the same `Arc` on every
    /// request — writing once at startup is sufficient and race-free.
    #[cfg(feature = "cloud-sync")]
    pub fn with_cloud_account_id_slot(
        mut self,
        slot: Arc<std::sync::Mutex<Option<String>>>,
    ) -> Self {
        self.cloud_account_id = slot;
        self
    }

    /// Return a clone of the `Arc<Mutex<Option<String>>>` holding the local
    /// Supabase account identity so callers (e.g. the P2P standing responder)
    /// can read it without coupling to the full `IpcServer`.
    ///
    /// CopyPaste-yw2k: the arc is cloned once at startup and forwarded to
    /// `start_p2p` so the standing responder can include our `supabase_account_id`
    /// in the `PeerMeta` it sends during in-band pairing.
    #[cfg(feature = "cloud-sync")]
    pub fn cloud_account_id_slot(&self) -> Arc<std::sync::Mutex<Option<String>>> {
        Arc::clone(&self.cloud_account_id)
    }

    /// Attach the broadcast sender for newly-ingested clipboard items so the
    /// `import` IPC method can notify the sync orchestrator (P2P Phase 3).
    pub fn with_new_item_tx(
        mut self,
        tx: tokio::sync::broadcast::Sender<copypaste_core::ClipboardItem>,
    ) -> Self {
        self.new_item_tx = Some(tx);
        self
    }

    /// Construct with an explicit readiness flag. The returned handle can be
    /// flipped to `true` once initialization completes. Intended for tests
    /// and for callers that want to bind the socket before the database is
    /// fully open.
    #[allow(dead_code)]
    pub fn new_with_ready(
        db: Arc<Mutex<Database>>,
        private_mode: Arc<AtomicBool>,
        local_key: Arc<zeroize::Zeroizing<[u8; 32]>>,
        device_public_key: Arc<[u8; 32]>,
        ready: Arc<AtomicBool>,
    ) -> Self {
        Self {
            db,
            read_pool: std::sync::Mutex::new(None),
            private_mode,
            private_mode_epoch: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            local_device_id: None,
            local_key,
            device_public_key,
            ready,
            pake_sessions: Arc::new(Mutex::new(HashMap::new())),
            pending_qr_token: Arc::new(Mutex::new(None)),
            p2p_peers: None,
            cert_fingerprint: None,
            p2p_cert: None,
            discovery: None,
            p2p_sync_addr: Arc::new(std::sync::Mutex::new(None)),
            self_write_change_count: Arc::new(std::sync::atomic::AtomicI64::new(-1)),
            #[cfg(any(feature = "cloud-sync", feature = "relay-sync"))]
            sync_key: Arc::new(Mutex::new(None)),
            // CopyPaste-jdq5: v2 per-account cloud key slot — populated by
            // `set_sync_passphrase` when a Supabase account id is known, wired to
            // the cloud loops via `with_cloud_sync_key_v2`.
            #[cfg(feature = "cloud-sync")]
            sync_key_v2: Arc::new(Mutex::new(None)),
            #[cfg(any(feature = "cloud-sync", feature = "relay-sync"))]
            last_sync_ms: Arc::new(std::sync::atomic::AtomicI64::new(0)),
            #[cfg(any(feature = "cloud-sync", feature = "relay-sync"))]
            cloud_signed_in: Arc::new(AtomicBool::new(false)),
            cloud_account_id: Arc::new(std::sync::Mutex::new(None)),
            new_item_tx: None,
            degraded_reason: Arc::new(std::sync::Mutex::new(None)),
            core_config: None,
            cached_public_ip: Arc::new(tokio::sync::RwLock::new(None)),
            pairing: Arc::new(crate::pairing_sm::PairingCoordinator::new()),
            live_peer_sinks: Arc::new(std::sync::Mutex::new(None)),
            live_peer_rtt_ms: Arc::new(std::sync::Mutex::new(None)),
            p2p_sync_crypto: None,
            pending_bootstrap: Arc::new(tokio::sync::Mutex::new(None)),
            peer_event_queue: Arc::new(std::sync::Mutex::new(std::collections::VecDeque::new())),
            discovery_browse_handle: Arc::new(std::sync::Mutex::new(None)),
            p2p_shutdown_token: Arc::new(std::sync::Mutex::new(None)),
            // nq39: initialise to None; populated by `store_cloud_password`
            // on non-macOS platforms where the Keychain is unavailable.
            #[cfg(not(target_os = "macos"))]
            in_memory_cloud_password: Arc::new(std::sync::Mutex::new(None)),
            // CopyPaste-6ot5: start with the full connection cap available.
            conn_semaphore: Arc::new(tokio::sync::Semaphore::new(MAX_CONCURRENT_CONNECTIONS)),
            // CopyPaste-44rq.67: empty until daemon::run starts the relay and
            // wires the shared slot via `with_relay_handle`.
            #[cfg(feature = "relay-sync")]
            relay_handle: Arc::new(tokio::sync::Mutex::new(None)),
            // CopyPaste-1jms.22: starts false (no sync in progress at
            // construction time). The daemon replaces this with a shared Arc via
            // `with_sync_in_flight` before spawning the sync loops.
            sync_in_flight: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Wire in a read connection pool (CopyPaste-j8p).
    ///
    /// Read-only handlers (`list`, `count`, `search`, `history_page`, `stats`)
    /// will acquire connections from `pool` instead of locking `self.db`,
    /// allowing concurrent reads without blocking the writer.
    pub fn with_read_pool(self, pool: Arc<copypaste_core::SqlitePool>) -> Self {
        *self.read_pool.lock().unwrap_or_else(|p| p.into_inner()) = Some(pool);
        self
    }

    /// CopyPaste-crh3.86: run a read-only DB closure on the blocking pool with
    /// the canonical pool-then-writer-lock fallback, in ONE place.
    ///
    /// Every read IPC handler previously copy-pasted ~15 lines: clone the read
    /// pool + `self.db`, `spawn_blocking`, try `pool.get()` → [`copypaste_core::ReadHandle`] else
    /// `db.blocking_lock()`, run the query, then `await`/join. Any fix to the
    /// fallback (or its error mapping) had to be applied at every site or the
    /// behaviour silently diverged. This helper centralises it; callers pass a
    /// closure over `&dyn DbRead` (the read fns are generic over `DbRead`, so the
    /// SAME closure body serves both the pooled connection and the writer lock —
    /// removing the in-branch duplication too) and map the returned value to a
    /// [`Response`].
    ///
    /// Returns `Err(String)` only when the blocking task itself failed (panic /
    /// runtime shutdown); the inner `Result<T, E>` is the query outcome.
    pub(crate) async fn with_read_db<T, E, F>(&self, f: F) -> Result<Result<T, E>, String>
    where
        F: FnOnce(&dyn DbRead) -> Result<T, E> + Send + 'static,
        T: Send + 'static,
        E: Send + 'static,
    {
        let pool_opt = self
            .read_pool
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        let db_arc = self.db.clone();
        tokio::task::spawn_blocking(move || {
            if let Some(pool) = pool_opt {
                if let Ok(conn) = pool.get() {
                    return f(&copypaste_core::ReadHandle(conn));
                }
            }
            let db = db_arc.blocking_lock();
            f(&*db)
        })
        .await
        .map_err(|e| format!("blocking task failed: {e}"))
    }

    /// Share the live relay-handle slot with `daemon::run` (CopyPaste-44rq.67).
    ///
    /// `daemon::run` owns the same `Arc` and writes the started
    /// [`crate::relay::RelayHandle`] into it after `start_relay` succeeds; the
    /// `set_config` handler reads it to shut the relay down when the user clears
    /// the relay URL. Passing the shared slot (rather than the default
    /// per-server empty one) is what connects the two.
    #[cfg(feature = "relay-sync")]
    pub fn with_relay_handle(
        mut self,
        slot: Arc<tokio::sync::Mutex<Option<crate::relay::RelayHandle>>>,
    ) -> Self {
        self.relay_handle = slot;
        self
    }

    /// Wire the shared in-flight sync flag (CopyPaste-1jms.22).
    ///
    /// `daemon::run` allocates the `Arc<AtomicBool>` once, clones it into each
    /// sync loop (cloud poll, cloud push, relay receive, relay push, P2P
    /// handshake), and passes another clone here so `get_sync_status` reads the
    /// SAME flag that the sync loops flip via [`crate::sync_in_flight::SyncInFlightGuard`].
    ///
    /// When not wired (unit tests, degraded mode), the field defaults to a local
    /// `Arc::new(AtomicBool::new(false))` that is never flipped, so
    /// `badge_state` is unaffected.
    pub fn with_sync_in_flight(mut self, flag: Arc<AtomicBool>) -> Self {
        self.sync_in_flight = flag;
        self
    }

    /// Attach the shared live core config (`config.toml`) for hot-reload.
    ///
    /// The `set_config` IPC handler writes updated limit/feature values into this
    /// Arc after persisting to disk, so the clipboard monitor, paste path, and
    /// prune code pick them up on the next tick without a daemon restart.
    pub fn with_core_config(
        mut self,
        core_config: Arc<std::sync::RwLock<copypaste_core::AppConfig>>,
    ) -> Self {
        self.core_config = Some(core_config);
        self
    }

    /// Share the pre-allocated public-IP cache slot with the daemon's
    /// STUN-refresh background task.
    ///
    /// The daemon creates the `Arc<RwLock<…>>`, passes it into the IPC server
    /// via this method, and also clones it into the refresh task so both can
    /// write to / read from the same slot without a process-wide lock.
    pub fn with_public_ip_cache(mut self, cache: Arc<tokio::sync::RwLock<Option<String>>>) -> Self {
        self.cached_public_ip = cache;
        self
    }
}
