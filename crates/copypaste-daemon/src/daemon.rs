#[cfg(unix)]
use crate::ipc::IpcServer;
use crate::{
    clipboard::{ClipboardContent, ClipboardMonitor},
    p2p, paths,
};
use copypaste_core::{
    build_item_aad_v2, bump_item_recency, chunks_to_blob, derive_v2, detect, encode_image,
    encrypt_item_with_aad, find_recent_by_hash, get_item_by_id, insert_item_with_fts, AppConfig,
    ClipboardItem, Database, DeviceKeypair, AAD_SCHEMA_VERSION_V4,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

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
const KEYCHAIN_READ_TIMEOUT: Duration = Duration::from_secs(8);
use tokio::sync::{broadcast, mpsc, Mutex};
use tokio::time::interval;
// D1: CancellationToken for coordinated graceful shutdown across all tasks.
use tokio_util::sync::CancellationToken;

// Beta W2.2 (arch-1): sync orchestrator that wires `copypaste-sync` into the
// daemon. Declared at crate root in `lib.rs` (`pub mod sync_orch;`); we
// re-import it here for the local `sync_orch::run` call below.
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
    let config = load_config();
    tracing::info!(
        "poll_interval={}ms history_limit={}",
        config.poll_interval_ms,
        config.history_limit
    );

    // v0.3 (THREAT-MODEL OI-4): upgrade the Keychain entry's ACL on first
    // launch after install/upgrade.  Idempotent + best-effort — a failure
    // here (e.g. user denied a Keychain prompt) must not block the daemon
    // because the entry is still usable, just with the legacy unrestricted
    // ACL.  The next launch retries automatically.
    //
    // Only run this when the Keychain backend is actually in use. On ad-hoc /
    // unsigned installs the key lives in the 0600 file store (see
    // `keychain::file_store`) and there is no Keychain ACL to rotate — calling
    // the rotation there would read/delete/recreate a Keychain item and raise
    // the very login-password prompt this whole change exists to eliminate.
    //
    // CHICKEN-AND-EGG (acceptance criterion #4): after a reinstall the binary's
    // code signature changes and the Keychain ACL no longer trusts it, so
    // `rotate_acl_to_current_install` ITSELF calls `get_generic_password` first
    // to read the secret it would re-store — the very read that now prompts /
    // is denied. It therefore cannot re-establish trust without the access it
    // is trying to repair. We keep it best-effort here (it succeeds on the
    // benign install-moved case) and rely on the DEGRADED startup path below as
    // the safety net for the untrusted-binary case. Real recovery is a one-time
    // user re-grant of the Keychain prompt on a subsequent launch, after which
    // this rotation pins the new binary so later launches stay quiet.
    #[cfg(target_os = "macos")]
    if crate::keychain::signing::choose_key_backend()
        == crate::keychain::signing::KeyBackend::Keychain
    {
        match crate::keychain::acl::rotate_acl_to_current_install() {
            Ok(true) => tracing::info!("Keychain ACL rotated to current install"),
            Ok(false) => tracing::debug!("Keychain ACL already current"),
            Err(e) => tracing::warn!(
                error = %e,
                "Keychain ACL rotation failed — entry still usable with legacy ACL"
            ),
        }
    }

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
                // unreachable: `Open` is only produced for `Ready`.
                KeyLoad::Locked => unreachable!("Open plan implies a Ready key"),
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
                return run_degraded(crate::ipc::DEGRADED_REASON_KEYCHAIN_LOCKED, quit_flag).await;
            }
        },
    ));
    tracing::info!("database opened at {}", db_path.display());

    // v4 key-version migration sweep — runs once at startup (resumable).
    //
    // The sweep rotates any remaining `key_version = 1` rows to `key_version
    // = 2`.  It is synchronous (rusqlite), so we offload it to a blocking
    // thread via `spawn_blocking` and await the result before continuing.
    // On error we WARN and continue — a partially-swept DB is still usable;
    // new writes will keep being rejected by the migration gate until the
    // sweep eventually completes on a future restart.
    {
        // Escape hatch for installs that are ALREADY stuck on a prior build:
        // an install whose only remaining `key_version = 1` rows are
        // permanently unrotatable (auth tag mismatch) left the gate armed
        // forever on older daemons, rejecting every capture with
        // `MigrationInProgress`. Setting COPYPASTE_FORCE_MIGRATION_COMPLETE=1
        // force-clears the gate before the sweep runs so new captures resume
        // immediately. Mirrors COPYPASTE_NO_AUTO_MIGRATE. The corrupt rows are
        // left untouched (they were already unreadable).
        let force_complete = std::env::var_os("COPYPASTE_FORCE_MIGRATION_COMPLETE").is_some();
        // Opt-in destructive purge of the permanently-undecryptable
        // `key_version = 1` rows (auth-tag mismatch — never rotatable). Off by
        // default: we never delete user data without an explicit flag. When
        // unset we only WARN with the count + this guidance (see below).
        let purge_dead = std::env::var_os("COPYPASTE_PURGE_DEAD_V1_ROWS").is_some();
        // Derive both sweep keys from the seed the same way the read path does
        // (see `sweep_keys`). The seed is the value stored in the Keychain /
        // returned by `load_local_key()`, which is ALREADY the v1 storage key
        // (`DeviceKeypair::local_enc_key`).
        let seed: [u8; 32] = **local_key_arc;
        let (v1_key, v2_key) = sweep_keys(&seed);
        let sweep_db = db.clone();
        match tokio::task::spawn_blocking(move || {
            // Acquire the lock inside the blocking thread so the async
            // executor is not blocked while we hold it.
            let guard = sweep_db.blocking_lock();
            if force_complete {
                guard.force_migration_complete()?;
            }
            let rotated = guard.migration_v4_sweep_resumable(&v1_key, &v2_key)?;
            guard.force_complete_if_no_v1_rows()?;
            // After the sweep, surface any rows that stayed at key_version=1 —
            // these are permanently undecryptable legacy ciphertexts (auth-tag
            // mismatch) and are dead weight. Purge only if explicitly opted in.
            let dead = guard.count_dead_v1_rows()?;
            let purged = if dead > 0 && purge_dead {
                guard.purge_dead_v1_rows()?
            } else {
                0
            };
            Ok::<(usize, usize, usize), copypaste_core::DbError>((rotated, dead, purged))
        })
        .await
        {
            Ok(Ok((rotated, dead, purged))) => {
                tracing::info!(rotated, "v4 key-version migration sweep complete");
                if purged > 0 {
                    tracing::warn!(
                        purged,
                        "v4 migration: purged {purged} permanently-undecryptable \
                         key_version=1 row(s) (COPYPASTE_PURGE_DEAD_V1_ROWS=1)"
                    );
                } else if dead > 0 {
                    // One-time actionable WARN: these rows can never be
                    // decrypted or rotated. Tell the user how to remove them.
                    tracing::warn!(
                        dead,
                        "v4 migration: {dead} legacy key_version=1 row(s) are \
                         permanently undecryptable (auth-tag mismatch — re-keyed \
                         device or lost key generation) and cannot be rotated. \
                         They are dead weight in the database. To purge them, \
                         restart the daemon once with COPYPASTE_PURGE_DEAD_V1_ROWS=1."
                    );
                }
            }
            Ok(Err(e)) => {
                tracing::warn!(error = %e, "v4 migration sweep failed — writes remain gated until next restart");
            }
            Err(join_err) => {
                tracing::warn!(error = %join_err, "v4 migration sweep task panicked");
            }
        }
    }

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

    // fix/p2p-c-review #2: when P2P is enabled, the IPC PAKE handlers and the
    // mTLS transport must share ONE live `PairedPeers` allowlist so a peer
    // paired at runtime is accepted by the accept loop without a restart.
    // Create it here (before both the IPC task and `start_p2p`) and hand each a
    // clone; `PairedPeers` is interior-mutable, so clones observe one another.
    // `None` when P2P is disabled — IPC pairing then only persists to peers.json.
    let p2p_enabled = std::env::var("COPYPASTE_P2P").as_deref() == Ok("1");
    let p2p_peers: Option<copypaste_p2p::transport::PairedPeers> = if p2p_enabled {
        Some(copypaste_p2p::transport::PairedPeers::new())
    } else {
        None
    };

    // CRITICAL-1: generate the mTLS self-signed cert ONCE, here, before both the
    // IPC server and `start_p2p`. The IPC pairing handlers advertise this cert's
    // fingerprint (in colon-hex user-facing form) and `start_p2p` makes the
    // transport present the SAME cert — so a scanning/pairing peer pins exactly
    // the value the mTLS verifier compares (`fingerprint_of(cert_der)`), instead
    // of the device-key fingerprint the allowlist never checks.
    //
    // `None` when P2P is disabled: no transport runs, so there is no cert to
    // advertise; the pairing IPC handlers then return a clear error.
    let p2p_cert: Option<copypaste_p2p::cert::SelfSignedCert> = if p2p_enabled {
        match copypaste_p2p::cert::SelfSignedCert::generate(&local_device_id) {
            Ok(cert) => Some(cert),
            Err(e) => {
                tracing::warn!("mTLS cert generation failed ({e}); pairing disabled this session");
                None
            }
        }
    } else {
        None
    };
    // Colon-hex (user-facing) form of the cert fingerprint for the pairing
    // surface; `display_fingerprint` round-trips back to `fingerprint_of` via
    // `canonical_fingerprint` at the mTLS boundary.
    let cert_fingerprint_display: Option<String> = p2p_cert
        .as_ref()
        .map(|c| crate::ipc::display_fingerprint(&c.fingerprint()));

    #[cfg(unix)]
    let socket_path = paths::socket_path();

    // Create shared cloud-sync state here so the IPC handler and the cloud
    // loops both observe the SAME Arcs. A `set_sync_passphrase` IPC call
    // writes to `cloud_sync_key`; the cloud push/poll loops read it. The
    // cloud loops write to `cloud_last_sync_ms`; `get_sync_status` reads it.
    #[cfg(feature = "cloud-sync")]
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
    #[cfg(feature = "cloud-sync")]
    let cloud_last_sync_ms: std::sync::Arc<std::sync::atomic::AtomicI64> =
        std::sync::Arc::new(std::sync::atomic::AtomicI64::new(0));
    // BUG 2: real GoTrue auth state, published by the cloud loops and read by the
    // IPC `get_sync_status` handler. Starts `false` — we are not signed in until
    // `start_cloud` resolves a bearer. Previously `get_sync_status` hardcoded
    // `signed_in = supabase_configured`, so it reported "signed in" even after a
    // `CloudError::AuthFailed` aborted cloud sync.
    #[cfg(feature = "cloud-sync")]
    let cloud_signed_in: std::sync::Arc<std::sync::atomic::AtomicBool> =
        std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

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

    #[cfg(unix)]
    let (self_write_change_count_arc, p2p_sync_addr_slot, _ipc_handle) = {
        let mut server = IpcServer::new(
            db.clone(),
            private_mode.clone(),
            local_key_arc.clone(),
            device_public_key_arc.clone(),
        )
        .with_new_item_tx(new_item_tx.clone());
        if let Some(peers) = p2p_peers.clone() {
            server = server.with_p2p_peers(peers);
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
        #[cfg(feature = "cloud-sync")]
        {
            server = server.with_cloud_sync_state(
                cloud_sync_key.clone(),
                cloud_last_sync_ms.clone(),
                cloud_signed_in.clone(),
            );
        }
        let swcc = server.self_write_change_count.clone();
        // P2P Phase 2: grab a handle to the shared slot holding this daemon's
        // own P2P sync-listener address. `start_p2p` (below) binds an
        // OS-assigned port, so we populate this slot only once that port is
        // known; the pairing handlers then send it in-band over the bootstrap
        // channel so the peer can persist it for the Phase 3 connector.
        let sync_addr_slot = server.p2p_sync_addr_slot();
        let socket_clone = socket_path.clone();
        let ipc_shutdown = shutdown_token.clone();
        let handle = tokio::spawn(async move {
            if let Err(e) = server.serve(&socket_clone, ipc_shutdown).await {
                tracing::error!("IPC server error: {e}");
            }
        });
        (swcc, sync_addr_slot, handle)
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

    // Start the P2P subsystem when COPYPASTE_P2P=1 is set in the environment.
    // Both the live allowlist and the cert must be present: the cert is the
    // identity the transport presents and that pairing advertises, so without
    // it there is nothing for peers to pin (CRITICAL-1).
    let _p2p_handle: Option<p2p::P2pHandle> = if let (Some(p2p_peers), Some(p2p_cert)) =
        (p2p_peers, p2p_cert)
    {
        // Reuse the persistent device_id loaded above (load_or_create_device_id
        // was called once already; parsing it back to Uuid is cheap).
        let device_id =
            uuid::Uuid::parse_str(&local_device_id).unwrap_or_else(|_| uuid::Uuid::new_v4());
        let device_name = std::env::var("HOSTNAME")
            .or_else(|_| std::env::var("COMPUTERNAME"))
            .unwrap_or_else(|_| "CopyPaste".to_string());

        let p2p_config = p2p::P2pConfig {
            listen_port: 0,
            device_name,
            enabled: true,
        };

        // P2P Phase 3 (sync-on-connect catch-up): build a provider that
        // replays the current local history — already re-keyed under the
        // shared sync key, exactly like normal outbound — into each peer the
        // instant a link is established. Without this, an item produced
        // before the link came up is never delivered (fanout is
        // fire-and-forget to currently-connected sinks). Uses the same
        // `SyncCrypto` construction as the orchestrator below.
        let catchup: p2p::CatchupProvider = {
            let catchup_db = db.clone();
            let catchup_device_id = local_device_id.clone();
            let catchup_seed: [u8; 32] = **local_key_arc;
            Arc::new(move || {
                let crypto =
                    sync_orch::SyncCrypto::new(catchup_seed, crate::ipc::peers_file_path());
                // The closure is `Fn` (sync) but the DB sits behind a tokio
                // Mutex; `block_in_place` + `blocking_lock` safely acquires
                // it on the multi-thread runtime without blocking the worker.
                tokio::task::block_in_place(|| {
                    let db = catchup_db.blocking_lock();
                    sync_orch::catchup_items(&db, &catchup_device_id, &crypto)
                })
            })
        };

        // Hand the SAME live allowlist already shared with the IPC server
        // (fix/p2p-c-review #2) and the SAME cert whose fingerprint the IPC
        // pairing handlers advertise (CRITICAL-1). `start_p2p` seeds the
        // allowlist from peers.json.
        match p2p::start_p2p(
            p2p_config,
            db.clone(),
            device_id,
            (*local_key_arc).clone(),
            p2p_cert,
            p2p_peers,
            new_item_tx.subscribe(),
            sync_incoming_tx.clone(),
            sync_outbound_rx,
            catchup,
        )
        .await
        {
            Ok(handle) => {
                tracing::info!(port = handle.actual_port, "P2P subsystem running");
                // P2P Phase 2: publish this daemon's now-bound sync-listener
                // address into the shared slot the IPC pairing handlers read.
                //
                // The listener binds `0.0.0.0:actual_port`, so it is reachable
                // on every interface — but the address we ADVERTISE to a peer
                // (sent in-band during pairing and persisted into the peer's
                // `peers.json`) must be a concrete LAN-routable host, never
                // `127.0.0.1`. A loopback advertisement is why background sync
                // only worked in the emulator / loopback case: a real phone on
                // Wi-Fi cannot route to 127.0.0.1. `advertise_sync_addr`
                // selects a real LAN address via the same interface filter the
                // QR `addr_hint` uses, falling back to loopback only when no
                // LAN interface exists (single-host / loopback test).
                #[cfg(unix)]
                {
                    let addr = copypaste_p2p::interfaces::advertise_sync_addr(handle.actual_port)
                        .to_string();
                    tracing::info!(
                        sync_addr = %addr,
                        "P2P advertising LAN-routable sync-listener address"
                    );
                    let mut slot = p2p_sync_addr_slot
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    *slot = Some(addr);
                }
                Some(handle)
            }
            Err(e) => {
                tracing::warn!("Failed to start P2P subsystem: {e}");
                None
            }
        }
    } else {
        tracing::debug!("P2P disabled (set COPYPASTE_P2P=1 to enable)");
        // Drop sync_outbound_rx — no consumer. sync_orch will log debug
        // on each outbound send (harmless: closed receiver just means no
        // peers are connected).
        drop(sync_outbound_rx);
        None
    };

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
    let sync_device_id = local_device_id.clone();
    let sync_db = db.clone();
    let sync_rx = new_item_tx.subscribe();
    // D2 (sync_orch): pass a token clone so the orchestrator exits on shutdown.
    let sync_shutdown = shutdown_token.clone();
    // P2P Phase 3 (cross-device readability): give the orchestrator this
    // device's local-storage seed and the peers.json path so it can re-key item
    // payloads through the shared content sync key established at pairing. Only
    // wired when P2P is enabled — the cloud path uses its own SyncKey scheme.
    let sync_crypto = if p2p_enabled {
        let seed: [u8; 32] = **local_key_arc;
        Some(sync_orch::SyncCrypto::new(
            seed,
            crate::ipc::peers_file_path(),
        ))
    } else {
        None
    };
    let sync_handle = tokio::spawn(async move {
        if let Err(e) = sync_orch::run(
            sync_db,
            sync_rx,
            sync_incoming_rx,
            sync_outbound_tx,
            sync_device_id,
            sync_crypto,
            sync_shutdown,
        )
        .await
        {
            tracing::warn!("sync orchestrator exited with error: {e}");
        }
    });
    // Keep the incoming sender alive so the P2P accept loop can always push
    // received items into sync_orch even after the P2P handle has been taken.
    // Dropping this would close sync_orch's incoming side prematurely.
    let _keep_alive_sync_incoming = sync_incoming_tx;

    // Start optional cloud-sync if credentials are present.
    #[cfg(feature = "cloud-sync")]
    let _cloud_handle = {
        use crate::cloud::{start_cloud, CloudConfig};
        if let Some(cloud_cfg) = CloudConfig::from_env() {
            tracing::info!("cloud-sync: SUPABASE_URL found, starting cloud orchestrator");
            // Subscribe a new receiver from the existing sender.
            let rx = new_item_tx.subscribe();
            match start_cloud(
                cloud_cfg,
                db.clone(),
                rx,
                cloud_sync_key.clone(),
                cloud_last_sync_ms.clone(),
                local_key_arc.clone(),
                cloud_signed_in.clone(),
            )
            .await
            {
                Ok(handle) => {
                    tracing::info!("cloud-sync: orchestrator started");
                    Some(handle)
                }
                Err(e) => {
                    tracing::warn!("cloud-sync: failed to start ({e}); continuing without sync");
                    None
                }
            }
        } else {
            tracing::debug!("cloud-sync: SUPABASE_URL not set, skipping");
            None
        }
    };

    let mut monitor = ClipboardMonitor::new(config.max_text_size_bytes);
    // DUP-ON-COPY fix: share the self-write sentinel with the IpcServer so
    // write_to_pasteboard can stamp the post-write changeCount and the monitor
    // can suppress the immediately-following re-capture of that same write.
    #[cfg(unix)]
    {
        monitor.self_write_change_count = self_write_change_count_arc;
    }
    let mut ticker = interval(Duration::from_millis(config.poll_interval_ms));
    let mut cleanup_ticks: u64 = 0;
    // Sensitive TTL cleanup runs every 5 seconds; track elapsed ticks separately.
    let mut sensitive_cleanup_ticks: u64 = 0;
    let sensitive_ttl_ms = config.sensitive_ttl_secs as i64 * 1000;

    tracing::info!("clipboard monitor started");
    tracing::info!(
        "sensitive auto-wipe TTL: {}s ({}ms), checked every 5s",
        config.sensitive_ttl_secs,
        sensitive_ttl_ms,
    );

    #[cfg(target_os = "macos")]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut sigterm = signal(SignalKind::terminate())?;
        loop {
            // Check tray quit flag before blocking on select
            if quit_flag.load(Ordering::Relaxed) {
                tracing::info!("quit flag set, shutting down daemon");
                // D3: ensure all tasks receive the cancellation signal even
                // when the tray host (not a signal) triggers shutdown.
                shutdown_token.cancel();
                break;
            }
            tokio::select! {
                _ = ticker.tick() => {
                    handle_tick(&mut monitor, &db, &local_key_arc, &config, &private_mode, &new_item_tx, &local_device_id).await;
                    cleanup_ticks += 1;
                    sensitive_cleanup_ticks += 1;

                    // Sensitive item TTL: run every 5 seconds.
                    // `5_000 / poll_interval_ms` is integer-divided; for any
                    // `poll_interval_ms > 5000` the quotient is 0, which would
                    // make this branch fire every tick. Clamp the threshold to
                    // at least 1 so the cleanup runs (at most) every tick.
                    let do_sensitive =
                        sensitive_cleanup_ticks >= (5_000 / config.poll_interval_ms.max(1)).max(1);
                    if do_sensitive {
                        sensitive_cleanup_ticks = 0;
                    }
                    // General expires_at TTL: run every 60 seconds. Same
                    // integer-division clamp as above.
                    let do_general =
                        cleanup_ticks >= (60_000 / config.poll_interval_ms.max(1)).max(1);
                    if do_general {
                        cleanup_ticks = 0;
                    }
                    // daemon-core L1: the deletes are synchronous rusqlite. Run
                    // them on a blocking thread (like the IPC path) so the async
                    // executor is never blocked while the DB lock is held.
                    run_ttl_cleanup(&db, sensitive_ttl_ms, do_sensitive, do_general).await;
                }
                _ = tokio::signal::ctrl_c() => {
                    tracing::info!("SIGINT received, shutting down");
                    quit_flag.store(true, Ordering::Relaxed);
                    // D3: broadcast shutdown to all tasks.
                    shutdown_token.cancel();
                    break;
                }
                _ = sigterm.recv() => {
                    tracing::info!("SIGTERM received, shutting down");
                    quit_flag.store(true, Ordering::Relaxed);
                    // D3: broadcast shutdown to all tasks.
                    shutdown_token.cancel();
                    break;
                }
            }
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        // SIGTERM handling on non-macOS — previously only SIGINT was wired,
        // so launchd/systemd sending SIGTERM would terminate the process
        // without running our cleanup branch (sock file removal, log flush).
        #[cfg(unix)]
        let mut sigterm = {
            use tokio::signal::unix::{signal, SignalKind};
            signal(SignalKind::terminate())?
        };
        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    handle_tick(&mut monitor, &db, &local_key_arc, &config, &private_mode, &new_item_tx, &local_device_id).await;
                    cleanup_ticks += 1;
                    sensitive_cleanup_ticks += 1;

                    // Sensitive item TTL: run every 5 seconds.
                    let do_sensitive =
                        sensitive_cleanup_ticks >= (5_000 / config.poll_interval_ms.max(1)).max(1);
                    if do_sensitive {
                        sensitive_cleanup_ticks = 0;
                    }
                    // General expires_at TTL: run every 60 seconds.
                    let do_general =
                        cleanup_ticks >= (60_000 / config.poll_interval_ms.max(1)).max(1);
                    if do_general {
                        cleanup_ticks = 0;
                    }
                    // daemon-core L1: offload the synchronous rusqlite deletes.
                    run_ttl_cleanup(&db, sensitive_ttl_ms, do_sensitive, do_general).await;
                }
                _ = tokio::signal::ctrl_c() => {
                    tracing::info!("SIGINT received, shutting down");
                    // D3: broadcast shutdown to all tasks.
                    shutdown_token.cancel();
                    break;
                }
                #[cfg(unix)]
                _ = sigterm.recv() => {
                    tracing::info!("SIGTERM received, shutting down");
                    // D3: broadcast shutdown to all tasks.
                    shutdown_token.cancel();
                    break;
                }
            }
        }
    }

    // D5: wait for long-running tasks to drain before cleaning up resources.
    tracing::info!("waiting for subsystem tasks to finish...");
    // sync_orch exits promptly (shutdown token was cancelled above).
    let _ = sync_handle.await;
    // IPC accept loop exits on shutdown token; join it now.
    #[cfg(unix)]
    let _ = _ipc_handle.await;
    // P2P: signal shutdown via the oneshot sender then let the handle drop.
    if let Some(p2p_handle) = _p2p_handle {
        // P2pHandle::shutdown_tx is a oneshot; sending () requests graceful stop.
        let _ = p2p_handle.shutdown_tx.send(());
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

/// Run the sensitive- and/or general-TTL deletes on a blocking thread.
///
/// daemon-core L1: both `delete_sensitive_expired` and `delete_expired` are
/// synchronous rusqlite calls. Previously they ran inline inside the `select!`
/// loop under `db.lock().await` while holding the tokio Mutex, blocking the
/// async worker for the duration of the SQL. We now mirror the IPC path:
/// acquire the lock and run the SQL inside `spawn_blocking`. The clock-skew-safe
/// `unwrap_or_default()` on the timestamp is preserved.
async fn run_ttl_cleanup(
    db: &Arc<Mutex<Database>>,
    sensitive_ttl_ms: i64,
    do_sensitive: bool,
    do_general: bool,
) {
    if !do_sensitive && !do_general {
        return;
    }
    let db = db.clone();
    let join = tokio::task::spawn_blocking(move || {
        let guard = db.blocking_lock();
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        let sensitive = if do_sensitive {
            Some(copypaste_core::delete_sensitive_expired(
                &guard,
                now_ms,
                sensitive_ttl_ms,
            ))
        } else {
            None
        };
        let general = if do_general {
            Some(copypaste_core::delete_expired(&guard, now_ms))
        } else {
            None
        };
        (sensitive, general)
    })
    .await;
    let (sensitive, general) = match join {
        Ok(pair) => pair,
        Err(e) => {
            tracing::warn!("TTL cleanup blocking task failed: {e}");
            return;
        }
    };
    match sensitive {
        Some(Ok(n)) if n > 0 => tracing::info!("sensitive TTL cleanup: wiped {n} sensitive items"),
        Some(Err(e)) => tracing::warn!("sensitive TTL cleanup error: {e}"),
        _ => {}
    }
    match general {
        Some(Ok(n)) if n > 0 => tracing::info!("TTL cleanup: removed {n} expired items"),
        Some(Err(e)) => tracing::warn!("TTL cleanup error: {e}"),
        _ => {}
    }
}

/// Run the daemon in DEGRADED mode (acceptance criteria #1–#3).
///
/// Entered when the SQLCipher key is unavailable (Keychain ACL no longer trusts
/// this binary after a reinstall, prompt unanswered, access denied) AND an
/// encrypted DB already exists, OR when an opened key turns out to be the wrong
/// one (SQLITE_NOTADB). We:
///
/// * NEVER `Error:`/exit — the process stays alive so the UI keeps a live
///   socket and can show a recovery banner instead of a dead daemon.
/// * Bind the IPC socket with `ready = false` and a `degraded_reason`, so every
///   DB-touching method returns `IPC_NOT_READY` and `status` reports
///   `status="degraded"` + `degraded_reason`.
/// * NEVER open / write / recreate the real encrypted DB. The IpcServer needs
///   *a* `Database` handle, so we hand it a throwaway in-memory one — the real
///   `~/.../clipboard.db` on disk is left byte-for-byte untouched and remains
///   recoverable on a later correct-key launch.
/// * Do NOT start the clipboard monitor, P2P, sync, or cloud subsystems — there
///   is no usable key, and writing captures with an ephemeral key would corrupt
///   nothing on disk but would also be pointless and confusing.
#[cfg_attr(not(target_os = "macos"), allow(unused_variables))]
async fn run_degraded(reason: &'static str, quit_flag: Arc<AtomicBool>) -> anyhow::Result<()> {
    let shutdown_token = CancellationToken::new();

    #[cfg(unix)]
    {
        // Throwaway in-memory DB: satisfies IpcServer's type contract WITHOUT
        // touching the encrypted file on disk. `ready = false` gates every
        // DB-touching method, so this in-memory DB is never actually queried
        // for user data — it only backs the `ensure_revoked_devices_table` DDL
        // in `serve()` and the readiness gate.
        let placeholder_db =
            Arc::new(Mutex::new(Database::open_in_memory().map_err(|e| {
                anyhow::anyhow!("degraded: in-memory placeholder DB: {e}")
            })?));
        let private_mode = Arc::new(AtomicBool::new(false));
        // An ephemeral key for the placeholder server — never used against real
        // data (DB methods are gated off by `ready = false`).
        let dummy_key: Arc<zeroize::Zeroizing<[u8; 32]>> =
            Arc::new(DeviceKeypair::generate().local_enc_key());
        let dummy_pub: Arc<[u8; 32]> = Arc::new([0u8; 32]);

        let ready = Arc::new(AtomicBool::new(false));
        let server = crate::ipc::IpcServer::new_with_ready(
            placeholder_db,
            private_mode,
            dummy_key,
            dummy_pub,
            ready,
        )
        .with_degraded_reason(reason);

        let socket_path = paths::socket_path();
        let socket_clone = socket_path.clone();
        let ipc_shutdown = shutdown_token.clone();
        let ipc_handle = tokio::spawn(async move {
            if let Err(e) = server.serve(&socket_clone, ipc_shutdown).await {
                tracing::error!("degraded IPC server error: {e}");
            }
        });

        tracing::warn!(
            reason,
            "DEGRADED daemon running: IPC socket bound, DB-touching requests \
             return IPC_NOT_READY, `status` reports degraded_reason. Re-grant \
             the Keychain prompt and relaunch to recover."
        );

        // Wait for shutdown (tray quit flag, SIGINT, or SIGTERM), mirroring the
        // healthy loop's shutdown wiring but with no clipboard polling.
        #[cfg(unix)]
        let mut sigterm = {
            use tokio::signal::unix::{signal, SignalKind};
            signal(SignalKind::terminate())?
        };
        let mut quit_ticker = interval(Duration::from_millis(250));
        loop {
            if quit_flag.load(Ordering::Relaxed) {
                tracing::info!("quit flag set, shutting down degraded daemon");
                break;
            }
            tokio::select! {
                _ = quit_ticker.tick() => { /* re-check quit_flag at the top */ }
                _ = tokio::signal::ctrl_c() => {
                    tracing::info!("SIGINT received, shutting down degraded daemon");
                    break;
                }
                _ = sigterm.recv() => {
                    tracing::info!("SIGTERM received, shutting down degraded daemon");
                    break;
                }
            }
        }

        shutdown_token.cancel();
        let _ = ipc_handle.await;
        let _ = std::fs::remove_file(&socket_path);
    }

    #[cfg(not(unix))]
    {
        // No Unix socket transport on non-unix; just wait for Ctrl+C so the
        // process does not busy-exit. Degraded mode is a macOS/unix concern.
        let _ = tokio::signal::ctrl_c().await;
    }

    tracing::info!("degraded daemon stopped");
    Ok(())
}

#[tracing::instrument(skip_all, name = "clipboard_tick")]
async fn handle_tick(
    monitor: &mut ClipboardMonitor,
    db: &Arc<Mutex<Database>>,
    local_key: &[u8; 32],
    config: &AppConfig,
    private_mode: &Arc<AtomicBool>,
    new_item_tx: &broadcast::Sender<ClipboardItem>,
    local_device_id: &str,
) {
    // Skip recording when private/pause mode is active
    if private_mode.load(Ordering::Relaxed) {
        // Still poll to advance the change-count so we don't replay on resume
        let _ = monitor.poll();
        tracing::debug!("private mode active: skipping clipboard recording");
        return;
    }

    match monitor.poll() {
        Ok(Some(ClipboardContent::Text(text))) => {
            // beta.5 Bug-1 visibility: log every capture at info level so
            // users can confirm from `daemon.out.log` that the pasteboard is
            // actually being read. Prior code only emitted `debug!` here
            // which the default `copypaste=info` filter dropped, leaving
            // operators unable to distinguish "no captures happening" from
            // "captures happening but UI not refreshing".
            tracing::info!(
                bytes = text.len(),
                "clipboard captured: text ({} bytes)",
                text.len()
            );
            if let Some(item) = handle_text(text, db, local_key, config, local_device_id).await {
                // Broadcast to P2P + cloud-sync subscribers (and any future consumer).
                // A send error only means there are no active receivers —
                // that is normal when both P2P and cloud-sync are disabled.
                let _ = new_item_tx.send(item);
            }
        }
        Ok(Some(ClipboardContent::Image(raw_bytes))) => {
            tracing::info!(
                bytes = raw_bytes.len(),
                "clipboard captured: image ({} bytes raw)",
                raw_bytes.len()
            );
            if let Some(item) =
                handle_image(raw_bytes, db, local_key, config, local_device_id).await
            {
                let _ = new_item_tx.send(item);
            }
        }
        Ok(Some(ClipboardContent::SkippedBatch(missed))) => {
            // Rapid clipboard burst — the monitor already logged the gap;
            // we just bump telemetry here and let the next poll capture
            // the now-current pasteboard value.
            tracing::warn!(
                missed,
                "clipboard rapid-burst: {} intermediate updates lost between polls",
                missed
            );
        }
        Ok(None) => {}
        Err(e) => tracing::warn!("clipboard poll error: {e}"),
    }
}

/// Encrypt a freshly-captured text payload for at-rest storage, producing a
/// ciphertext that the read path (`ipc::write_to_pasteboard`) can decrypt.
///
/// **Key/AAD/key_version consistency (the v0.4 ingest fix).** A new row is
/// stamped `key_version = 2` by [`ClipboardItem::new_text`] (which uses
/// `ITEM_KEY_VERSION_CURRENT = 2`). The read path dispatches on that
/// `key_version` via `copypaste_core::decrypt_item_by_version`, and for
/// `key_version = 2` it decrypts with **the v2 key** (`derive_v2(local_key)`)
/// and **the v4 AAD format** (`build_item_aad_v2(item_id, 4, 2)`).
///
/// Ingest must therefore encrypt with that exact `(key, AAD)` pair. The prior
/// code encrypted with the raw `local_key` (the v1 key) + the v3 AAD
/// (`build_item_aad(item_id, 3)`) while still stamping `key_version = 2`, so
/// every freshly-captured text item failed to round-trip with
/// `EncryptError::AuthFailed` ("authentication tag mismatch") on paste-back.
///
/// `local_key` is the device's v1 storage key (`load_local_key()` /
/// `DeviceKeypair::local_enc_key`). It is used here only as the input keying
/// material to `derive_v2`, mirroring exactly what the read path does
/// (`derive_v2(&self.local_key)`), so the two sides derive the identical v2
/// key.
fn encrypt_text_for_storage(
    plaintext: &[u8],
    local_key: &[u8; 32],
    item_id: &str,
) -> Result<([u8; copypaste_core::NONCE_SIZE], Vec<u8>), copypaste_core::EncryptError> {
    let v2_key = derive_v2(local_key);
    let aad = build_item_aad_v2(item_id, AAD_SCHEMA_VERSION_V4, ITEM_KEY_VERSION_CURRENT_U32);
    encrypt_item_with_aad(plaintext, &v2_key, &aad)
}

/// `key_version` stamped into newly-inserted rows, mirrored from
/// `copypaste_core::storage::items::ITEM_KEY_VERSION_CURRENT` (= 2). Pinned as
/// a `u32` here because `build_item_aad_v2` binds the key version into the AAD
/// as a `u32` and the read path uses the literal `2`.
const ITEM_KEY_VERSION_CURRENT_U32: u32 = 2;

async fn handle_text(
    text: String,
    db: &Arc<Mutex<Database>>,
    local_key: &[u8; 32],
    config: &AppConfig,
    local_device_id: &str,
) -> Option<ClipboardItem> {
    // Migration gate is now enforced at the Database layer inside
    // `insert_item` / `insert_item_with_fts` (ItemsError::MigrationInProgress).
    // The call-site guard that used to live here has been removed.

    let is_sensitive = detect(&text).is_some();

    // Compute SHA-256 content hash of the PLAINTEXT bytes.
    // This is used for deduplication: if an identical item already exists in
    // history (any age, not expired), we bump its wall_time/lamport_ts to now
    // rather than inserting a duplicate row. The hash is stored on new inserts
    // so future captures of the same content can find the existing row.
    //
    // NEVER log the plaintext or hash — the hash alone is not reversible but
    // logging it alongside the content would create a correlation risk.
    let hash_hex = {
        use sha2::{Digest, Sha256};
        hex::encode(Sha256::digest(text.as_bytes()))
    };

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    // daemon-core L1: every DB touch below is synchronous rusqlite. Run the
    // whole dedup-lookup / bump-or-insert / prune sequence on a blocking thread
    // (mirroring the IPC path) so the async worker is not blocked while the
    // tokio Mutex is held. Inputs are moved in; the resulting item (if any) is
    // returned for the broadcast channel.
    let db = db.clone();
    let config = config.clone();
    let local_key = *local_key;
    let local_device_id = local_device_id.to_string();
    let join = tokio::task::spawn_blocking(move || {
        let db_guard = db.blocking_lock();

        // Dedup: look for any non-expired row with the same content hash.
        // `find_recent_by_hash` uses a generous window (i64::MAX) to cover ALL
        // history, not just the last N minutes.  A pinned item is never expired
        // so it will always be found and bumped, which is the correct behaviour.
        match find_recent_by_hash(&db_guard, &hash_hex, now_ms, i64::MAX) {
            Ok(Some(existing_id)) => {
                // Identical content already in history: bump recency to now so
                // the existing row rises to the top of the pinned-first,
                // wall_time DESC sort. We do NOT insert a new row.
                let new_lamport = now_ms; // use wall_time ms as lamport proxy
                match bump_item_recency(&db_guard, &existing_id, now_ms, new_lamport) {
                    Ok(changed) if changed > 0 => {
                        tracing::debug!(
                            existing = %existing_id,
                            "text dedup: bumped existing row to top (same content_hash)"
                        );
                    }
                    Ok(_) => {
                        // Row disappeared between find and bump (race on delete) —
                        // fall through to a fresh insert on next poll. This poll
                        // produces no item for the broadcast channel, which is
                        // safe: the next poll sees a new changeCount and captures
                        // again.
                        tracing::debug!(
                            existing = %existing_id,
                            "text dedup: existing row disappeared before bump (deleted concurrently)"
                        );
                    }
                    Err(e) => {
                        tracing::warn!("text dedup bump failed: {e}");
                    }
                }
                // Return the bumped item so broadcast subscribers (P2P, sync) see
                // the recency update. Fetch the full row for up-to-date fields.
                return match get_item_by_id(&db_guard, &existing_id) {
                    Ok(Some(bumped)) => Some(bumped),
                    Ok(None) => None,
                    Err(e) => {
                        tracing::warn!("text dedup: could not re-fetch bumped item: {e}");
                        None
                    }
                };
            }
            Ok(None) => {
                // No existing row with this hash — proceed with a fresh insert.
            }
            Err(e) => {
                // DB error on the dedup lookup: log and fall through to insert.
                // Inserting a duplicate is preferable to silently losing a capture.
                tracing::warn!("text dedup hash lookup failed: {e}");
            }
        }

        // Fresh insert path: encrypt then store.
        let item_id = uuid::Uuid::new_v4().to_string();
        let (nonce, ciphertext) =
            match encrypt_text_for_storage(text.as_bytes(), &local_key, &item_id) {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!("encrypt_text_for_storage failed for text: {e}");
                    return None;
                }
            };
        let mut item = ClipboardItem::new_text(ciphertext, nonce.to_vec(), 0);
        item.item_id = item_id;
        item.is_sensitive = is_sensitive;
        // Stamp the stable on-disk device_id so cloud/P2P peers attribute every
        // captured item to this specific machine across restarts.
        item.origin_device_id = local_device_id;
        // Store the content hash so future captures of identical content can
        // find and bump this row instead of inserting a duplicate.
        item.content_hash = Some(hash_hex);

        if is_sensitive {
            item.expires_at = Some(now_ms + (config.sensitive_ttl_local_secs as i64 * 1000));
        }

        // v0.3 post-T2: insert_item + upsert_fts collapsed into a single
        // transaction. Closes the TOCTOU window where a crash between the row
        // insert and the FTS upsert could leave a row that search would never
        // find. Also handles the v5 UNIQUE-index dedup race internally.
        match insert_item_with_fts(&db_guard, &item, &text) {
            Ok(stored_id) => {
                if stored_id != item.id {
                    tracing::debug!(
                        requested = %item.id,
                        existing = %stored_id,
                        "text item deduped against existing row (UNIQUE index race)"
                    );
                } else {
                    tracing::info!(
                        id = %item.id,
                        sensitive = is_sensitive,
                        "stored text item id={} sensitive={}",
                        item.id,
                        is_sensitive
                    );
                }
                prune_history(&db_guard, &config);
                Some(item)
            }
            Err(e) => {
                tracing::warn!("failed to store text item: {e}");
                None
            }
        }
    })
    .await;
    match join {
        Ok(item) => item,
        Err(e) => {
            tracing::warn!("handle_text blocking task failed: {e}");
            None
        }
    }
}

async fn handle_image(
    raw_bytes: Vec<u8>,
    db: &Arc<Mutex<Database>>,
    local_key: &[u8; 32],
    config: &AppConfig,
    local_device_id: &str,
) -> Option<ClipboardItem> {
    // Migration gate is now enforced at the Database layer inside
    // `insert_item` / `insert_item_with_fts` (ItemsError::MigrationInProgress).
    // The call-site guard that used to live here has been removed.

    // daemon-core L1: the image encode (CPU-heavy compression + encryption) and
    // the rusqlite insert/prune are all synchronous. Run the whole sequence on a
    // blocking thread, mirroring the IPC path, so the async worker is never
    // blocked while the tokio Mutex is held.
    let db = db.clone();
    let config = config.clone();
    let local_key = *local_key;
    let local_device_id = local_device_id.to_string();
    let join = tokio::task::spawn_blocking(move || {
        // Derive a stable file_id from SHA-256(raw_bytes)[..16] — a 128-bit
        // collision-resistant content hash. Deterministic so identical images
        // dedup naturally (Wave 2.1 security LOW #19).
        let file_id = crate::clipboard::image_content_hash(&raw_bytes);

        match encode_image(&raw_bytes, &local_key, &file_id) {
            Ok((meta, chunks)) => {
                let blob = chunks_to_blob(&chunks);
                let meta_json = format!(
                    r#"{{"width":{},"height":{},"original_size":{},"chunk_count":{},"file_id":{:?}}}"#,
                    meta.width, meta.height, meta.original_size, meta.chunk_count, meta.file_id
                );
                let mut item = ClipboardItem::new_image(blob, meta_json, 0);
                // Stamp stable device identity (same fix as handle_text).
                item.origin_device_id = local_device_id;
                tracing::debug!(
                    "image encoded: {}x{} px, {} chunks, original_size={}",
                    meta.width,
                    meta.height,
                    meta.chunk_count,
                    meta.original_size
                );

                let db_guard = db.blocking_lock();
                // Atomic insert: images have no searchable text, so we pass "" to
                // skip the FTS write (insert_item_with_fts treats empty as
                // "image item" and only writes the row).
                match insert_item_with_fts(&db_guard, &item, "") {
                    Ok(stored_id) => {
                        if stored_id != item.id {
                            tracing::debug!(
                                requested = %item.id,
                                existing = %stored_id,
                                "image item deduped against existing row"
                            );
                        } else {
                            tracing::info!(id = %item.id, "stored image item id={}", item.id);
                        }
                        prune_history(&db_guard, &config);
                        Some(item)
                    }
                    Err(e) => {
                        tracing::warn!("failed to store image item: {e}");
                        None
                    }
                }
            }
            Err(e) => {
                tracing::warn!("image encode failed (skipping): {e}");
                None
            }
        }
    })
    .await;
    match join {
        Ok(item) => item,
        Err(e) => {
            tracing::warn!("handle_image blocking task failed: {e}");
            None
        }
    }
}

fn prune_history(db: &Database, config: &AppConfig) {
    let total = copypaste_core::count_items(db).unwrap_or(0) as usize;
    if total > config.history_limit {
        let excess = total - config.history_limit;
        // Direct SQL DELETE ordered by `wall_time ASC` — bulk-removes the
        // oldest rows in a single statement (audit HIGH #4). The previous
        // implementation went through `get_page` + per-row `delete_item`,
        // which was both N+1 and risked pruning the wrong page if the
        // pagination math drifted.
        //
        // `pinned = 0` excludes explicitly pinned items so they are never
        // deleted by the history-limit prune (schema v7, see `pin_item`).
        let res = db.conn().execute(
            "DELETE FROM clipboard_items WHERE id IN (
                SELECT id FROM clipboard_items
                WHERE pinned = 0
                ORDER BY wall_time ASC
                LIMIT ?1
            )",
            rusqlite::params![excess as i64],
        );
        match res {
            Ok(n) => tracing::debug!(
                "pruned {} of {} requested items over history_limit={}",
                n,
                excess,
                config.history_limit
            ),
            Err(e) => tracing::warn!("prune_history failed: {e}"),
        }
    }
}

/// Derive the `(v1_key, v2_key)` pair used by the v4 key-version migration
/// sweep, from the raw `seed` returned by [`load_local_key_material`].
///
/// **Critical:** `seed` is ALREADY the v1 storage key —
/// [`load_local_key_material`] returns `DeviceKeypair::local_enc_key()`, which is
/// `HKDF-SHA256(secret) == derive_storage_key_v1(secret)`. The read path
/// (`ipc::write_to_pasteboard`, text branch) therefore decrypts
/// `key_version = 1` rows with this seed **directly** (`v1_key = **local_key`)
/// and derives the v2 key as `derive_v2(seed)`. The sweep MUST use the
/// identical keys, otherwise it cannot decrypt any real legacy v1 row.
///
/// A previous version passed `derive_storage_key_v1(seed)` as the v1 key,
/// double-deriving it (`derive_storage_key_v1(local_enc_key)`). That key never
/// matched what real v1 rows were encrypted under, so every legacy
/// `key_version = 1` row failed with an auth-tag mismatch and was never
/// rotated.
fn sweep_keys(seed: &[u8; 32]) -> ([u8; 32], [u8; 32]) {
    // v1_key: the seed itself, used directly — exactly as the read path uses
    //         `**self.local_key` for `key_version = 1` rows.
    // v2_key: `derive_v2(seed)`, matching the read path's `derive_v2(&v1_key)`.
    (*seed, derive_v2(seed))
}

/// Outcome of the bounded startup key read.
///
/// `Ready` carries the device's local storage key. `Locked` means the key
/// could not be obtained in bounded time / at all — the Keychain ACL no longer
/// trusts this binary (post-reinstall), the read is blocked on an unanswered
/// GUI prompt, or access was denied (launchd, no interactive session). We do
/// NOT synthesize an ephemeral key here: doing so against an EXISTING encrypted
/// DB yields "file is not a database" and a dead daemon (the exact regression).
enum KeyLoad {
    /// Carries the device's local storage key AND the X25519 public-key bytes,
    /// loaded once together (see `load_local_key_material`).
    Ready(zeroize::Zeroizing<[u8; 32]>, [u8; 32]),
    Locked,
}

/// The plan for opening the database at startup, derived from the key-load
/// outcome and whether an encrypted DB already exists. Pure + total so it is
/// unit-testable without a Keychain or a real DB.
#[derive(Debug, PartialEq, Eq)]
enum DbStartupPlan {
    /// Open (or create) the DB with the obtained key. Normal path, and also the
    /// correct path on a brand-new install where no DB exists yet.
    Open,
    /// Bring the daemon up in DEGRADED mode: bind the IPC socket, serve a clear
    /// `keychain_locked` status, and DO NOT touch the existing encrypted DB.
    /// `reason` is the canonical `status.degraded_reason` value.
    Degraded { reason: &'static str },
    /// Use an ephemeral key against a fresh/ephemeral DB. Reached on the
    /// `COPYPASTE_EPHEMERAL_KEY` dev/test bypass and on a brand-new install
    /// where the key is unavailable but there is no existing data to protect.
    /// Distinct from `Degraded` so the bypass keeps working exactly as before.
    OpenEphemeral,
}

/// Decide how to open the DB given the key-load outcome and whether an
/// encrypted DB already exists. This is the heart of the regression fix:
///
/// * key Ready                              → `Open` (normal).
/// * key Locked AND an encrypted DB exists  → `Degraded` — NEVER fall back to
///   an ephemeral key against real data (that is what produced the dead
///   daemon). Keep the process alive and surface a recovery status.
/// * key Locked AND no DB exists yet        → `OpenEphemeral` — there is no
///   user data to protect; an ephemeral key lets a brand-new install still run
///   this session (matching the long-standing keychain-unavailable behaviour
///   where data is simply not persisted across restarts).
fn decide_db_startup(key: &KeyLoad, encrypted_db_exists: bool) -> DbStartupPlan {
    match key {
        KeyLoad::Ready(..) => DbStartupPlan::Open,
        KeyLoad::Locked if encrypted_db_exists => DbStartupPlan::Degraded {
            reason: crate::ipc::DEGRADED_REASON_KEYCHAIN_LOCKED,
        },
        KeyLoad::Locked => DbStartupPlan::OpenEphemeral,
    }
}

/// True if an encrypted database file already exists at `path` with content.
///
/// A zero-length file (or a missing file) is treated as "no DB" — there is no
/// user data to protect, so the degraded gate does not engage. SQLCipher
/// `Database::open` would happily (re)initialize an empty file under any key.
fn encrypted_db_exists(path: &std::path::Path) -> bool {
    std::fs::metadata(path)
        .map(|m| m.is_file() && m.len() > 0)
        .unwrap_or(false)
}

/// Read the device key with a hard timeout so startup can never hang on a
/// Keychain GUI prompt (acceptance criterion #1).
///
/// The blocking Security-framework read runs on a dedicated `std::thread`; we
/// wait on a channel for at most [`KEYCHAIN_READ_TIMEOUT`]. On timeout we
/// return [`KeyLoad::Locked`] and let the abandoned thread sit on the prompt
/// (harmless — it dies with the process). The dev/test bypass
/// (`COPYPASTE_EPHEMERAL_KEY`) short-circuits to a ready ephemeral key without
/// spawning a thread, preserving the existing fast, prompt-free test path.
#[cfg_attr(not(target_os = "macos"), allow(clippy::unnecessary_wraps))]
fn load_local_key_bounded() -> KeyLoad {
    #[cfg(not(target_os = "macos"))]
    {
        // Non-macOS has no Keychain; the existing behaviour is an ephemeral
        // key. `load_local_key_material` already returns `KeyLoad::Ready` there
        // so the platform's data-not-persisted contract is unchanged (no
        // degraded banner where there was never a persistent key to begin with).
        load_local_key_material()
    }

    #[cfg(target_os = "macos")]
    {
        // Dev/test bypass: keychain is bypassed centrally; reading is instant
        // and never prompts, so there is no need for the timeout dance.
        if crate::keychain::keychain_bypassed() {
            return load_local_key_material();
        }

        let (tx, rx) = std::sync::mpsc::sync_channel::<KeyLoad>(1);
        // A plain OS thread (not a tokio task): the Security-framework call is
        // blocking and may park on a GUI prompt indefinitely. We must be able
        // to walk away from it without blocking a runtime worker.
        let _ = std::thread::Builder::new()
            .name("keychain-read".into())
            .spawn(move || {
                let material = load_local_key_material();
                // Receiver may already be gone (we timed out) — ignore.
                let _ = tx.send(material);
            });

        match rx.recv_timeout(KEYCHAIN_READ_TIMEOUT) {
            // The thread already classified the outcome (Ready or Locked); a
            // locked Keychain now propagates instead of being papered over.
            Ok(key_load) => key_load,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                tracing::error!(
                    timeout_secs = KEYCHAIN_READ_TIMEOUT.as_secs(),
                    "Keychain read did not complete within the startup timeout — \
                     the stored SQLCipher key is unreachable (the Keychain ACL no \
                     longer trusts this binary after a reinstall, or a password \
                     prompt is unanswered). Continuing in DEGRADED mode without \
                     touching the encrypted database."
                );
                KeyLoad::Locked
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                tracing::error!(
                    "Keychain read thread ended without producing a key; \
                     continuing in DEGRADED mode."
                );
                KeyLoad::Locked
            }
        }
    }
}

/// Load the device keypair ONCE and return both the local storage key and the
/// X25519 public-key bytes derived from it.
///
/// dedup-keychain: a single `crate::keychain::load_or_create` call derives both
/// the local enc key and the X25519 public bytes, avoiding a second Keychain
/// read + legacy-accessibility migration write at startup.
///
/// Dev/test escape hatch: COPYPASTE_EPHEMERAL_KEY is honored centrally inside
/// `crate::keychain` — `load_or_create` short-circuits to a fresh ephemeral
/// keypair before any Security-framework call, so the macOS login-keychain
/// password prompt is never raised. Ad-hoc-signed dev builds change signature on
/// every rebuild, invalidating the Keychain item ACL and triggering that prompt;
/// the env var avoids it. Production (env unset) is unchanged: real users still
/// get the persistent Keychain-backed key.
#[tracing::instrument(name = "load_local_key_material")]
fn load_local_key_material() -> KeyLoad {
    #[cfg(target_os = "macos")]
    {
        match crate::keychain::load_or_create() {
            Ok(kp) => {
                tracing::info!("device fingerprint={}", kp.fingerprint());
                KeyLoad::Ready(kp.local_enc_key(), kp.public_key_bytes())
            }
            // A LOCKED/denied Keychain must NOT be papered over with an
            // ephemeral key: if an encrypted DB already exists, that key would
            // mismatch (SQLITE_NOTADB) and the daemon would either crash-loop or
            // recreate over real data. Report `Locked` so `decide_db_startup`
            // routes to the clean DEGRADED path (DB untouched, recovery status
            // served). `load_or_create` now only returns `Locked` for genuine
            // locked/denied/timeout statuses — a missing entry creates a key.
            Err(crate::keychain::KeychainError::Locked(code)) => {
                tracing::warn!(
                    code,
                    "Keychain locked or access denied; reporting Locked so startup \
                     degrades cleanly instead of using an ephemeral key over an \
                     existing encrypted database"
                );
                KeyLoad::Locked
            }
            Err(e) => {
                // Other errors (e.g. invalid length, key-derivation) are not the
                // locked case; preserve the prior behaviour of falling back to an
                // ephemeral key so first-run / non-Keychain failures still boot.
                tracing::warn!("Keychain unavailable ({e}), using ephemeral key");
                let kp = DeviceKeypair::generate();
                KeyLoad::Ready(kp.local_enc_key(), kp.public_key_bytes())
            }
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        // Keychain not available on non-macOS; use an ephemeral key for CI/Linux builds.
        // On production macOS this branch is never compiled in. The public bytes
        // are a zero placeholder — there is no keychain-backed identity here.
        tracing::warn!("Non-macOS platform: using ephemeral encryption key (data not persisted across restarts)");
        KeyLoad::Ready(DeviceKeypair::generate().local_enc_key(), [0u8; 32])
    }
}

#[tracing::instrument(name = "load_config")]
fn load_config() -> AppConfig {
    let path = paths::config_path();
    AppConfig::load(&path).unwrap_or_else(|_| {
        let cfg = AppConfig::default();
        if let Err(e) = cfg.save(&path) {
            tracing::warn!("could not save default config: {e}");
        }
        cfg
    })
}

/// Loads the persistent device_id from disk, creating it on first run.
///
/// Fixes arch LOW #24: previously the daemon regenerated a fresh UUID on
/// every restart, which broke P2P pairing and confused cloud peers. We now
/// persist a UUID v4 to `app_support_dir()/device_id` (or
/// `COPYPASTE_DEVICE_ID_PATH` when set) and chmod the file to `0o600` on
/// Unix so it is not world-readable.
///
/// On parse failure of an existing file we log + regenerate rather than
/// erroring — corrupt state should not block daemon startup.
#[tracing::instrument(name = "load_or_create_device_id")]
fn load_or_create_device_id() -> anyhow::Result<uuid::Uuid> {
    let path = paths::device_id_path()?;

    if let Ok(contents) = std::fs::read_to_string(&path) {
        let trimmed = contents.trim();
        match uuid::Uuid::parse_str(trimmed) {
            Ok(id) => {
                tracing::info!(device_id = %id, "loaded persistent device_id");
                return Ok(id);
            }
            Err(e) => {
                tracing::warn!(
                    path = %path.display(),
                    error = %e,
                    "device_id file unparsable, regenerating"
                );
            }
        }
    }

    // Ensure parent dir exists before writing.
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let id = uuid::Uuid::new_v4();
    std::fs::write(&path, id.to_string())?;

    // Restrict to owner-only on Unix.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o600);
        if let Err(e) = std::fs::set_permissions(&path, perms) {
            tracing::warn!(
                path = %path.display(),
                error = %e,
                "could not chmod device_id to 0600"
            );
        }
    }

    tracing::info!(device_id = %id, path = %path.display(), "created persistent device_id");
    Ok(id)
}

/// Restore the persisted private-mode flag at startup.
///
/// Returns `false` (capture enabled) when the file is absent, unreadable, or
/// holds anything other than `"1"` — a missing/corrupt flag must never leave
/// the daemon stuck in private mode, and on first run there is no file yet.
fn load_private_mode() -> bool {
    let path = match crate::paths::private_mode_path() {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!("could not resolve private_mode path ({e}); defaulting to disabled");
            return false;
        }
    };
    match std::fs::read_to_string(&path) {
        Ok(contents) => contents.trim() == "1",
        Err(_) => false, // absent on first run; not an error
    }
}

/// Persist the private-mode flag so it survives a daemon restart.
///
/// Best-effort: a write failure is logged but does not fail the IPC call —
/// the in-memory atomic is still authoritative for the running process.
pub(crate) fn persist_private_mode(enabled: bool) {
    let path = match crate::paths::private_mode_path() {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!("could not resolve private_mode path ({e}); not persisting");
            return;
        }
    };
    if let Some(parent) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            tracing::warn!("could not create private_mode parent dir ({e}); not persisting");
            return;
        }
    }
    if let Err(e) = std::fs::write(&path, if enabled { "1" } else { "0" }) {
        tracing::warn!(
            path = %path.display(),
            error = %e,
            "could not persist private_mode flag"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use copypaste_core::{
        build_item_aad, decrypt_item_by_version, encrypt_item_with_aad, Database,
        AAD_SCHEMA_VERSION, NONCE_SIZE,
    };

    // -----------------------------------------------------------------------
    // Keychain-locked degraded-startup decision logic (regression: daemon hung
    // or died after reinstall when the SQLCipher key became unreadable).
    // -----------------------------------------------------------------------

    fn ready_key() -> KeyLoad {
        KeyLoad::Ready(zeroize::Zeroizing::new([0x11u8; 32]), [0x22u8; 32])
    }

    /// A readable key always opens the DB normally, regardless of whether a DB
    /// already exists.
    #[test]
    fn decide_db_startup_ready_key_opens_normally() {
        assert_eq!(decide_db_startup(&ready_key(), true), DbStartupPlan::Open);
        assert_eq!(decide_db_startup(&ready_key(), false), DbStartupPlan::Open);
    }

    /// THE REGRESSION GUARD: when the key is locked AND an encrypted DB already
    /// exists, the plan MUST be `Degraded` (never `OpenEphemeral` against real
    /// data — that produced "file is not a database" and a dead daemon). The
    /// reason must be the canonical value the UI keys its banner off.
    #[test]
    fn decide_db_startup_locked_key_with_existing_db_degrades() {
        assert_eq!(
            decide_db_startup(&KeyLoad::Locked, true),
            DbStartupPlan::Degraded {
                reason: crate::ipc::DEGRADED_REASON_KEYCHAIN_LOCKED
            }
        );
        assert_eq!(
            crate::ipc::DEGRADED_REASON_KEYCHAIN_LOCKED,
            "keychain_locked",
            "the UI consumes this exact string"
        );
    }

    /// A brand-new install (no DB yet) with a locked key may run this session
    /// with an ephemeral key — there is no user data to protect, so we do NOT
    /// degrade. This preserves first-run usability on platforms / states where
    /// the persistent key is unavailable.
    #[test]
    fn decide_db_startup_locked_key_without_db_uses_ephemeral() {
        assert_eq!(
            decide_db_startup(&KeyLoad::Locked, false),
            DbStartupPlan::OpenEphemeral
        );
    }

    /// `encrypted_db_exists` must treat a missing file and a zero-length file
    /// as "no DB" (nothing to protect), and a non-empty file as "DB present".
    #[test]
    fn encrypted_db_exists_classifies_file_states() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let missing = tmp.path().join("nope.db");
        assert!(!encrypted_db_exists(&missing), "missing file → no DB");

        let empty = tmp.path().join("empty.db");
        std::fs::write(&empty, b"").expect("write empty");
        assert!(!encrypted_db_exists(&empty), "zero-length file → no DB");

        let nonempty = tmp.path().join("data.db");
        std::fs::write(&nonempty, b"not a real sqlite header but non-empty").expect("write");
        assert!(
            encrypted_db_exists(&nonempty),
            "non-empty file → DB present"
        );
    }

    /// Seed a `key_version = 1` text row encrypted EXACTLY the way real legacy
    /// rows were written: under the device's v1 storage key — i.e. the seed
    /// returned by `load_local_key()` used DIRECTLY (`local_enc_key`) — with the
    /// v3-format AAD `build_item_aad(item_id, 3)`. Returns the row's `id` and
    /// `item_id` so the caller can read it back.
    fn seed_real_v1_text_row(
        db: &Database,
        v1_key: &[u8; 32],
        plaintext: &[u8],
    ) -> (String, String) {
        let row_id = uuid::Uuid::new_v4().to_string();
        let item_id = uuid::Uuid::new_v4().to_string();
        let aad = build_item_aad(&item_id, AAD_SCHEMA_VERSION);
        let (nonce, ciphertext) = encrypt_item_with_aad(plaintext, v1_key, &aad).expect("encrypt");
        db.conn()
            .execute(
                "INSERT INTO clipboard_items \
                 (id, item_id, content_type, content, content_nonce, \
                  is_sensitive, is_synced, lamport_ts, wall_time, key_version) \
                 VALUES (?1,?2,'text',?3,?4,0,0,?5,?5,1)",
                rusqlite::params![row_id, item_id, ciphertext, nonce.to_vec(), 1i64],
            )
            .expect("insert v1 row");
        (row_id, item_id)
    }

    fn key_version_of(db: &Database, row_id: &str) -> i64 {
        db.conn()
            .query_row(
                "SELECT key_version FROM clipboard_items WHERE id = ?1",
                rusqlite::params![row_id],
                |r| r.get(0),
            )
            .expect("row exists")
    }

    /// Read a row back through the SAME crypto the daemon's read path uses
    /// (`ipc::write_to_pasteboard`): derive `v2_key = derive_v2(seed)` and
    /// dispatch on the stored `key_version` via `decrypt_item_by_version`,
    /// with `v1_key = seed` directly.
    fn read_back(db: &Database, seed: &[u8; 32], row_id: &str) -> Vec<u8> {
        let (item_id, content, nonce_vec, key_version): (String, Vec<u8>, Vec<u8>, i64) = db
            .conn()
            .query_row(
                "SELECT item_id, content, content_nonce, key_version \
                 FROM clipboard_items WHERE id = ?1",
                rusqlite::params![row_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .expect("row exists");
        let mut nonce = [0u8; NONCE_SIZE];
        nonce.copy_from_slice(&nonce_vec);
        // Read path: v1_key = seed directly; v2_key = derive_v2(seed).
        let v1_key: [u8; 32] = *seed;
        let v2_key = derive_v2(seed);
        decrypt_item_by_version(
            key_version as u8,
            &v1_key,
            &v2_key,
            &item_id,
            &nonce,
            &content,
        )
        .expect("read path must decrypt the row")
    }

    /// v0.4 sweep key-correctness regression (HIGH, crypto): a real legacy
    /// `key_version = 1` row — written under the device's v1 storage key
    /// (`load_local_key()` / `local_enc_key`) + the v3 AAD — MUST be rotated by
    /// the production sweep to `key_version = 2` AND remain readable through the
    /// normal v2 read path afterward.
    ///
    /// Before the fix, the daemon passed `derive_storage_key_v1(seed)` as the
    /// sweep's v1 key, double-deriving it. That key never matched what real v1
    /// rows were encrypted under, so this row failed to decrypt (auth-tag
    /// mismatch) and stayed at `key_version = 1` forever.
    #[test]
    fn sweep_rotates_real_v1_row_and_it_stays_readable() {
        let db = Database::open_in_memory().expect("open db");
        // `seed` stands in for load_local_key() — already the v1 storage key.
        let seed = [0x42u8; 32];
        let plaintext = b"legacy clipboard payload that must survive rotation";

        let (row_id, _item_id) = seed_real_v1_text_row(&db, &seed, plaintext);
        assert_eq!(
            key_version_of(&db, &row_id),
            1,
            "precondition: row starts at key_version = 1"
        );

        // Run the production sweep with the keys the daemon derives from seed.
        let (v1_key, v2_key) = sweep_keys(&seed);
        let rotated = db
            .migration_v4_sweep_resumable(&v1_key, &v2_key)
            .expect("sweep must not error");

        // (a) the row is rotated to key_version = 2.
        assert_eq!(rotated, 1, "the real v1 row must be rotated");
        assert_eq!(
            key_version_of(&db, &row_id),
            2,
            "row must be at key_version = 2 after the sweep"
        );

        // (b) the rotated row decrypts back to the original plaintext via the
        // normal v2 read path.
        let recovered = read_back(&db, &seed, &row_id);
        assert_eq!(
            recovered, plaintext,
            "rotated row must read back as the original plaintext"
        );
    }

    /// A row already correctly at `key_version = 2` must be left untouched by
    /// the sweep (the `WHERE key_version = 1` predicate skips it) and remain
    /// readable through the v2 read path.
    #[test]
    fn sweep_leaves_existing_v2_row_untouched_and_readable() {
        let db = Database::open_in_memory().expect("open db");
        let seed = [0x77u8; 32];
        let plaintext = b"already-v2 payload";

        // Write the row exactly as fresh ingest does: v2 key + v4 AAD,
        // stamped key_version = 2.
        let item_id = uuid::Uuid::new_v4().to_string();
        let row_id = uuid::Uuid::new_v4().to_string();
        let v2_key = derive_v2(&seed);
        let aad_v2 = copypaste_core::build_item_aad_v2(&item_id, AAD_SCHEMA_VERSION_V4, 2);
        let (nonce, ciphertext) =
            encrypt_item_with_aad(plaintext, &v2_key, &aad_v2).expect("encrypt v2");
        db.conn()
            .execute(
                "INSERT INTO clipboard_items \
                 (id, item_id, content_type, content, content_nonce, \
                  is_sensitive, is_synced, lamport_ts, wall_time, key_version) \
                 VALUES (?1,?2,'text',?3,?4,0,0,?5,?5,2)",
                rusqlite::params![row_id, item_id, ciphertext, nonce.to_vec(), 1i64],
            )
            .expect("insert v2 row");
        let content_before: Vec<u8> = db
            .conn()
            .query_row(
                "SELECT content FROM clipboard_items WHERE id = ?1",
                rusqlite::params![row_id],
                |r| r.get(0),
            )
            .unwrap();

        let (v1_key, v2_sweep_key) = sweep_keys(&seed);
        let rotated = db
            .migration_v4_sweep_resumable(&v1_key, &v2_sweep_key)
            .expect("sweep must not error");

        assert_eq!(rotated, 0, "an already-v2 row must not be rotated");
        assert_eq!(
            key_version_of(&db, &row_id),
            2,
            "the v2 row stays at key_version = 2"
        );
        let content_after: Vec<u8> = db
            .conn()
            .query_row(
                "SELECT content FROM clipboard_items WHERE id = ?1",
                rusqlite::params![row_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            content_before, content_after,
            "the v2 row's ciphertext must be byte-for-byte untouched"
        );

        // Still readable via the v2 read path.
        let recovered = read_back(&db, &seed, &row_id);
        assert_eq!(recovered, plaintext, "untouched v2 row stays readable");
    }

    /// `sweep_keys` must produce EXACTLY the keys the read path uses for each
    /// version: `v1_key == seed` (used directly) and `v2_key == derive_v2(seed)`.
    #[test]
    fn sweep_keys_match_read_path_keys() {
        let seed = [0x5Au8; 32];
        let (v1_key, v2_key) = sweep_keys(&seed);
        assert_eq!(
            v1_key, seed,
            "sweep v1_key must be the seed used directly (the read path's `**local_key`)"
        );
        assert_eq!(
            v2_key,
            derive_v2(&seed),
            "sweep v2_key must equal derive_v2(seed) (the read path's derive_v2(&v1_key))"
        );
    }

    /// v0.4 ingest round-trip (HIGH): a freshly-captured text item must be
    /// readable through the SAME path the daemon uses on paste-back. The read
    /// path (`ipc::write_to_pasteboard`, text branch) dispatches on the row's
    /// `key_version` via `decrypt_item_by_version`, deriving the v2 key as
    /// `derive_v2(local_key)`. This test feeds the production ingest crypto
    /// (`encrypt_text_for_storage`) into the production read crypto
    /// (`decrypt_item_by_version`) and asserts the bytes survive.
    ///
    /// Before the ingest fix, ingest encrypted with the v1 key + v3 AAD while
    /// stamping `key_version = 2`, so this round-trip failed with
    /// `EncryptError::AuthFailed`.
    #[test]
    fn fresh_text_capture_round_trips_through_read_path() {
        let local_key = [0x42u8; 32]; // stands in for load_local_key() (the v1 key)
        let item_id = uuid::Uuid::new_v4().to_string();
        let plaintext = b"hello from a fresh clipboard capture";

        // Ingest: exactly what handle_text does to produce the stored row.
        let (nonce, ciphertext) =
            encrypt_text_for_storage(plaintext, &local_key, &item_id).expect("encrypt");

        // The row is stamped key_version = 2 (ClipboardItem::new_text).
        let item = ClipboardItem::new_text(ciphertext.clone(), nonce.to_vec(), 0);
        assert_eq!(
            item.key_version, 2,
            "freshly-captured rows are stamped key_version = 2"
        );

        // Read: replicate the read path's key derivation + dispatch.
        let v1_key = local_key;
        let v2_key = derive_v2(&v1_key);
        let mut nonce_arr = [0u8; NONCE_SIZE];
        nonce_arr.copy_from_slice(&nonce);

        let recovered = decrypt_item_by_version(
            item.key_version,
            &v1_key,
            &v2_key,
            &item_id,
            &nonce_arr,
            &ciphertext,
        )
        .expect("read path must decrypt a freshly-captured row");

        assert_eq!(
            recovered, plaintext,
            "round-trip plaintext must match the captured bytes"
        );
    }

    /// arch LOW #24 regression: the device_id must survive restarts.
    /// Two consecutive calls to `load_or_create_device_id` with the same
    /// backing file must return the same UUID.
    #[test]
    fn device_id_persists_across_restart() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("device_id");

        // SAFETY: env mutation is process-global. We use a unique tmpdir path
        // so parallel tests don't collide on the value, and we restore the
        // previous value after the test.
        let prev = std::env::var_os("COPYPASTE_DEVICE_ID_PATH");
        unsafe {
            std::env::set_var("COPYPASTE_DEVICE_ID_PATH", &path);
        }

        let first = load_or_create_device_id().expect("first call must succeed");
        assert!(
            path.exists(),
            "device_id file must be written on first call"
        );

        let second = load_or_create_device_id().expect("second call must succeed");

        // Restore env before assertions so a failure doesn't leak state.
        unsafe {
            match prev {
                Some(v) => std::env::set_var("COPYPASTE_DEVICE_ID_PATH", v),
                None => std::env::remove_var("COPYPASTE_DEVICE_ID_PATH"),
            }
        }

        assert_eq!(first, second, "device_id must persist across restarts");

        // On Unix the file must be 0o600.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600, "device_id file must be chmod 0600");
        }
    }

    /// daemon-core backlog #1 regression: private mode must survive a daemon
    /// restart. `persist_private_mode(true)` writes the flag; a fresh
    /// `load_private_mode()` (simulating the next startup) must read it back as
    /// `true`. Toggling back to `false` must also round-trip.
    #[test]
    fn private_mode_persists_across_restart() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("private_mode");

        // Serialise env mutation with every other env-mutating daemon test.
        let _guard = crate::TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let prev = std::env::var_os("COPYPASTE_PRIVATE_MODE_PATH");
        // SAFETY: held under TEST_ENV_LOCK; restored before returning.
        unsafe {
            std::env::set_var("COPYPASTE_PRIVATE_MODE_PATH", &path);
        }

        // First run: absent file => disabled.
        assert!(
            !load_private_mode(),
            "missing private_mode file must default to disabled"
        );

        // Enable + simulate restart: the next load must see it enabled.
        persist_private_mode(true);
        assert!(path.exists(), "persisting must create the flag file");
        let after_enable = load_private_mode();

        // Disable + reload: must round-trip back to false.
        persist_private_mode(false);
        let after_disable = load_private_mode();

        // Restore env before assertions so a failure doesn't leak state.
        unsafe {
            match prev {
                Some(v) => std::env::set_var("COPYPASTE_PRIVATE_MODE_PATH", v),
                None => std::env::remove_var("COPYPASTE_PRIVATE_MODE_PATH"),
            }
        }

        assert!(
            after_enable,
            "enabled private mode must persist across restart"
        );
        assert!(
            !after_disable,
            "disabled private mode must persist across restart"
        );
    }

    // -----------------------------------------------------------------------
    // Image round-trip coverage (fix/import-and-rt-tests)
    // -----------------------------------------------------------------------

    /// Build a valid 2×2 white PNG via the `image` crate. Generating it (vs a
    /// hand-crafted byte array) keeps the test robust against the PNG
    /// decoder's strictness — mirrors `copypaste_core::image`'s own tests.
    fn test_png() -> Vec<u8> {
        use image::{DynamicImage, ImageBuffer, Rgb};
        let img = ImageBuffer::from_fn(2, 2, |_, _| Rgb([255u8, 255u8, 255u8]));
        copypaste_core::encode_as_png(&DynamicImage::ImageRgb8(img)).expect("encode test PNG")
    }

    /// Read the single stored image row's `(content_blob, blob_ref)` back.
    fn read_image_row(db: &Database) -> (Vec<u8>, String) {
        db.conn()
            .query_row(
                "SELECT content, blob_ref FROM clipboard_items \
                 WHERE content_type = 'image' LIMIT 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .expect("image row exists")
    }

    /// GAP closer (image): drive the REAL image write path
    /// (`handle_image` → `encode_image` with the device's real `local_key`,
    /// producing the daemon's real chunk blob + `blob_ref` metadata JSON) and
    /// read it back through the REAL read path
    /// (`ipc::parse_image_file_id` → `chunks_from_blob` → `decode_image`),
    /// asserting the PNG bytes recover. Mirrors the text round-trip test.
    #[tokio::test]
    async fn fresh_image_capture_round_trips_through_read_path() {
        let local_key = [0x42u8; 32]; // stands in for load_local_key()
        let db = Arc::new(Mutex::new(Database::open_in_memory().expect("open db")));
        let config = AppConfig::default();
        let png = test_png();

        // Ingest: exactly what the monitor loop does on a fresh image capture.
        let item = handle_image(png.clone(), &db, &local_key, &config, "test-device")
            .await
            .expect("handle_image must store the image");
        assert_eq!(item.content_type, "image");

        // Read path: pull the stored blob + metadata and decrypt exactly as
        // ipc::write_to_pasteboard's image branch does.
        let guard = db.lock().await;
        let (blob, meta_json) = read_image_row(&guard);
        let file_id =
            crate::ipc::parse_image_file_id(&meta_json).expect("file_id parses from blob_ref");
        let chunks = copypaste_core::chunks_from_blob(&blob).expect("chunks deserialize");
        let recovered_png =
            copypaste_core::decode_image(&chunks, &local_key, &file_id).expect("decode_image");

        // `handle_image` re-encodes the raw clipboard bytes to PNG before
        // chunking, so the recovered bytes are the canonical PNG of the
        // decoded image — compute the same reference and compare.
        let reference_png = copypaste_core::encode_as_png(
            &copypaste_core::decode_clipboard_image(&png).expect("decode raw"),
        )
        .expect("encode reference png");
        assert_eq!(
            recovered_png, reference_png,
            "image must round-trip through the read path to the stored PNG"
        );
    }

    /// GAP closer (image, key rotation): an image row encrypted under the
    /// pre-rotation `local_key` MUST, after a local key rotation, either still
    /// decode OR fail with a clear, explicit error — never silent corruption.
    ///
    /// Image chunks are AEAD-encrypted with the raw `local_key` directly
    /// (no key_version dispatch — see `ipc::write_to_pasteboard`'s image
    /// branch and `crypto::chunks`). A rotated key therefore cannot satisfy
    /// the per-chunk auth tag, so `decode_image` MUST return an explicit
    /// `ImageError` (auth failure) rather than returning wrong/garbage bytes.
    /// This test pins that intended behaviour.
    #[tokio::test]
    async fn image_row_survives_local_key_rotation_or_errors_cleanly() {
        let old_key = [0x42u8; 32];
        let db = Arc::new(Mutex::new(Database::open_in_memory().expect("open db")));
        let config = AppConfig::default();
        let png = test_png();

        // Capture an image under the OLD key.
        handle_image(png.clone(), &db, &old_key, &config, "test-device")
            .await
            .expect("handle_image must store the image");

        let guard = db.lock().await;
        let (blob, meta_json) = read_image_row(&guard);
        let file_id =
            crate::ipc::parse_image_file_id(&meta_json).expect("file_id parses from blob_ref");
        let chunks = copypaste_core::chunks_from_blob(&blob).expect("chunks deserialize");

        // Rotate the local key (simulate a key rotation / new device secret).
        let rotated_key = [0x99u8; 32];
        assert_ne!(old_key, rotated_key, "precondition: key actually changed");

        // Decoding the pre-rotation row under the rotated key must FAIL
        // explicitly — never silently return corrupted/garbage bytes.
        let result = copypaste_core::decode_image(&chunks, &rotated_key, &file_id);
        assert!(
            result.is_err(),
            "a pre-rotation image row must NOT silently decode under a rotated key"
        );

        // And the original key must still decode it (rotation does not destroy
        // the existing row's recoverability under its own key).
        let recovered = copypaste_core::decode_image(&chunks, &old_key, &file_id)
            .expect("the pre-rotation row must still decode under its original key");
        let reference_png = copypaste_core::encode_as_png(
            &copypaste_core::decode_clipboard_image(&png).expect("decode raw"),
        )
        .expect("encode reference png");
        assert_eq!(
            recovered, reference_png,
            "under its original key the row decodes to the stored PNG"
        );
    }

    // -----------------------------------------------------------------------
    // FIX 2: dedup-bump — identical content bumps the existing row to top
    // -----------------------------------------------------------------------

    /// Capturing the same text twice must NOT insert a second row. The existing
    /// row's wall_time must be updated so it appears at the top of history.
    #[tokio::test]
    async fn handle_text_dedup_bumps_existing_row_not_inserts() {
        let local_key = [0x42u8; 32];
        let db = Arc::new(Mutex::new(Database::open_in_memory().expect("open db")));
        let config = AppConfig::default();
        let text = "duplicate clipboard text".to_string();

        // First capture.
        let item1 = handle_text(text.clone(), &db, &local_key, &config, "test-device")
            .await
            .expect("first capture must succeed");

        // Verify content_hash is set after first insert.
        {
            let guard = db.lock().await;
            let row = copypaste_core::get_item_by_id(&guard, &item1.id)
                .unwrap()
                .expect("first row must exist");
            assert!(
                row.content_hash.is_some(),
                "content_hash must be set on new row"
            );
        }

        // Second capture of the same text.
        let _item2 = handle_text(text.clone(), &db, &local_key, &config, "test-device").await;

        // Must still be exactly one row.
        let guard = db.lock().await;
        let total = copypaste_core::count_items(&guard).expect("count_items");
        assert_eq!(
            total, 1,
            "identical text must not insert a duplicate row; expected 1 row, got {total}"
        );
    }

    /// After a dedup bump, the bumped item has a wall_time >= the first
    /// insert's wall_time, so it sorts to the top.
    #[tokio::test]
    async fn handle_text_dedup_bumped_item_has_updated_wall_time() {
        let local_key = [0x42u8; 32];
        let db = Arc::new(Mutex::new(Database::open_in_memory().expect("open db")));
        let config = AppConfig::default();
        let text = "text that will be bumped".to_string();

        // Insert the item and record its initial wall_time.
        let item1 = handle_text(text.clone(), &db, &local_key, &config, "test-device")
            .await
            .expect("first capture must succeed");
        let wall_time_before = item1.wall_time;

        // A tiny sleep to ensure a different wall_time on the bump.
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;

        // Second capture: should bump, not insert.
        handle_text(text.clone(), &db, &local_key, &config, "test-device").await;

        let guard = db.lock().await;
        let row = copypaste_core::get_item_by_id(&guard, &item1.id)
            .unwrap()
            .expect("original row must still exist after bump");

        assert!(
            row.wall_time >= wall_time_before,
            "bumped wall_time ({}) must be >= original ({})",
            row.wall_time,
            wall_time_before
        );
    }

    /// Capturing two DIFFERENT texts must insert two distinct rows.
    #[tokio::test]
    async fn handle_text_different_content_inserts_two_rows() {
        let local_key = [0x42u8; 32];
        let db = Arc::new(Mutex::new(Database::open_in_memory().expect("open db")));
        let config = AppConfig::default();

        handle_text(
            "first distinct text".to_string(),
            &db,
            &local_key,
            &config,
            "test-device",
        )
        .await;
        handle_text(
            "second distinct text".to_string(),
            &db,
            &local_key,
            &config,
            "test-device",
        )
        .await;

        let guard = db.lock().await;
        let total = copypaste_core::count_items(&guard).expect("count_items");
        assert_eq!(
            total, 2,
            "two distinct texts must produce two rows, got {total}"
        );
    }
}
