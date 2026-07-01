// size-exempt: version-ordered migration data table (ADR-017)
//
// This file holds the 15 versioned SQL migration consts (`Vn_*`) referenced
// by `build_migration_script` in `super::mod`. Each const is a self-contained,
// dated migration step; the ladder that decides which ones to apply lives in
// `mod.rs::build_migration_script` and MUST NOT be reordered or split here.
// See `mod.rs` module doc for the full version-bump rationale narrative.

/// Baseline (v1) schema as a single SQL script. Made `pub(crate)` so the
/// crate-internal `db` and `schema` tests can stage a legacy plaintext DB
/// without duplicating the SQL. Integration tests still inline a copy because
/// `include_str!` paths are crate-relative and not visible from `tests/`.
pub(crate) const V1_SCHEMA_SQL: &str = include_str!("../schema_v1.sql");

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
pub(crate) const V5_INDEXES_SQL: &str = include_str!("../schema_v2.sql");

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
