#[cfg(unix)]
use crate::ipc::IpcServer;
use crate::{
    clipboard::{ClipboardContent, ClipboardMonitor},
    p2p, paths,
};
use copypaste_core::{
    build_item_aad_v2, chunks_to_blob, derive_v2, detect, encode_image, encrypt_item_with_aad,
    insert_item_with_fts, AppConfig, ClipboardItem, Database, DeviceKeypair, AAD_SCHEMA_VERSION_V4,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
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
    #[cfg(target_os = "macos")]
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

    let local_key_arc: Arc<zeroize::Zeroizing<[u8; 32]>> = Arc::new(load_local_key());
    tracing::info!("local encryption key ready");

    let db_path = paths::db_path();
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
                // Fix B: fail with an actionable error instead of a bare bail.
                // The most common opaque failure here is SQLCipher's
                // "file is not a database" (SQLITE_NOTADB), which on an
                // *encrypted* file means the device key did not match — e.g.
                // the macOS Keychain returned a different key than the one the
                // DB was created under (re-keyed device, restored Keychain, or
                // a failed ThisDeviceOnly accessibility migration). Surface the
                // path + likely cause so the failure is diagnosable from the
                // daemon log alone rather than a cryptic one-liner.
                tracing::error!(
                    db_path = %db_path.display(),
                    error = %e,
                    "failed to open clipboard database — if this reports \
                     'file is not a database', the SQLCipher key from the \
                     Keychain does not match the key the DB was encrypted with \
                     (re-keyed device, restored/!=device Keychain entry, or a \
                     missing keychain entitlement). The daemon cannot continue \
                     without the correct key; the encrypted data is intact on \
                     disk and recoverable once the matching key is restored."
                );
                return Err(anyhow::anyhow!(
                    "Database open failed at {}: {e}",
                    db_path.display()
                ));
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
    // On non-macOS we don't have a keychain-backed keypair; use a zero
    // placeholder. Memory: Windows/Linux are cfg-frozen (macOS+Android only).
    #[cfg(target_os = "macos")]
    let device_public_key_arc: Arc<[u8; 32]> = {
        let kp = crate::keychain::load_or_create()
            .map_err(|e| anyhow::anyhow!("keychain load_or_create: {e}"))?;
        Arc::new(kp.public_key_bytes())
    };
    #[cfg(not(target_os = "macos"))]
    let device_public_key_arc: Arc<[u8; 32]> = Arc::new([0u8; 32]);

    // D1: create the process-wide cancellation token. Clones are passed to
    // every long-running task; calling `shutdown_token.cancel()` on SIGINT/
    // SIGTERM propagates to all of them simultaneously.
    let shutdown_token = CancellationToken::new();

    // Shared private-mode flag: when true, the clipboard monitor skips recording.
    // This is set/cleared via the IPC `set_private_mode` command.
    let private_mode = Arc::new(AtomicBool::new(false));

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

    #[cfg(unix)]
    let socket_path = paths::socket_path();
    // D2 (IPC): pass a token clone so the accept loop exits on shutdown.
    #[cfg(unix)]
    let _ipc_handle = {
        let ipc_db = db.clone();
        let ipc_private_mode = private_mode.clone();
        let ipc_local_key = local_key_arc.clone();
        let ipc_device_pub = device_public_key_arc.clone();
        let socket_clone = socket_path.clone();
        let ipc_shutdown = shutdown_token.clone();
        let ipc_peers = p2p_peers.clone();
        tokio::spawn(async move {
            let mut server =
                IpcServer::new(ipc_db, ipc_private_mode, ipc_local_key, ipc_device_pub);
            if let Some(peers) = ipc_peers {
                server = server.with_p2p_peers(peers);
            }
            if let Err(e) = server.serve(&socket_clone, ipc_shutdown).await {
                tracing::error!("IPC server error: {e}");
            }
        })
    };

    // Broadcast channel: carries newly-inserted clipboard items to any
    // subscriber (P2P sync, cloud-sync, future extensions).
    //
    // Capacity 256 (bumped from 64 — audit HIGH #8). The earlier 64-slot
    // buffer was too small for clipboard bursts (e.g. a rapid `pbcopy` loop
    // or a P2P peer momentarily backpressured by network jitter): subscribers
    // would receive `RecvError::Lagged` and silently drop items.
    //
    // Subscriber loops (p2p::subscriber_loop, cloud orchestrator, sync_orch)
    // still need to log `Lagged(n)` themselves — owned by the subsystems that
    // hold the receivers, not this file.
    let (new_item_tx, _new_item_rx) = broadcast::channel::<ClipboardItem>(256);

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
    let _p2p_handle: Option<p2p::P2pHandle> = if let Some(p2p_peers) = p2p_peers {
        // Persistent device_id: regenerating on every restart would break P2P
        // pairing and cloud peer recognition (arch LOW #24). Read from disk,
        // creating + writing a fresh UUID v4 on first run.
        let device_id = match load_or_create_device_id() {
            Ok(id) => id,
            Err(e) => {
                tracing::warn!("device_id load/create failed ({e}); using ephemeral UUID");
                uuid::Uuid::new_v4()
            }
        };
        let device_name = std::env::var("HOSTNAME")
            .or_else(|_| std::env::var("COMPUTERNAME"))
            .unwrap_or_else(|_| "CopyPaste".to_string());

        let p2p_config = p2p::P2pConfig {
            listen_port: 0,
            device_name,
            enabled: true,
        };

        // Hand the SAME live allowlist already shared with the IPC server
        // (fix/p2p-c-review #2). `start_p2p` seeds it from peers.json.
        match p2p::start_p2p(
            p2p_config,
            db.clone(),
            device_id,
            (*local_key_arc).clone(),
            p2p_peers,
            new_item_tx.subscribe(),
            sync_incoming_tx.clone(),
            sync_outbound_rx,
        )
        .await
        {
            Ok(handle) => {
                tracing::info!(port = handle.actual_port, "P2P subsystem running");
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
    // Persistent sync_device_id (audit HIGH #13).
    //
    // Previously this was `Uuid::new_v4().to_string()` on every startup,
    // which broke sync orchestrator correlation: peers saw a brand-new
    // device on every restart and could not deduplicate items by origin.
    //
    // We reuse the same on-disk identifier the P2P branch already loads
    // via `load_or_create_device_id` so the daemon presents a single
    // stable identity across the local-clipboard / P2P / sync surfaces.
    let sync_device_id = match load_or_create_device_id() {
        Ok(id) => id.to_string(),
        Err(e) => {
            tracing::warn!(
                "sync_device_id load/create failed ({e}); falling back to ephemeral UUID — \
                 sync orchestrator will treat this run as a new device"
            );
            uuid::Uuid::new_v4().to_string()
        }
    };
    let sync_db = db.clone();
    let sync_rx = new_item_tx.subscribe();
    // D2 (sync_orch): pass a token clone so the orchestrator exits on shutdown.
    let sync_shutdown = shutdown_token.clone();
    let sync_handle = tokio::spawn(async move {
        if let Err(e) = sync_orch::run(
            sync_db,
            sync_rx,
            sync_incoming_rx,
            sync_outbound_tx,
            sync_device_id,
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
            match start_cloud(cloud_cfg, db.clone(), rx).await {
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
                    handle_tick(&mut monitor, &db, &local_key_arc, &config, &private_mode, &new_item_tx).await;
                    cleanup_ticks += 1;
                    sensitive_cleanup_ticks += 1;

                    // Sensitive item TTL: run every 5 seconds.
                    // `5_000 / poll_interval_ms` is integer-divided; for any
                    // `poll_interval_ms > 5000` the quotient is 0, which would
                    // make this branch fire every tick. Clamp the threshold to
                    // at least 1 so the cleanup runs (at most) every tick.
                    if sensitive_cleanup_ticks >= (5_000 / config.poll_interval_ms.max(1)).max(1) {
                        sensitive_cleanup_ticks = 0;
                        let db_guard = db.lock().await;
                        // `unwrap_or_default()` matches the pattern at ipc.rs:799
                        // — clock skew (system clock moved backwards past UNIX
                        // epoch) must not panic the daemon.
                        let now_ms = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as i64;
                        match copypaste_core::delete_sensitive_expired(&db_guard, now_ms, sensitive_ttl_ms) {
                            Ok(n) if n > 0 => tracing::info!("sensitive TTL cleanup: wiped {n} sensitive items"),
                            Ok(_) => {}
                            Err(e) => tracing::warn!("sensitive TTL cleanup error: {e}"),
                        }
                    }

                    // General expires_at TTL: run every 60 seconds. Same
                    // integer-division clamp as above.
                    if cleanup_ticks >= (60_000 / config.poll_interval_ms.max(1)).max(1) {
                        cleanup_ticks = 0;
                        let db_guard = db.lock().await;
                        let now_ms = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as i64;
                        match copypaste_core::delete_expired(&db_guard, now_ms) {
                            Ok(n) if n > 0 => tracing::info!("TTL cleanup: removed {n} expired items"),
                            Ok(_) => {}
                            Err(e) => tracing::warn!("TTL cleanup error: {e}"),
                        }
                    }
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
                    handle_tick(&mut monitor, &db, &local_key_arc, &config, &private_mode, &new_item_tx).await;
                    cleanup_ticks += 1;
                    sensitive_cleanup_ticks += 1;

                    // Sensitive item TTL: run every 5 seconds.
                    if sensitive_cleanup_ticks >= (5_000 / config.poll_interval_ms.max(1)).max(1) {
                        sensitive_cleanup_ticks = 0;
                        let db_guard = db.lock().await;
                        let now_ms = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as i64;
                        match copypaste_core::delete_sensitive_expired(&db_guard, now_ms, sensitive_ttl_ms) {
                            Ok(n) if n > 0 => tracing::info!("sensitive TTL cleanup: wiped {n} sensitive items"),
                            Ok(_) => {}
                            Err(e) => tracing::warn!("sensitive TTL cleanup error: {e}"),
                        }
                    }

                    // General expires_at TTL: run every 60 seconds.
                    if cleanup_ticks >= (60_000 / config.poll_interval_ms.max(1)).max(1) {
                        cleanup_ticks = 0;
                        let db_guard = db.lock().await;
                        let now_ms = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as i64;
                        match copypaste_core::delete_expired(&db_guard, now_ms) {
                            Ok(n) if n > 0 => tracing::info!("TTL cleanup: removed {n} expired items"),
                            Ok(_) => {}
                            Err(e) => tracing::warn!("TTL cleanup error: {e}"),
                        }
                    }
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

#[tracing::instrument(skip_all, name = "clipboard_tick")]
async fn handle_tick(
    monitor: &mut ClipboardMonitor,
    db: &Arc<Mutex<Database>>,
    local_key: &[u8; 32],
    config: &AppConfig,
    private_mode: &Arc<AtomicBool>,
    new_item_tx: &broadcast::Sender<ClipboardItem>,
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
            if let Some(item) = handle_text(text, db, local_key, config).await {
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
            if let Some(item) = handle_image(raw_bytes, db, local_key, config).await {
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
) -> Option<ClipboardItem> {
    // Migration gate is now enforced at the Database layer inside
    // `insert_item` / `insert_item_with_fts` (ItemsError::MigrationInProgress).
    // The call-site guard that used to live here has been removed.

    let is_sensitive = detect(&text).is_some();

    let item_id = uuid::Uuid::new_v4().to_string();
    let (nonce, ciphertext) = match encrypt_text_for_storage(text.as_bytes(), local_key, &item_id) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!("encrypt_text_for_storage failed for text: {e}");
            return None;
        }
    };
    let mut item = ClipboardItem::new_text(ciphertext, nonce.to_vec(), 0);
    item.item_id = item_id;
    item.is_sensitive = is_sensitive;

    if is_sensitive {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;
        item.expires_at = Some(now_ms + (config.sensitive_ttl_local_secs as i64 * 1000));
    }

    let db_guard = db.lock().await;
    // v0.3 post-T2: insert_item + upsert_fts collapsed into a single
    // transaction. Closes the TOCTOU window where a crash between the row
    // insert and the FTS upsert could leave a row that search would never
    // find. Also handles the v5 UNIQUE-index dedup race internally — if
    // another writer wins the (content_hash, minute) race we get back the
    // existing row's id rather than a SQLITE_CONSTRAINT error.
    match insert_item_with_fts(&db_guard, &item, &text) {
        Ok(stored_id) => {
            // beta.5 Bug-1 visibility: promoted from debug! to info! so users
            // can verify in `daemon.out.log` that captured items reach the DB.
            if stored_id != item.id {
                tracing::debug!(
                    requested = %item.id,
                    existing = %stored_id,
                    "text item deduped against existing row"
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
            prune_history(&db_guard, config);
            Some(item)
        }
        Err(e) => {
            tracing::warn!("failed to store text item: {e}");
            None
        }
    }
}

async fn handle_image(
    raw_bytes: Vec<u8>,
    db: &Arc<Mutex<Database>>,
    local_key: &[u8; 32],
    config: &AppConfig,
) -> Option<ClipboardItem> {
    // Migration gate is now enforced at the Database layer inside
    // `insert_item` / `insert_item_with_fts` (ItemsError::MigrationInProgress).
    // The call-site guard that used to live here has been removed.

    // Derive a stable file_id from SHA-256(raw_bytes)[..16] — a 128-bit
    // collision-resistant content hash. This is deterministic so identical
    // images dedup naturally, and replaces the prior `DefaultHasher XOR
    // nanos` scheme (Wave 2.1 security LOW #19).
    let file_id = crate::clipboard::image_content_hash(&raw_bytes);

    match encode_image(&raw_bytes, local_key, &file_id) {
        Ok((meta, chunks)) => {
            let blob = chunks_to_blob(&chunks);
            let meta_json = format!(
                r#"{{"width":{},"height":{},"original_size":{},"chunk_count":{},"file_id":{:?}}}"#,
                meta.width, meta.height, meta.original_size, meta.chunk_count, meta.file_id
            );
            let item = ClipboardItem::new_image(blob, meta_json, 0);
            tracing::debug!(
                "image encoded: {}x{} px, {} chunks, original_size={}",
                meta.width,
                meta.height,
                meta.chunk_count,
                meta.original_size
            );

            let db_guard = db.lock().await;
            // Atomic insert: images have no searchable text, so we pass "" to
            // skip the FTS write (insert_item_with_fts treats empty as
            // "image item" and only writes the row).
            match insert_item_with_fts(&db_guard, &item, "") {
                Ok(stored_id) => {
                    // beta.5 Bug-1 visibility: promoted from debug! to info!.
                    if stored_id != item.id {
                        tracing::debug!(
                            requested = %item.id,
                            existing = %stored_id,
                            "image item deduped against existing row"
                        );
                    } else {
                        tracing::info!(id = %item.id, "stored image item id={}", item.id);
                    }
                    prune_history(&db_guard, config);
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
/// sweep, from the raw `seed` returned by [`load_local_key`].
///
/// **Critical:** `seed` is ALREADY the v1 storage key —
/// [`load_local_key`] returns `DeviceKeypair::local_enc_key()`, which is
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

#[tracing::instrument(name = "load_local_key")]
fn load_local_key() -> zeroize::Zeroizing<[u8; 32]> {
    #[cfg(target_os = "macos")]
    {
        match crate::keychain::load_or_create() {
            Ok(kp) => {
                tracing::info!("device fingerprint={}", kp.fingerprint());
                kp.local_enc_key()
            }
            Err(e) => {
                tracing::warn!("Keychain unavailable ({e}), using ephemeral key");
                DeviceKeypair::generate().local_enc_key()
            }
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        // Keychain not available on non-macOS; use an ephemeral key for CI/Linux builds.
        // On production macOS this branch is never compiled in.
        tracing::warn!("Non-macOS platform: using ephemeral encryption key (data not persisted across restarts)");
        DeviceKeypair::generate().local_enc_key()
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

#[cfg(test)]
mod tests {
    use super::*;
    use copypaste_core::{
        build_item_aad, decrypt_item_by_version, encrypt_item_with_aad, Database,
        AAD_SCHEMA_VERSION, NONCE_SIZE,
    };

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
        let item = handle_image(png.clone(), &db, &local_key, &config)
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
        handle_image(png.clone(), &db, &old_key, &config)
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
}
