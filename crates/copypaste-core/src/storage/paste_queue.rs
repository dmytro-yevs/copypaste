//! The device-local paste queue: ordered entries that reference clips or hold
//! sealed snapshot text, their status, and a revision that moves on every
//! change.
//!
//! Nothing here syncs. A pending entry keeps its clip out of retention
//! eviction (`retention`). Snapshot and override text are sealed with the item
//! key and bound to the entry id, so a ciphertext cannot move to another
//! entry. Positions are reals: an add takes the next value at either end and a
//! reorder renumbers the list.

use copypaste_ipc::paste_queue::{QueueAddedVia, QueueContentKind, QueueEntryStatus};
use copypaste_ipc::ItemAnnotations;
use rusqlite::types::Value;
use rusqlite::{params, params_from_iter, Connection, OptionalExtension, Row};
/// Snapshot, override and delivered text is wiped when dropped.
pub use zeroize::Zeroizing;

use super::connection::write_tx;
use super::local_state::{self, unexpired};
use super::model::{row_to_item, ItemColumns, StoreError, StoredItem};
use super::Store;
use crate::history_item::StaticHistoryPreview;
use crate::{decrypt, encrypt, ClipboardPayload, ItemKey, PreviewKey};

const SNAPSHOT_AAD: &str = "paste-queue-snapshot:";
const OVERRIDE_AAD: &str = "paste-queue-override:";

/// Every column, in schema order, so a cleared queue can be put back as is.
const COLUMNS: &str = "entry_id, position, source_kind, clip_id, snapshot_ciphertext, \
    snapshot_nonce, override_ciphertext, override_nonce, content_kind, status, added_at, \
    added_via, delivered_at, rendered_at";
const COLUMN_COUNT: usize = 14;

const ORDER: &str = "ORDER BY position, added_at, entry_id";

/// Whether an entry's clip is still there to deliver. A snapshot always is.
const AVAILABLE: &str = concat!(
    "CASE WHEN q.clip_id IS NULL THEN 1 ELSE EXISTS(SELECT 1 FROM clipboard_items ci \
     WHERE ci.id = q.clip_id AND ci.deleted = 0 AND ",
    unexpired!("ci."),
    ") END"
);

/// What an entry holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueSourceKind {
    /// A reference to a history clip.
    Clip,
    /// Text rendered from a snippet when it was added.
    Snippet,
    /// A rendered concealed snippet: masked and written as secret.
    ConcealedSnippet,
}

impl QueueSourceKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Clip => "clip",
            Self::Snippet => "snippet",
            Self::ConcealedSnippet => "concealed_snippet",
        }
    }

    fn parse(value: &str) -> Self {
        match value {
            "snippet" => Self::Snippet,
            "concealed_snippet" => Self::ConcealedSnippet,
            _ => Self::Clip,
        }
    }
}

/// One entry to add.
#[derive(Debug, Clone)]
pub struct NewQueueEntry {
    pub source: QueueSourceKind,
    pub clip_id: Option<String>,
    pub snapshot: Option<Zeroizing<String>>,
    pub content_kind: QueueContentKind,
    pub added_via: QueueAddedVia,
    pub rendered_at: Option<i64>,
}

/// One stored entry, without its text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueEntryRow {
    pub entry_id: String,
    pub source: QueueSourceKind,
    pub clip_id: Option<String>,
    pub content_kind: QueueContentKind,
    pub status: QueueEntryStatus,
    pub added_at: i64,
    pub added_via: QueueAddedVia,
    pub delivered_at: Option<i64>,
    pub rendered_at: Option<i64>,
    /// A local override replaces the content.
    pub edited: bool,
    /// The referenced clip is live and unexpired; always true for a snapshot.
    pub available: bool,
}

/// One entry as listed: the row, its clip's cached preview, and a bounded
/// preview of its override or snapshot text.
#[derive(Debug, Clone)]
pub struct QueueEntryPreview {
    pub entry: QueueEntryRow,
    pub clip: Option<(StoredItem, Option<StaticHistoryPreview>)>,
    pub annotations: Option<ItemAnnotations>,
    pub text: Option<String>,
}

/// The content one entry delivers, in precedence order: its override, its
/// snapshot, then its clip.
pub enum QueueEntryContent {
    /// The entry is gone, or its clip is deleted, expired or unreadable.
    Unavailable,
    Text {
        text: Zeroizing<String>,
        content_type: String,
        secret: bool,
    },
    /// A clip without an override, delivered as the clip itself.
    Clip { clip_id: String },
}

/// Entries removed by a clear, still sealed, so an undo can put them back.
#[derive(Debug, Clone, Default)]
pub struct ClearedQueue(Vec<Vec<Value>>);

impl ClearedQueue {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

fn status_str(status: QueueEntryStatus) -> &'static str {
    match status {
        QueueEntryStatus::Pending => "pending",
        QueueEntryStatus::Pasted => "pasted",
        QueueEntryStatus::Skipped => "skipped",
        QueueEntryStatus::Unavailable => "unavailable",
    }
}

fn parse_status(value: &str) -> QueueEntryStatus {
    match value {
        "pasted" => QueueEntryStatus::Pasted,
        "skipped" => QueueEntryStatus::Skipped,
        "unavailable" => QueueEntryStatus::Unavailable,
        _ => QueueEntryStatus::Pending,
    }
}

fn kind_str(kind: QueueContentKind) -> &'static str {
    match kind {
        QueueContentKind::Text => "text",
        QueueContentKind::RichText => "rich_text",
        QueueContentKind::Image => "image",
        QueueContentKind::Files => "files",
        QueueContentKind::Link => "link",
    }
}

fn parse_kind(value: &str) -> QueueContentKind {
    match value {
        "rich_text" => QueueContentKind::RichText,
        "image" => QueueContentKind::Image,
        "files" => QueueContentKind::Files,
        "link" => QueueContentKind::Link,
        _ => QueueContentKind::Text,
    }
}

fn via_str(via: QueueAddedVia) -> &'static str {
    match via {
        QueueAddedVia::Collect => "collect",
        QueueAddedVia::HistoryAdd => "history_add",
        QueueAddedVia::SnippetAdd => "snippet_add",
        QueueAddedVia::SelectionAction => "selection_action",
        QueueAddedVia::Share => "share",
    }
}

fn parse_via(value: &str) -> QueueAddedVia {
    match value {
        "history_add" => QueueAddedVia::HistoryAdd,
        "snippet_add" => QueueAddedVia::SnippetAdd,
        "selection_action" => QueueAddedVia::SelectionAction,
        "share" => QueueAddedVia::Share,
        _ => QueueAddedVia::Collect,
    }
}

/// The queue kind of a clip's content type.
pub fn content_kind_of(content_type: &str) -> QueueContentKind {
    use copypaste_ipc::{content_type, ContentClass};
    match content_type::classify(content_type) {
        ContentClass::Text if content_type == content_type::TEXT => QueueContentKind::Text,
        ContentClass::Text => QueueContentKind::RichText,
        ContentClass::Image => QueueContentKind::Image,
        ContentClass::File | ContentClass::Other => QueueContentKind::Files,
    }
}

fn row_entry(row: &Row<'_>) -> rusqlite::Result<QueueEntryRow> {
    Ok(QueueEntryRow {
        entry_id: row.get(0)?,
        source: QueueSourceKind::parse(&row.get::<_, String>(1)?),
        clip_id: row.get(2)?,
        content_kind: parse_kind(&row.get::<_, String>(3)?),
        status: parse_status(&row.get::<_, String>(4)?),
        added_at: row.get(5)?,
        added_via: parse_via(&row.get::<_, String>(6)?),
        delivered_at: row.get(7)?,
        rendered_at: row.get(8)?,
        edited: row.get(9)?,
        available: row.get(10)?,
    })
}

fn entries(conn: &Connection) -> Result<Vec<QueueEntryRow>, StoreError> {
    let sql = format!(
        "SELECT entry_id, source_kind, clip_id, content_kind, status, added_at, added_via, \
         delivered_at, rendered_at, override_ciphertext IS NOT NULL, {AVAILABLE} \
         FROM paste_queue_entries q {ORDER}"
    );
    let mut stmt = conn.prepare_cached(&sql)?;
    let rows = stmt.query_map([], row_entry)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

fn ordered_ids(conn: &Connection) -> rusqlite::Result<Vec<(String, QueueEntryStatus)>> {
    let mut stmt = conn.prepare_cached(&format!(
        "SELECT entry_id, status FROM paste_queue_entries {ORDER}"
    ))?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get(0)?, parse_status(&row.get::<_, String>(1)?)))
    })?;
    rows.collect()
}

fn renumber(conn: &Connection, ids: &[(String, QueueEntryStatus)]) -> rusqlite::Result<()> {
    let mut stmt =
        conn.prepare_cached("UPDATE paste_queue_entries SET position = ?2 WHERE entry_id = ?1")?;
    for (index, (id, _)) in ids.iter().enumerate() {
        stmt.execute(params![id, index as f64 + 1.0])?;
    }
    Ok(())
}

/// Advance the revision, recording collect activity when given.
fn bump(conn: &Connection, activity: Option<i64>) -> rusqlite::Result<u64> {
    conn.execute(
        "INSERT INTO paste_queue_state(id, last_activity_at, revision) VALUES (1, ?1, 1) \
         ON CONFLICT(id) DO UPDATE SET revision = revision + 1, \
         last_activity_at = COALESCE(excluded.last_activity_at, last_activity_at)",
        [activity],
    )?;
    conn.query_row(
        "SELECT revision FROM paste_queue_state WHERE id = 1",
        [],
        |row| row.get::<_, i64>(0),
    )
    .map(|revision| u64::try_from(revision).unwrap_or(0))
}

fn seal(
    key: &ItemKey,
    aad: &str,
    entry_id: &str,
    text: &str,
) -> Result<(Vec<u8>, Vec<u8>), StoreError> {
    let (nonce, ciphertext) = encrypt(text.as_bytes(), key, &format!("{aad}{entry_id}"))
        .map_err(|_| StoreError::InvalidPayload)?;
    Ok((ciphertext, nonce))
}

fn open_sealed(
    key: &ItemKey,
    aad: &str,
    entry_id: &str,
    ciphertext: &[u8],
    nonce: &[u8],
) -> Option<Zeroizing<String>> {
    let bytes = decrypt(ciphertext, nonce, key, &format!("{aad}{entry_id}")).ok()?;
    String::from_utf8(bytes.to_vec()).ok().map(Zeroizing::new)
}

type Sealed = (Option<Vec<u8>>, Option<Vec<u8>>);

fn sealed_texts(
    conn: &Connection,
    entry_id: &str,
) -> rusqlite::Result<Option<(QueueEntryRow, Sealed, Sealed)>> {
    let sql = format!(
        "SELECT entry_id, source_kind, clip_id, content_kind, status, added_at, added_via, \
         delivered_at, rendered_at, override_ciphertext IS NOT NULL, {AVAILABLE}, \
         snapshot_ciphertext, snapshot_nonce, override_ciphertext, override_nonce \
         FROM paste_queue_entries q WHERE entry_id = ?1"
    );
    conn.prepare_cached(&sql)?
        .query_row([entry_id], |row| {
            Ok((
                row_entry(row)?,
                (row.get(11)?, row.get(12)?),
                (row.get(13)?, row.get(14)?),
            ))
        })
        .optional()
}

fn open_pair(
    key: &ItemKey,
    aad: &str,
    entry_id: &str,
    sealed: &Sealed,
) -> Option<Zeroizing<String>> {
    match sealed {
        (Some(ciphertext), Some(nonce)) => open_sealed(key, aad, entry_id, ciphertext, nonce),
        _ => None,
    }
}

impl Store {
    /// The queue revision: it moves on every change and never goes back.
    pub fn paste_queue_revision(&self) -> Result<u64, StoreError> {
        let revision: Option<i64> = self
            .conn()?
            .query_row(
                "SELECT revision FROM paste_queue_state WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .optional()?;
        Ok(revision.map_or(0, |revision| u64::try_from(revision).unwrap_or(0)))
    }

    /// Advance the revision for a change held outside the tables, such as
    /// collecting turning on or off. `activity` records a collected item.
    pub fn paste_queue_touch(&self, activity: Option<i64>) -> Result<u64, StoreError> {
        let mut conn = self.conn()?;
        let tx = write_tx(&mut conn)?;
        let revision = bump(&tx, activity)?;
        tx.commit()?;
        Ok(revision)
    }

    /// Every entry in paste order.
    pub fn paste_queue_entries(&self) -> Result<Vec<QueueEntryRow>, StoreError> {
        entries(&*self.conn()?)
    }

    /// Every entry with what a list shows: the clip's cached preview and
    /// annotations, or a bounded preview of the entry's own text.
    pub fn paste_queue_previews(
        &self,
        item_key: &ItemKey,
        preview_key: &PreviewKey,
    ) -> Result<Vec<QueueEntryPreview>, StoreError> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        let generation: String = tx.query_row(
            "SELECT generation FROM resource_revisions WHERE singleton = 0",
            [],
            |row| row.get(0),
        )?;
        let listed = entries(&tx)?;
        let clip_sql = format!(
            "SELECT {} FROM clipboard_items ci WHERE ci.id = ?1 AND ci.deleted = 0",
            super::history::preview_projection()
        );
        let mut clip_stmt = tx.prepare(&clip_sql)?;
        let columns = ItemColumns::resolve(&clip_stmt)?;
        let mut previews = Vec::with_capacity(listed.len());
        let mut pending = Vec::new();
        for entry in listed {
            let mut shown = QueueEntryPreview {
                entry,
                clip: None,
                annotations: None,
                text: None,
            };
            let (_, snapshot, overridden) =
                sealed_texts(&tx, &shown.entry.entry_id)?.ok_or(StoreError::NotFound)?;
            let own = open_pair(item_key, OVERRIDE_AAD, &shown.entry.entry_id, &overridden)
                .or_else(|| open_pair(item_key, SNAPSHOT_AAD, &shown.entry.entry_id, &snapshot));
            shown.text = own.map(|text| copypaste_ipc::limits::preview_text(&text).0.to_owned());
            if let (Some(clip_id), true) = (&shown.entry.clip_id, shown.entry.available) {
                let mut rows = clip_stmt.query([clip_id])?;
                if let Some(row) = rows.next()? {
                    let item = row_to_item(row, &columns)?;
                    let resolved =
                        super::history_preview::resolve(&tx, row, &item, item_key, preview_key)?;
                    pending.extend(resolved.pending);
                    shown.annotations = local_state::annotations(&tx, clip_id)?;
                    shown.clip = Some((item, resolved.preview));
                }
            }
            previews.push(shown);
        }
        drop(clip_stmt);
        tx.commit()?;
        super::history_preview::store_pending(&mut conn, &generation, pending);
        Ok(previews)
    }

    /// The content hash and type of a live, unexpired clip.
    pub fn paste_queue_clip(&self, clip_id: &str) -> Result<Option<(String, String)>, StoreError> {
        let sql = concat!(
            "SELECT ci.content_hash, ci.content_type FROM clipboard_items ci \
             WHERE ci.id = ?1 AND ci.deleted = 0 AND ",
            unexpired!("ci.")
        );
        Ok(self
            .conn()?
            .prepare_cached(sql)?
            .query_row([clip_id], |row| Ok((row.get(0)?, row.get(1)?)))
            .optional()?)
    }

    /// Add `added` as one block at the end, or at the start, keeping its
    /// order. `None` when the queue cannot take them all within `max_entries`;
    /// nothing is added then.
    pub fn paste_queue_add(
        &self,
        key: &ItemKey,
        added: &[NewQueueEntry],
        at_start: bool,
        max_entries: u32,
        now_ms: i64,
        collected: bool,
    ) -> Result<Option<(Vec<String>, u64)>, StoreError> {
        let mut conn = self.conn()?;
        let tx = write_tx(&mut conn)?;
        let (count, low, high): (i64, Option<f64>, Option<f64>) = tx.query_row(
            "SELECT COUNT(*), MIN(position), MAX(position) FROM paste_queue_entries",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        let total = usize::try_from(count).unwrap_or(usize::MAX);
        if total.saturating_add(added.len()) > max_entries as usize {
            return Ok(None);
        }
        let first = if at_start {
            low.unwrap_or(1.0) - added.len() as f64
        } else {
            high.unwrap_or(0.0) + 1.0
        };
        let mut ids = Vec::with_capacity(added.len());
        let mut stmt = tx.prepare_cached(&format!(
            "INSERT INTO paste_queue_entries({COLUMNS}) \
             VALUES (?1,?2,?3,?4,?5,?6,NULL,NULL,?7,'pending',?8,?9,NULL,?10)"
        ))?;
        for (offset, entry) in added.iter().enumerate() {
            let entry_id = uuid::Uuid::new_v4().simple().to_string();
            let (snapshot, nonce) = match &entry.snapshot {
                Some(text) => {
                    let (ciphertext, nonce) = seal(key, SNAPSHOT_AAD, &entry_id, text)?;
                    (Some(ciphertext), Some(nonce))
                }
                None => (None, None),
            };
            stmt.execute(params![
                entry_id,
                first + offset as f64,
                entry.source.as_str(),
                entry.clip_id,
                snapshot,
                nonce,
                kind_str(entry.content_kind),
                now_ms,
                via_str(entry.added_via),
                entry.rendered_at,
            ])?;
            ids.push(entry_id);
        }
        drop(stmt);
        let revision = bump(&tx, collected.then_some(now_ms))?;
        tx.commit()?;
        Ok(Some((ids, revision)))
    }

    pub fn paste_queue_remove(&self, entry_ids: &[String]) -> Result<u64, StoreError> {
        let mut conn = self.conn()?;
        let tx = write_tx(&mut conn)?;
        {
            let mut stmt =
                tx.prepare_cached("DELETE FROM paste_queue_entries WHERE entry_id = ?1")?;
            for id in entry_ids {
                stmt.execute([id])?;
            }
        }
        let revision = bump(&tx, None)?;
        tx.commit()?;
        Ok(revision)
    }

    /// Remove every entry that is no longer pending: pasted, skipped and
    /// unavailable ones.
    pub fn paste_queue_remove_finished(&self) -> Result<u64, StoreError> {
        let mut conn = self.conn()?;
        let tx = write_tx(&mut conn)?;
        tx.execute(
            "DELETE FROM paste_queue_entries WHERE status <> 'pending'",
            [],
        )?;
        let revision = bump(&tx, None)?;
        tx.commit()?;
        Ok(revision)
    }

    /// Remove every entry and return them sealed for an undo.
    pub fn paste_queue_clear(&self) -> Result<(ClearedQueue, u64), StoreError> {
        let mut conn = self.conn()?;
        let tx = write_tx(&mut conn)?;
        let cleared = {
            let mut stmt = tx.prepare_cached(&format!(
                "SELECT {COLUMNS} FROM paste_queue_entries {ORDER}"
            ))?;
            let rows = stmt.query_map([], |row| {
                (0..COLUMN_COUNT).map(|index| row.get(index)).collect()
            })?;
            rows.collect::<rusqlite::Result<Vec<Vec<Value>>>>()?
        };
        tx.execute("DELETE FROM paste_queue_entries", [])?;
        let revision = bump(&tx, None)?;
        tx.commit()?;
        Ok((ClearedQueue(cleared), revision))
    }

    /// Put back entries a clear removed, ahead of anything added since.
    pub fn paste_queue_restore(&self, cleared: &ClearedQueue) -> Result<u64, StoreError> {
        let mut conn = self.conn()?;
        let tx = write_tx(&mut conn)?;
        let placeholders = vec!["?"; COLUMN_COUNT].join(",");
        {
            let mut stmt = tx.prepare_cached(&format!(
                "INSERT OR IGNORE INTO paste_queue_entries({COLUMNS}) VALUES ({placeholders})"
            ))?;
            for row in &cleared.0 {
                stmt.execute(params_from_iter(row.iter()))?;
            }
        }
        let mut ids = ordered_ids(&tx)?;
        let restored: Vec<&Value> = cleared.0.iter().filter_map(|row| row.first()).collect();
        ids.sort_by_key(|(id, _)| {
            !restored
                .iter()
                .any(|value| matches!(value, Value::Text(text) if text == id))
        });
        renumber(&tx, &ids)?;
        let revision = bump(&tx, None)?;
        tx.commit()?;
        Ok(revision)
    }

    /// Reverse the order of the pending entries; the others keep their place.
    pub fn paste_queue_reverse(&self) -> Result<u64, StoreError> {
        let mut conn = self.conn()?;
        let tx = write_tx(&mut conn)?;
        let mut ids = ordered_ids(&tx)?;
        let slots: Vec<usize> = (0..ids.len())
            .filter(|index| ids[*index].1 == QueueEntryStatus::Pending)
            .collect();
        for step in 0..slots.len() / 2 {
            ids.swap(slots[step], slots[slots.len() - 1 - step]);
        }
        renumber(&tx, &ids)?;
        let revision = bump(&tx, None)?;
        tx.commit()?;
        Ok(revision)
    }

    /// Move one entry to `to_index` in the list. `None` for an unknown entry.
    pub fn paste_queue_move(
        &self,
        entry_id: &str,
        to_index: usize,
    ) -> Result<Option<u64>, StoreError> {
        let mut conn = self.conn()?;
        let tx = write_tx(&mut conn)?;
        let mut ids = ordered_ids(&tx)?;
        let Some(from) = ids.iter().position(|(id, _)| id == entry_id) else {
            return Ok(None);
        };
        let moved = ids.remove(from);
        ids.insert(to_index.min(ids.len()), moved);
        renumber(&tx, &ids)?;
        let revision = bump(&tx, None)?;
        tx.commit()?;
        Ok(Some(revision))
    }

    /// Set or drop the local override of a text entry. `None` when the entry
    /// is unknown or not text.
    pub fn paste_queue_set_override(
        &self,
        key: &ItemKey,
        entry_id: &str,
        text: Option<&str>,
    ) -> Result<Option<u64>, StoreError> {
        let mut conn = self.conn()?;
        let tx = write_tx(&mut conn)?;
        let kind: Option<String> = tx
            .query_row(
                "SELECT content_kind FROM paste_queue_entries WHERE entry_id = ?1",
                [entry_id],
                |row| row.get(0),
            )
            .optional()?;
        let editable = kind.map(|kind| parse_kind(&kind)).is_some_and(|kind| {
            matches!(
                kind,
                QueueContentKind::Text | QueueContentKind::RichText | QueueContentKind::Link
            )
        });
        if !editable {
            return Ok(None);
        }
        let (ciphertext, nonce) = match text {
            Some(text) => {
                let (ciphertext, nonce) = seal(key, OVERRIDE_AAD, entry_id, text)?;
                (Some(ciphertext), Some(nonce))
            }
            None => (None, None),
        };
        tx.execute(
            "UPDATE paste_queue_entries SET override_ciphertext = ?2, override_nonce = ?3 \
             WHERE entry_id = ?1",
            params![entry_id, ciphertext, nonce],
        )?;
        let revision = bump(&tx, None)?;
        tx.commit()?;
        Ok(Some(revision))
    }

    /// Set the status of `entry_ids`. `delivered_at` is recorded for a
    /// delivery and cleared for any other status.
    pub fn paste_queue_set_status(
        &self,
        entry_ids: &[String],
        status: QueueEntryStatus,
        delivered_at: Option<i64>,
    ) -> Result<u64, StoreError> {
        let mut conn = self.conn()?;
        let tx = write_tx(&mut conn)?;
        {
            let mut stmt = tx.prepare_cached(
                "UPDATE paste_queue_entries SET status = ?2, delivered_at = ?3 \
                 WHERE entry_id = ?1",
            )?;
            for id in entry_ids {
                stmt.execute(params![id, status_str(status), delivered_at])?;
            }
        }
        let revision = bump(&tx, None)?;
        tx.commit()?;
        Ok(revision)
    }

    /// Return the most recently pasted entry to pending, ahead of every other
    /// pending entry. `None` when nothing was pasted.
    pub fn paste_queue_step_back(&self) -> Result<Option<u64>, StoreError> {
        let mut conn = self.conn()?;
        let tx = write_tx(&mut conn)?;
        let last: Option<String> = tx
            .query_row(
                "SELECT entry_id FROM paste_queue_entries WHERE status = 'pasted' \
                 ORDER BY delivered_at DESC, position DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()?;
        let Some(last) = last else {
            return Ok(None);
        };
        tx.execute(
            "UPDATE paste_queue_entries SET status = 'pending', delivered_at = NULL \
             WHERE entry_id = ?1",
            [&last],
        )?;
        let mut ids = ordered_ids(&tx)?;
        let from = ids.iter().position(|(id, _)| *id == last).unwrap_or(0);
        let head = ids
            .iter()
            .position(|(id, status)| *status == QueueEntryStatus::Pending && *id != last)
            .unwrap_or(ids.len());
        if head < from {
            let moved = ids.remove(from);
            ids.insert(head, moved);
            renumber(&tx, &ids)?;
        }
        let revision = bump(&tx, None)?;
        tx.commit()?;
        Ok(Some(revision))
    }

    /// Return every pasted and skipped entry to pending, for a loop.
    pub fn paste_queue_repend(&self) -> Result<u64, StoreError> {
        let mut conn = self.conn()?;
        let tx = write_tx(&mut conn)?;
        tx.execute(
            "UPDATE paste_queue_entries SET status = 'pending', delivered_at = NULL \
             WHERE status IN ('pasted', 'skipped')",
            [],
        )?;
        let revision = bump(&tx, None)?;
        tx.commit()?;
        Ok(revision)
    }

    /// What `entry_id` delivers. With `open_text_clips` a text clip is opened
    /// and returned as text (for an editor commit or a join); otherwise a clip
    /// without an override is returned as the clip. `plain` converts rich text.
    pub fn paste_queue_content(
        &self,
        key: &ItemKey,
        entry_id: &str,
        plain: bool,
        open_text_clips: bool,
    ) -> Result<QueueEntryContent, StoreError> {
        let conn = self.conn()?;
        let Some((entry, snapshot, overridden)) = sealed_texts(&conn, entry_id)? else {
            return Ok(QueueEntryContent::Unavailable);
        };
        drop(conn);
        if !entry.available {
            return Ok(QueueEntryContent::Unavailable);
        }
        let concealed = entry.source == QueueSourceKind::ConcealedSnippet;
        let clip = match &entry.clip_id {
            Some(clip_id) => match self.get(clip_id)? {
                Some(row) => Some(row),
                None => return Ok(QueueEntryContent::Unavailable),
            },
            None => None,
        };
        let secret = concealed
            || clip
                .as_ref()
                .is_some_and(|row| row.clipboard_privacy().secret);
        let own = if entry.edited {
            open_pair(key, OVERRIDE_AAD, entry_id, &overridden)
        } else {
            open_pair(key, SNAPSHOT_AAD, entry_id, &snapshot)
        };
        if let Some(text) = own {
            return Ok(QueueEntryContent::Text {
                text,
                content_type: copypaste_ipc::content_type::TEXT.to_owned(),
                secret,
            });
        }
        let Some(row) = clip else {
            return Ok(QueueEntryContent::Unavailable);
        };
        if !open_text_clips
            || !matches!(
                entry.content_kind,
                QueueContentKind::Text | QueueContentKind::RichText | QueueContentKind::Link
            )
        {
            return Ok(QueueEntryContent::Clip { clip_id: row.id });
        }
        let Ok(ClipboardPayload::Text(text)) = ClipboardPayload::open(&row, key) else {
            return Ok(QueueEntryContent::Unavailable);
        };
        if !plain || row.content_type == copypaste_ipc::content_type::TEXT {
            return Ok(QueueEntryContent::Text {
                text,
                content_type: row.content_type,
                secret,
            });
        }
        match crate::rich_text::plain_text(&row.content_type, &text) {
            Ok(text) => Ok(QueueEntryContent::Text {
                text: Zeroizing::new(text),
                content_type: copypaste_ipc::content_type::TEXT.to_owned(),
                secret,
            }),
            Err(_) => Ok(QueueEntryContent::Unavailable),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::test_support::{item, store, T0};

    fn key() -> ItemKey {
        crate::Keyring::from_secret(&[7; 32]).item_key()
    }

    fn clip_entry(clip_id: &str) -> NewQueueEntry {
        NewQueueEntry {
            source: QueueSourceKind::Clip,
            clip_id: Some(clip_id.to_owned()),
            snapshot: None,
            content_kind: QueueContentKind::Text,
            added_via: QueueAddedVia::HistoryAdd,
            rendered_at: None,
        }
    }

    #[test]
    fn retention_never_evicts_a_clip_a_pending_entry_references() {
        let s = store();
        let oldest = s.insert(item("oldest", T0)).unwrap().id;
        s.insert(item("middle", T0 + 1)).unwrap();
        s.insert(item("newest", T0 + 2)).unwrap();
        let (ids, _) = s
            .paste_queue_add(&key(), &[clip_entry(&oldest)], false, 200, T0, false)
            .unwrap()
            .unwrap();

        assert_eq!(s.evict_over_cap(1).unwrap(), 1);
        assert_eq!(s.evict_older_than(T0 + 10).unwrap(), 1);
        assert!(s.get(&oldest).unwrap().is_some());

        s.paste_queue_set_status(&ids, QueueEntryStatus::Pasted, Some(T0))
            .unwrap();
        assert_eq!(s.evict_older_than(T0 + 10).unwrap(), 1);
        assert!(s.get(&oldest).unwrap().is_none());
        assert!(!s.paste_queue_entries().unwrap()[0].available);
    }

    #[test]
    fn a_cleared_queue_comes_back_in_order_with_its_sealed_text() {
        let s = store();
        let clip = s.insert(item("clip", T0)).unwrap().id;
        let snapshot = NewQueueEntry {
            source: QueueSourceKind::Snippet,
            clip_id: None,
            snapshot: Some(Zeroizing::new("rendered".into())),
            content_kind: QueueContentKind::Text,
            added_via: QueueAddedVia::SnippetAdd,
            rendered_at: Some(T0),
        };
        let (ids, _) = s
            .paste_queue_add(
                &key(),
                &[clip_entry(&clip), snapshot],
                false,
                200,
                T0,
                false,
            )
            .unwrap()
            .unwrap();
        s.paste_queue_set_override(&key(), &ids[0], Some("edited"))
            .unwrap()
            .unwrap();
        let (cleared, _) = s.paste_queue_clear().unwrap();
        assert!(s.paste_queue_entries().unwrap().is_empty());

        s.paste_queue_restore(&cleared).unwrap();
        let restored = s.paste_queue_entries().unwrap();
        assert_eq!(
            restored
                .iter()
                .map(|e| e.entry_id.clone())
                .collect::<Vec<_>>(),
            ids
        );
        assert!(restored[0].edited);
        let QueueEntryContent::Text { text, .. } =
            s.paste_queue_content(&key(), &ids[1], false, true).unwrap()
        else {
            panic!("snapshot text");
        };
        assert_eq!(text.as_str(), "rendered");
    }
}
