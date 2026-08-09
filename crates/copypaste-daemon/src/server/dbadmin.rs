//! IPC adaptation for the core encrypted backup and atomic restore API.
//!
//! Restore leaves `sync_device_state` untouched, so this device keeps its
//! identity, cloud session and settings. Every failure maps to a fixed sentence
//! because a client-visible path discloses the local username.

use std::path::{Path, PathBuf};

use copypaste_core::RestoreError;
use copypaste_ipc::{BackupData, ErrorCode, Response, ResponseData};
use tracing::{info, warn};

#[cfg(test)]
use copypaste_core::storage::open_validated;

use super::messages::{
    MSG_BACKUP_EXISTS, MSG_BACKUP_FAILED, MSG_BACKUP_NO_DIR, MSG_BAD_PATH, MSG_NEEDS_CONFIRM,
    MSG_RESTORE_FAILED, MSG_RESTORE_NOT_A_BACKUP, MSG_RESTORE_NOT_FOUND,
};
use crate::AppState;

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

    if let Err(e) = state.store.backup_to(&dest) {
        // A partial file from a failed VACUUM would look like a backup.
        let _ = std::fs::remove_file(&dest);
        return failed(id, "write the backup", e, MSG_BACKUP_FAILED);
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
    match state
        .store
        .restore_from(&src, &state.keyring.db_key(), &state.detector)
    {
        Ok(report) => {
            if report.purged > 0 {
                info!(
                    purged = report.purged,
                    scanned = report.scanned,
                    "removed sensitive restored search entries"
                );
            }
            // Every restored row is "written below the cloud cursor" as far as
            // the upload floor is concerned — their stamps are older than it —
            // so without this the restored history would never leave the device
            // again.
            if let Ok(Some(oldest)) = state.store.oldest_version_ms() {
                crate::cloud::note_version_written(state, oldest);
                state.p2p.node().note_local_version(oldest);
            }
            state.note_local_change();
            info!("restored the history database from a backup");
            Response::ok(id, ResponseData::Empty {})
        }
        Err(RestoreError::InvalidBackup(error)) => {
            warn!(error = ?error, "the backup failed validation");
            Response::err(id, ErrorCode::InvalidRequest, MSG_RESTORE_NOT_A_BACKUP)
        }
        Err(RestoreError::Failed(error)) => {
            warn!(error = ?error, "restore failed; the database is unchanged");
            Response::err(id, ErrorCode::Internal, MSG_RESTORE_FAILED)
        }
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
        let conn = open_validated(state.db_path(), &state.keyring.db_key()).unwrap();
        conn.execute("DELETE FROM clipboard_fts WHERE id = ?1", [id])
            .unwrap();
        conn.execute(
            "INSERT INTO clipboard_fts (id, content_text) VALUES (?1, ?2)",
            rusqlite::params![id, text],
        )
        .unwrap();
    }

    fn fts_rows(state: &AppState, id: &str) -> i64 {
        let conn = open_validated(state.db_path(), &state.keyring.db_key()).unwrap();
        conn.query_row(
            "SELECT COUNT(*) FROM clipboard_fts WHERE id = ?1",
            [id],
            |row| row.get(0),
        )
        .unwrap()
    }

    fn dangling_fts_pointers(state: &AppState) -> i64 {
        let conn = open_validated(state.db_path(), &state.keyring.db_key()).unwrap();
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
        let conn = open_validated(state.db_path(), &state.keyring.db_key()).unwrap();
        conn.query_row(
            "SELECT deleted FROM clipboard_items WHERE id = ?1",
            [id],
            |row| row.get(0),
        )
        .unwrap()
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

        let conn = open_validated(&backup, &state.keyring.db_key()).unwrap();
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

        let conn = open_validated(&backup, &state.keyring.db_key()).unwrap();
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

        let conn = open_validated(&backup, &state.keyring.db_key()).unwrap();
        conn.execute("CREATE TABLE clipboard_fts_unrecognised (value TEXT)", [])
            .unwrap();
        drop(conn);

        let response = restore_from(&state, &backup, true);
        assert_eq!(response.error_code, Some(ErrorCode::InvalidRequest));
        assert_eq!(contents(&state), ["still here"]);
    }

    /// A backup taken on another device is encrypted under another key. It must
    /// fail authentication, never be half-read (AGENTS.md rule 4).
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

        state.meta.set_device_name("Current alpha").unwrap();
        add(&state, "later");
        assert!(restore_from(&state, &dest, true).ok);

        assert_eq!(state.meta.device_id(), device_id);
        assert_eq!(state.meta.device_name(), "Current alpha");
        assert_eq!(
            state
                .store
                .device_names(&[device_id])
                .unwrap()
                .values()
                .next(),
            Some(&"Current alpha".to_string())
        );
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
    fn a_restore_drops_polluted_non_text_fts_rows() {
        let (state, dir) = test_state("alpha");
        let image = crate::capture::ingest(&state, "opaque image bytes", "image/png")
            .unwrap()
            .into_item();
        let conn = open_validated(state.db_path(), &state.keyring.db_key()).unwrap();
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

        let conn = open_validated(state.db_path(), &state.keyring.db_key()).unwrap();
        let fts_rows: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM clipboard_fts WHERE id = ?1",
                [&image.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(fts_rows, 0);
    }
}
