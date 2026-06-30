use rusqlite::Connection;
use thiserror::Error;

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

/// Baseline (v1) schema as a single SQL script. Made `pub(crate)` so the
/// crate-internal `db` and `schema` tests can stage a legacy plaintext DB
/// without duplicating the SQL. Integration tests still inline a copy because
/// `include_str!` paths are crate-relative and not visible from `tests/`.
pub(crate) const V1_SCHEMA_SQL: &str = include_str!("schema_v1.sql");

/// v3 ALTER step — add `origin_device_id` to `clipboard_items`. SQLite
/// requires a literal constant default for `ALTER TABLE ADD COLUMN`, so we
/// use the empty string and let `items::backfill_origin_device_id` stamp the
/// real local UUID at daemon startup.
pub(crate) const V3_ALTER_SQL: &str = "\
ALTER TABLE clipboard_items \
    ADD COLUMN origin_device_id TEXT NOT NULL DEFAULT '';\n";

/// v4 ALTER step — add `key_version` to `clipboard_items`.
///
/// Default `1` ensures every existing row is marked as v1-key-encrypted, so
/// `super::migration_v4::migrate_v1_to_v2_keys` can find them via the
/// straightforward `WHERE key_version = 1` predicate. New `insert_item`
/// calls write the current key version (`2`) explicitly — the `DEFAULT 1`
/// here is exclusively for the existing-row backfill case.
pub(crate) const V4_ALTER_SQL: &str = "\
ALTER TABLE clipboard_items \
    ADD COLUMN key_version INTEGER NOT NULL DEFAULT 1;\n\
CREATE INDEX IF NOT EXISTS idx_clipboard_key_version \
    ON clipboard_items(key_version) WHERE key_version < 2;\n";

/// v5 step — add two UNIQUE INDEXes (`content_hash`+minute-bucket for TOCTOU
/// dedup, `item_id` for sync replay protection). Originally landed in beta
/// as user_version=4 (V4_INDEXES_SQL) but v3 already claimed v4 for
/// key_version. Bumped to v5 on merge into v0.3.
///
/// SQL file kept as `schema_v2.sql` for historical reasons.
pub(crate) const V5_INDEXES_SQL: &str = include_str!("schema_v2.sql");

/// v7 ALTER step — add `pinned` column to `clipboard_items`.
///
/// `DEFAULT 0` means all existing rows are treated as unpinned, which is
/// correct: items pinned under the old scheme (where `pin_item` only cleared
/// `expires_at`) become re-pinnable via the updated `pin_item` call that now
/// also sets `pinned = 1`. The `DEFAULT 0` here is exclusively for the
/// existing-row backfill case.
pub(crate) const V7_ALTER_SQL: &str = "\
ALTER TABLE clipboard_items \
    ADD COLUMN pinned INTEGER NOT NULL DEFAULT 0;\n\
CREATE INDEX IF NOT EXISTS idx_clipboard_pinned \
    ON clipboard_items(pinned) WHERE pinned = 1;\n";

/// v8 ALTER step — add `pin_order REAL DEFAULT NULL` to `clipboard_items`.
///
/// `DEFAULT NULL` means all existing rows start with no explicit order.
/// The migration immediately backfills all currently-pinned rows with
/// `CAST(rowid AS REAL)` to provide a stable initial order consistent with
/// insertion order.  Unpinned rows keep `NULL` and are never given a
/// `pin_order` value (the column is only meaningful when `pinned = 1`).
///
/// `pin_order` is a REAL so fractional values can be inserted between two
/// adjacent integers without renumbering the whole set (reserved for future
/// optimistic client-side reorder without a round-trip).
pub(crate) const V8_ALTER_SQL: &str = "\
ALTER TABLE clipboard_items \
    ADD COLUMN pin_order REAL DEFAULT NULL;\n\
UPDATE clipboard_items \
    SET pin_order = CAST(rowid AS REAL) WHERE pinned = 1;\n";

/// v9 ALTER step — add `thumb BLOB DEFAULT NULL` to `clipboard_items`.
///
/// `DEFAULT NULL` means every existing row (and any text row) carries no
/// thumbnail. Image rows captured after this migration store a small
/// XChaCha20-Poly1305-encrypted preview blob here (produced by
/// `image::encode_image_full` / `image::encode_thumbnail`); older image rows
/// can be lazily backfilled later via `items::set_thumb`. SQLite requires a
/// literal constant default for `ALTER TABLE ADD COLUMN`, and `NULL` is the
/// correct "no thumbnail yet" sentinel.
pub(crate) const V9_ALTER: &str = "\
ALTER TABLE clipboard_items ADD COLUMN thumb BLOB DEFAULT NULL;\n";

/// v10 ALTER step — add `deleted INTEGER NOT NULL DEFAULT 0` to
/// `clipboard_items` (op-propagation foundation).
///
/// `DEFAULT 0` backfills all existing rows as live (not soft-deleted). The
/// partial index covers only the tombstone minority so tombstone enumeration
/// during sync catchup remains O(tombstones) rather than a full-table scan.
/// UI list queries add `AND deleted = 0`; the merge layer reads through the
/// filter via `get_item_by_item_id` (no deleted filter) so LWW can apply
/// tombstone wins correctly.
pub(crate) const V10_ALTER: &str = "\
ALTER TABLE clipboard_items ADD COLUMN deleted INTEGER NOT NULL DEFAULT 0;\n\
CREATE INDEX IF NOT EXISTS idx_clipboard_deleted \
    ON clipboard_items(deleted) WHERE deleted = 1;\n";

/// v11 step — partial covering index that lets `prune_to_cap`'s size gate run
/// `SUM(LENGTH(content)) WHERE pinned = 0` as an **index-only** scan instead of
/// a full-table scan that reads (and discards) every encrypted `content` BLOB
/// on every clipboard write (CopyPaste-pvp4).
///
/// The pre-existing `idx_clipboard_pinned` is partial on `WHERE pinned = 1`, so
/// the inverted `pinned = 0` predicate could not use it. This index stores the
/// byte length of each unpinned row's `content` as the indexed expression and is
/// partial on `WHERE pinned = 0`, so the running `SUM` reads only the small
/// index B-tree — no table rows, no BLOB I/O. Cap eviction semantics are
/// unchanged (this only accelerates the cheap gate that decides whether any
/// pruning is needed at all).
pub(crate) const V11_INDEX: &str = "\
CREATE INDEX IF NOT EXISTS idx_clipboard_unpinned_len \
    ON clipboard_items(LENGTH(COALESCE(content, ''))) WHERE pinned = 0;\n";

/// v12 step — create the `revoked_devices` audit table and its timestamp index
/// (CopyPaste-61fu).
///
/// Previously this table was created ad-hoc by `devices::ensure_revoked_devices_table`
/// outside the migration sequence, which caused "no such table" panics on any DB
/// that was opened without an explicit call to that helper (e.g. daemons that were
/// newly upgraded and hadn't reached the post-open `ensure_revoked_devices_table`
/// call before a `revoke_device` was attempted).
///
/// Moving the DDL here guarantees the table exists in every DB that passes through
/// `apply_migrations`, regardless of which caller path opens it.  `CREATE TABLE IF
/// NOT EXISTS` / `CREATE INDEX IF NOT EXISTS` keep the step idempotent so DBs that
/// already have the table (created by the old ad-hoc path) are not affected.
pub(crate) const V12_REVOKED_DEVICES_SQL: &str = "\
CREATE TABLE IF NOT EXISTS revoked_devices (\n\
    fingerprint TEXT PRIMARY KEY NOT NULL,\n\
    name        TEXT NOT NULL DEFAULT '',\n\
    revoked_at  INTEGER NOT NULL\n\
);\n\
CREATE INDEX IF NOT EXISTS idx_revoked_devices_revoked_at\n\
    ON revoked_devices(revoked_at DESC);\n";

/// v13 step — purge stale `clipboard_fts` rows for sensitive items
/// (CopyPaste-i6pp).
///
/// Prior to this fix, `insert_item_with_fts` and `upsert_fts` did not check
/// `is_sensitive` before writing to `clipboard_fts`. As a result, existing
/// databases may contain plaintext secrets (passwords, tokens, credit-card
/// numbers) in the FTS table, where they would surface as search results to
/// any caller of `search_items`.
///
/// This DELETE is idempotent: on a clean database (no sensitive FTS rows) it
/// is a no-op. On an upgraded database it removes exactly the rows that leak
/// sensitive plaintext. The sub-select is a single indexed lookup on `id`
/// (PRIMARY KEY of `clipboard_items`) — O(n_sensitive) not O(n_total).
///
/// After this migration, the forward-going code paths (`insert_item_with_fts`,
/// `upsert_fts`, `search_items`) enforce the same policy at write and query
/// time respectively, so no new sensitive FTS rows can be created.
pub(crate) const V13_PURGE_SENSITIVE_FTS: &str = "\
DELETE FROM clipboard_fts\n\
WHERE id IN (\n\
    SELECT id FROM clipboard_items WHERE is_sensitive = 1\n\
);\n";

/// v14 step — partial covering index for `get_page_pinned_first` and
/// `get_page_pinned_first_lamport` (CopyPaste-89rd).
///
/// Root cause: the `history_page` IPC verb (the primary read path for both the
/// Tauri UI and the CLI `list` command) calls `get_page_pinned_first`, which
/// filters `WHERE deleted = 0` and sorts by
/// `CASE WHEN pinned=1 THEN 0 ELSE 1 END, pin_order IS NULL, pin_order,
///  wall_time DESC`.
///
/// The pre-existing `idx_clipboard_deleted` is partial on `WHERE deleted = 1`
/// (the tombstone minority) and therefore cannot be used for the live-row
/// path. `idx_clipboard_wall_time` covers `wall_time` but cannot be used when
/// a `CASE` expression leads the `ORDER BY` clause. SQLite therefore falls
/// back to a full-table scan + filesort on every call — O(n).
///
/// The new index is partial on `WHERE deleted = 0` (the live-row majority) and
/// covers `(pinned DESC, pin_order, wall_time DESC)`. SQLite can split the
/// sort into two bounded range scans — first the pinned=1 rows (already sorted
/// by `pin_order` via the index), then the pinned=0 rows (already sorted by
/// `wall_time DESC`). Result: `SEARCH clipboard_items USING INDEX` instead of
/// `SCAN clipboard_items`, verified by EXPLAIN QUERY PLAN.
pub(crate) const V14_INDEX: &str = "\
CREATE INDEX IF NOT EXISTS idx_clipboard_history_page\n\
    ON clipboard_items(pinned DESC, pin_order, wall_time DESC)\n\
    WHERE deleted = 0;\n";

/// CopyPaste-fuxl: rebuild the per-minute dedup UNIQUE index to EXCLUDE
/// soft-deleted rows. The original predicate (`content_hash IS NOT NULL`) kept
/// tombstones (`deleted = 1`) in the index, so re-copying content that was
/// soft-deleted within the SAME `wall_time / 60` bucket hit a UNIQUE violation
/// and the insert silently fell back to the tombstone id — the re-copy vanished.
/// Restricting to `deleted = 0` lets a re-copy create a fresh live row. The new
/// index covers a strict SUBSET of the old one's rows, so DROP+CREATE can never
/// fail on existing data (uniqueness over the deleted=0 subset was already held).
pub(crate) const V15_DEDUP_INDEX_FIX: &str = "\
DROP INDEX IF EXISTS idx_dedup_hash_minute;\n\
CREATE UNIQUE INDEX IF NOT EXISTS idx_dedup_hash_minute\n\
    ON clipboard_items(content_hash, (wall_time / 60))\n\
    WHERE content_hash IS NOT NULL AND deleted = 0;\n";

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

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn downgrade_returns_explicit_error() {
        // Open a fresh in-memory DB, run migrations to bring it to SCHEMA_VERSION,
        // then bump user_version past it to simulate a database written by a
        // newer build.
        let conn = Connection::open_in_memory().unwrap();
        apply_migrations(&conn).unwrap();
        conn.execute_batch("PRAGMA user_version = 999;").unwrap();

        let err = apply_migrations(&conn).unwrap_err();
        match err {
            SchemaError::Downgrade { found, expected } => {
                assert_eq!(found, 999);
                assert_eq!(expected, SCHEMA_VERSION);
            }
            other => panic!("expected SchemaError::Downgrade, got {:?}", other),
        }
    }

    /// CopyPaste-lmlr: `is_duplicate_column_error` must recognise the SQLite
    /// "duplicate column name" failure (the one the retry loop in
    /// `apply_migrations` is allowed to recover from) and must NOT match an
    /// unrelated error, so a genuine schema fault still propagates.
    #[test]
    fn is_duplicate_column_error_matches_only_duplicate_column() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE t (a INTEGER);").unwrap();
        conn.execute_batch("ALTER TABLE t ADD COLUMN b TEXT;")
            .unwrap();
        // Re-adding the same column raises "duplicate column name: b".
        let dup = conn
            .execute_batch("ALTER TABLE t ADD COLUMN b TEXT;")
            .unwrap_err();
        assert!(
            is_duplicate_column_error(&dup),
            "must detect duplicate-column error, got: {dup}"
        );
        // An unrelated error (missing table) must NOT be treated as retryable.
        let other = conn
            .execute_batch("ALTER TABLE nope ADD COLUMN c TEXT;")
            .unwrap_err();
        assert!(
            !is_duplicate_column_error(&other),
            "must not match unrelated error: {other}"
        );
    }

    /// CopyPaste-m45w: when `content_hash` already exists in the table but
    /// `user_version` is still at 1 (WAL-replay-onto-fresh-DB scenario triggered
    /// by `reset_database` racing with a concurrent connection), `apply_migrations`
    /// must skip the duplicate ALTER, apply all remaining steps, and reach
    /// `SCHEMA_VERSION` successfully — NOT fail with "duplicate column name".
    #[test]
    fn v2_migration_idempotent_when_column_exists() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(V1_SCHEMA_SQL).unwrap();
        conn.execute_batch("PRAGMA user_version = 1;").unwrap();

        // Pre-add the column that v2 would normally add (simulates WAL replay).
        conn.execute_batch("ALTER TABLE clipboard_items ADD COLUMN content_hash TEXT;")
            .unwrap();

        // Migration must now SUCCEED (idempotent guard skips the duplicate ALTER).
        let result = apply_migrations(&conn);
        assert!(
            result.is_ok(),
            "migration must succeed when content_hash already exists: {result:?}"
        );

        // Must have reached the current schema version.
        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            version, SCHEMA_VERSION,
            "user_version must reach SCHEMA_VERSION even when content_hash pre-exists"
        );

        // content_hash must appear exactly once.
        let mut stmt = conn.prepare("PRAGMA table_info(clipboard_items)").unwrap();
        let count = stmt
            .query_map([], |r| r.get::<_, String>(1))
            .unwrap()
            .filter_map(|r| r.ok())
            .filter(|name| name == "content_hash")
            .count();
        assert_eq!(count, 1, "content_hash must appear exactly once in schema");
    }

    /// Verify that the entire migration block is still atomic: when a step that
    /// cannot be skipped (v13 purges clipboard_fts) fails because the table was
    /// removed from the DB, `user_version` must remain unchanged.
    #[test]
    fn apply_migrations_is_atomic_on_failure() {
        // Build a v12 state but deliberately DROP `clipboard_fts` so the v13
        // migration (DELETE FROM clipboard_fts …) will fail with "no such table".
        // The BEGIN…COMMIT block must roll back in full, leaving user_version at 12.
        let conn = Connection::open_in_memory().unwrap();

        // Bring the DB up to the v12 schema shape by hand.
        conn.execute_batch(V1_SCHEMA_SQL).unwrap();
        conn.execute_batch(
            "ALTER TABLE clipboard_items ADD COLUMN content_hash TEXT;\n\
             ALTER TABLE clipboard_items ADD COLUMN origin_device_id TEXT NOT NULL DEFAULT '';\n\
             ALTER TABLE clipboard_items ADD COLUMN key_version INTEGER NOT NULL DEFAULT 1;\n\
             ALTER TABLE clipboard_items ADD COLUMN pinned INTEGER NOT NULL DEFAULT 0;\n\
             ALTER TABLE clipboard_items ADD COLUMN pin_order REAL DEFAULT NULL;\n\
             ALTER TABLE clipboard_items ADD COLUMN thumb BLOB DEFAULT NULL;\n\
             ALTER TABLE clipboard_items ADD COLUMN deleted INTEGER NOT NULL DEFAULT 0;\n\
             CREATE TABLE IF NOT EXISTS migration_state (\
               key TEXT PRIMARY KEY, key_version_in_progress INTEGER,\
               last_processed_id INTEGER NOT NULL DEFAULT 0,\
               started_at INTEGER, completed_at INTEGER);\n\
             INSERT OR IGNORE INTO migration_state VALUES ('v4-key-version-sweep', 2, 0, 0, 0);\n\
             CREATE TABLE IF NOT EXISTS revoked_devices (\
               fingerprint TEXT PRIMARY KEY NOT NULL,\
               name TEXT NOT NULL DEFAULT '',\
               revoked_at INTEGER NOT NULL);",
        )
        .unwrap();
        conn.execute_batch("PRAGMA user_version = 12;").unwrap();

        // Drop clipboard_fts so that the v13 DELETE fails.
        conn.execute_batch("DROP TABLE IF EXISTS clipboard_fts;")
            .unwrap();

        // The migration must fail (v13 cannot purge a table that doesn't exist).
        let result = apply_migrations(&conn);
        assert!(
            result.is_err(),
            "migration must fail when clipboard_fts is absent"
        );

        // user_version must NOT have advanced — the transaction was rolled back.
        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            version, 12,
            "user_version must remain at 12 after a rolled-back migration"
        );
    }

    #[test]
    fn fresh_db_reaches_current_schema_version() {
        let conn = Connection::open_in_memory().unwrap();
        apply_migrations(&conn).unwrap();
        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(version, SCHEMA_VERSION);
    }

    #[test]
    fn equal_version_is_noop() {
        let conn = Connection::open_in_memory().unwrap();
        apply_migrations(&conn).unwrap();
        // Second call hits the `current_version == SCHEMA_VERSION` fast path.
        apply_migrations(&conn).unwrap();
    }

    #[test]
    fn fresh_db_has_origin_device_id_column() {
        let conn = Connection::open_in_memory().unwrap();
        apply_migrations(&conn).unwrap();
        let mut stmt = conn.prepare("PRAGMA table_info(clipboard_items)").unwrap();
        let cols: Vec<String> = stmt
            .query_map([], |r| r.get::<_, String>(1))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert!(
            cols.iter().any(|c| c == "origin_device_id"),
            "v3 schema must include origin_device_id column"
        );
    }

    #[test]
    fn fresh_db_has_key_version_column() {
        let conn = Connection::open_in_memory().unwrap();
        apply_migrations(&conn).unwrap();
        let mut stmt = conn.prepare("PRAGMA table_info(clipboard_items)").unwrap();
        let cols: Vec<(String, String, i64)> = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(1)?, // column name
                    r.get::<_, String>(2)?, // declared type
                    r.get::<_, i64>(3)?,    // notnull
                ))
            })
            .unwrap()
            .map(|r| r.unwrap())
            .collect();

        let kv = cols
            .iter()
            .find(|c| c.0 == "key_version")
            .expect("v4 schema must include key_version column");
        assert_eq!(kv.1.to_uppercase(), "INTEGER");
        assert_eq!(kv.2, 1, "key_version must be NOT NULL");
    }

    #[test]
    fn fresh_db_has_migration_state_table() {
        let conn = Connection::open_in_memory().unwrap();
        apply_migrations(&conn).unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='migration_state'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            count, 1,
            "migration_state table must be created by v6 migration"
        );

        // The seed row must be present.
        let row_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM migration_state WHERE key = 'v4-key-version-sweep'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(row_count, 1, "seed row must be inserted by v6 migration");
    }

    #[test]
    fn v3_to_v4_migration_marks_existing_rows_as_v1_key() {
        // Bring a fresh DB only up to v3 by short-circuiting the v4 step,
        // then re-run apply_migrations and assert existing rows landed on
        // key_version=1 (the DEFAULT in V4_ALTER_SQL).
        let conn = Connection::open_in_memory().unwrap();

        // Hand-build v3 state.
        conn.execute_batch(V1_SCHEMA_SQL).unwrap();
        conn.execute_batch(
            "ALTER TABLE clipboard_items ADD COLUMN content_hash TEXT;\n\
             ALTER TABLE clipboard_items ADD COLUMN origin_device_id TEXT NOT NULL DEFAULT '';",
        )
        .unwrap();
        conn.execute_batch("PRAGMA user_version = 3;").unwrap();

        // Insert a v3-era row.
        conn.execute(
            "INSERT INTO clipboard_items \
             (id, item_id, content_type, lamport_ts, wall_time, content_hash, origin_device_id) \
             VALUES ('id-1', 'item-1', 'text', 1, 1000, NULL, '')",
            [],
        )
        .unwrap();

        // Run apply_migrations → must add key_version column and DEFAULT 1
        // backfills the pre-existing row.
        apply_migrations(&conn).unwrap();

        let kv: i64 = conn
            .query_row(
                "SELECT key_version FROM clipboard_items WHERE id = 'id-1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            kv, 1,
            "pre-v4 rows must land on key_version=1 so the v1→v2 sweep can find them"
        );

        let uv: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(uv, SCHEMA_VERSION);
    }

    #[test]
    fn fresh_db_has_pinned_column() {
        let conn = Connection::open_in_memory().unwrap();
        apply_migrations(&conn).unwrap();
        let mut stmt = conn.prepare("PRAGMA table_info(clipboard_items)").unwrap();
        let cols: Vec<(String, String, i64)> = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(1)?, // column name
                    r.get::<_, String>(2)?, // declared type
                    r.get::<_, i64>(3)?,    // notnull
                ))
            })
            .unwrap()
            .map(|r| r.unwrap())
            .collect();

        let pinned_col = cols
            .iter()
            .find(|c| c.0 == "pinned")
            .expect("v7 schema must include pinned column");
        assert_eq!(pinned_col.1.to_uppercase(), "INTEGER");
        assert_eq!(pinned_col.2, 1, "pinned must be NOT NULL");
    }

    #[test]
    fn fresh_db_has_thumb_column() {
        let conn = Connection::open_in_memory().unwrap();
        apply_migrations(&conn).unwrap();
        let mut stmt = conn.prepare("PRAGMA table_info(clipboard_items)").unwrap();
        let cols: Vec<(String, String)> = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(1)?, // column name
                    r.get::<_, String>(2)?, // declared type
                ))
            })
            .unwrap()
            .map(|r| r.unwrap())
            .collect();

        let thumb = cols
            .iter()
            .find(|c| c.0 == "thumb")
            .expect("v9 schema must include thumb column");
        assert_eq!(thumb.1.to_uppercase(), "BLOB");
    }

    #[test]
    fn v8_to_v9_migration_backfills_existing_rows_with_null_thumb() {
        // Simulate a v8 database (no thumb column), run migrations, and verify
        // existing rows land on thumb = NULL (the DEFAULT in V9_ALTER) and the
        // user_version reaches the current SCHEMA_VERSION.
        let conn = Connection::open_in_memory().unwrap();

        // Bring a fresh DB fully up to v8 by short-circuiting the v9 step: run
        // the real migrator (it will go straight to 9), then we can't easily
        // stop at 8 — so hand-build the v8 shape instead.
        conn.execute_batch(V1_SCHEMA_SQL).unwrap();
        conn.execute_batch(
            "ALTER TABLE clipboard_items ADD COLUMN content_hash TEXT;\n\
             ALTER TABLE clipboard_items ADD COLUMN origin_device_id TEXT NOT NULL DEFAULT '';\n\
             ALTER TABLE clipboard_items ADD COLUMN key_version INTEGER NOT NULL DEFAULT 1;\n\
             ALTER TABLE clipboard_items ADD COLUMN pinned INTEGER NOT NULL DEFAULT 0;\n\
             ALTER TABLE clipboard_items ADD COLUMN pin_order REAL DEFAULT NULL;\n\
             CREATE TABLE IF NOT EXISTS migration_state (\
               key TEXT PRIMARY KEY, key_version_in_progress INTEGER,\
               last_processed_id INTEGER NOT NULL DEFAULT 0,\
               started_at INTEGER, completed_at INTEGER);\n\
             INSERT OR IGNORE INTO migration_state VALUES ('v4-key-version-sweep', 2, 0, 0, 0);",
        )
        .unwrap();
        conn.execute_batch("PRAGMA user_version = 8;").unwrap();

        // Insert a v8-era row (no thumb column yet).
        conn.execute(
            "INSERT INTO clipboard_items \
             (id, item_id, content_type, lamport_ts, wall_time, origin_device_id, key_version, pinned) \
             VALUES ('id-v8', 'item-v8', 'image', 1, 1000, '', 2, 0)",
            [],
        )
        .unwrap();

        // Run apply_migrations → must add thumb column, DEFAULT NULL backfills.
        apply_migrations(&conn).unwrap();

        let thumb: Option<Vec<u8>> = conn
            .query_row(
                "SELECT thumb FROM clipboard_items WHERE id = 'id-v8'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(thumb.is_none(), "pre-v9 rows must land on thumb = NULL");

        let uv: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(uv, SCHEMA_VERSION);
    }

    #[test]
    fn v6_to_v7_migration_backfills_existing_rows_as_unpinned() {
        // Simulate a v6 database (no pinned column), run migrations, and
        // verify existing rows land on pinned=0 (the DEFAULT in V7_ALTER_SQL).
        let conn = Connection::open_in_memory().unwrap();

        // Hand-build v6 state.
        conn.execute_batch(V1_SCHEMA_SQL).unwrap();
        conn.execute_batch(
            "ALTER TABLE clipboard_items ADD COLUMN content_hash TEXT;\n\
             ALTER TABLE clipboard_items ADD COLUMN origin_device_id TEXT NOT NULL DEFAULT '';\n\
             ALTER TABLE clipboard_items ADD COLUMN key_version INTEGER NOT NULL DEFAULT 1;\n\
             CREATE TABLE IF NOT EXISTS migration_state (\
               key TEXT PRIMARY KEY, key_version_in_progress INTEGER,\
               last_processed_id INTEGER NOT NULL DEFAULT 0,\
               started_at INTEGER, completed_at INTEGER);\n\
             INSERT OR IGNORE INTO migration_state VALUES ('v4-key-version-sweep', 2, 0, 0, 0);",
        )
        .unwrap();
        conn.execute_batch("PRAGMA user_version = 6;").unwrap();

        // Insert a v6-era row (no pinned column yet).
        conn.execute(
            "INSERT INTO clipboard_items \
             (id, item_id, content_type, lamport_ts, wall_time, origin_device_id, key_version) \
             VALUES ('id-v6', 'item-v6', 'text', 1, 1000, '', 2)",
            [],
        )
        .unwrap();

        // Run apply_migrations → must add pinned column, DEFAULT 0 backfills.
        apply_migrations(&conn).unwrap();

        let pinned: i64 = conn
            .query_row(
                "SELECT pinned FROM clipboard_items WHERE id = 'id-v6'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(pinned, 0, "pre-v7 rows must land on pinned=0");

        let uv: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(uv, SCHEMA_VERSION);
    }

    #[test]
    fn fresh_db_has_deleted_column() {
        let conn = Connection::open_in_memory().unwrap();
        apply_migrations(&conn).unwrap();
        let mut stmt = conn.prepare("PRAGMA table_info(clipboard_items)").unwrap();
        let cols: Vec<(String, String, i64)> = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(1)?, // column name
                    r.get::<_, String>(2)?, // declared type
                    r.get::<_, i64>(3)?,    // notnull
                ))
            })
            .unwrap()
            .map(|r| r.unwrap())
            .collect();

        let deleted_col = cols
            .iter()
            .find(|c| c.0 == "deleted")
            .expect("v10 schema must include deleted column");
        assert_eq!(deleted_col.1.to_uppercase(), "INTEGER");
        assert_eq!(deleted_col.2, 1, "deleted must be NOT NULL");
    }

    #[test]
    fn v9_to_v10_migration_backfills_existing_rows_as_not_deleted() {
        // Simulate a v9 database (no deleted column), run migrations, and verify
        // existing rows land on deleted=0 (the DEFAULT in V10_ALTER) and the
        // user_version reaches the current SCHEMA_VERSION.
        let conn = Connection::open_in_memory().unwrap();

        // Hand-build v9 state.
        conn.execute_batch(V1_SCHEMA_SQL).unwrap();
        conn.execute_batch(
            "ALTER TABLE clipboard_items ADD COLUMN content_hash TEXT;\n\
             ALTER TABLE clipboard_items ADD COLUMN origin_device_id TEXT NOT NULL DEFAULT '';\n\
             ALTER TABLE clipboard_items ADD COLUMN key_version INTEGER NOT NULL DEFAULT 1;\n\
             ALTER TABLE clipboard_items ADD COLUMN pinned INTEGER NOT NULL DEFAULT 0;\n\
             ALTER TABLE clipboard_items ADD COLUMN pin_order REAL DEFAULT NULL;\n\
             ALTER TABLE clipboard_items ADD COLUMN thumb BLOB DEFAULT NULL;\n\
             CREATE TABLE IF NOT EXISTS migration_state (\
               key TEXT PRIMARY KEY, key_version_in_progress INTEGER,\
               last_processed_id INTEGER NOT NULL DEFAULT 0,\
               started_at INTEGER, completed_at INTEGER);\n\
             INSERT OR IGNORE INTO migration_state VALUES ('v4-key-version-sweep', 2, 0, 0, 0);",
        )
        .unwrap();
        conn.execute_batch("PRAGMA user_version = 9;").unwrap();

        // Insert a v9-era row (no deleted column yet).
        conn.execute(
            "INSERT INTO clipboard_items \
             (id, item_id, content_type, lamport_ts, wall_time, origin_device_id, key_version, pinned) \
             VALUES ('id-v9', 'item-v9', 'text', 1, 1000, '', 2, 0)",
            [],
        )
        .unwrap();

        // Run apply_migrations → must add deleted column, DEFAULT 0 backfills.
        apply_migrations(&conn).unwrap();

        let deleted: i64 = conn
            .query_row(
                "SELECT deleted FROM clipboard_items WHERE id = 'id-v9'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(deleted, 0, "pre-v10 rows must land on deleted=0");

        let uv: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(uv, SCHEMA_VERSION);
    }

    /// CopyPaste-61fu: migration v12 must create the `revoked_devices` table and
    /// its index as part of the standard migration chain so that the table exists
    /// on every properly-initialised DB without requiring an explicit
    /// `ensure_revoked_devices_table` call.
    #[test]
    fn fresh_db_has_revoked_devices_table() {
        let conn = Connection::open_in_memory().unwrap();
        apply_migrations(&conn).unwrap();

        // Table must exist.
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master \
                 WHERE type='table' AND name='revoked_devices'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            count, 1,
            "revoked_devices table must be created by v12 migration"
        );

        // Index must exist.
        let idx_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master \
                 WHERE type='index' AND name='idx_revoked_devices_revoked_at'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            idx_count, 1,
            "idx_revoked_devices_revoked_at must be created by v12 migration"
        );
    }

    /// CopyPaste-61fu: a v11 database (no revoked_devices table) upgraded via
    /// apply_migrations must end up with the table and user_version == SCHEMA_VERSION.
    #[test]
    fn v11_to_v12_migration_creates_revoked_devices_table() {
        let conn = Connection::open_in_memory().unwrap();

        // Hand-build a v11 state: all v1–v11 changes, no revoked_devices table.
        conn.execute_batch(V1_SCHEMA_SQL).unwrap();
        conn.execute_batch(
            "ALTER TABLE clipboard_items ADD COLUMN content_hash TEXT;\n\
             ALTER TABLE clipboard_items ADD COLUMN origin_device_id TEXT NOT NULL DEFAULT '';\n\
             ALTER TABLE clipboard_items ADD COLUMN key_version INTEGER NOT NULL DEFAULT 1;\n\
             ALTER TABLE clipboard_items ADD COLUMN pinned INTEGER NOT NULL DEFAULT 0;\n\
             ALTER TABLE clipboard_items ADD COLUMN pin_order REAL DEFAULT NULL;\n\
             ALTER TABLE clipboard_items ADD COLUMN thumb BLOB DEFAULT NULL;\n\
             ALTER TABLE clipboard_items ADD COLUMN deleted INTEGER NOT NULL DEFAULT 0;\n\
             CREATE TABLE IF NOT EXISTS migration_state (\
               key TEXT PRIMARY KEY, key_version_in_progress INTEGER,\
               last_processed_id INTEGER NOT NULL DEFAULT 0,\
               started_at INTEGER, completed_at INTEGER);\n\
             INSERT OR IGNORE INTO migration_state VALUES ('v4-key-version-sweep', 2, 0, 0, 0);\n\
             CREATE INDEX IF NOT EXISTS idx_clipboard_unpinned_len \
               ON clipboard_items(LENGTH(COALESCE(content, ''))) WHERE pinned = 0;",
        )
        .unwrap();
        conn.execute_batch("PRAGMA user_version = 11;").unwrap();

        // Sanity: table must not exist before migration.
        let before: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master \
                 WHERE type='table' AND name='revoked_devices'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            before, 0,
            "revoked_devices must not exist before v12 migration"
        );

        // Run the migration.
        apply_migrations(&conn).unwrap();

        // Table must now exist.
        let after: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master \
                 WHERE type='table' AND name='revoked_devices'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(after, 1, "revoked_devices must be created by v12 migration");

        let uv: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(uv, SCHEMA_VERSION);
    }

    /// CopyPaste-61fu: a DB that already has the revoked_devices table (created
    /// by the old ad-hoc path) must survive the v12 migration without error
    /// (CREATE TABLE IF NOT EXISTS is idempotent).
    #[test]
    fn v12_migration_is_idempotent_when_table_already_exists() {
        let conn = Connection::open_in_memory().unwrap();

        // Build a v11 state that already has revoked_devices (the old ad-hoc path).
        conn.execute_batch(V1_SCHEMA_SQL).unwrap();
        conn.execute_batch(
            "ALTER TABLE clipboard_items ADD COLUMN content_hash TEXT;\n\
             ALTER TABLE clipboard_items ADD COLUMN origin_device_id TEXT NOT NULL DEFAULT '';\n\
             ALTER TABLE clipboard_items ADD COLUMN key_version INTEGER NOT NULL DEFAULT 1;\n\
             ALTER TABLE clipboard_items ADD COLUMN pinned INTEGER NOT NULL DEFAULT 0;\n\
             ALTER TABLE clipboard_items ADD COLUMN pin_order REAL DEFAULT NULL;\n\
             ALTER TABLE clipboard_items ADD COLUMN thumb BLOB DEFAULT NULL;\n\
             ALTER TABLE clipboard_items ADD COLUMN deleted INTEGER NOT NULL DEFAULT 0;\n\
             CREATE TABLE IF NOT EXISTS migration_state (\
               key TEXT PRIMARY KEY, key_version_in_progress INTEGER,\
               last_processed_id INTEGER NOT NULL DEFAULT 0,\
               started_at INTEGER, completed_at INTEGER);\n\
             INSERT OR IGNORE INTO migration_state VALUES ('v4-key-version-sweep', 2, 0, 0, 0);\n\
             CREATE INDEX IF NOT EXISTS idx_clipboard_unpinned_len \
               ON clipboard_items(LENGTH(COALESCE(content, ''))) WHERE pinned = 0;\n\
             CREATE TABLE IF NOT EXISTS revoked_devices (\
               fingerprint TEXT PRIMARY KEY NOT NULL,\
               name TEXT NOT NULL DEFAULT '',\
               revoked_at INTEGER NOT NULL);\n\
             CREATE INDEX IF NOT EXISTS idx_revoked_devices_revoked_at \
               ON revoked_devices(revoked_at DESC);",
        )
        .unwrap();
        conn.execute_batch("PRAGMA user_version = 11;").unwrap();

        // Migration must succeed without error even though the table already exists.
        apply_migrations(&conn).unwrap();

        let uv: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(uv, SCHEMA_VERSION);
    }

    /// CopyPaste-i6pp: migration v13 must delete clipboard_fts rows that
    /// belong to sensitive items, and leave non-sensitive FTS rows intact.
    #[test]
    fn v13_migration_purges_sensitive_fts_rows() {
        let conn = Connection::open_in_memory().unwrap();

        // Build a v12 state with clipboard_items + clipboard_fts.
        conn.execute_batch(V1_SCHEMA_SQL).unwrap();
        conn.execute_batch(
            "ALTER TABLE clipboard_items ADD COLUMN content_hash TEXT;\n\
             ALTER TABLE clipboard_items ADD COLUMN origin_device_id TEXT NOT NULL DEFAULT '';\n\
             ALTER TABLE clipboard_items ADD COLUMN key_version INTEGER NOT NULL DEFAULT 1;\n\
             ALTER TABLE clipboard_items ADD COLUMN pinned INTEGER NOT NULL DEFAULT 0;\n\
             ALTER TABLE clipboard_items ADD COLUMN pin_order REAL DEFAULT NULL;\n\
             ALTER TABLE clipboard_items ADD COLUMN thumb BLOB DEFAULT NULL;\n\
             ALTER TABLE clipboard_items ADD COLUMN deleted INTEGER NOT NULL DEFAULT 0;\n\
             CREATE TABLE IF NOT EXISTS migration_state (\
               key TEXT PRIMARY KEY, key_version_in_progress INTEGER,\
               last_processed_id INTEGER NOT NULL DEFAULT 0,\
               started_at INTEGER, completed_at INTEGER);\n\
             INSERT OR IGNORE INTO migration_state VALUES ('v4-key-version-sweep', 2, 0, 0, 0);\n\
             CREATE INDEX IF NOT EXISTS idx_clipboard_unpinned_len \
               ON clipboard_items(LENGTH(COALESCE(content, ''))) WHERE pinned = 0;\n\
             CREATE TABLE IF NOT EXISTS revoked_devices (\
               fingerprint TEXT PRIMARY KEY NOT NULL,\
               name TEXT NOT NULL DEFAULT '',\
               revoked_at INTEGER NOT NULL);\n\
             CREATE INDEX IF NOT EXISTS idx_revoked_devices_revoked_at \
               ON revoked_devices(revoked_at DESC);",
        )
        .unwrap();
        conn.execute_batch("PRAGMA user_version = 12;").unwrap();

        // Insert one sensitive and one non-sensitive item.
        conn.execute(
            "INSERT INTO clipboard_items \
             (id, item_id, content_type, lamport_ts, wall_time, origin_device_id, key_version, pinned, is_sensitive) \
             VALUES ('id-secret', 'iid-s', 'text', 1, 1000, '', 2, 0, 1)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO clipboard_items \
             (id, item_id, content_type, lamport_ts, wall_time, origin_device_id, key_version, pinned, is_sensitive) \
             VALUES ('id-normal', 'iid-n', 'text', 2, 2000, '', 2, 0, 0)",
            [],
        )
        .unwrap();

        // Simulate the pre-fix bug: both items have FTS rows.
        conn.execute(
            "INSERT INTO clipboard_fts(id, content_text) VALUES ('id-secret', 'my super secret password')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO clipboard_fts(id, content_text) VALUES ('id-normal', 'ordinary clipboard text')",
            [],
        )
        .unwrap();

        // Sanity: both FTS rows exist before migration.
        let before: i64 = conn
            .query_row("SELECT COUNT(*) FROM clipboard_fts", [], |r| r.get(0))
            .unwrap();
        assert_eq!(before, 2, "both FTS rows must exist before v13 migration");

        // Run migration.
        apply_migrations(&conn).unwrap();

        // Sensitive FTS row must be gone.
        let secret_fts: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM clipboard_fts WHERE id = 'id-secret'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            secret_fts, 0,
            "v13 migration must remove FTS row for sensitive item (CopyPaste-i6pp)"
        );

        // Non-sensitive FTS row must survive.
        let normal_fts: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM clipboard_fts WHERE id = 'id-normal'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            normal_fts, 1,
            "v13 migration must preserve FTS row for non-sensitive item"
        );

        let uv: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(uv, SCHEMA_VERSION);
    }

    /// CopyPaste-i6pp: v13 migration is idempotent — running it on a DB that
    /// has no sensitive FTS rows must succeed without error.
    #[test]
    fn v13_migration_is_noop_when_no_sensitive_fts_rows_exist() {
        let conn = Connection::open_in_memory().unwrap();

        // Build a v12 state with only a non-sensitive item.
        conn.execute_batch(V1_SCHEMA_SQL).unwrap();
        conn.execute_batch(
            "ALTER TABLE clipboard_items ADD COLUMN content_hash TEXT;\n\
             ALTER TABLE clipboard_items ADD COLUMN origin_device_id TEXT NOT NULL DEFAULT '';\n\
             ALTER TABLE clipboard_items ADD COLUMN key_version INTEGER NOT NULL DEFAULT 1;\n\
             ALTER TABLE clipboard_items ADD COLUMN pinned INTEGER NOT NULL DEFAULT 0;\n\
             ALTER TABLE clipboard_items ADD COLUMN pin_order REAL DEFAULT NULL;\n\
             ALTER TABLE clipboard_items ADD COLUMN thumb BLOB DEFAULT NULL;\n\
             ALTER TABLE clipboard_items ADD COLUMN deleted INTEGER NOT NULL DEFAULT 0;\n\
             CREATE TABLE IF NOT EXISTS migration_state (\
               key TEXT PRIMARY KEY, key_version_in_progress INTEGER,\
               last_processed_id INTEGER NOT NULL DEFAULT 0,\
               started_at INTEGER, completed_at INTEGER);\n\
             INSERT OR IGNORE INTO migration_state VALUES ('v4-key-version-sweep', 2, 0, 0, 0);\n\
             CREATE INDEX IF NOT EXISTS idx_clipboard_unpinned_len \
               ON clipboard_items(LENGTH(COALESCE(content, ''))) WHERE pinned = 0;\n\
             CREATE TABLE IF NOT EXISTS revoked_devices (\
               fingerprint TEXT PRIMARY KEY NOT NULL,\
               name TEXT NOT NULL DEFAULT '',\
               revoked_at INTEGER NOT NULL);\n\
             CREATE INDEX IF NOT EXISTS idx_revoked_devices_revoked_at \
               ON revoked_devices(revoked_at DESC);",
        )
        .unwrap();
        conn.execute_batch("PRAGMA user_version = 12;").unwrap();

        conn.execute(
            "INSERT INTO clipboard_items \
             (id, item_id, content_type, lamport_ts, wall_time, origin_device_id, key_version, pinned, is_sensitive) \
             VALUES ('id-n', 'iid-n', 'text', 1, 1000, '', 2, 0, 0)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO clipboard_fts(id, content_text) VALUES ('id-n', 'hello world')",
            [],
        )
        .unwrap();

        // Must succeed without error.
        apply_migrations(&conn).unwrap();

        let fts: i64 = conn
            .query_row("SELECT COUNT(*) FROM clipboard_fts", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            fts, 1,
            "non-sensitive FTS row must survive a no-op v13 migration"
        );

        let uv: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(uv, SCHEMA_VERSION);
    }

    /// CopyPaste-89rd: migration v14 must create `idx_clipboard_history_page` so
    /// `get_page_pinned_first`'s `WHERE deleted=0 ORDER BY pinned DESC ...` query
    /// uses an index range scan instead of a full-table scan + filesort.
    ///
    /// Verified via EXPLAIN QUERY PLAN: the plan detail must contain "USING INDEX"
    /// referencing `idx_clipboard_history_page` rather than "SCAN clipboard_items".
    #[test]
    fn v14_migration_creates_history_page_index() {
        let conn = Connection::open_in_memory().unwrap();
        apply_migrations(&conn).unwrap();

        // Index must exist in sqlite_master.
        let idx_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master \
                 WHERE type='index' AND name='idx_clipboard_history_page'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            idx_count, 1,
            "idx_clipboard_history_page must be created by v14 migration (CopyPaste-89rd)"
        );
    }

    /// CopyPaste-89rd: EXPLAIN QUERY PLAN confirms the history-page query uses the
    /// new index rather than a full-table scan.
    ///
    /// This is the acceptance criterion from the issue: the plan detail must contain
    /// "USING INDEX idx_clipboard_history_page" (or equivalent "idx_clipboard_history_page"
    /// substring), meaning SQLite chose the partial covering index over a table scan.
    #[test]
    fn history_page_query_uses_index_not_full_scan() {
        let conn = Connection::open_in_memory().unwrap();
        apply_migrations(&conn).unwrap();

        // The exact SQL used by get_page_pinned_first (wall-time variant).
        // We run EXPLAIN QUERY PLAN and assert the plan contains the index name.
        let plan_rows: Vec<String> = conn
            .prepare(
                "EXPLAIN QUERY PLAN \
                 SELECT id FROM clipboard_items \
                 WHERE deleted = 0 \
                 ORDER BY \
                   CASE WHEN pinned = 1 THEN 0 ELSE 1 END ASC, \
                   pin_order IS NULL ASC, \
                   pin_order ASC, \
                   wall_time DESC \
                 LIMIT 50 OFFSET 0",
            )
            .unwrap()
            .query_map([], |r| r.get::<_, String>(3))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        let plan = plan_rows.join(" ");
        // The index must be referenced in the query plan — this is the primary
        // correctness signal. SQLite uses the index for the WHERE deleted=0 filter
        // even when the CASE expression in ORDER BY still requires a temp B-tree
        // for final sort ordering; the plan reads "SCAN ... USING INDEX ..." rather
        // than a bare "SCAN clipboard_items" (no index).
        assert!(
            plan.contains("idx_clipboard_history_page"),
            "EXPLAIN QUERY PLAN must reference idx_clipboard_history_page, \
             got plan: {plan:?} (CopyPaste-89rd)"
        );
        // Without the index SQLite emits "SCAN clipboard_items" with no "USING INDEX"
        // suffix. With the index the plan says "SCAN/SEARCH ... USING INDEX
        // idx_clipboard_history_page". Assert there is no bare unindexed scan.
        assert!(
            !plan.eq("SCAN clipboard_items"),
            "EXPLAIN QUERY PLAN must not be a bare unindexed full-table scan, \
             got plan: {plan:?} (CopyPaste-89rd)"
        );
    }

    /// CopyPaste-2lc9: regression for the WAL-replay duplicate-column race.
    ///
    /// Scenario: Connection A writes a v1 schema with `content_hash` already
    /// added to a REAL FILE database (simulating the WAL state left by a
    /// previous migration or crash) and drops WITHOUT checkpointing. Connection
    /// B opens the same file and calls `apply_migrations`.
    ///
    /// Before the fix: if the WAL was lazily applied between `column_exists`
    /// returning false and `execute_batch` running the BEGIN…COMMIT script, the
    /// ALTER TABLE would fail with "duplicate column name: content_hash".
    ///
    /// After the fix: `PRAGMA wal_checkpoint(TRUNCATE)` at the top of
    /// `apply_migrations` flushes any outstanding WAL frames into the main
    /// database file BEFORE `column_exists` runs, making the guard
    /// authoritative. Regardless of WAL state, `column_exists` always sees the
    /// complete post-checkpoint schema.
    ///
    /// Note: the true concurrent race (another writer commits `content_hash` to
    /// the WAL between `column_exists` and `execute_batch`) cannot be triggered
    /// deterministically in a single-threaded test. This test validates the
    /// file-DB code path of the guard and documents the scenario; the
    /// wal_checkpoint fix is the authoritative defence against the CI-observed
    /// intermittent failure.
    #[test]
    fn wal_replay_does_not_cause_duplicate_column() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("wal_race.db");

        // ── Connection A: write v1 schema + content_hash to a WAL-mode
        // file DB, then drop WITHOUT checkpointing. This leaves committed WAL
        // frames containing the schema on disk, visible to the next reader.
        {
            let conn_a = Connection::open(&path).expect("open conn_a");
            conn_a
                .execute_batch("PRAGMA journal_mode=WAL;")
                .expect("WAL mode");
            conn_a.execute_batch(V1_SCHEMA_SQL).expect("v1 schema");
            conn_a
                .execute_batch("ALTER TABLE clipboard_items ADD COLUMN content_hash TEXT;")
                .expect("pre-add content_hash");
            conn_a
                .execute_batch("PRAGMA user_version = 1;")
                .expect("set user_version=1");
            // conn_a is dropped here WITHOUT calling wal_checkpoint — the WAL
            // frames remain on disk exactly as a background writer would leave
            // them in the reset_database race scenario.
        }

        // ── Connection B: opens the same file. The WAL file is still present
        // (not checkpointed). `apply_migrations` must succeed:
        //   1. `PRAGMA wal_checkpoint(TRUNCATE)` at the top flushes the WAL.
        //   2. `column_exists` now sees `content_hash` → skips the ALTER.
        //   3. The migration script runs without "duplicate column name".
        {
            let conn_b = Connection::open(&path).expect("open conn_b");
            let result = apply_migrations(&conn_b);
            assert!(
                result.is_ok(),
                "apply_migrations must succeed when WAL contains pre-existing \
                 content_hash (CopyPaste-2lc9): {result:?}"
            );

            // Must reach the current schema version.
            let version: i64 = conn_b
                .query_row("PRAGMA user_version", [], |r| r.get(0))
                .unwrap();
            assert_eq!(
                version, SCHEMA_VERSION,
                "must reach SCHEMA_VERSION after migration on file DB with WAL"
            );

            // content_hash must appear exactly once — no duplicate from the race.
            let mut stmt = conn_b
                .prepare("PRAGMA table_info(clipboard_items)")
                .unwrap();
            let count = stmt
                .query_map([], |r| r.get::<_, String>(1))
                .unwrap()
                .filter_map(|r| r.ok())
                .filter(|name| name == "content_hash")
                .count();
            assert_eq!(
                count, 1,
                "content_hash must appear exactly once (no duplicate from WAL race)"
            );
        }
    }
}
