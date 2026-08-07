//! Backup and restore of the encrypted history database.
//!
//! # Restore is validate-then-swap, and the order is the whole point
//!
//! Restoring is the most destructive thing this product does. Phase A copies the
//! backup to a staging file beside the database, opens it **with this device's
//! real key**, runs `integrity_check`, and refuses anything whose schema version
//! or table set is not the one this build writes. Nothing has been touched at
//! that point, so a corrupt file, a file from another device, or a file that is
//! not a database at all leaves a working history exactly as it was
//! (`CopyPaste-8wbt`, `crh3.6`, `crh3.2`).
//!
//! Phase B is one SQLite transaction over an `ATTACH`ed staging database, not a
//! file swap. Manifest 04 §4.11 describes v1's Phase B as "quiesce, move the
//! live DB aside, copy in, reopen, and rebuild the r2d2 read pool", and the
//! manifest has been amended alongside this file to record the change. The
//! reason is inodes: the store's pool and `crate::meta` both hold open
//! connections to the live file, so renaming a new file over it leaves every
//! existing connection reading — and writing — an unlinked one. Doing it in a
//! transaction keeps every connection valid, makes rollback the database's job
//! rather than ours, and removes the `-wal`/`-shm` juggling a rename needs.
//!
//! # What a restore does not replace
//!
//! `sync_device_state` is deliberately left alone: it holds this device's
//! identity, its cloud session and its settings. A restore brings back history;
//! it must not make this device start claiming to be the one the backup came
//! from, and it must not silently change how the daemon behaves.
//!
//! # No path reaches a client
//!
//! Every failure here maps to a fixed sentence in [`super::messages`]. The
//! caller supplied the path and already knows it; putting it in a reply is how
//! it ends up in a log or a screenshot (CLAUDE.md rule 4).

use std::path::{Path, PathBuf};

use copypaste_core::{
    purge_indexed_secrets_in_transaction, verify_integrity, verify_schema, Detector, PurgeReport,
};
use copypaste_ipc::{BackupData, ErrorCode, Response, ResponseData};
use rusqlite::{Connection, Transaction};
use tracing::{info, warn};

use super::messages::{
    MSG_BACKUP_EXISTS, MSG_BACKUP_FAILED, MSG_BACKUP_NO_DIR, MSG_BAD_PATH, MSG_NEEDS_CONFIRM,
    MSG_RESTORE_FAILED, MSG_RESTORE_NOT_A_BACKUP, MSG_RESTORE_NOT_FOUND,
};
use crate::dbfile;
use crate::AppState;

/// Tables a restore replaces, in delete order (children first is not an issue
/// here — there are no foreign keys between them — but a stable order keeps the
/// transaction readable).
///
/// `sync_device_state` is absent on purpose; see the module header.
///
/// `sync_device_name` **is** restored, with the items, because the two are one
/// fact: `clipboard_items.origin_device_id` is a device id, and restoring the
/// id without the name it resolves to leaves every restored peer item labelled
/// with a bare UUID. Nothing keys off a name, so a stale row costs a label at
/// worst.
///
/// `clipboard_live_count` is absent because it is derived: its triggers count
/// the deletes and inserts below as they happen. Copying the backup's value on
/// top of that would apply the same rows twice.
const RESTORED_TABLES: &[&str] = &["clipboard_fts", "clipboard_items", "sync_device_name"];

/// Every table this build knows how to restore, including the ones it leaves in
/// place. A backup containing anything else is refused rather than partially
/// restored: a table added by a later build would otherwise be silently
/// dropped, and the user would not find out until they needed it.
const KNOWN_TABLES: &[&str] = &[
    "clipboard_items",
    "clipboard_fts",
    "clipboard_live_count",
    "sync_device_name",
    "sync_device_state",
];

pub(super) fn backup(state: &AppState, id: u64, dest_path: &str) -> Response {
    let Some(dest) = clean_path(dest_path) else {
        return Response::err(id, ErrorCode::InvalidRequest, MSG_BAD_PATH);
    };
    // Refusing to overwrite is not politeness: the obvious mistake is passing
    // the live database's own path, and `VACUUM INTO` onto it would be a wipe.
    if dest.exists() {
        return Response::err(id, ErrorCode::InvalidRequest, MSG_BACKUP_EXISTS);
    }
    if !dest.parent().is_some_and(Path::is_dir) {
        return Response::err(id, ErrorCode::InvalidRequest, MSG_BACKUP_NO_DIR);
    }

    let conn = match dbfile::open(state.db_path(), &state.keyring.db_key()) {
        Ok(conn) => conn,
        Err(e) => return failed(id, "open the database for backup", e, MSG_BACKUP_FAILED),
    };
    // `VACUUM INTO` rather than a file copy: it takes a consistent snapshot
    // through the pager while other connections are writing, and under
    // SQLCipher the copy is encrypted with the same key.
    if let Err(e) = conn.execute("VACUUM INTO ?1", [dest.to_string_lossy().as_ref()]) {
        // A partial file from a failed VACUUM would look like a backup.
        let _ = std::fs::remove_file(&dest);
        return failed(id, "VACUUM INTO", e, MSG_BACKUP_FAILED);
    }

    // The backup is ciphertext, but it is a whole history: owner-only, like the
    // database and the socket.
    if let Err(e) = set_owner_only(&dest) {
        warn!(error = %e, "could not restrict the backup to the owner");
    }

    let size_bytes = std::fs::metadata(&dest).map(|m| m.len()).unwrap_or(0);
    info!(size_bytes, "wrote a database backup");
    Response::ok(id, ResponseData::Backup(BackupData { size_bytes }))
}

pub(super) fn restore(state: &AppState, id: u64, src_path: &str, confirm: bool) -> Response {
    restore_with_purge(
        state,
        id,
        src_path,
        confirm,
        purge_indexed_secrets_in_transaction,
    )
}

type RestorePurge = for<'tx> fn(&Transaction<'tx>, &Detector) -> rusqlite::Result<PurgeReport>;

fn restore_with_purge(
    state: &AppState,
    id: u64,
    src_path: &str,
    confirm: bool,
    purge: RestorePurge,
) -> Response {
    if !confirm {
        return Response::err(id, ErrorCode::InvalidRequest, MSG_NEEDS_CONFIRM);
    }
    let Some(src) = clean_path(src_path) else {
        return Response::err(id, ErrorCode::InvalidRequest, MSG_BAD_PATH);
    };
    if !src.is_file() {
        return Response::err(id, ErrorCode::NotFound, MSG_RESTORE_NOT_FOUND);
    }
    let key = state.keyring.db_key();
    let staging = staging_path(state.db_path());
    // A leftover from a previous crash is not a reason to refuse.
    remove_database(&staging);

    let outcome: Result<(), &'static str> = (|| {
        // ---- Phase A: validate. Nothing live is touched. -------------------
        std::fs::copy(&src, &staging).map_err(|e| {
            warn!(error = %e, "could not stage the backup");
            MSG_RESTORE_FAILED
        })?;
        validate(&staging, &key)?;

        // ---- Phase B: swap, in one transaction. ----------------------------
        let report = swap(
            state.db_path(),
            &staging,
            &key,
            state.detector.as_ref(),
            purge,
        )
        .map_err(|e| {
            warn!(error = ?e, "restore transaction failed; the database is unchanged");
            MSG_RESTORE_FAILED
        })?;
        if report.purged > 0 {
            info!(
                purged = report.purged,
                scanned = report.scanned,
                "removed sensitive restored search entries"
            );
        }
        Ok(())
    })();

    remove_database(&staging);

    match outcome {
        Ok(()) => {
            // Every restored row is "written below the cloud cursor" as far as
            // the upload floor is concerned — their stamps are older than it —
            // so without this the restored history would never leave the device
            // again.
            if let Ok(Some(oldest)) = state.store.oldest_version_ms() {
                crate::cloud::note_version_written(state, oldest);
            }
            // The name table came from the backup, so this device's own row in
            // it is whatever it was called then. The *authority* on that name is
            // `sync_device_state`, which a restore leaves alone (see the module
            // header), so re-assert the row rather than leave the two
            // disagreeing about what this device is called.
            if let Err(e) = state
                .meta
                .record_device_name(state.meta.device_id(), state.meta.device_name())
            {
                warn!(error = ?e, "could not re-record this device's name after a restore");
            }
            state.note_local_change();
            info!("restored the history database from a backup");
            Response::ok(id, ResponseData::Empty {})
        }
        Err(message) => {
            let code = if message == MSG_RESTORE_NOT_A_BACKUP {
                ErrorCode::InvalidRequest
            } else {
                ErrorCode::Internal
            };
            Response::err(id, code, message)
        }
    }
}

/// Prove the candidate is a database this build wrote, with this device's key.
fn validate(staging: &Path, key: &[u8; 32]) -> Result<(), &'static str> {
    // `dbfile::open` applies the key and proves it opens the file; a wrong key
    // — a backup from another device, or a keychain that has been reset — comes
    // back as `InvalidKey` rather than as a fallback read.
    let conn = dbfile::open(staging, key).map_err(|e| {
        warn!(error = ?e, "the backup did not open with this device's key");
        MSG_RESTORE_NOT_A_BACKUP
    })?;

    verify_integrity(&conn).map_err(|e| {
        warn!(error = ?e, "integrity_check failed on the backup");
        MSG_RESTORE_NOT_A_BACKUP
    })?;

    verify_schema(&conn).map_err(|e| {
        warn!(error = ?e, "the backup schema does not match this build");
        MSG_RESTORE_NOT_A_BACKUP
    })?;

    let tables = user_tables(&conn).map_err(|_| MSG_RESTORE_NOT_A_BACKUP)?;
    for required in RESTORED_TABLES {
        if !tables.iter().any(|name| name == required) {
            warn!(missing = required, "the backup is missing a required table");
            return Err(MSG_RESTORE_NOT_A_BACKUP);
        }
    }
    if let Some(unknown) = tables
        .iter()
        .find(|name| !KNOWN_TABLES.contains(&name.as_str()))
    {
        warn!(table = %unknown, "the backup holds a table this build cannot restore");
        return Err(MSG_RESTORE_NOT_A_BACKUP);
    }
    Ok(())
}

/// Replace the restored tables' contents from `staging`, atomically.
fn swap(
    db_path: &Path,
    staging: &Path,
    key: &[u8; 32],
    detector: &Detector,
    purge: RestorePurge,
) -> Result<PurgeReport, crate::meta::MetaError> {
    let mut conn = dbfile::open(db_path, key)?;
    // The raw-key form has to be literal in the clause — see
    // `dbfile::attach_key_literal` for why binding it would silently derive a
    // different key.
    let attach = format!(
        "ATTACH DATABASE ?1 AS restore_src KEY {}",
        dbfile::attach_key_literal(key).as_str()
    );
    conn.execute(&attach, [staging.to_string_lossy().as_ref()])?;

    let result = (|| -> rusqlite::Result<PurgeReport> {
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        for table in RESTORED_TABLES {
            tx.execute(&format!("DELETE FROM {table}"), [])?;
        }
        tx.execute(
            "INSERT INTO clipboard_items SELECT * FROM restore_src.clipboard_items",
            [],
        )?;
        // Named columns, not `*`: an fts5 table's `*` includes the hidden rank
        // and table-name columns, which are not insertable.
        tx.execute(
            "INSERT INTO clipboard_fts (rowid, id, content_text) \
             SELECT ci.fts_rowid, fts.id, fts.content_text FROM restore_src.clipboard_fts fts \
             JOIN restore_src.clipboard_items ci ON ci.id = fts.id \
             WHERE ci.deleted = 0 AND ci.is_sensitive = 0 \
               AND (ci.content_type = 'text' OR ci.content_type LIKE 'text/%')",
            [],
        )?;
        tx.execute(
            "INSERT INTO sync_device_name SELECT * FROM restore_src.sync_device_name",
            [],
        )?;
        let report = purge(&tx, detector)?;
        tx.commit()?;
        Ok(report)
    })();

    // Detached whether or not the transaction committed; a rolled-back
    // transaction leaves the live database exactly as it was.
    let _ = conn.execute("DETACH DATABASE restore_src", []);
    result.map_err(Into::into)
}

/// User tables, excluding SQLite's own and the known fts5 shadow tables.
fn user_tables(conn: &Connection) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT name FROM sqlite_master \
          WHERE type IN ('table', 'view') AND name NOT LIKE 'sqlite\\_%' ESCAPE '\\'",
    )?;
    let all: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<_>>()?;

    // fts5 owns these five tables. An arbitrary table with the same prefix is
    // not a shadow table and must remain visible to the unknown-table check.
    Ok(all
        .iter()
        .filter(|name| !is_fts_shadow_table(name))
        .cloned()
        .collect())
}

fn is_fts_shadow_table(name: &str) -> bool {
    name.strip_prefix("clipboard_fts_")
        .is_some_and(|suffix| matches!(suffix, "data" | "idx" | "content" | "docsize" | "config"))
}

/// Where the candidate is staged: beside the database, so the copy and the
/// final transaction are on the same filesystem and the same permissions.
fn staging_path(db_path: &Path) -> PathBuf {
    let mut name = db_path.as_os_str().to_os_string();
    name.push(".restore-staging");
    PathBuf::from(name)
}

/// Remove a database and the two files SQLite keeps beside it.
fn remove_database(path: &Path) {
    for suffix in ["", "-wal", "-shm"] {
        let mut name = path.as_os_str().to_os_string();
        name.push(suffix);
        let _ = std::fs::remove_file(PathBuf::from(name));
    }
}

/// Reject a path that is empty or only whitespace before it reaches the
/// filesystem, where the failure would be an errno rather than a sentence.
fn clean_path(raw: &str) -> Option<PathBuf> {
    let trimmed = raw.trim();
    (!trimmed.is_empty()).then(|| PathBuf::from(trimmed))
}

#[cfg(unix)]
fn set_owner_only(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
}

/// **Unverified off unix.** The backup inherits the destination directory's
/// ACL, which is not the same guarantee; restricting it needs a DACL rather
/// than a mode, and that is a decision, not a cfg.
#[cfg(not(unix))]
fn set_owner_only(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

fn failed(
    id: u64,
    operation: &'static str,
    error: impl std::fmt::Display,
    message: &'static str,
) -> Response {
    warn!(operation, error = %error, "database administration failed");
    Response::err(id, ErrorCode::Internal, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{add, contents, test_state};
    use copypaste_ipc::Method;

    const RESTORED_SECRET: &str = "AKIAIOSFODNN7EXAMPLE";

    fn call(state: &AppState, method: Method) -> Response {
        crate::server::dispatch::dispatch_store(state, 1, method)
    }

    fn backup_to(state: &AppState, dest: &Path) -> Response {
        call(
            state,
            Method::Backup {
                dest_path: dest.to_string_lossy().into_owned(),
            },
        )
    }

    fn restore_from(state: &AppState, src: &Path, confirm: bool) -> Response {
        call(
            state,
            Method::Restore {
                src_path: src.to_string_lossy().into_owned(),
                confirm,
            },
        )
    }

    fn replace_index_text(state: &AppState, id: &str, text: &str) {
        let conn = dbfile::open(state.db_path(), &state.keyring.db_key()).unwrap();
        conn.execute("DELETE FROM clipboard_fts WHERE id = ?1", [id])
            .unwrap();
        conn.execute(
            "INSERT INTO clipboard_fts (id, content_text) VALUES (?1, ?2)",
            rusqlite::params![id, text],
        )
        .unwrap();
    }

    fn fts_rows(state: &AppState, id: &str) -> i64 {
        let conn = dbfile::open(state.db_path(), &state.keyring.db_key()).unwrap();
        conn.query_row(
            "SELECT COUNT(*) FROM clipboard_fts WHERE id = ?1",
            [id],
            |row| row.get(0),
        )
        .unwrap()
    }

    fn dangling_fts_pointers(state: &AppState) -> i64 {
        let conn = dbfile::open(state.db_path(), &state.keyring.db_key()).unwrap();
        conn.query_row(
            "SELECT COUNT(*) FROM clipboard_items ci \
              WHERE ci.fts_rowid IS NOT NULL \
                AND ci.id IS NOT (SELECT f.id FROM clipboard_fts f WHERE f.rowid = ci.fts_rowid)",
            [],
            |row| row.get(0),
        )
        .unwrap()
    }

    fn deleted(state: &AppState, id: &str) -> bool {
        let conn = dbfile::open(state.db_path(), &state.keyring.db_key()).unwrap();
        conn.query_row(
            "SELECT deleted FROM clipboard_items WHERE id = ?1",
            [id],
            |row| row.get(0),
        )
        .unwrap()
    }

    fn fail_after_partial_restore_purge(
        tx: &Transaction<'_>,
        _detector: &Detector,
    ) -> rusqlite::Result<PurgeReport> {
        let restored: i64 = tx.query_row(
            "SELECT COUNT(*) FROM clipboard_fts WHERE content_text = ?1",
            [RESTORED_SECRET],
            |row| row.get(0),
        )?;
        assert_eq!(restored, 1, "the failure must be injected after the swap");
        assert_eq!(
            tx.execute(
                "DELETE FROM clipboard_fts WHERE content_text = ?1",
                [RESTORED_SECRET],
            )?,
            1,
            "the injected purge must partially mutate the transaction"
        );
        Err(rusqlite::Error::InvalidQuery)
    }

    #[test]
    fn a_backup_round_trips_and_is_owner_only() {
        let (state, dir) = test_state("alpha");
        add(&state, "before the backup");
        let dest = dir.path().join("history.backup");

        let response = backup_to(&state, &dest);
        assert!(response.ok, "{:?}", response.error);
        match response.data {
            Some(ResponseData::Backup(data)) => assert!(data.size_bytes > 0),
            other => panic!("{other:?}"),
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&dest).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600, "a backup is a whole history");
        }

        // The history moves on, and the restore puts it back.
        add(&state, "after the backup");
        assert_eq!(contents(&state).len(), 2);

        assert!(restore_from(&state, &dest, true).ok);
        assert_eq!(contents(&state), ["before the backup"]);
    }

    #[test]
    fn a_backup_never_overwrites_and_needs_a_directory_that_exists() {
        let (state, dir) = test_state("alpha");
        let dest = dir.path().join("history.backup");
        assert!(backup_to(&state, &dest).ok);

        let second = backup_to(&state, &dest);
        assert_eq!(second.error_code, Some(ErrorCode::InvalidRequest));

        let missing = backup_to(&state, &dir.path().join("nope").join("x.backup"));
        assert_eq!(missing.error_code, Some(ErrorCode::InvalidRequest));

        // The live database's own path is the mistake that matters most.
        let onto_itself = backup_to(&state, state.db_path());
        assert_eq!(onto_itself.error_code, Some(ErrorCode::InvalidRequest));
        assert!(state.store.count().is_ok(), "the database was damaged");
    }

    /// The property this module exists for: a bad backup cannot replace a
    /// working database.
    #[test]
    fn a_corrupt_backup_is_refused_and_history_survives() {
        let (state, dir) = test_state("alpha");
        add(&state, "still here");

        let junk = dir.path().join("junk.backup");
        std::fs::write(&junk, b"this is not a database, not even close").unwrap();

        let response = restore_from(&state, &junk, true);
        assert!(!response.ok);
        assert_eq!(response.error_code, Some(ErrorCode::InvalidRequest));
        assert_eq!(contents(&state), ["still here"]);
    }

    #[test]
    fn a_backup_with_a_different_schema_version_is_refused() {
        let (state, dir) = test_state("alpha");
        add(&state, "still here");
        let backup = dir.path().join("wrong-version.backup");
        assert!(backup_to(&state, &backup).ok);

        let conn = dbfile::open(&backup, &state.keyring.db_key()).unwrap();
        conn.pragma_update(None, "user_version", 2).unwrap();
        drop(conn);

        let response = restore_from(&state, &backup, true);
        assert_eq!(response.error_code, Some(ErrorCode::InvalidRequest));
        assert_eq!(contents(&state), ["still here"]);
    }

    #[test]
    fn a_backup_with_a_column_type_mismatch_is_refused() {
        let (state, dir) = test_state("alpha");
        add(&state, "still here");
        let backup = dir.path().join("wrong-column-type.backup");
        assert!(backup_to(&state, &backup).ok);

        let conn = dbfile::open(&backup, &state.keyring.db_key()).unwrap();
        conn.execute_batch(
            "DROP TABLE sync_device_name;
             CREATE TABLE sync_device_name (
                 device_id BLOB PRIMARY KEY NOT NULL,
                 name TEXT NOT NULL
             );",
        )
        .unwrap();
        drop(conn);

        let response = restore_from(&state, &backup, true);
        assert_eq!(response.error_code, Some(ErrorCode::InvalidRequest));
        assert_eq!(contents(&state), ["still here"]);
    }

    #[test]
    fn a_backup_with_an_extra_shadow_looking_table_is_refused() {
        let (state, dir) = test_state("alpha");
        add(&state, "still here");
        let backup = dir.path().join("unknown-table.backup");
        assert!(backup_to(&state, &backup).ok);

        let conn = dbfile::open(&backup, &state.keyring.db_key()).unwrap();
        conn.execute("CREATE TABLE clipboard_fts_unrecognised (value TEXT)", [])
            .unwrap();
        drop(conn);

        let response = restore_from(&state, &backup, true);
        assert_eq!(response.error_code, Some(ErrorCode::InvalidRequest));
        assert_eq!(contents(&state), ["still here"]);
    }

    /// A backup taken on another device is encrypted under another key. It must
    /// fail authentication, never be half-read (CLAUDE.md rule 4).
    #[test]
    fn a_backup_from_another_device_is_refused_and_history_survives() {
        let (theirs, their_dir) = test_state("beta");
        add(&theirs, "someone else's clipboard");
        let foreign = their_dir.path().join("theirs.backup");
        assert!(backup_to(&theirs, &foreign).ok);

        let (mine, _dir) = test_state("alpha");
        add(&mine, "mine");
        let response = restore_from(&mine, &foreign, true);
        assert!(!response.ok);
        assert_eq!(contents(&mine), ["mine"]);
    }

    #[test]
    fn a_restore_requires_confirmation_and_an_existing_file() {
        let (state, dir) = test_state("alpha");
        add(&state, "untouched");
        let dest = dir.path().join("history.backup");
        assert!(backup_to(&state, &dest).ok);

        let unconfirmed = restore_from(&state, &dest, false);
        assert_eq!(unconfirmed.error_code, Some(ErrorCode::InvalidRequest));

        let absent = restore_from(&state, &dir.path().join("absent.backup"), true);
        assert_eq!(absent.error_code, Some(ErrorCode::NotFound));

        let empty = restore_from(&state, Path::new("   "), true);
        assert_eq!(empty.error_code, Some(ErrorCode::InvalidRequest));

        assert_eq!(contents(&state), ["untouched"]);
    }

    /// This device's identity, its cloud session and its settings are properties
    /// of *this* device, not of the backup.
    #[test]
    fn a_restore_keeps_this_devices_identity_and_settings() {
        let (state, dir) = test_state("alpha");
        state
            .settings
            .apply(
                &state.meta,
                &copypaste_ipc::ConfigPatch {
                    poll_interval_ms: Some(250),
                    ..Default::default()
                },
            )
            .unwrap();
        let device_id = state.meta.device_id().to_string();
        let dest = dir.path().join("history.backup");
        assert!(backup_to(&state, &dest).ok);

        add(&state, "later");
        assert!(restore_from(&state, &dest, true).ok);

        assert_eq!(state.meta.device_id(), device_id);
        assert_eq!(state.settings.get().poll_interval_ms, 250);
    }

    /// The search index has to come back with the items, or a restored history
    /// is unsearchable until every row is re-copied.
    #[test]
    fn the_search_index_survives_a_restore() {
        let (state, dir) = test_state("alpha");
        add(&state, "findable haystack needle");
        let dest = dir.path().join("history.backup");
        assert!(backup_to(&state, &dest).ok);

        state.store.delete_all().unwrap();
        assert!(state.store.search("needle", 10).unwrap().is_empty());

        assert!(restore_from(&state, &dest, true).ok);
        assert_eq!(state.store.search("needle", 10).unwrap().len(), 1);
        assert_eq!(
            dangling_fts_pointers(&state),
            0,
            "a restored fts_rowid names another item's index row"
        );
    }

    #[test]
    fn a_restore_carries_the_index_rowids_the_items_point_at() {
        let (state, dir) = test_state("alpha");
        let a = add(&state, "alpha aardvark");
        let b = add(&state, "bravo baboon");
        let c = add(&state, "charlie cheetah");
        let d = add(&state, "delta dingo");

        assert!(state.store.delete(&b).unwrap());
        let dest = dir.path().join("history.backup");
        assert!(backup_to(&state, &dest).ok);

        state.store.delete_all().unwrap();
        assert!(restore_from(&state, &dest, true).ok);
        assert_eq!(
            dangling_fts_pointers(&state),
            0,
            "the restore re-keyed the index without moving the back-pointers"
        );
        assert_eq!(fts_rows(&state, &a), 1);
        assert_eq!(fts_rows(&state, &c), 1);
        assert_eq!(fts_rows(&state, &d), 1);

        assert!(state.store.delete(&c).unwrap());
        assert_eq!(
            fts_rows(&state, &c),
            0,
            "the deleted item's plaintext is still in the search index"
        );
        assert_eq!(
            fts_rows(&state, &d),
            1,
            "deleting one item unindexed another"
        );
        assert!(state.store.search("cheetah", 10).unwrap().is_empty());
        assert_eq!(state.store.search("dingo", 10).unwrap().len(), 1);
    }

    #[test]
    fn a_restore_commits_only_after_the_current_detector_purges_fts() {
        let (state, dir) = test_state("alpha");
        let leaked = add(&state, "kept history row");
        replace_index_text(&state, &leaked, RESTORED_SECRET);
        add(&state, "ordinary restored search marker");
        let backup = dir.path().join("history.backup");
        assert!(backup_to(&state, &backup).ok);

        state.store.delete_all().unwrap();
        let response = restore_from(&state, &backup, true);

        assert!(response.ok, "{:?}", response.error);
        assert!(!deleted(&state, &leaked), "the restored row was committed");
        assert!(state.store.get(&leaked).unwrap().is_some());
        assert_eq!(fts_rows(&state, &leaked), 0);
        assert!(state.store.search(RESTORED_SECRET, 10).unwrap().is_empty());
        assert_eq!(
            state
                .store
                .search("ordinary restored search marker", 10)
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn a_post_swap_purge_failure_rolls_back_and_a_retry_is_safe() {
        let (state, dir) = test_state("alpha");
        let leaked = add(&state, "restored history row");
        replace_index_text(&state, &leaked, RESTORED_SECRET);
        add(&state, "ordinary restored search marker");
        let backup = dir.path().join("history.backup");
        assert!(backup_to(&state, &backup).ok);

        state.store.delete_all().unwrap();
        add(&state, "live rollback anchor");
        assert!(deleted(&state, &leaked));

        let failed = restore_with_purge(
            &state,
            1,
            backup.to_string_lossy().as_ref(),
            true,
            fail_after_partial_restore_purge,
        );

        assert!(!failed.ok);
        assert_eq!(failed.error_code, Some(ErrorCode::Internal));
        assert_eq!(failed.error.as_deref(), Some(MSG_RESTORE_FAILED));
        assert_eq!(contents(&state), ["live rollback anchor"]);
        assert!(deleted(&state, &leaked), "the restored row must roll back");
        assert_eq!(fts_rows(&state, &leaked), 0);
        assert_eq!(state.store.search("rollback anchor", 10).unwrap().len(), 1);
        assert!(state.store.search(RESTORED_SECRET, 10).unwrap().is_empty());

        let retry = restore_from(&state, &backup, true);
        assert!(retry.ok, "{:?}", retry.error);
        assert!(!deleted(&state, &leaked), "the retry must commit the row");
        assert_eq!(fts_rows(&state, &leaked), 0);
        assert!(state.store.search(RESTORED_SECRET, 10).unwrap().is_empty());
        assert!(state
            .store
            .search("rollback anchor", 10)
            .unwrap()
            .is_empty());
        assert_eq!(
            state
                .store
                .search("ordinary restored search marker", 10)
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn a_restore_drops_polluted_non_text_fts_rows() {
        let (state, dir) = test_state("alpha");
        let image = crate::capture::ingest(&state, "opaque image bytes", "image/png")
            .unwrap()
            .into_item();
        let conn = dbfile::open(state.db_path(), &state.keyring.db_key()).unwrap();
        conn.execute(
            "INSERT INTO clipboard_fts (id, content_text) VALUES (?1, ?2)",
            rusqlite::params![&image.id, "private image caption"],
        )
        .unwrap();
        let dest = dir.path().join("history.backup");
        assert!(backup_to(&state, &dest).ok);

        state.store.delete_all().unwrap();
        assert!(restore_from(&state, &dest, true).ok);
        assert!(state.store.search("caption", 10).unwrap().is_empty());

        let conn = dbfile::open(state.db_path(), &state.keyring.db_key()).unwrap();
        let fts_rows: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM clipboard_fts WHERE id = ?1",
                [&image.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(fts_rows, 0);
    }

    #[test]
    fn fts_shadow_tables_are_not_mistaken_for_tables_of_ours() {
        let (state, _dir) = test_state("alpha");
        let conn = dbfile::open(state.db_path(), &state.keyring.db_key()).unwrap();
        let mut tables = user_tables(&conn).unwrap();
        tables.sort();
        let mut expected: Vec<String> = KNOWN_TABLES.iter().map(|s| (*s).to_string()).collect();
        expected.sort();
        assert_eq!(tables, expected);
    }
}
