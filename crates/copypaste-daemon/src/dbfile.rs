//! Opening the SQLCipher history file on a private connection.
//!
//! One implementation, used by [`crate::meta`] and by `server::dbadmin`. It is
//! here rather than in either of them because the second caller is how a third
//! `PRAGMA key` gets written by hand: `copypaste-core` keeps its own inside the
//! pool and does not expose a raw connection, so the daemon needs exactly one
//! of its own and no more (CLAUDE.md rule 1).
//!
//! **The clean fix is still a `copypaste-core` change** — a `Store::backup_to`
//! and a `Store::validate` would delete this file. See `meta`'s module header
//! for the same debt described in full.

use std::path::Path;

use rusqlite::Connection;
use zeroize::Zeroizing;

use crate::meta::MetaError;

/// Pragmas every private connection gets, after `PRAGMA key`.
///
/// `temp_store = MEMORY` keeps temp B-trees — which hold decrypted
/// intermediates — off the disk.
const PRAGMAS: &[&str] = &[
    "PRAGMA busy_timeout = 5000",
    "PRAGMA foreign_keys = ON",
    "PRAGMA temp_store = MEMORY",
];

/// Open `path` with `db_key` and prove the key works before returning.
///
/// Fails closed: a key that does not open the file is [`MetaError::InvalidKey`],
/// never a fallback to an unkeyed read (CLAUDE.md rule 4).
pub fn open(path: &Path, db_key: &[u8; 32]) -> Result<Connection, MetaError> {
    let conn = Connection::open(path).map_err(MetaError::Sqlite)?;
    apply_key(&conn, db_key)?;
    validate_key(&conn)?;
    for pragma in PRAGMAS {
        run_pragma(&conn, pragma)?;
    }
    Ok(conn)
}

/// `PRAGMA key` in SQLCipher raw-key form.
///
/// **Must be the first statement on the connection.** The `x'<64 lowercase
/// hex>'` form takes the 32 bytes as the page key directly and skips PBKDF2;
/// both the hex and the statement are wrapped so neither lingers in freed heap.
pub fn apply_key(conn: &Connection, db_key: &[u8; 32]) -> Result<(), MetaError> {
    let key_hex = Zeroizing::new(hex::encode(db_key));
    let stmt = Zeroizing::new(format!("PRAGMA key = \"x'{}'\"", key_hex.as_str()));
    run_pragma(conn, &stmt)
}

/// The same key, rendered for an `ATTACH ... KEY` clause.
///
/// Inline in the SQL rather than bound as a parameter: SQLCipher recognises the
/// raw-key `x'…'` form only in the literal text of the clause, and a bound
/// value is taken as a passphrase and run through PBKDF2 instead — which
/// produces a different key and a file that will not open.
pub fn attach_key_literal(db_key: &[u8; 32]) -> Zeroizing<String> {
    let key_hex = Zeroizing::new(hex::encode(db_key));
    Zeroizing::new(format!("\"x'{}'\"", key_hex.as_str()))
}

/// Runs a pragma, tolerating the ones that return a row. `Connection::execute`
/// rejects those.
pub fn run_pragma(conn: &Connection, sql: &str) -> Result<(), MetaError> {
    let mut stmt = conn.prepare(sql)?;
    let mut rows = stmt.query([])?;
    while rows.next()?.is_some() {}
    Ok(())
}

/// Proves the key opens the file before anything else touches it.
pub fn validate_key(conn: &Connection) -> Result<(), MetaError> {
    match conn.query_row("SELECT COUNT(*) FROM sqlite_master", [], |r| {
        r.get::<_, i64>(0)
    }) {
        Ok(_) => Ok(()),
        Err(e) if crate::meta::is_not_a_database(&e) => Err(MetaError::InvalidKey),
        Err(e) => Err(MetaError::Sqlite(e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use copypaste_core::{Keyring, Store};

    #[test]
    fn a_wrong_key_fails_closed_rather_than_reading() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("copypaste-v2.db");
        let right = Keyring::from_secret(&[1u8; 32]).db_key();
        let _store = Store::open(&path, &right).unwrap();

        assert!(open(&path, &right).is_ok());
        let err = open(&path, &Keyring::from_secret(&[2u8; 32]).db_key())
            .expect_err("a wrong key must not open the database");
        assert!(matches!(err, MetaError::InvalidKey), "{err:?}");
    }
}
