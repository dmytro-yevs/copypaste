//! Opening the second connection, and this device's sync identity.
//!
//! # Concurrency
//!
//! Two writers on one file is ordinary SQLite: the store's pool and this
//! connection both run in WAL with a busy timeout, and every write here is one
//! transaction. The connection is behind a `std::sync::Mutex` because a
//! `rusqlite::Connection` is not `Sync`; the guard is never held across an
//! `.await`.

use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use rusqlite::{params, Connection, OptionalExtension};
use zeroize::Zeroizing;

use super::error::is_not_a_database;
use super::MetaError;

/// Key of the persisted device id in `sync_device_state`.
const KEY_DEVICE_ID: &str = "device_id";
/// Key of the persisted device name.
const KEY_DEVICE_NAME: &str = "device_name";

/// This device's sync identity and the metadata behind every session.
pub struct Meta {
    conn: Mutex<Connection>,
    device_id: String,
    device_name: String,
}

impl std::fmt::Debug for Meta {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // No path, and no connection: `Connection`'s own Debug prints the file.
        f.debug_struct("Meta")
            .field("device_id", &self.device_id)
            .finish_non_exhaustive()
    }
}

impl Meta {
    /// Open the history database on a private connection and resolve this
    /// device's identity, minting it on first run.
    ///
    /// `name_hint` is used only when no name has been stored yet: the device
    /// name is cosmetic and peer-visible, so it stays put across a hostname
    /// change rather than churning on every restart.
    pub fn open(path: &Path, db_key: &[u8; 32], name_hint: &str) -> Result<Self, MetaError> {
        let conn = Connection::open(path).map_err(MetaError::Sqlite)?;
        apply_key(&conn, db_key)?;
        validate_key(&conn)?;
        for pragma in [
            "PRAGMA busy_timeout = 5000",
            "PRAGMA foreign_keys = ON",
            "PRAGMA temp_store = MEMORY",
        ] {
            run_pragma(&conn, pragma)?;
        }

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS sync_device_state (
                 key   TEXT PRIMARY KEY NOT NULL,
                 value TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS sync_item_origin (
                 item_id          TEXT PRIMARY KEY NOT NULL,
                 origin_device_id TEXT NOT NULL
             );",
        )?;

        let device_id = load_or_set(&conn, KEY_DEVICE_ID, || uuid::Uuid::new_v4().to_string())?;
        let device_name = load_or_set(&conn, KEY_DEVICE_NAME, || sanitise_name(name_hint))?;

        Ok(Self {
            conn: Mutex::new(conn),
            device_id,
            device_name,
        })
    }

    #[must_use]
    pub fn device_id(&self) -> &str {
        &self.device_id
    }

    #[must_use]
    pub fn device_name(&self) -> &str {
        &self.device_name
    }

    /// Replace the stored device name. Cosmetic; takes effect on the next hello.
    pub fn set_device_name(&mut self, name: &str) -> Result<(), MetaError> {
        let name = sanitise_name(name);
        {
            let conn = self.lock()?;
            conn.execute(
                "INSERT INTO sync_device_state (key, value) VALUES (?1, ?2) \
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![KEY_DEVICE_NAME, &name],
            )?;
        }
        self.device_name = name;
        Ok(())
    }

    /// Poisoning is surfaced rather than recovered: this connection decides
    /// what leaves the device, and a map observed mid-update is not a basis for
    /// that decision.
    pub(super) fn lock(&self) -> Result<MutexGuard<'_, Connection>, MetaError> {
        self.conn.lock().map_err(|_| MetaError::Poisoned)
    }
}

fn apply_key(conn: &Connection, db_key: &[u8; 32]) -> Result<(), MetaError> {
    // Same shape as the store's: SQLCipher wants the raw key as
    // `x'<64 hex>'`, applied before any other statement, and both the hex and
    // the array are wrapped so neither lingers in freed heap.
    let key_hex = Zeroizing::new(hex::encode(db_key));
    let stmt = Zeroizing::new(format!("PRAGMA key = \"x'{}'\"", key_hex.as_str()));
    run_pragma(conn, &stmt)
}

fn run_pragma(conn: &Connection, sql: &str) -> Result<(), MetaError> {
    let mut stmt = conn.prepare(sql)?;
    let mut rows = stmt.query([])?;
    while rows.next()?.is_some() {}
    Ok(())
}

/// Proves the key opens the file before anything else touches it.
fn validate_key(conn: &Connection) -> Result<(), MetaError> {
    match conn.query_row("SELECT COUNT(*) FROM sqlite_master", [], |r| {
        r.get::<_, i64>(0)
    }) {
        Ok(_) => Ok(()),
        Err(e) if is_not_a_database(&e) => Err(MetaError::InvalidKey),
        Err(e) => Err(MetaError::Sqlite(e)),
    }
}

fn load_or_set(
    conn: &Connection,
    key: &str,
    mint: impl FnOnce() -> String,
) -> Result<String, MetaError> {
    let existing: Option<String> = conn
        .query_row(
            "SELECT value FROM sync_device_state WHERE key = ?1",
            [key],
            |r| r.get(0),
        )
        .optional()?;
    if let Some(value) = existing.filter(|v| !v.is_empty()) {
        return Ok(value);
    }
    let value = mint();
    conn.execute(
        "INSERT INTO sync_device_state (key, value) VALUES (?1, ?2) \
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, &value],
    )?;
    Ok(value)
}

/// A device name goes on the wire and into a peer's UI, so it is bounded and
/// stripped here rather than at each use.
///
/// `MAX_DEVICE_NAME_BYTES` is a *byte* bound and the name is user-supplied
/// UTF-8, so the truncation walks characters.
fn sanitise_name(raw: &str) -> String {
    let cleaned: String = raw
        .trim()
        .chars()
        .filter(|c| !c.is_control())
        .take(copypaste_p2p::protocol::MAX_DEVICE_NAME_BYTES / 4)
        .collect();
    if cleaned.is_empty() {
        "CopyPaste device".to_string()
    } else {
        cleaned
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use copypaste_core::{Keyring, Store};

    #[test]
    fn a_device_identity_is_minted_once_and_then_reused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("copypaste-v2.db");
        let key = Keyring::from_secret(&[9u8; 32]).db_key();
        let _store = Store::open(&path, &key).unwrap();

        let first = Meta::open(&path, &key, "laptop").unwrap();
        let id = first.device_id().to_string();
        assert!(!id.is_empty());
        assert_eq!(first.device_name(), "laptop");
        drop(first);

        // A different hint must not move the identity: peers key off it.
        let second = Meta::open(&path, &key, "something else").unwrap();
        assert_eq!(second.device_id(), id);
        assert_eq!(second.device_name(), "laptop");
    }

    #[test]
    fn a_wrong_key_fails_closed_rather_than_reading() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("copypaste-v2.db");
        let _store = Store::open(&path, &Keyring::from_secret(&[1u8; 32]).db_key()).unwrap();

        let err = Meta::open(&path, &Keyring::from_secret(&[2u8; 32]).db_key(), "x")
            .expect_err("a wrong key must not open the database");
        assert!(matches!(err, MetaError::InvalidKey), "{err:?}");
    }

    #[test]
    fn a_device_name_is_bounded_and_stripped() {
        assert_eq!(sanitise_name("  laptop\n "), "laptop");
        assert_eq!(sanitise_name(""), "CopyPaste device");
        assert!(sanitise_name(&"x".repeat(500)).len() <= 128);
    }
}
