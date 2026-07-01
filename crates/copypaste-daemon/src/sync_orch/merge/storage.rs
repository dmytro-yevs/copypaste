//! Atomic replace-by-`item_id` storage primitive for the sync merge path.
//!
//! Split out of the former flat `sync_orch/merge.rs` (ADR-017,
//! CopyPaste-vp63.49) — moved verbatim, no behavior change.

use copypaste_core::{ClipboardItem, Database, MigrationState};

/// Atomically replace (or insert) a clipboard row and its FTS index for the
/// sync merge path (sync M1).
///
/// Runs DELETE (when `existed`) + INSERT + FTS rewrite inside one
/// `unchecked_transaction`, so a failed insert rolls the whole thing back and
/// the prior row survives intact. Unlike `insert_item` / `insert_item_with_fts`
/// in core (plain INSERT, dedup-on-conflict), this path is a true replace keyed
/// on the cross-device `item_id` (the CRDT identity), which is what LWW
/// `TakeRemote` requires. The caller preserves the existing local row's primary
/// key on `item.id`, so the DELETE-by-item_id + INSERT keeps the same `id` and
/// the FTS rewrite below (keyed on `item.id`) stays consistent.
///
/// `fts_text` is the already-decrypted plaintext to index; `None`/empty skips
/// FTS (e.g. verbatim or image rows). The stored `key_version` is taken from
/// `item.key_version` rather than hardcoded to ITEM_KEY_VERSION_CURRENT so that
/// a verbatim (non-rewrapped) incoming row with key_version=1 is stored as v1
/// and can be decrypted by the existing v1 path, instead of being stamped v2
/// (which would make it permanently undecryptable — auth-tag mismatch).
pub(super) fn replace_item_atomic(
    db: &Database,
    existed: bool,
    item: &ClipboardItem,
    fts_text: Option<&str>,
) -> anyhow::Result<()> {
    use rusqlite::params;

    // Honour the same write gate the core `insert_item` enforces: while the v4
    // key-version sweep is running, reject writes so a key_version=2 row can't
    // corrupt the cursor-based resume.
    if matches!(db.migration_state()?, MigrationState::InProgress { .. }) {
        anyhow::bail!("sync_orch: refusing write while v4 migration is in progress");
    }

    let tx = db.conn().unchecked_transaction()?;
    if existed {
        // Delete the prior version by its cross-device `item_id` (the row's
        // local PK is preserved on `item.id`, so the subsequent INSERT reuses
        // the same `id`). Deleting by `item_id` also defends the UNIQUE
        // `idx_clipboard_item_id` index from a conflict on re-insert.
        tx.execute(
            "DELETE FROM clipboard_items WHERE item_id = ?1",
            params![item.item_id],
        )?;
    }
    tx.execute(
        "INSERT INTO clipboard_items
         (id, item_id, content_type, content, content_nonce, blob_ref,
          is_sensitive, is_synced, lamport_ts, wall_time, expires_at, app_bundle_id,
          content_hash, origin_device_id, key_version, pinned, pin_order, deleted)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18)",
        params![
            item.id,
            item.item_id,
            item.content_type,
            item.content,
            item.content_nonce,
            item.blob_ref,
            item.is_sensitive as i64,
            item.is_synced as i64,
            item.lamport_ts,
            item.wall_time,
            item.expires_at,
            item.app_bundle_id,
            item.content_hash,
            item.origin_device_id,
            // Use item.key_version (set by rekey_inbound=2 or wire_to_local=wire.key_version)
            // rather than the hardcoded ITEM_KEY_VERSION_CURRENT. A verbatim legacy
            // key_version=1 row would be stamped v2 here but its ciphertext is still
            // v1-encrypted → permanent auth-tag failure on every subsequent decrypt.
            item.key_version as i64,
            item.pinned as i64,
            // pin_order: the wire now carries pin_order directly via wire_to_local,
            // so this correctly reflects the sender's pinned ordering.
            item.pin_order,
            // deleted: wire_to_local propagates this from the WireItem; for
            // non-tombstone items this is always false (tombstones are handled
            // by the soft_delete_item fast-path above and never reach here).
            item.deleted as i64,
        ],
    )?;
    if let Some(text) = fts_text {
        if !text.is_empty() {
            tx.execute("DELETE FROM clipboard_fts WHERE id = ?1", params![item.id])?;
            tx.execute(
                "INSERT INTO clipboard_fts(id, content_text) VALUES (?1, ?2)",
                params![item.id, text],
            )?;
        }
    }
    tx.commit()?;
    Ok(())
}
