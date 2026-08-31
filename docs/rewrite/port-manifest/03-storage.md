# Port Manifest 03 — Storage

CopyPaste v2 has one SQLCipher database filename and one canonical schema. The
schema is created transactionally for a new v2 database and verified exactly on
every open. An existing file that does not match is refused without probing,
upgrading, repairing, deleting or overwriting it.

The schema in `copypaste-core::storage::schema` is the authoring source. This
manifest owns observable behaviour and security properties; it does not repeat
column order or DDL as a second schema definition.

## 1. Responsibilities

Storage owns:

- creation and exact verification of the v2 SQLCipher database;
- per-connection keying and pragma policy;
- encrypted item rows, tombstones, pin state and device-local settings;
- atomic dedup, delete, index and retention operations;
- the FTS5 search index and its sensitive-content exclusion;
- deterministic keyset pagination;
- bounded read pooling and durable backup/restore of v2 data.

Crypto owns the item envelope and keys. Sync owns comparison and transport.
IPC owns request limits and user-facing errors.

## 2. Schema lifecycle

1. The product opens only its v2 database path.
2. A missing file is reserved and receives the canonical schema in one
   transaction.
3. An existing file must authenticate with the supplied SQLCipher key and match
   the canonical schema exactly.
4. Wrong key, plaintext SQLite, missing/extra schema objects and altered SQL are
   failures. None starts an alternate open path.
5. Schema verification compares structured SQLite metadata, including tables,
   indexes, triggers and FTS configuration. Formatting differences in stored
   SQL are normalized only where SQLite itself makes them insignificant.
6. The pool is built only after key and schema verification succeeds. Every
   pooled connection applies the raw key before any other statement.
7. No `user_version` ladder, `ALTER TABLE` chain, encounter detector, database
   conversion, format repair or migration-state table belongs in v2.

Adding a second schema is a product change. It requires a decision about what
creates it, what opens it, how failure preserves data and when the first schema
stops being supported. It must not arrive as a convenience branch in `open`.

## 3. Item and transaction invariants

### 3.1 Identity and authentication

- The item id is the cross-device identity and the AEAD AAD identity. It exists
  before encryption and is never regenerated while reconstructing a known item.
- Ciphertext, nonce, content type, content hash, creation stamp, deletion state
  and origin metadata are committed as one item version.
- The four sync comparison fields are stored with the row. Storage does not
  maintain a shadow version table with a second answer.
- Decryption failure never falls back to plaintext or another key. A page may
  skip and count an unreadable row, but may not accept its bytes.

### 3.2 Sensitive content and FTS

Sensitive content never reaches search results. All layers are mandatory:

1. The insert path drops search text when the item is sensitive, even if a
   caller supplied non-empty plaintext.
2. The FTS write re-reads sensitivity inside the transaction before inserting
   an index row.
3. The search query joins only live, non-sensitive, searchable content, so a
   stale or manually planted FTS row cannot surface.
4. A bounded purge removes index rows that have no live searchable owner and
   re-evaluates indexed text against the current detector rules.

The purge removes only FTS entries. It never deletes, tombstones or newly flags
the clipboard item. That asymmetry is deliberate: a false positive may make an
item temporarily unsearchable, but must not destroy user data.

Every delete, tombstone and sensitivity transition removes the matching FTS row
in the same transaction. Every indexed insert commits the item and index entry
in the same transaction. FTS row identifiers and row back-pointers are cleared
together so a reused FTS rowid cannot target the wrong item.

### 3.3 Dedup and ingest

- Content hashing uses the full lowercase SHA-256 digest of the pre-encryption
  bytes.
- The application probe and database uniqueness constraint cooperate: the
  probe avoids expected duplicates; the constraint closes the concurrent
  SELECT-before-INSERT race.
- Conflict recovery re-reads the winner inside the same transaction and returns
  its stored id.
- A dedup bump updates the existing row's recency and returns that row, never
  the rejected candidate id.
- Tombstones do not win live-ingest dedup. Copying deleted content creates a new
  live item rather than returning the grave.
- Simultaneous captures from different devices remain separate sync identities
  unless the sync merge contract explicitly identifies them as one version.

### 3.4 Tombstones and deletion

A soft delete atomically:

- marks the row deleted;
- wipes ciphertext, nonce and presentation payload metadata;
- clears pin state and local search linkage;
- retains the identity and merge fields required to propagate deletion;
- removes the FTS row and dependent upload work.

UI, list, count, copy and search reads exclude tombstones. Sync/version reads
include them. Delete-before-create is representable, so an arriving tombstone
does not need a live row to exist first.

Hard deletion is limited to explicit destructive operations and retention paths
whose policy authorizes it. All dependent state is removed in the same
transaction, with lookup-dependent cleanup occurring before the row disappears.

### 3.5 Pinning

- Pinned rows sort before unpinned rows.
- Pin order is total and stable; id and recency provide deterministic
  tie-breaks.
- Reordering is transactional. Unknown or concurrently unpinned ids are ignored
  rather than corrupting the remaining order.
- A complete client ordering reorders the named items; unmentioned pinned items
  retain relative order after them.
- Pinning is device-local unless the sync contract explicitly adds a separate
  pin-version field. An incoming content version must not silently erase local
  pin state.
- Pinned rows are excluded from TTL and quota deletion.

**Stable rule I9:** pinned items are never auto-deleted.

### 3.11 Retention and quota

- TTL and sensitive auto-wipe operate only on live, unpinned rows.
- Sensitive deletion requires the configured high-confidence verdict. Lower
  confidence may flag or de-index, not delete.
- Retention work is bounded and batchable; one sweep cannot hold the database
  lock or materialize all payloads without a limit.
- Byte quota is calculated from maintained size metadata and covering indexes,
  not by reading every ciphertext on each capture.
- The newest unpinned item is protected from immediate quota eviction. A single
  oversize capture may exceed the quota rather than vanish immediately.
- Victim order is deterministic on equal timestamps.
- Item, FTS and dependent-state cleanup is atomic.
- Arithmetic on timestamps and byte totals saturates or uses checked
  conversion; attacker-controlled sizes do not wrap into an unlimited query.

### 3.12 Pagination and search

History pagination uses an opaque keyset cursor over a total order:

1. pinned before unpinned;
2. pin order inside the pinned run;
3. newest creation stamp first;
4. item id as the final tie-break.

The cursor names the last row of the prior page. An invalid cursor fails
explicitly instead of restarting at the first page. Inserts above the cursor do
not duplicate or skip rows below it. Mixed sort directions use explicit
lexicographic branches rather than an invalid single row-value comparison.

Pinned and unpinned runs may use separate indexable queries, but their union
must have the same order and reach every live row exactly once. Deep-page cost
must be bounded by the seek, not linear in all earlier pages.

History page allocation belongs to storage. It measures actual ciphertext
lengths while lazily iterating the seek queries, retains the first row even
when that row alone exceeds the byte budget, and otherwise stops before the
first over-budget row. The cursor is the last retained row from that read; no
caller may trim a materialized page or re-query to manufacture one. A byte stop
in the pinned run does not top up from unpinned rows.

Search sanitizes arbitrary text into an FTS expression without exposing FTS
operators unintentionally. Unicode alphanumerics survive; malformed quotes,
reserved operators, NUL and punctuation cannot produce SQL or FTS syntax
errors. Empty sanitized input returns no results. Search remains restricted to
live, non-sensitive content even when the index is stale.

## 4. Database administration

- A valid legacy text row whose authenticated body exceeds the current IPC
  content ceiling remains stored unchanged. Full-body read, copy and export
  refuse it safely; list, search and pin responses use a bounded grapheme-safe
  preview so the row can still be protected or deleted.
- Export also refuses the whole operation when the included aggregate cannot
  fit one bounded IPC response; it never truncates or writes a partial result.
- Backup creates a new destination and never overwrites one implicitly.
- Restore requires explicit confirmation, validates the candidate with the
  current device key, exact schema and integrity check, and durably swaps only
  after all checks pass.
- A failed restore leaves the working database and active pool unchanged.
- Restored search state is purged against current sensitive rules before the
  replacement is committed for use.
- User-facing administration errors contain no source path, destination path
  or username.
- Export/import of the product's current interchange format is not a database
  schema path. Imported items pass through normal size, detector, dedup and
  indexing rules.

## 5. Acceptance tests

### 5.1 Open and schema

- A missing v2 database is created with the canonical schema and reopens.
- Wrong key, plaintext SQLite, corrupt database and every missing/altered schema
  object fail without modifying the file.
- No compatibility or schema-ladder dependency appears in production source.
- The key is the first statement on every writer and pooled reader.
- Schema and key errors contain no filesystem path.
- Concurrent readers do not block each other; concurrent writers lose no
  committed update.

### 5.2 Sensitive index

- Supplying search text for a sensitive item creates no FTS row.
- Direct FTS upsert for a sensitive id is refused inside the transaction.
- A planted stale FTS row for a sensitive item is never returned by search.
- Marking an item sensitive removes its index entry atomically.
- The startup purge removes stale/unsearchable entries and is idempotent.
- A rule added after capture removes the matching index entry but preserves the
  clipboard item and its ciphertext.
- An undecodable index row does not prevent later rows from being checked.
- Purge memory is bounded by row and byte caps.

### 5.3 Dedup, tombstones and pinning

- Two concurrent equal ingests produce one live row and both callers receive
  the winner's id.
- A recopy bumps the stored row and moves it to the top.
- Recopy after deletion creates a fresh live row.
- Tombstoning wipes payload bytes, keeps merge identity and removes FTS in one
  transaction.
- Delete-before-create persists a tombstone and prevents resurrection under the
  merge comparator.
- Pin, unpin and reorder are deterministic; unknown ids do not abort the rest.
- TTL and quota sweeps never remove pinned rows.

Stable acceptance IDs used by source comments:

- **D9:** a dedup bump returns and publishes the existing stored row, not the
  rejected candidate.
- **Q10:** a page read skips and counts a row whose authenticated content cannot
  be opened; it never bypasses authentication.

### 5.4 Retention

- Expiry boundary tests cover before, at and after the deadline.
- Low-confidence sensitive findings do not delete an item.
- High-confidence auto-wipe removes only eligible unpinned items and reports the
  count to the event layer.
- A database error in the “has sensitive items” optimization fails closed and
  does not suppress the sweep.
- Quota eviction keeps the newest unpinned row, chooses deterministic victims
  and leaves total removable bytes within policy when possible.
- Cleanup rolls back as one unit on an injected FTS/dependent-state failure.

### 5.5 Pagination and search

- Cursor paging is stable under a concurrent insert above the window.
- Equal timestamps, NULL pin order and transitions between pinned/unpinned runs
  produce no duplicate or missing row.
- Invalid, malformed and foreign cursor tokens fail explicitly.
- Query plans use the intended bounded seek indexes at deep history depths.
- Unicode, hyphenated, quoted, operator-only and adversarial search input never
  panics and returns the intended prefix matches.
- Search never returns tombstones or sensitive rows.

### 5.6 Backup and restore

- Backup refuses overwrite and round-trips current v2 history.
- Wrong-key, corrupt, wrong-schema and failed-integrity candidates do not replace
  the working database.
- An interrupted durable swap yields either the old valid file or the new valid
  file, never a partially copied destination.
- The rebuilt pool observes restored data and no stale pool remains active.
- Sensitive-index purge runs before restored history becomes searchable.

## 6. Load-bearing implementation choices

- Keep one schema constant and one structured verifier.
- Bind row projections by name or generate mapper and projection together.
- Use maintained pooling, migration-free schema creation, temporary-file and
  durable-replacement packages already present in the tree.
- Keep sensitive write/read/purge checks separate because they defend different
  trust boundaries; share predicates and test vectors so they cannot drift.
- Keep keyset cursors opaque. Their representation may change with the order,
  while clients depend only on round-tripping the token.
