use anyhow::Context as _; // CopyPaste-crh3.90
use copypaste_core::{ClipboardItem, Database};

// ── Helper: exists_item ───────────────────────────────────────────────────────

/// Return `true` when a row with the given `id` already exists locally.
pub fn exists_item(db: &Database, id: &str) -> Result<bool, anyhow::Error> {
    let count: i64 = db
        .conn()
        .query_row(
            "SELECT COUNT(1) FROM clipboard_items WHERE id = ?1",
            rusqlite::params![id],
            |row| row.get(0),
        )
        .context("exists_item query")?;
    Ok(count > 0)
}

// ── JSON serialisation helpers ────────────────────────────────────────────────

/// Convert a [`ClipboardItem`] to the JSON shape expected by the Supabase REST
/// API, embedding the cloud-re-encrypted payload as `payload_ct` (base64).
///
/// Column mapping (matches `docs/supabase/schema.sql`):
///   * `id`               — item UUID (PK)
///   * `item_id`          — stable item identity UUID
///   * `content_type`     — "text" | "image" | ...
///   * `payload_ct`       — base64(nonce\[24\]||ciphertext) from `encrypt_for_cloud`
///   * `lamport_ts`       — LWW clock
///   * `wall_time`        — Unix ms
///   * `expires_at`       — TTL (nullable)
///   * `app_bundle_id`    — origin app (nullable)
///   * `device_id`        — maps to `origin_device_id`
///   * `deleted`          — soft-delete tombstone flag; false for live items.
///     When true the receiving device must call delete_item rather than
///     inserting/updating. Tombstone rows still carry the item_id so the
///     receiver can locate the row.
///   * `pinned`           — whether the item is explicitly pinned on the source device.
///   * `pin_order`        — drag-to-reorder sort key for pinned items (nullable).
///
/// `user_id` is intentionally omitted — the default `auth.uid()` on the
/// column fills it in automatically, and the RLS `with check` enforces it.
/// Build the PostgREST upsert JSON body for a clipboard item.
///
/// `payload_ct_b64` is `Some(base64_ciphertext)` for live items and `None`
/// for tombstones (`item.deleted == true`). Tombstone rows set `deleted: true`
/// and send `payload_ct: null` so the server stores NULL (no ciphertext leak)
/// and receiving devices know to apply a soft-delete.
///
/// # CopyPaste-e89n
///
/// The previous implementation hardcoded `"deleted": false` because
/// `ClipboardItem` was not yet expected to carry a `deleted` field. Now that
/// `soft_delete_item` materialises tombstone rows locally (schema v10) and
/// resets `is_synced = 0`, the backlog sweep can feed tombstones into this path.
pub(super) fn clipboard_item_to_json(
    item: &ClipboardItem,
    payload_ct_b64: Option<&str>,
) -> serde_json::Value {
    // CLOUD-ROUNDTRIP fix: `payload_ct` is a Postgres `bytea` column. PostgREST
    // accepts a string assigned to a bytea column in Postgres' INPUT formats —
    // a bare base64 string is NOT one of them (it is stored as the literal
    // ASCII bytes of the base64 text), and PostgREST then returns bytea on read
    // in HEX output form (`\x..`), so the poll path's base64-decode failed and
    // cloud DOWNLOAD never worked. We therefore send the canonical hex input
    // form `\x<hex>` so the column holds the true ciphertext bytes and the
    // read-back round-trips. See `decode_payload_ct` for the symmetric read.
    //
    // Tombstones (deleted = true) send null for payload_ct so the server stores
    // NULL — no ciphertext to leak — and receiving devices know to call delete_item.
    let payload_ct_val: serde_json::Value = match payload_ct_b64 {
        Some(b64) => serde_json::Value::String(encode_payload_ct_hex(b64)),
        None => serde_json::Value::Null,
    };
    serde_json::json!({
        "id":            item.id,
        "item_id":       item.item_id,
        "content_type":  item.content_type,
        "payload_ct":    payload_ct_val,
        "lamport_ts":    item.lamport_ts,
        "wall_time":     item.wall_time,
        "expires_at":    item.expires_at,
        "app_bundle_id": item.app_bundle_id,
        "device_id":     item.origin_device_id,
        // CopyPaste-e89n: use the item's actual deleted flag so tombstone rows
        // (soft_delete_item sets deleted=1, is_synced=0) propagate to the cloud.
        "deleted":       item.deleted,
        // Pin state: propagate so a pin/unpin on one device is reflected on
        // every other device after the next cloud sync round.
        "pinned":        item.pinned,
        "pin_order":     item.pin_order,
    })
}

/// Encode the base64 cloud ciphertext as a Postgres `bytea` hex-input literal
/// (`\x<hex>`) so PostgREST stores the *true* ciphertext bytes (not the ASCII
/// of the base64 text). Returns the original string unchanged if it is not
/// valid base64 (defensive — should not happen for `encrypt_for_cloud` output).
pub(crate) fn encode_payload_ct_hex(payload_ct_b64: &str) -> String {
    use base64::Engine as _;
    match base64::engine::general_purpose::STANDARD.decode(payload_ct_b64) {
        Ok(bytes) => format!("\\x{}", hex::encode(bytes)),
        Err(_) => payload_ct_b64.to_owned(),
    }
}
