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
// CopyPaste-9fb6: opt-in error reporting. `reporter` is constructed at startup
// with `ReportConsent::Disabled` (safe default — no data leaves the device)
// until a future settings surface exposes the consent toggle. Calling
// `report_and_log` at recoverable-error sites means the infrastructure is wired
// and consent-gating is correct, so enabling it later is a one-line change.
use copypaste_telemetry::{report_and_log, OsTag, ReportConsent, ReportableError};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

pub(crate) mod capture;
pub(crate) mod startup;

pub(crate) use capture::{handle_tick, run_ttl_cleanup, FrontmostAppCache};
pub(crate) use startup::{
    decide_db_startup, encrypted_db_exists, load_config, load_local_key_bounded,
    load_or_create_device_id, load_private_mode, persist_private_mode, run_degraded, sweep_keys,
    DbStartupPlan, KeyLoad,
};

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

/// How often the sensitive-item TTL cleanup runs (milliseconds). Used in both
/// the macOS and non-macOS monitor loops to avoid magic literals.
const SENSITIVE_CLEANUP_INTERVAL_MS: u64 = 5_000;

/// How often the general expires_at TTL cleanup runs (milliseconds).
const GENERAL_CLEANUP_INTERVAL_MS: u64 = 60_000;

/// How often the degraded-mode loop re-checks the quit flag (milliseconds).
/// 1 s is responsive enough for human perception while burning ~4× less CPU
/// than the old 250 ms value.
pub(super) const DEGRADED_QUIT_POLL_INTERVAL_MS: u64 = 1_000;

use std::sync::RwLock;
use tokio::sync::{broadcast, mpsc, Mutex};
use tokio::time::interval;
// D1: CancellationToken for coordinated graceful shutdown across all tasks.
use tokio_util::sync::CancellationToken;

// Beta W2.2 (arch-1): sync orchestrator that wires `copypaste-sync` into the
// daemon. Declared at crate root in `lib.rs` (`pub mod sync_orch;`); we
// re-import it here for the local `sync_orch::run` call below.
use crate::sync_orch;

/// Resolve a human-readable device name for P2P advertisements and QR pairing.
///
/// On macOS uses `scutil --get ComputerName` (the user-visible name, e.g.
/// "Dmytro's MacBook Air") as the primary source, which is the same path used
/// by `DeviceMeta::collect()` for the QR-pairing PeerMeta.  Falls back to the
/// `HOSTNAME` env var (Unix), then `COMPUTERNAME` (Windows), then the `hostname`
/// binary, and finally the literal `"CopyPaste"` only when all else fails.
///
/// This is the single source of truth for the P2P mDNS registration, the QR
/// payload `device_name` field, and the relay identity — every advertising path
/// calls this function so all peers see the same consistent name.
pub(crate) fn resolve_device_name() -> String {
    crate::device_meta::collect_device_name().unwrap_or_else(|| "CopyPaste".to_string())
}

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
    // CopyPaste-9fb6: initialise the telemetry reporter.  The default is
    // `Disabled` — no events leave the device until the user opts in via the
    // settings UI (a future PR).  The reporter is passed into error-handling
    // sites below so wiring is complete and adding consent is a one-line
    // change.  When `COPYPASTE_SENTRY_DSN` is not set (OSS builds, CI) this
    // always returns a `NoopReporter`.
    let reporter: Box<dyn copypaste_telemetry::ErrorReporter> =
        match option_env!("COPYPASTE_SENTRY_DSN") {
            Some(dsn) => copypaste_telemetry::init_with_dsn(ReportConsent::Disabled, dsn)
                .unwrap_or_else(|e| {
                    tracing::warn!(error = %e, "telemetry init failed; using no-op reporter");
                    copypaste_telemetry::init(ReportConsent::Disabled)
                }),
            None => copypaste_telemetry::init(ReportConsent::Disabled),
        };

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
            // One-time repair: image/file rows that were encrypted with the v1
            // key but mistakenly stamped key_version=2 by the pre-fix writer.
            // Idempotent: repaired rows fail the v1-decrypt probe on subsequent
            // runs and are silently skipped.
            let repaired = guard.repair_mislabeled_kv2_blob_rows(&v1_key, &v2_key)?;
            // After the sweep, surface any rows that stayed at key_version=1 —
            // these are permanently undecryptable legacy ciphertexts (auth-tag
            // mismatch) and are dead weight. Purge only if explicitly opted in.
            let dead = guard.count_dead_v1_rows()?;
            let purged = if dead > 0 && purge_dead {
                guard.purge_dead_v1_rows()?
            } else {
                0
            };
            Ok::<(usize, usize, usize, usize), copypaste_core::DbError>((
                rotated, repaired, dead, purged,
            ))
        })
        .await
        {
            Ok(Ok((rotated, repaired, dead, purged))) => {
                tracing::info!(rotated, "v4 key-version migration sweep complete");
                if repaired > 0 {
                    tracing::info!(
                        repaired,
                        "v4 migration: repaired {repaired} mislabeled kv2 blob row(s) \
                         (were encrypted with v1 key but stamped key_version=2)"
                    );
                }
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
                // CopyPaste-9fb6: report this recoverable failure to the
                // telemetry backend (no-op while consent is Disabled).
                report_and_log(
                    &*reporter,
                    ReportableError::new(
                        env!("CARGO_PKG_NAME"),
                        env!("CARGO_PKG_VERSION"),
                        "db.v4_migration_sweep_failed",
                        OsTag::current(),
                    ),
                );
            }
            Err(join_err) => {
                tracing::warn!(error = %join_err, "v4 migration sweep task panicked");
            }
        }
    }

    // One-time startup sweep: delete poison rows created before the
    // inbound-merge guard was added (CopyPaste-jww / CopyPaste-5y4).
    // A poison row is a text item where content IS NOT NULL AND
    // content_nonce IS NULL, or a file/image item where content IS NOT
    // NULL AND content_nonce IS NULL AND blob_ref IS NULL. The peer
    // re-sends these items on the next catch-up cycle. Idempotent.
    {
        let sweep_db = db.clone();
        match tokio::task::spawn_blocking(move || {
            let guard = sweep_db.blocking_lock();
            crate::sync_orch::sweep_poison_rows(&guard)
        })
        .await
        {
            Ok(Ok(swept)) => {
                if swept > 0 {
                    tracing::warn!(
                        swept,
                        "startup: swept {swept} poison row(s) created before \
                         the inbound-merge guard (CopyPaste-jww/5y4) — \
                         peers will re-send them on next connect"
                    );
                } else {
                    tracing::debug!("startup: no poison rows found (clean)");
                }
            }
            Ok(Err(e)) => {
                tracing::warn!(error = %e, "startup: poison row sweep failed (non-fatal)");
            }
            Err(join_err) => {
                tracing::warn!(error = %join_err, "startup: poison row sweep task panicked");
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
    //
    // A-SET-4: env var is the override (the app always spawns with COPYPASTE_P2P=1
    // when it wants P2P enabled); when env var is absent, fall back to the
    // persisted IPC config's p2p_enabled so the user's UI toggle takes effect.
    // The IPC AppConfig (config.json) owns p2p_enabled; the core AppConfig
    // (config.toml) owns limits. Read the IPC config here just for this field.
    let p2p_enabled = match std::env::var("COPYPASTE_P2P").as_deref() {
        Ok("1") => true,
        Ok("0") => false,
        // Item 6: single source of truth — delegate to the public accessor so
        // daemon.rs and any future caller always agree on the read path.
        _ => crate::ipc::p2p_enabled_from_config(),
    };
    // lan_visibility is persisted in config.toml (overlaid by update_core_config
    // on set_config). Read it here so start_p2p can gate mDNS at startup.
    let lan_visibility_at_start = {
        let core =
            copypaste_core::AppConfig::load(&crate::paths::config_path()).unwrap_or_default();
        core.lan_visibility
    };
    let p2p_peers: Option<copypaste_p2p::transport::PairedPeers> = if p2p_enabled {
        Some(copypaste_p2p::transport::PairedPeers::new())
    } else {
        None
    };

    // LAN/SAS Phase 0: construct ONE DiscoveryService here, before both the IPC
    // server and `start_p2p`, so both share the same instance.  The IPC
    // `list_discovered` handler reads peers from this Arc; `start_p2p` calls
    // `register()` + `start()` on it.  `None` when P2P is disabled — discovery
    // makes no sense without the P2P stack.
    let p2p_discovery: Option<std::sync::Arc<copypaste_p2p::discovery::DiscoveryService>> =
        if p2p_enabled {
            Some(std::sync::Arc::new(
                copypaste_p2p::discovery::DiscoveryService::new(),
            ))
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
        // P2P-DURABILITY: persist the mTLS identity so the fingerprint peers
        // pin at pairing time is STABLE across daemon restarts. Generating a
        // fresh cert on every launch (the previous behaviour) silently
        // invalidated every existing pairing on restart, so P2P sync never
        // survived a daemon restart. `load_or_create` reloads the same cert
        // from `p2p_identity.json` when it exists, generating + persisting one
        // only on first run.
        //
        // EXCEPTION: tests/dev set COPYPASTE_EPHEMERAL_KEY=1 to keep each
        // instance isolated (no shared on-disk identity), so honour that by
        // falling back to an ephemeral `generate()`.
        let ephemeral = std::env::var("COPYPASTE_EPHEMERAL_KEY").as_deref() == Ok("1");
        let result = if ephemeral {
            copypaste_p2p::cert::SelfSignedCert::generate(&local_device_id)
                .map_err(|e| std::io::Error::other(format!("cert generate: {e}")))
        } else {
            copypaste_p2p::cert::SelfSignedCert::load_or_create(
                &paths::p2p_identity_path(),
                &local_device_id,
            )
        };
        match result {
            Ok(cert) => Some(cert),
            Err(e) => {
                tracing::warn!(
                    "mTLS cert load/generate failed ({e}); pairing disabled this session"
                );
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
    // server is constructed.  `DeviceMeta::collect` spawns child processes
    // (`scutil`, `sysctl`, `sw_vers`) that together can take up to ~6 s on a
    // cold macOS system.  By running them here on a blocking thread we pay the
    // cost exactly once; all subsequent calls to `collect_own_peer_meta` and
    // `get_own_device_info` are instant cache reads (OnceLock — wait-free after
    // the first write).
    match tokio::task::spawn_blocking(|| crate::device_meta::warm_cache(env!("CARGO_PKG_VERSION")))
        .await
    {
        Ok(()) => tracing::debug!("device_meta: startup cache warmed"),
        Err(e) => tracing::warn!(
            error = %e,
            "device_meta: startup cache warm task panicked — \
             per-request collection will be used as fallback"
        ),
    }

    // P2 (ugv7) — startup TTL purge: run the same TTL cleanup the tick loop
    // performs, ONCE, right after the database is opened and BEFORE the IPC
    // socket is bound.  Without this, expired sensitive items remain readable
    // and searchable during the sub-second window between DB open and the first
    // tick.  We reuse `run_ttl_cleanup` exactly as the tick loop does, passing
    // `do_sensitive = sensitive_ttl_ms.is_some()` and `do_general = true`.
    {
        let startup_sensitive_ttl_ms = if config.sensitive_ttl_secs == 0 {
            None
        } else {
            Some(config.sensitive_ttl_secs as i64 * 1000)
        };
        run_ttl_cleanup(
            &db,
            startup_sensitive_ttl_ms.unwrap_or(0),
            startup_sensitive_ttl_ms.is_some(), // do_sensitive
            true,                               // do_general: always purge expired items at startup
        )
        .await;
        tracing::debug!("startup TTL purge complete (before IPC socket bind)");
    }

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

    // Start the P2P subsystem when p2p_enabled is true (resolved above via
    // A-SET-4: COPYPASTE_P2P env override, falling back to persisted config via
    // `ipc::p2p_enabled_from_config()`).
    // The live allowlist, cert, and shared DiscoveryService must all be present:
    // the cert is the identity the transport presents and that pairing advertises,
    // and the DiscoveryService must be the SAME Arc handed to the IPC server so
    // list_discovered sees live peers (LAN/SAS Phase 0, CRITICAL-1).
    let _p2p_handle: Option<p2p::P2pHandle> = if let (
        Some(p2p_peers),
        Some(p2p_cert),
        Some(p2p_disc),
    ) = (p2p_peers, p2p_cert, p2p_discovery)
    {
        // Reuse the persistent device_id loaded above (load_or_create_device_id
        // was called once already; parsing it back to Uuid is cheap).
        let device_id =
            uuid::Uuid::parse_str(&local_device_id).unwrap_or_else(|_| uuid::Uuid::new_v4());
        let device_name = resolve_device_name();

        let p2p_config = p2p::P2pConfig {
            listen_port: 0,
            device_name,
            enabled: true,
            lan_visibility: lan_visibility_at_start,
        };

        // P2P Phase 3 (sync-on-connect catch-up): build a provider that
        // replays the current local history — already re-keyed under the
        // shared sync key, exactly like normal outbound — into each peer the
        // instant a link is established. Without this, an item produced
        // before the link came up is never delivered (fanout is
        // fire-and-forget to currently-connected sinks). Uses the same
        // `SyncCrypto` construction as the orchestrator below.
        // CopyPaste-716: the closure now takes the connecting peer's
        // fingerprint so `catchup_items` uses that peer's specific pairwise
        // sync key rather than the first cached key for all peers.
        let catchup: p2p::CatchupProvider = {
            let catchup_db = db.clone();
            let catchup_device_id = local_device_id.clone();
            let catchup_seed: [u8; 32] = **local_key_arc;
            Arc::new(move |peer_fingerprint: &str| {
                let crypto =
                    sync_orch::SyncCrypto::new(catchup_seed, crate::ipc::peers_file_path());
                // The closure is `Fn` (sync) but the DB sits behind a tokio
                // Mutex; `block_in_place` + `blocking_lock` safely acquires
                // it on the multi-thread runtime without blocking the worker.
                //
                // Fix B (P2P image perf): split the catch-up into two phases so
                // the DB lock is held ONLY for the sequential read, not for the
                // CPU-heavy per-image re-key (chunk-decrypt + shared-key
                // re-encrypt).  On reconnect with a large history this previously
                // blocked the tokio executor for hundreds of milliseconds while
                // holding the mutex across every image's AEAD work.
                //   Phase 1: acquire lock, read all raw pages, release lock.
                //   Phase 2: re-key items (CPU, no DB lock).
                let fp = peer_fingerprint.to_owned();

                // Pre-flight: if the connecting peer has no sync key nothing is
                // decryptable, so skip both phases entirely (fast path).
                if crypto.sync_key_for_peer(&fp).is_none() {
                    return Vec::new();
                }

                // Phase 1: read raw items (DB lock held only here).
                let raw = tokio::task::block_in_place(|| {
                    let db = catchup_db.blocking_lock();
                    sync_orch::catchup_read_raw(&db, &catchup_device_id)
                });

                // Phase 2: re-key outside the DB lock (CPU work).
                sync_orch::rekey_catchup_items(raw, &crypto, &fp)
            })
        };

        // Hand the SAME live allowlist already shared with the IPC server
        // (fix/p2p-c-review #2), the SAME cert whose fingerprint the IPC
        // pairing handlers advertise (CRITICAL-1), and the SAME
        // DiscoveryService handed to the IPC server so list_discovered sees
        // live peers (LAN/SAS Phase 0). `start_p2p` seeds the allowlist
        // from peers.json.
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
            p2p_disc,
            // LAN/SAS Phase 2: the SAME pairing coordinator the IPC server
            // exposes, and the SAME sync-addr slot, so the standing
            // discovery-pairing responder routes its SAS through the shared
            // state machine and advertises a routable sync address in-band.
            std::sync::Arc::clone(&pairing_coordinator),
            std::sync::Arc::clone(&p2p_sync_addr_slot),
            // B1: the SAME public-IP cache the IPC server reads and the STUN
            // refresh task writes, so the standing LAN/SAS responder advertises
            // our own global IP in-band exactly like the IPC pairing paths.
            std::sync::Arc::clone(&public_ip_cache),
            // CopyPaste-1w7 (H8 fix): share a SyncCrypto clone with the
            // standing responder so it can call reload_sync_key after a
            // successful button-pair.  All clones share the same
            // Arc<Mutex<…>> backing store built above.
            sync_crypto.clone(),
            // CopyPaste-7ub: the shared live core config so the P2P outbound
            // fanout honours sync_on_wifi_only (same Arc the IPC server hot-reloads).
            core_config_arc.clone(),
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
                {
                    // Populate the single shared slot used for both online-status
                    // (list_peers) and mutual-unpair signalling (unpair/revoke).
                    // live_sinks and peer_sinks on P2pHandle are Arc clones of the
                    // same underlying map; we write live_sinks here.
                    let mut slot = live_sinks_slot
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    *slot = Some(std::sync::Arc::clone(&handle.live_sinks));
                }
                {
                    // Populate the RTT slot so list_peers can include latency_ms.
                    let mut slot = live_rtt_ms_slot
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    *slot = Some(std::sync::Arc::clone(&handle.peer_rtt_ms));
                }
                {
                    // Populate the P2P shutdown token slot so rescan_discovered
                    // can cancel the mDNS browse task on P2P shutdown
                    // (CopyPaste-fbxj). Mirrors the live_sinks_slot pattern.
                    let mut slot = p2p_shutdown_token_slot
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    *slot = Some(handle.shutdown_token.clone());
                }
                {
                    // Subscribe to peer connect/disconnect events and relay
                    // them into the IPC queue so `poll_peer_events` callers
                    // (e.g. the Tauri event bridge) can push live presence
                    // updates to the UI without waiting for the 10 s poll.
                    let mut event_rx = handle.peer_event_tx.subscribe();
                    let event_queue = std::sync::Arc::clone(&peer_event_queue);
                    let event_shutdown = handle.shutdown_token.clone();
                    tokio::spawn(async move {
                        loop {
                            tokio::select! {
                                biased;
                                _ = event_shutdown.cancelled() => { break; }
                                recv = event_rx.recv() => {
                                    match recv {
                                        Ok(ev) => {
                                            let record = match &ev {
                                                crate::p2p::PeerEvent::Connected { fingerprint } => {
                                                    crate::ipc::PeerEventRecord {
                                                        kind: "connected",
                                                        fingerprint: fingerprint.clone(),
                                                    }
                                                }
                                                crate::p2p::PeerEvent::Disconnected { fingerprint } => {
                                                    crate::ipc::PeerEventRecord {
                                                        kind: "disconnected",
                                                        fingerprint: fingerprint.clone(),
                                                    }
                                                }
                                            };
                                            let mut q = event_queue
                                                .lock()
                                                .unwrap_or_else(|p| p.into_inner());
                                            // Cap the queue to avoid unbounded growth
                                            // when no consumer is draining it.
                                            if q.len() >= crate::ipc::PEER_EVENT_QUEUE_CAP {
                                                q.pop_front();
                                            }
                                            q.push_back(record);
                                        }
                                        // Broadcast lagged: receiver fell behind.
                                        // The channel stays open — log and continue
                                        // so live-presence push survives event bursts
                                        // (network flaps etc.).
                                        Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                                            tracing::warn!(
                                                skipped = n,
                                                "P2P event bridge lagged; skipped {n} events"
                                            );
                                            continue;
                                        }
                                        // The sender dropped (P2P shutdown) — exit.
                                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                                    }
                                }
                            }
                        }
                    });
                }
                Some(handle)
            }
            Err(e) => {
                tracing::warn!("Failed to start P2P subsystem: {e}");
                // CopyPaste-9fb6: P2P startup failure is recoverable (daemon
                // continues without sync) but actionable for diagnostics.
                report_and_log(
                    &*reporter,
                    ReportableError::new(
                        env!("CARGO_PKG_NAME"),
                        env!("CARGO_PKG_VERSION"),
                        "p2p.startup_failed",
                        OsTag::current(),
                    ),
                );
                None
            }
        }
    } else {
        tracing::debug!(
            "P2P disabled (via COPYPASTE_P2P=0 or persisted p2p_enabled=false in config.json; \
                 set COPYPASTE_P2P=1 or toggle in Settings to enable)"
        );
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

    let sync_handle = tokio::spawn(async move {
        if let Err(e) = sync_orch::run(
            sync_db,
            sync_rx,
            sync_incoming_rx,
            sync_outbound_tx,
            sync_device_id,
            sync_crypto,
            sync_quota_bytes,
            sync_auto_apply,
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

    // Start optional cloud-sync if credentials are present.
    #[cfg(feature = "cloud-sync")]
    let _cloud_handle = {
        use crate::cloud::{start_cloud, CloudConfig};
        if !sync_enabled_at_start {
            tracing::info!("cloud-sync: sync_enabled=false — not starting cloud orchestrator");
            None
        } else if let Some(cloud_cfg) = CloudConfig::from_env() {
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
                core_config_arc.clone(),
                sync_in_flight.clone(),
            )
            .await
            {
                Ok(handle) => {
                    tracing::info!("cloud-sync: orchestrator started");
                    // CopyPaste-1jms.34: publish the canonical account id into
                    // the shared slot. The IpcServer's `get_sync_status` handler
                    // holds the same Arc and reads through it on every request,
                    // so this one write at startup is sufficient.
                    *cloud_account_id_slot
                        .lock()
                        .unwrap_or_else(|p| p.into_inner()) = handle.cloud_account_id.clone();
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

    // Start the relay-as-database sync path iff `relay_url` is configured.
    //
    // TOPOLOGY (dtq3): relay and Supabase are ADDITIVE, INDEPENDENT transports.
    // When both are configured this block and the cloud block above BOTH run; a
    // locally-captured item is broadcast to both.  A peer subscribed to both
    // transports may receive the same item_id twice, but `ingest_page_blocking`
    // (relay) and the cloud poll path both guard with `get_item_by_item_id` +
    // `remote_wins` before writing — the second delivery is a no-op (LWW keeps
    // exactly one row per item_id).  See `relay.rs` § "Multi-transport topology"
    // for the full contract.  No mutual-exclusion gate is needed here because
    // the consumer-side dedup is already enforced.
    //
    // All of an account's devices co-register one shared inbox (derived from the
    // sync key) and push to / poll it. The reqwest client is built once and
    // shared by both relay loops.
    #[cfg(feature = "relay-sync")]
    // CopyPaste-44rq.67: store the started handle in the shared slot (rather than
    // a bare local) so the IPC `set_config` handler can shut it down at runtime
    // when the user clears `relay_url`. The slot keeps the handle alive.
    {
        let relay_url = core_config_arc
            .read()
            .ok()
            .and_then(|c| c.relay_url.clone());
        let started = if !sync_enabled_at_start {
            tracing::info!("relay-sync: sync_enabled=false — not starting relay orchestrator");
            None
        } else if let Some(relay_url) = relay_url {
            tracing::info!("relay-sync: relay_url configured, starting relay orchestrator");
            // CopyPaste-16vr: the previous fallback was `reqwest::Client::new()`
            // which has no request timeout — a stalled relay endpoint would
            // block the sync loop forever. The builder can fail (e.g. when the
            // platform TLS stack is unavailable). The fallback now also applies
            // the timeout: a second builder call with identical settings is
            // tried; if that also fails, SYNC_HTTP_TIMEOUT is applied via
            // `tokio::time::timeout` at the call sites in relay.rs (which
            // already wrap each request). Using `Client::new()` without a
            // timeout is no longer an option.
            let client = reqwest::Client::builder()
                .timeout(crate::sync_common::SYNC_HTTP_TIMEOUT)
                .build()
                .unwrap_or_else(|_| {
                    // Re-attempt: building with only `.timeout()` set cannot
                    // fail on any supported platform (the error path only triggers
                    // for native-TLS config failures, not bare timeouts).
                    // `expect` is justified: if even this minimal builder fails
                    // there is a fundamental platform issue and daemon startup
                    // should abort rather than run without timeouts.
                    reqwest::Client::builder()
                        .timeout(crate::sync_common::SYNC_HTTP_TIMEOUT)
                        .build()
                        .expect(
                            "reqwest Client::builder().timeout().build() \
                             must succeed — platform TLS unavailable",
                        )
                });
            // CopyPaste-7ub: wire the self-write sentinel so relay auto-apply
            // does not re-capture its own pasteboard writes.  On Unix the
            // sentinel is shared with the ClipboardMonitor and the IPC
            // copy_item handler; on non-Unix there is no NSPasteboard so
            // the sentinel is disabled.
            #[cfg(unix)]
            let relay_auto_apply_cc: Option<Arc<std::sync::atomic::AtomicI64>> =
                Some(self_write_change_count_arc.clone());
            #[cfg(not(unix))]
            let relay_auto_apply_cc: Option<Arc<std::sync::atomic::AtomicI64>> = None;

            match crate::relay::start_relay(
                client,
                relay_url,
                resolve_device_name(),
                local_device_id.clone(),
                db.clone(),
                new_item_tx.subscribe(),
                cloud_sync_key.clone(),
                local_key_arc.clone(),
                cloud_last_sync_ms.clone(),
                core_config_arc.clone(),
                relay_auto_apply_cc,
                sync_in_flight.clone(),
            ) {
                Ok(handle) => {
                    tracing::info!("relay-sync: orchestrator started");
                    Some(handle)
                }
                Err(e) => {
                    tracing::warn!("relay-sync: failed to start ({e}); continuing without relay");
                    None
                }
            }
        } else {
            tracing::debug!("relay-sync: relay_url not set, skipping");
            None
        };
        // Publish into the shared slot the IPC server holds a clone of.
        *relay_handle_slot.lock().await = started;
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
    // at2m: ticker is `mut` so we can recreate it when poll_interval_ms changes
    // at runtime via set_config.  The current interval value is tracked in
    // `current_poll_ms`; when live_config diverges we replace the interval.
    let mut current_poll_ms = config.poll_interval_ms;
    let mut ticker = interval(Duration::from_millis(current_poll_ms));
    let mut cleanup_ticks: u64 = 0;
    // Sensitive TTL cleanup runs every 5 seconds; track elapsed ticks separately.
    let mut sensitive_cleanup_ticks: u64 = 0;

    tracing::info!("clipboard monitor started");
    tracing::info!(
        "sensitive auto-wipe TTL: {}s, checked every 5s",
        config.sensitive_ttl_secs,
    );

    #[cfg(target_os = "macos")]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut sigterm = signal(SignalKind::terminate())?;
        // CopyPaste-44rq.33: one cache instance shared across all ticks so
        // lsappinfo is forked at most once per FRONTMOST_APP_CACHE_TTL_SECS.
        let mut frontmost_cache = FrontmostAppCache::new();
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
                    // Hot-reload: snapshot the current live config on every tick
                    // so limit/feature changes from set_config take effect without
                    // a daemon restart (excluded_app_bundle_ids, paste_as_plain_text,
                    // sensitive_ttl_secs, etc.).
                    let live_config = core_config_arc
                        .read()
                        .map(|g| g.clone())
                        .unwrap_or_else(|_| config.clone());
                    // at2m: hot-reload the poll interval when set_config changes it.
                    // Recreating the interval resets its internal deadline to "now",
                    // which is safe: at worst we poll once immediately on the next
                    // select! iteration.  Reset cleanup_ticks to avoid a spurious
                    // early TTL run after a potentially large interval change.
                    if live_config.poll_interval_ms != current_poll_ms {
                        tracing::info!(
                            old_ms = current_poll_ms,
                            new_ms = live_config.poll_interval_ms,
                            "clipboard: poll_interval_ms changed — recreating interval timer"
                        );
                        current_poll_ms = live_config.poll_interval_ms;
                        ticker = interval(Duration::from_millis(current_poll_ms));
                    }
                    // P2: guard sensitive_ttl_secs == 0 → "disabled". When the
                    // user sets ttl to 0 (no auto-wipe), sensitive_ttl_ms would be
                    // 0, making threshold = now_ms - 0 = now_ms which deletes ALL
                    // sensitive items on every tick. Skip the cleanup entirely when
                    // ttl is 0 to honour the "disabled" intent.
                    let sensitive_ttl_ms = if live_config.sensitive_ttl_secs == 0 {
                        None
                    } else {
                        Some(live_config.sensitive_ttl_secs as i64 * 1000)
                    };
                    // Hot-reload the monitor's READ gate from the live config so
                    // raising/lowering the text/image/file cap via set_config
                    // takes effect without a restart (cheap: three field writes per tick).
                    monitor.set_max_text_bytes(live_config.max_text_size_bytes);
                    monitor.set_max_image_bytes(
                        usize::try_from(live_config.max_image_size_bytes).unwrap_or(usize::MAX),
                    );
                    monitor.set_max_file_bytes(
                        usize::try_from(live_config.max_file_size_bytes).unwrap_or(usize::MAX),
                    );
                    handle_tick(&mut monitor, &db, &local_key_arc, &live_config, &private_mode, &new_item_tx, &local_device_id, &mut frontmost_cache).await;
                    cleanup_ticks += 1;
                    sensitive_cleanup_ticks += 1;

                    // Sensitive item TTL: run every SENSITIVE_CLEANUP_INTERVAL_MS.
                    // Integer-divide gives 0 when poll_interval > interval; clamp
                    // to 1 so cleanup runs at most once per tick in that case.
                    let do_sensitive = sensitive_ttl_ms.is_some()
                        && sensitive_cleanup_ticks
                            >= (SENSITIVE_CLEANUP_INTERVAL_MS
                                / current_poll_ms.max(1))
                            .max(1);
                    if do_sensitive {
                        sensitive_cleanup_ticks = 0;
                    }
                    // General expires_at TTL: run every GENERAL_CLEANUP_INTERVAL_MS.
                    let do_general =
                        cleanup_ticks >= (GENERAL_CLEANUP_INTERVAL_MS / current_poll_ms.max(1)).max(1);
                    if do_general {
                        cleanup_ticks = 0;
                    }
                    // daemon-core L1: the deletes are synchronous rusqlite. Run
                    // them on a blocking thread (like the IPC path) so the async
                    // executor is never blocked while the DB lock is held.
                    run_ttl_cleanup(&db, sensitive_ttl_ms.unwrap_or(0), do_sensitive, do_general).await;
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
        // SIGTERM future: a real terminate-signal stream on unix, a never-resolving
        // future elsewhere. Boxed + always-defined so the select! branch below needs
        // NO in-macro #[cfg] attribute — tokio 1.52's select! macro rejects attributes
        // on branches ("no rules expected `}`", CopyPaste-l07l).
        let mut sigterm_fut: std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> = {
            #[cfg(unix)]
            {
                use tokio::signal::unix::{signal, SignalKind};
                let mut sig = signal(SignalKind::terminate())?;
                Box::pin(async move {
                    sig.recv().await;
                })
            }
            #[cfg(not(unix))]
            {
                Box::pin(std::future::pending::<()>())
            }
        };
        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    let live_config = core_config_arc
                        .read()
                        .map(|g| g.clone())
                        .unwrap_or_else(|_| config.clone());
                    // at2m: hot-reload the poll interval when set_config changes it.
                    if live_config.poll_interval_ms != current_poll_ms {
                        tracing::info!(
                            old_ms = current_poll_ms,
                            new_ms = live_config.poll_interval_ms,
                            "clipboard: poll_interval_ms changed — recreating interval timer"
                        );
                        current_poll_ms = live_config.poll_interval_ms;
                        ticker = interval(Duration::from_millis(current_poll_ms));
                    }
                    let sensitive_ttl_ms = if live_config.sensitive_ttl_secs == 0 {
                        None
                    } else {
                        Some(live_config.sensitive_ttl_secs as i64 * 1000)
                    };
                    // Hot-reload the monitor's READ gate from the live config so
                    // raising/lowering the text/image/file cap via set_config
                    // takes effect without a restart (cheap: three field writes per tick).
                    monitor.set_max_text_bytes(live_config.max_text_size_bytes);
                    monitor.set_max_image_bytes(
                        usize::try_from(live_config.max_image_size_bytes).unwrap_or(usize::MAX),
                    );
                    monitor.set_max_file_bytes(
                        usize::try_from(live_config.max_file_size_bytes).unwrap_or(usize::MAX),
                    );
                    handle_tick(&mut monitor, &db, &local_key_arc, &live_config, &private_mode, &new_item_tx, &local_device_id).await;
                    cleanup_ticks += 1;
                    sensitive_cleanup_ticks += 1;

                    // Sensitive item TTL: run every SENSITIVE_CLEANUP_INTERVAL_MS.
                    let do_sensitive = sensitive_ttl_ms.is_some()
                        && sensitive_cleanup_ticks
                            >= (SENSITIVE_CLEANUP_INTERVAL_MS
                                / current_poll_ms.max(1))
                            .max(1);
                    if do_sensitive {
                        sensitive_cleanup_ticks = 0;
                    }
                    // General expires_at TTL: run every GENERAL_CLEANUP_INTERVAL_MS.
                    let do_general =
                        cleanup_ticks >= (GENERAL_CLEANUP_INTERVAL_MS / current_poll_ms.max(1)).max(1);
                    if do_general {
                        cleanup_ticks = 0;
                    }
                    // daemon-core L1: offload the synchronous rusqlite deletes.
                    run_ttl_cleanup(&db, sensitive_ttl_ms.unwrap_or(0), do_sensitive, do_general).await;
                }
                _ = tokio::signal::ctrl_c() => {
                    tracing::info!("SIGINT received, shutting down");
                    // D3: broadcast shutdown to all tasks.
                    shutdown_token.cancel();
                    break;
                }
                _ = &mut sigterm_fut => {
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
