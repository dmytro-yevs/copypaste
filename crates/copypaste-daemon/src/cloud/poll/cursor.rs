use copypaste_core::Database;

use crate::sync_cursor::CloudCursor;

/// The settings-table key under which the download high-water-mark (max ingested
/// `wall_time`, in Unix ms) is persisted so a restart resumes forward pagination
/// instead of re-downloading the entire cloud history.
const POLL_WATERMARK_KEY: &str = "cloud_poll_watermark";

/// Forward-pagination cursor for the cloud poll loop.
///
/// `wall` is the Unix-ms wall_time of the last row ingested (the persisted
/// high-water-mark). `id` is that row's primary key, the secondary keyset
/// component used to page forward through rows that share the same `wall`
/// millisecond (see [`build_poll_url`]). `id` is empty on a cold start (only the
/// `wall` lower bound is applied) and is populated once a row is ingested.
// PartialEq: the burst-drain loop compares the post-poll cursor against the
// pre-poll snapshot to detect a no-advance stall (full batch but no usable
// keyset progress) and break rather than re-poll the same window forever.
//
// CopyPaste-w47w #3: the struct has moved to `crate::sync_cursor::CloudCursor`;
// `PollCursor` is a type alias kept for all existing call sites.
pub(crate) type PollCursor = CloudCursor;

/// Base poll query (no lower bound). The keyset cursor filter is appended by
/// [`build_poll_url`] when a watermark is known. Order is the **compound**
/// `(wall_time, id)` so pagination is deterministic even within one millisecond.
/// Base poll query string. The `limit=` value MUST match `POLL_BATCH_SIZE`
/// (`super::POLL_BATCH_SIZE`); a compile-time assertion in `poll_once` enforces
/// this.
const POLL_SELECT_QS: &str = "select=id,item_id,content_type,payload_ct,lamport_ts,wall_time,expires_at,app_bundle_id,device_id,deleted,pinned,pin_order&order=wall_time.asc,id.asc&limit=20";

/// Construct the poll URL for a single tick using a `(wall_time, id)` keyset
/// cursor.
///
/// WATERMARK BUG FIX: the previous query used a `wall_time`-only cursor
/// (`order=wall_time.asc&limit=20` + strict `wall_time=gt.<max>`). Because
/// `wall_time` is millisecond granularity, a burst of ≥ `limit` rows sharing the
/// SAME max millisecond was fatal: a tick fetched `limit` of them, advanced the
/// watermark to that millisecond, and the next tick's strict `gt` filtered out
/// the remaining same-millisecond rows FOREVER (silent download data loss).
///
/// The fix is a proper compound keyset cursor `(watermark_wall, watermark_id)`
/// ordered by `(wall_time, id)`: each tick requests rows strictly *after* the
/// `(wall_time, id)` pair of the last row ingested. Expressed in PostgREST:
///
/// ```text
/// or=(wall_time.gt.W, and(wall_time.eq.W, id.gt.ID))
/// ```
///
/// i.e. a later millisecond OR the same millisecond with a larger `id`. This
/// advances forward through same-millisecond rows by `id` instead of stalling,
/// so ≥20 rows sharing one wall_time are all eventually fetched, in order, with
/// no gaps. Forward (`asc`) direction is preserved. `watermark_id` is empty on a
/// fresh start (only a `wall_time` lower bound is used) or for a watermark
/// restored from the persisted `wall_time`-only setting.
pub(crate) fn build_poll_url(
    supabase_url: &str,
    watermark_wall: i64,
    watermark_id: &str,
) -> String {
    let base = format!("{supabase_url}/rest/v1/clipboard_items?{POLL_SELECT_QS}");
    if watermark_wall <= 0 {
        return base;
    }
    if watermark_id.is_empty() {
        // No id component yet (cold start from a persisted wall_time-only
        // watermark): use an inclusive `gte` so the boundary millisecond's rows
        // are (re-)offered; the per-row item_id dedup drops already-ingested
        // ones. Once a row is ingested the id component is populated and the
        // strict keyset below takes over.
        return format!("{base}&wall_time=gte.{watermark_wall}");
    }
    // Strict `(wall_time, id)` keyset: a later ms, OR the same ms with a larger
    // id. URL-encode the parens-bearing PostgREST `or=` expression's commas are
    // significant; reqwest will percent-encode the whole query value for us when
    // we pass it through the URL, but we build the canonical PostgREST syntax
    // here (matching the existing hand-built query strings in this module).
    format!(
        "{base}&or=(wall_time.gt.{watermark_wall},and(wall_time.eq.{watermark_wall},id.gt.{watermark_id}))"
    )
}

/// Seed the download watermark on startup from the larger of the persisted
/// `cloud_poll_watermark` setting and the local `MAX(wall_time)`. Either source
/// missing/unreadable contributes `0` (download from the beginning). Never
/// errors — a fresh DB or absent setting simply yields `0`.
pub(crate) fn load_poll_watermark(db: &Database) -> i64 {
    let persisted: i64 = db
        .conn()
        .query_row(
            "SELECT value FROM settings WHERE key = ?1",
            rusqlite::params![POLL_WATERMARK_KEY],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0);
    let local_max: i64 = db
        .conn()
        .query_row(
            "SELECT COALESCE(MAX(wall_time), 0) FROM clipboard_items",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);
    persisted.max(local_max)
}

/// Persist the download watermark into the `settings` table (upsert). Returns the
/// rusqlite error on failure so the caller can log it; the watermark also lives
/// in memory, so a persist failure only costs re-pagination after a restart.
pub(crate) fn save_poll_watermark(db: &Database, watermark: i64) -> rusqlite::Result<()> {
    db.conn().execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2) \
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        rusqlite::params![POLL_WATERMARK_KEY, watermark.to_string()],
    )?;
    Ok(())
}
