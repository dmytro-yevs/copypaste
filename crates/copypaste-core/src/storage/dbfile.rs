//! Opening the history file on a private connection, outside the pool.
//!
//! The pool ([`super::connection`]) is how the store itself reads and writes.
//! This is for the two jobs a pooled handle cannot do: hold a second connection
//! for columns [`super::StoredItem`] does not project, and prove a *candidate*
//! file — a backup being restored — before anything live is touched.
//!
//! It exists so there is one `PRAGMA key` in the tree. A caller that opens its
//! own keyed connection is writing the second one, and the raw-key form has two
//! ways to be silently wrong: it must be the first statement on the connection,
//! and it must be literal in the SQL rather than bound (see
//! [`attach_key_literal`]).

use std::io::Write;
use std::path::Path;

use rusqlite::{Connection, OpenFlags};
use zeroize::Zeroizing;

use super::connection::{apply_connection_pragmas, apply_key, validate_key};
use super::model::{stored_item_columns, StoreError};

// The live counter is derived by the target database's item triggers.
const RESTORED_TABLES: &[&str] = &["clipboard_fts", "clipboard_items", "sync_device_name"];

#[derive(Debug, thiserror::Error)]
pub enum RestoreError {
    #[error("the backup is not valid for this device")]
    InvalidBackup(#[source] StoreError),

    #[error("the restore could not be completed")]
    Failed(#[source] StoreError),
}

/// Open `path` with `db_key` and prove the key and schema before returning.
///
/// Fails closed: a key that does not open the file is [`StoreError::InvalidKey`],
/// never a fallback to an unkeyed read (AGENTS.md rule 4).
///
pub fn open_validated(path: &Path, db_key: &[u8; 32]) -> Result<Connection, StoreError> {
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    apply_key(&conn, db_key)?;
    validate_key(&conn)?;
    verify_schema(&conn)?;
    apply_connection_pragmas(&conn)?;
    Ok(conn)
}

/// Run SQLite's `integrity_check` and turn anything but `ok` into an error.
///
/// Separate from [`open_validated`] because it reads the whole file: worth it
/// before replacing a working history with a backup, and not worth it on every
/// daemon start.
pub fn verify_integrity(conn: &Connection) -> Result<(), StoreError> {
    let result: String = conn.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
    if result == "ok" {
        Ok(())
    } else {
        tracing::warn!(result = %result, "a database failed its integrity check");
        Err(StoreError::IntegrityCheckFailed)
    }
}

/// Refuse a candidate whose version, tables, columns, or declared types do
/// not exactly match the schema this build writes.
pub fn verify_schema(conn: &Connection) -> Result<(), StoreError> {
    super::schema_verify::verify_schema(conn)
}

/// The raw key rendered for an `ATTACH … KEY` clause.
///
/// Inline in the SQL rather than bound as a parameter: SQLCipher recognises the
/// raw-key `x'…'` form only in the literal text of the clause, and a bound
/// value is taken as a passphrase and run through PBKDF2 instead — which
/// produces a different key and a file that will not open.
#[must_use]
pub fn attach_key_literal(db_key: &[u8; 32]) -> Zeroizing<String> {
    let key_hex = Zeroizing::new(hex::encode(db_key));
    Zeroizing::new(format!("\"x'{}'\"", key_hex.as_str()))
}

impl super::Store {
    /// Write a consistent encrypted snapshot of the database to `dest`.
    ///
    /// `VACUUM INTO` rather than a file copy: it takes the snapshot through the
    /// pager while other connections are writing, and under SQLCipher the copy
    /// carries the same key — so a backup is as unreadable without the device
    /// secret as the original.
    ///
    /// SQLite refuses to write onto an existing file, which is the guard that
    /// matters: the obvious mistake is naming the live database, and a copy
    /// onto it would be a wipe. A caller that wants a nicer message should
    /// check first; this cannot destroy data either way.
    pub fn backup_to(&self, dest: &Path) -> Result<(), StoreError> {
        let conn = self.conn()?;
        conn.execute("VACUUM INTO ?1", [dest.to_string_lossy().as_ref()])?;
        Ok(())
    }

    /// Stage and validate a same-device encrypted backup, then atomically replace history.
    pub fn restore_from(
        &self,
        src: &Path,
        db_key: &[u8; 32],
        detector: &crate::Detector,
    ) -> Result<crate::PurgeReport, RestoreError> {
        let staged = self
            .stage_restore_candidate(src)
            .map_err(RestoreError::Failed)?;
        let candidate =
            open_validated(staged.path(), db_key).map_err(RestoreError::InvalidBackup)?;
        verify_integrity(&candidate).map_err(RestoreError::InvalidBackup)?;
        verify_schema(&candidate).map_err(RestoreError::InvalidBackup)?;
        let tables = user_tables(&candidate)
            .map_err(StoreError::from)
            .map_err(RestoreError::InvalidBackup)?;
        if RESTORED_TABLES
            .iter()
            .any(|required| !tables.iter().any(|table| table == required))
            || tables
                .iter()
                .any(|table| !super::schema_verify::is_current_table(table))
        {
            return Err(RestoreError::InvalidBackup(StoreError::InvalidSchema));
        }
        drop(candidate);

        let mut conn = self.conn().map_err(RestoreError::Failed)?;
        let attach = format!(
            "ATTACH DATABASE ?1 AS restore_src KEY {}",
            attach_key_literal(db_key).as_str()
        );
        conn.execute(&attach, [staged.path().to_string_lossy().as_ref()])
            .map_err(StoreError::from)
            .map_err(RestoreError::Failed)?;
        let result = (|| -> rusqlite::Result<crate::PurgeReport> {
            let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
            for table in RESTORED_TABLES {
                tx.execute(&format!("DELETE FROM {table}"), [])?;
            }
            // `content_bytes` is recomputed from the payload that actually
            // arrives rather than copied, so a source file whose column had
            // drifted cannot import a byte quota that disagrees with its rows.
            tx.execute(
                concat!(
                    "INSERT INTO clipboard_items (",
                    stored_item_columns!(),
                    ", content_bytes) SELECT ",
                    stored_item_columns!(),
                    ", LENGTH(COALESCE(content_ciphertext, X'')) \
                     FROM restore_src.clipboard_items"
                ),
                [],
            )?;
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
            tx.execute(
                "INSERT INTO sync_device_name (device_id, name) \
                 SELECT id.value, name.value \
                 FROM sync_device_state id, sync_device_state name \
                 WHERE id.key = 'device_id' AND name.key = 'device_name' \
                 ON CONFLICT(device_id) DO UPDATE SET name = excluded.name",
                [],
            )?;
            let report = crate::purge_indexed_secrets_in_transaction(&tx, detector)?;
            tx.commit()?;
            Ok(report)
        })();
        let _ = conn.execute("DETACH DATABASE restore_src", []);
        result
            .map_err(StoreError::from)
            .map_err(RestoreError::Failed)
    }

    fn stage_restore_candidate(&self, src: &Path) -> Result<tempfile::NamedTempFile, StoreError> {
        let mut staged = match self.path.as_deref().and_then(|path| path.parent()) {
            Some(parent) => tempfile::Builder::new()
                .prefix(".copypaste-restore-")
                .tempfile_in(parent)?,
            None => tempfile::NamedTempFile::new()?,
        };
        let mut source = std::fs::File::open(src)?;
        std::io::copy(&mut source, staged.as_file_mut())?;
        staged.as_file_mut().flush()?;
        Ok(staged)
    }
}

fn user_tables(conn: &Connection) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT name FROM sqlite_master WHERE type IN ('table', 'view') \
         AND name NOT LIKE 'sqlite\\_%' ESCAPE '\\'",
    )?;
    let all = stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(all
        .into_iter()
        .filter(|name| {
            !name.strip_prefix("clipboard_fts_").is_some_and(|suffix| {
                matches!(suffix, "data" | "idx" | "content" | "docsize" | "config")
            })
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::test_support::{item, KEY, OTHER_KEY, T0};
    use crate::storage::Store;

    fn file_store(dir: &tempfile::TempDir) -> (Store, std::path::PathBuf) {
        let path = dir.path().join("copypaste-v2.db");
        let store = Store::open(&path, &KEY).unwrap();
        (store, path)
    }

    #[test]
    fn a_wrong_key_fails_closed_rather_than_reading() {
        let dir = tempfile::tempdir().unwrap();
        let (_store, path) = file_store(&dir);

        assert!(open_validated(&path, &KEY).is_ok());
        let err =
            open_validated(&path, &OTHER_KEY).expect_err("a wrong key must not open the database");
        assert!(matches!(err, StoreError::InvalidKey), "{err:?}");
    }

    #[test]
    fn a_file_that_is_not_a_database_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("junk.backup");
        std::fs::write(&path, b"this is not a database, not even close").unwrap();
        assert!(matches!(
            open_validated(&path, &KEY),
            Err(StoreError::InvalidKey)
        ));
    }

    #[test]
    fn a_backup_round_trips_under_the_same_key_and_not_another() {
        let dir = tempfile::tempdir().unwrap();
        let (store, _path) = file_store(&dir);
        store.insert(item("worth keeping", T0)).unwrap();

        let dest = dir.path().join("history.backup");
        store.backup_to(&dest).unwrap();
        assert!(dest.is_file());

        let restored = Store::open(&dest, &KEY).unwrap();
        assert_eq!(restored.count().unwrap(), 1);
        assert!(matches!(
            open_validated(&dest, &OTHER_KEY),
            Err(StoreError::InvalidKey)
        ));
    }

    /// The mistake that matters: naming the live database as the destination.
    #[test]
    fn a_backup_will_not_overwrite_an_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let (store, path) = file_store(&dir);
        store.insert(item("still here", T0)).unwrap();

        assert!(store.backup_to(&path).is_err());
        assert_eq!(store.count().unwrap(), 1, "the database was damaged");
    }

    #[test]
    fn a_healthy_database_passes_its_integrity_check() {
        let dir = tempfile::tempdir().unwrap();
        let (store, path) = file_store(&dir);
        store.insert(item("intact", T0)).unwrap();
        drop(store);

        let conn = open_validated(&path, &KEY).unwrap();
        assert!(verify_integrity(&conn).is_ok());
    }

    /// The literal form is what SQLCipher recognises; a bound parameter would
    /// be run through PBKDF2 and derive a different key.
    #[test]
    fn the_attach_literal_is_the_raw_key_form() {
        let literal = attach_key_literal(&[0xab; 32]);
        assert_eq!(literal.as_str(), format!("\"x'{}'\"", "ab".repeat(32)));
    }

    /// An `ATTACH` with that literal opens a database written under the same
    /// key — which is the whole reason the helper exists.
    #[test]
    fn a_database_attaches_with_the_literal_key() {
        let dir = tempfile::tempdir().unwrap();
        let (store, path) = file_store(&dir);
        store.insert(item("attached", T0)).unwrap();
        let other = dir.path().join("second.db");
        store.backup_to(&other).unwrap();

        let conn = open_validated(&path, &KEY).unwrap();
        let attach = format!(
            "ATTACH DATABASE ?1 AS src KEY {}",
            attach_key_literal(&KEY).as_str()
        );
        conn.execute(&attach, [other.to_string_lossy().as_ref()])
            .unwrap();
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM src.clipboard_items", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1);
        conn.execute("DETACH DATABASE src", []).unwrap();
    }

    #[test]
    fn restore_validates_before_replacing_live_history() {
        let dir = tempfile::tempdir().unwrap();
        let (store, _path) = file_store(&dir);
        store.insert(item("backup row", T0)).unwrap();
        let backup = dir.path().join("history.backup");
        store.backup_to(&backup).unwrap();
        store.insert(item("later row", T0 + 1)).unwrap();

        store
            .restore_from(&backup, &KEY, &crate::Detector::new().unwrap())
            .unwrap();
        assert_eq!(store.count().unwrap(), 1);

        drop(store);
        let store = Store::open(&dir.path().join("copypaste-v2.db"), &KEY).unwrap();
        assert_eq!(store.count().unwrap(), 1, "restore did not survive reopen");

        let junk = dir.path().join("junk.backup");
        std::fs::write(&junk, b"not a database").unwrap();
        assert!(store
            .restore_from(&junk, &KEY, &crate::Detector::new().unwrap())
            .is_err());
        assert_eq!(store.count().unwrap(), 1, "failed restore changed history");
    }

    #[test]
    fn restore_derives_the_live_count_from_restored_items() {
        let dir = tempfile::tempdir().unwrap();
        let (store, _path) = file_store(&dir);
        store.insert(item("backup row", T0)).unwrap();
        let backup = dir.path().join("history.backup");
        store.backup_to(&backup).unwrap();

        let candidate = open_validated(&backup, &KEY).unwrap();
        candidate
            .execute("UPDATE clipboard_live_count SET live = 99", [])
            .unwrap();
        drop(candidate);
        store.insert(item("later row", T0 + 60_000)).unwrap();

        store
            .restore_from(&backup, &KEY, &crate::Detector::new().unwrap())
            .unwrap();
        assert_eq!(store.count().unwrap(), 1);
    }

    #[test]
    fn restore_with_another_devices_key_preserves_history() {
        let dir = tempfile::tempdir().unwrap();
        let (store, _path) = file_store(&dir);
        store.insert(item("keep", T0)).unwrap();
        let foreign = dir.path().join("foreign.backup");
        let other = Store::open(&foreign, &OTHER_KEY).unwrap();
        other.insert(item("foreign", T0)).unwrap();
        drop(other);

        assert!(store
            .restore_from(&foreign, &KEY, &crate::Detector::new().unwrap())
            .is_err());
        assert_eq!(store.count().unwrap(), 1);
    }

    /// A restore is the one writer that copies rows rather than making them, so
    /// it is the one that could import a `content_bytes` disagreeing with the
    /// payload beside it and hand the byte quota a number it cannot act on.
    #[test]
    fn restore_recomputes_the_byte_column_rather_than_trusting_the_source() {
        let dir = tempfile::tempdir().unwrap();
        let (store, _path) = file_store(&dir);
        store.insert(item("restored payload", T0)).unwrap();
        let backup = dir.path().join("drifted.backup");
        store.backup_to(&backup).unwrap();

        // Corrupt the source's derived column the way a stale writer would.
        let conn = open_validated(&backup, &KEY).unwrap();
        conn.execute("UPDATE clipboard_items SET content_bytes = 999999", [])
            .unwrap();
        drop(conn);

        store
            .restore_from(&backup, &KEY, &crate::Detector::new().unwrap())
            .unwrap();

        let drifted: i64 = store
            .conn()
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM clipboard_items \
                  WHERE content_bytes <> LENGTH(COALESCE(content_ciphertext, X''))",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(drifted, 0, "the restore imported a stale byte count");
    }

    #[test]
    fn restore_rejects_nonexistent_schema_objects_and_preserves_history() {
        let dir = tempfile::tempdir().unwrap();
        let (store, _path) = file_store(&dir);
        store.insert(item("backup row", T0)).unwrap();
        let backup = dir.path().join("invalid-schema.backup");
        store.backup_to(&backup).unwrap();

        let conn = open_validated(&backup, &KEY).unwrap();
        conn.execute("CREATE TABLE future_schema_object (value INTEGER)", [])
            .unwrap();
        drop(conn);
        store.insert(item("live row", T0 + 1)).unwrap();

        assert!(matches!(
            store.restore_from(&backup, &KEY, &crate::Detector::new().unwrap()),
            Err(RestoreError::InvalidBackup(StoreError::InvalidSchema))
        ));
        assert_eq!(store.count().unwrap(), 2);
    }

    #[test]
    fn restore_reasserts_the_current_device_name() {
        let dir = tempfile::tempdir().unwrap();
        let (store, _path) = file_store(&dir);
        let identity = store.device_identity("Backup name").unwrap();
        let backup = dir.path().join("old-name.backup");
        store.backup_to(&backup).unwrap();
        store
            .set_device_name(&identity.device_id, "Current name")
            .unwrap();

        store
            .restore_from(&backup, &KEY, &crate::Detector::new().unwrap())
            .unwrap();

        assert_eq!(store.current_device_name().unwrap(), "Current name");
        let names = store
            .device_names(std::slice::from_ref(&identity.device_id))
            .unwrap();
        assert_eq!(names[&identity.device_id], "Current name");
    }

    #[test]
    fn restore_purges_sensitive_search_text_before_commit() {
        let dir = tempfile::tempdir().unwrap();
        let (store, _path) = file_store(&dir);
        let restored = store.insert(item("ordinary row", T0)).unwrap();
        let conn = store.conn().unwrap();
        let rowid: i64 = conn
            .query_row(
                "SELECT fts_rowid FROM clipboard_items WHERE id = ?1",
                [&restored.id],
                |row| row.get(0),
            )
            .unwrap();
        conn.execute("DELETE FROM clipboard_fts WHERE rowid = ?1", [rowid])
            .unwrap();
        conn.execute(
            "INSERT INTO clipboard_fts (rowid, id, content_text) VALUES (?1, ?2, ?3)",
            rusqlite::params![rowid, &restored.id, "AKIAIOSFODNN7EXAMPLE"],
        )
        .unwrap();
        drop(conn);
        let backup = dir.path().join("sensitive-index.backup");
        store.backup_to(&backup).unwrap();
        store.delete_all().unwrap();

        let report = store
            .restore_from(&backup, &KEY, &crate::Detector::new().unwrap())
            .unwrap();

        assert_eq!(report.purged, 1);
        assert!(store.get(&restored.id).unwrap().is_some());
        assert!(store.search("AKIAIOSFODNN7EXAMPLE", 10).unwrap().is_empty());
    }

    #[test]
    fn restore_failure_rolls_back_every_live_table() {
        let dir = tempfile::tempdir().unwrap();
        let (store, _path) = file_store(&dir);
        store.device_identity("Backup name").unwrap();
        store.insert(item("restored row", T0)).unwrap();
        let backup = dir.path().join("rollback.backup");
        store.backup_to(&backup).unwrap();

        store.delete_all().unwrap();
        store.insert(item("live rollback anchor", T0 + 1)).unwrap();
        let conn = store.conn().unwrap();
        conn.execute_batch(
            "CREATE TRIGGER reject_restored_name BEFORE INSERT ON sync_device_name
             BEGIN SELECT RAISE(ABORT, 'injected restore failure'); END;",
        )
        .unwrap();
        drop(conn);

        assert!(matches!(
            store.restore_from(&backup, &KEY, &crate::Detector::new().unwrap()),
            Err(RestoreError::Failed(_))
        ));
        assert_eq!(store.count().unwrap(), 1);
        assert_eq!(store.search("rollback anchor", 10).unwrap().len(), 1);
        assert!(store.search("restored row", 10).unwrap().is_empty());
    }
}
