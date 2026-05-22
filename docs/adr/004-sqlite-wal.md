# ADR-004: SQLite WAL Mode

**Date:** 2026-05-22  
**Status:** Accepted

## Context

SQLite supports two journaling modes: rollback journal (default) and Write-Ahead Logging (WAL).

## Decision

Use WAL mode for all CopyPaste databases (clipboard + relay).

## Rationale

1. **Concurrent reads** — WAL allows readers and one writer to operate simultaneously. Critical for daemon writing while IPC server reads.
2. **Better write performance** — WAL appends to the log file sequentially; much faster than rollback journal which requires fsync on the main file.
3. **Crash safety** — WAL provides the same ACID guarantees as rollback journal.
4. **FTS5 compatibility** — SQLite FTS5 requires WAL mode or shared-cache for concurrent access.

## Consequences

- Two extra files per database: `.db-shm` and `.db-wal`
- Checkpointing must happen periodically (SQLite does this automatically at 1000 pages by default)
- Not portable to platforms without WAL support (embedded systems with read-only filesystem) — not a concern for macOS/Windows/Android
