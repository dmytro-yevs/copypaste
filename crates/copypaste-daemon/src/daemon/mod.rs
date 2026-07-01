#[cfg(unix)]
use crate::ipc::IpcServer;
use crate::{
    clipboard::{ClipboardContent, ClipboardMonitor},
    p2p, paths,
};
use copypaste_core::{
    build_item_aad_v2, bump_item_recency, chunks_to_blob, derive_v2, encode_image_full,
    encrypt_item_with_aad, find_recent_by_hash, get_item_by_id, insert_item_with_fts,
    is_sensitive_for_autowipe, prune_to_cap, AppConfig, ClipboardItem, Database, DeviceKeypair,
    AAD_SCHEMA_VERSION_V4, ITEM_KEY_VERSION_CURRENT,
};
// CopyPaste-9fb6: opt-in error reporting. `reporter` (below) is constructed at
// startup with `ReportConsent::Disabled` (safe default — no data leaves the
// device) until a future settings surface exposes the consent toggle. Its
// `report_and_log` call sites now live in `p2p_bringup` (CopyPaste-vp63.12);
// this file only threads `&*reporter` through to them.
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

mod bootstrap;
pub(crate) mod capture;
mod monitor_loop;
mod p2p_bringup;
pub(crate) mod startup;
mod sync_bringup;

pub(crate) use capture::{handle_tick, run_ttl_cleanup};
// crh3.78: `FrontmostAppCache` is `#[cfg(target_os = "macos")]`; gate the
// re-export to match so the non-macOS (Linux) build resolves (CI E0432).
#[cfg(target_os = "macos")]
pub(crate) use capture::FrontmostAppCache;
pub(crate) use startup::{
    decide_db_startup, encrypted_db_exists, load_config, load_local_key_bounded,
    load_or_create_device_id, load_private_mode, persist_private_mode, run_degraded, sweep_keys,
    DbStartupPlan, KeyLoad,
};

// CopyPaste-vp63.12: one-shot bootstrap tasks (telemetry reporter, Keychain
// ACL rotation, v4/poison-row sweeps, DeviceMeta cache warm, startup TTL
// purge) and device-name resolution, extracted from this file.
// `resolve_device_name` is re-exported at the `daemon` root (not just
// `pub(crate)` inside `bootstrap`) because `ipc::handlers_pairing_qr` calls
// `crate::daemon::resolve_device_name` directly.
pub(crate) use bootstrap::resolve_device_name;
#[cfg(target_os = "macos")]
use bootstrap::rotate_keychain_acl_best_effort;
use bootstrap::{
    init_reporter, run_poison_row_sweep, run_startup_ttl_purge, run_v4_migration_sweep,
    warm_device_meta_cache,
};

// CopyPaste-vp63.12: the steady-state clipboard monitor loop, extracted from
// this file (lowest-risk extraction — already took explicit args).
use monitor_loop::run_monitor_loop;

/// Upper bound on the synchronous Keychain read that fetches the SQLCipher
/// device key at startup.
///
/// LIVE-CONFIRMED REGRESSION: after the macOS app is reinstalled the daemon
/// binary's code signature changes, so the Keychain ACL on the stored key no
/// longer trusts it. An interactive launch then BLOCKS FOREVER on a
/// SecurityAgent (Keychain password) GUI prompt inside the Security-framework
/// call — the daemon never reaches "IPC listening" and never binds the socket.
/// We run the read on a dedicated thread and abandon it after this timeout so
/// startup always proceeds to a defined state (degraded) in bounded time. The
/// abandoned thread may stay parked on the OS prompt; that is acceptable — it
/// holds only a clone of nothing and is reaped when the process exits.
pub(super) const KEYCHAIN_READ_TIMEOUT: Duration = Duration::from_secs(8);

/// How often the degraded-mode loop re-checks the quit flag (milliseconds).
/// 1 s is responsive enough for human perception while burning ~4× less CPU
/// than the old 250 ms value.
pub(super) const DEGRADED_QUIT_POLL_INTERVAL_MS: u64 = 1_000;

use std::sync::RwLock;
use tokio::sync::{broadcast, mpsc, Mutex};
// D1: CancellationToken for coordinated graceful shutdown across all tasks.
use tokio_util::sync::CancellationToken;

// Beta W2.2 (arch-1): sync orchestrator that wires `copypaste-sync` into the
// daemon. Declared at crate root in `lib.rs` (`pub mod sync_orch;`); we
// re-import it here for the local `sync_orch::SyncCrypto`/`AutoApplyCtx`
// construction below (the `sync_orch::run` call itself was hoisted into
// `sync_bringup::spawn_sync_orch`, CopyPaste-vp63.12).
use crate::sync_orch;

/// Run the daemon until `Ctrl+C` / `SIGTERM` is received.
///
/// This is the entry point used on non-macOS platforms and in tests.
pub async fn run() -> anyhow::Result<()> {
    run_with_quit_flag(Arc::new(AtomicBool::new(false))).await
}

/// Run the daemon until `Ctrl+C`, `SIGTERM`, or `quit_flag` is set.
///
/// On macOS the tray icon sets `quit_flag` when the user clicks Quit.
#[cfg_attr(not(target_os = "macos"), allow(unused_variables))]
pub async fn run_with_quit_flag(quit_flag: Arc<AtomicBool>) -> anyhow::Result<()> {
    let reporter = init_reporter();

    let config = load_config();
    tracing::info!(
        "poll_interval={}ms storage_quota_bytes={}",
        config.poll_interval_ms,
        config.storage_quota_bytes
    );
    // Shared live core config — written by `set_config` IPC handler so limit/feature
    // changes (e.g. paste_as_plain_text, excluded_app_bundle_ids, sync_on_wifi_only)
    // hot-reload into the tick loop and paste path without a daemon restart.
    let core_config_arc: Arc<RwLock<AppConfig>> = Arc::new(RwLock::new(config.clone()));

    // v0.3 (THREAT-MODEL OI-4): best-effort Keychain ACL rotation on first
    // launch after install/upgrade. See `rotate_keychain_acl_best_effort`.
    #[cfg(target_os = "macos")]
    rotate_keychain_acl_best_effort();

    // dedup-keychain + bounded/degraded startup, combined: load the device
    // keypair material (local enc key + X25519 public bytes) ONCE via the
    // bounded reader so we (a) never hang on a Keychain GUI prompt
    // (acceptance #1), (b) avoid a second Keychain read for the public bytes,
    // and (c) decide the DB-open plan from the key outcome + whether encrypted
    // data already exists. When the key is unavailable AND an encrypted DB
    // exists, we MUST NOT fall back to an ephemeral key against that DB (that
    // yields SQLITE_NOTADB and a dead daemon); instead we go DEGRADED — alive,
    // socket bound, recovery status served, DB untouched. COPYPASTE_EPHEMERAL_KEY
    // is honoured centrally inside `load_or_create`.
    let db_path = paths::db_path();
    let key_load = load_local_key_bounded();
    let db_exists = encrypted_db_exists(&db_path);
    let plan = decide_db_startup(&key_load, db_exists);

    // The key + public bytes to hand subsystems. For `Open` they are the real
    // material just read; for `OpenEphemeral` a fresh ephemeral keypair (no real
    // data to protect); the `Degraded` path never reaches the key-using
    // subsystems (it returns early below).
    let (local_key_arc, device_public_key): (Arc<zeroize::Zeroizing<[u8; 32]>>, [u8; 32]) =
        match plan {
            DbStartupPlan::Open => match key_load {
                KeyLoad::Ready(enc, pubk) => (Arc::new(enc), pubk),
                // Should be unreachable: `Open` is only produced for
                // `Ready` by `decide_db_startup`. However, treating this
                // as a process-aborting panic (P1-4) is fragile — a future
                // refactor or new `KeyLoad` variant can make this reachable.
                // Instead, degrade gracefully: log an error and enter
                // `run_degraded` (which binds the socket and serves a
                // recovery status) so the daemon stays alive and the user
                // receives a clear banner rather than a cryptic crash.
                KeyLoad::Locked => {
                    tracing::error!(
                        "BUG: decide_db_startup returned Open but KeyLoad is Locked — \
                         entering DEGRADED mode instead of panicking (P1-4)"
                    );
                    return run_degraded(crate::ipc::DEGRADED_REASON_KEYCHAIN_LOCKED, quit_flag)
                        .await;
                }
            },
            DbStartupPlan::OpenEphemeral => {
                let kp = DeviceKeypair::generate();
                (Arc::new(kp.local_enc_key()), kp.public_key_bytes())
            }
            DbStartupPlan::Degraded { reason } => {
                // Safety net for the post-reinstall regression: do NOT open,
                // write, or recreate the existing encrypted DB (acceptance #3).
                // Bring the daemon up alive with the socket bound and a clear
                // recovery status, and wait for shutdown. Recovery happens on a
                // later launch once the Keychain access is re-granted.
                tracing::warn!(
                    reason,
                    db_path = %db_path.display(),
                    "starting in DEGRADED mode: the SQLCipher key is unavailable and an \
                     encrypted database already exists. The daemon will stay alive and \
                     serve a recovery status; the encrypted data is left UNTOUCHED on \
                     disk and is recoverable once the Keychain key is restored \
                     (re-grant the Keychain prompt and relaunch)."
                );
                return run_degraded(reason, quit_flag).await;
            }
        };
    tracing::info!("local encryption key ready");

    let db = Arc::new(Mutex::new(
        match if std::env::var_os("COPYPASTE_NO_AUTO_MIGRATE").is_some() {
            // A.M1 Option C: operator opted out of silent plaintext→SQLCipher migration.
            // Returns DbError::PlaintextMigrationBlocked if a legacy database is found.
            Database::open_no_auto_migrate(&db_path, &local_key_arc)
        } else {
            Database::open(&db_path, &local_key_arc)
        } {
            Ok(db) => db,
            Err(e) => {
                // Belt-and-suspenders: `decide_db_startup` already routes the
                // common "key unavailable + encrypted DB exists" case to the
                // DEGRADED path above, so we should not normally reach an open
                // failure here. If we still do (e.g. the key WAS read but is the
                // wrong one — restored/!=device Keychain entry — surfacing as
                // SQLITE_NOTADB "file is not a database"), degrade instead of
                // exiting: keep the process alive, bind the socket, and leave
                // the encrypted file untouched so a later correct-key launch can
                // open it. Never `Error:`/exit on this condition.
                tracing::error!(
                    db_path = %db_path.display(),
                    error = %e,
                    "failed to open clipboard database — if this reports \
                     'file is not a database', the SQLCipher key does not match \
                     the key the DB was encrypted with (re-keyed device, \
                     restored/!=device Keychain entry, or a missing keychain \
                     entitlement). Entering DEGRADED mode; the encrypted data is \
                     intact on disk and recoverable once the matching key is \
                     restored."
                );
                // The key WAS obtained (we are on the `Open`/`OpenEphemeral`
                // path) but it does not match this database, so the accurate
                // reason is a key MISMATCH — not a locked/unreachable Keychain.
                // Reporting `keychain_locked` here would wrongly tell the user
                // to re-grant the Keychain prompt, which cannot fix a wrong key.
                return run_degraded(crate::ipc::DEGRADED_REASON_DB_KEY_MISMATCH, quit_flag).await;
            }
        },
    ));
    tracing::info!("database opened at {}", db_path.display());

    // v4 key-version migration sweep — runs once at startup (resumable).
    run_v4_migration_sweep(&db, &local_key_arc, &*reporter).await;

    // One-time startup sweep: delete poison rows created before the
    // inbound-merge guard was added (CopyPaste-jww / CopyPaste-5y4).
    run_poison_row_sweep(&db).await;

    // Device-keypair public bytes — passed into IpcServer so
    // `get_own_fingerprint` returns a stable cryptographic fingerprint
    // (audit HIGH #6: DefaultHasher(hostname,pid) changed every restart).
    // dedup-keychain: reuse the public bytes derived from the single
    // `load_local_key_material()` call above instead of a second
    // `load_or_create()`. On non-macOS this is the zero placeholder that
    // `load_local_key_material` returns. Memory: Windows/Linux are cfg-frozen.
    let device_public_key_arc: Arc<[u8; 32]> = Arc::new(device_public_key);

    // Load (or create on first run) the persistent device_id once so P2P,
    // sync_orch, cloud push, and clipboard capture all share the same stable
    // identity. Previously P2P and sync_orch each called load_or_create_device_id
    // separately, and clipboard items were never stamped at all, causing every
    // captured item to carry origin_device_id="" → each restart appeared as a
    // new anonymous device in the Supabase peer list (duplicate devices bug).
    let local_device_id: String = match load_or_create_device_id() {
        Ok(id) => id.to_string(),
        Err(e) => {
            tracing::warn!(
                "device_id load/create failed ({e}); using ephemeral UUID — \
                 items captured this session will carry a one-time device_id"
            );
            uuid::Uuid::new_v4().to_string()
        }
    };

    // D1: create the process-wide cancellation token. Clones are passed to
    // every long-running task; calling `shutdown_token.cancel()` on SIGINT/
    // SIGTERM propagates to all of them simultaneously.
    let shutdown_token = CancellationToken::new();

    // Shared private-mode flag: when true, the clipboard monitor skips recording.
    // This is set/cleared via the IPC `set_private_mode` command, which also
    // persists the new value to disk. We restore the persisted value here so
    // private mode survives a daemon restart (previously it always reset to
    // false on startup, silently resuming clipboard capture).
    let private_mode = Arc::new(AtomicBool::new(load_private_mode()));
    tracing::info!(
        private_mode = private_mode.load(Ordering::Relaxed),
        "restored persisted private-mode state"
    );

    // CopyPaste-vp63.12: P2P identity/config resolution (enable flag, paired-
    // peers allowlist, mDNS discovery service, mTLS identity cert, its
    // colon-hex fingerprint), extracted verbatim — see
    // `p2p_bringup::resolve_p2p_identity`. Must run BEFORE the IPC server is
    // constructed below: the IPC server needs clones of these exact same
    // instances (fix/p2p-c-review #2, LAN/SAS Phase 0, CRITICAL-1).
    let p2p_bringup::P2pIdentity {
        p2p_enabled,
        lan_visibility_at_start,
        p2p_peers,
        p2p_discovery,
        p2p_cert,
        cert_fingerprint_display,
    } = p2p_bringup::resolve_p2p_identity(&local_device_id);

    #[cfg(unix)]
    let socket_path = paths::socket_path();

    // Create shared cloud-sync state here so the IPC handler and the cloud
    // loops both observe the SAME Arcs. A `set_sync_passphrase` IPC call
    // writes to `cloud_sync_key`; the cloud push/poll loops read it. The
    // cloud loops write to `cloud_last_sync_ms`; `get_sync_status` reads it.
    #[cfg(any(feature = "cloud-sync", feature = "relay-sync"))]
    let cloud_sync_key: std::sync::Arc<tokio::sync::Mutex<Option<copypaste_core::SyncKey>>> = {
        // Restore a previously-set sync passphrase key on startup so cloud
        // sync resumes without re-entering the passphrase. On ad-hoc/unsigned
        // installs the key lives in the non-prompting 0600 file store; we only
        // read that here (the Keychain path is left untouched to avoid a
        // prompt on builds that still use it). See `keychain::file_store`.
        #[cfg(target_os = "macos")]
        let restored: Option<copypaste_core::SyncKey> = if crate::keychain::keychain_bypassed() {
            None
        } else {
            match crate::keychain::signing::choose_key_backend() {
                crate::keychain::signing::KeyBackend::File => {
                    match crate::keychain::file_store::load_cloud_sync_key() {
                        Ok(Some(bytes)) => Some(copypaste_core::SyncKey::from_bytes(bytes)),
                        Ok(None) => None,
                        Err(e) => {
                            tracing::warn!(error = %e, "could not restore cloud-sync key from file store");
                            None
                        }
                    }
                }
                // Keychain path: not auto-restored here (reading would risk a
                // prompt); the key is re-established via `set_sync_passphrase`.
                crate::keychain::signing::KeyBackend::Keychain => None,
            }
        };
        #[cfg(not(target_os = "macos"))]
        let restored: Option<copypaste_core::SyncKey> = None;
        if restored.is_some() {
            tracing::info!("restored cloud-sync key from persistent store");
        }
        std::sync::Arc::new(tokio::sync::Mutex::new(restored))
    };
    #[cfg(any(feature = "cloud-sync", feature = "relay-sync"))]
    let cloud_last_sync_ms: std::sync::Arc<std::sync::atomic::AtomicI64> =
        std::sync::Arc::new(std::sync::atomic::AtomicI64::new(0));
    // BUG 2: real GoTrue auth state, published by the cloud loops and read by the
    // IPC `get_sync_status` handler. Starts `false` — we are not signed in until
    // `start_cloud` resolves a bearer. Previously `get_sync_status` hardcoded
    // `signed_in = supabase_configured`, so it reported "signed in" even after a
    // `CloudError::AuthFailed` aborted cloud sync.
    #[cfg(any(feature = "cloud-sync", feature = "relay-sync"))]
    let cloud_signed_in: std::sync::Arc<std::sync::atomic::AtomicBool> =
        std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

    // CopyPaste-1jms.22: shared in-flight flag for the SyncBadgeState::Syncing
    // variant. Created once here; clones are passed into each sync round-trip
    // loop (cloud poll, cloud push, relay receive, relay push, P2P handshake)
    // and into the IpcServer so get_sync_status can read it. Each loop wraps the
    // active network exchange in a SyncInFlightGuard, which sets the flag true
    // on entry and resets to false on Drop (all exit paths, including `?`).
    #[cfg(any(feature = "cloud-sync", feature = "relay-sync"))]
    let sync_in_flight: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));

    // CopyPaste-bps: warm the DeviceMeta cache ONCE at startup, before the IPC
    // server is constructed. See `warm_device_meta_cache`.
    warm_device_meta_cache().await;

    // P2 (ugv7) — startup TTL purge: run the same TTL cleanup the tick loop
    // performs, ONCE, right after the database is opened and BEFORE the IPC
    // socket is bound. See `run_startup_ttl_purge`.
    run_startup_ttl_purge(&db, &config).await;

    // D2 (IPC): pass a token clone so the accept loop exits on shutdown.
    // DUP-ON-COPY fix: build IpcServer before spawning so we can extract
    // `self_write_change_count` and wire it into ClipboardMonitor below.
    // When `write_to_pasteboard` runs it stamps the post-write NSPasteboard
    // changeCount into this Arc; the monitor's next `poll()` skips that count.
    // Broadcast channel: carries newly-inserted clipboard items to any
    // subscriber (P2P sync, cloud-sync, future extensions). Created BEFORE the
    // IPC server so the `import` handler can be handed a sender clone (P2P
    // Phase 3 — imported items are broadcast so they sync like captured ones).
    //
    // Capacity 256 (bumped from 64 — audit HIGH #8). The earlier 64-slot
    // buffer was too small for clipboard bursts (e.g. a rapid `pbcopy` loop
    // or a P2P peer momentarily backpressured by network jitter): subscribers
    // would receive `RecvError::Lagged` and silently drop items.
    let (new_item_tx, _new_item_rx) = broadcast::channel::<ClipboardItem>(256);

    // H8 perf fix: build SyncCrypto early so a clone can be shared with both
    // the IpcServer (to call reload_sync_key after pairing) and the sync
    // orchestrator (to do the actual re-keying). Because the cached sync key is
    // stored behind an Arc<Mutex> inside SyncCrypto, all clones share the same
    // backing store — one reload_sync_key() call updates every holder.
    let sync_crypto: Option<sync_orch::SyncCrypto> = if p2p_enabled {
        let seed: [u8; 32] = **local_key_arc;
        Some(sync_orch::SyncCrypto::new(
            seed,
            crate::ipc::peers_file_path(),
        ))
    } else {
        None
    };

    // CopyPaste-j8p: open a read-only connection pool on the same database file.
    // SQLite WAL mode allows multiple readers to proceed in parallel without
    // blocking the single writer (the Arc<Mutex<Database>> above).  Pool size 4
    // covers the typical 2-3 concurrent UI/CLI read requests with headroom.
    // Schema migrations have already run above; the pool connections open the
    // post-migration file and require no DDL themselves.
    //
    // `None` on failure (e.g. wrong key, file locked) — the IPC server falls
    // back to the write mutex transparently so the daemon remains functional.
    #[cfg(unix)]
    let ipc_read_pool: Option<Arc<copypaste_core::SqlitePool>> = {
        let key: [u8; 32] = **local_key_arc;
        match copypaste_core::open_pool(&db_path, &key, 4) {
            Ok(pool) => {
                tracing::info!("read pool opened (4 connections) for concurrent IPC reads");
                Some(Arc::new(pool))
            }
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "read pool open failed — IPC reads will use write mutex (degraded perf)"
                );
                None
            }
        }
    };

    // CopyPaste-44rq.67: shared slot holding the live relay orchestrator handle.
    // The IPC server gets a clone (`with_relay_handle`) so its `set_config`
    // handler can shut the relay down when the user clears `relay_url`; this
    // local clone keeps the handle alive for the daemon's lifetime.
    #[cfg(feature = "relay-sync")]
    let relay_handle_slot: Arc<tokio::sync::Mutex<Option<crate::relay::RelayHandle>>> =
        Arc::new(tokio::sync::Mutex::new(None));

    // CopyPaste-1jms.34: allocate the cloud account-id slot BEFORE the server
    // tuple block so it can be written by the `start_cloud` result AFTER the
    // server is spawned. The IpcServer receives a clone of this Arc via
    // `with_cloud_account_id_slot` inside the block; the cloud startup block at
    // the bottom of this function writes the value through the outer binding.
    #[cfg(feature = "cloud-sync")]
    let cloud_account_id_slot: Arc<std::sync::Mutex<Option<String>>> =
        Arc::new(std::sync::Mutex::new(None));

    #[cfg(unix)]
    let (
        self_write_change_count_arc,
        p2p_sync_addr_slot,
        live_sinks_slot,
        live_rtt_ms_slot,
        p2p_shutdown_token_slot,
        peer_event_queue,
        pairing_coordinator,
        _ipc_handle,
        // B1: surfaced out of this block so the P2P subsystem (below) can share
        // the SAME public-IP cache the IPC server reads and the STUN task writes.
        public_ip_cache,
    ) = {
        let mut server = IpcServer::new(
            db.clone(),
            private_mode.clone(),
            local_key_arc.clone(),
            device_public_key_arc.clone(),
        )
        .with_new_item_tx(new_item_tx.clone())
        .with_core_config(core_config_arc.clone())
        .with_local_device_id(local_device_id.clone());
        // CopyPaste-44rq.67: share the relay-handle slot so set_config can stop
        // the relay at runtime (relay-sync feature only).
        #[cfg(feature = "relay-sync")]
        {
            server = server.with_relay_handle(relay_handle_slot.clone());
        }
        if let Some(peers) = p2p_peers.clone() {
            server = server.with_p2p_peers(peers);
        }
        // H8: share a SyncCrypto clone with the IPC server so it can call
        // reload_sync_key after any pairing write, propagating the new key to
        // the orchestrator (they share the same Arc<Mutex> backing store).
        if let Some(ref crypto) = sync_crypto {
            server = server.with_p2p_sync_crypto(crypto.clone());
        }
        if let Some(ref fp) = cert_fingerprint_display {
            server = server.with_cert_fingerprint(fp.clone());
        }
        // P2P Phase 1: hand the IPC pairing handlers a clone of the SAME mTLS
        // cert the transport presents, so they can TLS-wrap the unauthenticated
        // bootstrap pairing channel (responder listener / initiator dial). The
        // fingerprints peers learn over that channel then match what the pinned
        // mTLS layer compares.
        if let Some(ref cert) = p2p_cert {
            server = server.with_p2p_cert(cert.cert_der.clone(), cert.key_der.clone());
        }
        // LAN/SAS Phase 0: hand the IPC server the SAME DiscoveryService that
        // start_p2p will use, so list_discovered sees live mDNS peers.
        if let Some(ref disc) = p2p_discovery {
            server = server.with_discovery(std::sync::Arc::clone(disc));
        }
        #[cfg(any(feature = "cloud-sync", feature = "relay-sync"))]
        {
            server = server.with_cloud_sync_state(
                cloud_sync_key.clone(),
                cloud_last_sync_ms.clone(),
                cloud_signed_in.clone(),
            );
        }
        // CopyPaste-1jms.34: wire the outer cloud_account_id_slot Arc so the
        // server and the start_cloud result share the SAME backing Mutex. When
        // start_cloud writes into `cloud_account_id_slot` below, the server's
        // `get_sync_status` handler immediately sees the new value.
        #[cfg(feature = "cloud-sync")]
        {
            server = server.with_cloud_account_id_slot(cloud_account_id_slot.clone());
        }
        // CopyPaste-1jms.22: wire the shared in-flight flag so get_sync_status
        // can emit SyncBadgeState::Syncing while a sync round-trip is active.
        #[cfg(any(feature = "cloud-sync", feature = "relay-sync"))]
        {
            server = server.with_sync_in_flight(sync_in_flight.clone());
        }
        // Public-IP cache: shared between the IPC server (read path) and the
        // STUN refresh task (write path).  Created here so both can hold an
        // `Arc` clone before `server` is consumed by `tokio::spawn`.
        let public_ip_cache: Arc<tokio::sync::RwLock<Option<String>>> =
            Arc::new(tokio::sync::RwLock::new(None));
        server = server.with_public_ip_cache(public_ip_cache.clone());
        // CopyPaste-j8p: wire the read pool so list/count/search/history_page/stats
        // bypass the write mutex for concurrent reads.
        if let Some(pool) = ipc_read_pool {
            server = server.with_read_pool(pool);
        }
        // Spawn the STUN refresh loop if the user has not opted out.
        // The loop resolves once immediately, then re-resolves every 15 minutes.
        // All failures are best-effort: logged at debug and silently skipped.
        if config.collect_public_ip {
            let cache_for_task = public_ip_cache.clone();
            let stun_shutdown = shutdown_token.clone();
            tokio::spawn(async move {
                const REFRESH_INTERVAL: Duration = Duration::from_secs(15 * 60);
                loop {
                    // spawn_blocking so the blocking UDP socket call does not
                    // occupy an async worker thread.
                    let resolved = tokio::task::spawn_blocking(crate::public_ip::resolve_public_ip)
                        .await
                        .unwrap_or(None); // JoinError (panic in blocking task) → None
                    {
                        let mut slot = cache_for_task.write().await;
                        *slot = resolved;
                    }
                    tokio::select! {
                        _ = tokio::time::sleep(REFRESH_INTERVAL) => {},
                        _ = stun_shutdown.cancelled() => break,
                    }
                }
                tracing::debug!("public_ip: STUN refresh task shut down");
            });
        }

        let swcc = server.self_write_change_count.clone();
        // P2P Phase 2: grab a handle to the shared slot holding this daemon's
        // own P2P sync-listener address. `start_p2p` (below) binds an
        // OS-assigned port, so we populate this slot only once that port is
        // known; the pairing handlers then send it in-band over the bootstrap
        // channel so the peer can persist it for the Phase 3 connector.
        let sync_addr_slot = server.p2p_sync_addr_slot();
        // Online-status + mutual-unpair control slot (both consumers share the same Arc).
        let live_sinks_slot = server.live_peer_sinks_slot();
        // RTT slot — populated after start_p2p returns, read by list_peers.
        let live_rtt_ms_slot = server.live_peer_rtt_ms_slot();
        // P2P shutdown token slot — populated after start_p2p returns so that
        // rescan_discovered can cancel the mDNS browse task on P2P shutdown
        // (CopyPaste-fbxj). Mirrors the live_peer_sinks_slot wiring pattern.
        let p2p_shutdown_token_slot = server.p2p_shutdown_token_slot();
        // Peer-event queue — drained by `poll_peer_events` IPC handler; fed by
        // the background task below that subscribes to P2pHandle::peer_event_tx.
        let peer_event_queue = server.peer_event_queue();
        // LAN/SAS Phase 2: grab a clone of the shared discovery-pairing
        // coordinator so the standing responder in `start_p2p` routes its SAS
        // through the SAME state machine the IPC handlers observe.
        let pairing_coordinator = server.pairing_coordinator();
        let ipc_shutdown = shutdown_token.clone();
        // DUAL-DAEMON FIX: bind the IPC listener SYNCHRONOUSLY here, before
        // spawning the accept loop and before `start_p2p` runs. If the bind
        // fails, another healthy daemon already owns the socket
        // (`bind_with_stale_cleanup` refuses to steal it) — this instance is
        // the loser and must EXIT WITHOUT starting any P2P/mDNS stack. The old
        // code bound inside the spawned future, so a bind failure only logged
        // and `start_p2p` ran anyway, leaving a second concurrent P2P stack.
        let listener = match server.bind(&socket_path) {
            Ok(l) => l,
            Err(e) => {
                tracing::error!(
                    "IPC bind failed on {}: {e} — another daemon owns the socket; \
                     exiting WITHOUT starting P2P to avoid a duplicate P2P/mDNS stack",
                    socket_path.display()
                );
                return Err(e);
            }
        };
        let handle = tokio::spawn(async move {
            if let Err(e) = server.serve_on(listener, ipc_shutdown).await {
                tracing::error!("IPC server error: {e}");
            }
        });
        (
            swcc,
            sync_addr_slot,
            live_sinks_slot,
            live_rtt_ms_slot,
            p2p_shutdown_token_slot,
            peer_event_queue,
            pairing_coordinator,
            handle,
            public_ip_cache,
        )
    };

    // Subscriber loops (p2p outbound_loop, cloud orchestrator, sync_orch) log
    // `Lagged(n)` themselves — owned by the subsystems that hold the receivers.

    // Beta W2.2 (arch-1): create sync orchestrator channels up-front so they
    // can be wired into the P2P subsystem below.
    //
    // W2.2: `sync_outbound_rx` is owned by the P2P accept/fanout tasks; items
    // produced locally flow: sync_orch → sync_outbound_tx → sync_outbound_rx →
    // P2P outbound_loop → connected peers. Items received from peers flow:
    // P2P accept_loop → sync_incoming_tx → sync_incoming_rx → sync_orch.
    let (sync_outbound_tx, sync_outbound_rx) = mpsc::channel::<copypaste_sync::WireItem>(64);
    let (sync_incoming_tx, sync_incoming_rx) = mpsc::channel::<copypaste_sync::WireItem>(64);

    // CopyPaste-vp63.12: start the P2P subsystem when p2p_enabled is true —
    // extracted verbatim, see `p2p_bringup::start_p2p_subsystem`. Resolved
    // above via A-SET-4: COPYPASTE_P2P env override, falling back to
    // persisted config via `ipc::p2p_enabled_from_config()`.
    let _p2p_handle: Option<p2p::P2pHandle> = p2p_bringup::start_p2p_subsystem(
        p2p_peers,
        p2p_cert,
        p2p_discovery,
        &local_device_id,
        lan_visibility_at_start,
        db.clone(),
        &local_key_arc,
        &new_item_tx,
        sync_incoming_tx.clone(),
        sync_outbound_rx,
        std::sync::Arc::clone(&pairing_coordinator),
        std::sync::Arc::clone(&p2p_sync_addr_slot),
        std::sync::Arc::clone(&public_ip_cache),
        sync_crypto.clone(),
        core_config_arc.clone(),
        // CopyPaste-yw2k: non-secret Supabase account identity slot so the
        // standing LAN/SAS responder can include it in PeerMeta in-band.
        // The outer `cloud_account_id_slot` Arc is written by start_cloud later;
        // reading through it at runtime is safe (Mutex-guarded). Only wired when
        // the cloud-sync feature is compiled in (the Arc always exists then).
        #[cfg(feature = "cloud-sync")]
        Some(std::sync::Arc::clone(&cloud_account_id_slot)),
        #[cfg(not(feature = "cloud-sync"))]
        None,
        live_sinks_slot,
        live_rtt_ms_slot,
        p2p_shutdown_token_slot,
        peer_event_queue,
        &*reporter,
    )
    .await;

    // Beta W2.2 (arch-1): start the sync orchestrator.
    //
    // The orchestrator owns the bridge between the local clipboard broadcast
    // channel and the peer transport(s). We always spawn it — even when P2P
    // is disabled — because the inbound side may still receive items from the
    // cloud-sync path once that worker (W2.3) wires its incoming sender in.
    //
    // Reuse the persistent device_id loaded once above — all subsystems
    // (P2P, sync_orch, cloud push, clipboard capture) share the same stable
    // identity across restarts.
    // P2P Phase 3 (cross-device readability): the orchestrator uses the
    // SyncCrypto built earlier (H8: shared with the IpcServer so pairing
    // immediately refreshes the cached key via reload_sync_key).
    // Pass the configured quota so the P2P merge path prunes to the same cap
    // as the cloud path (Fix HIGH-3). Saturating cast: values above i64::MAX
    // (>9 EB) are unreachable in practice.
    let sync_quota_bytes = config.storage_quota_bytes.min(i64::MAX as u64) as i64;

    // Universal Clipboard auto-apply: wire the self-write sentinel Arc (shared
    // with ClipboardMonitor via IpcServer) and the local key into the sync
    // orchestrator so freshly-synced items are immediately written to
    // NSPasteboard. The same changeCount guard that prevents IPC copy_item
    // from being re-captured is reused here — zero new primitives required.
    // Only wired on Unix (where the IPC socket and ClipboardMonitor exist).
    #[cfg(unix)]
    let sync_auto_apply = Some(sync_orch::AutoApplyCtx {
        self_write_change_count: self_write_change_count_arc.clone(),
        local_key: local_key_arc.clone(),
        core_config: core_config_arc.clone(),
    });
    #[cfg(not(unix))]
    let sync_auto_apply: Option<sync_orch::AutoApplyCtx> = None;

    // CopyPaste-vp63.12: spawn the sync orchestrator — extracted verbatim, see
    // `sync_bringup::spawn_sync_orch`.
    let sync_handle = sync_bringup::spawn_sync_orch(
        db.clone(),
        &new_item_tx,
        sync_incoming_rx,
        sync_outbound_tx,
        local_device_id.clone(),
        sync_crypto,
        sync_quota_bytes,
        sync_auto_apply,
        shutdown_token.clone(),
    );
    // Keep the incoming sender alive so the P2P accept loop can always push
    // received items into sync_orch even after the P2P handle has been taken.
    // Dropping this would close sync_orch's incoming side prematurely.
    let _keep_alive_sync_incoming = sync_incoming_tx;

    // tke7 (PG-30): read sync_enabled master gate once at startup.  Transports
    // check this flag again on every tick for hot-reload support, but we also
    // skip starting the loops entirely when sync is disabled at boot to avoid
    // allocating threads/connections that will never do work.
    // allow(unused_variables): `sync_enabled_at_start` is only read inside
    // #[cfg(feature = "cloud-sync")] and #[cfg(feature = "relay-sync")] blocks;
    // when neither feature is active the compiler sees it as unused.
    #[allow(unused_variables)]
    let sync_enabled_at_start = core_config_arc
        .read()
        .map(|c| c.sync_enabled)
        .unwrap_or(true);

    // CopyPaste-vp63.12: start optional cloud-sync if credentials are present
    // — extracted verbatim, see `sync_bringup::start_cloud_sync`.
    #[cfg(feature = "cloud-sync")]
    let _cloud_handle = sync_bringup::start_cloud_sync(
        db.clone(),
        &new_item_tx,
        cloud_sync_key.clone(),
        cloud_last_sync_ms.clone(),
        &local_key_arc,
        cloud_signed_in.clone(),
        core_config_arc.clone(),
        sync_in_flight.clone(),
        cloud_account_id_slot.clone(),
        sync_enabled_at_start,
    )
    .await;

    // CopyPaste-vp63.12: start the relay-as-database sync path iff `relay_url`
    // is configured — extracted verbatim, see `sync_bringup::start_relay_sync`.
    //
    // TOPOLOGY (dtq3): relay and Supabase are ADDITIVE, INDEPENDENT transports.
    // When both are configured this and the cloud step above BOTH run; a
    // locally-captured item is broadcast to both. Consumer-side dedup
    // (`ingest_page_blocking` / `remote_wins`) makes a double delivery a
    // no-op. See `relay.rs` § "Multi-transport topology" for the full
    // contract — no mutual-exclusion gate is needed here.
    #[cfg(feature = "relay-sync")]
    {
        // CopyPaste-7ub: wire the self-write sentinel so relay auto-apply does
        // not re-capture its own pasteboard writes. On Unix the sentinel is
        // shared with the ClipboardMonitor and the IPC copy_item handler; on
        // non-Unix there is no NSPasteboard so the sentinel is disabled.
        // Computed here (rather than inside `start_relay_sync`) so that
        // function's parameter list stays platform-independent, mirroring the
        // `sync_auto_apply` construction above for `spawn_sync_orch`.
        #[cfg(unix)]
        let relay_auto_apply_cc: Option<Arc<std::sync::atomic::AtomicI64>> =
            Some(self_write_change_count_arc.clone());
        #[cfg(not(unix))]
        let relay_auto_apply_cc: Option<Arc<std::sync::atomic::AtomicI64>> = None;

        // CopyPaste-44rq.67: `start_relay_sync` publishes the started handle
        // into `relay_handle_slot` (rather than returning a bare local) so the
        // IPC `set_config` handler can shut the relay down at runtime.
        sync_bringup::start_relay_sync(
            core_config_arc.clone(),
            sync_enabled_at_start,
            &local_device_id,
            db.clone(),
            &new_item_tx,
            cloud_sync_key.clone(),
            &local_key_arc,
            cloud_last_sync_ms.clone(),
            relay_auto_apply_cc,
            sync_in_flight.clone(),
            relay_handle_slot,
        )
        .await;
    }

    let mut monitor = ClipboardMonitor::new(config.max_text_size_bytes);
    // Override the READ-gate image cap with the user-configured value so the
    // clipboard poll gate and the encode gate are consistent.  Without this,
    // `poll()` was capped at the hardcoded core const (10 MiB) even when the
    // user configured a higher value (default 25 MiB), making configs above
    // 10 MiB silently ineffective.
    monitor.set_max_image_bytes(usize::try_from(config.max_image_size_bytes).unwrap_or(usize::MAX));
    monitor.set_max_file_bytes(usize::try_from(config.max_file_size_bytes).unwrap_or(usize::MAX));
    // DUP-ON-COPY fix: share the self-write sentinel with the IpcServer so
    // write_to_pasteboard can stamp the post-write changeCount and the monitor
    // can suppress the immediately-following re-capture of that same write.
    #[cfg(unix)]
    {
        monitor.self_write_change_count = self_write_change_count_arc;
    }

    // crh3.78: hand the configured monitor to the steady-state lifecycle loop.
    // It owns the poll ticker, the periodic sensitive/general TTL cleanups, the
    // live-config hot-reload, and signal/quit-flag handling. It returns once
    // shutdown is requested — after cancelling `shutdown_token` so the drain
    // below can join the remaining subsystem tasks.
    run_monitor_loop(
        monitor,
        db.clone(),
        local_key_arc,
        private_mode,
        new_item_tx,
        local_device_id,
        core_config_arc,
        config,
        quit_flag,
        shutdown_token,
    )
    .await?;

    // D5: wait for long-running tasks to drain before cleaning up resources.
    tracing::info!("waiting for subsystem tasks to finish...");
    // sync_orch exits promptly (shutdown token was cancelled above).
    let _ = sync_handle.await;
    // IPC accept loop exits on shutdown token; join it now.
    #[cfg(unix)]
    let _ = _ipc_handle.await;
    // P2P: signal shutdown by cancelling the token then let the handle drop.
    if let Some(p2p_handle) = _p2p_handle {
        // BUG F1: cancel the shared CancellationToken — this stops ALL five P2P
        // tasks (accept, standing responder, outbound, connector, discovery),
        // not just the accept loop as the old single-receiver oneshot did.
        p2p_handle.shutdown_token.cancel();
    }

    #[cfg(unix)]
    let _ = std::fs::remove_file(&socket_path);

    // D5: log DB close — the Arc<Mutex<Database>> is dropped here once all
    // task clones have been joined above, so SQLite will flush its WAL and
    // close cleanly.
    tracing::info!("database closing");
    drop(db);
    tracing::info!("daemon stopped");
    Ok(())
}

// Suppress unused import warnings for items imported for mod.rs use only.
// These are used in cfg-gated and test-only paths so the compiler may
// flag them as unused in certain build configurations.
// Kept BEFORE the test module so clippy::items_after_test_module is satisfied.
#[allow(unused_imports)]
use build_item_aad_v2 as _;
#[allow(unused_imports)]
use bump_item_recency as _;
#[allow(unused_imports)]
use chunks_to_blob as _;
#[allow(unused_imports)]
use derive_v2 as _;
#[allow(unused_imports)]
use encode_image_full as _;
#[allow(unused_imports)]
use encrypt_item_with_aad as _;
#[allow(unused_imports)]
use find_recent_by_hash as _;
#[allow(unused_imports)]
use get_item_by_id as _;
#[allow(unused_imports)]
use insert_item_with_fts as _;
#[allow(unused_imports)]
use is_sensitive_for_autowipe as _;
#[allow(unused_imports)]
use prune_to_cap as _;
#[allow(unused_imports)]
use ClipboardContent as _;
#[allow(unused_imports)]
use ClipboardMonitor as _;
#[allow(unused_imports)]
use Database as _;
#[allow(unused_imports)]
use DeviceKeypair as _;
#[allow(unused_imports)]
use AAD_SCHEMA_VERSION_V4 as _;
#[allow(unused_imports)]
use ITEM_KEY_VERSION_CURRENT as _;

// CopyPaste-vp63.12: `lifecycle_tests` (init_reporter_returns_usable_reporter,
// startup_ttl_purge_is_noop_on_empty_db, poison_row_sweep_is_noop_on_empty_db)
// moved to `bootstrap.rs` alongside the functions they pin
// (init_reporter/run_startup_ttl_purge/run_poison_row_sweep now live there).
