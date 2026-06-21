// ── Poison-row guard (CopyPaste-jww / CopyPaste-5y4) ─────────────────────────

use copypaste_core::Database;
use copypaste_sync::protocol::WireItem;
use tracing::warn;

/// Returns `true` when a [`WireItem`] would become a poison row if stored
/// verbatim — i.e. when `rekey_inbound` failed because the shared sync key is
/// missing or wrong and the item was sync-key-wrapped.
///
/// A sync-key-wrapped item has `content` (the wrapped blob) but the sender
/// strips `content_nonce` (which is the "no local-nonce" sentinel on the wire)
/// and for file/image items also strips `blob_ref`. Storing such an item means
/// consumers will see a row with ciphertext they cannot decrypt AND no nonce /
/// no blob reference — causing "missing content_nonce" or "missing blob_ref
/// metadata" errors on every read.
///
/// The check is intentionally conservative:
/// * `text` is poison when `content` is present and `content_nonce` is absent.
/// * `file` / `image` are poison when `content` is present, `content_nonce` is
///   absent, AND `blob_ref` is also absent.  A file item that arrived via the
///   large-blob path carries `blob_ref` even without a nonce — that is a
///   legitimate row and must not be discarded.
pub fn is_poison_wire(w: &WireItem) -> bool {
    if w.content.is_none() {
        // No ciphertext at all (tombstone or empty) — not a poison row.
        return false;
    }
    match w.content_type.as_str() {
        "text" => w.content_nonce.is_none(),
        "file" | "image" => w.content_nonce.is_none() && w.blob_ref.is_none(),
        // Unknown content types: be conservative, do not treat as poison.
        _ => false,
    }
}

/// Delete all poison rows from `clipboard_items` and return the count removed.
///
/// A poison row is any row that was stored verbatim from a sync-key-wrapped
/// wire item (i.e. `rekey_inbound` failed) and therefore lacks the fields
/// consumers need to decrypt it:
/// * `content_type = 'text'` with `content IS NOT NULL` and `content_nonce IS NULL`
/// * `content_type IN ('file', 'image')` with `content IS NOT NULL`,
///   `content_nonce IS NULL`, and `blob_ref IS NULL`
///
/// Safe to call at startup on every restart — idempotent.  The affected peers
/// will re-send the items on their next catch-up cycle (sync is idempotent).
///
/// Returns `Err` only on SQLite failures; a zero-row result is `Ok(0)`.
pub fn sweep_poison_rows(db: &Database) -> Result<usize, anyhow::Error> {
    // e5oe: collect ids of poison rows before deleting so we can remove the
    // matching clipboard_fts rows in the same transaction — otherwise orphaned
    // FTS content_text rows accumulate and remain searchable as plaintext.
    const POISON_WHERE: &str = "(content_type = 'text' \
                AND content IS NOT NULL \
                AND content_nonce IS NULL) \
            OR (content_type IN ('file', 'image') \
                AND content IS NOT NULL \
                AND content_nonce IS NULL \
                AND blob_ref IS NULL)";
    let conn = db.conn();
    let tx = conn.unchecked_transaction()?;
    let ids: Vec<String> = {
        let mut stmt = tx.prepare(&format!(
            "SELECT id FROM clipboard_items WHERE {POISON_WHERE}"
        ))?;
        let rows = stmt.query_map([], |r| r.get(0))?;
        rows.collect::<Result<_, _>>()?
    };
    if ids.is_empty() {
        return Ok(0);
    }
    // Delete the clipboard_items rows.
    let n = tx.execute(
        &format!("DELETE FROM clipboard_items WHERE {POISON_WHERE}"),
        [],
    )?;
    // Delete matching FTS rows in the same transaction (e5oe fix).
    let placeholders = std::iter::repeat_n("?", ids.len())
        .collect::<Vec<_>>()
        .join(",");
    tx.execute(
        &format!("DELETE FROM clipboard_fts WHERE id IN ({placeholders})"),
        rusqlite::params_from_iter(ids.iter()),
    )?;
    tx.commit()?;
    if n > 0 {
        warn!(
            swept = n,
            "sync_orch: swept {n} poison row(s) \
             (sync-key-wrapped items stored without content_nonce/blob_ref \
             — peers will re-send on next connect) (CopyPaste-jww/5y4)"
        );
    }
    Ok(n)
}
