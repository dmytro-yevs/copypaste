//! The sync view of the item table, plus this device's identity.
//!
//! # Why this module opens the database itself
//!
//! A sync session needs five things about an item: its id, its version stamp,
//! its content hash, whether it is a tombstone, and which device first captured
//! it. `copypaste_core::StoredItem` carries the first two. The next two are
//! *columns that already exist* in `clipboard_items` (`content_hash`,
//! `deleted`) but are not projected into `StoredItem`, and the last has no
//! column at all.
//!
//! There were two ways to close that gap without touching `copypaste-core`:
//!
//! 1. Shadow the missing fields in a second store the daemon owns, writing
//!    every hash, every tombstone and every timestamp twice.
//! 2. Read the columns that are already there, on a second connection to the
//!    same SQLCipher file, and add one table for the one genuinely new fact.
//!
//! The first is the failure mode `CLAUDE.md` rule 1 is written about: two
//! implementations of "what is in this device's history", which drift the first
//! time an eviction or a delete lands on one and not the other. So this module
//! does the second. It owns exactly one new table — `sync_item_origin` — and
//! otherwise only reads and updates rows the store already manages.
//!
//! **This is a layering compromise, and it should be repaid.** The clean fix is
//! four additions to `copypaste-core::Store`, at which point every raw statement
//! below deletes itself:
//!
//! * `content_hash` and `deleted` on `StoredItem`,
//! * `Store::summaries()` — id, stamp, hash, tombstone flag, live and deleted,
//! * `Store::upsert(NewItem, deleted: bool)` — the LWW write, which is the one
//!   thing the insert-only API genuinely cannot express,
//! * an `origin_device_id` column, so this module keeps only the device row.
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

use copypaste_p2p::protocol::ItemSummary;
use rusqlite::{params, Connection, OptionalExtension};
use zeroize::Zeroizing;

/// Key of the persisted device id in `sync_device_state`.
const KEY_DEVICE_ID: &str = "device_id";
/// Key of the persisted device name.
const KEY_DEVICE_NAME: &str = "device_name";

/// Ceiling on how many summaries one session advertises.
///
/// `sync::advertise` truncates to `MAX_SUMMARIES_PER_MESSAGE` itself, so this
/// exists to bound the *query*, not the message: without it a history at the
/// 10 000-item cap plus its tombstones would be read out of SQLite in full only
/// to be thrown away.
const SUMMARY_LIMIT: i64 = copypaste_p2p::protocol::MAX_SUMMARIES_PER_MESSAGE as i64;

/// Everything this module can fail with.
///
/// No variant renders a path: `rusqlite`'s messages come from
/// `sqlite3_errmsg`, which does not embed the filename, and the cause is a
/// `#[source]` rather than interpolated text so it cannot reach a user
/// (`CLAUDE.md` rule 4).
#[derive(Debug, thiserror::Error)]
pub enum MetaError {
    #[error("the history database could not be read or written")]
    Sqlite(#[source] rusqlite::Error),

    /// Fail closed: a key that does not open the file is an error, never a
    /// fallback to an unkeyed read (`CLAUDE.md` rule 4).
    #[error("the history database could not be opened with this device's key")]
    InvalidKey,

    #[error("the sync metadata is no longer usable in this process")]
    Poisoned,
}

impl From<rusqlite::Error> for MetaError {
    fn from(e: rusqlite::Error) -> Self {
        MetaError::Sqlite(e)
    }
}

/// One item as the merge sees it locally.
#[derive(Debug, Clone)]
pub struct LocalVersion {
    pub summary: ItemSummary,
    /// Never empty — `SyncMessage::validate` rejects an empty id, and an item
    /// with no recorded origin is one this device captured.
    pub origin_device_id: String,
    pub is_sensitive: bool,
}

/// One item on its way out to a peer, still encrypted.
#[derive(Debug, Clone)]
pub struct StoredVersion {
    pub item_id: String,
    /// `None` on a tombstone: the soft delete wiped the payload.
    pub content_ciphertext: Option<Vec<u8>>,
    pub nonce: Option<Vec<u8>>,
    pub content_type: String,
    pub content_hash: String,
    pub created_at: i64,
    pub deleted: bool,
    pub origin_device_id: String,
}

/// One item on its way in from a peer, already sealed under the local key.
pub struct IncomingVersion<'a> {
    pub item_id: &'a str,
    pub content_ciphertext: Option<&'a [u8]>,
    pub nonce: Option<&'a [u8]>,
    pub content_type: &'a str,
    pub content_hash: &'a str,
    pub created_at: i64,
    pub deleted: bool,
    pub is_sensitive: bool,
    pub origin_device_id: &'a str,
    /// Plaintext for the search index. Ignored when the item is sensitive or a
    /// tombstone — the write-time layer of "sensitive items are never indexed".
    pub search_text: Option<&'a str>,
}

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

    /// Everything eligible to sync, newest first, tombstones included.
    ///
    /// **Sensitive items are excluded here and nowhere else matters more.**
    /// This is the query that decides what leaves the device: `sync::advertise`
    /// turns the result into the session's `advertised` set, and `serve_items`
    /// refuses to send anything outside it. A sensitive item that slipped into
    /// this list would be served on request.
    pub fn summaries(&self) -> Result<Vec<ItemSummary>, MetaError> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare_cached(
            "SELECT id, created_at, deleted, content_hash \
               FROM clipboard_items \
              WHERE is_sensitive = 0 \
              ORDER BY created_at DESC, id DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map([SUMMARY_LIMIT], |row| {
            Ok(ItemSummary {
                item_id: row.get(0)?,
                created_at: row.get(1)?,
                deleted: row.get::<_, i64>(2)? != 0,
                content_hash: row.get(3)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// The local version of one item — live, tombstoned, or sensitive.
    ///
    /// Sensitive rows are *included*: `apply` has to compare against them, or a
    /// peer's copy of something this device flagged would be stored a second
    /// time under the same id.
    pub fn local_version(&self, item_id: &str) -> Result<Option<LocalVersion>, MetaError> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare_cached(
            "SELECT ci.created_at, ci.deleted, ci.content_hash, ci.is_sensitive, \
                    o.origin_device_id \
               FROM clipboard_items ci \
               LEFT JOIN sync_item_origin o ON o.item_id = ci.id \
              WHERE ci.id = ?1",
        )?;
        let found = stmt
            .query_row([item_id], |row| {
                let origin: Option<String> = row.get(4)?;
                Ok(LocalVersion {
                    summary: ItemSummary {
                        item_id: item_id.to_string(),
                        created_at: row.get(0)?,
                        deleted: row.get::<_, i64>(1)? != 0,
                        content_hash: row.get(2)?,
                    },
                    // An item with no recorded origin was captured here: the
                    // origin table only gains a row for something that arrived
                    // from a peer or was ingested after this feature existed.
                    origin_device_id: origin.unwrap_or_else(|| self.device_id.clone()),
                    is_sensitive: row.get::<_, i64>(3)? != 0,
                })
            })
            .optional()?;
        Ok(found)
    }

    /// The rows behind a request, still encrypted. Sensitive items are omitted.
    ///
    /// Unknown ids are omitted rather than erroring, which is what
    /// `SyncSource::fetch` promises.
    pub fn fetch(&self, ids: &[String]) -> Result<Vec<StoredVersion>, MetaError> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let conn = self.lock()?;
        // One statement per batch size rather than a bound-parameter loop: the
        // batch is at most `MAX_ITEMS_PER_MESSAGE`.
        let placeholders = std::iter::repeat_n("?", ids.len())
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "SELECT ci.id, ci.content_ciphertext, ci.nonce, ci.content_type, ci.content_hash, \
                    ci.created_at, ci.deleted, o.origin_device_id \
               FROM clipboard_items ci \
               LEFT JOIN sync_item_origin o ON o.item_id = ci.id \
              WHERE ci.is_sensitive = 0 AND ci.id IN ({placeholders})"
        );
        let mut stmt = conn.prepare(&sql)?;
        let params = rusqlite::params_from_iter(ids.iter());
        let rows = stmt.query_map(params, |row| {
            let origin: Option<String> = row.get(7)?;
            Ok(StoredVersion {
                item_id: row.get(0)?,
                content_ciphertext: row.get(1)?,
                nonce: row.get(2)?,
                content_type: row.get(3)?,
                content_hash: row.get(4)?,
                created_at: row.get(5)?,
                deleted: row.get::<_, i64>(6)? != 0,
                origin_device_id: origin.unwrap_or_else(|| self.device_id.clone()),
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Record which device first captured an item.
    ///
    /// Called on every local ingest. Idempotent, and deliberately *not* an
    /// update: an origin is the device an item was born on, and restamping it
    /// on a later hop destroys the merge tie-break's determinism across three
    /// devices (`protocol::SyncItem::origin_device_id`).
    pub fn record_origin(&self, item_id: &str, origin_device_id: &str) -> Result<(), MetaError> {
        let conn = self.lock()?;
        conn.execute(
            "INSERT INTO sync_item_origin (item_id, origin_device_id) VALUES (?1, ?2) \
             ON CONFLICT(item_id) DO NOTHING",
            params![item_id, origin_device_id],
        )?;
        Ok(())
    }

    /// Write the version a session decided should win.
    ///
    /// One transaction, and the only write path this module has. Returns
    /// `false` when the store refused the row, which happens for exactly one
    /// reason: the history table's dedup index already holds a *different* id
    /// with this content in the same 60-second bucket. Refusing is the safe
    /// direction — the content is already on this device under another id — and
    /// the caller reports it as skipped rather than failing the session.
    ///
    /// What it preserves deliberately: `pinned` and `pin_order` are local
    /// decisions and never come off the wire, so an incoming version keeps
    /// whatever pin this device has. A tombstone clears them, matching
    /// `Store::delete`.
    pub fn apply(&self, incoming: &IncomingVersion<'_>) -> Result<bool, MetaError> {
        let mut conn = self.lock()?;
        let tx = conn.transaction()?;

        let written = tx.execute(
            "INSERT INTO clipboard_items \
                 (id, content_ciphertext, nonce, content_type, content_hash, \
                  is_sensitive, pinned, pin_order, created_at, deleted) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, NULL, ?7, ?8) \
             ON CONFLICT(id) DO UPDATE SET \
                 content_ciphertext = excluded.content_ciphertext, \
                 nonce              = excluded.nonce, \
                 content_type       = excluded.content_type, \
                 content_hash       = excluded.content_hash, \
                 is_sensitive       = excluded.is_sensitive, \
                 created_at         = excluded.created_at, \
                 deleted            = excluded.deleted, \
                 pinned    = CASE WHEN excluded.deleted = 1 THEN 0 ELSE clipboard_items.pinned END, \
                 pin_order = CASE WHEN excluded.deleted = 1 THEN NULL ELSE clipboard_items.pin_order END",
            params![
                incoming.item_id,
                incoming.content_ciphertext,
                incoming.nonce,
                incoming.content_type,
                incoming.content_hash,
                incoming.is_sensitive,
                incoming.created_at,
                incoming.deleted,
            ],
        );

        match written {
            Ok(_) => {}
            Err(e) if is_constraint_violation(&e) => {
                tracing::warn!(
                    "an incoming item collides with the dedup index under another id; skipping it"
                );
                return Ok(false);
            }
            Err(e) => return Err(e.into()),
        }

        // Unconditional, so a version that arrives sensitive, deleted, or
        // simply changed cannot leave a stale index row behind. This is the
        // write-time layer of "sensitive items never reach the search index"
        // for the sync path (`CLAUDE.md` rule 4).
        tx.execute(
            "DELETE FROM clipboard_fts WHERE id = ?1",
            [incoming.item_id],
        )?;
        if !incoming.deleted && !incoming.is_sensitive {
            if let Some(text) = incoming.search_text.filter(|t| !t.trim().is_empty()) {
                tx.execute(
                    "INSERT INTO clipboard_fts (id, content_text) VALUES (?1, ?2)",
                    params![incoming.item_id, text],
                )?;
            }
        }

        tx.execute(
            "INSERT INTO sync_item_origin (item_id, origin_device_id) VALUES (?1, ?2) \
             ON CONFLICT(item_id) DO UPDATE SET origin_device_id = excluded.origin_device_id",
            params![incoming.item_id, incoming.origin_device_id],
        )?;

        tx.commit()?;
        Ok(true)
    }

    /// Poisoning is surfaced rather than recovered: this connection decides
    /// what leaves the device, and a map observed mid-update is not a basis for
    /// that decision.
    fn lock(&self) -> Result<MutexGuard<'_, Connection>, MetaError> {
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

fn is_not_a_database(e: &rusqlite::Error) -> bool {
    matches!(
        e,
        rusqlite::Error::SqliteFailure(err, _)
            if err.code == rusqlite::ErrorCode::NotADatabase
    )
}

fn is_constraint_violation(e: &rusqlite::Error) -> bool {
    matches!(
        e,
        rusqlite::Error::SqliteFailure(err, _)
            if err.code == rusqlite::ErrorCode::ConstraintViolation
    )
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
    use copypaste_core::{Keyring, NewItem, Store};

    struct Fixture {
        _dir: tempfile::TempDir,
        store: Store,
        meta: Meta,
    }

    fn fixture() -> Fixture {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("copypaste-v2.db");
        let keyring = Keyring::from_secret(&[7u8; 32]);
        let store = Store::open(&path, &keyring.db_key()).expect("store");
        let meta = Meta::open(&path, &keyring.db_key(), "test-device").expect("meta");
        Fixture {
            _dir: dir,
            store,
            meta,
        }
    }

    fn insert(store: &Store, id: &str, hash: &str, created_at: i64, sensitive: bool) {
        store
            .insert(NewItem {
                id: id.to_string(),
                content_ciphertext: vec![1, 2, 3],
                nonce: vec![4, 5, 6],
                content_type: "text".into(),
                content_hash: hash.to_string(),
                is_sensitive: sensitive,
                search_text: if sensitive {
                    None
                } else {
                    Some("indexed text".into())
                },
                created_at,
            })
            .expect("insert");
    }

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
    fn summaries_exclude_sensitive_items_and_include_tombstones() {
        let f = fixture();
        insert(&f.store, "plain", "hash-plain", 1_000, false);
        insert(&f.store, "secret", "hash-secret", 2_000, true);
        insert(&f.store, "gone", "hash-gone", 3_000, false);
        assert!(f.store.delete("gone").unwrap());

        let summaries = f.meta.summaries().unwrap();
        let ids: Vec<&str> = summaries.iter().map(|s| s.item_id.as_str()).collect();
        assert!(ids.contains(&"plain"), "{ids:?}");
        assert!(
            ids.contains(&"gone"),
            "a tombstone is a version, not an absence"
        );
        assert!(
            !ids.contains(&"secret"),
            "a sensitive item must never be advertised"
        );

        let tombstone = summaries.iter().find(|s| s.item_id == "gone").unwrap();
        assert!(tombstone.deleted);
        assert_eq!(tombstone.content_hash, "hash-gone");
    }

    #[test]
    fn fetch_omits_sensitive_and_unknown_ids() {
        let f = fixture();
        insert(&f.store, "plain", "hash-plain", 1_000, false);
        insert(&f.store, "secret", "hash-secret", 2_000, true);

        let rows = f
            .meta
            .fetch(&[
                "plain".to_string(),
                "secret".to_string(),
                "never-existed".to_string(),
            ])
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].item_id, "plain");
        assert_eq!(rows[0].content_ciphertext.as_deref(), Some(&[1, 2, 3][..]));
    }

    #[test]
    fn an_item_with_no_recorded_origin_belongs_to_this_device() {
        let f = fixture();
        insert(&f.store, "plain", "hash-plain", 1_000, false);
        let local = f.meta.local_version("plain").unwrap().expect("known item");
        assert_eq!(local.origin_device_id, f.meta.device_id());
    }

    #[test]
    fn a_recorded_origin_is_never_restamped() {
        let f = fixture();
        insert(&f.store, "plain", "hash-plain", 1_000, false);
        f.meta.record_origin("plain", "device-a").unwrap();
        f.meta.record_origin("plain", "device-b").unwrap();
        let local = f.meta.local_version("plain").unwrap().unwrap();
        assert_eq!(local.origin_device_id, "device-a");
    }

    #[test]
    fn applying_an_unknown_item_stores_it_with_its_remote_identity() {
        let f = fixture();
        let applied = f
            .meta
            .apply(&IncomingVersion {
                item_id: "from-peer",
                content_ciphertext: Some(&[9, 9, 9]),
                nonce: Some(&[8, 8, 8]),
                content_type: "text",
                content_hash: "hash-remote",
                created_at: 5_000,
                deleted: false,
                is_sensitive: false,
                origin_device_id: "device-a",
                search_text: Some("remote text"),
            })
            .unwrap();
        assert!(applied);

        let stored = f.store.get("from-peer").unwrap().expect("stored");
        assert_eq!(stored.created_at, 5_000);
        let local = f.meta.local_version("from-peer").unwrap().unwrap();
        assert_eq!(local.origin_device_id, "device-a");
        assert_eq!(local.summary.content_hash, "hash-remote");
        assert!(!f.store.search("remote", 10).unwrap().is_empty());
    }

    #[test]
    fn applying_a_sensitive_version_keeps_it_out_of_the_index() {
        let f = fixture();
        f.meta
            .apply(&IncomingVersion {
                item_id: "flagged",
                content_ciphertext: Some(&[1]),
                nonce: Some(&[2]),
                content_type: "text",
                content_hash: "hash-flagged",
                created_at: 5_000,
                deleted: false,
                is_sensitive: true,
                origin_device_id: "device-a",
                // Even when a caller supplies it, as the store's own layer does.
                search_text: Some("AKIAIOSFODNN7EXAMPLE"),
            })
            .unwrap();

        assert!(f
            .store
            .search("AKIAIOSFODNN7EXAMPLE", 10)
            .unwrap()
            .is_empty());
        assert!(f.store.get("flagged").unwrap().is_some());
    }

    #[test]
    fn applying_a_tombstone_over_a_live_item_wipes_it_and_unindexes_it() {
        let f = fixture();
        insert(&f.store, "doomed", "hash-doomed", 1_000, false);
        f.store.set_pinned("doomed", true).unwrap();

        f.meta
            .apply(&IncomingVersion {
                item_id: "doomed",
                content_ciphertext: None,
                nonce: None,
                content_type: "text",
                content_hash: "hash-doomed",
                created_at: 2_000,
                deleted: true,
                is_sensitive: false,
                origin_device_id: "device-a",
                search_text: None,
            })
            .unwrap();

        assert!(f.store.get("doomed").unwrap().is_none(), "still live");
        assert!(f.store.search("indexed", 10).unwrap().is_empty());
        let local = f.meta.local_version("doomed").unwrap().unwrap();
        assert!(local.summary.deleted);
    }

    #[test]
    fn a_pin_is_local_and_survives_an_incoming_version() {
        let f = fixture();
        insert(&f.store, "kept", "hash-kept", 1_000, false);
        f.store.set_pinned("kept", true).unwrap();

        f.meta
            .apply(&IncomingVersion {
                item_id: "kept",
                content_ciphertext: Some(&[3]),
                nonce: Some(&[4]),
                content_type: "text",
                content_hash: "hash-newer",
                created_at: 9_000,
                deleted: false,
                is_sensitive: false,
                origin_device_id: "device-a",
                search_text: Some("newer text"),
            })
            .unwrap();

        assert!(f.store.get("kept").unwrap().unwrap().pinned);
    }

    #[test]
    fn a_collision_with_the_dedup_index_is_refused_not_an_error() {
        let f = fixture();
        insert(&f.store, "mine", "same-hash", 1_000, false);

        // Same content hash, same 60-second bucket, different id: the store's
        // dedup index owns that pair.
        let applied = f
            .meta
            .apply(&IncomingVersion {
                item_id: "theirs",
                content_ciphertext: Some(&[1]),
                nonce: Some(&[2]),
                content_type: "text",
                content_hash: "same-hash",
                created_at: 1_500,
                deleted: false,
                is_sensitive: false,
                origin_device_id: "device-a",
                search_text: Some("text"),
            })
            .unwrap();
        assert!(!applied, "the collision must be reported, not stored");
        assert!(f.store.get("theirs").unwrap().is_none());
        assert!(f.store.get("mine").unwrap().is_some(), "no local data lost");
    }

    #[test]
    fn error_messages_contain_no_paths() {
        for message in [
            MetaError::InvalidKey.to_string(),
            MetaError::Poisoned.to_string(),
            MetaError::Sqlite(rusqlite::Error::QueryReturnedNoRows).to_string(),
        ] {
            assert!(!message.contains('/'), "{message}");
            assert!(!message.contains('\\'), "{message}");
        }
    }

    #[test]
    fn a_device_name_is_bounded_and_stripped() {
        assert_eq!(sanitise_name("  laptop\n "), "laptop");
        assert_eq!(sanitise_name(""), "CopyPaste device");
        assert!(sanitise_name(&"x".repeat(500)).len() <= 128);
    }
}
