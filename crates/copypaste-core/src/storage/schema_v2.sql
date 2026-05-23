-- Schema v2 → v4 migration step.
--
-- Two unique indexes that close two production race windows. They live
-- in their own migration step (v3 → v4 — v3 already added
-- origin_device_id) so the upgrade is idempotent and atomic.

-- (1) Hash-window dedup, TOCTOU-safe.
--
-- `find_recent_by_hash` does a SELECT before INSERT, but two ingest
-- events with the same content within the same minute can both observe
-- "no recent row" and both insert. A UNIQUE INDEX over
-- (content_hash, wall_time / 60) forces SQLite to reject the second
-- INSERT with SQLITE_CONSTRAINT_UNIQUE; the application then re-queries
-- the existing row and returns its id, achieving idempotent dedup at
-- the storage layer.
--
-- The /60 bucket means "within the same wall-clock minute" — looser
-- than the application's 60_000 ms window but cheap and good enough
-- to prevent the double-insert in practice (the application still does
-- its own SELECT for the more precise window).
--
-- `WHERE content_hash IS NOT NULL` mirrors the partial index that v2
-- already created over the same column — image rows have NULL
-- content_hash and would otherwise all collide on the bucket.
CREATE UNIQUE INDEX IF NOT EXISTS idx_dedup_hash_minute
    ON clipboard_items(content_hash, (wall_time / 60))
    WHERE content_hash IS NOT NULL;

-- (2) item_id uniqueness, for sync dedup.
--
-- `item_id` is the cross-device stable id used by the sync layer. The
-- v1 schema did not enforce uniqueness, so a sync replay (peer
-- re-broadcasts the same item) could double-insert. A UNIQUE INDEX
-- closes that window at the storage layer; the sync code's own dedup
-- becomes a perf optimisation rather than a correctness requirement.
CREATE UNIQUE INDEX IF NOT EXISTS idx_clipboard_item_id
    ON clipboard_items(item_id);
