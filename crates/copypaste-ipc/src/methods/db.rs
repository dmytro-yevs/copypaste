//! Database-maintenance METHOD_* constants and their DTO types.

use serde::{Deserialize, Serialize};

// ── Database maintenance ────────────────────────────────────────────────────

/// Return lightweight storage statistics for the local clipboard database.
///
/// Params: none (empty `{}`).
/// Response: `{ item_count: u64, size_bytes: u64 }`.
///
/// - `item_count` — total number of items stored (includes deleted/tombstoned rows).
/// - `size_bytes` — approximate on-disk size of the main database file in bytes.
///   Does not include the WAL file; use [`METHOD_VACUUM`] to flush WAL into the main
///   file before calling this if you need an accurate compacted size.
///
/// Used by the macOS UI's settings panel (SettingsView.gq51) to show a storage
/// usage summary without triggering the heavier `stats` computation.
pub const METHOD_DB_STATS: &str = "db_stats";

/// Success payload for [`METHOD_DB_STATS`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct DbStatsResponse {
    /// Total number of items in `clipboard_items` (all rows, including tombstones).
    pub item_count: u64,
    /// On-disk size of the main database file in bytes (WAL not included).
    pub size_bytes: u64,
}

/// Run `VACUUM` (and optionally `REINDEX`) on the encrypted clipboard database.
///
/// The daemon holds the write-lock for the duration and runs the operation on a
/// blocking thread so the async executor is not starved. The daemon MUST be
/// running for this method to be callable — the client no longer needs to stop
/// the daemon, open the DB directly, or touch the macOS Keychain.
///
/// ## Parameters ([`VacuumRequest`])
/// - `reindex_only` (`bool`, default `false`): skip `VACUUM`, run only `REINDEX`.
/// - `dry_run` (`bool`, default `false`): open the DB to verify the key, report
///   current size, but do NOT mutate any data.
///
/// ## Response ([`VacuumResponse`])
/// - `size_before` (`u64`): file size in bytes before the operation.
/// - `size_after` (`u64`): file size in bytes after (same as `size_before` on
///   `dry_run`).
/// - `reclaimed` (`i64`): `size_before - size_after` (negative = file grew,
///   e.g. after `REINDEX` on a fragmented DB).
///
/// Success is conveyed solely by the outer `Response.ok` envelope field;
/// the payload carries only meaningful data fields (c4q2.22).
pub const METHOD_VACUUM: &str = "vacuum";

/// Parameters for the [`METHOD_VACUUM`] method.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct VacuumRequest {
    /// When `true`, skip `VACUUM` and run only `REINDEX`. Faster; does not
    /// require free space equal to the current DB size.
    #[serde(default)]
    pub reindex_only: bool,
    /// When `true`, report what would happen without mutating the database.
    #[serde(default)]
    pub dry_run: bool,
}

/// Success payload for the [`METHOD_VACUUM`] method.
///
/// The outer `Response.ok` envelope is the authoritative success indicator;
/// this struct carries only data fields that add information (c4q2.22 —
/// removed the formerly-redundant `ok: bool` field).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct VacuumResponse {
    /// DB file size in bytes *before* the operation.
    pub size_before: u64,
    /// DB file size in bytes *after* the operation (equals `size_before` on
    /// `dry_run`).
    pub size_after: u64,
    /// `size_before - size_after`; negative when the file grew.
    pub reclaimed: i64,
}

/// Method name for the destructive "reset database" recovery operation.
///
/// This wipes `clipboard.db` (and its `-wal` / `-shm` siblings) and recreates a
/// fresh, empty encrypted database with the daemon's current key. It is the
/// explicit escape hatch a user invokes from the desktop UI when the daemon is
/// running DEGRADED because the existing database cannot be decrypted (key
/// mismatch / "file is not a database"). Unlike every other DB-touching method,
/// the daemon honours this one *in* degraded mode — that is the whole point.
///
/// MUST carry [`ResetDatabaseRequest::confirm`] = `true` or the daemon refuses
/// it, so it can never fire by accident.
pub const METHOD_RESET_DATABASE: &str = "reset_database";

/// Parameters for the [`METHOD_RESET_DATABASE`] method.
///
/// `confirm` is a mandatory explicit acknowledgement of the destructive intent.
/// The daemon rejects the request with `invalid_argument` unless `confirm` is
/// `true`, so a stray or replayed `reset_database` call with no/false confirm
/// cannot erase the user's history.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ResetDatabaseRequest {
    /// Must be `true` to authorise the destructive wipe-and-recreate.
    #[serde(default)]
    pub confirm: bool,
}

/// Success payload for the [`METHOD_RESET_DATABASE`] method.
///
/// On success the daemon has deleted the old database files, created a fresh
/// empty encrypted database with its current key, and brought itself OUT of
/// degraded mode in-place — so a subsequent `history_page` (or any DB-touching
/// method) succeeds against the new empty DB without a process restart.
///
/// The outer `Response.ok` envelope is the authoritative success indicator.
/// The former `reset: bool` field (always `true` on success) was removed as
/// redundant (c4q2.22); callers must check the envelope `ok` field instead.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ResetDatabaseResponse {
    /// `true` when the daemon recovered IN-PLACE (no restart needed): the new
    /// empty DB is live and the daemon is now ready. The current implementation
    /// always recovers in-place, so this is always `true` on success.
    pub ready: bool,
}

// ── Database backup / restore (CopyPaste-x94p / CopyPaste-8wbt) ─────────────

/// Create an encrypted SQLCipher backup of the local clipboard database.
///
/// The daemon owns both the database file and the encryption key, so it can
/// produce a hot, consistent backup without stopping itself. Internally the
/// handler runs `VACUUM INTO '<dest>'` which copies every non-empty page into
/// a new file encrypted with the **same key** as the source database.
///
/// ## Parameters ([`DbBackupRequest`])
/// - `dest_path` (`String`): absolute path for the output backup file.
///   The file must NOT already exist; the daemon refuses to overwrite.
///
/// ## Response ([`DbBackupResponse`])
/// - `dest_path` (`String`): the path the backup was written to.
/// - `size_bytes` (`u64`): size of the backup file in bytes.
///
/// (`ok` field removed c4q2.22 — the outer `Response.ok` envelope is authoritative.)
///
/// (CopyPaste-x94p)
pub const METHOD_DB_BACKUP: &str = "db_backup";

/// Parameters for [`METHOD_DB_BACKUP`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DbBackupRequest {
    /// Absolute path where the backup file will be written.
    /// The daemon refuses to overwrite an existing file.
    pub dest_path: String,
}

/// Success payload for [`METHOD_DB_BACKUP`].
///
/// The outer `Response.ok` envelope is the authoritative success indicator;
/// the former `ok: bool` field (always `true` on success) was removed as
/// redundant (c4q2.22).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct DbBackupResponse {
    /// The path the backup was written to (mirrors `DbBackupRequest::dest_path`).
    pub dest_path: String,
    /// Size of the backup file in bytes.
    pub size_bytes: u64,
}

/// Restore the local clipboard database from an encrypted SQLCipher backup.
///
/// The daemon must be running to service this call. The handler:
///
/// 1. Validates `confirm = true` (refuses without it).
/// 2. Verifies the backup file exists and is readable.
/// 3. Swaps the live DB handle to an in-memory instance so all pending writes
///    are quiesced (mirrors the `reset_database` safe-swap pattern).
/// 4. Renames the existing `clipboard.db` (+ WAL/SHM) aside to a timestamped
///    `.before-restore-<ts>` name (or deletes them when `force = true`).
/// 5. Copies the backup file into place as `clipboard.db`.
/// 6. Reopens the database with the daemon's current key.
///    The backup **must** have been encrypted with this same key — if the key
///    mismatches, `Database::open` returns an error and the daemon remains
///    degraded (the aside file is intact for manual recovery).
/// 7. Swaps the live handle back to the restored database and returns ready.
///
/// ## Parameters ([`DbRestoreRequest`])
/// - `confirm` (`bool`): must be `true`; prevents accidental invocations.
/// - `src_path` (`String`): absolute path to the backup file to restore.
/// - `force` (`bool`, default `false`): delete the existing DB instead of
///   renaming it aside. Use when disk space is tight.
///
/// ## Response ([`DbRestoreResponse`])
/// - `ready` (`bool`): always `true`; the restored DB is live.
///
/// (`ok` field removed c4q2.22 — the outer `Response.ok` envelope is authoritative.)
///
/// (CopyPaste-8wbt)
pub const METHOD_DB_RESTORE: &str = "db_restore";

/// Parameters for [`METHOD_DB_RESTORE`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct DbRestoreRequest {
    /// Must be `true` to authorise the destructive replace-in-place.
    #[serde(default)]
    pub confirm: bool,
    /// Absolute path to the backup file to restore from.
    pub src_path: String,
    /// When `true`, delete the existing live DB instead of renaming it aside.
    #[serde(default)]
    pub force: bool,
}

/// Success payload for [`METHOD_DB_RESTORE`].
///
/// The outer `Response.ok` envelope is the authoritative success indicator;
/// the former `ok: bool` field (always `true` on success) was removed as
/// redundant (c4q2.22).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct DbRestoreResponse {
    /// `true` when the restored database is live (no restart needed).
    pub ready: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn db_stats_method_has_correct_wire_name() {
        assert_eq!(METHOD_DB_STATS, "db_stats");
    }

    #[test]
    fn db_stats_response_roundtrip() {
        let resp = DbStatsResponse {
            item_count: 42,
            size_bytes: 1024 * 512,
        };
        let s = serde_json::to_string(&resp).unwrap();
        let back: DbStatsResponse = serde_json::from_str(&s).unwrap();
        assert_eq!(resp, back);
        assert!(s.contains("\"item_count\":42"), "wire: {s}");
        assert!(s.contains("\"size_bytes\":"), "wire: {s}");
    }

    #[test]
    fn db_stats_response_default_is_zero() {
        let resp = DbStatsResponse::default();
        assert_eq!(resp.item_count, 0);
        assert_eq!(resp.size_bytes, 0);
    }

    #[test]
    fn vacuum_method_has_correct_wire_name() {
        assert_eq!(METHOD_VACUUM, "vacuum");
    }

    #[test]
    fn vacuum_request_defaults_all_false() {
        // An empty params object must parse with all flags false so a bare
        // `{"method":"vacuum","params":{}}` call runs the full VACUUM + REINDEX.
        let req: VacuumRequest = serde_json::from_str("{}").unwrap();
        assert!(!req.reindex_only);
        assert!(!req.dry_run);
    }

    #[test]
    fn vacuum_request_roundtrip() {
        let req = VacuumRequest {
            reindex_only: true,
            dry_run: false,
        };
        let s = serde_json::to_string(&req).unwrap();
        let back: VacuumRequest = serde_json::from_str(&s).unwrap();
        assert_eq!(req, back);
    }

    /// c4q2.22: VacuumResponse no longer has an `ok` field (removed as redundant —
    /// the outer response envelope's `ok` is the authoritative success indicator).
    #[test]
    fn vacuum_response_roundtrip() {
        let resp = VacuumResponse {
            size_before: 2048,
            size_after: 1024,
            reclaimed: 1024,
        };
        let s = serde_json::to_string(&resp).unwrap();
        let back: VacuumResponse = serde_json::from_str(&s).unwrap();
        assert_eq!(resp, back);
        assert!(
            !s.contains("\"ok\""),
            "c4q2.22: ok field must not appear in wire format: {s}"
        );
    }

    #[test]
    fn reset_request_defaults_confirm_false() {
        // An empty params object must deserialize with confirm = false so a
        // caller who forgets the flag is rejected rather than silently wiping.
        let req: ResetDatabaseRequest = serde_json::from_str("{}").unwrap();
        assert!(!req.confirm);
    }

    #[test]
    fn reset_request_roundtrip() {
        let req = ResetDatabaseRequest { confirm: true };
        let s = serde_json::to_string(&req).unwrap();
        let back: ResetDatabaseRequest = serde_json::from_str(&s).unwrap();
        assert_eq!(req, back);
        assert!(s.contains("\"confirm\":true"));
    }

    /// c4q2.22: ResetDatabaseResponse no longer has a `reset` field.
    #[test]
    fn reset_response_roundtrip() {
        let resp = ResetDatabaseResponse { ready: true };
        let s = serde_json::to_string(&resp).unwrap();
        let back: ResetDatabaseResponse = serde_json::from_str(&s).unwrap();
        assert_eq!(resp, back);
        assert!(
            !s.contains("\"reset\""),
            "c4q2.22: reset field must not appear in wire: {s}"
        );
    }

    // ── db_backup / db_restore (CopyPaste-x94p / CopyPaste-8wbt) ────────────

    #[test]
    fn db_backup_method_has_correct_wire_name() {
        assert_eq!(METHOD_DB_BACKUP, "db_backup");
    }

    #[test]
    fn db_restore_method_has_correct_wire_name() {
        assert_eq!(METHOD_DB_RESTORE, "db_restore");
    }

    #[test]
    fn db_backup_request_roundtrip() {
        let req = DbBackupRequest {
            dest_path: "/tmp/backup.db.enc".to_string(),
        };
        let s = serde_json::to_string(&req).unwrap();
        let back: DbBackupRequest = serde_json::from_str(&s).unwrap();
        assert_eq!(req, back);
        assert!(s.contains("dest_path"), "wire: {s}");
    }

    #[test]
    fn db_backup_response_roundtrip() {
        // c4q2.22: ok field removed from DbBackupResponse; success is conveyed
        // by the outer Response.ok envelope, not a redundant inner field.
        let resp = DbBackupResponse {
            dest_path: "/tmp/backup.db.enc".to_string(),
            size_bytes: 4096,
        };
        let s = serde_json::to_string(&resp).unwrap();
        let back: DbBackupResponse = serde_json::from_str(&s).unwrap();
        assert_eq!(resp, back);
        assert!(!s.contains("\"ok\""), "no redundant ok field on wire: {s}");
        assert!(s.contains("\"size_bytes\":4096"), "wire: {s}");
    }

    #[test]
    fn db_restore_request_defaults_confirm_false() {
        // An empty params object must parse with confirm = false so a caller who
        // forgets the flag is rejected rather than silently replacing the DB.
        let req: DbRestoreRequest =
            serde_json::from_str(r#"{"src_path": "/tmp/b.db.enc"}"#).unwrap();
        assert!(!req.confirm, "confirm must default to false");
        assert!(!req.force, "force must default to false");
    }

    #[test]
    fn db_restore_request_roundtrip() {
        let req = DbRestoreRequest {
            confirm: true,
            src_path: "/tmp/backup.db.enc".to_string(),
            force: false,
        };
        let s = serde_json::to_string(&req).unwrap();
        let back: DbRestoreRequest = serde_json::from_str(&s).unwrap();
        assert_eq!(req, back);
        assert!(s.contains("\"confirm\":true"), "wire: {s}");
        assert!(s.contains("src_path"), "wire: {s}");
    }

    #[test]
    fn db_restore_response_roundtrip() {
        // c4q2.22: ok field removed from DbRestoreResponse; success is conveyed
        // by the outer Response.ok envelope, not a redundant inner field.
        let resp = DbRestoreResponse { ready: true };
        let s = serde_json::to_string(&resp).unwrap();
        let back: DbRestoreResponse = serde_json::from_str(&s).unwrap();
        assert_eq!(resp, back);
        assert!(!s.contains("\"ok\""), "no redundant ok field on wire: {s}");
        assert!(s.contains("\"ready\":true"), "wire: {s}");
    }
}
