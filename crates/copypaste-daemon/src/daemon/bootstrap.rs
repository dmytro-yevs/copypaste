//! One-shot daemon bootstrap tasks: telemetry reporter init, best-effort
//! Keychain ACL rotation, the v4 key-version migration sweep, the poison-row
//! sweep, the DeviceMeta cache warm, the startup TTL purge, and device-name
//! resolution.

use copypaste_core::{AppConfig, Database};
use copypaste_telemetry::{report_and_log, OsTag, ReportConsent, ReportableError};
use std::sync::Arc;
use tokio::sync::Mutex;

use super::{run_ttl_cleanup, sweep_keys};

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

/// CopyPaste-9fb6: initialise the telemetry reporter.  The default is
/// `Disabled` — no events leave the device until the user opts in via the
/// settings UI (a future PR).  The reporter is passed into error-handling
/// sites in the daemon lifecycle so wiring is complete and adding consent is a
/// one-line change.  When `COPYPASTE_SENTRY_DSN` is not set (OSS builds, CI)
/// this always returns a `NoopReporter`.
pub(crate) fn init_reporter() -> Box<dyn copypaste_telemetry::ErrorReporter> {
    match option_env!("COPYPASTE_SENTRY_DSN") {
        Some(dsn) => copypaste_telemetry::init_with_dsn(ReportConsent::Disabled, dsn)
            .unwrap_or_else(|e| {
                tracing::warn!(error = %e, "telemetry init failed; using no-op reporter");
                copypaste_telemetry::init(ReportConsent::Disabled)
            }),
        None => copypaste_telemetry::init(ReportConsent::Disabled),
    }
}

/// v0.3 (THREAT-MODEL OI-4): upgrade the Keychain entry's ACL on first launch
/// after install/upgrade.  Idempotent + best-effort — a failure here (e.g. user
/// denied a Keychain prompt) must not block the daemon because the entry is
/// still usable, just with the legacy unrestricted ACL.  The next launch retries
/// automatically.
///
/// Only runs when the Keychain backend is actually in use. On ad-hoc / unsigned
/// installs the key lives in the 0600 file store (see `keychain::file_store`)
/// and there is no Keychain ACL to rotate — calling the rotation there would
/// read/delete/recreate a Keychain item and raise the very login-password prompt
/// this whole change exists to eliminate.
///
/// CHICKEN-AND-EGG (acceptance criterion #4): after a reinstall the binary's
/// code signature changes and the Keychain ACL no longer trusts it, so
/// `rotate_acl_to_current_install` ITSELF calls `get_generic_password` first to
/// read the secret it would re-store — the very read that now prompts / is
/// denied. It therefore cannot re-establish trust without the access it is trying
/// to repair. We keep it best-effort here (it succeeds on the benign
/// install-moved case) and rely on the DEGRADED startup path as the safety net
/// for the untrusted-binary case. Real recovery is a one-time user re-grant of
/// the Keychain prompt on a subsequent launch, after which this rotation pins
/// the new binary so later launches stay quiet.
#[cfg(target_os = "macos")]
pub(crate) fn rotate_keychain_acl_best_effort() {
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
}

/// v4 key-version migration sweep — runs once at startup (resumable).
///
/// The sweep rotates any remaining `key_version = 1` rows to `key_version = 2`.
/// It is synchronous (rusqlite), so we offload it to a blocking thread via
/// `spawn_blocking` and await the result before continuing. On error we WARN and
/// continue — a partially-swept DB is still usable; new writes keep being
/// rejected by the migration gate until the sweep eventually completes on a
/// future restart.
pub(crate) async fn run_v4_migration_sweep(
    db: &Arc<Mutex<Database>>,
    local_key_arc: &Arc<zeroize::Zeroizing<[u8; 32]>>,
    reporter: &dyn copypaste_telemetry::ErrorReporter,
) {
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
    let seed: [u8; 32] = ***local_key_arc;
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
                reporter,
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

/// One-time startup sweep: delete poison rows created before the inbound-merge
/// guard was added (CopyPaste-jww / CopyPaste-5y4). A poison row is a text item
/// where content IS NOT NULL AND content_nonce IS NULL, or a file/image item
/// where content IS NOT NULL AND content_nonce IS NULL AND blob_ref IS NULL. The
/// peer re-sends these items on the next catch-up cycle. Idempotent.
pub(crate) async fn run_poison_row_sweep(db: &Arc<Mutex<Database>>) {
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

/// CopyPaste-bps: warm the DeviceMeta cache ONCE at startup, before the IPC
/// server is constructed. `DeviceMeta::collect` spawns child processes
/// (`scutil`, `sysctl`, `sw_vers`) that together can take up to ~6 s on a cold
/// macOS system. By running them here on a blocking thread we pay the cost
/// exactly once; all subsequent calls to `collect_own_peer_meta` and
/// `get_own_device_info` are instant cache reads (OnceLock — wait-free after the
/// first write).
pub(crate) async fn warm_device_meta_cache() {
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
}

/// P2 (ugv7) — startup TTL purge: run the same TTL cleanup the tick loop
/// performs, ONCE, right after the database is opened and BEFORE the IPC socket
/// is bound. Without this, expired sensitive items remain readable and
/// searchable during the sub-second window between DB open and the first tick.
/// Reuses `run_ttl_cleanup` exactly as the tick loop does, passing
/// `do_sensitive = sensitive_ttl_ms.is_some()` and `do_general = true`.
pub(crate) async fn run_startup_ttl_purge(db: &Arc<Mutex<Database>>, config: &AppConfig) {
    let startup_sensitive_ttl_ms = if config.sensitive_ttl_secs == 0 {
        None
    } else {
        Some(config.sensitive_ttl_secs as i64 * 1000)
    };
    run_ttl_cleanup(
        db,
        startup_sensitive_ttl_ms.unwrap_or(0),
        startup_sensitive_ttl_ms.is_some(), // do_sensitive
        true,                               // do_general: always purge expired items at startup
    )
    .await;
    tracing::debug!("startup TTL purge complete (before IPC socket bind)");
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;

    // crh3.78: regression guard for the lifecycle helpers extracted from the
    // `run_with_quit_flag` god-function. These pin the extracted symbols'
    // existence and their behaviour-preserving contracts (no panic, no error
    // on the empty-DB startup paths).

    #[test]
    fn init_reporter_returns_usable_reporter() {
        // With no COPYPASTE_SENTRY_DSN baked in (OSS/CI builds) this returns a
        // NoopReporter; report_and_log must be a safe no-op.
        let reporter = init_reporter();
        report_and_log(
            &*reporter,
            ReportableError::new(
                env!("CARGO_PKG_NAME"),
                env!("CARGO_PKG_VERSION"),
                "test.init_reporter",
                OsTag::current(),
            ),
        );
    }

    #[tokio::test]
    async fn startup_ttl_purge_is_noop_on_empty_db() {
        let db = Arc::new(Mutex::new(Database::open_in_memory().expect("open db")));
        let config = AppConfig::default();
        // Must complete without panicking on an empty database.
        run_startup_ttl_purge(&db, &config).await;
    }

    #[tokio::test]
    async fn poison_row_sweep_is_noop_on_empty_db() {
        let db = Arc::new(Mutex::new(Database::open_in_memory().expect("open db")));
        // No poison rows in a fresh DB → sweep completes cleanly.
        run_poison_row_sweep(&db).await;
    }
}
