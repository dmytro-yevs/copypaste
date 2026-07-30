//! Storage — the single durable store of clipboard history on a device.
//!
//! One SQLCipher-encrypted SQLite file, one schema version, one r2d2 pool.
//!
//! # What this module carries over from v1 (port manifest 03)
//!
//! * **Sensitive items never reach the search index** (ADR-015). Three
//!   enforcement layers, all required, all present here: the write guard in
//!   [`Store::insert`], the in-transaction re-read in [`upsert_fts_in_tx`], and
//!   the `is_sensitive = 0` predicate on the search JOIN. v1 shipped databases
//!   containing plaintext passwords in FTS because one layer was missing.
//! * **Tombstones.** [`Store::delete`] soft-deletes: the ciphertext and nonce
//!   are wiped, the FTS row is removed in the same transaction, and the row
//!   survives as a delete event that a later sync/LWW layer can propagate.
//!   `content_hash` is deliberately kept on the tombstone, which is why every
//!   dedup query filters `deleted = 0` — without that a re-copy of a deleted
//!   item could never come back (`CopyPaste-crh3.67`, `CopyPaste-fuxl`).
//! * **Pinned items are never auto-deleted** — every eviction path filters
//!   `pinned = 0`.
//! * **Total ordering.** Every list query ends in an `id` tiebreak so the sort
//!   is total; that is what keyset pagination requires and what keeps offset
//!   pages from duplicating or skipping rows that tie on `created_at`.
//! * **Errors never contain a filesystem path** (CLAUDE.md rule 4 — the path
//!   discloses the local username). Nothing in [`StoreError`] formats a path,
//!   and the underlying `rusqlite` errors do not carry one either.
//!
//! # What this module deliberately does *not* carry over
//!
//! v2 drops backward compatibility (`docs/rewrite/port-manifest/README.md`), so
//! the v1→v15 migration ladder, `migration_state`, `key_version` dispatch and
//! the `(wall_time / 60)` millisecond dedup bucket are gone. The schema starts
//! clean at version 1 and the ladder is owned by `rusqlite_migration` rather
//! than by a hand-rolled `user_version` runner with a retry loop that
//! string-matched `"duplicate column name"`.

use std::path::Path;
use std::sync::LazyLock;
use std::time::Duration;

use r2d2::{Pool, PooledConnection};
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{params, Connection, ErrorCode, OptionalExtension, Row, Transaction};
use rusqlite_migration::{Migrations, M};
use zeroize::Zeroizing;

/// Dedup window: two captures of identical content inside this interval are the
/// same clipboard event.
///
/// A genuine 60-second interval. v1 bucketed on `(wall_time / 60)` where
/// `wall_time` is *milliseconds*, so its "minute" bucket was really 60 ms; that
/// wart is reference-only now.
pub const DEDUP_WINDOW_MS: i64 = 60_000;

/// Width of the storage-level dedup bucket, in milliseconds. Kept equal to
/// [`DEDUP_WINDOW_MS`] so the UNIQUE-index backstop and the application probe
/// agree about what "the same clipboard event" means.
const DEDUP_BUCKET_MS: i64 = DEDUP_WINDOW_MS;

/// WAL gives many concurrent readers and one writer; four connections is what
/// the daemon needed in v1 (`CopyPaste-j8p`).
const POOL_SIZE: u32 = 4;

/// Per-connection pragmas, applied in order *after* `PRAGMA key`.
///
/// These are connection-scoped and are not persisted, so they must be
/// re-applied to every connection the pool creates.
const CONNECTION_PRAGMAS: &[&str] = &[
    // Without it a reader and the writer race instantly and surface a silent
    // SQLITE_BUSY.
    "PRAGMA busy_timeout = 5000",
    "PRAGMA journal_mode = WAL",
    "PRAGMA synchronous = NORMAL",
    "PRAGMA foreign_keys = ON",
    // Keeps temp B-trees (which hold decrypted intermediates) off the disk.
    "PRAGMA temp_store = MEMORY",
    "PRAGMA wal_autocheckpoint = 1000",
    "PRAGMA journal_size_limit = 67108864",
    "PRAGMA cache_size = -8192",
];

/// Schema v1 — the whole schema, created in one step on a fresh database.
///
/// No `IF NOT EXISTS` guards and no `pragma_table_info` probes: the version
/// marker is `rusqlite_migration`'s business now, and every statement here runs
/// exactly once, inside its transaction.
const SCHEMA_V1: &str = r#"
CREATE TABLE clipboard_items (
    id                 TEXT    PRIMARY KEY NOT NULL,
    -- NULL only on a tombstone: soft delete wipes the payload.
    content_ciphertext BLOB,
    nonce              BLOB,
    content_type       TEXT    NOT NULL,
    -- SHA-256 hex of the pre-encryption bytes. Kept on tombstones on purpose.
    content_hash       TEXT    NOT NULL DEFAULT '',
    is_sensitive       INTEGER NOT NULL DEFAULT 0,
    pinned             INTEGER NOT NULL DEFAULT 0,
    -- REAL so a reorder can insert between two neighbours without renumbering.
    pin_order          REAL,
    -- Milliseconds since the Unix epoch. Every timestamp in this schema is ms.
    created_at         INTEGER NOT NULL,
    deleted            INTEGER NOT NULL DEFAULT 0
);

-- Serves the list query verbatim: pinned first, then pin order, then newest
-- first, with an id tiebreak that makes the order total.
CREATE INDEX idx_items_history
    ON clipboard_items(pinned DESC, pin_order, created_at DESC, id DESC)
    WHERE deleted = 0;

-- Serves find_recent_by_hash.
CREATE INDEX idx_items_hash
    ON clipboard_items(content_hash, created_at DESC)
    WHERE deleted = 0;

-- TOCTOU backstop for dedup: the application probe is a SELECT-before-INSERT,
-- so two ingest events with identical content can both observe "no recent
-- row". This makes the second INSERT fail; insert() then re-reads the winner
-- inside the same transaction and returns it, which is what makes dedup
-- idempotent. Excludes empty hashes so hash-less rows do not all collide, and
-- excludes tombstones so a re-copy after a delete is a fresh live row.
CREATE UNIQUE INDEX idx_items_dedup
    ON clipboard_items(content_hash, created_at / 60000)
    WHERE deleted = 0 AND content_hash <> '';

-- Serves the eviction scans (oldest unpinned live rows first).
CREATE INDEX idx_items_evictable
    ON clipboard_items(created_at, id)
    WHERE deleted = 0 AND pinned = 0;

-- External-content mode is NOT used: there is no cascade from clipboard_items,
-- so every delete path must remove the FTS row explicitly, in the same
-- transaction as the row change.
CREATE VIRTUAL TABLE clipboard_fts USING fts5(id UNINDEXED, content_text);
"#;

static MIGRATIONS: LazyLock<Migrations<'static>> =
    LazyLock::new(|| Migrations::new(vec![M::up(SCHEMA_V1)]));

/// The projection every read uses. Bound by *name*, never by position — v1
/// maintained three parallel positional column lists and an off-by-one panic in
/// the row mapper was the result (`CopyPaste-crh3.85`).
macro_rules! item_columns {
    () => {
        "id, content_ciphertext, nonce, content_type, created_at, pinned, is_sensitive"
    };
}

/// The same projection through the `ci` alias used by the FTS JOIN, aliased back
/// to the bare names so one row mapper serves both.
macro_rules! item_columns_ci {
    () => {
        "ci.id AS id, ci.content_ciphertext AS content_ciphertext, ci.nonce AS nonce, \
         ci.content_type AS content_type, ci.created_at AS created_at, \
         ci.pinned AS pinned, ci.is_sensitive AS is_sensitive"
    };
}

/// An item on its way into the store.
pub struct NewItem {
    /// Primary key, chosen by the caller — not generated here.
    ///
    /// The item AEAD binds this id as associated data, so the ciphertext in
    /// `content_ciphertext` was sealed against it and cannot be moved to a row
    /// with a different id. That means the id has to exist before the seal, and
    /// therefore before this insert. Letting the store mint it would produce
    /// rows whose ciphertext authenticates against an id they do not have,
    /// which fails closed on every later read — a silent, total loss of the
    /// item's content.
    pub id: String,
    pub content_ciphertext: Vec<u8>,
    pub nonce: Vec<u8>,
    pub content_type: String,
    pub content_hash: String,
    pub is_sensitive: bool,
    /// Plaintext for the search index. MUST be `None` when `is_sensitive` is
    /// true. [`Store::insert`] enforces this rather than trusting it: a
    /// non-`None` value on a sensitive item is dropped (and logged), never
    /// indexed. The insert itself still succeeds — refusing it would lose the
    /// user's clipboard content, and data loss is the worst outcome.
    pub search_text: Option<String>,
    /// Milliseconds since the Unix epoch.
    pub created_at: i64,
}

/// A row as it comes back out. The plaintext never leaves the store: callers
/// decrypt `content_ciphertext` themselves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredItem {
    pub id: String,
    pub content_ciphertext: Vec<u8>,
    pub nonce: Vec<u8>,
    pub content_type: String,
    pub created_at: i64,
    pub pinned: bool,
    pub is_sensitive: bool,
}

/// Storage failures.
///
/// No variant carries a filesystem path (CLAUDE.md rule 4). `rusqlite` and
/// `r2d2` errors do not embed one either, so these are safe to show a user.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("database error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("connection pool error: {0}")]
    Pool(#[from] r2d2::Error),

    #[error("schema migration failed: {0}")]
    Migration(#[from] rusqlite_migration::Error),

    /// The supplied key does not open this database, or the file is not a
    /// database at all. Fail closed: never fall back to an unkeyed read.
    #[error("database could not be opened with the supplied key")]
    InvalidKey,

    #[error("item not found")]
    NotFound,
}

/// The clipboard store.
///
/// Cheap to clone (the pool is reference-counted) and safe to share across
/// threads.
#[derive(Clone)]
pub struct Store {
    pool: Pool<SqliteConnectionManager>,
}

impl std::fmt::Debug for Store {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Deliberately opaque: the pool's own Debug would print the database
        // path, and a path discloses the local username.
        f.write_str("Store { .. }")
    }
}

impl Store {
    /// Opens the database at `path`, creating it if absent, and migrates it to
    /// the current schema. The parent directory must already exist.
    ///
    /// `db_key` is the raw 32-byte SQLCipher key. A key that does not open an
    /// existing file yields [`StoreError::InvalidKey`]; there is no fallback
    /// read and no unkeyed plaintext probe.
    pub fn open(path: &Path, db_key: &[u8; 32]) -> Result<Self, StoreError> {
        // Probe on a private connection first so a wrong key surfaces as
        // InvalidKey instead of an opaque pool-construction failure, and so the
        // migration runs once rather than once per pooled connection.
        let mut conn = Connection::open(path)?;
        apply_key(&conn, db_key)?;
        validate_key(&conn)?;
        apply_connection_pragmas(&conn)?;
        migrate(&mut conn)?;
        drop(conn);

        let pool = build_pool(SqliteConnectionManager::file(path), *db_key, false)?;
        Ok(Self { pool })
    }

    /// Opens a private in-memory database.
    ///
    /// The pool shares one in-memory database through a named shared-cache URI
    /// (`SqliteConnectionManager::memory()` would give every pooled connection
    /// its *own* empty database), and keeps one connection permanently idle so
    /// the database is not dropped when the pool goes quiet.
    pub fn open_in_memory(db_key: &[u8; 32]) -> Result<Self, StoreError> {
        let uri = format!(
            "file:copypaste-{}?mode=memory&cache=shared",
            uuid::Uuid::new_v4()
        );
        let manager = SqliteConnectionManager::file(&uri).with_flags(
            rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE
                | rusqlite::OpenFlags::SQLITE_OPEN_CREATE
                | rusqlite::OpenFlags::SQLITE_OPEN_URI
                | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        );
        let pool = build_pool(manager, *db_key, true)?;
        let mut conn = pool.get()?;
        migrate(&mut conn)?;
        drop(conn);
        Ok(Self { pool })
    }

    /// Inserts an item and returns it as stored.
    ///
    /// Dedup: if an identical `content_hash` already occupies the same dedup
    /// bucket, no new row is written and the existing item is returned, so the
    /// call is idempotent under a race.
    pub fn insert(&self, item: NewItem) -> Result<StoredItem, StoreError> {
        // Layer 1 of the ADR-015 enforcement — unconditional, and it ignores
        // what the caller passed. A sensitive item is never indexed even if the
        // caller hands us plaintext for it.
        let search_text = if item.is_sensitive {
            if item.search_text.is_some() {
                tracing::warn!(
                    "search_text supplied for a sensitive item; dropping it (it must be None)"
                );
            }
            None
        } else {
            item.search_text.as_deref().filter(|t| !t.trim().is_empty())
        };

        // The caller's id, not a fresh one: the ciphertext is already sealed
        // against it (see `NewItem::id`).
        let id = item.id.clone();
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;

        let insert = tx.execute(
            "INSERT INTO clipboard_items \
                 (id, content_ciphertext, nonce, content_type, content_hash, \
                  is_sensitive, pinned, pin_order, created_at, deleted) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, NULL, ?7, 0)",
            params![
                &id,
                &item.content_ciphertext,
                &item.nonce,
                &item.content_type,
                &item.content_hash,
                item.is_sensitive,
                item.created_at,
            ],
        );

        match insert {
            Ok(_) => {}
            Err(e) if is_constraint_violation(&e) => {
                // The dedup backstop fired. Resolve the winner *inside* the same
                // transaction so there is no TOCTOU gap between the failed
                // INSERT and this lookup.
                let existing = find_in_bucket(&tx, &item.content_hash, item.created_at)?;
                return match existing {
                    Some(existing) => {
                        tx.rollback()?;
                        Ok(existing)
                    }
                    None => Err(e.into()),
                };
            }
            Err(e) => return Err(e.into()),
        }

        if let Some(text) = search_text {
            upsert_fts_in_tx(&tx, &id, text)?;
        }
        tx.commit()?;

        Ok(StoredItem {
            id,
            content_ciphertext: item.content_ciphertext,
            nonce: item.nonce,
            content_type: item.content_type,
            created_at: item.created_at,
            pinned: false,
            is_sensitive: item.is_sensitive,
        })
    }

    /// Pinned first, then newest first.
    ///
    /// The order is *total* (`pinned DESC, pin_order, created_at DESC, id DESC`)
    /// — the trailing `id` tiebreak is what a keyset cursor would seek on, and
    /// it keeps offset pages stable when rows tie on `created_at`.
    pub fn list(&self, limit: u32, offset: u32) -> Result<Vec<StoredItem>, StoreError> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare_cached(concat!(
            "SELECT ",
            item_columns!(),
            " FROM clipboard_items WHERE deleted = 0 \
              ORDER BY pinned DESC, pin_order ASC, created_at DESC, id DESC \
              LIMIT ?1 OFFSET ?2"
        ))?;
        let rows = stmt.query_map(params![i64::from(limit), i64::from(offset)], row_to_item)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Full-text search over non-sensitive items, best match first.
    ///
    /// Never returns a sensitive item: the JOIN filters `is_sensitive = 0` even
    /// though the write path already refuses to index one. That is layer 3 — a
    /// stale FTS row (from test tooling, or from a database written before a
    /// fix) can never surface.
    pub fn search(&self, query: &str, limit: u32) -> Result<Vec<StoredItem>, StoreError> {
        let Some(match_expr) = sanitize_fts5_query(query) else {
            return Ok(Vec::new());
        };
        let conn = self.conn()?;
        let mut stmt = conn.prepare_cached(concat!(
            "SELECT ",
            item_columns_ci!(),
            " FROM clipboard_fts fts \
              JOIN clipboard_items ci ON ci.id = fts.id \
              WHERE clipboard_fts MATCH ?1 AND ci.deleted = 0 AND ci.is_sensitive = 0 \
              ORDER BY rank LIMIT ?2"
        ))?;
        let rows = stmt.query_map(params![match_expr, i64::from(limit)], row_to_item)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Fetches one live item. Tombstones are not live and return `None`.
    pub fn get(&self, id: &str) -> Result<Option<StoredItem>, StoreError> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare_cached(concat!(
            "SELECT ",
            item_columns!(),
            " FROM clipboard_items WHERE id = ?1 AND deleted = 0"
        ))?;
        Ok(stmt.query_row([id], row_to_item).optional()?)
    }

    /// Soft-deletes an item, returning whether a live row was affected.
    ///
    /// The ciphertext and nonce are wiped and the FTS row is removed in the same
    /// transaction; the row survives as a tombstone so a later sync layer can
    /// propagate the delete instead of resurrecting the item.
    pub fn delete(&self, id: &str) -> Result<bool, StoreError> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        let changed = tx.execute(
            "UPDATE clipboard_items \
                SET deleted = 1, content_ciphertext = NULL, nonce = NULL, \
                    pinned = 0, pin_order = NULL \
              WHERE id = ?1 AND deleted = 0",
            [id],
        )?;
        // Unconditional: this also repairs a stale row left by an earlier
        // partial failure.
        tx.execute("DELETE FROM clipboard_fts WHERE id = ?1", [id])?;
        tx.commit()?;
        Ok(changed > 0)
    }

    /// Soft-deletes every live item, returning how many were affected.
    pub fn delete_all(&self) -> Result<u64, StoreError> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        // Pinned rows survive. Manifest 04 is explicit that delete_all
        // tombstones non-pinned rows only, and pinning is the one gesture by
        // which a user says "keep this" — clearing history must not be the
        // thing that discards it.
        let changed = tx.execute(
            "UPDATE clipboard_items \
                SET deleted = 1, content_ciphertext = NULL, nonce = NULL, \
                    pin_order = NULL \
              WHERE deleted = 0 AND pinned = 0",
            [],
        )?;
        // Drop index rows for everything that is no longer live, which also
        // sweeps orphans. Pinned rows stay live and keep their entries.
        tx.execute(
            "DELETE FROM clipboard_fts \
              WHERE id NOT IN (SELECT id FROM clipboard_items WHERE deleted = 0)",
            [],
        )?;
        tx.commit()?;
        Ok(changed as u64)
    }

    /// Pins or unpins an item. Returns `false` if there is no such live item.
    ///
    /// Pinning is idempotent; a newly pinned item lands at the end of the pinned
    /// section.
    pub fn set_pinned(&self, id: &str, pinned: bool) -> Result<bool, StoreError> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        let exists: Option<i64> = tx
            .query_row(
                "SELECT 1 FROM clipboard_items WHERE id = ?1 AND deleted = 0",
                [id],
                |r| r.get(0),
            )
            .optional()?;
        if exists.is_none() {
            return Ok(false);
        }
        if pinned {
            tx.execute(
                "UPDATE clipboard_items \
                    SET pinned = 1, \
                        pin_order = (SELECT COALESCE(MAX(pin_order), 0) + 1 \
                                       FROM clipboard_items \
                                      WHERE pinned = 1 AND deleted = 0) \
                  WHERE id = ?1 AND deleted = 0 AND pinned = 0",
                [id],
            )?;
        } else {
            tx.execute(
                "UPDATE clipboard_items SET pinned = 0, pin_order = NULL \
                  WHERE id = ?1 AND deleted = 0",
                [id],
            )?;
        }
        tx.commit()?;
        Ok(true)
    }

    /// Number of live items. Tombstones do not count.
    pub fn count(&self) -> Result<u64, StoreError> {
        let conn = self.conn()?;
        let n: i64 = conn.query_row(
            "SELECT COUNT(*) FROM clipboard_items WHERE deleted = 0",
            [],
            |r| r.get(0),
        )?;
        Ok(n.max(0) as u64)
    }

    /// Most recent live item with this content hash at or after `since_ms`.
    ///
    /// Callers pass `now_ms - DEDUP_WINDOW_MS`. `deleted = 0` is mandatory:
    /// tombstones keep their `content_hash`, and without the filter a re-copy of
    /// a deleted item would match the tombstone and never come back.
    pub fn find_recent_by_hash(
        &self,
        content_hash: &str,
        since_ms: i64,
    ) -> Result<Option<StoredItem>, StoreError> {
        if content_hash.is_empty() {
            return Ok(None);
        }
        let conn = self.conn()?;
        let mut stmt = conn.prepare_cached(concat!(
            "SELECT ",
            item_columns!(),
            " FROM clipboard_items \
              WHERE content_hash = ?1 AND created_at >= ?2 AND deleted = 0 \
              ORDER BY created_at DESC, id DESC LIMIT 1"
        ))?;
        Ok(stmt
            .query_row(params![content_hash, since_ms], row_to_item)
            .optional()?)
    }

    /// Enforces the history cap: hard-deletes the oldest unpinned items until at
    /// most `max_items` live items remain. Returns how many were removed.
    ///
    /// Two rules carried from v1, both load-bearing:
    ///
    /// * **Pinned items are never evicted** (I9). If pins alone exceed the cap,
    ///   the cap is simply not reached.
    /// * **The newest unpinned item is never evicted**, so "user copies a huge
    ///   item and it instantly vanishes" cannot happen.
    ///
    /// Eviction is a hard delete, not a tombstone: it is local housekeeping, not
    /// a user-visible delete event to propagate.
    pub fn evict_over_cap(&self, max_items: u64) -> Result<u64, StoreError> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;

        let live: i64 = tx.query_row(
            "SELECT COUNT(*) FROM clipboard_items WHERE deleted = 0",
            [],
            |r| r.get(0),
        )?;
        let live = live.max(0) as u64;
        if live <= max_items {
            return Ok(0);
        }
        let excess = live - max_items;

        // Empty-string sentinel: `id <> ''` matches every real row.
        let keep_id: String = tx
            .query_row(
                "SELECT id FROM clipboard_items WHERE deleted = 0 AND pinned = 0 \
                  ORDER BY created_at DESC, id DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .optional()?
            .unwrap_or_default();

        let victims: Vec<String> = {
            let mut stmt = tx.prepare(
                "SELECT id FROM clipboard_items \
                  WHERE deleted = 0 AND pinned = 0 AND id <> ?1 \
                  ORDER BY created_at ASC, id ASC LIMIT ?2",
            )?;
            let rows = stmt.query_map(params![keep_id, excess as i64], |r| r.get(0))?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };

        let removed = hard_delete_in_tx(&tx, &victims)?;
        tx.commit()?;
        Ok(removed)
    }

    /// Enforces a TTL: hard-deletes unpinned live items created before
    /// `cutoff_ms`. Returns how many were removed.
    ///
    /// Pinned items are never TTL-deleted (I9).
    pub fn evict_older_than(&self, cutoff_ms: i64) -> Result<u64, StoreError> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        let victims: Vec<String> = {
            let mut stmt = tx.prepare(
                "SELECT id FROM clipboard_items \
                  WHERE deleted = 0 AND pinned = 0 AND created_at < ?1",
            )?;
            let rows = stmt.query_map([cutoff_ms], |r| r.get(0))?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };
        let removed = hard_delete_in_tx(&tx, &victims)?;
        tx.commit()?;
        Ok(removed)
    }

    fn conn(&self) -> Result<PooledConnection<SqliteConnectionManager>, StoreError> {
        Ok(self.pool.get()?)
    }
}

// ---------------------------------------------------------------------------
// Connection setup
// ---------------------------------------------------------------------------

fn build_pool(
    manager: SqliteConnectionManager,
    db_key: [u8; 32],
    in_memory: bool,
) -> Result<Pool<SqliteConnectionManager>, StoreError> {
    // The key has to live as long as the pool: every connection the pool opens
    // later needs it. That is inherent to pooling an encrypted database.
    let manager = manager.with_init(move |conn| {
        apply_key(conn, &db_key)?;
        apply_connection_pragmas(conn)
    });

    let mut builder = Pool::builder()
        .max_size(POOL_SIZE)
        .connection_timeout(Duration::from_secs(10));
    if in_memory {
        // A shared-cache in-memory database exists only while a connection to
        // it is open, so keep one open for the life of the pool.
        builder = builder
            .min_idle(Some(1))
            .idle_timeout(None)
            .max_lifetime(None);
    }
    Ok(builder.build(manager)?)
}

/// `PRAGMA key` in SQLCipher raw-key form. **Must be the first statement on
/// every connection**, before any other pragma or query.
///
/// The `x'<64 lowercase hex>'` form means SQLCipher takes the 32 bytes as the
/// page key directly and skips PBKDF2 over a passphrase. No other cipher
/// parameter is set: the SQLCipher 4 defaults are what we want, and setting
/// `cipher_page_size` / `kdf_iter` / `cipher_hmac_algorithm` would change the
/// derived key or the page layout.
fn apply_key(conn: &Connection, db_key: &[u8; 32]) -> rusqlite::Result<()> {
    let key_hex = Zeroizing::new(hex::encode(db_key));
    let stmt = Zeroizing::new(format!("PRAGMA key = \"x'{}'\"", key_hex.as_str()));
    run_pragma(conn, &stmt)
}

fn apply_connection_pragmas(conn: &Connection) -> rusqlite::Result<()> {
    for pragma in CONNECTION_PRAGMAS {
        run_pragma(conn, pragma)?;
    }
    Ok(())
}

/// Runs a pragma, tolerating the ones that return a row (`journal_mode`,
/// `busy_timeout`, …). `Connection::execute` rejects those.
fn run_pragma(conn: &Connection, sql: &str) -> rusqlite::Result<()> {
    let mut stmt = conn.prepare(sql)?;
    let mut rows = stmt.query([])?;
    while rows.next()?.is_some() {}
    Ok(())
}

/// Proves the key is right before anything else touches the file. Fails closed:
/// a wrong key is an error, never a fallback to an unkeyed read.
fn validate_key(conn: &Connection) -> Result<(), StoreError> {
    match conn.query_row("SELECT COUNT(*) FROM sqlite_master", [], |r| {
        r.get::<_, i64>(0)
    }) {
        Ok(_) => Ok(()),
        Err(e) if is_not_a_database(&e) => Err(StoreError::InvalidKey),
        Err(e) => Err(e.into()),
    }
}

/// Runs the ladder. `rusqlite_migration` owns `PRAGMA user_version`; a database
/// written by a *newer* schema is refused rather than silently downgraded.
fn migrate(conn: &mut Connection) -> Result<(), StoreError> {
    MIGRATIONS.to_latest(conn)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Row mapping and small helpers
// ---------------------------------------------------------------------------

fn row_to_item(row: &Row<'_>) -> rusqlite::Result<StoredItem> {
    Ok(StoredItem {
        id: row.get("id")?,
        // NULL only on a tombstone, which no read path returns; map defensively
        // rather than failing the whole query.
        content_ciphertext: row
            .get::<_, Option<Vec<u8>>>("content_ciphertext")?
            .unwrap_or_default(),
        nonce: row.get::<_, Option<Vec<u8>>>("nonce")?.unwrap_or_default(),
        content_type: row.get("content_type")?,
        created_at: row.get("created_at")?,
        pinned: row.get("pinned")?,
        is_sensitive: row.get("is_sensitive")?,
    })
}

/// Layer 2 of ADR-015: re-read `is_sensitive` **inside the same transaction** as
/// the FTS write, so a concurrent update that flips an item to sensitive cannot
/// slip its plaintext into the index. A missing row is also a no-op.
///
/// FTS5 has no `ON CONFLICT`, so the upsert idiom is DELETE + INSERT; both run
/// in this transaction, so a crash cannot leave an item permanently
/// unsearchable.
fn upsert_fts_in_tx(tx: &Transaction<'_>, id: &str, text: &str) -> rusqlite::Result<()> {
    let sensitive: Option<bool> = tx
        .query_row(
            "SELECT is_sensitive FROM clipboard_items WHERE id = ?1 AND deleted = 0",
            [id],
            |r| r.get(0),
        )
        .optional()?;
    if sensitive != Some(false) {
        return Ok(());
    }
    tx.execute("DELETE FROM clipboard_fts WHERE id = ?1", [id])?;
    tx.execute(
        "INSERT INTO clipboard_fts (id, content_text) VALUES (?1, ?2)",
        params![id, text],
    )?;
    Ok(())
}

/// Resolves the row that won a dedup race, inside the caller's transaction.
fn find_in_bucket(
    tx: &Transaction<'_>,
    content_hash: &str,
    created_at: i64,
) -> rusqlite::Result<Option<StoredItem>> {
    if content_hash.is_empty() {
        return Ok(None);
    }
    let mut stmt = tx.prepare(concat!(
        "SELECT ",
        item_columns!(),
        " FROM clipboard_items \
          WHERE content_hash = ?1 AND created_at / ?2 = ?3 AND deleted = 0 \
          ORDER BY created_at DESC, id DESC LIMIT 1"
    ))?;
    stmt.query_row(
        params![content_hash, DEDUP_BUCKET_MS, created_at / DEDUP_BUCKET_MS],
        row_to_item,
    )
    .optional()
}

/// Hard-deletes rows and their FTS entries in the caller's transaction, so the
/// index can never drift from the table.
fn hard_delete_in_tx(tx: &Transaction<'_>, ids: &[String]) -> rusqlite::Result<u64> {
    if ids.is_empty() {
        return Ok(0);
    }
    let mut del_fts = tx.prepare("DELETE FROM clipboard_fts WHERE id = ?1")?;
    let mut del_item = tx.prepare("DELETE FROM clipboard_items WHERE id = ?1")?;
    let mut removed = 0u64;
    for id in ids {
        del_fts.execute([id])?;
        removed += del_item.execute([id])? as u64;
    }
    Ok(removed)
}

fn is_constraint_violation(err: &rusqlite::Error) -> bool {
    matches!(err, rusqlite::Error::SqliteFailure(e, _)
        if e.code == ErrorCode::ConstraintViolation)
}

fn is_not_a_database(err: &rusqlite::Error) -> bool {
    matches!(err, rusqlite::Error::SqliteFailure(e, _)
        if e.code == ErrorCode::NotADatabase || e.code == ErrorCode::DatabaseCorrupt)
}

/// The content hash used for dedup: the **full** 64-character lowercase
/// SHA-256 hex digest of the raw, pre-encryption bytes.
///
/// Never truncate it. A v1 daemon helper cut the digest to 16 bytes / 32 hex
/// chars, which threw away second-preimage resistance for nothing
/// (`CopyPaste-y4v1`). Hashing the content only — not the source app or any
/// other metadata — is what makes the same text copied from two apps dedup.
pub fn compute_content_hash(raw: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(raw))
}

/// Turns arbitrary user input into an FTS5 MATCH expression, or `None` when
/// there is nothing left to search for.
///
/// A whitelist tokenizer, not an escaper. Each rule here was a reported bug in
/// v1:
///
/// * `-` becomes a space *first*: FTS5 reads `-bar` as a column filter and
///   errors with "no such column: bar", so `foo-bar` must become
///   `foo* AND bar*`.
/// * Only alphanumerics (Unicode, so Cyrillic/CJK survive), `_`, `"`, `*` and
///   whitespace are kept.
/// * An odd number of quotes is an unclosed phrase — an FTS5 syntax error — so
///   all quotes are dropped.
/// * `*` is appended to *every* token, not just the last: search-as-you-type
///   means any token can be mid-word, and last-token-only made `"priv key"`
///   match nothing.
fn sanitize_fts5_query(raw: &str) -> Option<String> {
    const RESERVED: [&str; 4] = ["NOT", "OR", "AND", "NEAR"];

    let mut cleaned = String::with_capacity(raw.len());
    for ch in raw.chars() {
        match ch {
            '-' => cleaned.push(' '),
            c if c.is_alphanumeric() || matches!(c, '_' | '"' | '*' | ' ' | '\t') => {
                cleaned.push(c)
            }
            _ => {}
        }
    }

    let mut cleaned = cleaned.trim().to_string();
    if cleaned.is_empty() {
        return None;
    }
    if cleaned.matches('"').count() % 2 == 1 {
        cleaned = cleaned.replace('"', "").trim().to_string();
        if cleaned.is_empty() {
            return None;
        }
    }
    if (cleaned.len() > 1 && cleaned.starts_with('"') && cleaned.ends_with('"'))
        || cleaned.ends_with('*')
    {
        return Some(cleaned);
    }

    let tokens: Vec<String> = cleaned
        .split_whitespace()
        .filter(|t| t.chars().any(|c| c.is_alphanumeric() || c == '_'))
        .filter(|t| !RESERVED.iter().any(|r| r.eq_ignore_ascii_case(t)))
        .map(|t| {
            if t.ends_with('*') {
                t.to_string()
            } else {
                format!("{t}*")
            }
        })
        .collect();
    if tokens.is_empty() {
        return None;
    }
    Some(tokens.join(" AND "))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    const KEY: [u8; 32] = [7u8; 32];
    const OTHER_KEY: [u8; 32] = [9u8; 32];
    const T0: i64 = 1_700_000_000_000;

    fn store() -> Store {
        Store::open_in_memory(&KEY).expect("in-memory store")
    }

    /// Distinct content gets a distinct hash; that is all the dedup index needs.
    fn hash_of(text: &str) -> String {
        hex::encode(text.as_bytes())
    }

    fn item(text: &str, created_at: i64) -> NewItem {
        NewItem {
            id: uuid::Uuid::new_v4().to_string(),
            content_ciphertext: format!("ct:{text}").into_bytes(),
            nonce: vec![1u8; 24],
            content_type: "text".to_string(),
            content_hash: hash_of(text),
            is_sensitive: false,
            search_text: Some(text.to_string()),
            created_at,
        }
    }

    fn sensitive_item(text: &str, created_at: i64) -> NewItem {
        NewItem {
            is_sensitive: true,
            search_text: None,
            ..item(text, created_at)
        }
    }

    fn fts_row_count(store: &Store, id: &str) -> i64 {
        let conn = store.conn().unwrap();
        conn.query_row(
            "SELECT COUNT(*) FROM clipboard_fts WHERE id = ?1",
            [id],
            |r| r.get(0),
        )
        .unwrap()
    }

    fn fts_dump(store: &Store) -> String {
        let conn = store.conn().unwrap();
        let mut stmt = conn
            .prepare("SELECT content_text FROM clipboard_fts")
            .unwrap();
        let rows: Vec<String> = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        rows.join("\n")
    }

    #[test]
    fn insert_and_get_round_trip() {
        let s = store();
        let stored = s.insert(item("hello world", T0)).unwrap();

        let fetched = s.get(&stored.id).unwrap().expect("item is present");
        assert_eq!(fetched, stored);
        assert_eq!(fetched.content_ciphertext, b"ct:hello world");
        assert_eq!(fetched.nonce, vec![1u8; 24]);
        assert_eq!(fetched.content_type, "text");
        assert_eq!(fetched.created_at, T0);
        assert!(!fetched.pinned);
        assert!(!fetched.is_sensitive);

        assert!(s.get("no-such-id").unwrap().is_none());
    }

    #[test]
    fn list_is_pinned_first_then_newest_first() {
        let s = store();
        let oldest = s.insert(item("oldest", T0)).unwrap();
        let middle = s.insert(item("middle", T0 + 60_000)).unwrap();
        let newest = s.insert(item("newest", T0 + 120_000)).unwrap();

        let ids: Vec<_> = s.list(10, 0).unwrap().into_iter().map(|i| i.id).collect();
        assert_eq!(
            ids,
            vec![newest.id.clone(), middle.id.clone(), oldest.id.clone()]
        );

        // Pinning the oldest floats it to the top; the rest stay newest-first.
        assert!(s.set_pinned(&oldest.id, true).unwrap());
        let page = s.list(10, 0).unwrap();
        assert_eq!(page[0].id, oldest.id);
        assert!(page[0].pinned);
        assert_eq!(page[1].id, newest.id);
        assert_eq!(page[2].id, middle.id);

        // Pins keep their pin order among themselves.
        assert!(s.set_pinned(&middle.id, true).unwrap());
        let ids: Vec<_> = s.list(10, 0).unwrap().into_iter().map(|i| i.id).collect();
        assert_eq!(ids, vec![oldest.id, middle.id, newest.id]);

        // Offset pagination walks the same total order.
        let s2 = store();
        for n in 0..5 {
            s2.insert(item(&format!("item-{n}"), T0 + n * 60_000))
                .unwrap();
        }
        let all: Vec<_> = s2.list(10, 0).unwrap().into_iter().map(|i| i.id).collect();
        let page2: Vec<_> = s2.list(2, 2).unwrap().into_iter().map(|i| i.id).collect();
        assert_eq!(page2, all[2..4].to_vec());
    }

    #[test]
    fn search_finds_a_non_sensitive_item() {
        let s = store();
        let hit = s.insert(item("meeting notes for tuesday", T0)).unwrap();
        s.insert(item("unrelated payload", T0 + 60_000)).unwrap();

        let found = s.search("tuesday", 10).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, hit.id);

        // Prefix / search-as-you-type.
        assert_eq!(s.search("meet", 10).unwrap().len(), 1);
        // Multi-token: every token gets a prefix star.
        assert_eq!(s.search("meeting notes", 10).unwrap().len(), 1);
        // A hyphenated query must not error (FTS5 would read `-notes` as a
        // column filter).
        assert_eq!(s.search("meeting-notes", 10).unwrap().len(), 1);
        // No match, and queries that sanitize away to nothing.
        assert!(s.search("zzzznotpresent", 10).unwrap().is_empty());
        assert!(s.search("   ", 10).unwrap().is_empty());
        assert!(s.search("^:;", 10).unwrap().is_empty());
    }

    #[test]
    fn sensitive_items_never_reach_the_search_index() {
        let s = store();
        let secret = s
            .insert(sensitive_item("hunter2 super secret token", T0))
            .unwrap();
        let normal = s
            .insert(item("ordinary shopping list", T0 + 60_000))
            .unwrap();

        // Layer 1: the write guard ignores what the caller passed. A sensitive
        // item that arrives *with* search_text is still not indexed.
        let leaky = s
            .insert(NewItem {
                search_text: Some("leaked passphrase correcthorse".to_string()),
                ..sensitive_item("another secret", T0 + 120_000)
            })
            .unwrap();

        assert_eq!(fts_row_count(&s, &secret.id), 0);
        assert_eq!(fts_row_count(&s, &leaky.id), 0);
        assert_eq!(fts_row_count(&s, &normal.id), 1);

        // No sensitive plaintext is anywhere in the FTS table.
        let dump = fts_dump(&s);
        assert!(!dump.contains("hunter2"));
        assert!(!dump.contains("secret"));
        assert!(!dump.contains("passphrase"));
        assert!(!dump.contains("correcthorse"));
        assert!(dump.contains("shopping"));

        // Searching for the secret finds nothing.
        assert!(s.search("hunter2", 10).unwrap().is_empty());
        assert!(s.search("passphrase", 10).unwrap().is_empty());

        // Layer 3: even a stale FTS row planted directly cannot surface.
        {
            let conn = s.conn().unwrap();
            conn.execute(
                "INSERT INTO clipboard_fts (id, content_text) VALUES (?1, ?2)",
                params![&secret.id, "hunter2 super secret token"],
            )
            .unwrap();
        }
        assert_eq!(fts_row_count(&s, &secret.id), 1);
        assert!(s.search("hunter2", 10).unwrap().is_empty());
        assert!(s.search("token", 10).unwrap().is_empty());

        // Layer 2: the in-transaction re-read refuses to index a sensitive row.
        {
            let mut conn = s.conn().unwrap();
            let tx = conn.transaction().unwrap();
            tx.execute("DELETE FROM clipboard_fts WHERE id = ?1", [&secret.id])
                .unwrap();
            upsert_fts_in_tx(&tx, &secret.id, "hunter2 again").unwrap();
            tx.commit().unwrap();
        }
        assert_eq!(fts_row_count(&s, &secret.id), 0);
    }

    #[test]
    fn delete_tombstones_the_row_and_clears_the_index() {
        let s = store();
        let a = s.insert(item("first payload", T0)).unwrap();
        let b = s.insert(item("second payload", T0 + 60_000)).unwrap();

        assert!(s.delete(&a.id).unwrap());
        assert!(s.get(&a.id).unwrap().is_none());
        assert_eq!(s.count().unwrap(), 1);
        assert_eq!(fts_row_count(&s, &a.id), 0);
        assert!(s.search("first", 10).unwrap().is_empty());
        assert_eq!(s.list(10, 0).unwrap().len(), 1);

        // The tombstone survives with its payload wiped.
        {
            let conn = s.conn().unwrap();
            let (deleted, ct): (bool, Option<Vec<u8>>) = conn
                .query_row(
                    "SELECT deleted, content_ciphertext FROM clipboard_items WHERE id = ?1",
                    [&a.id],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .unwrap();
            assert!(deleted);
            assert!(ct.is_none());
        }

        // Deleting twice, or deleting an unknown id, reports "nothing done".
        assert!(!s.delete(&a.id).unwrap());
        assert!(!s.delete("no-such-id").unwrap());
        assert!(s.get(&b.id).unwrap().is_some());
    }

    #[test]
    fn delete_all_clears_every_live_item() {
        let s = store();
        for n in 0..4 {
            s.insert(item(&format!("payload {n}"), T0 + n * 60_000))
                .unwrap();
        }
        assert_eq!(s.count().unwrap(), 4);

        assert_eq!(s.delete_all().unwrap(), 4);
        assert_eq!(s.count().unwrap(), 0);
        assert!(s.list(10, 0).unwrap().is_empty());
        assert!(s.search("payload", 10).unwrap().is_empty());
        assert_eq!(fts_dump(&s), "");

        // Idempotent.
        assert_eq!(s.delete_all().unwrap(), 0);
    }

    /// Manifest 04: `delete_all` tombstones non-pinned rows only.
    ///
    /// Pinning is the one gesture by which a user says "keep this", so clearing
    /// history must not be the thing that discards it. This regression existed
    /// and was silent — every other test passed with pinned rows being wiped,
    /// because none of them pinned anything first.
    #[test]
    fn delete_all_leaves_pinned_items_intact() {
        let s = store();
        let keep = s.insert(item("pinned payload", T0)).unwrap();
        for n in 1..4 {
            s.insert(item(&format!("payload {n}"), T0 + n * 60_000))
                .unwrap();
        }
        assert!(s.set_pinned(&keep.id, true).unwrap());

        // Only the three unpinned rows are cleared.
        assert_eq!(s.delete_all().unwrap(), 3);
        assert_eq!(s.count().unwrap(), 1);

        let left = s.list(10, 0).unwrap();
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].id, keep.id);
        assert!(left[0].pinned, "the survivor must still be pinned");
        assert!(
            s.get(&keep.id).unwrap().is_some(),
            "a pinned item must survive delete_all"
        );

        // It keeps its search entry; the cleared rows lose theirs.
        assert_eq!(s.search("pinned", 10).unwrap().len(), 1);
        assert!(s.search("payload 2", 10).unwrap().is_empty());

        // A second call is a no-op rather than finally taking the pinned row.
        assert_eq!(s.delete_all().unwrap(), 0);
        assert_eq!(s.count().unwrap(), 1);
    }

    #[test]
    fn set_pinned_toggles_and_reports_existence() {
        let s = store();
        let a = s.insert(item("pin me", T0)).unwrap();

        assert!(s.set_pinned(&a.id, true).unwrap());
        assert!(s.get(&a.id).unwrap().unwrap().pinned);
        // Idempotent.
        assert!(s.set_pinned(&a.id, true).unwrap());
        assert!(s.get(&a.id).unwrap().unwrap().pinned);

        assert!(s.set_pinned(&a.id, false).unwrap());
        assert!(!s.get(&a.id).unwrap().unwrap().pinned);

        assert!(!s.set_pinned("no-such-id", true).unwrap());

        // A tombstone cannot be pinned.
        s.delete(&a.id).unwrap();
        assert!(!s.set_pinned(&a.id, true).unwrap());
    }

    #[test]
    fn count_tracks_live_items_only() {
        let s = store();
        assert_eq!(s.count().unwrap(), 0);
        let a = s.insert(item("one", T0)).unwrap();
        s.insert(item("two", T0 + 60_000)).unwrap();
        assert_eq!(s.count().unwrap(), 2);
        s.delete(&a.id).unwrap();
        assert_eq!(s.count().unwrap(), 1);
    }

    #[test]
    fn dedup_window_hit_and_miss() {
        let s = store();
        let hash = hash_of("repeated clipboard content");
        let first = s.insert(item("repeated clipboard content", T0)).unwrap();

        // Hit: the item is inside the window.
        let hit = s
            .find_recent_by_hash(&hash, (T0 + 30_000) - DEDUP_WINDOW_MS)
            .unwrap();
        assert_eq!(hit.map(|i| i.id), Some(first.id.clone()));

        // Miss: the item is older than the window.
        let miss = s
            .find_recent_by_hash(&hash, (T0 + 90_000) - DEDUP_WINDOW_MS)
            .unwrap();
        assert!(miss.is_none());

        // Miss: unknown hash.
        assert!(s
            .find_recent_by_hash(&hash_of("never copied"), T0 - DEDUP_WINDOW_MS)
            .unwrap()
            .is_none());

        // A tombstone must not satisfy the probe, or a re-copy of a deleted item
        // could never come back.
        s.delete(&first.id).unwrap();
        assert!(s
            .find_recent_by_hash(&hash, T0 - DEDUP_WINDOW_MS)
            .unwrap()
            .is_none());
        let recopy = s.insert(item("repeated clipboard content", T0)).unwrap();
        assert_ne!(recopy.id, first.id);
        assert_eq!(s.count().unwrap(), 1);
    }

    #[test]
    fn dedup_index_makes_a_same_bucket_reinsert_idempotent() {
        let s = store();
        let first = s.insert(item("same content", T0)).unwrap();
        // Same bucket (well inside 60s): no new row, the winner comes back.
        let again = s.insert(item("same content", T0 + 5_000)).unwrap();
        assert_eq!(again.id, first.id);
        assert_eq!(s.count().unwrap(), 1);

        // A later bucket is a genuinely new clipboard event.
        let later = s.insert(item("same content", T0 + 120_000)).unwrap();
        assert_ne!(later.id, first.id);
        assert_eq!(s.count().unwrap(), 2);
    }

    #[test]
    fn eviction_respects_pins() {
        let s = store();
        let mut ids = Vec::new();
        for n in 0..5 {
            ids.push(
                s.insert(item(&format!("item {n}"), T0 + n * 60_000))
                    .unwrap()
                    .id,
            );
        }
        // Pin the oldest — it must survive even though it is the first victim by
        // age.
        assert!(s.set_pinned(&ids[0], true).unwrap());

        assert_eq!(s.evict_over_cap(10).unwrap(), 0);

        let removed = s.evict_over_cap(2).unwrap();
        assert_eq!(removed, 3);
        assert_eq!(s.count().unwrap(), 2);

        // The pinned oldest survived; so did the newest unpinned item.
        assert!(s.get(&ids[0]).unwrap().is_some());
        assert!(s.get(&ids[4]).unwrap().is_some());
        for id in &ids[1..4] {
            assert!(s.get(id).unwrap().is_none());
            // Evicted rows leave no FTS orphans.
            assert_eq!(fts_row_count(&s, id), 0);
        }

        // Pins alone can hold the store over the cap; they are never evicted.
        assert_eq!(s.evict_over_cap(0).unwrap(), 0);
        assert!(s.get(&ids[0]).unwrap().is_some());
    }

    #[test]
    fn ttl_eviction_respects_pins() {
        let s = store();
        let old = s.insert(item("stale", T0)).unwrap();
        let old_pinned = s.insert(item("stale but pinned", T0 + 1)).unwrap();
        let fresh = s.insert(item("fresh", T0 + 600_000)).unwrap();
        assert!(s.set_pinned(&old_pinned.id, true).unwrap());

        assert_eq!(s.evict_older_than(T0 + 300_000).unwrap(), 1);
        assert!(s.get(&old.id).unwrap().is_none());
        assert!(s.get(&old_pinned.id).unwrap().is_some());
        assert!(s.get(&fresh.id).unwrap().is_some());
    }

    #[test]
    fn file_backed_store_round_trips_and_rejects_a_wrong_key() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("clipboard.db");

        let id = {
            let s = Store::open(&path, &KEY).unwrap();
            let stored = s.insert(item("persisted payload", T0)).unwrap();
            assert_eq!(s.count().unwrap(), 1);
            stored.id
        };

        // Re-opening with the right key is a no-op migration and keeps the data.
        let s = Store::open(&path, &KEY).unwrap();
        assert_eq!(
            s.get(&id).unwrap().unwrap().content_ciphertext,
            b"ct:persisted payload"
        );
        assert_eq!(s.search("persisted", 10).unwrap().len(), 1);
        drop(s);

        // The wrong key must fail closed — never a fallback read.
        let err = Store::open(&path, &OTHER_KEY).unwrap_err();
        assert!(
            matches!(err, StoreError::InvalidKey),
            "expected InvalidKey, got {err:?}"
        );

        // And the error must not leak the path (it discloses the username).
        let rendered = err.to_string();
        assert!(!rendered.contains(&*path.to_string_lossy()));
        assert!(!rendered.contains("clipboard.db"));
    }

    #[test]
    fn database_is_encrypted_on_disk_and_in_wal_mode() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("clipboard.db");
        let s = Store::open(&path, &KEY).unwrap();
        s.insert(item("plaintext marker chinchilla", T0)).unwrap();
        {
            let conn = s.conn().unwrap();
            let mode: String = conn
                .query_row("PRAGMA journal_mode", [], |r| r.get(0))
                .unwrap();
            assert_eq!(mode, "wal");
            run_pragma(&conn, "PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
        }
        drop(s);

        let bytes = std::fs::read(&path).unwrap();
        assert!(!bytes.starts_with(b"SQLite format 3\0"));
        assert!(!bytes
            .windows(b"chinchilla".len())
            .any(|w| w == b"chinchilla"));
    }

    #[test]
    fn a_future_schema_version_is_refused() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("clipboard.db");
        {
            let s = Store::open(&path, &KEY).unwrap();
            let conn = s.conn().unwrap();
            run_pragma(&conn, "PRAGMA user_version = 999").unwrap();
        }
        let err = Store::open(&path, &KEY).unwrap_err();
        assert!(
            matches!(err, StoreError::Migration(_)),
            "expected a migration error, got {err:?}"
        );
    }

    #[test]
    fn in_memory_pool_shares_one_database() {
        // Every pooled connection must see the same in-memory database, so hold
        // one while working through another.
        let s = store();
        let held = s.conn().unwrap();
        let stored = s.insert(item("across connections", T0)).unwrap();
        assert_eq!(s.count().unwrap(), 1);
        assert!(s.get(&stored.id).unwrap().is_some());
        drop(held);
    }

    #[test]
    fn content_hash_is_the_full_sha256_hex() {
        let empty = compute_content_hash(b"");
        assert_eq!(empty.len(), 64);
        assert_eq!(
            empty,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            compute_content_hash(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        // Content-only: identical bytes hash identically, different bytes do not.
        assert_eq!(compute_content_hash(b"abc"), compute_content_hash(b"abc"));
        assert_ne!(compute_content_hash(b"abc"), compute_content_hash(b"abd"));
    }

    #[test]
    fn fts5_query_sanitizer() {
        assert_eq!(
            sanitize_fts5_query("foo-bar").as_deref(),
            Some("foo* AND bar*")
        );
        assert_eq!(
            sanitize_fts5_query("priv key").as_deref(),
            Some("priv* AND key*")
        );
        assert_eq!(sanitize_fts5_query("foo*").as_deref(), Some("foo*"));
        assert_eq!(
            sanitize_fts5_query("\"exact phrase\"").as_deref(),
            Some("\"exact phrase\"")
        );
        // Unbalanced quote: strip rather than hand FTS5 a syntax error.
        assert_eq!(sanitize_fts5_query("\"oops").as_deref(), Some("oops*"));
        // Reserved words are dropped, non-word noise is stripped.
        assert_eq!(
            sanitize_fts5_query("foo OR bar").as_deref(),
            Some("foo* AND bar*")
        );
        assert_eq!(
            sanitize_fts5_query("col:val;--").as_deref(),
            Some("colval*")
        );
        assert_eq!(sanitize_fts5_query("привет").as_deref(), Some("привет*"));
        assert!(sanitize_fts5_query("").is_none());
        assert!(sanitize_fts5_query("   ").is_none());
        assert!(sanitize_fts5_query("^^^").is_none());
        assert!(sanitize_fts5_query("AND OR").is_none());
    }
}
