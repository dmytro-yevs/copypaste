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

use copypaste_ipc::{BackupData, ErrorCode, Response, ResponseData};
use rusqlite::Connection;
use tracing::{info, warn};

use super::messages::{
    MSG_BACKUP_EXISTS, MSG_BACKUP_FAILED, MSG_BACKUP_NO_DIR, MSG_BAD_PATH, MSG_LEGACY_DATABASE,
    MSG_NEEDS_CONFIRM, MSG_RESTORE_FAILED, MSG_RESTORE_NOT_A_BACKUP, MSG_RESTORE_NOT_FOUND,
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
const RESTORED_TABLES: &[&str] = &["clipboard_fts", "clipboard_items", "sync_device_name"];

/// Every table this build knows how to restore, including the ones it leaves in
/// place. A backup containing anything else is refused rather than partially
/// restored: a table added by a later build would otherwise be silently
/// dropped, and the user would not find out until they needed it.
const KNOWN_TABLES: &[&str] = &[
    "clipboard_items",
    "clipboard_fts",
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
    if !confirm {
        return Response::err(id, ErrorCode::InvalidRequest, MSG_NEEDS_CONFIRM);
    }
    let Some(src) = clean_path(src_path) else {
        return Response::err(id, ErrorCode::InvalidRequest, MSG_BAD_PATH);
    };
    if !src.is_file() {
        return Response::err(id, ErrorCode::NotFound, MSG_RESTORE_NOT_FOUND);
    }
    // Before the file is staged, not after: the validate path would meet a v0.4
    // history as `InvalidKey` and call it damaged, which is the one thing
    // CLAUDE.md rule 3 says a v2 build must not do to a user's own data. The
    // probe never applies a key and never writes, so the old file is as intact
    // after this as before — which is what makes the sentence true.
    if copypaste_core::is_v1_database(&src) {
        info!("a restore was asked for a CopyPaste 0.4 history; it was not opened or changed");
        return Response::err(id, ErrorCode::LegacyDatabase, MSG_LEGACY_DATABASE);
    }

    let key = state.keyring.db_key();
    let staging = staging_path(state.db_path());
    // A leftover from a previous crash is not a reason to refuse.
    remove_database(&staging);

    let outcome = (|| {
        // ---- Phase A: validate. Nothing live is touched. -------------------
        std::fs::copy(&src, &staging).map_err(|e| {
            warn!(error = %e, "could not stage the backup");
            MSG_RESTORE_FAILED
        })?;
        validate(&staging, &key)?;

        // ---- Phase B: swap, in one transaction. ----------------------------
        swap(state.db_path(), &staging, &key).map_err(|e| {
            warn!(error = ?e, "restore transaction failed; the database is unchanged");
            MSG_RESTORE_FAILED
        })
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

    let integrity: String = conn
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(|e| {
            warn!(error = %e, "integrity_check failed on the backup");
            MSG_RESTORE_NOT_A_BACKUP
        })?;
    if integrity != "ok" {
        warn!(result = %integrity, "the backup failed its integrity check");
        return Err(MSG_RESTORE_NOT_A_BACKUP);
    }

    // Schema sanity. `user_version` is `rusqlite_migration`'s marker, so a
    // backup from a newer build is refused here rather than half-read.
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
fn swap(db_path: &Path, staging: &Path, key: &[u8; 32]) -> Result<(), crate::meta::MetaError> {
    let mut conn = dbfile::open(db_path, key)?;
    // The raw-key form has to be literal in the clause — see
    // `dbfile::attach_key_literal` for why binding it would silently derive a
    // different key.
    let attach = format!(
        "ATTACH DATABASE ?1 AS restore_src KEY {}",
        dbfile::attach_key_literal(key).as_str()
    );
    conn.execute(&attach, [staging.to_string_lossy().as_ref()])?;

    let result = (|| -> rusqlite::Result<()> {
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
            "INSERT INTO clipboard_fts (id, content_text) \
             SELECT fts.id, fts.content_text FROM restore_src.clipboard_fts fts \
             JOIN restore_src.clipboard_items ci ON ci.id = fts.id \
             WHERE ci.deleted = 0 AND ci.is_sensitive = 0 \
               AND (ci.content_type = 'text' OR ci.content_type LIKE 'text/%')",
            [],
        )?;
        tx.execute(
            "INSERT INTO sync_device_name SELECT * FROM restore_src.sync_device_name",
            [],
        )?;
        tx.commit()
    })();

    // Detached whether or not the transaction committed; a rolled-back
    // transaction leaves the live database exactly as it was.
    let _ = conn.execute("DETACH DATABASE restore_src", []);
    result.map_err(Into::into)
}

/// User tables, excluding SQLite's own and the shadow tables an fts5 virtual
/// table creates.
fn user_tables(conn: &Connection) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT name FROM sqlite_master \
          WHERE type IN ('table', 'view') AND name NOT LIKE 'sqlite\\_%' ESCAPE '\\'",
    )?;
    let all: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<_>>()?;

    // fts5 owns `<name>_data`, `_idx`, `_content`, `_docsize`, `_config`. They
    // are the virtual table's storage, not tables of ours, and they are
    // rebuilt by writing through the virtual table.
    Ok(all
        .iter()
        .filter(|name| {
            !all.iter()
                .any(|other| other != *name && name.starts_with(&format!("{other}_")))
        })
        .cloned()
        .collect())
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

    /// The user's own history from an earlier version is neither a foreign
    /// backup nor a damaged one, and telling them it is damaged is telling them
    /// their data is gone when it is intact on disk (CLAUDE.md rule 3).
    #[test]
    fn restoring_a_v0_4_history_says_so_rather_than_reporting_damage() {
        let (state, dir) = test_state("alpha");
        add(&state, "the new history");

        // v0.4.x's plaintext schema, which is what a user who never upgraded
        // past the pre-encryption builds still has (manifest 03 §3.4).
        let old = dir.path().join("clipboard.db");
        let conn = Connection::open(&old).unwrap();
        conn.execute_batch(
            "CREATE TABLE clipboard_items (
                 id          TEXT PRIMARY KEY NOT NULL,
                 item_id     TEXT,
                 content     BLOB,
                 lamport_ts  INTEGER NOT NULL DEFAULT 0,
                 wall_time   INTEGER NOT NULL DEFAULT 0
             );",
        )
        .unwrap();
        conn.pragma_update(None, "user_version", 15).unwrap();
        drop(conn);
        let before = std::fs::metadata(&old).unwrap().len();

        let response = restore_from(&state, &old, true);
        assert!(!response.ok);
        assert_eq!(response.error_code, Some(ErrorCode::LegacyDatabase));
        assert_eq!(response.error.as_deref(), Some(MSG_LEGACY_DATABASE));
        assert!(!response.error_code.unwrap().retryable());

        // The two halves of the sentence, both of which have to stay true.
        assert_eq!(contents(&state), ["the new history"]);
        assert_eq!(std::fs::metadata(&old).unwrap().len(), before);
        for suffix in ["-wal", "-shm"] {
            let beside = dir.path().join(format!("clipboard.db{suffix}"));
            assert!(!beside.exists(), "the probe journalled the old history");
        }
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
