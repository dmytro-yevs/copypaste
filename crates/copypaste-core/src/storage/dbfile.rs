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

use std::path::Path;

use rusqlite::Connection;
use zeroize::Zeroizing;

use super::connection::{apply_connection_pragmas, apply_key, validate_key};
use super::model::StoreError;

/// Open `path` with `db_key` and prove the key works before returning.
///
/// Fails closed: a key that does not open the file is [`StoreError::InvalidKey`],
/// never a fallback to an unkeyed read (CLAUDE.md rule 4).
///
/// **No migration is run.** That is the point of the "validating" half — a
/// candidate file is inspected as it stands, and a file this build has never
/// seen is refused rather than upgraded in place.
///
pub fn open_validated(path: &Path, db_key: &[u8; 32]) -> Result<Connection, StoreError> {
    let conn = Connection::open(path)?;
    apply_key(&conn, db_key)?;
    validate_key(&conn)?;
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
    super::schema::verify_schema(conn)
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
}
