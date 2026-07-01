use rusqlite::Connection;
use thiserror::Error;

mod versions;

#[cfg(test)]
mod tests;

use versions::{
    V10_ALTER, V11_INDEX, V12_REVOKED_DEVICES_SQL, V13_PURGE_SENSITIVE_FTS, V14_INDEX,
    V15_DEDUP_INDEX_FIX, V1_SCHEMA_SQL, V3_ALTER_SQL, V4_ALTER_SQL, V5_INDEXES_SQL, V7_ALTER_SQL,
    V8_ALTER_SQL, V9_ALTER,
};

#[derive(Debug, Error)]
pub enum SchemaError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    /// The on-disk database was created by a *newer* version of the application
    /// than the one currently running. Downgrading the schema would silently
    /// drop forward-compatible columns / tables, so we refuse to open the file.
    #[error(
        "Database schema downgrade detected (on-disk version {found}, \
         binary expects {expected}). Refusing to open to avoid data loss."
    )]
    Downgrade { found: i64, expected: i64 },
}

/// Current on-disk schema version.
///
/// Bumps:
///   * 2 → 3: added `origin_device_id` for the LWW merge tie-break
///     (see `copypaste-sync::merge::resolve`).
///   * 3 → 4 (v0.3 T5): added `key_version` column to `clipboard_items` to
///     track which HKDF key generation (v1 or v2) encrypted each row's
///     ciphertext. See `V4_ALTER_SQL` and `super::migration_v4`.
///   * 4 → 5 (beta.6 merge): added two UNIQUE INDEXes — `content_hash`+minute
///     bucket for TOCTOU dedup, `item_id` for sync replay protection.
///     See `V5_INDEXES_SQL` / `schema_v2.sql`.
///   * 5 → 6 (wave1a-atomic): added `migration_state` table for resumable
///     v4 key-rotation sweep tracking. Seeds the initial row so
///     `Database::migration_state()` always returns a valid state.
///   * 6 → 7 (v0.3 pinned-fix): added `pinned` column to `clipboard_items`
///     so explicitly pinned items are distinguishable from normal rows with
///     `expires_at = NULL`. The TTL prune and history-limit prune both
///     filter `WHERE pinned = 0` to guarantee pinned items are never deleted.
///   * 7 → 8 (A1 drag-to-reorder): added `pin_order REAL DEFAULT NULL` to
///     `clipboard_items` so pinned items can be reordered by the user. Existing
///     pinned rows are backfilled with `CAST(rowid AS REAL)` to give them a
///     stable initial order. Unpinned rows keep `NULL`. The `history_page` and
///     `get_page_pinned_first` queries order pinned items by `pin_order ASC`
///     instead of the old `pinned DESC, wall_time DESC`.
///   * 8 → 9 (Variant B image thumbnail): added `thumb BLOB DEFAULT NULL` to
///     `clipboard_items` so each image row can carry a small capture-time
///     encrypted thumbnail blob (see `image::encode_thumbnail` /
///     `image::encode_image_full`). `DEFAULT NULL` backfills all existing rows
///     with no thumbnail; the daemon may lazily backfill later via
///     `items::set_thumb`. Text rows keep `NULL`.
///   * 9 → 10 (op-propagation foundation): added `deleted INTEGER NOT NULL
///     DEFAULT 0` to `clipboard_items` so logical deletions can be represented
///     as soft-delete tombstones that propagate via the LWW sync protocol.
///     `DEFAULT 0` backfills all existing rows as live (not deleted), which is
///     correct. A partial index on `deleted = 1` supports efficient tombstone
///     enumeration (sync catchup). The UI list queries filter `deleted = 0`;
///     `get_item_by_item_id` intentionally does NOT filter so the merge layer
///     can see tombstones and apply LWW correctly.
///   * 10 → 11 (CopyPaste-pvp4): added a partial covering index
///     (`idx_clipboard_unpinned_len`) on `LENGTH(COALESCE(content, ''))` for
///     unpinned rows so `prune_to_cap`'s per-write size gate can compute the
///     running `SUM(LENGTH(content))` as an index-only scan rather than a
///     full-table scan that reads every encrypted BLOB. No data change; index
///     only. See `V11_INDEX`.
///   * 11 → 12 (CopyPaste-61fu): moved `revoked_devices` audit table creation
///     from an ad-hoc `CREATE TABLE IF NOT EXISTS` call in `devices::ensure_revoked_devices_table`
///     into the versioned migration chain. Previously the table was created lazily
///     at daemon startup via an explicit `ensure_revoked_devices_table` call, which
///     caused "no such table" panics on any DB that opened without that call first.
///     Migration v12 creates the table (and its index) unconditionally during
///     `apply_migrations`, so every properly-initialised DB has the table regardless
///     of call order.
///   * 12 → 13 (CopyPaste-i6pp): purge stale `clipboard_fts` rows for sensitive
///     items. Before this fix `insert_item_with_fts` and `upsert_fts` did not
///     guard against `is_sensitive = 1`, so existing databases may contain
///     plaintext FTS entries for passwords, tokens, or other secrets. This
///     migration deletes all such rows. The forward-going code paths (`insert_item_with_fts`,
///     `upsert_fts`, `search_items`) are also patched to prevent new leakage.
///   * 13 → 14 (CopyPaste-89rd): add `idx_clipboard_history_page`, a partial
///     covering index on `(deleted, pinned DESC, pin_order, wall_time DESC) WHERE
///     deleted = 0`. The existing `idx_clipboard_deleted` is partial on `WHERE
///     deleted = 1` (tombstone minority) and does not help `get_page_pinned_first`.
///     Without this index every `history_page` IPC call performs a full-table scan
///     of `clipboard_items` and filesort — O(n) per page request. With the index
///     SQLite can split the pinned-first ORDER BY into two bounded index range
///     scans (pinned=1 rows, then pinned=0 rows), keeping each call O(log n +
///     page_size). No data change; index only.
pub const SCHEMA_VERSION: i64 = 15;

/// Return `true` if `column` already exists in `table`.
///
/// Uses `pragma_table_info` which is available on all SQLite versions we
/// target and works inside and outside transactions.  The result is used to
/// make `ALTER TABLE … ADD COLUMN` steps idempotent: if a column is already
/// present (e.g. because a WAL file was replayed onto a freshly-created
/// database file after `reset_database` deleted and recreated the main .db
/// file while another connection was still writing its WAL), we skip the
/// ALTER rather than failing with "duplicate column name".
fn column_exists(conn: &Connection, table: &str, column: &str) -> Result<bool, rusqlite::Error> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info(?1) WHERE name = ?2",
        rusqlite::params![table, column],
        |r| r.get(0),
    )?;
    Ok(count > 0)
}

/// Apply pending schema migrations atomically inside a single transaction.
///
/// Behavior contract:
///   * `current_version == SCHEMA_VERSION` → no-op, `Ok(())`.
///   * `current_version <  SCHEMA_VERSION` → run migrations inside a
///     transaction. If any step fails, the transaction is rolled back and
///     `user_version` remains untouched.
///   * `current_version >  SCHEMA_VERSION` → return `SchemaError::Downgrade`.
///     Previously this branch fell through to `Ok(())` and silently masked
///     the version mismatch (CRITICAL edge-case #2).
///
/// Each `ALTER TABLE … ADD COLUMN` step is guarded by a pre-check via
/// `column_exists`. This makes the migration chain robust against the
/// "WAL-replay onto fresh DB" scenario: when `reset_database` deletes the
/// main .db file while a concurrent connection is writing its WAL, SQLite
/// may replay that WAL onto the new empty file, leaving `user_version = 0`
/// but some columns already present. Without the guard the migrator would
/// fail with "SQLite error: duplicate column name". The guard makes each
/// step a safe no-op when its column already exists.
pub fn apply_migrations(conn: &Connection) -> Result<(), SchemaError> {
    // Connection-level pragmas that MUST run before BEGIN (PRAGMA journal_mode
    // is a no-op inside a transaction). The `Database::open*` paths apply the
    // full per-connection set (including a configurable cache_size) separately,
    // and RE-ASSERT the configured cache_size *after* this function returns, so
    // the value set here is only the DEFAULT used by raw-connection callers
    // (e.g. the migration unit tests). Keep it equal to the shipping default so
    // those callers behave as before; tuned callers override it post-migration.
    conn.execute_batch("PRAGMA journal_mode=WAL;")?;
    // CopyPaste-2lc9: force the WAL to be fully applied (checkpointed) BEFORE we
    // read `user_version` or call `column_exists`. The race: `column_exists` is
    // evaluated at SCRIPT-BUILD time. If a stale WAL is lazily replayed by SQLite
    // between the `column_exists` call (which returned false) and the subsequent
    // `execute_batch` call (which runs the ALTER inside BEGIN…COMMIT), the column
    // will already exist when the ALTER runs → "duplicate column name: content_hash".
    //
    // `wal_checkpoint(TRUNCATE)` flushes all committed WAL frames into the main
    // database file and truncates the WAL. After this point, `pragma_table_info`
    // (used by `column_exists`) and `PRAGMA user_version` observe the
    // post-checkpoint state — there is no remaining opportunity for a lagged WAL
    // replay to introduce new columns mid-script.
    //
    // Safety: this is a no-op on a fresh/empty database (no WAL file) and on
    // in-memory databases (WAL mode is silently downgraded to MEMORY journal by
    // SQLite, so the pragma runs but does nothing).
    //
    // This checkpoint is a DEFENSIVE BELT, never a correctness requirement: the
    // `column_exists` guard on every ALTER is the authoritative backstop against
    // WAL-replay. It is therefore NON-FATAL by design. Under heavy contention a
    // WAL checkpoint can fail the file-locking protocol race with SQLITE_BUSY or
    // SQLITE_PROTOCOL — and crucially `busy_timeout` does NOT cover SQLITE_PROTOCOL
    // (the WAL-index lock has its own internal retry budget that exhausts under a
    // slow/CPU-starved runner, e.g. coverage instrumentation: CopyPaste-2lc9
    // observed reset_database aborting with "SQLite error: locking protocol" in
    // the coverage job). Propagating that as a fatal SchemaError would fail the
    // whole DB open for a belt that did not even need to run, so we log and
    // continue — the column_exists guard still makes every migration step safe.
    if let Err(e) = conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);") {
        tracing::warn!(
            error = %e,
            "apply_migrations: wal_checkpoint(TRUNCATE) failed (non-fatal); \
             column_exists guards remain the authoritative WAL-replay backstop"
        );
    }
    conn.execute_batch(&format!(
        "PRAGMA cache_size=-{};",
        i64::from(crate::config::SQLITE_CACHE_MB) * 1024
    ))?;
    // CopyPaste-kexs: enable incremental auto_vacuum mode so PRAGMA incremental_vacuum
    // can reclaim free pages in bounded increments without a full blocking VACUUM.
    // This pragma is a no-op on databases that already have tables (auto_vacuum cannot
    // change once the schema is created — it takes effect only on a fresh empty DB).
    // For existing databases, incremental_vacuum() still works but reclaims nothing
    // until a full VACUUM is run to rebuild the file with the new mode.
    conn.execute_batch("PRAGMA auto_vacuum = INCREMENTAL;")?;

    let current_version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;

    if current_version == SCHEMA_VERSION {
        return Ok(());
    }
    if current_version > SCHEMA_VERSION {
        return Err(SchemaError::Downgrade {
            found: current_version,
            expected: SCHEMA_VERSION,
        });
    }

    // --- Atomic migration block (architecture MEDIUM #15) ---
    //
    // Build + execute in a bounded retry loop. A concurrent connection's
    // WAL-replay can materialise a later-migration column (e.g. content_hash)
    // AFTER our build-time `column_exists` probe returned false but BEFORE the
    // queued ALTER runs, so execute_batch fails with "duplicate column name".
    // SQLite rolls the transaction back, the racily-added column genuinely
    // persists, and rebuilding the script re-evaluates `column_exists` — now
    // true — so the offending ALTER is skipped and the retry converges. Bounded
    // so a genuine schema bug still surfaces instead of looping. Each attempt is
    // one self-contained BEGIN…COMMIT, so atomicity is preserved
    // (apply_migrations_is_atomic_on_failure). (CopyPaste-lmlr / -2lc9)
    for attempt in 0..3u8 {
        let script = build_migration_script(conn, current_version)?;
        match conn.execute_batch(&script) {
            Ok(()) => return Ok(()),
            Err(e) if attempt < 2 && is_duplicate_column_error(&e) => {
                tracing::warn!(
                    error = %e, attempt,
                    "apply_migrations: duplicate-column race (concurrent WAL-replay); \
                     rebuilding migration script and retrying"
                );
            }
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}

/// True when `e` is the SQLite "duplicate column name" error — an
/// `ALTER TABLE … ADD COLUMN` whose column was materialised by a concurrent
/// connection's WAL-replay between our `column_exists` probe and the ALTER
/// (CopyPaste-lmlr / -2lc9 race class).
fn is_duplicate_column_error(e: &rusqlite::Error) -> bool {
    e.to_string().contains("duplicate column name")
}

/// Build the atomic migration SQL script (`BEGIN … COMMIT`) bringing a DB at
/// `current_version` up to [`SCHEMA_VERSION`]. Pure string construction plus
/// `column_exists` probes against `conn`; executing it is the caller's job so a
/// duplicate-column race can be retried by rebuilding (see [`apply_migrations`]).
fn build_migration_script(conn: &Connection, current_version: i64) -> Result<String, SchemaError> {
    let mut script = String::with_capacity(2048);
    script.push_str("BEGIN;\n");

    if current_version < 1 {
        script.push_str(V1_SCHEMA_SQL);
        script.push('\n');
    }

    if current_version < 2 {
        // Migration v2: add content_hash column for SHA-256-based deduplication.
        // ALTER TABLE is used (not DROP/CREATE) to preserve existing data.
        // Guard: skip ALTER if the column already exists — the WAL-replay-onto-
        // fresh-DB scenario (reset_database race) can leave user_version=0 while
        // columns from later migrations are already present.
        if !column_exists(conn, "clipboard_items", "content_hash")? {
            script.push_str("ALTER TABLE clipboard_items ADD COLUMN content_hash TEXT;\n");
        }
        // The index uses CREATE … IF NOT EXISTS so it is always safe to include.
        script.push_str(
            "CREATE INDEX IF NOT EXISTS idx_clipboard_content_hash\n\
                 ON clipboard_items(content_hash) WHERE content_hash IS NOT NULL;\n",
        );
    }

    if current_version < 3 {
        // Migration v3: add origin_device_id column used by the LWW merge
        // tie-break (see `copypaste-sync::merge::resolve`). Defaults to the
        // empty string for legacy rows; the daemon calls
        // `items::backfill_origin_device_id` after open to stamp the local
        // device UUID onto any rows still carrying the empty default.
        if !column_exists(conn, "clipboard_items", "origin_device_id")? {
            script.push_str(V3_ALTER_SQL);
        }
    }

    if current_version < 4 {
        // Migration v4 (T5): add `key_version` column so the re-encrypt sweep
        // can identify rows still encrypted under the v1 HKDF key family.
        // The actual decrypt-with-v1 + re-encrypt-with-v2 work is performed
        // by `super::migration_v4::migrate_v1_to_v2_keys`, invoked by the
        // daemon at startup after the schema migration commits.
        if !column_exists(conn, "clipboard_items", "key_version")? {
            script.push_str(V4_ALTER_SQL);
        } else {
            // Column already exists; still need the index (IF NOT EXISTS is safe).
            script.push_str(
                "CREATE INDEX IF NOT EXISTS idx_clipboard_key_version \
                 ON clipboard_items(key_version) WHERE key_version < 2;\n",
            );
        }
    }

    if current_version < 5 {
        // Migration v5 (beta.6 merge): two UNIQUE INDEXes. CREATE INDEX IF
        // NOT EXISTS is idempotent so safe to re-run during partial-rollout.
        // See schema_v2.sql for per-index rationale.
        script.push_str(V5_INDEXES_SQL);
        script.push('\n');
    }

    if current_version < 6 {
        // Migration v6 (wave1a-atomic): create `migration_state` table for
        // resumable v4 key-rotation sweep tracking.
        //
        // Seed the row with completed_at already set when there are no
        // key_version=1 rows (fresh install or database already clean).
        // This prevents the gate in insert_item from blocking writes on a
        // brand-new database that has nothing to sweep.
        //
        // For upgrades from an earlier schema that may have key_version=1
        // rows, completed_at is left NULL so the daemon startup sweep runs.
        script.push_str(
            "CREATE TABLE IF NOT EXISTS migration_state (\n\
             key                     TEXT PRIMARY KEY,\n\
             key_version_in_progress INTEGER,\n\
             last_processed_id       INTEGER NOT NULL DEFAULT 0,\n\
             started_at              INTEGER,\n\
             completed_at            INTEGER\n\
             );\n\
             INSERT OR IGNORE INTO migration_state \
             (key, key_version_in_progress, last_processed_id, started_at, completed_at) \
             VALUES (\n\
               'v4-key-version-sweep', 2, 0, strftime('%s','now'),\n\
               CASE WHEN (SELECT COUNT(*) FROM clipboard_items WHERE key_version = 1) = 0\n\
                    THEN strftime('%s','now') ELSE NULL END\n\
             );\n",
        );
    }

    if current_version < 7 {
        // Migration v7 (v0.3 pinned-fix): add `pinned` column so explicitly
        // pinned items survive both the TTL prune and the history-limit prune.
        // `DEFAULT 0` backfills all existing rows as unpinned, which is safe:
        // items were pinned only by clearing `expires_at`, so no data is lost.
        if !column_exists(conn, "clipboard_items", "pinned")? {
            script.push_str(V7_ALTER_SQL);
        } else {
            script.push_str(
                "CREATE INDEX IF NOT EXISTS idx_clipboard_pinned \
                 ON clipboard_items(pinned) WHERE pinned = 1;\n",
            );
        }
    }

    if current_version < 8 {
        // Migration v8 (A1 drag-to-reorder): add `pin_order REAL DEFAULT NULL`
        // so pinned items carry an explicit sort key that the UI can update via
        // the `reorder_pinned` IPC verb. Existing pinned rows are backfilled
        // with their rowid so they start in a stable insertion-order sequence.
        // Unpinned rows keep NULL — the column is only meaningful for pinned items.
        if !column_exists(conn, "clipboard_items", "pin_order")? {
            script.push_str(V8_ALTER_SQL);
        }
        // If column exists, the UPDATE (backfill) was already applied; skip.
    }

    if current_version < 9 {
        // Migration v9 (Variant B image thumbnail): add `thumb BLOB DEFAULT
        // NULL` so image rows can carry a small capture-time encrypted preview.
        // `DEFAULT NULL` backfills existing rows with no thumbnail; this is safe
        // — the column is optional and the daemon backfills lazily.
        if !column_exists(conn, "clipboard_items", "thumb")? {
            script.push_str(V9_ALTER);
        }
    }

    if current_version < 10 {
        // Migration v10 (op-propagation foundation): add `deleted INTEGER NOT
        // NULL DEFAULT 0` for soft-delete tombstones that LWW-propagate across
        // devices. `DEFAULT 0` backfills existing rows as live. The partial
        // index on `deleted = 1` keeps tombstone enumeration efficient.
        if !column_exists(conn, "clipboard_items", "deleted")? {
            script.push_str(V10_ALTER);
        } else {
            script.push_str(
                "CREATE INDEX IF NOT EXISTS idx_clipboard_deleted \
                 ON clipboard_items(deleted) WHERE deleted = 1;\n",
            );
        }
    }

    if current_version < 11 {
        // Migration v11 (CopyPaste-pvp4): add a partial covering index on the
        // byte length of unpinned rows' content so `prune_to_cap`'s per-write
        // size gate computes its SUM index-only instead of full-scanning the
        // table and reading every encrypted BLOB. Index-only, no data change.
        script.push_str(V11_INDEX);
    }

    if current_version < 12 {
        // Migration v12 (CopyPaste-61fu): create the `revoked_devices` audit
        // table inside the versioned migration chain. Previously it was created
        // ad-hoc by `devices::ensure_revoked_devices_table`, which was called
        // explicitly at daemon startup — but any code path that invoked
        // `revoke_device` before that call (or on a DB opened without it) would
        // panic with "no such table: revoked_devices".
        //
        // `CREATE TABLE IF NOT EXISTS` / `CREATE INDEX IF NOT EXISTS` make this
        // step idempotent: DBs that already have the table from the old ad-hoc
        // path are unaffected.
        script.push_str(V12_REVOKED_DEVICES_SQL);
    }

    if current_version < 13 {
        // Migration v13 (CopyPaste-i6pp): purge stale clipboard_fts rows for
        // sensitive items. Before this fix, insert_item_with_fts and upsert_fts
        // did not guard against is_sensitive = 1, leaving plaintext secrets
        // (passwords, tokens, etc.) in the FTS table where search_items would
        // return them. This DELETE is idempotent and O(n_sensitive). Forward-
        // going code paths are patched separately to prevent new leakage.
        script.push_str(V13_PURGE_SENSITIVE_FTS);
    }

    if current_version < 14 {
        // Migration v14 (CopyPaste-89rd): add idx_clipboard_history_page so
        // get_page_pinned_first / get_page_pinned_first_lamport can use an
        // index range scan instead of a full-table scan + filesort on every
        // history_page IPC call. Index-only change — no data modified.
        script.push_str(V14_INDEX);
    }

    if current_version < 15 {
        // Migration v15 (CopyPaste-fuxl): exclude soft-deleted rows from the
        // per-minute dedup UNIQUE index so a re-copy after a same-bucket delete
        // creates a fresh live row instead of being silently dropped. See
        // V15_DEDUP_INDEX_FIX.
        script.push_str(V15_DEDUP_INDEX_FIX);
    }

    script.push_str(&format!("PRAGMA user_version={};\n", SCHEMA_VERSION));
    script.push_str("COMMIT;\n");
    Ok(script)
}
