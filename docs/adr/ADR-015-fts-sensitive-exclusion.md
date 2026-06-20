# ADR-015: Exclude Sensitive Items from FTS5 Index

## Status

Accepted

Date: 2026-06-21

## Context

CopyPaste stores clipboard history in SQLCipher-encrypted SQLite (`clipboard_items`). A companion
FTS5 virtual table (`clipboard_fts`) indexes the **decrypted plaintext** of each item so that
`search_items` can perform full-text search without decrypting every row on every query.

Both tables reside inside the same SQLCipher database file and are encrypted under the same key at
the page level. However, the FTS index stores **plaintext** at rest (inside the encrypted
container), meaning:

1. Any future SQLCipher vulnerability or key-leak exposes all FTS content in one step, with no
   additional per-field protection.
2. A bug or design oversight allowing `search_items` to return rows from `clipboard_fts` without
   filtering `is_sensitive` would leak secret material (passwords, tokens, API keys, PII) to callers
   that do not expect sensitive results.

Issue **CopyPaste-i6pp** identified exactly this problem: `insert_item_with_fts` and `upsert_fts`
were not guarding against `is_sensitive = 1`, so sensitive items were indexed as plaintext into
`clipboard_fts`, and `search_items` returned them alongside normal results.

### FTS5 at-rest trade-off

FTS5 in SQLite is a virtual table implemented over shadow tables (B-trees, prefix trees, posting
lists). The content of these shadow tables is **always plaintext from the application's perspective**
— the indexing layer sees decrypted strings. SQLCipher encrypts the underlying pages at the file
level, which protects data if the file is stolen while the database is closed. However:

- The FTS shadow tables are not separately keyed or access-controlled within SQLite.
- Any query that can JOIN `clipboard_fts` with `clipboard_items` can reconstruct plaintext, even
  without the encryption key (if the connection is already open).
- FTS5 does not support per-row encryption or masking.

This means that the FTS index is an inherently lower-security store than the main table's
`content BLOB`, which is encrypted by the application layer with XChaCha20-Poly1305 before being
written (see ADR-001 and ADR-003). The `content` column holds ciphertext; `clipboard_fts.content_text`
holds cleartext.

## Decision

**Sensitive items (those with `is_sensitive = 1`) are never written to `clipboard_fts` and are
never returned by `search_items`.**

This is enforced in three places, in order of increasing defense depth:

1. **`insert_item_with_fts`** (primary guard): if `item.is_sensitive == true`, the function skips
   the FTS `INSERT` entirely regardless of the `plaintext_for_fts` argument. Callers should pass
   `""` for sensitive items, but the guard is unconditional so a future caller cannot accidentally
   index secret content.

2. **`upsert_fts`** (secondary guard): before writing to `clipboard_fts`, the function queries
   `clipboard_items` for `is_sensitive`. If the row is sensitive (or missing), it returns `Ok(())`
   without touching the FTS table. This catches callers that call `upsert_fts` independently of
   `insert_item_with_fts` (e.g. post-decryption FTS backfill on sync).

3. **`search_items`** (defense-in-depth filter): the SQL query adds `AND ci.is_sensitive = 0` to
   the JOIN predicate. Even if a stale FTS row exists for a sensitive item (e.g. from a pre-fix
   database or direct SQL injection in a test), the item will never appear in search results.

### Migration v13 — purge existing stale FTS rows

Schema migration v13 (added alongside this fix) executes:

```sql
DELETE FROM clipboard_fts
WHERE id IN (
    SELECT id FROM clipboard_items WHERE is_sensitive = 1
);
```

This is a one-time, idempotent cleanup that removes any FTS rows written by the pre-fix code paths.
The migration runs atomically as part of `apply_migrations` and is guarded by `user_version = 12`.
On a clean database (no sensitive FTS rows) it is a no-op.

## Consequences

**Positive:**
- Secrets (passwords, tokens, PII) are never stored as plaintext in `clipboard_fts`, closing the
  CopyPaste-i6pp information-disclosure vulnerability.
- Three independent enforcement layers (write guard, update guard, query filter) provide defense
  in depth; any one of them alone would prevent the leak.
- Existing affected databases are cleaned up automatically on first open after upgrade.

**Negative / neutral:**
- Sensitive items are not full-text searchable. Users cannot search for a password by a keyword in
  its content. This is intentional: searching for secrets by plaintext keyword would defeat the
  purpose of classifying them as sensitive in the first place.
- `upsert_fts` now performs an extra `SELECT` against `clipboard_items` per call when invoked on a
  sensitive item. This SELECT hits the primary key index and is O(1); the overhead is negligible
  relative to FTS write cost.
- Callers that pass non-empty `plaintext_for_fts` for a sensitive item will see the string silently
  discarded. This is intentional and is documented in the function's doc comment.

**Operational:**
- Sensitive items (password-manager content, credit-card numbers, tokens detected by the
  `copypaste-core::sensitive` module) are only findable via exact `item_id` lookup or the history
  list (`get_page`), not via FTS search. The daemon's TTL-based expiry still applies.

## Alternatives Considered

- **Filter `is_sensitive` in `search_items` only, keep indexing** — rejected. This leaves secret
  plaintext in the FTS shadow tables at rest. A SQL query bypass or a future regression could
  re-expose it. Defense-in-depth prefers not writing the data at all.
- **Per-field encryption inside FTS** — not possible with SQLite FTS5. FTS5 has no hook for
  per-value transformation at the tokenizer or storage layer that would allow encrypting individual
  values before they enter the shadow tables.
- **Separate encrypted FTS table for sensitive items** — over-engineered for the use case. Sensitive
  items are not searched by content by design; there is no feature request for "search my passwords".
- **User opt-in to sensitive search** — rejected on security grounds. An opt-in is a footgun: a
  misconfigured or malicious app can trivially enable it. The policy must be unconditional.
