# Port Manifest 03 — Storage Subsystem (SQLCipher / SQLite)

> STATUS: COMPLETE. Harvested from `copypaste-core` v0.4.1, on-disk schema
> `user_version = 15`.
>
> Source of truth for the old implementation:
> `crates/copypaste-core/src/storage/**` plus
> `crates/copypaste-core/tests/{migration,dedup,fts5_search,encryption_at_rest,corruption,key_version_tests,pool_stress,concurrent_writers}.rs`.
> All `file:line` citations are repo-relative and refer to the **old** tree at
> v0.4.1 — they are provenance, not instructions to copy.
>
> Related ADRs: ADR-003 (SQLCipher at rest), ADR-004 (WAL), ADR-015 (FTS
> sensitive exclusion), ADR-017 (file-size budget — `schema/versions.rs` is
> `// size-exempt` under it).
>
> **Read §2 and §4 before writing a single line of the new storage layer.**
> Everything else is detail; those two are the contract with existing user data.

---

## 1. Purpose & scope

The storage subsystem is the **only** durable store of clipboard history on a
device. It owns:

* a single SQLCipher-encrypted SQLite file (the "clipboard DB"),
* the versioned schema and its forward-only migration ladder (`user_version` 1→15),
* per-connection PRAGMA policy (WAL, busy timeout, cache, temp store),
* SQLCipher keying, plaintext→encrypted auto-migration, and crash-safe rekey,
* the `clipboard_items` row model + CRUD, dedup, soft-delete/tombstones,
  pinning + reorder, TTL/expiry purge, byte-cap eviction,
* the FTS5 search index and its **sensitive-exclusion policy** (ADR-015),
* keyset ("seek") and offset pagination contracts consumed by IPC/UI/FFI,
* an r2d2 read pool for concurrent WAL readers,
* the paired-device (`devices`) and revocation-audit (`revoked_devices`) tables,
* resumable-upload bookkeeping (`pending_uploads`) and app settings (`settings`).

Out of scope for this manifest (covered elsewhere): the AEAD content
encryption itself (see `02-crypto.md`), the LWW merge algorithm (sync crate),
the IPC verbs (see `04-ipc-protocol.md`).

**The rewrite MUST be able to open, migrate, and keep serving an existing
user database written by v0.4.1 (schema v15) and by every shipped predecessor
(v1..v14).** That constraint dominates every design decision below.

### File layout on disk

| File | Notes |
|---|---|
| `<name>.db` | main SQLCipher database |
| `<name>.db-wal` | WAL, always present (journal_mode=WAL) |
| `<name>.db-shm` | WAL index |
| `<name>.db.tmp` | transient — plaintext→encrypted migration staging (`db/keying.rs:72`) |
| `<name>.db.rekey-tmp` | transient — rekey staging (`db/mod.rs:325`, via `path.with_extension("db.rekey-tmp")`) |

Both `.tmp` files are removed before use (`let _ = fs::remove_file(&tmp_path)`)
so a crashed previous attempt cannot poison the next one.

---

## 2. Invariants (MUST hold)

**I1 — Openability.** A database file at any `user_version` in `1..=15` MUST
open and be migrated forward to the current version in a single atomic
transaction. Never require the user to "start fresh".

**I2 — No silent downgrade.** If `PRAGMA user_version > SCHEMA_VERSION`, the
open MUST fail with a typed downgrade error and MUST NOT touch the file.
(`schema/mod.rs`: `SchemaError::Downgrade`; the pre-fix code fell through to
`Ok(())` and silently masked the mismatch — "CRITICAL edge-case #2".)

**I3 — Migration atomicity.** All migration steps for a single open run inside
one `BEGIN … COMMIT`, with `PRAGMA user_version = N` set as the last statement
before `COMMIT`. Any failure rolls back and leaves `user_version` untouched.

**I4 — Migration idempotency.** Every `ALTER TABLE … ADD COLUMN` is guarded by
a `pragma_table_info` existence probe; every `CREATE TABLE`/`CREATE INDEX` uses
`IF NOT EXISTS`; every data-fix step (v13 purge, v8 backfill) is a no-op when
re-run. Rationale: WAL-replay onto a freshly recreated `.db` file can leave
`user_version = 0` while later-version columns already exist.

**I5 — SQLCipher key format is load-bearing.** `PRAGMA key = "x'<64 lowercase
hex chars>'"` MUST be the **first** statement on every connection, before any
other pragma or query. A different key encoding (passphrase form, different
hex case handling, 96-hex key+salt form) will not open existing files.

**I6 — No `PRAGMA rekey` on disk-backed databases.** Rekey MUST use the
ATTACH + `sqlcipher_export` + fsync + atomic-rename procedure (§3.6).

**I7 — Sensitive items are never in FTS.** `is_sensitive = 1` rows MUST NOT
have a `clipboard_fts` row, and MUST NOT be returned by search even if a stale
row exists. Three enforcement layers required (ADR-015). See §3.5.

**I8 — Sensitive items never carry a thumbnail.** Both insert paths and the
`set_thumb` backfill suppress `thumb` when `is_sensitive = 1`
(`CopyPaste-44rq.49`).

**I9 — Pinned items are never auto-deleted.** Every TTL sweep, every
history-limit prune, and the byte-cap eviction filter `pinned = 0`.

**I10 — FTS and rows never drift.** Every path that removes or tombstones a
`clipboard_items` row deletes the matching `clipboard_fts` row **in the same
transaction**. Every path that inserts a row + FTS entry does both in one
transaction.

**I11 — `pending_uploads` is cleaned before the row it references vanishes.**
There is no FK/cascade; the cleanup DELETE resolves `item_id` through
`clipboard_items`, so it MUST run *before* the row delete, inside the same
transaction (`CopyPaste-6fd`).

**I12 — `item_id` is the cross-device identity; `id` is the local row PK.**
They are different UUIDs. Sync/merge/dedup key on `item_id`. `item_id` is bound
into the AEAD AAD, so it MUST NEVER be regenerated when reconstructing a known
item.

**I13 — Tombstones are visible to the merge layer.** `get_item_by_item_id`
deliberately does **not** filter `deleted = 0`; every UI/list/search/count
query does.

**I14 — Pool refuses an unmigrated file.** Opening the read pool on a file with
`user_version = 0` MUST fail with a descriptive `SchemaNotInitialized` error,
not a later "no such table".

**I15 — `key_version` ∈ {1, 2} on write.** Anything else is rejected at write
time (`UnsupportedKeyVersion`); a stored value that does not fit `u8` is
surfaced as `CorruptKeyVersion`, never truncated.

---

## 3. Exact schema, indexes, pragmas

### 3.1 Final schema (user_version = 15)

Reconstructed final state after the full ladder. `clipboard_items` is created
by `schema_v1.sql` and then extended by ALTERs; SQLite preserves column order
as `v1 columns … then v2, v3, v4, v7, v8, v9, v10 additions in ladder order`.

```sql
CREATE TABLE clipboard_items (
    id                TEXT PRIMARY KEY NOT NULL,  -- local row PK, UUIDv4 (RowId)
    item_id           TEXT NOT NULL,              -- cross-device identity, UUIDv4 (ItemId); UNIQUE via index
    content_type      TEXT NOT NULL,              -- 'text' | 'image' | 'file'
    content           BLOB,                       -- AEAD ciphertext (text) or chunk blob (image/file); NULL for tombstones
    content_nonce     BLOB,                       -- 24-byte XChaCha20 nonce for text; NULL for image/file (per-chunk nonces) and tombstones
    blob_ref          TEXT,                       -- image/file metadata JSON (width/height/chunk_count/file_id/filename/mime/original_size)
    is_sensitive      INTEGER NOT NULL DEFAULT 0, -- 0/1 boolean
    is_synced         INTEGER NOT NULL DEFAULT 0, -- 0/1 boolean
    lamport_ts        INTEGER NOT NULL,           -- unified LWW clock: max(prev+1, now_ms)
    wall_time         INTEGER NOT NULL,           -- ms since Unix epoch
    expires_at        INTEGER,                    -- ms since Unix epoch; NULL = no TTL
    app_bundle_id     TEXT,                       -- source app identifier, nullable
    -- v2
    content_hash      TEXT,                       -- SHA-256 hex (64 chars) of raw pre-encryption bytes; NULL for image rows
    -- v3
    origin_device_id  TEXT NOT NULL DEFAULT '',   -- device UUID; '' until backfilled
    -- v4
    key_version       INTEGER NOT NULL DEFAULT 1, -- 1 = HKDF-SHA256 legacy, 2 = HKDF-SHA512 current
    -- v7
    pinned            INTEGER NOT NULL DEFAULT 0, -- 0/1
    -- v8
    pin_order         REAL DEFAULT NULL,          -- sort key among pinned rows; NULL when unpinned
    -- v9
    thumb             BLOB DEFAULT NULL,          -- encrypted thumbnail chunk blob (image rows only, never sensitive)
    -- v10
    deleted           INTEGER NOT NULL DEFAULT 0  -- 0 = live, 1 = tombstone
);
```

**The 19-column canonical order** (used by both INSERT and SELECT, positionally
mapped in `items/types.rs:row_to_item`):

```
id, item_id, content_type, content, content_nonce, blob_ref,
is_sensitive, is_synced, lamport_ts, wall_time, expires_at, app_bundle_id,
content_hash, origin_device_id, key_version, pinned, pin_order, thumb, deleted
```

Defined once each in `items/insert.rs:11` (`ITEM_INSERT_COLUMNS`),
`items/query.rs:12` (`ITEM_SELECT_COLUMNS`) and `items/query.rs:20`
(`ITEM_SELECT_COLUMNS_CI`, the `ci.`-aliased variant for the FTS JOIN).

```sql
CREATE VIRTUAL TABLE clipboard_fts USING fts5(id UNINDEXED, content_text);

CREATE TABLE devices (
    id          TEXT PRIMARY KEY NOT NULL,  -- device UUID
    name        TEXT NOT NULL,
    platform    TEXT NOT NULL,
    public_key  TEXT NOT NULL,
    fingerprint TEXT NOT NULL,
    verified    INTEGER NOT NULL DEFAULT 0,
    last_seen   INTEGER
);

CREATE TABLE settings (
    key   TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
);

CREATE TABLE pending_uploads (
    item_id              TEXT PRIMARY KEY NOT NULL,  -- cross-device item_id, NOT the row id; no FK
    tus_url              TEXT NOT NULL,
    bytes_uploaded       INTEGER NOT NULL DEFAULT 0,
    total_bytes          INTEGER NOT NULL,
    chunk_format_version INTEGER NOT NULL DEFAULT 1,
    created_at           INTEGER NOT NULL,
    expires_at           INTEGER NOT NULL
);

-- v6
CREATE TABLE migration_state (
    key                     TEXT PRIMARY KEY,   -- only value in use: 'v4-key-version-sweep'
    key_version_in_progress INTEGER,            -- always seeded 2; never read (see §6)
    last_processed_id       INTEGER NOT NULL DEFAULT 0,  -- written, never used to resume (see §6)
    started_at              INTEGER,            -- unix SECONDS (strftime('%s','now'))
    completed_at            INTEGER             -- unix SECONDS; NULL = sweep in progress
);

-- v12
CREATE TABLE revoked_devices (
    fingerprint TEXT PRIMARY KEY NOT NULL,      -- colon-separated hex fingerprint
    name        TEXT NOT NULL DEFAULT '',
    revoked_at  INTEGER NOT NULL                -- unix SECONDS
);
```

> **Unit trap to preserve:** `clipboard_items.wall_time` / `expires_at` /
> `lamport_ts` are **milliseconds**. `migration_state.started_at` /
> `completed_at` and `revoked_devices.revoked_at` are **seconds**. The dedup
> minute bucket is `wall_time / 60` — i.e. it buckets *milliseconds* by 60, so
> a "minute bucket" is really a 60 ms bucket. This is a known wart documented
> in `schema_v2.sql`; **the on-disk index expression must be reproduced
> verbatim** or existing rows change bucket and dedup behaviour shifts.

### 3.2 Indexes — exact DDL and the query each serves

| Index | DDL | Added | Serves |
|---|---|---|---|
| `idx_clipboard_wall_time` | `ON clipboard_items(wall_time DESC)` | v1 | `get_page`, `get_page_meta`, `get_page_seek`, `get_page_meta_seek` (`ORDER BY wall_time DESC`) |
| `idx_clipboard_expires` | `ON clipboard_items(expires_at) WHERE expires_at IS NOT NULL` | v1 | `delete_expired` / `delete_sensitive_expired` (`WHERE expires_at IS NOT NULL AND expires_at < ?`) |
| `idx_clipboard_content_hash` | `ON clipboard_items(content_hash) WHERE content_hash IS NOT NULL` | v2 | `find_recent_by_hash` |
| `idx_clipboard_key_version` | `ON clipboard_items(key_version) WHERE key_version < 2` | v4 | v4 sweep `WHERE key_version = 1`, `count_dead_v1_rows`, `purge_dead_v1_rows` |
| `idx_dedup_hash_minute` | `UNIQUE ON clipboard_items(content_hash, (wall_time / 60)) WHERE content_hash IS NOT NULL AND deleted = 0` | v5, **rebuilt in v15** | TOCTOU-safe dedup (§3.7) |
| `idx_clipboard_item_id` | `UNIQUE ON clipboard_items(item_id)` | v5 | sync-replay dedup; `get_item_by_item_id`, `exists_item_by_item_id` |
| `idx_clipboard_pinned` | `ON clipboard_items(pinned) WHERE pinned = 1` | v7 | `pin_item`'s `MAX(pin_order) … WHERE pinned = 1` subquery |
| `idx_clipboard_deleted` | `ON clipboard_items(deleted) WHERE deleted = 1` | v10 | tombstone enumeration for sync catch-up (tombstone minority) |
| `idx_clipboard_unpinned_len` | `ON clipboard_items(LENGTH(COALESCE(content, ''))) WHERE pinned = 0` | v11 | `prune_to_cap`'s per-write size gate: `SELECT COALESCE(SUM(LENGTH(COALESCE(content,''))),0) FROM clipboard_items WHERE pinned = 0` → **index-only scan** |
| `idx_clipboard_history_page` | `ON clipboard_items(pinned DESC, pin_order, wall_time DESC) WHERE deleted = 0` | v14 | `get_page_pinned_first` / `_lamport` / `_seek` — the `history_page` IPC hot path |
| `idx_revoked_devices_revoked_at` | `ON revoked_devices(revoked_at DESC)` | v12 | revocation audit listing |

Partial-index predicates are **not decorative**:

* `idx_clipboard_unpinned_len` is partial on `WHERE pinned = 0` precisely
  because `idx_clipboard_pinned` is partial on `WHERE pinned = 1` and therefore
  useless for the inverted predicate. `prune_to_cap`'s gate SUM deliberately
  omits `AND deleted = 0` so its `WHERE` matches the index verbatim
  (`CopyPaste-crh3.3`): tombstones have `content = NULL` so they contribute 0.
* `idx_clipboard_history_page` is partial on `WHERE deleted = 0` (the live-row
  majority). `idx_clipboard_deleted` (partial on `deleted = 1`) cannot serve the
  live path, and `idx_clipboard_wall_time` cannot be used when a `CASE`
  expression leads the `ORDER BY`. Without v14 every `history_page` call is a
  full scan + filesort (`CopyPaste-89rd`).

### 3.3 PRAGMAs — verbatim

**Applied before `BEGIN` inside `apply_migrations` (`schema/mod.rs`):**

```sql
PRAGMA journal_mode=WAL;
PRAGMA wal_checkpoint(TRUNCATE);       -- NON-FATAL: log-and-continue on error
PRAGMA cache_size=-<SQLITE_CACHE_MB * 1024>;   -- default: -8192
PRAGMA auto_vacuum = INCREMENTAL;
```

* `journal_mode` must be outside a transaction (no-op inside one).
* `wal_checkpoint(TRUNCATE)` is a **defensive belt only** (`CopyPaste-2lc9`):
  it forces a stale WAL to be applied before `pragma_table_info` /
  `PRAGMA user_version` are read. It is deliberately non-fatal because
  `busy_timeout` does **not** cover `SQLITE_PROTOCOL` ("locking protocol"),
  which was observed under CPU-starved coverage runners. The `column_exists`
  guard remains the authoritative WAL-replay backstop.
* `auto_vacuum = INCREMENTAL` only takes effect on a **fresh empty** database.
  Pre-existing DBs keep `auto_vacuum = NONE` and `PRAGMA incremental_vacuum(n)`
  is a silent no-op there (`CopyPaste-kexs`).

**Per-connection set, `db/pragmas.rs:42` `CONNECTION_PRAGMAS` — verbatim:**

```sql
PRAGMA busy_timeout = 5000;
PRAGMA synchronous = NORMAL;
PRAGMA foreign_keys = ON;
PRAGMA temp_store = MEMORY;
PRAGMA wal_autocheckpoint = 1000;
PRAGMA journal_size_limit = 67108864;
```

plus, appended by `connection_pragmas(cache_mb)`:

```sql
PRAGMA cache_size = -<clamp(cache_mb, 1, 256) * 1024>;
```

Rationale to carry over:

* `busy_timeout = 5000` — without it the UI reader and daemon writer race
  instantly and surface silent `SQLITE_BUSY`.
* `foreign_keys = ON` — connection-scoped, not persisted. NOTE (`CopyPaste-6fd`):
  the schema declares **no** `ON DELETE CASCADE` FKs; `pending_uploads.item_id`
  is a bare PK. Do not rely on this pragma for `pending_uploads` cleanup.
* `temp_store = MEMORY` — keeps temp B-trees (which contain decrypted
  intermediates) off the filesystem.
* `wal_autocheckpoint = 1000` — set explicitly (equal to the SQLite default) so
  pool and single-connection paths provably agree (`CopyPaste-ayg`); bounds WAL
  growth during the v4 sweep.
* `journal_size_limit = 67108864` (64 MiB) — caps WAL file size after a
  checkpoint that cannot shrink it immediately.

**Ordering contract:** `PRAGMA key` first → key validation read
(`SELECT COUNT(*) FROM sqlite_master`) → `CONNECTION_PRAGMAS` + cache →
`apply_migrations` → **re-assert** `cache_size` (because `apply_migrations`
resets it to the compiled-in default).

**Pool path (`pool.rs:154`) builds the batch as:**

```
PRAGMA key = "x'<hex>'";
PRAGMA journal_mode = WAL;
<CONNECTION_PRAGMAS>
<cache_size_pragma, trailing newline trimmed>
```

**Cache tunables:** `SQLITE_CACHE_MB = 8`, `SQLITE_CACHE_MB_MIN = 1`,
`SQLITE_CACHE_MB_MAX = 256` (`config/defaults.rs:62,66,67`). `cache_size` is
expressed as a negative value = KiB budget.

### 3.4 SQLCipher keying

**Key format (verbatim, `db/pragmas.rs:7-16`):**

```rust
PRAGMA key = "x'<64 lowercase hex chars>'"
```

built by `write!(hex, "{:02x}", b)` over the 32-byte key, wrapped in
`zeroize::Zeroizing<String>` so the hex and the full statement are scrubbed on
drop. The same encoding is used by `pool::key_hex` and by the `ATTACH … KEY`
statements in `encrypt_existing` / `rekey`.

**KDF parameters:** none are set explicitly. Because the key is supplied in
**raw-hex form** (`x'…'`, exactly 64 hex chars = 32 bytes, no appended salt),
SQLCipher bypasses PBKDF2 for the page key entirely and uses the 16-byte random
salt stored in the database header. All other cipher settings are the
**SQLCipher 4 defaults** shipped by `rusqlite 0.32.1` +
`libsqlite3-sys 0.30.1` with feature `bundled-sqlcipher`
(AES-256-CBC, HMAC-SHA512, 4096-byte pages, `PBKDF2_HMAC_SHA512` for the HMAC
subkey). **The rewrite must not set `cipher_page_size`, `kdf_iter`,
`cipher_hmac_algorithm`, `cipher_kdf_algorithm`, or
`cipher_compatibility` — any of those would change the derived page key /
layout and existing files would fail to open.**

Dependencies pinned at v0.4.1: `rusqlite = "0.32"` with
`features = ["bundled-sqlcipher", "backup"]`, `r2d2_sqlite = "0.25"` with
`default-features = false, features = ["bundled-sqlcipher"]`
(`Cargo.toml:55-57`).

**Open flow (`db/mod.rs:57-161`):**

1. `Connection::open_with_flags(path, READ_WRITE | CREATE)`.
2. `PRAGMA key = "x'…'"` — **first statement, always**.
3. Validate: `SELECT COUNT(*) FROM sqlite_master`.
   * `Ok` → apply `CONNECTION_PRAGMAS`, run migrations, re-assert cache.
   * `Err` with `extended_code == SQLITE_NOTADB` **or**
     `code == DatabaseCorrupt` → ambiguous: wrong key *or* plaintext file.
     Disambiguate by re-opening **without** a key and probing
     `SELECT COUNT(*) FROM sqlite_master`:
     * probe succeeds → confirmed plaintext → log a WARN with path + size,
       run `encrypt_existing`, re-open encrypted, migrate.
     * probe fails → wrong key → propagate the original error.
4. `Database::open_no_auto_migrate*` performs the same probe but returns
   `DbError::PlaintextMigrationBlocked { path, size }` instead of migrating.
   Driven by env var `COPYPASTE_NO_AUTO_MIGRATE=1`.

### 3.5 Plaintext→encrypted migration (`encrypt_existing`, `db/keying.rs:69`)

```
remove stale <path>.db.tmp
open plaintext source (no key)
ATTACH DATABASE '<path>.db.tmp' AS encrypted KEY "x'<hex>'"
SELECT sqlcipher_export('encrypted')
DETACH DATABASE encrypted
drop connection
fsync(tmp)                      -- MANDATORY: without it a power-cut can leave a
                                --   zero-length destination after rename
rename(tmp, path)               -- atomic
fsync(parent dir)               -- best-effort; POSIX requires it for a durable rename.
                                --   Windows / some FUSE return EISDIR/EACCES/EINVAL → ignore
```

### 3.6 Rekey (`Database::rekey`, `db/mod.rs:287-421`)

**`PRAGMA rekey` is rejected for disk-backed databases.** It rewrites pages
in place; an interruption (power cut, panic, SIGKILL) leaves a file with a mix
of old-key and new-key pages — *neither* key opens it and there is no automatic
recovery.

The crash-safe procedure (consumes `self` so a half-rekeyed handle cannot be
reused):

1. `keying::checkpoint_with_retry(&conn)` — `PRAGMA wal_checkpoint(TRUNCATE)`,
   up to **3 attempts**, **100 ms** backoff. Result row is
   `(busy, log_pages, checkpointed_pages)`; only `busy == 0` counts as success.
   `QueryReturnedNoRows` ⇒ WAL not active ⇒ success. Failure is a **hard**
   `DbError::CheckpointFailed` — unlike the migration belt, here an unmerged WAL
   would make `sqlcipher_export` see only the main-file half.
2. `ATTACH DATABASE '<path>.db.rekey-tmp' AS rekeyed KEY "x'<newhex>'"`
   (stale tmp removed first).
3. `SELECT sqlcipher_export('rekeyed')`.
4. **Carry `user_version` across explicitly**:
   `PRAGMA rekeyed.user_version = <src>;`. `sqlcipher_export` copies tables,
   indexes and triggers but **not** pragmas; without this the re-open sees v0
   and re-runs every ALTER → "duplicate column".
5. `DETACH DATABASE rekeyed`; `drop(self)` to release the file (required on
   Windows).
6. `fsync(tmp)` → `rename(tmp, path)` → `fsync(parent dir)`.
7. Re-open under the new key: `PRAGMA key` → validate with
   `SELECT COUNT(*) FROM sqlite_master` → `CONNECTION_PRAGMAS` →
   `apply_migrations` → re-assert cache. Every error at this stage embeds the
   path so callers can report a location.

**In-memory fallback:** `path == None` (`:memory:`) uses
`PRAGMA rekey = "x'<hex>'"` — acceptable because a crash loses the whole
volatile DB anyway.

**Crash-safety property:** at every instant a power cut leaves *either* the old
file (old key opens it) *or* the new file (new key opens it) — never a
half-rekeyed file.

### 3.7 FTS5

```sql
CREATE VIRTUAL TABLE clipboard_fts USING fts5(id UNINDEXED, content_text);
```

* `id` is `UNINDEXED` and mirrors `clipboard_items.id` (the **row PK**, not
  `item_id`). It is a plain text column carried for the JOIN.
* `content_text` holds the **decrypted plaintext**. Only text items are indexed;
  image/file items pass `""` and are skipped.
* External-content mode is NOT used and there is no `ON DELETE CASCADE`, so
  every delete path must explicitly delete from `clipboard_fts`.
* FTS5 has no `ON CONFLICT`; the upsert idiom is `DELETE` + `INSERT` wrapped in
  one transaction (`CopyPaste-j9pv`) so a crash cannot leave an item
  permanently unsearchable.

**Sensitive exclusion — ADR-015 (`CopyPaste-i6pp`). Three layers, all required:**

1. **Write guard** — `insert_item_with_fts` writes FTS only when
   `!plaintext_for_fts.is_empty() && !item.is_sensitive`. Unconditional: it
   ignores what the caller passed.
2. **Upsert guard** — `upsert_fts` re-reads `is_sensitive` **inside the same
   transaction** as the FTS write (`CopyPaste-44rq.64`) so a concurrent UPDATE
   flipping `is_sensitive = 1` cannot slip plaintext in. `Some(1)` → return
   `Ok(())` (tx rolls back on drop); `None` (row missing) → `Ok(())`.
3. **Query filter** — `search_items_filtered` adds `AND ci.is_sensitive = 0`
   (and `AND ci.deleted = 0`) to the JOIN so a stale row from a pre-fix DB or
   from test tooling can never surface.

Plus: **`mark_sensitive(db, id)`** (`CopyPaste-44rq.45`) is the transition path
— `UPDATE … SET is_sensitive = 1` **and** `DELETE FROM clipboard_fts` in one
transaction. The FTS delete always runs even if `is_sensitive` was already 1,
to repair a stale row from an earlier partial failure.

Plus: migration **v13** purges pre-existing leaked rows (§4).

**Search SQL (no type filter):**

```sql
SELECT <ITEM_SELECT_COLUMNS_CI>
  FROM clipboard_fts fts
  JOIN clipboard_items ci ON ci.id = fts.id
 WHERE clipboard_fts MATCH ?1 AND ci.deleted = 0 AND ci.is_sensitive = 0
 ORDER BY rank
 LIMIT ?2
```

With a type filter, an `AND ci.content_type = ?3` clause is added
(`CopyPaste-tteo`). Two **static** SQL strings are used rather than dynamic
construction, to stay injection-safe and to keep the `prepare_cached` cache key
stable per branch.

**Query sanitizer (`sanitize_fts5_query`, `items/fts.rs:230`)** — whitelist
tokenizer, S8:

* Rewrite `-` → space **first** (`-` is the FTS5 NOT/column operator; a
  hyphenated token makes FTS5 parse `-bar` as a column filter and error with
  "no such column: bar"). So `foo-bar` becomes `foo* AND bar*`.
* Keep only `char::is_alphanumeric()` (Unicode — covers Cyrillic/CJK), `_`,
  `"`, `*`, space, tab. Everything else (`:`, `^`, `;`, `'`, `\`, NUL, …) is
  stripped.
* Trim; empty → `None` (caller returns no results).
* Count `"`; if **odd**, strip all quotes (an unclosed phrase is an FTS5 syntax
  error). Re-trim; empty → `None`.
* If the result is a fully-quoted phrase (`"…"`, len > 1) **or** already ends
  with `*`, pass through unchanged.
* Otherwise split on whitespace, drop reserved keywords `NOT`, `OR`, `AND`,
  `NEAR` case-insensitively (compared with `eq_ignore_ascii_case`, no
  allocation — `CopyPaste-pbre`). All tokens dropped → `None`.
* Append `*` to **every** token (not just the last) and join with `" AND "`
  (`CopyPaste-8ebg.57`: search-as-you-type means any token can be mid-word;
  the old last-token-only behaviour made `"priv key"` match nothing).

**Preview reads:** `fetch_text_preview` / `fetch_text_previews_batch` read
`content_text` from FTS and clamp to `MAX_PREVIEW_BYTES = 1_024`, truncating at
a UTF-8 char boundary and appending `…`. The batch variant collapses a 50-item
page from 51 round-trips to 1.

**Content hash:** `compute_content_hash(raw) = hex(SHA-256(raw))` — the **full**
64-char lowercase digest. `CopyPaste-y4v1`: an earlier daemon helper truncated
to 16 bytes / 32 hex chars; that is forbidden (weakens second-preimage
resistance for free).

### 3.8 Dedup semantics

Two cooperating mechanisms:

**(a) Application-level probe** — `find_recent_by_hash(db, hash, now_ms, within_ms)`:

```sql
SELECT id FROM clipboard_items
 WHERE content_hash = ?1 AND wall_time >= ?2 AND deleted = 0
 ORDER BY wall_time DESC LIMIT 1
```
`?2 = now_ms.saturating_sub(within_ms)` (saturating for parity with
`delete_sensitive_expired` and to avoid a debug-mode overflow panic). The
daemon's window is 60_000 ms. `AND deleted = 0` is mandatory
(`CopyPaste-crh3.67`): soft-delete keeps `content_hash` and bumps `wall_time`,
so without it a tombstone matches the probe and a re-copy of a deleted item can
never come back.

**(b) Storage-level UNIQUE index (TOCTOU backstop)** —

```sql
CREATE UNIQUE INDEX idx_dedup_hash_minute
    ON clipboard_items(content_hash, (wall_time / 60))
    WHERE content_hash IS NOT NULL AND deleted = 0;
```

The probe is a SELECT-before-INSERT, so two ingest events with identical
content in the same bucket can both observe "no recent row". The UNIQUE index
makes the second INSERT fail with `SQLITE_CONSTRAINT_UNIQUE`; the application
then re-queries and returns the existing id — **idempotent dedup at the storage
layer**. `WHERE content_hash IS NOT NULL` exists because image rows have NULL
`content_hash` and would otherwise all collide on the bucket.

**Conflict-recovery path** (`insert_item_with_fts` → `lookup_existing_id_in_tx`,
`items/insert.rs:194`): on any `ConstraintViolation`, still **inside the same
transaction** (so there is no TOCTOU gap between the failed INSERT and the
fallback SELECT):

1. If `item.content_hash` is `Some`, try
   `WHERE content_hash = ?1 AND (wall_time / 60) = ?2 AND deleted = 0
    ORDER BY wall_time DESC LIMIT 1` where `?2 = item.wall_time / 60`.
2. Else / on no rows, fall back to `WHERE item_id = ?1`
   (the `idx_clipboard_item_id` sync-replay case).
3. Found → return that id as if dedup had won the race. Not found → propagate
   the original SQLite error.

**`idx_clipboard_item_id`** (`UNIQUE(item_id)`) closes the sync-replay window:
a peer re-broadcasting the same item cannot double-insert. Sync-layer dedup is
therefore a performance optimisation, not a correctness requirement.

### 3.9 Soft delete / tombstones

`soft_delete_item(db, id, lamport_ts, wall_time)` (and the in-transaction
variant `soft_delete_item_in_tx`, `CopyPaste-jvzm.3`, used by batch
`delete_all`) performs, in one transaction:

```sql
UPDATE clipboard_items
   SET deleted = 1,
       is_synced = 0,
       content = NULL,
       content_nonce = NULL,
       thumb = NULL,
       lamport_ts = ?2,
       wall_time = ?3
 WHERE id = ?1
```

then, only if `changed > 0`:
* `delete_pending_uploads_for_ids` (`CopyPaste-bhm9` / `-6fd`), and
* `DELETE FROM clipboard_fts WHERE id = ?1`.

Notes:
* `content_hash` is deliberately **kept** on the tombstone (hence the
  `deleted = 0` filters in both dedup paths).
* `id` and `item_id` are kept so LWW can converge; the row is the delete event.
* Tombstones are excluded from every list/count/search query
  (`WHERE deleted = 0`) and included in `get_item_by_item_id`.
* **`insert_tombstone`** (`items/insert.rs:269`, `CopyPaste-bfiu`) handles the
  delete-before-create race: when a delete arrives for an `item_id` not yet
  known locally (relay has no cross-push ordering; cloud realtime can reorder),
  a fresh row is inserted with `content_type='text'`, all blobs NULL,
  `is_sensitive=0`, `is_synced=1`, `deleted=1`, the incoming
  `lamport_ts`/`wall_time`, the incoming `origin_device_id`, and
  `key_version = ITEM_KEY_VERSION_CURRENT`. Never FTS-indexed. A UNIQUE
  conflict here is surfaced as an error (the caller should have taken the
  soft-delete-existing path).
* Hard delete (`delete_item`) removes the row outright, plus its
  `pending_uploads` row (first) and its FTS row, in one transaction.

### 3.10 Pinning and `pin_order`

`pin_order` is `REAL` (not INTEGER) so a fractional value can be inserted
between two adjacent orders without renumbering the set (reserved for
optimistic client-side reorder without a round-trip).

**`pin_item(db, id)`** — one atomic UPDATE:

```sql
UPDATE clipboard_items
   SET pinned = 1,
       expires_at = NULL,
       pin_order = (SELECT COALESCE(MAX(pin_order), 0) + 1
                      FROM clipboard_items WHERE pinned = 1),
       lamport_ts = MAX((SELECT lamport_ts + 1 FROM clipboard_items WHERE id = ?1), ?2),
       wall_time  = ?2
 WHERE id = ?1
```
`?2 = now_ms`. Newly-pinned items land at the **end** of the pinned section.

**`unpin_item`** — `pinned = 0, pin_order = NULL`, same lamport/wall bump.
`expires_at` stays NULL unless the caller sets a new one.

**`reorder_pinned(db, ids)`** — one transaction; for each `id` at index `i`:

```sql
UPDATE clipboard_items
   SET pin_order = ?1,                       -- (i + 1) as f64  → 1.0, 2.0, …
       lamport_ts = MAX((SELECT lamport_ts + 1 FROM clipboard_items WHERE id = ?2), ?3),
       wall_time  = ?3
 WHERE id = ?2 AND pinned = 1
```
Non-pinned or unknown ids are silently skipped (0 rows) — the "idempotent
reorder" contract. Returns the number of rows actually changed.

**Lamport stamping rule (`CopyPaste-ojhe`)** — every mutating operation stamps
`lamport_ts = max(prev_lamport + 1, now_ms)` (`next_lamport_ts`, `types.rs:65`).
Before this, three colliding conventions shared the same `i64` field (fresh
capture `0`, recopy `now_ms ≈ 1.75e12`, pin/delete `existing + 1`), so a stale
recopy permanently outranked a newer pin/delete under lamport-only LWW: pins
were silently overwritten and deletes resurrected. The unified value space is
both monotonic per row and time-ordered across devices. No migration was needed
— a fresh `now_ms`-based write deterministically dominates a stale low value.
`now_ms` is bound as a parameter (not `strftime`) so `lamport_ts` and
`wall_time` agree exactly for the tie-break.

### 3.11 Expiry / TTL / cap eviction

**`delete_expired(db, now_ms)`** — one transaction:
1. `SELECT id … WHERE expires_at IS NOT NULL AND expires_at < ?1 AND pinned = 0`
2. `delete_pending_uploads_for_ids(ids)` — **before** the row delete
3. `DELETE FROM clipboard_items WHERE expires_at IS NOT NULL AND expires_at < ?1 AND pinned = 0`
4. `delete_fts_for_ids(ids)` — one batched `IN (…)` statement
   (`CopyPaste-c1dd`; was N+1)

**`delete_sensitive_expired(db, now_ms, sensitive_ttl_ms)`** — the *unified*
TTL path (`CopyPaste-3e7y`, atomicity from `CopyPaste-44rq.62`). One
transaction containing:
1. Backfill:
   `UPDATE clipboard_items SET expires_at = MIN(wall_time + ?1, 9223372036854775807)
    WHERE is_sensitive = 1 AND expires_at IS NULL AND pinned = 0`
2. The identical select/clean/delete/fts sequence as `delete_expired`, inlined
   so both steps share one transaction.

Previously the sensitive sweep used a divergent `wall_time < now_ms - ttl_ms`
predicate; a sensitive item with no explicit `expires_at` was invisible to
`delete_expired` and could outlive its TTL. **`expires_at < now_ms` is now the
single canonical TTL predicate.**

**`has_sensitive_items(db)`** — cheap pre-flight
`SELECT EXISTS(SELECT 1 FROM clipboard_items WHERE is_sensitive = 1 AND pinned = 0)`.
**Fail-closed (`CopyPaste-ny0g`): on query error it returns `true`**, so the
sweep still runs. Returning `false` on error would silently suppress the sweep
— a data-retention violation.

**`bump_item_recency(db, id, now_ms, new_lamport, sensitive_ttl_ms)`** — the
re-copy promote path. Sets `wall_time = now_ms`, `lamport_ts = new_lamport`, and
when `sensitive_ttl_ms = Some(t)`:
`expires_at = CASE WHEN is_sensitive = 1 THEN ?1 + ?4 ELSE expires_at END`
(`CopyPaste-89ib` — without the recompute the stale deadline fires immediately
after the bump and wipes content the user just re-copied).

**`prune_to_cap(db, max_bytes)`** — byte-cap eviction:
* Gate: `SELECT COALESCE(SUM(LENGTH(COALESCE(content,''))),0) FROM clipboard_items WHERE pinned = 0`
  — matches `idx_clipboard_unpinned_len` verbatim → index-only.
  `total_unpinned <= max_bytes` → return 0.
* `excess = total_unpinned - max_bytes`.
* **Never evict the newest unpinned live row**: `SELECT id … WHERE pinned = 0
  AND deleted = 0 ORDER BY wall_time DESC, id DESC LIMIT 1` → `keep_id`
  (empty-string sentinel when there is none). Prevents "user copies a huge
  image and it instantly vanishes".
* Single window-function pass (`CopyPaste-yfm8`; the CTE used to be evaluated
  twice):
  ```sql
  WITH ranked AS (
      SELECT id,
             LENGTH(COALESCE(content, '')) AS row_bytes,
             SUM(LENGTH(COALESCE(content, ''))) OVER (
                 ORDER BY wall_time ASC, id ASC ROWS UNBOUNDED PRECEDING
             ) AS cum_bytes
        FROM clipboard_items
       WHERE pinned = 0 AND deleted = 0 AND id <> ?2
  )
  SELECT id FROM ranked WHERE cum_bytes - row_bytes < ?1
  ```
* Eviction order `(wall_time ASC, id ASC)` — deterministic on ties. The
  "tipping" row **is** evicted, so remaining unpinned bytes ≤ `max_bytes`.
* Then `delete_pending_uploads_for_ids` → `DELETE … WHERE id IN (…)` →
  `delete_fts_for_ids`, all in one transaction.
* Requires SQLite ≥ 3.25 (window functions); bundled SQLCipher ships ≥ 3.47.

**`incremental_vacuum(db, max_pages)`** — `PRAGMA incremental_vacuum(N)`.
`N = 0` reclaims all free pages. No-op unless `auto_vacuum = INCREMENTAL`
(only settable on a fresh DB), so callers must not depend on a freed-page count.

### 3.12 Pagination contracts

**Offset variants** (`LIMIT ?1 OFFSET ?2`; both clamped with
`x.min(i64::MAX as usize) as i64` because a negative LIMIT means *unlimited* in
SQLite — "Fix 6"):

| Function | Filter | ORDER BY |
|---|---|---|
| `get_page` | `deleted = 0` | `wall_time DESC` |
| `get_page_meta` | `deleted = 0` | `wall_time DESC` (emits `NULL AS content`) |
| `get_page_pinned_first` | `deleted = 0` | `CASE WHEN pinned = 1 THEN 0 ELSE 1 END ASC, pin_order IS NULL ASC, pin_order ASC, wall_time DESC` |
| `get_page_pinned_first_lamport` | `deleted = 0` | same, then `lamport_ts DESC, wall_time DESC, origin_device_id ASC` |

`get_page_meta` exists because image `content` blobs can be hundreds of KB and
the list view renders from `blob_ref`/type/hash; it emits a literal
`NULL AS content` so the same positional `row_to_item` mapper works.

`get_page_pinned_first` is what the `history_page` IPC verb calls;
`get_page` is kept for pure-recency callers (tests, sync).
`_lamport` is for the Android FFI (`PG-19` / `CopyPaste-o0t3`) — causally
correct after cross-device sync; the daemon path keeps wall-time order to avoid
a behaviour change for existing macOS users.

**Keyset (seek) variants (`CopyPaste-8ebg.57`)** — additive; offset variants
unchanged. Rationale: with OFFSET, a row inserted above the window shifts
everything down, so the next page duplicates or skips rows. Keyset seeks from
the last-seen row's own sort key.

`WallCursor { wall_time, id }` → `get_page_seek` / `get_page_meta_seek`:

```sql
WHERE deleted = 0 AND (?1 = 0 OR wall_time < ?2 OR (wall_time = ?2 AND id < ?3))
ORDER BY wall_time DESC, id DESC LIMIT ?4
```

`?1` is a has-cursor flag (`0` = first page). The `id DESC` tiebreak makes the
order **total**, which keyset pagination requires; `get_page` had no tiebreak
because OFFSET does not need one.

**Mixed ASC/DESC comparison rule.** `get_page_pinned_first(_lamport)` sort by
five/seven columns with **mixed directions**. A single SQLite row-value
comparison `(a,b,c,d,e) < (v1,…,v5)` assumes all columns compare in the same
direction and therefore **cannot** express this. The portable construction is
the standard keyset boolean-OR expansion — one OR-branch per sort column, with
`>` on ASC columns and `<` on DESC columns:

```
col1 >? v1
OR (col1 = v1 AND col2 >? v2)
OR (col1 = v1 AND col2 = v2 AND col3 >? v3)
OR …
```

`PinnedCursor { bucket, pin_order_is_null, pin_order: Option<f64>, wall_time, id }`
→ `get_page_pinned_first_seek`, with ORDER BY
`CASE WHEN pinned = 1 THEN 0 ELSE 1 END ASC, pin_order IS NULL ASC,
pin_order ASC, wall_time DESC, id ASC` and the predicate:

```sql
WHERE deleted = 0 AND (?1 = 0
  OR  (CASE WHEN pinned=1 THEN 0 ELSE 1 END) > ?2
  OR ((CASE WHEN pinned=1 THEN 0 ELSE 1 END) = ?2 AND (CASE WHEN pin_order IS NULL THEN 1 ELSE 0 END) > ?3)
  OR ((CASE …) = ?2 AND (CASE …NULL…) = ?3 AND pin_order IS NOT ?4 AND (pin_order > ?4 OR ?4 IS NULL))
  OR ((CASE …) = ?2 AND (CASE …NULL…) = ?3 AND pin_order IS ?4 AND wall_time < ?5)
  OR ((CASE …) = ?2 AND (CASE …NULL…) = ?3 AND pin_order IS ?4 AND wall_time = ?5 AND id > ?6))
```

Note the **NULL-safe** comparisons: `pin_order IS ?4` / `IS NOT ?4` (SQLite's
`IS` is null-safe `=`), plus the explicit `OR ?4 IS NULL` so a cursor sitting on
a NULL `pin_order` advances past all NULL-order rows.

`PinnedLamportCursor { bucket, pin_order_is_null, pin_order, lamport_ts,
wall_time, origin_device_id, id }` → `get_page_pinned_first_lamport_seek`,
ORDER BY `… pin_order ASC, lamport_ts DESC, wall_time DESC,
origin_device_id ASC, id ASC` with the same expansion (7 OR-branches).

**Cursor semantics:** the cursor is the *last row of the previous page*.
`None` = first page. `id ASC` is the final deterministic tiebreak the cursor
variants add (the offset variants have none).

**Other reads:**
* `count_items` → `SELECT COUNT(*) … WHERE deleted = 0`
* `get_item_by_id` → by row PK, no `deleted` filter; re-maps
  `rusqlite::Error::IntegralValueOutOfRange(14, v)` → `ItemsError::CorruptKeyVersion(v)`
* `get_item_by_item_id` → by `item_id`, **no** `deleted` filter (merge layer)
* `exists_item_by_item_id` → `SELECT COUNT(1) … WHERE item_id = ?1`
* `decrypt_page` (`CopyPaste-00zz`) → `get_page` then per-row decrypt;
  a row that fails AEAD verification, has a malformed/absent nonce, or an
  unknown `key_version` is **skipped and counted** in `DecryptedPage::skipped`
  instead of surfacing one error per row (~629 errors on a single launch after
  a key rotation). "Graceful" means *skip*, never *bypass*: a failed auth tag is
  never accepted. Tombstone/blob rows (`content = None`) count as skipped.

### 3.13 Connection pool (`pool.rs`)

* `SqlitePool = r2d2::Pool<SqliteConnectionManager>`; `ReadHandle` wraps a
  pooled connection and implements `DbRead`.
* `DbRead` is implemented by both `Database` (the single writer) and
  `ReadHandle`, so every read-only storage function takes `&D: DbRead + ?Sized`.
* WAL allows many readers + one writer concurrently. The daemon routes
  `list`/`count`/`search`/`history_page`/`stats` through a **4-connection** pool
  (`CopyPaste-j8p`).
* `with_init` hook re-applies the whole pragma batch (key first) to every fresh
  connection — these pragmas are per-connection and are not persisted.
* **Pre-flight**: before building the pool, a throw-away connection applies the
  pragmas and reads `PRAGMA user_version`; `0` → `PoolError::SchemaNotInitialized`
  (`CopyPaste-44rq.63`). `open_pool` does **not** run migrations.

### 3.14 Error taxonomy worth porting

`DbError` (`db/error.rs`) promotes operational SQLite failures via `From`:
`SQLITE_FULL` → `DiskFull`, `SQLITE_READONLY` → `ReadOnly`,
`SQLITE_BUSY`/`SQLITE_LOCKED` → `Locked`. Plus `Schema`, `Migration(String)`,
`CheckpointFailed(String)`, `PlaintextMigrationBlocked { path, size }`.

`ItemsError`: `Sqlite`, `Db`, `MigrationInProgress`, `UnsupportedKeyVersion(u8)`,
`CorruptKeyVersion(i64)`.

`SchemaError`: `Sqlite`, `Downgrade { found, expected }`.

`PoolError`: `Build`, `Sqlite`, `SchemaNotInitialized`.

### 3.15 Environment-variable escape hatches (part of the data contract)

| Var | Effect |
|---|---|
| `COPYPASTE_NO_AUTO_MIGRATE=1` | plaintext DB → `PlaintextMigrationBlocked` instead of in-place encryption |
| `COPYPASTE_FORCE_MIGRATION_COMPLETE` | `force_migration_complete()` — unconditionally sets `completed_at`, releasing the ingest write-gate even with `key_version = 1` rows remaining |
| `COPYPASTE_PURGE_DEAD_V1_ROWS=1` | `purge_dead_v1_rows()` — permanently deletes undecryptable `key_version = 1` rows (+ their FTS rows) in one transaction |

---

## 4. Migration ladder v1 → v15

Runner: `schema::apply_migrations` → `build_migration_script(conn, current_version)`.
The script is one `BEGIN; … PRAGMA user_version=15; COMMIT;` string built by
appending each `if current_version < N` block in order.

**Retry loop (`CopyPaste-lmlr` / `CopyPaste-2lc9`):** build + execute is wrapped
in a bounded loop of **3 attempts**. A concurrent connection's WAL-replay can
materialise a column *after* the build-time `column_exists` probe returned false
but *before* the queued ALTER runs → "duplicate column name". SQLite rolls back,
the racily-added column genuinely persists, the script is rebuilt (now the probe
says true, the ALTER is skipped) and the retry converges. Bounded so a real
schema bug still surfaces. Each attempt is a self-contained `BEGIN…COMMIT`, so
atomicity holds.

| v | What changed | Why | Idempotency |
|---|---|---|---|
| **1** | Baseline: `clipboard_items` (12 cols), `idx_clipboard_wall_time`, `idx_clipboard_expires`, `clipboard_fts`, `devices`, `settings`, `pending_uploads` (`schema_v1.sql`) | initial schema | whole file uses `CREATE … IF NOT EXISTS` |
| **2** | `+ content_hash TEXT`; `+ idx_clipboard_content_hash … WHERE content_hash IS NOT NULL` | SHA-256 dedup | ALTER guarded by `column_exists`; index `IF NOT EXISTS` (always emitted) |
| **3** | `+ origin_device_id TEXT NOT NULL DEFAULT ''` (`V3_ALTER_SQL`) | LWW merge tie-break (`copypaste-sync::merge::resolve`). SQLite requires a *literal* default for ADD COLUMN, hence `''`; the daemon stamps the real UUID via `backfill_origin_device_id` after open | ALTER guarded; backfill is `WHERE origin_device_id = ''` so peer-origin rows are never overwritten |
| **4** | `+ key_version INTEGER NOT NULL DEFAULT 1`; `+ idx_clipboard_key_version … WHERE key_version < 2` (`V4_ALTER_SQL`) | v0.3 T5: mark which HKDF family encrypted each row. `DEFAULT 1` backfills existing rows as legacy so the sweep's `WHERE key_version = 1` finds them; new inserts write `2` explicitly | ALTER guarded; **if the column already exists the index is still emitted** (separate `else` branch) |
| **5** | Two UNIQUE indexes: `idx_dedup_hash_minute` on `(content_hash, (wall_time / 60)) WHERE content_hash IS NOT NULL`, and `idx_clipboard_item_id` on `(item_id)` (`schema_v2.sql`) | TOCTOU dedup + sync-replay protection. Historically shipped in beta as user_version=4 (`V4_INDEXES_SQL`) but v3 had already claimed 4 for `key_version`, so it was renumbered to 5 on merge into v0.3. The SQL file keeps the name `schema_v2.sql` for historical reasons | both `CREATE UNIQUE INDEX IF NOT EXISTS` |
| **6** | `CREATE TABLE migration_state (…)` + seed row `'v4-key-version-sweep'` | resumable v4 key-rotation sweep tracking; `Database::migration_state()` must always find a valid state | `CREATE TABLE IF NOT EXISTS` + `INSERT OR IGNORE`. **Seed logic:** `completed_at = CASE WHEN (SELECT COUNT(*) FROM clipboard_items WHERE key_version = 1) = 0 THEN strftime('%s','now') ELSE NULL END` — a fresh install is seeded Complete so the ingest write-gate never blocks a brand-new DB; an upgrade with legacy rows is seeded NULL so the startup sweep runs |
| **7** | `+ pinned INTEGER NOT NULL DEFAULT 0`; `+ idx_clipboard_pinned … WHERE pinned = 1` (`V7_ALTER_SQL`) | v0.3 pinned-fix: pinning used to be "clear `expires_at`", indistinguishable from a normal no-TTL row, so prunes could delete pinned items. `DEFAULT 0` is safe — nothing is lost, old pins are re-pinnable | ALTER guarded; **else-branch still emits the index** |
| **8** | `+ pin_order REAL DEFAULT NULL`; then `UPDATE clipboard_items SET pin_order = CAST(rowid AS REAL) WHERE pinned = 1` (`V8_ALTER_SQL`) | A1 drag-to-reorder. Backfill gives existing pinned rows a stable insertion-order sequence; unpinned rows stay NULL. REAL so fractional inserts avoid renumbering | ALTER + backfill are emitted **together**, both skipped if the column exists (the comment records: "If column exists, the UPDATE (backfill) was already applied; skip") |
| **9** | `+ thumb BLOB DEFAULT NULL` (`V9_ALTER`) | Variant B image thumbnails: small capture-time encrypted preview keyed by a distinct `thumb_file_id` recorded in the image `blob_ref` meta JSON. NULL = "no thumbnail yet"; lazily backfillable via `set_thumb` | ALTER guarded |
| **10** | `+ deleted INTEGER NOT NULL DEFAULT 0`; `+ idx_clipboard_deleted … WHERE deleted = 1` (`V10_ALTER`) | op-propagation foundation: soft-delete tombstones that LWW-propagate. `DEFAULT 0` = all existing rows live. Partial index keeps tombstone enumeration O(tombstones) | ALTER guarded; **else-branch still emits the index** |
| **11** | `+ idx_clipboard_unpinned_len ON clipboard_items(LENGTH(COALESCE(content,''))) WHERE pinned = 0` (`V11_INDEX`, `CopyPaste-pvp4`) | `prune_to_cap`'s per-write gate was full-scanning the table and reading every encrypted BLOB on **every clipboard write**. `idx_clipboard_pinned` is partial on `pinned = 1` and cannot serve the inverted predicate | index-only, `IF NOT EXISTS`, no data change |
| **12** | `CREATE TABLE revoked_devices (…)` + `idx_revoked_devices_revoked_at` (`V12_REVOKED_DEVICES_SQL`, `CopyPaste-61fu`) | the table used to be created ad-hoc by `devices::ensure_revoked_devices_table` at daemon startup; any path calling `revoke_device` first panicked with "no such table". Moving the DDL into the chain guarantees existence regardless of call order | `CREATE TABLE/INDEX IF NOT EXISTS`; DBs that already have it from the ad-hoc path are unaffected. `ensure_revoked_devices_table` is retained only as a defence-in-depth net |
| **13** | `DELETE FROM clipboard_fts WHERE id IN (SELECT id FROM clipboard_items WHERE is_sensitive = 1)` (`V13_PURGE_SENSITIVE_FTS`, `CopyPaste-i6pp`, ADR-015) | pre-fix `insert_item_with_fts`/`upsert_fts` did not guard on `is_sensitive`, so shipped databases can contain **plaintext passwords/tokens/PII** in the FTS table. One-time purge | idempotent (no-op on a clean DB); sub-select is an indexed PK lookup, O(n_sensitive) |
| **14** | `+ idx_clipboard_history_page ON clipboard_items(pinned DESC, pin_order, wall_time DESC) WHERE deleted = 0` (`V14_INDEX`, `CopyPaste-89rd`) | `history_page` (primary read path for Tauri UI and CLI `list`) did a full scan + filesort on every call — O(n) per page. With this index SQLite splits the pinned-first ORDER BY into two bounded range scans. Verified with `EXPLAIN QUERY PLAN` (`SEARCH … USING INDEX` instead of `SCAN`) | index-only, `IF NOT EXISTS`, no data change |
| **15** | `DROP INDEX IF EXISTS idx_dedup_hash_minute;` then recreate with `WHERE content_hash IS NOT NULL AND deleted = 0` (`V15_DEDUP_INDEX_FIX`, `CopyPaste-fuxl`) | the original predicate kept tombstones in the dedup index, so re-copying content that had been soft-deleted **within the same bucket** hit a UNIQUE violation and the insert fell back to the tombstone id — the re-copy silently vanished | DROP+CREATE cannot fail on existing data: the new index covers a strict **subset** of the old one's rows, so uniqueness over the `deleted = 0` subset was already guaranteed. Fresh installs run v15 in the same chain, so every DB converges on the same predicate |

### 4.1 Migration invariants the rewrite must preserve

* `column_exists(conn, table, column)` uses
  `SELECT COUNT(*) FROM pragma_table_info(?1) WHERE name = ?2` — works inside
  and outside transactions, on every targeted SQLite version.
* The `else` branches on v4 / v7 / v10 exist because the ALTER const bundles the
  index with the column; if the column pre-exists the index must **still** be
  created. Do not drop them.
* v8's backfill is bundled with the ALTER and is skipped as a unit. A rewrite
  that separates them must keep the `WHERE pinned = 1` predicate and the
  `CAST(rowid AS REAL)` value so existing pin orders do not change.
* `PRAGMA user_version` is the **only** version marker; it is set once, at the
  end of the script.
* Nothing in the ladder ever drops a column or rewrites the table — SQLite
  column order in `clipboard_items` is therefore the ladder order (this matters
  because the old code reads columns **positionally**).

### 4.2 v4 key-rotation sweep (data migration, not schema)

Runs after `apply_migrations` commits, driven by the daemon at startup.
Tracked in `migration_state` under key `'v4-key-version-sweep'`.

`MigrationState` derivation (`db/migration_state.rs:43`):

| Row state | Result |
|---|---|
| no row | `NotStarted` |
| `completed_at IS NOT NULL` | `Complete` |
| `completed_at IS NULL` | `InProgress { last_id: last_processed_id }` |

**Write gate:** `insert_item`, `insert_item_with_fts` and `insert_tombstone` all
return `ItemsError::MigrationInProgress` while the state is `InProgress`.

`migration_v4_sweep_resumable(v1_key, v2_key)`:
1. ensure table + `INSERT OR IGNORE` seed row,
2. if `Complete` **and** `COUNT(*) WHERE key_version = 1 == 0` → return 0,
3. if `Complete` but v1 rows exist → set `completed_at = NULL` and sweep,
4. run `migrate_v1_to_v2_keys` (batched, `BATCH_SIZE` / `INTER_BATCH_SLEEP`),
5. **unconditionally** set `last_processed_id = MAX(rowid)` and
   `completed_at = strftime('%s','now')`, even when rows remain.
   Leaving `completed_at = NULL` for permanently-undecryptable rows kept the
   write-gate armed **forever**, rejecting every new capture (the live-install
   bug). Remaining rows are logged with a WARN count.

Related helpers: `force_complete_if_no_v1_rows()` (recovery for fresh installs
seeded `InProgress` with zero rows), `force_migration_complete()` (env escape
hatch), `count_dead_v1_rows()`, `purge_dead_v1_rows()` (opt-in destructive),
`repair_mislabeled_kv2_blob_rows(v1_key, v2_key)` (image/file rows encrypted
with the v1 key but stamped `key_version = 2` by a pre-fix writer in
`daemon::handle_image` / `handle_file`; probes v1-decrypt, re-encrypts on
success, leaves correctly-v2 rows alone; idempotent).

---

## 5. Acceptance tests to re-create

These are the tests that encode the earned knowledge. Names are the old ones so
they can be cross-referenced; the rewrite should reproduce the **behaviour**,
not the implementation.

### 5.1 MANDATORY — migration & openability

| # | Test | Assertion | Old location |
|---|---|---|---|
| M1 | **open a v1 DB and migrate to current** | Stage a **plaintext** SQLite file with `schema_v1.sql` + `PRAGMA user_version = 1`, close it, then `Database::open(path, key)`. Must (a) auto-encrypt to SQLCipher in place, (b) migrate 1→15, (c) `PRAGMA user_version == 15`, (d) all v1 rows intact | `tests/migration.rs::pragma_user_version_advances_atomically`, `::stage_v1_plaintext` |
| M2 | v0 (empty file, `user_version = 0`) migrates to current | baseline tables created, then all later steps, in one atomic batch | `tests/migration.rs::migrate_v0_to_v1_adds_baseline_tables` |
| M3 | Fresh DB lands directly at current version | not via per-step replay | `tests/migration.rs::fresh_db_creates_at_current_user_version`, `schema/tests.rs::fresh_db_reaches_current_schema_version` |
| M4 | **Row bytes survive migration unchanged** | `content`, `content_nonce`, `lamport_ts`, `wall_time` byte-identical after v1→15 — proves migrations are pure ALTER/CREATE and never rewrite rows | `tests/migration.rs::existing_rows_preserved_through_migration` |
| M5 | Legacy rows get NULL `content_hash` after v2 | | `tests/migration.rs::partial_migration_does_not_corrupt_data` |
| M6 | Re-open is a no-op (equal-version fast path) ×3 | no corruption, no duplicate-column error | `tests/migration.rs::migrate_idempotent_rerun_is_noop`, `schema/tests.rs::equal_version_is_noop`, `db/tests.rs::migration_is_idempotent` |
| M7 | **Downgrade is refused** | `user_version = 999` → `SchemaError::Downgrade { found: 999, expected: 15 }`, file untouched | `schema/tests.rs::downgrade_returns_explicit_error` |
| M8 | **Migration is atomic on failure** | build a v12 DB, `DROP TABLE clipboard_fts`, run migrations → v13 fails → `user_version` still 12 | `schema/tests.rs::apply_migrations_is_atomic_on_failure` |
| M9 | ALTER is idempotent when the column pre-exists | v1 DB + pre-added `content_hash` + `user_version = 1` → succeeds, reaches 15, `content_hash` appears **exactly once** | `schema/tests.rs::v2_migration_idempotent_when_column_exists` |
| M10 | **WAL-replay duplicate-column regression** | file DB: conn A writes v1 + `content_hash` in WAL mode and drops **without checkpointing**; conn B `apply_migrations` must succeed and reach 15 with one `content_hash` | `schema/tests.rs::wal_replay_does_not_cause_duplicate_column` |
| M11 | Duplicate-column detector matches only that error | a real duplicate-column error → retryable; "no such table" → NOT retryable | `schema/tests.rs::is_duplicate_column_error_matches_only_duplicate_column` |
| M12 | Per-version column/table presence on a fresh DB | `origin_device_id`, `key_version`, `pinned`, `pin_order`, `thumb`, `deleted`, `migration_state`, `revoked_devices` | `schema/tests.rs::fresh_db_has_*` |
| M13 | Per-version backfill defaults on an upgraded DB | v3→v4 marks existing rows `key_version = 1`; v6→v7 marks them unpinned; v8→v9 gives NULL `thumb`; v9→v10 gives `deleted = 0` | `schema/tests.rs::v3_to_v4_…`, `v6_to_v7_…`, `v8_to_v9_…`, `v9_to_v10_…` |
| M14 | v12 creates `revoked_devices` and is idempotent when the ad-hoc table already exists | | `schema/tests.rs::v11_to_v12_migration_creates_revoked_devices_table`, `::v12_migration_is_idempotent_when_table_already_exists` |
| M15 | v14 creates `idx_clipboard_history_page` **and** `EXPLAIN QUERY PLAN` for the history-page SQL references it (not a bare `SCAN clipboard_items`) | | `schema/tests.rs::v14_migration_creates_history_page_index`, `::history_page_query_uses_index_not_full_scan` |
| M16 | v5 UNIQUE indexes actually enforce uniqueness | duplicate `(content_hash, wall_time/60)` and duplicate `item_id` both rejected | `tests/migration.rs` (v3→v4 index section) |

### 5.2 MANDATORY — sensitive / FTS (ADR-015)

| # | Test | Assertion | Old location |
|---|---|---|---|
| S1 | **A sensitive item is never in FTS** — insert path | `insert_item_with_fts(db, sensitive_item, "secret text")` → zero rows in `clipboard_fts` for that id, **even though non-empty plaintext was passed** | `items/tests.rs::sensitive_item_not_indexed_in_fts_by_insert_item_with_fts` |
| S2 | A sensitive item is never in FTS — upsert path | `upsert_fts(db, sensitive_id, "secret")` → no-op, `Ok(())` | `items/tests.rs::upsert_fts_rejects_sensitive_item` |
| S3 | Search never returns a sensitive item even with a **stale** FTS row | direct `INSERT INTO clipboard_fts` for a sensitive id, then `search_items` → empty | `items/tests.rs::search_items_does_not_return_sensitive_items` |
| S4 | **v13 purges pre-existing sensitive FTS rows** | build a v12 DB with one sensitive + one normal FTS row → migrate → sensitive row gone, normal row survives, `user_version == 15` | `schema/tests.rs::v13_migration_purges_sensitive_fts_rows` |
| S5 | v13 is a no-op on a clean DB | | `schema/tests.rs::v13_migration_is_noop_when_no_sensitive_fts_rows_exist` |
| S6 | `mark_sensitive` removes the FTS entry atomically, and repairs a stale row for an already-sensitive item | | `items/tests.rs::mark_sensitive_removes_fts_entry`, `::mark_sensitive_clears_stale_fts_for_already_sensitive_item`, `::mark_sensitive_unknown_id_is_noop` |
| S7 | **Sensitive items never carry a thumbnail** | both insert paths suppress it; `set_thumb` refuses a non-None blob on a sensitive row but always allows clearing | `items/tests.rs::sensitive_image_insert_item_suppresses_thumb`, `::sensitive_image_insert_item_with_fts_suppresses_thumb`, `::set_thumb_suppresses_backfill_for_sensitive_item`, `::non_sensitive_image_insert_retains_thumb` |
| S8 | `has_sensitive_items` **fails closed** — returns `true` on a query error | | `items/tests.rs::has_sensitive_items_fails_closed_on_db_error` |

### 5.3 Encryption / keying

| # | Test | Assertion | Old location |
|---|---|---|---|
| E1 | Wrong key is rejected | `SQLITE_NOTADB` surfaces as an invalid-key error, not a plaintext-migration attempt | `db/tests.rs::encrypted_db_rejects_wrong_key`, `tests/wrong_key_err.rs`, `tests/encryption_at_rest.rs::wrong_key_open_returns_invalid_key_error` |
| E2 | Round-trip with the correct key | | `db/tests.rs::encrypted_db_round_trips_with_correct_key` |
| E3 | **Plaintext DB is auto-encrypted on first keyed open** | | `db/tests.rs::plaintext_db_is_migrated_on_first_encrypted_open` |
| E4 | `COPYPASTE_NO_AUTO_MIGRATE=1` blocks it | `PlaintextMigrationBlocked { path, size }` | `Database::open_no_auto_migrate` |
| E5 | **Rekey preserves data and `user_version`**, old key stops working, new key works | | `db/tests.rs::rekey_changes_encryption_key`, `tests/corruption.rs::rekey_changes_key_and_data_still_readable`, `tests/encryption_at_rest.rs::key_rotation_old_key_no_longer_works_new_key_works` |
| E6 | Rekey to the same key is a no-op | | `tests/corruption.rs::rekey_to_same_key_is_noop` |
| E7 | **No plaintext on disk** — raw `.db` / `.db-wal` / `.db-shm` bytes never contain the payload (text or image) | | `tests/encryption_at_rest.rs::db_file_bytes_do_not_contain_plaintext_payload`, `::db_file_bytes_do_not_contain_plaintext_image` |
| E8 | Encrypted file does **not** start with the `SQLite format 3\0` header | | `tests/encryption_at_rest.rs::db_file_starts_with_sqlite_format_header_only_if_unencrypted` |
| E9 | Corrupted WAL does not silently lose data; corrupted main file returns an error | | `tests/corruption.rs::corrupted_wal_does_not_silently_lose_data`, `::corrupted_main_file_returns_error` |

### 5.4 Pragmas & pool

| # | Test | Assertion | Old location |
|---|---|---|---|
| P1 | DB opens in WAL mode | `PRAGMA journal_mode == "wal"` | `db/tests.rs::database_opens_with_wal_mode` |
| P2 | `cache_size` maps MB→negative KiB and clamps to `[1, 256]` | 8 → `-8192` | `db/tests.rs::cache_size_pragma_maps_mb_to_negative_kib`, `::cache_size_pragma_clamps_out_of_range`, `::open_with_cache_mb_applies_configured_cache_size`, `::open_uses_default_cache_size`, `::open_with_cache_mb_clamps_out_of_range_on_connection` |
| P3 | All tables exist after open | | `db/tests.rs::schema_creates_all_tables` |
| P4 | Pool applies key + WAL + all per-connection pragmas via `with_init` | `PRAGMA cipher_version` non-empty | `pool.rs::pool_pragmas_applied` |
| P5 | **Pool refuses an unmigrated file** | raw SQLCipher file with `user_version = 0` → `PoolError::SchemaNotInitialized` | `pool.rs::pool_rejects_uninitialized_schema` |
| P6 | Pool rejects a wrong key | | `pool.rs::pool_rejects_wrong_key` |
| P7 | N concurrent `ReadHandle`s do not deadlock and go through the `DbRead` trait | | `pool.rs::pool_supports_concurrent_connections`, `::read_handle_concurrent_reads_dont_deadlock`, `tests/pool_stress.rs` |
| P8 | Concurrent writers lose no updates | | `tests/concurrent_writers.rs::concurrent_writers_no_lost_updates` |

### 5.5 Dedup

| # | Test | Assertion | Old location |
|---|---|---|---|
| D1 | Inserting the same text twice returns the existing id and creates no new row | | `tests/dedup.rs::insert_same_text_twice_returns_existing_id_no_new_row`, `items/tests.rs::insert_item_with_fts_dedup_returns_existing_id_on_hash_race` |
| D2 | Same for a sync-replay `item_id` collision | | `items/tests.rs::insert_item_with_fts_dedup_returns_existing_id_on_item_id_race` |
| D3 | `find_recent_by_hash` ignores tombstones | | `tests/dedup.rs::find_recent_by_hash_ignores_soft_deleted_tombstone` |
| D4 | **v15 behaviour: a re-copy after a same-bucket soft delete creates a fresh live row** | | `items/tests.rs::recopy_after_same_bucket_soft_delete_inserts_fresh_live_row` |
| D5 | Different content → two rows; hash is collision-resistant on realistic payloads | | `tests/dedup.rs::different_content_different_hash_creates_two_rows`, `::hash_collision_resistant_for_realistic_payloads` |
| D6 | Hash is content-only — origin app id does not split it | | `tests/dedup.rs::dedup_is_content_only_origin_app_id_does_not_split_hash` |
| D7 | `compute_content_hash` returns the **full 64-char** SHA-256 hex | | `items/tests.rs::compute_content_hash_returns_full_sha256_hex` |
| D8 | `find_recent_by_hash` cutoff does not overflow (`now_ms = 0, within_ms = i64::MAX`) | | `items/tests.rs::find_recent_by_hash_cutoff_no_overflow` |
| D9 | Dedup bump promotes the existing row to the top instead of duplicating | | `items/tests.rs::dedup_bump_prevents_duplicate_row_and_sorts_to_top` |

### 5.6 Soft delete / pinning / ordering

| # | Test | Assertion | Old location |
|---|---|---|---|
| T1 | `insert_tombstone` persists a hidden `deleted = 1` row | invisible to list queries, visible to `get_item_by_item_id` | `items/tests.rs::insert_tombstone_persists_hidden_deleted_row` |
| T2 | Soft delete wipes content/nonce/thumb, clears FTS, cleans `pending_uploads` | | `items/tests.rs::soft_delete_item_cleans_pending_uploads` |
| T3 | `pin_item` clears `expires_at`; pinned items sort first | | `items/tests.rs::pin_item_removes_expiry`, `::get_page_pinned_first_pins_before_unpinned` |
| T4 | Multiple pins sort by `pin_order`; NULL `pin_order` sorts **last among pins** | | `items/tests.rs::get_page_pinned_first_multiple_pins_sorted_by_pin_order`, `::get_page_pinned_first_null_pin_order_sorts_last_among_pins` |
| T5 | Pin/unpin/reorder all bump `lamport_ts` to `max(prev+1, now_ms)` | | `items/tests.rs::pin_unpin_bumps_lamport_ts`, `::reorder_pinned_bumps_lamport_ts`, `::next_lamport_ts_is_monotonic_and_time_ordered` |
| T6 | **A newer pin beats an older recopy under lamport LWW** (the `CopyPaste-ojhe` regression) | | `items/tests.rs::newer_pin_lamport_beats_older_recopy_lamport` |
| T7 | Pin/unpin changes sort position | | `items/tests.rs::pin_and_unpin_changes_sort_position` |

### 5.7 Expiry / prune / eviction

| # | Test | Assertion | Old location |
|---|---|---|---|
| X1 | `delete_expired` removes expired rows and their FTS rows and `pending_uploads` | | `items/tests.rs::delete_expired_removes_old_items`, `::delete_expired_cleans_pending_uploads` |
| X2 | **Pinned items are never TTL-deleted** | | `items/tests.rs::delete_sensitive_expired_keeps_pinned_items` |
| X3 | Sensitive TTL goes through the unified `expires_at` path (backfill + delete in one tx) | | `items/tests.rs::delete_sensitive_expired_unified_via_expires_at`, `::delete_sensitive_expired_removes_old_sensitive_items` |
| X4 | `bump_item_recency` recomputes `expires_at` for sensitive items only | | `items/tests.rs::bump_item_recency_recomputes_expires_at_for_sensitive_items`, `::bump_item_recency_does_not_set_expires_at_for_non_sensitive_items` |
| X5 | `prune_to_cap`: no-op under/at quota; evicts oldest-first; **the tipping row is evicted**; pinned never evicted; NULL content counts as 0; no FTS orphans; cleans `pending_uploads`; never evicts tombstones | | `items/tests.rs::prune_to_cap_*` (11 tests) |
| X6 | `prune_to_cap` single-pass result equals the naive reference on a large dataset | | `items/tests.rs::prune_to_cap_large_dataset_matches_naive_eviction`, `::prune_to_cap_single_pass_matches_reference` |
| X7 | The size gate uses the covering index (`EXPLAIN QUERY PLAN`) and the index exists | | `items/tests.rs::schema_has_unpinned_len_covering_index`, `::prune_to_cap_size_gate_uses_covering_index` |

### 5.8 Query / pagination / FTS behaviour

| # | Test | Assertion | Old location |
|---|---|---|---|
| Q1 | Offset pagination returns the correct page; `get_page_meta` omits `content` but keeps metadata | | `items/tests.rs::pagination_returns_correct_page`, `::get_page_meta_omits_content_blob_but_keeps_metadata` |
| Q2 | `get_item_by_id` finds a row **beyond the first page** (the paging-footgun regression) | | `items/tests.rs::get_item_by_id_finds_row_beyond_first_page` |
| Q3 | Corrupt `key_version` surfaces as `CorruptKeyVersion`, never truncated | | `items/tests.rs::row_to_item_corrupt_key_version_returns_error`, `::insert_rejects_out_of_range_key_version` |
| Q4 | New rows land on `key_version = 2`; the value is persisted from the item, not a constant | | `items/tests.rs::newly_inserted_items_land_on_key_version_2`, `::insert_persists_item_key_version_not_constant` |
| Q5 | `upsert_fts` insert + atomic replace; `delete_fts` removes the entry and is OK for a missing id | | `items/tests.rs::upsert_fts_inserts_and_replaces`, `::upsert_fts_atomic_replace`, `::delete_fts_*` |
| Q6 | Search: exact word, prefix `*`, quoted phrase, no-match, Unicode, BM25 rank order, case-insensitive, special chars escaped safely | | `tests/fts5_search.rs` (all) |
| Q7 | **A hyphenated query does not error and finds the hyphenated term** | `foo-bar` → `foo* AND bar*` | `items/tests.rs::search_items_hyphen_query_does_not_error`, `::search_items_finds_hyphenated_term`, `::sanitize_fts5_query_rewrites_hyphen_to_space` |
| Q8 | `search_items_filtered` by content type; unknown type → empty; empty query → empty | | `items/tests.rs::search_items_filtered_*` |
| Q9 | Preview clamping: short unchanged, long truncated with `…`, UTF-8 boundary respected; batch variant matches per-id variant and is a no-op for empty ids | | `items/tests.rs::clamp_preview_*`, `::fetch_text_preview*`, `::fetch_text_previews_batch_*` |
| Q10 | `decrypt_page` skips undecryptable legacy rows and **counts** them | | `items/tests.rs::decrypt_page_skips_undecryptable_legacy_rows_and_counts_them` |
| Q11 | **Keyset pagination: two consecutive fetches never duplicate or skip an unmoved row, including across the pinned/unpinned boundary and with NULL `pin_order`** | (no direct old test — the seek functions are additive and under-tested; **add this**) | — |

### 5.9 v4 key sweep

| # | Test | Assertion | Old location |
|---|---|---|---|
| K1 | 50 rows all land on `key_version = 2`; sweep is idempotent; zero v1 rows returns 0 | | `migration_v4/tests.rs::migrate_50_rows_all_land_on_key_version_2`, `::migration_is_idempotent`, `::migration_with_no_v1_rows_returns_zero` |
| K2 | A migrated row is **undecryptable with the v1 key** (proves the rotation really happened) | | `migration_v4/tests.rs::migrated_row_is_undecryptable_with_v1_key` |
| K3 | A corrupt row does not abort the sweep (text and image); a **full batch** of undecryptable rows still terminates | | `migration_v4/tests.rs::corrupt_v1_row_does_not_abort_the_sweep`, `::corrupt_image_row_does_not_abort_the_sweep`, `::full_batch_of_undecryptable_*_rows_terminates` |
| K4 | Image chunk rows rotate and preserve `file_id` from `blob_ref`; idempotent | | `migration_v4/tests.rs::image_chunk_row_migrates_to_key_version_2`, `::image_chunk_migration_is_idempotent`, `::parse_file_id_*` |
| K5 | Mislabeled kv2 blob rows are repaired; correctly-encrypted kv2 rows are **not touched**; multi-batch paging works | | `migration_v4/tests.rs::kv2_mislabeled_image_row_repairs_via_migration`, `::kv2_correctly_encrypted_row_not_touched_by_repair_migration`, `::repair_processes_multi_batch_in_pages` |
| K6 | **A stuck sweep releases the write gate** and inserts succeed afterwards | | `db/tests.rs::stuck_sweep_releases_write_gate_and_insert_succeeds` |
| K7 | `COPYPASTE_FORCE_MIGRATION_COMPLETE` clears a stuck gate | | `db/tests.rs::force_migration_complete_env_clears_a_stuck_gate`, `copypaste-daemon/tests/migration_gate_clears.rs` |
| K8 | `count_dead_v1_rows` / `purge_dead_v1_rows`: atomic across items+FTS, removes orphaned FTS entries | | `db/tests.rs::count_and_purge_dead_v1_rows`, `::purge_dead_v1_rows_is_atomic_fts_and_items_consistent`, `::purge_dead_v1_rows_removes_orphaned_fts_entries` |

### 5.10 New tests the rewrite should add

* **A golden v0.4.1 database fixture** checked into the repo (small, fixed key
  `[0u8; 32]`, a handful of text + image + pinned + sensitive + tombstone rows)
  that every CI run opens and migrates. The old suite only ever *constructs* a
  legacy shape by hand, so it can never catch a divergence between the
  hand-built shape and what v0.4.1 actually wrote.
* **One fixture per shipped `user_version` 1..15**, each opened and migrated to
  current, with row counts and column values asserted. The old suite covers
  v1, v2, v3, v11, v12 and "v12-by-hand"; v4–v10 and v13–v15 upgrade paths are
  only covered indirectly.
* **Round-trip property test**: for every column, `insert → select` returns the
  exact value written (catches positional-projection drift, see §6.3).
* **Keyset pagination property test** (Q11 above).

---

## 6. Known-unjustified complexity we should NOT port

Each item below was verified against the source before being recorded.

### 6.1 The hand-rolled `user_version` migration runner — replace with `rusqlite_migration`

**Verified.** Neither `rusqlite_migration` nor any migration crate appears in
`Cargo.toml` or `Cargo.lock`. The runner is bespoke:
`schema/mod.rs::build_migration_script` concatenates 15 `if current_version < N`
blocks into one `String`, `apply_migrations` executes it, and both live in a
file that had to be marked `// size-exempt` under ADR-017 to stay legal.

What is hand-rolled and should go away:

* string concatenation of SQL with `push_str` / `format!`, including
  `PRAGMA user_version={}` interpolated at the end;
* an ad-hoc `column_exists` probe called at *script-build* time, whose
  build-time/execute-time gap is itself the cause of the retry loop below;
* a 3-attempt retry loop keyed on `is_duplicate_column_error(e)`, which is
  **string matching on an error message**
  (`e.to_string().contains("duplicate column name")`, `schema/mod.rs`) — brittle
  against a SQLite or rusqlite message change, and only pinned by one unit test;
* a manually-maintained narrative of the version ladder in three places: the
  `SCHEMA_VERSION` doc comment (`schema/mod.rs`), the per-const doc comments
  (`schema/versions.rs`), and a duplicated copy in
  `tests/migration.rs:41-62`.

**Recommendation:** express the ladder as 15 `M::up(...)` steps in a
`Migrations` set and call `to_version(15)` / `to_latest()`. `rusqlite_migration`
already uses `PRAGMA user_version` as its marker with the same
"version == number of applied migrations" mapping, so **existing v1..v14
databases resume at exactly the right step with no fixup**.

**But preserve these, they are earned, not accidental:**

* the **downgrade refusal** (I2) — the library will not do this for you;
* **idempotency of every step**. Prefer expressing v2/v3/v4/v7/v8/v9/v10 as
  plain unconditional `ALTER TABLE` (they are guarded by `user_version` once the
  runner is trustworthy) **only if** you also keep the WAL-checkpoint belt;
  otherwise keep an existence probe. The WAL-replay-onto-recreated-file scenario
  (`reset_database` racing a live writer) is real and produced CI failures
  (`CopyPaste-2lc9` / `-lmlr` / `-m45w`);
* `PRAGMA wal_checkpoint(TRUNCATE)` before reading `user_version`, **non-fatal**;
* the `else` branches that still create the index when the column pre-exists
  (v4, v7, v10) — a library step must not lose them;
* v8's ALTER-plus-backfill as an atomic unit.

### 6.2 `migration_state`'s cursor columns — written, never used to resume

**Verified.** The table has `key_version_in_progress` and `last_processed_id`,
both advertised as sweep-resume state. In reality:

* `key_version_in_progress` is written as the literal `2` in three places
  (`schema/mod.rs` v6 seed, `migration_state.rs:90`, `:238`) and **is never read
  anywhere in the workspace**. A `grep` finds only DDL and INSERT column lists.
* `last_processed_id` is written twice (seeded `0`; set to
  `COALESCE(MAX(rowid), 0)` *after* the sweep already finished,
  `migration_state.rs:153-163`) and read exactly once, into
  `MigrationState::InProgress { last_id }`. **`last_id` is never consumed by any
  production code path** — the only reference outside the enum definition is a
  pattern match in `tests/key_version_tests.rs:387`.
* The sweep's own module doc says so explicitly
  (`migration_v4/mod.rs:28-36`): *"`last_processed_id` is NOT written per row or
  per batch. Instead, crash-safety is achieved by the `WHERE key_version = 1`
  predicate itself … The predicate is therefore load-bearing for crash-safety."*

So the "resumable cursor" is fiction. The table's real semantics are a **single
boolean**: `completed_at IS NULL` ⇒ ingest write-gate armed.

**Recommendation:** model it as one flag (plus `started_at`/`completed_at` for
diagnostics) and delete `MigrationState::InProgress`'s `last_id` payload.
**Do not drop the columns from the on-disk table** — SQLite would need a table
rebuild and existing DBs keep them harmlessly; just stop pretending they mean
anything. Keep the `WHERE key_version = 1` predicate — that *is* the resume
mechanism.

Related, and also not worth porting:

* `Database::migration_state()` runs `CREATE TABLE IF NOT EXISTS migration_state`
  **on every call** (`migration_state.rs:45`), and `insert_item` /
  `insert_item_with_fts` / `insert_tombstone` each call it before every write.
  That is a DDL statement plus a SELECT on the clipboard hot path. Once v6 is in
  the ladder the table is guaranteed to exist; read the flag once at startup and
  cache it.
* Two dead statements kept only to silence unused-const warnings:
  `let _ = BATCH_SIZE; let _ = INTER_BATCH_SLEEP;` (`migration_state.rs:174-175`).
* The DDL is duplicated between `schema/mod.rs` (v6, one formatting) and
  `migration_state.rs::MIGRATION_STATE_DDL` (another formatting) — the
  `CopyPaste-crh3.84` comment claims a "single source of truth" that is not
  actually single.

### 6.3 Positional column lists — replace with name-based mapping

**Verified.** The 19-column projection is maintained as **three** parallel string
constants that must stay byte-aligned with each other *and* with a positional
reader:

* `items/insert.rs:11` `ITEM_INSERT_COLUMNS` (INSERT list, 19 `?n` placeholders
  in a hand-written `params![…]`),
* `items/query.rs:12` `ITEM_SELECT_COLUMNS`,
* `items/query.rs:20` `ITEM_SELECT_COLUMNS_CI` (the `ci.`-aliased duplicate),
* `items/types.rs:300` `row_to_item`, which reads `row.get(0)` … `row.get(18)`.

Plus two hand-written SQL strings that inline the same 19 columns with
`NULL AS content` substituted at position 3 (`get_page_meta`,
`get_page_meta_seek`) — those are **not** covered by the constants at all.

The comments record the bug history this caused:

* `CopyPaste-crh3.83` — "a missed edit when adding a column silently corrupts
  positional writes";
* `CopyPaste-crh3.85` — "a missed edit when adding a column caused an
  off-by-one panic in `row_to_item`".

There is further positional coupling: the column index `14` (`key_version`) is
hardcoded twice — in `types.rs:321`
(`rusqlite::Error::IntegralValueOutOfRange(14, kv)`) and in `query.rs:587`
(`Err(rusqlite::Error::IntegralValueOutOfRange(14, v)) => …CorruptKeyVersion`).
Insert a column before `key_version` and that error handling silently stops
matching.

**Recommendation:** derive the row mapping from the struct with
`serde_rusqlite` (`from_row::<ClipboardItem>`) so column *names*, not
positions, bind fields. Then:

* the three projection constants collapse into one generated `SELECT` (or a
  `columns_from_statement` call);
* `get_page_meta`'s `NULL AS content` keeps working (it is aliased to the right
  name already);
* the `ci.`-aliased variant becomes unnecessary if the JOIN uses
  `SELECT ci.* FROM …`;
* the hardcoded index `14` disappears — validate `key_version` in a
  `TryFrom`/`deserialize_with` on the field.

Caveats to honour: `RowId`/`ItemId` are `#[serde(transparent)]` newtypes with
delegating `ToSql`/`FromSql`, so they already round-trip; the boolean columns
are stored as `INTEGER 0/1` and are read as `i64 != 0`, which needs an explicit
converter rather than serde's default `bool`.

**Amended in v2: the defect is fixed, the recommended crate is not used.** The
requirement here is that fields bind by column *name*, and
`copypaste-core/src/storage/model.rs` does that directly —
`row_to_item` is a list of `row.get("…")` calls against one projection macro,
with no positional index anywhere in the crate. What made v1's version painful
was the *scale*: 19 columns, three parallel constants, and a `key_version`
index hardcoded twice. v2 projects seven columns, has one projection (plus its
`ci.`-aliased twin, generated from the same macro), and no `key_version` at all
— rule 3 removed it. `serde_rusqlite` was declared as a dependency in
anticipation of this and never used; carrying a crate to generate seven
`row.get`s that would still need a manual converter for the `INTEGER 0/1`
booleans is cost without a defect to prevent, so the declaration has been
dropped. If the projection grows back towards v1's, revisit this.

### 6.4 Additional accidental complexity found while harvesting

| Item | Evidence | Recommendation |
|---|---|---|
| **Four open constructors** (`open`, `open_with_cache_mb`, `open_no_auto_migrate`, `open_no_auto_migrate_with_cache_mb`) plus two in-memory variants, all differing by one parameter | `db/mod.rs:50-259` | one `DatabaseOptions { cache_mb, auto_migrate_plaintext }` builder |
| **`open_no_auto_migrate` opens the file, probes, then delegates to `open_with_cache_mb`, which opens and probes again** | `db/mod.rs:175-214` | single open, branch on the plaintext-detection result |
| **`cache_size` is applied, clobbered, then re-applied** on every open path: `apply_migrations` sets the compile-time default, and each caller then re-asserts the configured value | `schema/mod.rs`, `db/mod.rs:96,142,253,411` | don't set `cache_size` inside the migration runner at all |
| **`revoked_devices` DDL duplicated** in `versions.rs::V12_REVOKED_DEVICES_SQL` and `devices.rs::ensure_revoked_devices_table` (and re-inlined in several tests) | `versions.rs`, `devices.rs:45-56` | delete the "defence-in-depth safety net" function; v12 guarantees the table |
| **`delete_expired`'s body is copy-pasted inside `delete_sensitive_expired`** to share a transaction | `items/delete.rs:55-79` vs `:145-175` | one `expire_in_tx(tx, now_ms)` helper, two callers |
| **`test-helpers` feature gates `open_in_memory` but the production daemon enables it in `[dependencies]`** — the gate gates nothing | `db/mod.rs:238-259` | either make it unconditional and document why `:memory:` is safe, or give the daemon a real API for the quiesce/restore swap |
| **Duplicated/garbled doc comments**: `sanitize_fts5_query` has two concatenated doc blocks; the `CopyPaste-tteo` comment appears verbatim twice inside `search_items_filtered` | `items/fts.rs:199-229`, `:363-370` | cosmetic, but don't carry it over |
| **`ITEM_KEY_VERSION_CURRENT` is duplicated** as an `i64` in storage and as a `u8` on `ClipboardItem::key_version`, with `validate_key_version` converting between them | `items/mod.rs:26`, `items/types.rs:31` | one enum `KeyVersion { V1, V2 }` with a `TryFrom<i64>` |
| **The `NULL AS content` trick** to reuse the positional mapper for metadata-only pages | `query.rs:212`, `:462` | with name-based mapping (§6.3) this can become a real `ItemMeta` type, or stay — but it is only a wart because of positional reads |

### 6.5 Things that look like complexity but are LOAD-BEARING — port them

Listed explicitly so a reviewer does not "simplify" them away:

1. The **boolean-OR keyset expansion** in the pinned-first seek queries. It is
   verbose because SQLite row-value comparison cannot express a mixed ASC/DESC
   composite key. §3.12.
2. The **NULL-safe `pin_order IS ?` / `IS NOT ?` plus `OR ?4 IS NULL`** guards in
   the same predicates.
3. The **`(wall_time / 60)` expression** in `idx_dedup_hash_minute` — even though
   `wall_time` is milliseconds and the "minute" bucket is really 60 ms. Changing
   it changes dedup behaviour on existing data.
4. `prune_to_cap`'s gate SUM deliberately omitting `AND deleted = 0` so it
   matches the partial index verbatim (`CopyPaste-crh3.3`).
5. `has_sensitive_items` **failing closed** on error (`CopyPaste-ny0g`).
6. Marking the v4 sweep `Complete` even when undecryptable rows remain — the
   alternative armed the ingest write-gate forever.
7. Never evicting the newest unpinned live row in `prune_to_cap`.
8. The `sqlcipher_export` + `PRAGMA rekeyed.user_version = <src>` step — without
   it the rebuilt file re-runs every ALTER.
9. `fsync(tmp)` → `rename` → `fsync(parent dir)` in both `encrypt_existing` and
   `rekey`.
10. All three ADR-015 enforcement layers.

