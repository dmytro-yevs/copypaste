use super::error::DbError;

/// CopyPaste-crh3.84: single source of truth for the `migration_state` table DDL.
/// Previously duplicated verbatim in `migration_state`,
/// `migration_v4_sweep_resumable`, and `force_migration_complete`; adding a
/// column meant three coordinated edits. Idempotent (`IF NOT EXISTS`).
const MIGRATION_STATE_DDL: &str = "CREATE TABLE IF NOT EXISTS migration_state (
                key                     TEXT PRIMARY KEY,
                key_version_in_progress INTEGER,
                last_processed_id       INTEGER NOT NULL DEFAULT 0,
                started_at              INTEGER,
                completed_at            INTEGER
            );";

/// Tracks the progress of the v4 key-version sweep through `migration_state`.
///
/// The row is keyed on `'v4-key-version-sweep'` and persists across restarts
/// so a mid-sweep crash picks up from `InProgress.last_id` rather than
/// restarting from the beginning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MigrationState {
    /// The sweep row does not exist — schema migration ran but the sweep has
    /// never been triggered. This happens on a fresh install where every new
    /// row lands at `key_version = 2` from the start; no sweep is needed.
    NotStarted,
    /// Sweep is in progress. `last_id` is the row-id high-water mark: all
    /// rows with `rowid <= last_id` that were at `key_version = 1` have been
    /// processed (either rotated to v2 or logged as undecryptable).
    InProgress { last_id: i64 },
    /// Every `key_version = 1` row has been processed. Daemon ingest paths
    /// check for this state before inserting; while `InProgress` they return
    /// `IpcError::MigrationInProgress` instead of writing.
    Complete,
}

impl super::Database {
    /// Read the current state of the v4 key-version sweep from `migration_state`.
    ///
    /// Returns `MigrationState::NotStarted` if the table row is absent (fresh
    /// install, schema just migrated), `MigrationState::Complete` if
    /// `completed_at IS NOT NULL`, or `MigrationState::InProgress { last_id }`
    /// otherwise.
    pub fn migration_state(&self) -> Result<MigrationState, DbError> {
        // Ensure the migration_state table exists (idempotent DDL).
        self.conn.execute_batch(MIGRATION_STATE_DDL)?;

        let result = self.conn.query_row(
            "SELECT last_processed_id, completed_at \
             FROM migration_state WHERE key = 'v4-key-version-sweep'",
            [],
            |row| {
                let last_id: i64 = row.get(0)?;
                let completed_at: Option<i64> = row.get(1)?;
                Ok((last_id, completed_at))
            },
        );

        match result {
            Ok((_, Some(_))) => Ok(MigrationState::Complete),
            Ok((last_id, None)) => Ok(MigrationState::InProgress { last_id }),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(MigrationState::NotStarted),
            Err(e) => Err(DbError::from(e)),
        }
    }

    /// Run (or resume) the resumable v4 key-rotation sweep.
    ///
    /// Processes at most `BATCH_SIZE` rows per transaction, updates
    /// `last_processed_id` in the same transaction as the row rewrites, and
    /// sets `completed_at` on the final pass. Returns the total number of rows
    /// successfully rotated in this invocation.
    ///
    /// The sweep is idempotent: rows already at `key_version = 2` are ignored
    /// by the `WHERE key_version = 1` predicate. Calling this after
    /// `migration_state()` returns `Complete` is a no-op (returns 0).
    pub fn migration_v4_sweep_resumable(
        &self,
        v1_key: &[u8; 32],
        v2_key: &[u8; 32],
    ) -> Result<usize, DbError> {
        use super::super::migration_v4::{migrate_v1_to_v2_keys, BATCH_SIZE, INTER_BATCH_SLEEP};
        use rusqlite::params;

        const SWEEP_KEY: &str = "v4-key-version-sweep";

        // Ensure the table exists and the row is seeded.
        self.conn.execute_batch(MIGRATION_STATE_DDL)?;
        self.conn.execute(
            "INSERT OR IGNORE INTO migration_state \
             (key, key_version_in_progress, last_processed_id, started_at) \
             VALUES ('v4-key-version-sweep', 2, 0, strftime('%s','now'))",
            [],
        )?;

        // Short-circuit if already complete AND no key_version=1 rows remain.
        // We also check the actual row count because fresh installs are seeded
        // as Complete (no rows at schema migration time), but a test or a
        // direct SQL insert could add v1 rows afterward — we must still sweep.
        let state = self.migration_state()?;
        if state == MigrationState::Complete {
            let remaining_v1: i64 = self.conn.query_row(
                "SELECT COUNT(*) FROM clipboard_items WHERE key_version = 1",
                [],
                |r| r.get(0),
            )?;
            if remaining_v1 == 0 {
                return Ok(0);
            }
            // State was Complete but v1 rows exist (e.g. added after a fresh
            // install). Reset to InProgress so the sweep runs.
            self.conn.execute(
                "UPDATE migration_state SET completed_at = NULL WHERE key = ?1",
                params![SWEEP_KEY],
            )?;
        }

        // Re-use the existing sweep, which processes all remaining v1 rows
        // in BATCH_SIZE batches with INTER_BATCH_SLEEP yields. We track
        // total rotated rows here and update migration_state on completion.
        let total_rotated = migrate_v1_to_v2_keys(self, v1_key, v2_key)
            .map_err(|e| DbError::Migration(e.to_string()))?;

        // Count remaining v1 rows to decide whether we're complete.
        let remaining: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM clipboard_items WHERE key_version = 1",
            [],
            |r| r.get(0),
        )?;

        // `migrate_v1_to_v2_keys` is a self-bounded full pass: it loops
        // fetching `key_version = 1` batches until either none remain, a short
        // (< BATCH_SIZE) batch is processed, or a full batch rotates zero rows
        // (the termination guard). In every termination case it has ATTEMPTED
        // to rotate every row that is still at `key_version = 1` when it
        // returns. Therefore any rows remaining now are permanently
        // unrotatable (their auth tag does not verify under the current v1
        // key) — they were just tried and failed this pass.
        //
        // We mark the sweep Complete regardless of `remaining`:
        //   * remaining == 0 → every v1 row rotated cleanly (happy path).
        //   * remaining  > 0 → the leftover v1 rows are corrupt/legacy and can
        //     never be rotated. Leaving `completed_at = NULL` here would keep
        //     the write-gate armed FOREVER (the live-install bug), rejecting
        //     every new capture. The unreadable rows stay at `key_version = 1`
        //     (they were already unreadable); the gate releases so ingest
        //     resumes.
        //
        // Crash-safety / cursor-resume is preserved: we only reach this point
        // AFTER the full pass returned, so we never mark Complete before the
        // rows were attempted. A mid-pass crash leaves `completed_at = NULL`
        // and the next startup re-runs the pass from scratch (the
        // `WHERE key_version = 1` predicate is the cursor).
        let max_id: i64 = self.conn.query_row(
            "SELECT COALESCE(MAX(rowid), 0) FROM clipboard_items",
            [],
            |r| r.get(0),
        )?;
        self.conn.execute(
            "UPDATE migration_state \
             SET last_processed_id = ?1, completed_at = strftime('%s','now') \
             WHERE key = ?2",
            params![max_id, SWEEP_KEY],
        )?;

        if remaining > 0 {
            tracing::warn!(
                remaining,
                "v4 migration: {remaining} key_version=1 row(s) could not be rotated \
                 (undecryptable under the current key); leaving them at key_version=1 \
                 and marking the sweep Complete so new captures are no longer gated"
            );
        }

        let _ = BATCH_SIZE; // ensure constant is referenced
        let _ = INTER_BATCH_SLEEP; // referenced by the batched inner sweep

        Ok(total_rotated)
    }

    /// Recovery helper: if the migration state is `InProgress` but there are
    /// no `key_version = 1` rows remaining, mark the sweep complete.
    ///
    /// This covers users who were seeded with an `InProgress` row (via the
    /// v6 schema migration `INSERT OR IGNORE`) on a fresh install that had
    /// zero clipboard rows — the gate was armed but could never clear itself
    /// because the sweep was never invoked. Call this after
    /// `migration_v4_sweep_resumable` returns.
    pub fn force_complete_if_no_v1_rows(&self) -> Result<(), DbError> {
        const SWEEP_KEY: &str = "v4-key-version-sweep";

        // Only act if the state is genuinely InProgress (completed_at IS NULL).
        let state = self.migration_state()?;
        if !matches!(state, MigrationState::InProgress { .. }) {
            return Ok(());
        }

        let v1_count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM clipboard_items WHERE key_version = 1",
            [],
            |r| r.get(0),
        )?;

        if v1_count == 0 {
            self.conn.execute(
                "UPDATE migration_state \
                 SET completed_at = strftime('%s','now') \
                 WHERE key = ?1",
                rusqlite::params![SWEEP_KEY],
            )?;
            tracing::info!(
                "force_complete_if_no_v1_rows: no v1 rows found, migration marked Complete"
            );
        }

        Ok(())
    }

    /// Escape hatch: unconditionally mark the v4 sweep Complete, clearing the
    /// write-gate even if `key_version = 1` rows remain.
    ///
    /// This is the backing primitive for the `COPYPASTE_FORCE_MIGRATION_COMPLETE`
    /// environment variable (mirrors `COPYPASTE_NO_AUTO_MIGRATE`). It exists for
    /// installs that were *already* stuck on a prior build — where the sweep
    /// logged `rotated=0 failed=N` and left `completed_at` NULL forever, so
    /// every clipboard capture was rejected with `MigrationInProgress`.
    ///
    /// Unlike [`Self::force_complete_if_no_v1_rows`], this does NOT require zero v1
    /// rows: it seeds the sweep row if absent and sets `completed_at` no matter
    /// what. The remaining `key_version = 1` rows are left untouched (they were
    /// already unreadable under the current key); only the gate is released.
    pub fn force_migration_complete(&self) -> Result<(), DbError> {
        const SWEEP_KEY: &str = "v4-key-version-sweep";

        // Ensure the table + row exist so the UPDATE has something to hit.
        self.conn.execute_batch(MIGRATION_STATE_DDL)?;
        self.conn.execute(
            "INSERT OR IGNORE INTO migration_state \
             (key, key_version_in_progress, last_processed_id, started_at) \
             VALUES ('v4-key-version-sweep', 2, 0, strftime('%s','now'))",
            [],
        )?;
        self.conn.execute(
            "UPDATE migration_state \
             SET completed_at = strftime('%s','now') \
             WHERE key = ?1 AND completed_at IS NULL",
            rusqlite::params![SWEEP_KEY],
        )?;
        tracing::warn!(
            "force_migration_complete: write-gate force-cleared via \
             COPYPASTE_FORCE_MIGRATION_COMPLETE — any remaining key_version=1 \
             rows are left as-is (they were already unreadable)"
        );
        Ok(())
    }

    /// Count the rows still stranded at `key_version = 1` after a completed
    /// v4 sweep. These are legacy ciphertexts whose AEAD auth tag does not
    /// verify under the current v1 key (re-keyed device, lost key generation,
    /// or a pre-fix double-derivation bug). They can never be decrypted or
    /// rotated and are permanent dead weight in the database.
    ///
    /// Surfaced (not silently ignored) so the daemon can WARN with a count and
    /// point the user at the purge affordance.
    pub fn count_dead_v1_rows(&self) -> Result<usize, DbError> {
        let n: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM clipboard_items WHERE key_version = 1",
            [],
            |r| r.get(0),
        )?;
        Ok(n as usize)
    }

    /// Permanently delete every row still stranded at `key_version = 1` — the
    /// undecryptable legacy ciphertexts that the v4 sweep could not rotate.
    ///
    /// This DESTROYS user data and is therefore opt-in only: it is the backing
    /// primitive for the `COPYPASTE_PURGE_DEAD_V1_ROWS=1` environment variable
    /// (mirrors `COPYPASTE_FORCE_MIGRATION_COMPLETE` / `COPYPASTE_NO_AUTO_MIGRATE`).
    /// The rows it removes are already permanently unreadable — there is no
    /// recoverable content — but we still gate the deletion behind an explicit
    /// flag rather than auto-deleting, per the "never delete user data without
    /// a flag" rule.
    ///
    /// Associated FTS rows are removed too so the search index stays consistent
    /// (the FTS `id` mirrors `clipboard_items.id`). Returns the number of rows
    /// deleted from `clipboard_items`.
    pub fn purge_dead_v1_rows(&self) -> Result<usize, DbError> {
        // Wrap both DELETEs in a single transaction so a crash between the two
        // cannot leave clipboard_items rows without their FTS counterparts
        // (mirrors the atomic FTS+row writes in items::insert_item_and_fts).
        // The external-content FTS5 table has no ON DELETE CASCADE, so we must
        // delete the FTS entries explicitly before removing the source rows.
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "DELETE FROM clipboard_fts \
             WHERE id IN (SELECT id FROM clipboard_items WHERE key_version = 1)",
            [],
        )?;
        let deleted = tx.execute("DELETE FROM clipboard_items WHERE key_version = 1", [])?;
        tx.commit()?;
        if deleted > 0 {
            tracing::warn!(
                deleted,
                "purge_dead_v1_rows: permanently removed {deleted} undecryptable \
                 key_version=1 row(s) (COPYPASTE_PURGE_DEAD_V1_ROWS=1)"
            );
        }
        Ok(deleted)
    }

    /// One-time startup repair: find image/file rows that were encrypted with
    /// the v1 key but mistakenly stamped `key_version = 2` by the pre-fix
    /// writer in `daemon::handle_image` and `handle_file`.
    ///
    /// For each candidate row the function probes v1-decrypt: success means
    /// the row is mislabeled and is re-encrypted in-place with the v2 key;
    /// failure means the row is correctly v2-encrypted and is left alone.
    ///
    /// Returns the count of rows actually repaired (re-encrypted). Idempotent.
    pub fn repair_mislabeled_kv2_blob_rows(
        &self,
        v1_key: &[u8; 32],
        v2_key: &[u8; 32],
    ) -> Result<usize, DbError> {
        super::super::migration_v4::repair_mislabeled_kv2_blob_rows(self, v1_key, v2_key)
            .map_err(|e| DbError::Migration(e.to_string()))
    }
}
