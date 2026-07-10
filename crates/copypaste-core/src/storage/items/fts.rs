use super::super::db::Database;
use super::super::pool::DbRead;
use super::query::ITEM_SELECT_COLUMNS_CI;
use super::types::{row_to_item, ClipboardItem, ItemsError};
use rusqlite::{params, OptionalExtension};
use sha2::{Digest, Sha256};

/// Maximum byte length of a text preview returned by [`fetch_text_preview`].
///
/// The UI history list renders one row per item. Sending more than 1 KiB per
/// row for a potentially-long list locks the UI rendering thread on large
/// clipboard entries. Full content is still stored encrypted; only the preview
/// is capped here. A proper rich-preview panel is planned for v0.4.
pub const MAX_PREVIEW_BYTES: usize = 1_024;

/// Compute the SHA-256 content hash of raw (pre-encryption) clipboard bytes.
///
/// Returns the **full** 64-character lowercase hexadecimal encoding of the
/// 32-byte SHA-256 digest.
///
/// # Why the full digest (CopyPaste-y4v1)
///
/// The original implementation in `copypaste-daemon::clipboard::image_content_hash`
/// truncated the SHA-256 output to its first 16 bytes (a 128-bit fingerprint),
/// producing a 32-character hex string. While 128 bits is collision-resistant for
/// most practical purposes, the truncation:
///   * weakens second-preimage resistance unnecessarily (the daemon already pays
///     the full SHA-256 computation cost; dropping half the bits is pure loss);
///   * risks silent cross-content collisions in a large history — a 1-in-2^128
///     collision probability per pair is acceptable, but adversarial or
///     corpus-sensitive inputs could be crafted to collide at 16 bytes while
///     not colliding at 32 bytes.
///
/// This canonical function always returns the full 256-bit (64 hex char) hash.
/// The daemon should migrate `image_content_hash` to call this instead.
pub fn compute_content_hash(raw: &[u8]) -> String {
    let digest = Sha256::digest(raw);
    hex::encode(digest)
}

/// Fetch a clamped plaintext preview for `id` from the FTS5 index.
///
/// Returns `Some(text)` for text items that have an FTS entry, where `text`
/// is at most [`MAX_PREVIEW_BYTES`] bytes long (truncated at a UTF-8 char
/// boundary with an ellipsis appended when clamped).
///
/// Returns `None` when no FTS entry exists for the given id (image items or
/// pre-FTS rows). Callers should render an appropriate placeholder in that
/// case (e.g. `"[image — id:XXXXXXXX]"`).
pub fn fetch_text_preview<D: DbRead + ?Sized>(
    db: &D,
    id: &str,
) -> Result<Option<String>, ItemsError> {
    let result: Option<String> = db
        .conn()
        .query_row(
            "SELECT content_text FROM clipboard_fts WHERE id = ?1 LIMIT 1",
            params![id],
            |row| row.get(0),
        )
        .optional()
        .map_err(ItemsError::Sqlite)?;

    Ok(result.map(|text| clamp_preview(text, MAX_PREVIEW_BYTES)))
}

/// Batch variant of [`fetch_text_preview`]: fetch clamped previews for many ids
/// in a **single** `SELECT ... WHERE id IN (...)` round-trip instead of one
/// query per id.
///
/// `history_page` renders up to [`crate::storage`]'s page size of text items;
/// the per-item `fetch_text_preview` previously fired one SQL round-trip each
/// (a 50-item page = 51 round-trips). This collapses the preview fetch to one
/// statement and returns a `id → clamped preview` map. Ids with no FTS row are
/// simply absent from the map (callers render the usual placeholder).
///
/// Returns an empty map when `ids` is empty (no SQL issued).
pub fn fetch_text_previews_batch<D: DbRead + ?Sized>(
    db: &D,
    ids: &[&str],
) -> Result<std::collections::HashMap<String, String>, ItemsError> {
    if ids.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    // Build a `?,?,…` placeholder list sized to `ids`. Each id is bound as a
    // parameter (never interpolated), so this is injection-safe even though the
    // placeholder count is dynamic.
    let placeholders = super::sql_placeholders(ids.len());
    let sql = format!("SELECT id, content_text FROM clipboard_fts WHERE id IN ({placeholders})");
    let conn = db.conn();
    let mut stmt = conn.prepare(&sql)?;
    let params = rusqlite::params_from_iter(ids.iter());
    let rows = stmt.query_map(params, |row| {
        let id: String = row.get(0)?;
        let text: String = row.get(1)?;
        Ok((id, text))
    })?;
    let mut map = std::collections::HashMap::with_capacity(ids.len());
    for row in rows {
        let (id, text) = row.map_err(ItemsError::Sqlite)?;
        map.insert(id, clamp_preview(text, MAX_PREVIEW_BYTES));
    }
    Ok(map)
}

/// Clamp `text` to at most `max_bytes` bytes, truncating at a UTF-8 character
/// boundary and appending `…` when truncation occurs.
pub(crate) fn clamp_preview(text: String, max_bytes: usize) -> String {
    if text.len() <= max_bytes {
        return text;
    }
    // Walk back from max_bytes to find a valid UTF-8 char boundary.
    let boundary = (0..=max_bytes)
        .rev()
        .find(|&i| text.is_char_boundary(i))
        .unwrap_or(0);
    format!("{}…", &text[..boundary])
}

/// Insert or replace a plaintext snippet into the FTS5 index.
/// `plaintext` must already be decrypted by the caller.
/// Call this once per item after `insert_item`.
///
/// FTS5 does not support `ON CONFLICT`, so the canonical upsert pattern is a
/// DELETE followed by INSERT. The two writes are wrapped in a single transaction
/// so a crash between them cannot leave the item permanently unsearchable
/// (CopyPaste-j9pv): either both succeed (the FTS entry is updated) or neither
/// does (the old FTS row survives, a stale-text miss at worst, not a
/// permanently-missing row).
///
/// CopyPaste-i6pp: **sensitive items are never indexed**. If `is_sensitive = 1`
/// in `clipboard_items` for `id`, this function is a no-op and returns `Ok(())`.
/// This is the second enforcement layer: callers should not pass sensitive
/// plaintext at all, but this guard ensures a future caller cannot accidentally
/// put secrets into the FTS table.
pub fn upsert_fts(db: &Database, id: &str, plaintext: &str) -> Result<(), ItemsError> {
    let conn = db.conn();

    // CopyPaste-44rq.64 / CopyPaste-i6pp: the is_sensitive check MUST be
    // inside the same transaction as the FTS write to close the TOCTOU window
    // where a concurrent UPDATE could flip is_sensitive=1 between the SELECT
    // and the INSERT, letting sensitive plaintext reach the FTS index.
    //
    // `unchecked_transaction` matches the storage-layer convention: the daemon
    // holds the Database behind a Mutex and only hands out `&Connection`, so
    // there is no concurrent borrow to guard against at the Rust level; the
    // SQLite write lock acquired by the transaction serialises any concurrent
    // writer at the DB level.
    let tx = conn.unchecked_transaction()?;

    // Re-read is_sensitive INSIDE the transaction so the sensitivity check and
    // the FTS write are atomic under the same write lock.
    let is_sensitive: Option<i64> = tx
        .query_row(
            "SELECT is_sensitive FROM clipboard_items WHERE id = ?1",
            params![id],
            |r| r.get(0),
        )
        .optional()?;
    match is_sensitive {
        Some(1) => return Ok(()), // sensitive — do not index (tx auto-rolled-back on drop)
        None => return Ok(()),    // row not found — nothing to index
        _ => {}                   // non-sensitive — proceed
    }

    tx.execute("DELETE FROM clipboard_fts WHERE id = ?1", params![id])?;
    tx.execute(
        "INSERT INTO clipboard_fts(id, content_text) VALUES (?1, ?2)",
        params![id, plaintext],
    )?;
    tx.commit()?;
    Ok(())
}

/// Fetch a map of device UUID → device name from the `devices` table.
///
/// Used by `history_page` to resolve `origin_device_id` to a human-readable
/// name without requiring a per-item JOIN on every history query.  The map
/// is built once per page request; unknown device UUIDs (items captured
/// before the peer was paired, or orphaned rows) map to `None` at the call
/// site rather than appearing here.
///
/// Returns an empty map when the `devices` table is empty or when no paired
/// devices exist yet.
pub fn get_device_names<D: DbRead + ?Sized>(
    db: &D,
) -> Result<std::collections::HashMap<String, String>, ItemsError> {
    let mut stmt = db.conn().prepare("SELECT id, name FROM devices")?;
    let pairs = stmt
        .query_map([], |row| {
            let id: String = row.get(0)?;
            let name: String = row.get(1)?;
            Ok((id, name))
        })?
        .collect::<Result<std::collections::HashMap<_, _>, _>>()?;
    Ok(pairs)
}

/// Sanitize a user-supplied FTS5 query string, keeping only characters
/// that are safe to pass through the FTS5 MATCH operator:
///
/// Allowed:
///   - Unicode letters and digits (covers ASCII + Cyrillic, CJK, etc.)
///   - `_` and `-` (word-separator conventions)
///   - `"` (phrase-query delimiters, e.g. `"bar baz"`)
///   - `*` (explicit prefix operator)
///   - ASCII space
///
/// Stripped (FTS5 structural operators and SQL special chars):
///   - `:` (column filter, e.g. `col:term`)
///   - `^` (initial-token anchor)
///   - `;`, `'`, `\`, `\0` and other chars with no legitimate FTS use
///
/// Since the sanitized string is passed as a bound parameter (not
/// interpolated into SQL), SQL injection via MATCH is not possible even
/// Sanitize a raw user query into a safe FTS5 MATCH expression (S8 whitelist tokenizer).
///
/// Strategy:
/// - Strip every character that is not alphanumeric, `_`, `-`, `"`, `*`, or whitespace.
/// - If the cleaned query contains a quoted phrase (starts with `"` and ends with `"`),
///   pass it through as-is (FTS5 phrase queries are safe once other operators are stripped).
/// - Otherwise split on whitespace into individual tokens, discard empty tokens, join with
///   ` AND ` so all terms must appear, and append `*` to EVERY token for prefix search
///   (CopyPaste-8ebg.57: search-as-you-type means any token, not just the last, may
///   still be mid-word).
/// - Return `None` if no valid tokens remain after filtering (caller returns empty results).
///
/// This is a whitelist approach: only known-safe characters pass through, preventing
/// FTS5 operator injection (e.g. `NOT`, `OR`, `NEAR`, column filters).
pub(crate) fn sanitize_fts5_query(raw: &str) -> Option<String> {
    // Keep only alphanum, underscore, quote, asterisk, and whitespace.
    //
    // `-` (hyphen/minus) is an FTS5 operator: in a MATCH expression `foo -bar`
    // means "foo AND NOT column bar", so a hyphen-joined token like `foo-bar*`
    // makes FTS5 parse `-bar` as a column filter and error with
    // "no such column: bar". We therefore REWRITE `-` to whitespace (rather than
    // keeping or stripping it) so `foo-bar` splits into two AND-ed terms
    // (`foo* AND bar*`) before any per-token `*` prefix logic runs, and no raw
    // `-` ever reaches the MATCH operator.
    let cleaned: String = raw
        .chars()
        .map(|c| if c == '-' { ' ' } else { c })
        .filter(|c| c.is_alphanumeric() || matches!(c, '_' | '"' | '*' | ' ' | '\t'))
        .collect();

    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        return None;
    }

    // Fix 5: count double-quotes; if the count is odd the phrase is unclosed and
    // FTS5 will return a syntax error.  Strip all double-quotes in that case so
    // the query degrades to a plain token search rather than an SQL error.
    let quote_count = trimmed.chars().filter(|&c| c == '"').count();
    let balanced = if quote_count % 2 == 0 {
        trimmed.to_string()
    } else {
        // Odd number of quotes — remove all quotes to avoid an unclosed FTS5 phrase.
        trimmed.chars().filter(|&c| c != '"').collect()
    };
    let trimmed = balanced.trim();
    if trimmed.is_empty() {
        return None;
    }

    // Pass through quoted phrases and explicit prefix queries unchanged.
    // A quoted phrase looks like `"foo bar"` — starts and ends with a double-quote.
    if (trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() > 1)
        || trimmed.ends_with('*')
    {
        return Some(trimmed.to_string());
    }

    // Multi-word input: split into tokens, strip FTS5 reserved keywords
    // (NOT, OR, AND, NEAR) case-insensitively so a query like "secret NOT test"
    // degrades to a valid MATCH instead of an FTS5 operator-syntax error.
    let tokens: Vec<&str> = trimmed
        .split_whitespace()
        // CopyPaste-pbre: compare case-insensitively WITHOUT allocating a new
        // uppercased String per token (the old `t.to_ascii_uppercase()` heap-
        // allocated on every token just to feed a 4-way match).
        .filter(|t| {
            !["NOT", "OR", "AND", "NEAR"]
                .iter()
                .any(|kw| t.eq_ignore_ascii_case(kw))
        })
        .collect();
    // All tokens may have been stripped (e.g. query was "NOT AND") — return None
    // so the caller returns empty results rather than panicking on len()-1.
    if tokens.is_empty() {
        return None;
    }
    // CopyPaste-8ebg.57: append the prefix `*` to EVERY token, not just the
    // last one. This is a search-as-you-type box — the user is mid-word on
    // every token while typing a multi-word query (e.g. "priv keys" while
    // still typing toward "private keychain"), not just the final one. The
    // previous last-token-only behavior meant earlier tokens required an
    // exact whole-word match, so a query like "priv key" (both partial) found
    // nothing even though "private keychain" exists — only "priv keychain" or
    // similar (first token complete) would have matched.
    let parts: Vec<String> = tokens.iter().map(|tok| format!("{tok}*")).collect();

    Some(parts.join(" AND "))
}

/// Search clipboard items by full-text query.
///
/// Returns up to `limit` full `ClipboardItem` rows ordered by FTS5 rank (best match first).
///
/// Implementation: single SQL JOIN between `clipboard_fts` and `clipboard_items` — eliminates
/// the previous two-phase N+1 fetch (FTS ID list → dynamic IN-list → Rust re-sort).
/// `prepare_cached` reuses the compiled statement across repeated calls on the same connection.
///
/// The query is sanitized via `sanitize_fts5_query` (S8 whitelist tokenizer) before being
/// passed to the FTS5 MATCH operator to prevent operator injection.
pub fn search_items<D: DbRead + ?Sized>(
    db: &D,
    query: &str,
    limit: usize,
) -> Result<Vec<ClipboardItem>, ItemsError> {
    search_items_filtered(db, query, limit, None)
}

/// Full-text search over clipboard items with an optional content-type filter.
///
/// Identical to [`search_items`] but accepts an optional `content_type` filter
/// so callers can restrict results to `"text"`, `"image"`, or `"file"` items.
/// When `content_type_filter` is `None` all types are returned (same as
/// [`search_items`]).
///
/// CopyPaste-tteo: adds the type filter that was previously absent from the
/// search surface, enabling the CLI `--kind` flag and consistent daemon
/// search results without breaking existing callers (which keep using
/// `search_items` with no filter).
pub fn search_items_filtered<D: DbRead + ?Sized>(
    db: &D,
    query: &str,
    limit: usize,
    content_type_filter: Option<&str>,
) -> Result<Vec<ClipboardItem>, ItemsError> {
    if query.trim().is_empty() {
        return Ok(vec![]);
    }

    let safe_query = match sanitize_fts5_query(query) {
        Some(q) => q,
        None => return Ok(vec![]),
    };

    // Fix 6: clamp before cast to avoid negative LIMIT in SQLite.
    let limit_i64 = limit.min(i64::MAX as usize) as i64;

    // Single JOIN: FTS5 drives rank order; clipboard_items supplies full row data.
    // `fts.id` is the UNINDEXED text UUID column (matches `clipboard_items.id`).
    //
    // CopyPaste-i6pp (defense-in-depth): `AND ci.is_sensitive = 0` ensures that
    // even if a stale FTS row exists for a sensitive item (written before this
    // fix, or via a direct `INSERT INTO clipboard_fts` in tests/tooling), the
    // item is never surfaced by search. The primary guard is in
    // `insert_item_with_fts` and `upsert_fts`, which refuse to write sensitive
    // rows into clipboard_fts at all. This filter is the last line of defence.
    //
    // CopyPaste-tteo: branch on optional content_type filter. We use two static
    // SQL strings (one with the extra WHERE clause, one without) rather than
    // building a dynamic query to stay injection-safe and to keep the
    // `prepare_cached` cache key stable for each branch.
    // CopyPaste-tteo: branch on optional content_type filter. We use two static
    // SQL strings (one with the extra WHERE clause, one without) rather than
    // building a dynamic query to stay injection-safe and to keep the
    // `prepare_cached` cache key stable for each branch.
    //
    // Lifetime note: `prepare_cached` borrows `conn()` and the `MappedRows`
    // iterator borrows `stmt`, so we must collect inside the same block before
    // `stmt` (and the borrowed connection) are dropped. We bind a named
    // variable `rows` in each arm — NOT at the `if` expression level — to give
    // the borrow checker enough scope information.
    let rows: Vec<ClipboardItem> = if let Some(ct) = content_type_filter {
        let conn = db.conn();
        let mut stmt = conn.prepare_cached(&format!(
            "SELECT {ITEM_SELECT_COLUMNS_CI} \
                 FROM clipboard_fts fts
             JOIN clipboard_items ci ON ci.id = fts.id
             WHERE clipboard_fts MATCH ?1
               AND ci.deleted = 0
               AND ci.is_sensitive = 0
               AND ci.content_type = ?3
             ORDER BY rank
             LIMIT ?2"
        ))?;
        let r: Vec<ClipboardItem> = stmt
            .query_map(params![safe_query, limit_i64, ct], row_to_item)?
            .collect::<Result<Vec<_>, _>>()?;
        r
    } else {
        let conn = db.conn();
        let mut stmt = conn.prepare_cached(&format!(
            "SELECT {ITEM_SELECT_COLUMNS_CI} \
                 FROM clipboard_fts fts
             JOIN clipboard_items ci ON ci.id = fts.id
             WHERE clipboard_fts MATCH ?1 AND ci.deleted = 0 AND ci.is_sensitive = 0
             ORDER BY rank
             LIMIT ?2"
        ))?;
        let r: Vec<ClipboardItem> = stmt
            .query_map(params![safe_query, limit_i64], row_to_item)?
            .collect::<Result<Vec<_>, _>>()?;
        r
    };

    Ok(rows)
}
