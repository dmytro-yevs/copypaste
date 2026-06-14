//! Durable persistence layer for the relay store (R1b — "relay works as a
//! database").
//!
//! ## What is persisted
//!
//! The relay's durable state — device records, their *sets* of co-registered
//! auth tokens (R1a), and per-device inbox items — is mirrored into a plain
//! SQLite database so it survives a process restart. The in-memory
//! [`RelayStore`](crate::state::RelayStore) keeps the hot read path (pull /
//! verify never touch SQLite); every *mutation* is written through to this
//! database, and on open the database is loaded back into the in-memory maps.
//! Ephemeral runtime signals (SSE wake channels, the per-IP/per-device
//! rate-limit buckets) are intentionally NOT persisted.
//!
//! ## Plain SQLite, never SQLCipher
//!
//! The relay stores only opaque, already-encrypted ciphertext (`content_b64`)
//! and must never hold keys or see plaintext. We therefore use rusqlite's
//! **`bundled`** feature (vendored *plain* SQLite), NOT `bundled-sqlcipher`.
//!
//! Feature-unification note: `copypaste-core` depends on rusqlite with
//! `bundled-sqlcipher`. Cargo unifies features across the workspace, so the
//! single shared `libsqlite3-sys` ends up with both `bundled` and
//! `bundled-sqlcipher` enabled and is built as the SQLCipher amalgamation
//! (a superset of plain SQLite). That is harmless here: this module opens
//! connections WITHOUT ever issuing `PRAGMA key`, so the on-disk file is an
//! ordinary unencrypted SQLite database — the relay never performs encryption
//! and never holds a key. Declaring `bundled` (not `bundled-sqlcipher`) keeps
//! the relay's *intent* explicit and avoids it pulling SQLCipher/OpenSSL on its
//! own account; the unification is a workspace build artifact, not a relay
//! dependency.
//!
//! ## Blocking model
//!
//! rusqlite's `Connection` is `Send` but not `Sync`, so it lives inside the
//! existing `std::sync::Mutex<RelayStore>` (one connection, serialized by the
//! same lock that already serializes the store). Reads are served from memory,
//! so the only SQLite work on the request path is a single small row
//! insert/update/delete on a *local* WAL database — sub-millisecond and bounded,
//! matching the store's existing short-critical-section model (the crate denies
//! `clippy::await_holding_lock`, so the lock — and hence the connection — is
//! never held across an `.await`). `synchronous=NORMAL` + WAL keep fsync cost
//! off the critical path. A heavier off-runtime path (`spawn_blocking`) is not
//! used because the durable data is reached only while holding the synchronous
//! store mutex, which a spawned blocking task cannot carry.

use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension};

/// Sentinel path that selects an in-memory database (no on-disk persistence).
/// This is the relay's default so existing tests and ephemeral deploys behave
/// exactly as the pre-R1b in-memory store did.
pub const IN_MEMORY_PATH: &str = ":memory:";

/// Idempotent schema. Created on every open via `CREATE TABLE IF NOT EXISTS`,
/// so re-opening an existing database file is a no-op.
///
/// Tables:
/// - `devices` — one row per registered `device_id` (record metadata).
/// - `device_tokens` — the R1a token *set*: many rows per `device_id`, ordered
///   by `issue_seq` (issuance order, oldest first) to drive FIFO token eviction
///   deterministically.
/// - `inbox_items` — per-device inbox of opaque ciphertext items.
///
/// Indexes target the hot queries:
/// - `idx_inbox_cursor` — inbox scan by `(device_id, wall_time, item_id)`, the
///   exact `(wall_time, id)` pull cursor order.
/// - `idx_inbox_ttl` — TTL/eviction sweep by `inserted_at_unix`.
/// - `idx_tokens_device` — token lookup / verify by `device_id`.
const SCHEMA: &str = "
    CREATE TABLE IF NOT EXISTS devices (
        device_id          TEXT PRIMARY KEY NOT NULL,
        device_name        TEXT NOT NULL,
        public_key_b64     TEXT NOT NULL,
        tier               TEXT NOT NULL,
        registered_from_ip TEXT,
        registered_at_unix INTEGER NOT NULL,
        last_seen_unix     INTEGER NOT NULL,
        next_sync_id       INTEGER NOT NULL DEFAULT 1,
        registered_pop     TEXT
    );

    CREATE TABLE IF NOT EXISTS device_tokens (
        device_id       TEXT NOT NULL,
        token           TEXT NOT NULL,
        expires_at_unix INTEGER NOT NULL,
        issue_seq       INTEGER NOT NULL,
        PRIMARY KEY (device_id, token),
        FOREIGN KEY (device_id) REFERENCES devices(device_id) ON DELETE CASCADE
    );
    CREATE INDEX IF NOT EXISTS idx_tokens_device ON device_tokens(device_id, issue_seq);

    CREATE TABLE IF NOT EXISTS inbox_items (
        device_id        TEXT NOT NULL,
        item_id          INTEGER NOT NULL,
        content_type     TEXT NOT NULL,
        content_b64      TEXT NOT NULL,
        wall_time        INTEGER NOT NULL,
        inserted_at_unix INTEGER NOT NULL,
        PRIMARY KEY (device_id, item_id),
        FOREIGN KEY (device_id) REFERENCES devices(device_id) ON DELETE CASCADE
    );
    CREATE INDEX IF NOT EXISTS idx_inbox_cursor ON inbox_items(device_id, wall_time, item_id);
    CREATE INDEX IF NOT EXISTS idx_inbox_ttl ON inbox_items(inserted_at_unix);
";

/// A persisted device record + its token set + inbox, as loaded from SQLite.
/// Plain data — the in-memory [`RelayStore`](crate::state::RelayStore)
/// reconstructs its `Instant`-based fields from the stored Unix timestamps.
pub struct LoadedDevice {
    pub device_id: String,
    pub device_name: String,
    pub public_key_b64: String,
    /// `"free"` / `"pro"` — parsed back to `Tier` by the caller.
    pub tier: String,
    /// Source IP string as stored (`None` when registered without a transport).
    pub registered_from_ip: Option<String>,
    pub registered_at_unix: i64,
    pub last_seen_unix: i64,
    pub next_sync_id: i64,
    /// The proof-of-possession stored at first registration: base64-encoded
    /// HMAC-SHA256 output. `None` for devices registered before the PoP fix
    /// (CopyPaste-n2l); the store maps that to `[0u8; 32]` sentinel.
    pub registered_pop: Option<String>,
    /// Token set in issuance order (oldest first), matching `issue_seq`.
    pub tokens: Vec<(String, i64)>,
    /// Inbox items, ascending by `(wall_time, item_id)`.
    pub items: Vec<LoadedItem>,
}

pub struct LoadedItem {
    pub id: i64,
    pub content_type: String,
    pub content_b64: String,
    pub wall_time: u64,
    pub inserted_at_unix: u64,
}

/// The relay's durable store handle: one SQLite connection plus a monotonic
/// per-process counter used to stamp `device_tokens.issue_seq` so that FIFO
/// token eviction order is preserved across restart.
pub struct Db {
    conn: Connection,
    /// Whether this connection is backed by an on-disk file (`true`) or is the
    /// in-memory default (`false`). Purely informational today.
    #[allow(dead_code)]
    persistent: bool,
}

impl Db {
    /// Open (or create) the database at `path`, applying the schema idempotently.
    /// `":memory:"` ([`IN_MEMORY_PATH`]) selects a private in-memory database.
    pub fn open(path: &str) -> Result<Self, rusqlite::Error> {
        let persistent = path != IN_MEMORY_PATH;
        let conn = if persistent {
            Connection::open(Path::new(path))?
        } else {
            Connection::open_in_memory()?
        };
        // WAL + NORMAL keep fsync off the critical path on a local file; both
        // are no-ops / harmless on an in-memory database. foreign_keys=ON makes
        // the ON DELETE CASCADE on device removal actually fire so a device's
        // tokens + inbox are reclaimed atomically.
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=NORMAL;
             PRAGMA foreign_keys=ON;",
        )?;
        conn.execute_batch(SCHEMA)?;
        // Schema migration: add `registered_pop` to pre-existing databases that
        // were created before CopyPaste-n2l was fixed. We check
        // `pragma_table_info` first (instead of `IF NOT EXISTS`) because bundled
        // SQLite in rusqlite 0.32 may predate 3.37.0 where that extension was
        // added. Devices loaded from old rows have `registered_pop = NULL`, which
        // the store maps to the `[0u8; 32]` sentinel.
        let needs_pop_col: bool = conn
            .query_row(
                "SELECT COUNT(*) = 0 FROM pragma_table_info('devices') \
                 WHERE name = 'registered_pop'",
                [],
                |r| r.get(0),
            )
            .unwrap_or(false);
        if needs_pop_col {
            conn.execute_batch("ALTER TABLE devices ADD COLUMN registered_pop TEXT;")?;
        }
        Ok(Self { conn, persistent })
    }

    // -- Loading (restart recovery) -----------------------------------------

    /// Load every persisted device with its token set and inbox, ready to
    /// rehydrate the in-memory maps. Tokens come back in `issue_seq` order and
    /// items in `(wall_time, item_id)` order so the in-memory invariants
    /// (issuance-ordered token Vec, cursor-sorted inbox) hold without re-sorting.
    pub fn load_all(&self) -> Result<Vec<LoadedDevice>, rusqlite::Error> {
        let mut devices = Vec::new();
        {
            let mut stmt = self.conn.prepare(
                "SELECT device_id, device_name, public_key_b64, tier, registered_from_ip,
                        registered_at_unix, last_seen_unix, next_sync_id, registered_pop
                 FROM devices",
            )?;
            let rows = stmt.query_map([], |r| {
                Ok(LoadedDevice {
                    device_id: r.get(0)?,
                    device_name: r.get(1)?,
                    public_key_b64: r.get(2)?,
                    tier: r.get(3)?,
                    registered_from_ip: r.get(4)?,
                    registered_at_unix: r.get(5)?,
                    last_seen_unix: r.get(6)?,
                    next_sync_id: r.get(7)?,
                    registered_pop: r.get(8)?,
                    tokens: Vec::new(),
                    items: Vec::new(),
                })
            })?;
            for row in rows {
                devices.push(row?);
            }
        }

        for dev in &mut devices {
            {
                let mut stmt = self.conn.prepare(
                    "SELECT token, expires_at_unix FROM device_tokens
                     WHERE device_id = ?1 ORDER BY issue_seq ASC",
                )?;
                let rows = stmt.query_map(params![dev.device_id], |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
                })?;
                for row in rows {
                    dev.tokens.push(row?);
                }
            }
            {
                let mut stmt = self.conn.prepare(
                    "SELECT item_id, content_type, content_b64, wall_time, inserted_at_unix
                     FROM inbox_items WHERE device_id = ?1
                     ORDER BY wall_time ASC, item_id ASC",
                )?;
                let rows = stmt.query_map(params![dev.device_id], |r| {
                    Ok(LoadedItem {
                        id: r.get(0)?,
                        content_type: r.get(1)?,
                        content_b64: r.get(2)?,
                        wall_time: r.get::<_, i64>(3)? as u64,
                        inserted_at_unix: r.get::<_, i64>(4)? as u64,
                    })
                })?;
                for row in rows {
                    dev.items.push(row?);
                }
            }
        }
        Ok(devices)
    }

    // -- Device mutations ----------------------------------------------------

    /// Insert a brand-new device record (first registration of this id).
    // All columns of the devices table (id, name, token, expires_at, key, pop,
    // scope, tier, registered_at) map to separate parameters — grouping them
    // into a struct would not improve clarity for a direct DB insert.
    #[allow(clippy::too_many_arguments)]
    pub fn insert_device(
        &self,
        device_id: &str,
        device_name: &str,
        public_key_b64: &str,
        tier: &str,
        registered_from_ip: Option<&str>,
        registered_at_unix: i64,
        last_seen_unix: i64,
        registered_pop_b64: &str,
    ) -> Result<(), rusqlite::Error> {
        self.conn.execute(
            "INSERT INTO devices
               (device_id, device_name, public_key_b64, tier, registered_from_ip,
                registered_at_unix, last_seen_unix, next_sync_id, registered_pop)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1, ?8)",
            params![
                device_id,
                device_name,
                public_key_b64,
                tier,
                registered_from_ip,
                registered_at_unix,
                last_seen_unix,
                registered_pop_b64,
            ],
        )?;
        Ok(())
    }

    /// Replace the full token set for a device (used after `add_token` prunes
    /// expired / FIFO-evicts entries — simplest correct way to keep the
    /// persisted set byte-identical to the in-memory Vec, including order).
    /// `tokens` is `(token, expires_at_unix)` in issuance order (oldest first).
    pub fn replace_tokens(
        &self,
        device_id: &str,
        tokens: &[(String, i64)],
    ) -> Result<(), rusqlite::Error> {
        self.conn.execute(
            "DELETE FROM device_tokens WHERE device_id = ?1",
            params![device_id],
        )?;
        for (seq, (token, expires_at_unix)) in tokens.iter().enumerate() {
            self.conn.execute(
                "INSERT INTO device_tokens (device_id, token, expires_at_unix, issue_seq)
                 VALUES (?1, ?2, ?3, ?4)",
                params![device_id, token, expires_at_unix, seq as i64],
            )?;
        }
        Ok(())
    }

    /// Stamp `last_seen_unix` for a device (no-op if the device is gone).
    pub fn update_last_seen(
        &self,
        device_id: &str,
        last_seen_unix: i64,
    ) -> Result<(), rusqlite::Error> {
        self.conn.execute(
            "UPDATE devices SET last_seen_unix = ?2 WHERE device_id = ?1",
            params![device_id, last_seen_unix],
        )?;
        Ok(())
    }

    /// Persist a device's `next_sync_id` counter so restart can't re-issue an id.
    pub fn set_next_sync_id(&self, device_id: &str, next: i64) -> Result<(), rusqlite::Error> {
        self.conn.execute(
            "UPDATE devices SET next_sync_id = ?2 WHERE device_id = ?1",
            params![device_id, next],
        )?;
        Ok(())
    }

    /// Delete a device and (via `ON DELETE CASCADE`) its tokens + inbox.
    pub fn delete_device(&self, device_id: &str) -> Result<(), rusqlite::Error> {
        self.conn.execute(
            "DELETE FROM devices WHERE device_id = ?1",
            params![device_id],
        )?;
        Ok(())
    }

    // -- Inbox mutations -----------------------------------------------------

    /// Insert one inbox item.
    pub fn insert_item(
        &self,
        device_id: &str,
        item_id: i64,
        content_type: &str,
        content_b64: &str,
        wall_time: u64,
        inserted_at_unix: u64,
    ) -> Result<(), rusqlite::Error> {
        self.conn.execute(
            "INSERT INTO inbox_items
               (device_id, item_id, content_type, content_b64, wall_time, inserted_at_unix)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                device_id,
                item_id,
                content_type,
                content_b64,
                wall_time as i64,
                inserted_at_unix as i64,
            ],
        )?;
        Ok(())
    }

    /// Delete a single inbox item by `(device_id, item_id)`.
    pub fn delete_item(&self, device_id: &str, item_id: i64) -> Result<(), rusqlite::Error> {
        self.conn.execute(
            "DELETE FROM inbox_items WHERE device_id = ?1 AND item_id = ?2",
            params![device_id, item_id],
        )?;
        Ok(())
    }

    /// Delete the `count` oldest items (by `(wall_time, item_id)`) for a device.
    /// Used to mirror the history-cap prune-oldest behaviour.
    pub fn delete_oldest_items(
        &self,
        device_id: &str,
        count: usize,
    ) -> Result<(), rusqlite::Error> {
        if count == 0 {
            return Ok(());
        }
        self.conn.execute(
            "DELETE FROM inbox_items
             WHERE rowid IN (
                 SELECT rowid FROM inbox_items WHERE device_id = ?1
                 ORDER BY wall_time ASC, item_id ASC LIMIT ?2
             )",
            params![device_id, count as i64],
        )?;
        Ok(())
    }

    /// TTL eviction in SQL: delete every item with
    /// `inserted_at_unix <= cutoff`. Returns the number of rows removed.
    pub fn prune_expired(&self, cutoff: u64) -> Result<usize, rusqlite::Error> {
        let n = self.conn.execute(
            "DELETE FROM inbox_items WHERE inserted_at_unix <= ?1",
            params![cutoff as i64],
        )?;
        Ok(n)
    }

    // -- Test/diagnostic helpers --------------------------------------------

    /// Count inbox items for a device (used by persistence tests).
    #[allow(dead_code)]
    pub fn item_count(&self, device_id: &str) -> Result<i64, rusqlite::Error> {
        self.conn
            .query_row(
                "SELECT COUNT(*) FROM inbox_items WHERE device_id = ?1",
                params![device_id],
                |r| r.get(0),
            )
            .optional()
            .map(|c| c.unwrap_or(0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_in_memory_applies_schema() {
        let db = Db::open(IN_MEMORY_PATH).expect("open in-memory");
        // All three tables must exist.
        for table in ["devices", "device_tokens", "inbox_items"] {
            let count: i64 = db
                .conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
                    params![table],
                    |r| r.get(0),
                )
                .expect("query");
            assert_eq!(count, 1, "table {table} must exist after schema init");
        }
    }

    #[test]
    fn insert_load_roundtrip() {
        let db = Db::open(IN_MEMORY_PATH).unwrap();
        db.insert_device("d1", "Dev", "pk", "free", Some("10.0.0.1"), 100, 200, "AAAA")
            .unwrap();
        db.replace_tokens("d1", &[("tok-a".into(), 999), ("tok-b".into(), 1000)])
            .unwrap();
        db.insert_item("d1", 1, "text", "Yg==", 1000, 50).unwrap();
        db.insert_item("d1", 2, "text", "Yw==", 2000, 60).unwrap();

        let loaded = db.load_all().unwrap();
        assert_eq!(loaded.len(), 1);
        let d = &loaded[0];
        assert_eq!(d.device_id, "d1");
        assert_eq!(d.registered_from_ip.as_deref(), Some("10.0.0.1"));
        assert_eq!(
            d.tokens,
            vec![("tok-a".into(), 999), ("tok-b".into(), 1000)]
        );
        assert_eq!(d.items.len(), 2);
        assert_eq!(d.items[0].wall_time, 1000);
        assert_eq!(d.items[1].id, 2);
    }

    #[test]
    fn cascade_delete_removes_tokens_and_items() {
        let db = Db::open(IN_MEMORY_PATH).unwrap();
        db.insert_device("d1", "Dev", "pk", "free", None, 1, 1, "AAAA")
            .unwrap();
        db.replace_tokens("d1", &[("t".into(), 1)]).unwrap();
        db.insert_item("d1", 1, "text", "Yg==", 1, 1).unwrap();
        db.delete_device("d1").unwrap();
        assert!(db.load_all().unwrap().is_empty());
        assert_eq!(db.item_count("d1").unwrap(), 0);
    }

    #[test]
    fn prune_expired_removes_old_items_in_sql() {
        let db = Db::open(IN_MEMORY_PATH).unwrap();
        db.insert_device("d1", "Dev", "pk", "free", None, 1, 1, "AAAA")
            .unwrap();
        db.insert_item("d1", 1, "text", "Yg==", 1000, 10).unwrap();
        db.insert_item("d1", 2, "text", "Yw==", 2000, 100).unwrap();
        // cutoff = 50 → only the item inserted at 10 is removed.
        let removed = db.prune_expired(50).unwrap();
        assert_eq!(removed, 1);
        assert_eq!(db.item_count("d1").unwrap(), 1);
    }
}
