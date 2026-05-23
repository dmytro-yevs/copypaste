# Backup & Restore — CopyPaste (encrypted DB)

Operator-facing workflow for backing up and restoring the SQLCipher-encrypted
clipboard database used by `copypaste-daemon`.

> **Status:** Beta (v0.2.0). Manual workflow. A scheduled job for automated
> nightly backups is tracked separately.

---

## What gets backed up

| File / dir | Backed up? | Notes |
|---|---|---|
| `clipboard.db` | **Yes** | The SQLCipher database (clipboard history, devices, etc). |
| `db_key` | **No (by design)** | The 32-byte key file lives next to the DB. **Without it, a backup is useless.** Treat key + backup as a pair. |
| `config.toml` | No | Non-sensitive; recreate from defaults if lost. |
| `device_id` | No | Recreated by the daemon on first start. |

A backup file alone is **not** sufficient to recover. You must preserve the
`db_key` file separately (e.g. password manager, Keychain export, hardware key)
or backup the entire data dir with the key file included.

The data dir on macOS lives at:

```
~/Library/Application Support/CopyPaste/
  ├── clipboard.db
  ├── db_key
  ├── config.toml
  └── device_id
```

Override with `COPYPASTE_DATA_HOME=<path>` for tests / non-standard installs.

---

## Prerequisites

- **sqlcipher CLI** on `PATH`. The system `sqlite3` binary will **not** work —
  it cannot open SQLCipher-encrypted files.

  ```bash
  # macOS
  brew install sqlcipher

  # Debian / Ubuntu
  sudo apt-get install sqlcipher
  ```

- A working `db_key` file in the data dir.

---

## Backup

```bash
# Default: writes to ./backups/copypaste-YYYYMMDD-HHMMSS.db.enc
./scripts/backup-db.sh

# Custom output dir
./scripts/backup-db.sh --output-dir /Volumes/MyBackups/copypaste

# Caller manages daemon lifecycle (no stop, no restart)
./scripts/backup-db.sh --no-stop --no-restart

# See what would happen without touching anything
./scripts/backup-db.sh --dry-run
```

### What the script does

1. Locates the data dir (canonical `CopyPaste`, or legacy `copypaste` /
   `Copypaste` aliases).
2. Stops the daemon via `launchctl bootout`, falling back to `pkill` if the
   LaunchAgent plist isn't installed. Skipped with `--no-stop`.
3. Reads the `db_key` and runs `sqlcipher .backup` — a hot, consistent,
   re-encrypted copy. Output remains encrypted under the **same** key.
4. `chmod 600` on the output file.
5. Restarts the daemon if it was stopped (skipped with `--no-restart`).

Output filename: `copypaste-YYYYMMDD-HHMMSS.db.enc`.

### Recommended frequency

| Usage profile | Suggested cadence |
|---|---|
| Heavy daily user | Daily, retain last 7 |
| Casual user | Weekly, retain last 4 |
| Pre-upgrade / pre-migration | **Always** take a fresh backup |

The DB is small (typically <100 MB even for power users), so storage cost is
negligible. Backups are encrypted — safe to store on cloud storage **if** the
key file is stored separately.

---

## Restore

> **Stop the daemon first.** The restore script does **not** touch the daemon
> lifecycle — that's the caller's job, to keep the script composable.

```bash
# 1. Stop the daemon
launchctl bootout "gui/$(id -u)/com.copypaste.daemon" 2>/dev/null || true

# 2. Restore. Existing live DB is renamed aside with a timestamp suffix.
./scripts/restore-db.sh ./backups/copypaste-20260523-101500.db.enc

# Use --force to delete the existing live DB instead of renaming it aside.
./scripts/restore-db.sh ./backups/copypaste-20260523-101500.db.enc --force

# 3. Start the daemon back up
launchctl bootstrap "gui/$(id -u)" ~/Library/LaunchAgents/com.copypaste.daemon.plist
```

### What the script does

1. **Verifies the current `db_key` opens the backup** via a quick
   `PRAGMA key; SELECT count(*) FROM sqlite_master` smoke test. If the key
   doesn't match, restore is **aborted** — your live data is not touched.
2. Renames the existing live DB to
   `clipboard.db.before-restore-YYYYMMDD-HHMMSS` (along with any `-wal` /
   `-shm` sidecars). Use `--force` to delete instead.
3. Copies the backup into place as `clipboard.db`.
4. `chmod 600`.

### Key-mismatch failure mode

If you restore a backup taken with a different key (e.g. after a fresh install
that re-generated `db_key`), the smoke test fails with:

```
ERROR: Backup did not open with current db_key.
       The key in <data-dir>/db_key does NOT match the key used for this backup.
       Restore aborted to avoid data loss.
```

**Fix:** put the matching `db_key` file in place before re-running restore.
There is no recovery path if the original key is lost — the DB stays encrypted
forever.

---

## Operational checklist

Before any risky operation (upgrade, schema migration, manual SQL on the DB):

- [ ] Daemon stopped.
- [ ] `./scripts/backup-db.sh` runs cleanly and produces a non-empty file.
- [ ] Backup file copied off-machine (separate disk, S3, etc).
- [ ] `db_key` backed up separately (password manager, encrypted vault).
- [ ] Restore dry-run validated: `./scripts/restore-db.sh <file> --dry-run`.

---

## See also

- `scripts/backup-db.sh` — source.
- `scripts/restore-db.sh` — source.
