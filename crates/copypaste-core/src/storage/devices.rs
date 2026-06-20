// devices.rs — paired-device storage helpers + manual revocation audit.
//
// v0.3 ships the UI half of OI-2 (peer revocation). The full cryptographic
// revocation protocol lands in v1.0; until then revocation is a local-only
// operation: the daemon removes the peer from its peers list and records the
// event in an additive `revoked_devices` audit table.
//
// SCHEMA VERSIONING (CopyPaste-61fu):
// The `revoked_devices` table is now created by migration v12 in `schema.rs`,
// inside the numbered migration chain, so it exists on every properly-initialised
// DB after `apply_migrations` runs (which `Database::open*` always calls).
//
// The old ad-hoc `ensure_revoked_devices_table` call at daemon startup is no
// longer necessary for correctness. The function is retained as a defence-in-depth
// safety net (it is idempotent: `CREATE TABLE IF NOT EXISTS`) but callers should
// NOT depend on it — the migration guarantees the table's existence.

use rusqlite::{params, Connection};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DevicesError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
}

/// Defence-in-depth safety net — idempotent DDL for the `revoked_devices` table.
///
/// As of schema v12 (CopyPaste-61fu) the `revoked_devices` table is created by
/// `schema::apply_migrations`, which runs on every `Database::open*` call.
/// This function is **no longer the primary creation path** and does not need to
/// be called explicitly.  It is retained only as a safety net so that any code
/// path that calls it on a pre-v12 database (e.g. tests that bypass `Database`
/// and use a raw `Connection`) still succeeds rather than panicking.
///
/// `CREATE TABLE IF NOT EXISTS` / `CREATE INDEX IF NOT EXISTS` make this idempotent:
/// calling it on a v12+ database that already has the table is a no-op.
///
/// Columns:
///   * `fingerprint` — primary key; colon-separated hex fingerprint of the
///     revoked device (matches `peers.json` and the `devices` table).
///   * `name`        — best-effort human-readable name captured at revoke time
///     (may be empty if the peer record was already gone).
///   * `revoked_at`  — unix seconds when the user clicked Revoke.
pub fn ensure_revoked_devices_table(conn: &Connection) -> Result<(), DevicesError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS revoked_devices (\
             fingerprint TEXT PRIMARY KEY NOT NULL,\
             name        TEXT NOT NULL DEFAULT '',\
             revoked_at  INTEGER NOT NULL\
         );\n\
         CREATE INDEX IF NOT EXISTS idx_revoked_devices_revoked_at \
             ON revoked_devices(revoked_at DESC);",
    )?;
    Ok(())
}

/// Record a manual peer revocation event.
///
/// Ensures the audit table exists, then wraps the DELETE (from `devices`)
/// and INSERT (into `revoked_devices`) in a single SQLite transaction so a
/// crash between them cannot leave the paired-device row gone without the
/// matching audit entry — either both writes commit or neither does
/// (CopyPaste-d7um). Absence of the peer row in `devices` is treated as a
/// no-op so callers that haven't paired the device yet still record the marker.
///
/// Returns the unix-seconds timestamp written to `revoked_at` so the caller
/// can echo it back to the UI without a follow-up query.
pub fn revoke_device(
    conn: &Connection,
    fingerprint: &str,
    name: &str,
) -> Result<u64, DevicesError> {
    ensure_revoked_devices_table(conn)?;

    let revoked_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    // `unchecked_transaction` matches the storage-layer convention: the daemon
    // holds a `Database` behind a `Mutex` and hands out `&Connection` only, so
    // there is never a concurrent borrow to guard against (same pattern used by
    // `revoke_devices` and `insert_item_with_fts`).
    let tx = conn.unchecked_transaction()?;

    // Best-effort delete from the canonical paired-devices table. The table
    // is part of v1 schema so it always exists; `execute` returns 0 rows
    // affected when the fingerprint isn't there, which is fine.
    tx.execute(
        "DELETE FROM devices WHERE fingerprint = ?1",
        params![fingerprint],
    )?;

    tx.execute(
        "INSERT INTO revoked_devices (fingerprint, name, revoked_at) \
         VALUES (?1, ?2, ?3) \
         ON CONFLICT(fingerprint) DO UPDATE SET \
             name       = excluded.name, \
             revoked_at = excluded.revoked_at",
        params![fingerprint, name, revoked_at as i64],
    )?;

    tx.commit()?;
    Ok(revoked_at)
}

/// Revoke many peers atomically.
///
/// Wraps the per-peer delete + audit-insert for every `(fingerprint, name)`
/// pair in a single SQLite transaction: either every audit row is written and
/// every matching `devices` row removed, or — on any error — the whole batch
/// rolls back and nothing is persisted. This lets the caller clear its
/// (separate JSON) peer store only after the audit log is durably committed,
/// so the two stores can never drift into the "store empty but audit rows
/// missing" state.
///
/// Returns the unix-seconds timestamp stamped on all rows in the batch.
/// An empty `peers` slice is a no-op that still returns a timestamp.
pub fn revoke_devices(conn: &Connection, peers: &[(String, String)]) -> Result<u64, DevicesError> {
    ensure_revoked_devices_table(conn)?;

    let revoked_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    // `unchecked_transaction` (rather than `&mut self` `transaction`) matches
    // the storage layer's convention (see `insert_item_with_fts`): the daemon
    // holds the `Database` behind a `Mutex` and only ever hands out
    // `&Connection`, so there is no concurrent borrow to guard against.
    let tx = conn.unchecked_transaction()?;
    for (fingerprint, name) in peers {
        tx.execute(
            "DELETE FROM devices WHERE fingerprint = ?1",
            params![fingerprint],
        )?;
        tx.execute(
            "INSERT INTO revoked_devices (fingerprint, name, revoked_at) \
             VALUES (?1, ?2, ?3) \
             ON CONFLICT(fingerprint) DO UPDATE SET \
                 name       = excluded.name, \
                 revoked_at = excluded.revoked_at",
            params![fingerprint, name, revoked_at as i64],
        )?;
    }
    tx.commit()?;

    Ok(revoked_at)
}

/// A single audit row from `revoked_devices`. Returned by
/// [`list_revoked_devices`] for tests and for the (future) v1.0 sync worker
/// that will translate rows into outbound revocation markers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevokedDevice {
    pub fingerprint: String,
    pub name: String,
    pub revoked_at: i64,
}

/// Read all audit rows, newest first.
pub fn list_revoked_devices(conn: &Connection) -> Result<Vec<RevokedDevice>, DevicesError> {
    ensure_revoked_devices_table(conn)?;
    let mut stmt = conn.prepare(
        "SELECT fingerprint, name, revoked_at \
         FROM revoked_devices \
         ORDER BY revoked_at DESC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(RevokedDevice {
            fingerprint: row.get(0)?,
            name: row.get(1)?,
            revoked_at: row.get(2)?,
        })
    })?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Database;

    fn fresh_db() -> Database {
        Database::open_in_memory().expect("open_in_memory")
    }

    #[test]
    fn ensure_table_is_idempotent() {
        let db = fresh_db();
        ensure_revoked_devices_table(db.conn()).unwrap();
        ensure_revoked_devices_table(db.conn()).unwrap();

        let count: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master \
                 WHERE type='table' AND name='revoked_devices'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "table must exist exactly once");
    }

    #[test]
    fn revoke_device_inserts_audit_row() {
        let db = fresh_db();
        let fp = "ab:cd:ef:01:23:45:67:89";
        let ts = revoke_device(db.conn(), fp, "Laptop").unwrap();

        let rows = list_revoked_devices(db.conn()).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].fingerprint, fp);
        assert_eq!(rows[0].name, "Laptop");
        assert_eq!(rows[0].revoked_at as u64, ts);
    }

    #[test]
    fn revoke_device_removes_from_devices_table() {
        let db = fresh_db();
        let fp = "11:22:33:44:55:66:77:88";

        // Seed a paired device row using the baseline v1 schema.
        db.conn()
            .execute(
                "INSERT INTO devices (id, name, platform, public_key, fingerprint) \
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params!["dev-1", "Laptop", "macos", "PUBKEY", fp],
            )
            .unwrap();

        revoke_device(db.conn(), fp, "Laptop").unwrap();

        let remaining: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM devices WHERE fingerprint = ?1",
                params![fp],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(remaining, 0, "paired device row must be deleted");

        let audit = list_revoked_devices(db.conn()).unwrap();
        assert_eq!(audit.len(), 1, "audit row must be present");
        assert_eq!(audit[0].fingerprint, fp);
    }

    #[test]
    fn revoke_device_is_idempotent() {
        let db = fresh_db();
        let fp = "aa:bb:cc:dd:ee:ff:00:11";
        revoke_device(db.conn(), fp, "Phone").unwrap();
        // Second call — UPSERT must not duplicate the row.
        revoke_device(db.conn(), fp, "Phone (renamed)").unwrap();

        let rows = list_revoked_devices(db.conn()).unwrap();
        assert_eq!(rows.len(), 1, "fingerprint is PK; second call must UPSERT");
        assert_eq!(rows[0].name, "Phone (renamed)");
    }

    #[test]
    fn revoke_devices_writes_all_audit_rows() {
        let db = fresh_db();
        let peers = vec![
            ("aa:aa:aa:aa:aa:aa:aa:aa".to_string(), "Laptop".to_string()),
            ("bb:bb:bb:bb:bb:bb:bb:bb".to_string(), "Phone".to_string()),
            ("cc:cc:cc:cc:cc:cc:cc:cc".to_string(), "Tablet".to_string()),
        ];
        let ts = revoke_devices(db.conn(), &peers).unwrap();
        assert!(ts > 0, "timestamp must be populated");

        let rows = list_revoked_devices(db.conn()).unwrap();
        assert_eq!(rows.len(), 3, "every peer must get an audit row");
        for (fp, _name) in &peers {
            assert!(
                rows.iter().any(|r| &r.fingerprint == fp),
                "audit row missing for {fp}"
            );
        }
    }

    #[test]
    fn revoke_devices_empty_slice_is_noop() {
        let db = fresh_db();
        let ts = revoke_devices(db.conn(), &[]).unwrap();
        assert!(ts > 0, "timestamp returned even for empty batch");
        assert_eq!(
            list_revoked_devices(db.conn()).unwrap().len(),
            0,
            "no rows written for empty batch"
        );
    }

    #[test]
    fn revoke_devices_removes_from_devices_table() {
        let db = fresh_db();
        let fp = "12:34:56:78:9a:bc:de:f0";
        db.conn()
            .execute(
                "INSERT INTO devices (id, name, platform, public_key, fingerprint) \
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params!["dev-x", "Laptop", "macos", "PUBKEY", fp],
            )
            .unwrap();

        revoke_devices(db.conn(), &[(fp.to_string(), "Laptop".to_string())]).unwrap();

        let remaining: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM devices WHERE fingerprint = ?1",
                params![fp],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(remaining, 0, "paired device row must be deleted in batch");
        assert_eq!(list_revoked_devices(db.conn()).unwrap().len(), 1);
    }

    #[test]
    fn revoke_device_works_without_prior_pairing() {
        // The peer-store is JSON-backed in v0.3, so the `devices` SQLite
        // table may be empty even when the user pairs/unpairs peers. The
        // audit row must still be written in that case.
        let db = fresh_db();
        let fp = "de:ad:be:ef:de:ad:be:ef";
        let ts = revoke_device(db.conn(), fp, "").unwrap();
        assert!(ts > 0, "timestamp must be populated");

        let rows = list_revoked_devices(db.conn()).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].fingerprint, fp);
        assert_eq!(rows[0].name, "");
    }

    /// CopyPaste-d7um: `revoke_device` must remove the paired-device row AND
    /// write the audit row atomically.  We can't easily simulate a mid-crash
    /// state in-process, but we verify the observable all-or-nothing guarantee:
    /// after a successful call exactly one audit row exists AND the paired row
    /// is gone, so neither side of the pair can be in limbo.
    #[test]
    fn revoke_device_atomic_delete_and_audit() {
        let db = fresh_db();
        let fp = "ca:fe:ba:be:00:11:22:33";

        // Seed a paired device row so we exercise both the DELETE and INSERT
        // arms of the transaction.
        db.conn()
            .execute(
                "INSERT INTO devices (id, name, platform, public_key, fingerprint) \
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params!["dev-atomic", "Desktop", "macos", "PUBKEY", fp],
            )
            .unwrap();

        let ts = revoke_device(db.conn(), fp, "Desktop").unwrap();

        // After commit: paired row must be gone.
        let device_count: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM devices WHERE fingerprint = ?1",
                params![fp],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            device_count, 0,
            "devices row must be removed as part of the atomic revocation"
        );

        // After commit: audit row must exist with correct data.
        let audit = list_revoked_devices(db.conn()).unwrap();
        assert_eq!(audit.len(), 1, "audit row must be written atomically");
        assert_eq!(audit[0].fingerprint, fp);
        assert_eq!(audit[0].name, "Desktop");
        assert_eq!(audit[0].revoked_at as u64, ts);
    }
}
