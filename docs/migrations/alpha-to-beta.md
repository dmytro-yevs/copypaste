# Alpha → Beta Migration

`CopyPaste v0.1.0-alpha` → `v0.2.0-beta`

This document describes what changed between the alpha and beta on-disk data
formats, and how to run the migration helper.

## TL;DR

```sh
# Dry run first (recommended).
./scripts/migrate-alpha-to-beta.sh --dry-run

# Real run (creates timestamped .bak backup).
./scripts/migrate-alpha-to-beta.sh
```

The script is **idempotent**: rerunning it is safe and produces no further
changes once the data dir is already on the beta layout.

## What changed in beta

### SQLite schema (`clipboard.db`)

| Version | Change | Reversible? |
|---------|--------|-------------|
| v1 → v2 | `ALTER TABLE clipboard_items ADD COLUMN content_hash TEXT;` | Backward-compatible (column is nullable) |
| v1 → v2 | New partial index `idx_clipboard_content_hash` for SHA-256 dedup | Index-only, no data shape change |
| v1 → v2 | `PRAGMA user_version = 2` | Bumped after both steps |

The column carries the SHA-256 of the decrypted payload and is used by the
beta daemon for fast duplicate detection (see
`crates/copypaste-core/src/storage/schema.rs`). Existing rows have
`content_hash IS NULL`; the daemon backfills lazily on next read.

The schema migration is implemented **twice** for defence in depth:
1. In Rust, `apply_migrations()` runs at every daemon startup (atomic, transactional).
2. In this script, so users can pre-migrate before launching the new binary
   (useful for staged rollouts and offline restores).

Both paths converge on `user_version = 2` and the same column/index shape.

### Data directory canonicalisation

Alpha builds sometimes wrote to a **lowercase** `~/Library/Application
Support/copypaste/` (early dev builds), while the beta consistently uses the
canonical mixed-case `~/Library/Application Support/CopyPaste/` (matches the
macOS app bundle name).

The script detects both and renames lowercase → canonical when no conflict
exists. If both directories exist, it warns and exits without merging —
users must reconcile manually.

### Config keys

Beta introduces the `copypaste-config` crate (`crates/copypaste-config/`) which
defines a unified `AppConfig`. However, on-disk config files
(`config.toml`, `device_id`, `db_key`) are **format-compatible** with alpha
and require no migration. The script merely confirms their presence and
leaves them untouched.

| File | Format | Beta behaviour |
|------|--------|----------------|
| `config.toml` | TOML | Read as-is; new keys default if absent |
| `device_id` | UUID v4 (string) | Reused verbatim |
| `db_key` | 32-byte SQLCipher key | Reused verbatim (when SQLCipher pool active) |

### Preserved across migration

- All existing clipboard history rows
- Sync state (`pending_uploads`)
- Paired devices (`devices`)
- Settings (`settings`)
- FTS index (`clipboard_fts`)
- `device_id`, `config.toml`, `db_key`

## Script flags

```
--dry-run     Print actions only, change nothing on disk. Always exits 0.
--no-backup   Skip the timestamped .bak/ snapshot (NOT recommended).
--help        Print embedded help and exit 0.
```

Environment variable `COPYPASTE_DATA_HOME` overrides the parent directory
(default: `~/Library/Application Support`). Used by tests and by users with
non-standard layouts.

## Rollback

The backup directory is named `<orig>.bak.YYYYMMDD-HHMMSS` next to the
original. To roll back:

```sh
DATA="$HOME/Library/Application Support/CopyPaste"
mv "$DATA" "$DATA.failed"
mv "$DATA".bak.<timestamp> "$DATA"
```

Then downgrade the binary. Note: the beta daemon refuses to open a database
that has `user_version > 2`, but a v2 database opened by an alpha binary
(which expects v1) will encounter the `content_hash` column as an unknown
field — alpha tolerates this only because it uses named columns in queries.
Downgrading is **not officially supported** beyond emergency rollback.

## Troubleshooting

| Symptom | Cause | Fix |
|---------|-------|-----|
| `sqlite3: command not found` | macOS missing CLI tool | `brew install sqlite` |
| `database is locked` | Daemon still running | `launchctl unload com.copypaste.daemon`, then retry |
| Script exits 0 with "nothing to migrate" | No prior alpha install | Expected — fresh install path |
| `Cannot rename: target exists` | Both `copypaste/` and `CopyPaste/` exist | Inspect both, merge manually, then delete the obsolete one |

## See also

- `crates/copypaste-core/src/storage/schema.rs` — canonical migration logic
- `crates/copypaste-core/src/storage/schema_v1.sql` — alpha schema
- `docs/adr/004-sqlite-wal.md` — WAL mode rationale
