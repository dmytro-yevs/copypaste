//! `IpcServer` struct definition + `PeerEventRecord` (split from ipc/mod.rs,
//! ADR-017 daemon-ipc track, CopyPaste-vp63.19). Only the STRUCT DEFINITION
//! lives here — the `impl IpcServer` blocks (builder, connection, dispatch,
//! handlers, pairing_ops) remain in their existing sibling files; Rust allows
//! inherent impls in any module of the same crate as long as the type is in
//! scope via the `pub use server::IpcServer;` re-export in mod.rs.
use super::*;

pub struct IpcServer {
    pub(super) db: Arc<Mutex<Database>>,
    /// Optional r2d2 connection pool for concurrent read-only queries (CopyPaste-j8p).
    ///
    /// When present, the read-only handlers (`list`, `count`, `search`,
    /// `history_page`, `stats`) acquire a pooled connection and bypass the
    /// single write mutex, allowing N parallel reads without serializing on
    /// the clipboard-write path. SQLite WAL mode guarantees readers always
    /// see committed data without blocking the writer.
    ///
    /// Falls back to `self.db` (write mutex) when `None` (degraded startup,
    /// tests that don't need pool concurrency, or pool exhaustion).
    ///
    /// Wrapped in a `std::sync::Mutex` so `db_restore` can atomically rebuild
    /// the pool against the restored database file (CopyPaste-crh3.2). The
    /// pooled connections hold file descriptors to the *old* inode; after a
    /// restore swaps the on-disk DB they must be replaced or every read keeps
    /// serving pre-restore data. The lock is only ever held long enough to
    /// `clone()` the inner `Arc` (no `.await` across the guard).
    pub(super) read_pool: std::sync::Mutex<Option<Arc<copypaste_core::SqlitePool>>>,
    /// Shared private-mode flag. When true, the clipboard monitor skips recording.
    pub(super) private_mode: Arc<AtomicBool>,
    /// Monotonically-increasing epoch counter for the private-mode flag.
    ///
    /// CopyPaste-48k0: the tray's `spawn_tray_private_mode_resync` helper is a
    /// one-shot poller — it exits after a stable round-trip and never re-runs.
    /// After a daemon restart the tray's cached state may be stale (the new
    /// daemon loaded private-mode from disk but the tray already exited its
    /// poller).
    ///
    /// Fix: expose this counter in the `status` and `get_private_mode` responses
    /// so any periodic `status` poll (e.g. the UI's health check) can detect that
    /// private-mode changed and trigger a re-sync.  The counter starts at 0 and
    /// is incremented on every `set_private_mode` call, making it cheap to compare
    /// across polls: a changed epoch → re-read `private_mode`.
    pub(super) private_mode_epoch: Arc<std::sync::atomic::AtomicU64>,
    /// Stable device UUID loaded (or created) at daemon start via
    /// `load_or_create_device_id`. Stamped on every locally-captured clipboard
    /// item as `origin_device_id`. Returned in `history_page` as `own_device_id`
    /// so the UI can label "This device" vs. synced items from other devices.
    /// `None` when not wired in (unit tests / degraded-mode builds).
    pub(super) local_device_id: Option<String>,
    /// Local symmetric encryption key (XChaCha20-Poly1305). Required by the
    /// `copy`/`paste` handlers so paste-back can decrypt the ciphertext
    /// stored in `clipboard_items.content` and write *plaintext* to
    /// NSPasteboard. Audit CRIT #1: previously the handler wrote raw
    /// ciphertext bytes back, so paste produced "content is not valid
    /// UTF-8" for text and garbage for images.
    pub(super) local_key: Arc<zeroize::Zeroizing<[u8; 32]>>,
    /// Device public-key bytes (X25519). Historically `get_own_fingerprint`
    /// derived its value from this via `keychain::own_fingerprint` (audit HIGH
    /// #6, superseding an unstable DefaultHasher scheme). CRITICAL-1: pairing
    /// now advertises the mTLS **cert** fingerprint (`cert_fingerprint`)
    /// instead, since the device-key fingerprint is never what the mTLS layer
    /// pins. The bytes are retained here — they remain part of the
    /// `IpcServer::new` contract and the device identity is still useful for
    /// future non-pairing surfaces.
    // The X25519 device public-key bytes (32 bytes). SHA-256 of this value is
    // surfaced in the `status` response as `device_key_fingerprint` (hex) so
    // operators and diagnostic tooling can correlate daemon identity without
    // reading the Keychain.  NOTE: pairing uses the mTLS cert fingerprint
    // (`cert_fingerprint`), not this value — they must never be confused.
    pub(super) device_public_key: Arc<[u8; 32]>,
    /// Readiness gate. While `false`, all data-touching methods return
    /// `IPC_NOT_READY` instead of dispatching. Default `true` for production
    /// use (db is fully constructed before `IpcServer::new` is called); tests
    /// use [`IpcServer::new_with_ready`] to exercise the not-ready path.
    pub(super) ready: Arc<AtomicBool>,
    /// DUP-ON-COPY fix: after `write_to_pasteboard` completes, record the new
    /// NSPasteboard `changeCount` here. The clipboard monitor reads this on
    /// the next tick and skips recording when it matches — preventing the
    /// daemon's own pasteboard writes from being captured as new clipboard events.
    /// Sentinel -1 means "no pending self-write".
    pub self_write_change_count: Arc<std::sync::atomic::AtomicI64>,
    /// In-progress PAKE sessions keyed by session_id UUID string.
    ///
    /// Each entry lives from the first IPC call (initiate / accept) until the
    /// matching finish call consumes it. Bounded against unbounded growth
    /// (fix/p2p-c-review #1 — DoS): entries older than [`PAKE_SESSION_TTL`]
    /// are evicted on every insert, and the live count is capped at
    /// [`MAX_PAKE_SESSIONS`]. See [`IpcServer::insert_pake_session`].
    pub(super) pake_sessions: Arc<Mutex<HashMap<String, StampedPakeSession>>>,
    /// The single active QR-pairing token issued by `pair_generate_qr`, with
    /// its issue time for TTL eviction.
    ///
    /// QR pairing is the displaying-device-is-responder flow: this device
    /// generates a fresh token, renders it in the QR, and stores it here so the
    /// `pair_accept_qr` handler can re-derive the same PAKE password when the
    /// scanning device's `message1` arrives — without the user re-typing
    /// anything. Only one QR is active at a time (regenerating replaces it),
    /// matching the single-token pairing UX. Bounded by [`PAKE_SESSION_TTL`].
    /// `None` until the first `pair_generate_qr` call.
    pub(super) pending_qr_token:
        Arc<Mutex<Option<(copypaste_core::PairingToken, std::time::Instant)>>>,
    /// Live P2P paired-peer allowlist, shared with the running mTLS transport
    /// (fix/p2p-c-review #2). When a PAKE handshake finishes, the newly-paired
    /// peer fingerprint is fed into this same instance via
    /// [`copypaste_p2p::transport::PairedPeers::rotate_peer`] so the accept loop immediately honours it
    /// (the S10 grace path is exercised). `None` when P2P is disabled — the
    /// PAKE handlers then only persist to `peers.json` (loaded on next start).
    pub(super) p2p_peers: Option<copypaste_p2p::transport::PairedPeers>,
    /// Our live mTLS **certificate** fingerprint in user-facing colon-hex form,
    /// i.e. `display_fingerprint(hex(SHA-256(cert_der)))` for the exact same
    /// cert the running `PeerTransport` presents and that peers pin
    /// ([`copypaste_p2p::transport::PeerTransport::fingerprint`] /
    /// [`copypaste_p2p::cert::fingerprint_of`]).
    ///
    /// CRITICAL-1 fix: pairing (`pair_generate_qr`, `get_own_fingerprint`)
    /// MUST advertise this value — NOT the device-key fingerprint
    /// (`keychain::own_fingerprint`, SHA-256 of the X25519 public key), which
    /// the mTLS allowlist never compares against, so cert-pinning could never
    /// match and pairing could never authenticate.
    ///
    /// `None` when P2P is disabled (`COPYPASTE_P2P` unset): no transport runs,
    /// so there is no cert to advertise and the pairing handlers return a clear
    /// error rather than a fingerprint that cannot authenticate any channel.
    pub(super) cert_fingerprint: Option<String>,
    /// Our self-signed mTLS certificate DER + key, used to TLS-wrap the
    /// unauthenticated bootstrap pairing channel (P2P Phase 1). This is a clone
    /// of the SAME cert `start_p2p`'s transport presents and whose fingerprint
    /// `cert_fingerprint` advertises, so the fingerprints a pairing peer learns
    /// over the bootstrap channel match the ones the pinned mTLS layer compares.
    ///
    /// `None` when P2P is disabled — the QR pairing handlers then fall back to
    /// the legacy IPC-relayed PAKE path (no network bootstrap channel).
    pub(super) p2p_cert: Option<Arc<(Vec<u8>, Vec<u8>)>>,
    /// Optional mDNS discovery handle used by the initiator's QR-accept path to
    /// resolve the responder's `host:port` when the QR carries no `addr_hint`
    /// (best-effort fallback — loopback mDNS is unreliable, so `addr_hint` is
    /// the primary path). `None` when P2P discovery is not wired in.
    pub(super) discovery: Option<Arc<copypaste_p2p::discovery::DiscoveryService>>,
    /// This daemon's own P2P sync-listener address (`host:port`), filled once
    /// `start_p2p` has bound its accept loop (the port is OS-assigned, so it is
    /// not known when `IpcServer` is constructed). The pairing handlers send
    /// this value in-band over the bootstrap channel so the peer can persist it
    /// for the Phase 3 outbound connector. A `std::sync::Mutex` (not tokio's) is
    /// used because the critical section is a trivial clone with no `.await`.
    /// Holds `None` until populated, or when P2P is disabled.
    pub(super) p2p_sync_addr: Arc<std::sync::Mutex<Option<String>>>,
    /// Shared passphrase-derived cloud sync key (Argon2id, 32 bytes).
    ///
    /// `None` means the user has not yet configured a sync passphrase, so
    /// cloud upload/download is skipped. Set via `set_sync_passphrase`; shared
    /// with the cloud push/poll loops via `Arc<Mutex<Option<SyncKey>>>`.
    #[cfg(any(feature = "cloud-sync", feature = "relay-sync"))]
    pub sync_key: Arc<Mutex<Option<SyncKey>>>,
    /// Monotonic timestamp (ms since UNIX epoch) of the last successful cloud
    /// sync round-trip. `0` means never synced. Shared with cloud loops so
    /// `get_sync_status` returns a live value.
    #[cfg(any(feature = "cloud-sync", feature = "relay-sync"))]
    pub last_sync_ms: Arc<std::sync::atomic::AtomicI64>,
    /// Real GoTrue auth state, published by the cloud push/poll loops (BUG 2).
    /// `true` once `start_cloud` resolves a bearer, `false` on a bearer-resolution
    /// failure (`CloudError::AuthFailed`) or a failed 401-refresh. Read by
    /// `get_sync_status` so the UI reflects the actual signed-in state instead of
    /// the old hardcoded `signed_in = supabase_configured`.
    #[cfg(any(feature = "cloud-sync", feature = "relay-sync"))]
    pub cloud_signed_in: Arc<AtomicBool>,
    /// Canonical Supabase account identity for this device (CopyPaste-1jms.34).
    ///
    /// Set by `with_cloud_account_id` after `start_cloud` returns. The value
    /// is `copypaste_supabase::supabase_account_id(url, user_id)` — a non-secret
    /// stable identifier derived from the Supabase project URL + GoTrue user UUID.
    ///
    /// The `get_sync_status` handler includes this in the response so the UI can
    /// surface a banner when two paired devices report different account IDs
    /// (= different Supabase projects or different GoTrue accounts).
    ///
    /// `None` when cloud-sync is off, not configured, or anon-key-only
    /// (no GoTrue session). Interior-mutable so it can be updated if the cloud
    /// loops are restarted at runtime without taking the entire IpcServer lock.
    ///
    /// Always present (not cfg-gated): the in-band pairing path reads it
    /// unconditionally to advertise the account id to the peer (it is simply
    /// `None` without cloud-sync), so gating the field would break
    /// `--no-default-features`.
    pub cloud_account_id: Arc<std::sync::Mutex<Option<String>>>,
    /// Broadcast sender for newly-ingested clipboard items, shared with the
    /// clipboard monitor and the sync orchestrator (P2P Phase 3).
    ///
    /// Captured-by-polling items already flow through this channel from the
    /// monitor. The `import` IPC method historically inserted straight into the
    /// DB without notifying anyone, so imported items never reached the sync
    /// orchestrator and could not be pushed to a paired peer. Wiring the sender
    /// here lets `import` broadcast each inserted row so it syncs like a captured
    /// one. `None` when the daemon did not provide a sender (e.g. unit tests).
    pub(super) new_item_tx: Option<tokio::sync::broadcast::Sender<copypaste_core::ClipboardItem>>,
    /// Degraded-startup reason, surfaced verbatim in the `status` response so
    /// the UI can render a recovery banner instead of treating an unreachable
    /// socket as a dead daemon.
    ///
    /// `None` in the normal case (DB opened, key available). `Some(reason)`
    /// when the daemon came up in degraded mode — e.g. the SQLCipher key could
    /// not be obtained from the Keychain (`keychain_locked`) so the existing
    /// encrypted DB could not be opened (`db_unavailable`). In degraded mode
    /// `ready` is `false`, so every DB-touching method already returns
    /// `IPC_NOT_READY`; this field tells the client *why* and that recovery is
    /// possible (re-grant Keychain access, then relaunch). See the
    /// [`DEGRADED_REASON_KEYCHAIN_LOCKED`] constant for the canonical value.
    ///
    /// Interior-mutable (`Arc<Mutex<…>>`) because the `reset_database` recovery
    /// handler clears it in-place — after wiping and recreating a fresh empty DB
    /// it brings the daemon OUT of degraded mode (sets `ready = true`, clears
    /// this reason) without a process restart. A `std::sync::Mutex` (not tokio's)
    /// is used because every critical section is a trivial read/write with no
    /// `.await`.
    pub(super) degraded_reason: Arc<std::sync::Mutex<Option<String>>>,
    /// Shared live core config (`config.toml`). The `set_config` IPC handler
    /// writes new limit/feature values here after persisting to disk so the
    /// clipboard monitor, paste path, and prune code pick them up on the next
    /// tick without a daemon restart.
    /// `None` when not wired in (degraded mode / tests that don't need hot-reload).
    pub core_config: Option<Arc<std::sync::RwLock<copypaste_core::AppConfig>>>,

    /// Best-effort cached public / WAN IP (resolved via STUN on startup, then
    /// refreshed every ~15 minutes by a background task spawned in `daemon.rs`).
    /// `None` before the first resolution attempt completes, on failure, or when
    /// the user has opted out via `AppConfig::collect_public_ip = false`.
    ///
    /// `tokio::sync::RwLock` (not `std::sync::Mutex`) because the
    /// `get_own_device_info` hot path is async and must not block the executor.
    pub cached_public_ip: Arc<tokio::sync::RwLock<Option<String>>>,

    /// Discovery-initiated SAS pairing coordinator (LAN/SAS Phase 2).
    ///
    /// Holds the single-active-pairing state machine plus the confirmation
    /// `oneshot` channel that wires `pair_confirm_sas`/`pair_abort` into the
    /// in-flight bootstrap handshake's `confirm` callback. Shared (`Arc`) with
    /// the standing discovery-pairing responder task in `start_p2p`, so an
    /// inbound pair routes its SAS through the SAME machine the IPC handlers
    /// observe. Always present (the machine is `Idle` when nothing is pairing).
    pub(super) pairing: Arc<crate::pairing_sm::PairingCoordinator>,

    /// Shared live peer-sink map — serves two purposes:
    ///   1. Online-status computation (`list_peers`): iterate to find non-closed senders.
    ///   2. Mutual-unpair signalling (`unpair_peer` / `revoke_peer` / `revoke_all_peers`):
    ///      look up a specific peer's sender and deliver `ControlMsg::Unpair`.
    ///
    /// `LivePeerSinks` and `PeerSinks` are identical type aliases
    /// (`Arc<tokio::sync::Mutex<HashMap<DeviceFingerprint, mpsc::Sender<PeerFrame>>>>`).
    /// `P2pHandle` exposes both names only because they were introduced at different times;
    /// both fields on that struct are `Arc::clone`s of the same underlying map.
    /// daemon.rs writes `P2pHandle::live_sinks` here after `start_p2p` returns.
    pub(super) live_peer_sinks: Arc<std::sync::Mutex<Option<crate::p2p::LivePeerSinks>>>,
    /// Last-measured round-trip times per connected peer (milliseconds).
    ///
    /// The P2P subsystem's ping task writes to this map; `list_peers` reads it
    /// to populate the `latency_ms` field in each peer entry.  Wrapped in an
    /// `Option` (in a `std::sync::Mutex`) for the same lazy-injection pattern as
    /// `live_peer_sinks`: `None` until `start_p2p` returns and writes the value.
    pub(super) live_peer_rtt_ms: Arc<std::sync::Mutex<Option<crate::p2p::PeerRttMs>>>,
    /// Clone of the running sync orchestrator's `SyncCrypto` context (H8).
    ///
    /// Because `SyncCrypto` stores its cached sync key behind an `Arc<Mutex>`,
    /// this clone shares the SAME backing store as the orchestrator's copy.
    /// Calling `reload_sync_key()` here after a pairing write propagates to the
    /// orchestrator immediately without any channel or restart. `None` when P2P
    /// is disabled (no orchestrator crypto context exists).
    pub(super) p2p_sync_crypto: Option<crate::sync_orch::SyncCrypto>,

    /// Race-fix (CopyPaste-7mf): handle for the in-flight QR bootstrap responder
    /// task. `spawn_bootstrap_responder` stores the `JoinHandle` here so that
    /// `list_peers` can await it with a short timeout before reading peers.json.
    /// This guarantees that a caller doing `pair_generate_qr` (responder side)
    /// followed immediately by `list_peers` will see the freshly-persisted peer
    /// once the bootstrap PAKE completes, rather than racing the detached spawn.
    ///
    /// Protected by a `tokio::sync::Mutex` because the critical section includes
    /// an `.await` (waiting on the JoinHandle).
    pub(super) pending_bootstrap: Arc<tokio::sync::Mutex<Option<tokio::task::JoinHandle<()>>>>,

    /// Bounded queue of recent peer connect/disconnect events, drained by the
    /// `poll_peer_events` IPC handler.
    ///
    /// Populated by a background task in `daemon.rs` that subscribes to
    /// `P2pHandle::peer_event_tx` and enqueues each event here. Capped at
    /// `PEER_EVENT_QUEUE_CAP` to prevent unbounded growth when no consumer
    /// drains it (e.g. the Tauri UI is not open). The `poll_peer_events`
    /// handler drains and returns all pending events atomically.
    ///
    /// `std::sync::Mutex` (not tokio's) because the critical section is a
    /// short drain with no `.await`.
    pub(super) peer_event_queue: Arc<std::sync::Mutex<std::collections::VecDeque<PeerEventRecord>>>,

    /// Handle to the most-recently-started mDNS-SD browse task (CopyPaste-ydhw).
    ///
    /// `rescan_discovered` calls `DiscoveryService::start()` which aborts the
    /// previous browse task via `shutdown_inner()`.  The old code detached the
    /// new browse handle with a bare `tokio::spawn` — the task ran indefinitely
    /// without participating in P2P shutdown or being replaceable on the next
    /// rescan.
    ///
    /// The fix: store the live browse `JoinHandle` here.  On each
    /// `rescan_discovered` call the previous handle (if any) is aborted before
    /// the new browse starts, and the new handle is stored in its place.  This
    /// prevents handle accumulation across multiple rescans.
    ///
    /// `std::sync::Mutex` because every critical section is a quick
    /// take/replace with no `.await`.
    pub(super) discovery_browse_handle: Arc<std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>>,

    /// Optional P2P subsystem shutdown token (CopyPaste-ydhw).
    ///
    /// When populated (via [`p2p_shutdown_token_slot`](Self::p2p_shutdown_token_slot)),
    /// the `rescan_discovered` handler wraps the replacement browse handle in a
    /// `select!` that exits on P2P shutdown, ensuring the detached browse
    /// participates in graceful teardown.
    ///
    /// `daemon.rs` writes this slot after `start_p2p` returns (same pattern as
    /// `live_peer_sinks_slot`).  `None` means the slot has not been populated
    /// yet (or P2P is disabled) — the browse task then runs until the next
    /// rescan or process exit.
    ///
    /// `std::sync::Mutex` because the critical section is a trivial clone with
    /// no `.await`.
    pub(super) p2p_shutdown_token: Arc<std::sync::Mutex<Option<CancellationToken>>>,

    /// nq39: in-memory Supabase password cache for non-macOS platforms.
    ///
    /// On macOS the `store_cloud_password` IPC handler writes directly to the
    /// macOS Keychain and never populates this field. On non-macOS (Linux,
    /// Windows-frozen) the Keychain is unavailable, so the password is held
    /// here for the duration of the daemon process — it is never written to
    /// `config.json` via this path. `None` until `store_cloud_password` is
    /// called.
    ///
    /// `zeroize::Zeroizing` ensures the heap string is scrubbed when the
    /// `Arc` is dropped (daemon shutdown or field replacement on update).
    /// `std::sync::Mutex` (not tokio's) because the critical section is a
    /// trivial clone/replace with no `.await`.
    #[cfg(not(target_os = "macos"))]
    pub(super) in_memory_cloud_password: Arc<std::sync::Mutex<Option<zeroize::Zeroizing<String>>>>,

    /// Semaphore that bounds the number of simultaneously-active IPC connections
    /// (CopyPaste-6ot5). Each accepted connection acquires one permit via
    /// `try_acquire_owned` (non-blocking); the permit is moved into the spawned
    /// task and dropped on task completion. When all permits are taken, the
    /// accept loop drops the incoming `UnixStream` immediately rather than
    /// queueing or blocking. `Arc`-wrapped so it can be shared with the spawned
    /// connection tasks without lifetime issues.
    pub(super) conn_semaphore: Arc<tokio::sync::Semaphore>,

    /// Live relay orchestrator handle (CopyPaste-44rq.67).
    ///
    /// `daemon::run` starts the relay (if `relay_url` is configured) and stores
    /// the resulting [`crate::relay::RelayHandle`] here so the `set_config`
    /// handler can shut it down at runtime when the user clears the relay URL
    /// (`set_config { relay_url: "" }`). Dropping/`shutdown()`-ing the handle
    /// stops the push + receive loops within one poll cycle, so the user can
    /// disable relay sync without restarting the daemon. `None` when no relay is
    /// running (not configured, failed to start, or already cleared).
    ///
    /// tokio `Mutex` because the `set_config` handler `.await`s while holding it.
    #[cfg(feature = "relay-sync")]
    pub(super) relay_handle: Arc<tokio::sync::Mutex<Option<crate::relay::RelayHandle>>>,

    /// Shared in-flight sync flag (CopyPaste-1jms.22).
    ///
    /// Set to `true` by a [`crate::sync_in_flight::SyncInFlightGuard`] at the
    /// start of each active sync round-trip (cloud poll, cloud push, relay
    /// receive, relay push, P2P handshake) and reset to `false` when the guard
    /// is dropped (on success, error, or early return via `?`).
    ///
    /// The `get_sync_status` handler passes this value as `in_flight` to
    /// [`copypaste_ipc::compute_sync_badge_state_with_inflight`] so that
    /// `SyncBadgeState::Syncing` is emitted during active exchanges rather than
    /// the dead-code path it was before this fix.
    ///
    /// `AtomicBool` (not `Mutex`) because the read in `get_sync_status` and the
    /// writes in the sync loops are all best-effort races — a brief window where
    /// the badge says "idle" while a round-trip just started is acceptable, but a
    /// blocking lock on the hot IPC path is not.
    pub(super) sync_in_flight: Arc<AtomicBool>,
}

/// Wire-serialisable peer event record returned by `poll_peer_events`.
#[derive(serde::Serialize, Clone, Debug)]
pub struct PeerEventRecord {
    /// `"connected"` or `"disconnected"`.
    pub kind: &'static str,
    /// Canonical lowercase colon-free hex fingerprint of the peer's cert.
    pub fingerprint: String,
}
