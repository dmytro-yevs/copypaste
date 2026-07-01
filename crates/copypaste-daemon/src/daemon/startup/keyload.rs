//! Startup key/plan logic: `KeyLoad`, `DbStartupPlan`, `decide_db_startup`,
//! `encrypted_db_exists`, `load_local_key_bounded` (timeout thread),
//! `load_local_key_material` (Keychain), and `sweep_keys` (v4 migration key
//! pair).

use copypaste_core::DeviceKeypair;

// crh3.78: only the macOS `load_local_key_bounded` branch waits on this timeout;
// gate the import so the non-macOS (Linux) build stays clean under -D warnings.
#[cfg(target_os = "macos")]
use crate::daemon::KEYCHAIN_READ_TIMEOUT;

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
/// wait on a channel for at most `KEYCHAIN_READ_TIMEOUT`. On timeout we
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
                // CopyPaste-qvtg.1: the full 64-char fingerprint is the device's
                // mTLS-pinning identity; emitting it at info on every restart
                // leaks it into persistent log stores (OSLog/journald) where
                // diagnostics tooling may collect it, enabling targeted
                // impersonation/enumeration. Log only the short prefix at info
                // (matches keychain::generate's new-key path) and keep the full
                // value at debug for deep diagnostics.
                let fp = kp.fingerprint();
                tracing::info!(
                    "device fingerprint_prefix={}",
                    fp.get(..23).unwrap_or(fp.as_str())
                );
                tracing::debug!("full device fingerprint={}", fp);
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
            copypaste_core::V1Key(&v1_key),
            copypaste_core::V2Key(&v2_key),
            &copypaste_core::ItemId::from(item_id.as_str()),
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
        let aad = build_item_aad(
            &copypaste_core::ItemId::from(item_id.as_str()),
            AAD_SCHEMA_VERSION,
        );
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
        let aad_v2 = copypaste_core::build_item_aad_v2(
            &copypaste_core::ItemId::from(item_id.as_str()),
            AAD_SCHEMA_VERSION_V4,
            2,
        );
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
}
