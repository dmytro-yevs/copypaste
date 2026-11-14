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

use rusqlite::backup::{Backup, StepResult};
use rusqlite::{Connection, OpenFlags};
use zeroize::Zeroizing;

use super::connection::{apply_connection_pragmas, apply_key, run_pragma, validate_key};

/// Pages one backup step copies: 1 MiB at the default page size.
const BACKUP_STEP_PAGES: i32 = 256;
use super::content_bytes::content_bytes_sql;
use super::model::{stored_item_columns, StoreError};

// The live counter is derived by the target database's item triggers.
const RESTORED_TABLES: &[&str] = &[
    "organization_records",
    "history_tags",
    "history_clip_tags",
    "history_pin_state",
    "snippets",
    "snippet_bodies",
    "snippet_fts",
    "snippet_collections",
    "content_rules",
    "content_rule_placement",
    "transform_pipelines",
    "library_preferences",
    "library_set_members",
    "clip_notes",
    "module_records",
    "clip_local_state",
    "clip_rule_tags",
    "rule_originals",
    "content_rule_conflicts",
    "snippet_usage",
    "snippet_origin",
    "paste_queue_entries",
    "paste_queue_state",
    "cloud_withheld",
    "expired_ids",
    "file_references",
    "file_transfer_staging",
    "downloaded_files",
    "file_transfer_files",
    "file_transfer_chunks",
    "history_file_payloads",
    "shared_binary_payloads",
    "clipboard_fts",
    "clipboard_items",
    "source_app_icons",
    "sync_device_name",
];

/// Organization projections, copied after the clips they may describe.
const RESTORED_PROJECTIONS: &[&str] = &[
    "organization_records",
    "history_tags",
    "history_clip_tags",
    "history_pin_state",
    "snippets",
    "snippet_bodies",
    "snippet_collections",
    "content_rules",
    "content_rule_placement",
    "transform_pipelines",
    "library_preferences",
    "library_set_members",
    "clip_notes",
    "module_records",
];

/// Local state copied last, once every clip and record it refers to exists.
const RESTORED_LOCAL_STATE: &[&str] = &[
    "clip_local_state",
    "clip_rule_tags",
    "rule_originals",
    "content_rule_conflicts",
    "snippet_usage",
    "snippet_origin",
    "paste_queue_entries",
    "paste_queue_state",
    "cloud_withheld",
    "expired_ids",
];

const FTS_TABLES: &[&str] = &["clipboard_fts", "snippet_fts"];

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
/// never a fallback to an unkeyed read (the privacy contract).
///
pub fn open_validated(path: &Path, db_key: &[u8; 32]) -> Result<Connection, StoreError> {
    let mut conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    apply_key(&conn, db_key)?;
    validate_key(&conn)?;
    // Detect truncated files without scanning every payload on ordinary opens.
    let page_size = match conn.pragma_query_value(None, "page_size", |row| {
        row.get::<_, rusqlite::types::Value>(0)
    })? {
        rusqlite::types::Value::Integer(size) => {
            u64::try_from(size).map_err(|_| StoreError::IntegrityCheckFailed)?
        }
        rusqlite::types::Value::Text(size) => size
            .parse::<u64>()
            .map_err(|_| StoreError::IntegrityCheckFailed)?,
        _ => return Err(StoreError::IntegrityCheckFailed),
    };
    if page_size == 0 || !std::fs::metadata(path)?.len().is_multiple_of(page_size) {
        return Err(StoreError::IntegrityCheckFailed);
    }
    conn.pragma_update(None, "foreign_keys", true)?;
    super::migrations::upgrade(&mut conn)?;
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
    /// Copy a consistent encrypted snapshot of the database into the empty
    /// file at `dest`, reporting copied and total pages after every step.
    ///
    /// SQLite's online backup keyed with `db_key`, so the copy is as unreadable
    /// without the device secret as the original. The source holds one read
    /// transaction throughout: concurrent writes neither enter the copy nor
    /// restart it. `progress` returning `false` stops the copy with
    /// [`std::io::ErrorKind::Interrupted`], leaving `dest` incomplete for the
    /// caller to discard.
    pub fn backup_to(
        &self,
        dest: &Path,
        db_key: &[u8; 32],
        mut progress: impl FnMut(u64, u64) -> bool,
    ) -> Result<(), StoreError> {
        // Never onto existing content: the obvious mistake is naming the live
        // database, and a copy onto it would be a wipe.
        if std::fs::metadata(dest).is_ok_and(|metadata| metadata.len() > 0) {
            return Err(std::io::Error::from(std::io::ErrorKind::AlreadyExists).into());
        }
        let conn = self.conn()?;
        let snapshot = conn.unchecked_transaction()?;
        snapshot.query_row("SELECT count(*) FROM sqlite_schema", [], |_| Ok(()))?;
        let mut target = Connection::open_with_flags(
            dest,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        apply_key(&target, db_key)?;
        run_pragma(&target, "PRAGMA journal_mode = OFF")?;
        let backup = Backup::new(&snapshot, &mut target)?;
        loop {
            let step = backup.step(BACKUP_STEP_PAGES)?;
            let pages = backup.progress();
            let total = u64::try_from(pages.pagecount).unwrap_or(0);
            let remaining = u64::try_from(pages.remaining).unwrap_or(0);
            if !progress(total.saturating_sub(remaining), total) {
                return Err(std::io::Error::from(std::io::ErrorKind::Interrupted).into());
            }
            match step {
                StepResult::Done => return Ok(()),
                StepResult::More => {}
                _ => std::thread::sleep(std::time::Duration::from_millis(10)),
            }
        }
    }

    /// Stage and validate a same-device encrypted backup, then atomically replace history.
    pub fn restore_from(&self, src: &Path, db_key: &[u8; 32]) -> Result<(), RestoreError> {
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
        let result = (|| -> rusqlite::Result<()> {
            let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
            // Previews are disposable and may have been derived from the replaced history.
            tx.execute("DELETE FROM history_preview_cache", [])?;
            for table in RESTORED_TABLES {
                tx.execute(&format!("DELETE FROM {table}"), [])?;
            }
            // `content_bytes` is recomputed from ciphertext and metadata that
            // actually arrive, so a source file cannot import a byte quota
            // that disagrees with its rows.
            tx.execute(
                "INSERT INTO source_app_icons SELECT * FROM restore_src.source_app_icons",
                [],
            )?;
            tx.execute(
                concat!(
                    "INSERT INTO clipboard_items (",
                    stored_item_columns!(),
                    ", content_bytes) SELECT ",
                    stored_item_columns!(),
                    ", LENGTH(COALESCE(content_ciphertext, X'')) + \
                       LENGTH(COALESCE(payload_metadata, '')) \
                     FROM restore_src.clipboard_items"
                ),
                [],
            )?;
            for table in RESTORED_PROJECTIONS {
                tx.execute(
                    &format!("INSERT INTO {table} SELECT * FROM restore_src.{table}"),
                    [],
                )?;
            }
            tx.execute("INSERT INTO shared_binary_payloads SELECT * FROM restore_src.shared_binary_payloads", [])?;
            tx.execute("INSERT INTO file_transfer_chunks SELECT c.* FROM restore_src.file_transfer_chunks c WHERE EXISTS (SELECT 1 FROM restore_src.file_transfer_files f WHERE f.blob_id=c.blob_id)", [])?;
            tx.execute(
                "INSERT INTO file_transfer_files SELECT * FROM restore_src.file_transfer_files",
                [],
            )?;
            tx.execute(
                "INSERT INTO downloaded_files SELECT * FROM restore_src.downloaded_files",
                [],
            )?;
            tx.execute(
                "INSERT INTO file_references SELECT * FROM restore_src.file_references",
                [],
            )?;
            tx.execute(
                "INSERT INTO history_file_payloads SELECT * FROM restore_src.history_file_payloads",
                [],
            )?;
            tx.execute(
                concat!(
                    "UPDATE clipboard_items SET content_bytes = ",
                    content_bytes_sql!("payload_metadata")
                ),
                [],
            )?;
            tx.execute(
                "INSERT INTO clipboard_fts (rowid, id, content_text) \
                 SELECT ci.fts_rowid, fts.id, fts.content_text FROM restore_src.clipboard_fts fts \
                 JOIN restore_src.clipboard_items ci ON ci.id = fts.id \
                 WHERE ci.deleted = 0 \
                   AND (ci.content_type = 'text' OR ci.content_type LIKE 'text/%')",
                [],
            )?;
            tx.execute(
                "INSERT INTO snippet_fts (rowid, id, title, abbreviation, body_text) \
                 SELECT rowid, id, title, abbreviation, body_text FROM restore_src.snippet_fts",
                [],
            )?;
            for table in RESTORED_LOCAL_STATE {
                tx.execute(
                    &format!("INSERT INTO {table} SELECT * FROM restore_src.{table}"),
                    [],
                )?;
            }
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
            tx.execute("UPDATE resource_revisions SET generation = lower(hex(randomblob(16))) WHERE singleton = 0", [])?;
            tx.commit()?;
            Ok(())
        })();
        let _ = conn.execute("DETACH DATABASE restore_src", []);
        let result = result
            .map_err(StoreError::from)
            .map_err(RestoreError::Failed);
        drop(conn);
        if result.is_ok() {
            self.clear_semantic_cache();
            *self
                .facet_cache
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
            self.request_space_reclaim();
        }
        result
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
            !FTS_TABLES.iter().any(|fts| {
                name.strip_prefix(fts)
                    .and_then(|rest| rest.strip_prefix('_'))
                    .is_some_and(|suffix| {
                        matches!(suffix, "data" | "idx" | "content" | "docsize" | "config")
                    })
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
    fn an_earlier_schema_is_refused_without_upgrade_or_repair() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("copypaste-v2.db");
        let conn = Connection::open(&path).unwrap();
        apply_key(&conn, &KEY).unwrap();
        validate_key(&conn).unwrap();
        conn.execute_batch(
            "CREATE TABLE clipboard_items (
                 id TEXT PRIMARY KEY NOT NULL,
                 content_ciphertext BLOB,
                 nonce BLOB,
                 content_type TEXT NOT NULL,
                 content_hash TEXT NOT NULL DEFAULT '',
                 pinned INTEGER NOT NULL DEFAULT 0,
                 pin_order REAL,
                 created_at INTEGER NOT NULL,
                 deleted INTEGER NOT NULL DEFAULT 0
             );
             CREATE INDEX idx_items_history
                 ON clipboard_items(pinned DESC, pin_order, created_at DESC, id DESC)
                 WHERE deleted = 0;
             CREATE INDEX idx_items_hash
                 ON clipboard_items(content_hash, created_at DESC)
                 WHERE deleted = 0;
             CREATE UNIQUE INDEX idx_items_dedup
                 ON clipboard_items(content_hash, created_at / 60000)
                 WHERE deleted = 0 AND content_hash <> '';
             CREATE VIRTUAL TABLE clipboard_fts USING fts5(id UNINDEXED, content_text);
             CREATE TABLE __rusqlite_migrations (
                 version INTEGER PRIMARY KEY NOT NULL,
                 applied_on TEXT NOT NULL
             );
             INSERT INTO clipboard_items (
                 id, content_ciphertext, nonce, content_type, content_hash,
                 pinned, pin_order, created_at, deleted
             ) VALUES (
                 'keep-me', X'010203', X'040506', 'text/plain', 'hash',
                 0, NULL, 1234, 0
             );
             INSERT INTO clipboard_fts(rowid, id, content_text)
             VALUES (1, 'keep-me', 'indexed marker');
             INSERT INTO __rusqlite_migrations (version, applied_on)
             VALUES (1, 'earlier-build');",
        )
        .unwrap();
        drop(conn);

        assert!(matches!(
            open_validated(&path, &KEY),
            Err(StoreError::InvalidSchema)
        ));

        let conn = Connection::open(&path).unwrap();
        apply_key(&conn, &KEY).unwrap();
        validate_key(&conn).unwrap();
        let row: (Vec<u8>, Vec<u8>, i64) = conn
            .query_row(
                "SELECT content_ciphertext, nonce, created_at
                 FROM clipboard_items WHERE id = 'keep-me'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(row, (vec![1, 2, 3], vec![4, 5, 6], 1234));
        let objects_added: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_schema
                 WHERE name IN (
                     'clipboard_live_count', 'sync_device_state', 'sync_device_name',
                     'idx_items_syncable'
                 )",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(objects_added, 0, "open must not upgrade an earlier schema");
        let marker: String = conn
            .query_row(
                "SELECT applied_on FROM __rusqlite_migrations WHERE version = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(marker, "earlier-build");
    }

    #[test]
    fn a_backup_round_trips_under_the_same_key_and_not_another() {
        let dir = tempfile::tempdir().unwrap();
        let (store, _path) = file_store(&dir);
        store.insert(item("worth keeping", T0)).unwrap();

        let dest = dir.path().join("history.backup");
        store.backup_to(&dest, &KEY, |_, _| true).unwrap();
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

        assert!(store.backup_to(&path, &KEY, |_, _| true).is_err());
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
        store.backup_to(&other, &KEY, |_, _| true).unwrap();

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
        store.backup_to(&backup, &KEY, |_, _| true).unwrap();
        store.insert(item("later row", T0 + 1)).unwrap();
        let generation = store.resource_revisions().unwrap().generation;
        store
            .history_facets("local", "Laptop", copypaste_ipc::DeviceClass::Laptop)
            .unwrap();
        assert!(store.facet_cache.lock().unwrap().is_some());

        store.restore_from(&backup, &KEY).unwrap();
        assert!(store.facet_cache.lock().unwrap().is_none());
        assert_ne!(store.resource_revisions().unwrap().generation, generation);
        assert_eq!(store.count().unwrap(), 1);

        drop(store);
        let store = Store::open(&dir.path().join("copypaste-v2.db"), &KEY).unwrap();
        assert_eq!(store.count().unwrap(), 1, "restore did not survive reopen");

        let junk = dir.path().join("junk.backup");
        std::fs::write(&junk, b"not a database").unwrap();
        assert!(store.restore_from(&junk, &KEY).is_err());
        assert_eq!(store.count().unwrap(), 1, "failed restore changed history");
    }

    #[test]
    fn restore_derives_the_live_count_from_restored_items() {
        let dir = tempfile::tempdir().unwrap();
        let (store, _path) = file_store(&dir);
        store.insert(item("backup row", T0)).unwrap();
        let backup = dir.path().join("history.backup");
        store.backup_to(&backup, &KEY, |_, _| true).unwrap();

        let candidate = open_validated(&backup, &KEY).unwrap();
        candidate
            .execute("UPDATE clipboard_live_count SET live = 99", [])
            .unwrap();
        drop(candidate);
        store.insert(item("later row", T0 + 60_000)).unwrap();

        store.restore_from(&backup, &KEY).unwrap();
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

        assert!(store.restore_from(&foreign, &KEY).is_err());
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
        store.backup_to(&backup, &KEY, |_, _| true).unwrap();

        // Corrupt the source's derived column the way a stale writer would.
        let conn = open_validated(&backup, &KEY).unwrap();
        conn.execute("UPDATE clipboard_items SET content_bytes = 999999", [])
            .unwrap();
        drop(conn);

        store.restore_from(&backup, &KEY).unwrap();

        let drifted: i64 = store
            .conn()
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM clipboard_items \
                  WHERE content_bytes <> LENGTH(COALESCE(content_ciphertext, X'')) \
                    + LENGTH(COALESCE(payload_metadata, ''))",
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
        store.backup_to(&backup, &KEY, |_, _| true).unwrap();

        let conn = open_validated(&backup, &KEY).unwrap();
        conn.execute("CREATE TABLE future_schema_object (value INTEGER)", [])
            .unwrap();
        drop(conn);
        store.insert(item("live row", T0 + 1)).unwrap();

        assert!(matches!(
            store.restore_from(&backup, &KEY),
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
        store.backup_to(&backup, &KEY, |_, _| true).unwrap();
        store
            .set_device_name(&identity.device_id, "Current name")
            .unwrap();

        store.restore_from(&backup, &KEY).unwrap();

        assert_eq!(store.current_device_name().unwrap(), "Current name");
        let names = store
            .device_names(std::slice::from_ref(&identity.device_id))
            .unwrap();
        assert_eq!(names[&identity.device_id], "Current name");
    }

    #[test]
    fn restore_preserves_all_text_and_its_search_index() {
        let dir = tempfile::tempdir().unwrap();
        let (store, _path) = file_store(&dir);
        let restored = store.insert(item("AKIAIOSFODNN7EXAMPLE", T0)).unwrap();
        let backup = dir.path().join("text-index.backup");
        store.backup_to(&backup, &KEY, |_, _| true).unwrap();
        store.delete_all().unwrap();
        store.restore_from(&backup, &KEY).unwrap();
        assert!(store.get(&restored.id).unwrap().is_some());
        assert_eq!(store.search("AKIAIOSFODNN7EXAMPLE", 10).unwrap().len(), 1);
    }

    #[test]
    fn restore_failure_rolls_back_every_live_table() {
        let dir = tempfile::tempdir().unwrap();
        let (store, _path) = file_store(&dir);
        store.device_identity("Backup name").unwrap();
        store.insert(item("restored row", T0)).unwrap();
        let backup = dir.path().join("rollback.backup");
        store.backup_to(&backup, &KEY, |_, _| true).unwrap();

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
            store.restore_from(&backup, &KEY),
            Err(RestoreError::Failed(_))
        ));
        assert_eq!(store.count().unwrap(), 1);
        assert_eq!(store.search("rollback anchor", 10).unwrap().len(), 1);
        assert!(store.search("restored row", 10).unwrap().is_empty());
    }

    /// One row in every library and per-clip table, the clip-keyed
    /// ones pointing at `clip`.
    fn library_rows(store: &Store, clip: &str) {
        store
            .conn()
            .unwrap()
            .execute_batch(&format!(
                "INSERT INTO snippets (id, record_id, title, template, pinned, order_key, concealed, meta_stamp, created_at, created_device) VALUES ('s', 'r1', 't', 0, 0, 'a', 0, 1, 1, 'd');
                 INSERT INTO snippet_bodies VALUES ('s', 'r2', 'rev', '[]', 'body', NULL, 1);
                 INSERT INTO snippet_fts (id, title, abbreviation, body_text) VALUES ('s', 't', NULL, 'body');
                 INSERT INTO snippet_collections VALUES ('c', 'r3', 'n', 'i', 'a', 'k', NULL);
                 INSERT INTO content_rules VALUES ('rule', 'r4', 'user', NULL, '[]', 'n', '{{}}', '[]', 1, 1, NULL, 1);
                 INSERT INTO content_rule_placement VALUES ('rule', 'r5', 1, 'a');
                 INSERT INTO transform_pipelines VALUES ('p', 'r6', 'n', NULL, '[]', NULL, 1);
                 INSERT INTO library_preferences VALUES ('snippets', 'k', 'r7', 'true', 1);
                 INSERT INTO library_set_members VALUES ('snippets.s', 'm', 'r8', 1, 1);
                 INSERT INTO clip_notes VALUES ('{clip}', 'r10', 1, 1, 0, 0, NULL, NULL, NULL, NULL);
                 INSERT INTO module_records VALUES ('example.module', 'k', 'r11', '1', 0, 1);
                 INSERT INTO clip_local_state (clip_id, rule_secret) VALUES ('{clip}', 1);
                 INSERT INTO clip_rule_tags VALUES ('{clip}', 'tag');
                 INSERT INTO rule_originals VALUES ('{clip}', X'01', X'02', 'text', 1);
                 INSERT INTO content_rule_conflicts VALUES ('x', 'rule', '{{}}', 'd', 1);
                 INSERT INTO snippet_usage VALUES ('s', 1, 1);
                 INSERT INTO snippet_origin VALUES ('s', '{clip}');
                 INSERT INTO paste_queue_entries (entry_id, position, source_kind, clip_id, content_kind, status, added_at, added_via) VALUES ('e', 1, 'clip', '{clip}', 'text', 'pending', 1, 'hotkey');
                 INSERT INTO paste_queue_state (id, revision) VALUES (1, 1);
                 INSERT INTO cloud_withheld VALUES ('{clip}', 1);
                 INSERT INTO expired_ids VALUES ('gone', 1);"
            ))
            .unwrap();
    }

    fn count(store: &Store, table: &str) -> i64 {
        store
            .conn()
            .unwrap()
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
            .unwrap()
    }

    #[test]
    fn restore_keeps_every_library_and_per_clip_row_copied_after_its_clip() {
        let dir = tempfile::tempdir().unwrap();
        let (store, _path) = file_store(&dir);
        let clip = store.insert(item("described", T0)).unwrap();
        library_rows(&store, &clip.id);
        let before: Vec<i64> = RESTORED_TABLES.iter().map(|t| count(&store, t)).collect();
        let backup = dir.path().join("library.backup");
        store.backup_to(&backup, &KEY, |_, _| true).unwrap();

        store.delete_all().unwrap();
        assert_eq!(count(&store, "clip_local_state"), 0);
        assert_eq!(count(&store, "rule_originals"), 0);
        store.restore_from(&backup, &KEY).unwrap();

        let after: Vec<i64> = RESTORED_TABLES.iter().map(|t| count(&store, t)).collect();
        assert_eq!(after, before);
        assert!(RESTORED_LOCAL_STATE
            .iter()
            .chain(RESTORED_PROJECTIONS)
            .filter(|table| !table.starts_with("history_") && **table != "organization_records")
            .all(|table| count(&store, table) > 0));
    }
}
