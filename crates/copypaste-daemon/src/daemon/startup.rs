use crate::{keychain, paths};
use copypaste_core::{AppConfig, Database, DeviceKeypair};
use copypaste_telemetry::{report_and_log, OsTag, ReportConsent, ReportableError};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use super::DEGRADED_QUIT_POLL_INTERVAL_MS;
use super::KEYCHAIN_READ_TIMEOUT;

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
pub(crate) async fn run_degraded(
    reason: &'static str,
    quit_flag: Arc<AtomicBool>,
) -> anyhow::Result<()> {
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
        // PG-14 (CopyPaste-tpvi): Load private-mode from the filesystem flag
        // file — NOT the DB, which is unavailable in degraded mode.  The flag
        // file is completely independent of the encrypted database, so
        // load_private_mode() is safe to call here.  Defaulting to `false`
        // (capture ON) when the prior state was private would be a silent
        // privacy regression; we mirror the normal-startup path instead.
        let prior_private = load_private_mode();
        if prior_private {
            tracing::warn!(
                "degraded boot: private mode was ON before this degraded boot; \
                 preserving capture-OFF state from persisted flag"
            );
        }
        let private_mode = Arc::new(AtomicBool::new(prior_private));
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
        let mut quit_ticker =
            tokio::time::interval(Duration::from_millis(DEGRADED_QUIT_POLL_INTERVAL_MS));
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
pub(crate) fn sweep_keys(seed: &[u8; 32]) -> ([u8; 32], zeroize::Zeroizing<[u8; 32]>) {
    // v1_key: the seed itself, used directly — exactly as the read path uses
    //         `**self.local_key` for `key_version = 1` rows.
    // v2_key: `derive_v2(seed)`, matching the read path's `derive_v2(&v1_key)`.
    // Item 5: derive_v2 now returns Zeroizing<[u8;32]>; propagate the wrapper
    // so the key bytes are scrubbed when the caller drops the tuple.
    (*seed, copypaste_core::derive_v2(seed))
}

/// Outcome of the bounded startup key read.
///
/// `Ready` carries the device's local storage key. `Locked` means the key
/// could not be obtained in bounded time / at all — the Keychain ACL no longer
/// trusts this binary (post-reinstall), the read is blocked on an unanswered
/// GUI prompt, or access was denied (launchd, no interactive session). We do
/// NOT synthesize an ephemeral key here: doing so against an EXISTING encrypted
/// DB yields "file is not a database" and a dead daemon (the exact regression).
pub(crate) enum KeyLoad {
    /// Carries the device's local storage key AND the X25519 public-key bytes,
    /// loaded once together (see `load_local_key_material`).
    Ready(zeroize::Zeroizing<[u8; 32]>, [u8; 32]),
    Locked,
}

/// The plan for opening the database at startup, derived from the key-load
/// outcome and whether an encrypted DB already exists. Pure + total so it is
/// unit-testable without a Keychain or a real DB.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum DbStartupPlan {
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
pub(crate) fn decide_db_startup(key: &KeyLoad, encrypted_db_exists: bool) -> DbStartupPlan {
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
pub(crate) fn encrypted_db_exists(path: &std::path::Path) -> bool {
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
pub(crate) fn load_local_key_bounded() -> KeyLoad {
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
pub(crate) fn load_local_key_material() -> KeyLoad {
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
pub(crate) fn load_config() -> AppConfig {
    let path = paths::config_path();
    AppConfig::load(&path).unwrap_or_else(|e| {
        // P3 (audit): distinguish a missing config file (first run — silent,
        // expected) from a TOML parse error (corrupted/hand-edited config —
        // warn so the operator knows their edits were discarded).
        match &e {
            copypaste_core::config::ConfigError::Io(io_err)
                if io_err.kind() == std::io::ErrorKind::NotFound =>
            {
                // First run: config file does not exist yet; defaults are fine.
                tracing::debug!(
                    "config file not found at {}; using defaults",
                    path.display()
                );
            }
            _ => {
                // Parse error or unexpected IO error — operator action may be needed.
                tracing::warn!(
                    error = %e,
                    path = %path.display(),
                    "config file could not be loaded (TOML parse error?); \
                     falling back to defaults — fix or delete the file to silence this"
                );
            }
        }
        let cfg = AppConfig::default();
        if let Err(e) = cfg.save(&path) {
            tracing::warn!("could not save default config: {e}");
        }
        cfg
    })
}

/// Write `text` to `path` atomically with mode `0600` (Fix-2).
///
/// Creates a uniquely-named temp file in the SAME directory (so rename is
/// atomic and same-filesystem), sets mode `0600` before writing any bytes,
/// writes + flushes + syncs, then renames over the destination. This prevents
/// the world-readable window that exists between `std::fs::write` (creates at
/// umask-derived mode, typically `0644`) and a subsequent `set_permissions`.
pub(crate) fn write_text_atomic_0600(path: &std::path::Path, text: &str) -> anyhow::Result<()> {
    use std::io::Write as _;

    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("path has no parent directory: {}", path.display()))?;
    std::fs::create_dir_all(parent)?;

    let tmp = parent.join(format!(
        ".{}.tmp.{}.{}",
        path.file_name().and_then(|n| n.to_str()).unwrap_or("file"),
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));

    let write_result = (|| -> std::io::Result<()> {
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut f = opts.open(&tmp)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            f.set_permissions(std::fs::Permissions::from_mode(0o600))?;
        }
        f.write_all(text.as_bytes())?;
        f.flush()?;
        f.sync_all()?;
        Ok(())
    })();

    if let Err(e) = write_result {
        let _ = std::fs::remove_file(&tmp);
        return Err(e.into());
    }
    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e.into());
    }
    Ok(())
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
pub(crate) fn load_or_create_device_id() -> anyhow::Result<uuid::Uuid> {
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
    // Fix-2 (atomic 0600 write): write via temp-then-rename so the device_id
    // is never world-readable between create and chmod.  The device_id is not
    // a secret per se, but it is used as the stable identity for pairing/sync
    // and should be owner-only for consistency with peers.json and config.json.
    write_text_atomic_0600(&path, &id.to_string())?;

    tracing::info!(device_id = %id, path = %path.display(), "created persistent device_id");
    Ok(id)
}

/// Restore the persisted private-mode flag at startup.
///
/// Returns `false` (capture enabled) when the file is absent, unreadable, or
/// holds anything other than `"1"` — a missing/corrupt flag must never leave
/// the daemon stuck in private mode, and on first run there is no file yet.
pub(crate) fn load_private_mode() -> bool {
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
///
/// CopyPaste-ki7p: previously used `std::fs::write` which inherits the process
/// umask (typically 0022), creating the flag file world-readable at 0644.
/// The flag file is not a secret itself but its presence reveals whether the user
/// is in private/pause mode — information that should not leak to other local
/// users on a multi-user machine. We use `write_text_atomic_0600` which opens
/// the temp file with O_CREAT|mode(0600) before any bytes are written, so there
/// is never a window where the file exists at a permissive mode.
pub(crate) fn persist_private_mode(enabled: bool) {
    let path = match crate::paths::private_mode_path() {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!("could not resolve private_mode path ({e}); not persisting");
            return;
        }
    };
    // write_text_atomic_0600 creates the parent directory, writes atomically via
    // a temp-file rename, and sets mode 0600 before any bytes are written.
    if let Err(e) = write_text_atomic_0600(&path, if enabled { "1" } else { "0" }) {
        tracing::warn!(
            path = %path.display(),
            error = %e,
            "could not persist private_mode flag"
        );
    }
}

// Suppress unused import warnings for items only used via cfg-gated paths.
#[allow(unused_imports)]
use keychain as _;
#[allow(unused_imports)]
use report_and_log as _;
#[allow(unused_imports)]
use OsTag as _;
#[allow(unused_imports)]
use ReportConsent as _;
#[allow(unused_imports)]
use ReportableError as _;

#[cfg(test)]
mod tests {
    use super::*;
    use copypaste_core::derive_v2;
    use copypaste_core::{
        build_item_aad, encrypt_item_with_aad, Database, AAD_SCHEMA_VERSION, AAD_SCHEMA_VERSION_V4,
        NONCE_SIZE,
    };

    fn ready_key() -> KeyLoad {
        KeyLoad::Ready(zeroize::Zeroizing::new([0x11u8; 32]), [0x22u8; 32])
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
        use copypaste_core::decrypt_item_by_version;
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

    // -----------------------------------------------------------------------
    // Keychain-locked degraded-startup decision logic (regression: daemon hung
    // or died after reinstall when the SQLCipher key became unreadable).
    // -----------------------------------------------------------------------

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

    /// CopyPaste-ki7p: `persist_private_mode` must create the flag file with
    /// mode 0600, not the umask-derived 0644. Verified on Unix only — Windows
    /// has no meaningful POSIX mode bits.
    #[cfg(unix)]
    #[test]
    fn private_mode_flag_file_is_created_with_mode_0600() {
        use std::os::unix::fs::PermissionsExt;

        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("private_mode");

        let _guard = crate::TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let prev = std::env::var_os("COPYPASTE_PRIVATE_MODE_PATH");
        // SAFETY: held under TEST_ENV_LOCK; restored unconditionally below.
        unsafe {
            std::env::set_var("COPYPASTE_PRIVATE_MODE_PATH", &path);
        }

        persist_private_mode(true);

        // Restore env before any assertions so a failure doesn't leak state.
        unsafe {
            match prev {
                Some(v) => std::env::set_var("COPYPASTE_PRIVATE_MODE_PATH", v),
                None => std::env::remove_var("COPYPASTE_PRIVATE_MODE_PATH"),
            }
        }

        assert!(path.exists(), "flag file must be created");
        let mode = std::fs::metadata(&path)
            .expect("metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(
            mode, 0o600,
            "CopyPaste-ki7p: private_mode flag file must be 0600, got {mode:#o}"
        );
    }

    // -----------------------------------------------------------------------
    // P1-4: Open+Locked combination now routes to degraded (not unreachable!)
    // -----------------------------------------------------------------------

    /// `decide_db_startup` can never return `Open` when `KeyLoad::Locked` — the
    /// invariant is structural.  But if it ever did (future refactor), the code
    /// path at daemon startup must NOT panic.  This test documents the intended
    /// contract: `Open` is produced only for `Ready`.  If the invariant ever
    /// breaks, the daemon now enters `run_degraded` rather than crashing — this
    /// test asserts that `decide_db_startup(&Ready(..), true) == Open` (the only
    /// path that feeds into the key-extraction match), while the Locked arm is
    /// covered by `decide_db_startup_locked_key_with_existing_db_degrades`.
    #[test]
    fn open_plan_requires_ready_key() {
        // `decide_db_startup` with a Ready key → Open (only path to the Open arm).
        assert_eq!(decide_db_startup(&ready_key(), true), DbStartupPlan::Open);
        assert_eq!(decide_db_startup(&ready_key(), false), DbStartupPlan::Open);
        // Locked never produces Open — so the unreachable!→graceful arm is never
        // reached through decide_db_startup; the graceful arm is a belt-and-
        // suspenders guard for direct callers / future code paths.
        assert_ne!(
            decide_db_startup(&KeyLoad::Locked, true),
            DbStartupPlan::Open
        );
        assert_ne!(
            decide_db_startup(&KeyLoad::Locked, false),
            DbStartupPlan::Open
        );
    }

    // -----------------------------------------------------------------------
    // PG-14 (CopyPaste-tpvi): degraded boot must not default capture to ON
    // -----------------------------------------------------------------------

    /// Regression guard for PG-14: when the user had private mode enabled
    /// before a degraded boot (e.g. Keychain locked), the degraded path must
    /// preserve `private_mode = true` (capture OFF), not silently reset it to
    /// false (capture ON).
    ///
    /// This is a pure unit test of `load_private_mode()` — the same function
    /// now called by `run_degraded`.  It simulates a persisted-ON flag being
    /// read at degraded-boot time and asserts that the value is `true`, which
    /// means the `AtomicBool` would be initialised capture-OFF.
    #[test]
    fn degraded_boot_respects_persisted_private_mode() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("private_mode");

        // Serialise env mutation with all other env-mutating daemon tests.
        let _guard = crate::TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let prev = std::env::var_os("COPYPASTE_PRIVATE_MODE_PATH");
        // SAFETY: held under TEST_ENV_LOCK; restored unconditionally below.
        unsafe {
            std::env::set_var("COPYPASTE_PRIVATE_MODE_PATH", &path);
        }

        // Simulate: user had private mode ON at the time of the degraded boot.
        persist_private_mode(true);

        // This is what run_degraded now calls — must return true (capture OFF).
        let loaded = load_private_mode();

        // Restore env before assertions so a failure doesn't leak state.
        unsafe {
            match prev {
                Some(v) => std::env::set_var("COPYPASTE_PRIVATE_MODE_PATH", v),
                None => std::env::remove_var("COPYPASTE_PRIVATE_MODE_PATH"),
            }
        }

        assert!(
            loaded,
            "PG-14: degraded boot with prior private_mode=ON must load true, \
             not silently reset to false (capture ON)"
        );

        // Explicitly release the first lock before acquiring it again for the
        // second scenario. `std::sync::Mutex` is NOT reentrant — holding `_guard`
        // while calling `.lock()` below would deadlock the current thread.
        drop(_guard);

        // Also verify the inverse: absent flag file (first-ever run or cleared)
        // correctly defaults to false (capture ON is the correct default for a
        // fresh install, not for a return from private mode).
        let tmp2 = tempfile::tempdir().expect("tempdir2");
        let path2 = tmp2.path().join("private_mode");

        let _guard2 = crate::TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let prev2 = std::env::var_os("COPYPASTE_PRIVATE_MODE_PATH");
        unsafe {
            std::env::set_var("COPYPASTE_PRIVATE_MODE_PATH", &path2);
        }
        // path2 does not exist yet — first-run scenario.
        let loaded_absent = load_private_mode();
        unsafe {
            match prev2 {
                Some(v) => std::env::set_var("COPYPASTE_PRIVATE_MODE_PATH", v),
                None => std::env::remove_var("COPYPASTE_PRIVATE_MODE_PATH"),
            }
        }
        assert!(
            !loaded_absent,
            "absent private_mode file (first run) must default to false (capture ON)"
        );
    }

    // -----------------------------------------------------------------------
    // 58ou (PG-31): auto_apply_synced_clip config field contract test
    // -----------------------------------------------------------------------

    /// Verifies that auto_apply_synced_clip defaults to true and can be
    /// persisted/loaded from config.toml.  The actual pasteboard write is
    /// tested in sync_orch; this test confirms the config field contract.
    #[test]
    fn auto_apply_synced_clip_defaults_to_true_in_appconfig() {
        let cfg = AppConfig::default();
        assert!(
            cfg.auto_apply_synced_clip,
            "auto_apply_synced_clip must default to true"
        );
    }

    #[test]
    fn auto_apply_synced_clip_false_persists_and_loads() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("config.toml");
        let cfg = AppConfig {
            auto_apply_synced_clip: false,
            ..Default::default()
        };
        cfg.save(&path).unwrap();
        let loaded = AppConfig::load(&path).unwrap();
        assert!(
            !loaded.auto_apply_synced_clip,
            "auto_apply_synced_clip=false must survive save/load"
        );
    }

    // ───────────────────────────────────────────────────────────────────────
    // CopyPaste-9fb6: telemetry wiring smoke tests
    // ───────────────────────────────────────────────────────────────────────

    /// The Disabled reporter (the production default) must never panic and must
    /// accept any ReportableError without performing any I/O.
    #[test]
    fn telemetry_reporter_disabled_is_noop() {
        use copypaste_telemetry::{report_and_log, OsTag, ReportConsent, ReportableError};
        let reporter = copypaste_telemetry::init(ReportConsent::Disabled);
        // Should not panic and should return Ok.
        report_and_log(
            &*reporter,
            ReportableError::new(
                env!("CARGO_PKG_NAME"),
                env!("CARGO_PKG_VERSION"),
                "test.noop_event",
                OsTag::current(),
            ),
        );
    }

    /// `init_with_dsn` with Disabled consent must return a NoopReporter (no
    /// network I/O) even when a DSN is supplied.
    #[test]
    fn telemetry_init_with_dsn_disabled_returns_noop() {
        use copypaste_telemetry::{report_and_log, OsTag, ReportConsent, ReportableError};
        let reporter = copypaste_telemetry::init_with_dsn(
            ReportConsent::Disabled,
            "https://public@sentry.example/1",
        )
        .expect("disabled init must not fail");
        report_and_log(
            &*reporter,
            ReportableError::new(
                env!("CARGO_PKG_NAME"),
                env!("CARGO_PKG_VERSION"),
                "test.dsn_disabled_noop",
                OsTag::current(),
            ),
        );
    }

    /// `init_with_dsn` with a garbage DSN and Disabled consent must still
    /// succeed (the DSN is not parsed until the SDK is initialised, and
    /// `Disabled` skips initialisation entirely).
    #[test]
    fn telemetry_init_with_garbage_dsn_disabled_is_ok() {
        use copypaste_telemetry::ReportConsent;
        let reporter =
            copypaste_telemetry::init_with_dsn(ReportConsent::Disabled, "not-a-dsn-at-all")
                .expect("garbage DSN + Disabled must succeed (SDK not initialised)");
        drop(reporter);
    }
}
