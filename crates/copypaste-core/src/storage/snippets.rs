//! Snippet library queries and writes.
//!
//! Snippets, bodies and collections are organization records; this module
//! holds the rules above their projections: limits and validation,
//! abbreviation uniqueness, ordering, Recently Deleted and purge, ranked quick
//! search, and publishing conflict copies once the item key is at hand. Usage
//! and the origin clip are local rows and never sync.

use std::collections::{HashMap, HashSet};

use rusqlite::{params, params_from_iter, Connection, OptionalExtension, Row};

use super::connection::write_tx;
use super::organization::{
    self, Record, RecordKey, SnippetBodyRecord, SnippetCollectionRecord, SnippetRecord,
};
use super::{Store, StoreError};
use crate::snippets::abbreviation::{self, AbbreviationError};
use crate::snippets::conflict;
use crate::snippets::ranking::{self, RankSignals, SearchTier};
use crate::ItemKey;

pub const MAX_TITLE_CHARS: usize = 200;
pub const MAX_BODY_BYTES: usize = 64 * 1024;
/// Plain and rich representations together.
pub const MAX_BODY_WITH_RICH_BYTES: usize = 256 * 1024;
/// Above this many snippets the library shows a warning.
pub const SOFT_SNIPPET_LIMIT: usize = 10_000;
pub const MAX_SNIPPETS: usize = 20_000;
pub const MAX_COLLECTIONS: usize = 200;
pub const MAX_COLLECTION_NAME_CHARS: usize = 64;
const MAX_COLLECTION_STYLE_CHARS: usize = 64;
/// How long a deleted snippet stays in Recently Deleted before it is purged.
pub const RECENTLY_DELETED_MS: i64 = 30 * 24 * 60 * 60 * 1000;

#[derive(Debug, thiserror::Error)]
pub enum SnippetError {
    #[error("the abbreviation is already used by another snippet")]
    AbbreviationTaken { snippet_id: String, title: String },
    #[error("the abbreviation is not valid")]
    InvalidAbbreviation(AbbreviationError),
    #[error("a snippet body cannot be empty")]
    EmptyBody,
    #[error("the snippet is larger than the limit")]
    TooLarge,
    #[error("the title is not valid")]
    InvalidTitle,
    #[error("the collection is not valid")]
    InvalidCollection,
    #[error("a collection with that name already exists")]
    CollectionNameTaken { collection_id: String },
    #[error("the snippet library is full")]
    LimitReached,
    #[error("snippet or collection not found")]
    NotFound,
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl From<rusqlite::Error> for SnippetError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Store(error.into())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SnippetScope {
    All,
    /// No collection, or one that has been deleted.
    Unfiled,
    Collection(String),
    Pinned,
    RecentlyDeleted,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SnippetSummary {
    pub id: String,
    /// `None` when unfiled, including when its collection was deleted.
    pub collection_id: Option<String>,
    pub title: String,
    pub abbreviation: Option<String>,
    pub template: bool,
    pub pinned: bool,
    pub pin_order: Option<f64>,
    pub order_key: String,
    pub concealed: bool,
    pub deleted_at: Option<i64>,
    pub created_at: i64,
    /// The newer of the metadata and body versions.
    pub updated_at: i64,
    /// The start of the body; empty for a concealed snippet.
    pub preview: String,
    /// Another live snippet has the same normalized abbreviation.
    pub duplicate: bool,
    pub last_used_at: Option<i64>,
    pub use_count: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SnippetDetail {
    pub snippet: SnippetSummary,
    pub body_text: String,
    pub body_rich: Option<String>,
    pub revision: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SnippetPage {
    pub snippets: Vec<SnippetSummary>,
    /// Pass back as the cursor for the following page.
    pub next: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SnippetDraft {
    pub id: Option<String>,
    pub collection_id: Option<String>,
    pub title: String,
    pub abbreviation: Option<String>,
    pub body_text: String,
    pub body_rich: Option<String>,
    pub template: bool,
    pub pinned: bool,
    pub concealed: bool,
    /// The revision the editor started from; a newer stored body that this
    /// edit did not see is kept as a conflict copy.
    pub base_revision: Option<String>,
    /// The history clip this snippet was saved from (local only).
    pub origin_clip_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnippetCollection {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub accent: String,
    pub order_key: String,
    pub snippet_count: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CollectionDraft {
    pub id: Option<String>,
    pub name: String,
    pub icon: String,
    pub accent: String,
}

/// Returns the rewritten body, or `None` to leave it unchanged.
pub type BodyRewrite<'a> = &'a dyn Fn(&str) -> Option<String>;

#[derive(Debug, Clone, PartialEq)]
pub struct SnippetHit {
    pub snippet: SnippetSummary,
    pub tier: SearchTier,
}

const SUMMARY_SELECT: &str =
    "SELECT s.id,CASE WHEN c.id IS NULL THEN NULL ELSE s.collection_id END,\
     s.title,s.abbreviation,s.template,s.pinned,s.pin_order,s.order_key,s.concealed,s.deleted_at,\
     s.created_at,MAX(s.meta_stamp,COALESCE(b.body_stamp,0)),\
     CASE WHEN s.concealed THEN '' ELSE substr(COALESCE(b.body_text,''),1,1024) END,\
     s.abbreviation_key IS NOT NULL AND s.deleted_at IS NULL AND EXISTS(SELECT 1 FROM snippets d \
     WHERE d.abbreviation_key=s.abbreviation_key AND d.deleted_at IS NULL AND d.purged=0 \
     AND d.id<>s.id),u.last_used_at,COALESCE(u.use_count,0),b.body_text,b.body_rich,b.revision \
     FROM snippets s LEFT JOIN snippet_bodies b ON b.id=s.id \
     LEFT JOIN snippet_collections c ON c.id=s.collection_id AND c.deleted_at IS NULL \
     LEFT JOIN snippet_usage u ON u.id=s.id";

/// Only the summary columns are read, so list pages never load whole bodies.
fn summary(r: &Row<'_>) -> rusqlite::Result<SnippetSummary> {
    let preview: String = r.get(12)?;
    Ok(SnippetSummary {
        id: r.get(0)?,
        collection_id: r.get(1)?,
        title: r.get(2)?,
        abbreviation: r.get(3)?,
        template: r.get(4)?,
        pinned: r.get(5)?,
        pin_order: r.get(6)?,
        order_key: r.get(7)?,
        concealed: r.get(8)?,
        deleted_at: r.get(9)?,
        created_at: r.get(10)?,
        updated_at: r.get(11)?,
        preview: copypaste_ipc::limits::preview_text(&preview).0.to_owned(),
        duplicate: r.get(13)?,
        last_used_at: r.get(14)?,
        use_count: r.get(15)?,
    })
}

fn detail(r: &Row<'_>) -> rusqlite::Result<SnippetDetail> {
    Ok(SnippetDetail {
        snippet: summary(r)?,
        body_text: r.get::<_, Option<String>>(16)?.unwrap_or_default(),
        body_rich: r.get(17)?,
        revision: r.get(18)?,
    })
}

fn record(conn: &Connection, id: &str) -> Result<Option<SnippetRecord>, StoreError> {
    Ok(match organization::read(conn, RecordKey::Snippet(id))? {
        Some(Record::Snippet(record)) => Some(record),
        _ => None,
    })
}

fn body(conn: &Connection, id: &str) -> Result<Option<SnippetBodyRecord>, StoreError> {
    Ok(
        match organization::read(conn, RecordKey::SnippetBody(id))? {
            Some(Record::SnippetBody(body)) => Some(body),
            _ => None,
        },
    )
}

fn collection(conn: &Connection, id: &str) -> Result<Option<SnippetCollectionRecord>, StoreError> {
    Ok(
        match organization::read(conn, RecordKey::SnippetCollection(id))? {
            Some(Record::SnippetCollection(collection)) => Some(collection),
            _ => None,
        },
    )
}

/// A snippet that is neither purged nor in Recently Deleted.
fn live(conn: &Connection, id: &str) -> Result<SnippetRecord, SnippetError> {
    record(conn, id)?
        .filter(|record| !record.purged && record.deleted_at.is_none())
        .ok_or(SnippetError::NotFound)
}

fn live_collection(conn: &Connection, id: &str) -> Result<SnippetCollectionRecord, SnippetError> {
    collection(conn, id)?
        .filter(|collection| collection.deleted_at.is_none())
        .ok_or(SnippetError::NotFound)
}

fn local_device(conn: &Connection) -> Result<String, StoreError> {
    Ok(conn
        .query_row(
            "SELECT value FROM sync_device_state WHERE key='device_id'",
            [],
            |r| r.get(0),
        )
        .optional()?
        .unwrap_or_default())
}

pub(super) fn normalize_line_endings(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

fn single_line(text: &str, max_chars: usize) -> bool {
    text.chars().count() <= max_chars && !text.chars().any(char::is_control)
}

fn abbreviation_owner(
    conn: &Connection,
    abbreviation: &str,
    except: &str,
) -> Result<Option<(String, String)>, StoreError> {
    Ok(conn
        .query_row(
            "SELECT id,title FROM snippets WHERE abbreviation_key=?1 AND deleted_at IS NULL \
             AND purged=0 AND id<>?2 ORDER BY id LIMIT 1",
            params![abbreviation::normalize(abbreviation), except],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?)
}

/// Checks a new abbreviation; keeping the stored one is always allowed, so a
/// duplicate left by a merge can still be edited.
fn check_abbreviation(
    conn: &Connection,
    id: &str,
    current: Option<&str>,
    wanted: Option<&str>,
) -> Result<(), SnippetError> {
    let Some(wanted) = wanted else {
        return Ok(());
    };
    abbreviation::validate(wanted).map_err(SnippetError::InvalidAbbreviation)?;
    if current.map(abbreviation::normalize) == Some(abbreviation::normalize(wanted)) {
        return Ok(());
    }
    match abbreviation_owner(conn, wanted, id)? {
        Some((snippet_id, title)) => Err(SnippetError::AbbreviationTaken { snippet_id, title }),
        None => Ok(()),
    }
}

const ORDER_DIGITS: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

fn order_digit(byte: u8) -> usize {
    ORDER_DIGITS
        .iter()
        .position(|digit| *digit == byte)
        .unwrap_or(if byte < b'0' {
            0
        } else {
            ORDER_DIGITS.len() - 1
        })
}

/// A key strictly between `low` and `high` (fractional indexing over base-62
/// digits), so a move rewrites one record. `None` is the open end.
fn order_key_between(low: Option<&str>, high: Option<&str>) -> String {
    fn midpoint(low: &[usize], high: Option<&[usize]>, out: &mut Vec<usize>) {
        if let Some(high) = high {
            let shared = high
                .iter()
                .enumerate()
                .take_while(|(at, digit)| low.get(*at).copied().unwrap_or(0) == **digit)
                .count();
            if shared > 0 {
                out.extend_from_slice(&high[..shared]);
                return midpoint(low.get(shared..).unwrap_or(&[]), Some(&high[shared..]), out);
            }
        }
        let below = low.first().copied().unwrap_or(0);
        let above = high
            .and_then(|high| high.first().copied())
            .unwrap_or(ORDER_DIGITS.len());
        if above > below + 1 {
            out.push((below + above) / 2);
        } else if let Some(high) = high.filter(|high| high.len() > 1) {
            out.push(high[0]);
        } else {
            out.push(below);
            midpoint(low.get(1..).unwrap_or(&[]), None, out);
        }
    }
    let digits = |key: &str| key.bytes().map(order_digit).collect::<Vec<_>>();
    let low = low.map(digits).unwrap_or_default();
    let high = high.map(digits);
    let mut out = Vec::new();
    midpoint(&low, high.as_deref(), &mut out);
    out.into_iter()
        .map(|digit| ORDER_DIGITS[digit] as char)
        .collect()
}

fn last_order_key(conn: &Connection, collection_id: Option<&str>) -> Result<String, StoreError> {
    let last: Option<String> = conn.query_row(
        "SELECT MAX(order_key) FROM snippets WHERE collection_id IS ?1",
        [collection_id],
        |r| r.get(0),
    )?;
    Ok(order_key_between(last.as_deref(), None))
}

fn last_pin_order(conn: &Connection) -> Result<f64, StoreError> {
    let last: Option<f64> = conn.query_row(
        "SELECT MAX(pin_order) FROM snippets WHERE pinned=1",
        [],
        |r| r.get(0),
    )?;
    Ok(last.map_or(1.0, |last| last + 1.0))
}

fn write_snippet(tx: &Connection, key: &ItemKey, record: SnippetRecord) -> Result<(), StoreError> {
    organization::write(tx, key, Record::Snippet(record))
}

/// Writes a new body revision descending from `base` (or the stored body).
fn write_body(
    tx: &Connection,
    key: &ItemKey,
    id: &str,
    text: String,
    rich_html: Option<String>,
    base: Option<&str>,
) -> Result<(), StoreError> {
    let current = body(tx, id)?;
    let current_revision = current
        .as_ref()
        .map(|current| (current.revision.as_str(), current.ancestors.as_slice()));
    let base = base.or(current_revision.map(|(revision, _)| revision));
    let ancestors = base.map_or_else(Vec::new, |base| {
        conflict::ancestors_from(base, current_revision)
    });
    organization::write(
        tx,
        key,
        Record::SnippetBody(SnippetBodyRecord {
            id: id.to_owned(),
            revision: conflict::new_revision(),
            ancestors,
            text,
            rich_html,
        }),
    )
}

/// A delete only holds while its stamp is not older than the body's: a peer
/// clock running ahead must not make a local delete a no-op.
fn mark_deleted(
    tx: &Connection,
    key: &ItemKey,
    mut record: SnippetRecord,
    now: i64,
) -> Result<(), StoreError> {
    let id = record.id.clone();
    record.deleted_at = Some(now);
    write_snippet(tx, key, record)?;
    let stamps: Option<(String, i64, i64)> = tx
        .query_row(
            "SELECT s.record_id,s.meta_stamp,b.body_stamp FROM snippets s \
             JOIN snippet_bodies b ON b.id=s.id WHERE s.id=?1",
            [&id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    if let Some((record_id, meta, body)) = stamps.filter(|(_, meta, body)| meta < body) {
        let stamp = body.saturating_add(1).max(meta);
        tx.execute(
            "UPDATE organization_records SET created_at=?2 WHERE id=?1",
            params![record_id, stamp],
        )?;
        tx.execute(
            "UPDATE snippets SET meta_stamp=?2,deleted_at=?3 WHERE id=?1",
            params![id, stamp, now],
        )?;
    }
    Ok(())
}

/// Rewrites both records as content-free stubs. Terminal on every device.
fn purge(
    tx: &Connection,
    key: &ItemKey,
    record: SnippetRecord,
    now: i64,
) -> Result<(), StoreError> {
    let id = record.id.clone();
    write_snippet(
        tx,
        key,
        SnippetRecord {
            collection_id: None,
            title: String::new(),
            abbreviation: None,
            pinned: false,
            pin_order: None,
            deleted_at: record.deleted_at.or(Some(now)),
            purged: true,
            ..record
        },
    )?;
    organization::write(
        tx,
        key,
        Record::SnippetBody(SnippetBodyRecord {
            id: id.clone(),
            revision: conflict::PURGED_REVISION.to_owned(),
            ancestors: Vec::new(),
            text: String::new(),
            rich_html: None,
        }),
    )?;
    tx.execute("DELETE FROM snippet_usage WHERE id=?1", [&id])?;
    tx.execute("DELETE FROM snippet_origin WHERE id=?1", [&id])?;
    Ok(())
}

/// Conflict copies are projected without records by an apply that held no
/// key; this writes their records so they reach every other device.
fn publish_copies(tx: &Connection, key: &ItemKey) -> Result<usize, StoreError> {
    let pending = |sql: &str| -> Result<Vec<String>, StoreError> {
        Ok(tx
            .prepare(sql)?
            .query_map([conflict::COPY_RANGE.0, conflict::COPY_RANGE.1], |r| {
                r.get(0)
            })?
            .collect::<rusqlite::Result<_>>()?)
    };
    let snippets = pending(
        "SELECT p.id FROM snippets p WHERE p.id>=?1 AND p.id<?2 \
         AND NOT EXISTS(SELECT 1 FROM organization_records r WHERE r.id=p.record_id)",
    )?;
    let bodies = pending(
        "SELECT p.id FROM snippet_bodies p WHERE p.id>=?1 AND p.id<?2 \
         AND NOT EXISTS(SELECT 1 FROM organization_records r WHERE r.id=p.record_id)",
    )?;
    let published = snippets.len() + bodies.len();
    for id in snippets {
        if let Some(record) = record(tx, &id)? {
            write_snippet(tx, key, record)?;
        }
    }
    for id in bodies {
        if let Some(body) = body(tx, &id)? {
            organization::write(tx, key, Record::SnippetBody(body))?;
        }
    }
    Ok(published)
}

fn save(tx: &Connection, key: &ItemKey, draft: &SnippetDraft) -> Result<String, SnippetError> {
    let title = draft.title.trim();
    if !single_line(title, MAX_TITLE_CHARS) {
        return Err(SnippetError::InvalidTitle);
    }
    let text = normalize_line_endings(&draft.body_text);
    if text.trim().is_empty() {
        return Err(SnippetError::EmptyBody);
    }
    let wanted_abbreviation = draft
        .abbreviation
        .as_deref()
        .map(str::trim)
        .filter(|abbreviation| !abbreviation.is_empty());
    let existing = match &draft.id {
        Some(id) => Some(
            record(tx, id)?
                .filter(|record| !record.purged)
                .ok_or(SnippetError::NotFound)?,
        ),
        None => None,
    };
    if existing.is_none() && snippet_total(tx)? >= MAX_SNIPPETS {
        return Err(SnippetError::LimitReached);
    }
    if let Some(collection_id) = &draft.collection_id {
        live_collection(tx, collection_id)?;
    }
    let id = existing.as_ref().map_or_else(
        || uuid::Uuid::new_v4().to_string(),
        |existing| existing.id.clone(),
    );
    check_abbreviation(
        tx,
        &id,
        existing
            .as_ref()
            .and_then(|existing| existing.abbreviation.as_deref()),
        wanted_abbreviation,
    )?;

    let current = body(tx, &id)?;
    let unchanged_text = current.as_ref().is_some_and(|current| current.text == text);
    let rich_html = if draft.template {
        None
    } else if unchanged_text {
        draft.body_rich.clone().or_else(|| {
            current
                .as_ref()
                .and_then(|current| current.rich_html.clone())
        })
    } else {
        draft.body_rich.clone()
    };
    if text.len() > MAX_BODY_BYTES
        || text.len() + rich_html.as_ref().map_or(0, String::len) > MAX_BODY_WITH_RICH_BYTES
    {
        return Err(SnippetError::TooLarge);
    }

    let order_key = match &existing {
        Some(existing) if existing.collection_id == draft.collection_id => {
            existing.order_key.clone()
        }
        _ => last_order_key(tx, draft.collection_id.as_deref())?,
    };
    let pin_order = match &existing {
        _ if !draft.pinned => None,
        Some(existing) if existing.pinned => existing.pin_order,
        _ => Some(last_pin_order(tx)?),
    };
    let (created_at, created_device) = match &existing {
        Some(existing) => (existing.created_at, existing.created_device.clone()),
        None => (crate::now_ms(), local_device(tx)?),
    };
    write_snippet(
        tx,
        key,
        SnippetRecord {
            id: id.clone(),
            collection_id: draft.collection_id.clone(),
            title: title.to_owned(),
            abbreviation: wanted_abbreviation.map(str::to_owned),
            template: draft.template,
            pinned: draft.pinned,
            pin_order,
            order_key,
            concealed: draft.concealed,
            deleted_at: None,
            purged: false,
            created_at,
            created_device,
        },
    )?;
    let stored = current
        .as_ref()
        .map(|current| (current.text.as_str(), current.rich_html.as_deref()));
    if stored != Some((text.as_str(), rich_html.as_deref())) {
        write_body(
            tx,
            key,
            &id,
            text,
            rich_html,
            draft.base_revision.as_deref(),
        )?;
    }
    if let Some(clip_id) = &draft.origin_clip_id {
        tx.execute(
            "INSERT INTO snippet_origin(id,clip_id) VALUES (?1,?2) \
             ON CONFLICT(id) DO UPDATE SET clip_id=excluded.clip_id",
            params![id, clip_id],
        )?;
    }
    publish_copies(tx, key)?;
    Ok(id)
}

fn snippet_total(conn: &Connection) -> Result<usize, StoreError> {
    Ok(
        conn.query_row("SELECT COUNT(*) FROM snippets WHERE purged=0", [], |r| {
            r.get(0)
        })?,
    )
}

/// Non-purged template snippets other than `except` whose body mentions
/// `needle`; the caller's parser decides which of them really reference it.
fn reference_candidates(
    conn: &Connection,
    needle: &str,
    except: &str,
) -> Result<Vec<(String, String)>, StoreError> {
    Ok(conn
        .prepare(
            "SELECT s.id,b.body_text FROM snippets s JOIN snippet_bodies b ON b.id=s.id \
             WHERE s.purged=0 AND s.template=1 AND s.id<>?2 AND instr(b.body_text,?1)>0 \
             ORDER BY s.id",
        )?
        .query_map(params![needle, except], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?)
}

/// Every snippet still filed in `collection_id`, live or recently deleted.
fn filed_in(conn: &Connection, collection_id: &str) -> Result<Vec<SnippetRecord>, StoreError> {
    let ids: Vec<String> = conn
        .prepare(
            "SELECT id FROM snippets WHERE collection_id=?1 AND purged=0 ORDER BY order_key,id",
        )?
        .query_map([collection_id], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    let mut records = Vec::with_capacity(ids.len());
    for id in ids {
        records.extend(record(conn, &id)?);
    }
    Ok(records)
}

fn move_to(
    tx: &Connection,
    key: &ItemKey,
    record: SnippetRecord,
    collection_id: Option<&str>,
) -> Result<(), StoreError> {
    let order_key = last_order_key(tx, collection_id)?;
    write_snippet(
        tx,
        key,
        SnippetRecord {
            collection_id: collection_id.map(str::to_owned),
            order_key,
            ..record
        },
    )
}

impl Store {
    fn snippet_write<T>(
        &self,
        work: impl FnOnce(&Connection) -> Result<T, SnippetError>,
    ) -> Result<T, SnippetError> {
        let mut conn = self.conn()?;
        let tx = write_tx(&mut conn)?;
        let result = work(&tx)?;
        tx.commit()?;
        Ok(result)
    }

    /// One page of a list scope, in display order.
    pub fn snippet_page(
        &self,
        scope: &SnippetScope,
        cursor: Option<&str>,
        limit: u32,
    ) -> Result<SnippetPage, StoreError> {
        let limit = copypaste_ipc::clamp_page(limit, copypaste_ipc::DEFAULT_LIST_PAGE);
        let conn = self.conn()?;
        if let Some(cursor) = cursor {
            if record(&conn, cursor)?.is_none() {
                return Err(StoreError::InvalidCursor);
            }
        }
        let (filter, order, after) = match scope {
            SnippetScope::All => (
                "s.purged=0 AND s.deleted_at IS NULL",
                "s.order_key,s.id",
                "(s.order_key,s.id)>(SELECT order_key,id FROM snippets WHERE id=?2)",
            ),
            SnippetScope::Unfiled => (
                "s.purged=0 AND s.deleted_at IS NULL AND c.id IS NULL",
                "s.order_key,s.id",
                "(s.order_key,s.id)>(SELECT order_key,id FROM snippets WHERE id=?2)",
            ),
            SnippetScope::Collection(_) => (
                "s.purged=0 AND s.deleted_at IS NULL AND c.id IS NOT NULL AND s.collection_id=?3",
                "s.order_key,s.id",
                "(s.order_key,s.id)>(SELECT order_key,id FROM snippets WHERE id=?2)",
            ),
            SnippetScope::Pinned => (
                "s.purged=0 AND s.deleted_at IS NULL AND s.pinned=1",
                "s.pin_order,s.id",
                "(s.pin_order,s.id)>(SELECT pin_order,id FROM snippets WHERE id=?2)",
            ),
            SnippetScope::RecentlyDeleted => (
                "s.purged=0 AND s.deleted_at IS NOT NULL",
                "s.deleted_at DESC,s.id DESC",
                "(s.deleted_at,s.id)<(SELECT deleted_at,id FROM snippets WHERE id=?2)",
            ),
        };
        let sql = format!(
            "{SUMMARY_SELECT} WHERE {filter} AND (?2 IS NULL OR {after}) ORDER BY {order} LIMIT ?1"
        );
        let mut values: Vec<rusqlite::types::Value> = vec![
            (i64::from(limit) + 1).into(),
            cursor.map(str::to_owned).into(),
        ];
        if let SnippetScope::Collection(id) = scope {
            values.push(id.clone().into());
        }
        let mut snippets = conn
            .prepare(&sql)?
            .query_map(params_from_iter(values), summary)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let next = (snippets.len() > limit as usize).then(|| {
            snippets.truncate(limit as usize);
            snippets.last().map(|last| last.id.clone())
        });
        Ok(SnippetPage {
            snippets,
            next: next.flatten(),
        })
    }

    /// A snippet with its body; `None` once purged.
    pub fn snippet(&self, id: &str) -> Result<Option<SnippetDetail>, StoreError> {
        Ok(self
            .conn()?
            .query_row(
                &format!("{SUMMARY_SELECT} WHERE s.id=?1 AND s.purged=0"),
                [id],
                detail,
            )
            .optional()?)
    }

    /// Creates or edits a snippet in place. Metadata and body are separate
    /// records, and the body gets a new revision only when it changed.
    pub fn save_snippet(
        &self,
        key: &ItemKey,
        draft: &SnippetDraft,
    ) -> Result<SnippetDetail, SnippetError> {
        let id = self.snippet_write(|tx| save(tx, key, draft))?;
        self.snippet(&id)?.ok_or(SnippetError::NotFound)
    }

    /// Moves live snippets to Recently Deleted; returns how many moved.
    pub fn delete_snippets(&self, key: &ItemKey, ids: &[String]) -> Result<usize, SnippetError> {
        self.snippet_write(|tx| {
            let now = crate::now_ms();
            let mut deleted = 0;
            for id in ids {
                if let Ok(record) = live(tx, id) {
                    mark_deleted(tx, key, record, now)?;
                    deleted += 1;
                }
            }
            Ok(deleted)
        })
    }

    /// Brings snippets back from Recently Deleted.
    pub fn restore_snippets(&self, key: &ItemKey, ids: &[String]) -> Result<usize, SnippetError> {
        self.snippet_write(|tx| {
            let mut restored = 0;
            for id in ids {
                let Some(record) =
                    record(tx, id)?.filter(|record| !record.purged && record.deleted_at.is_some())
                else {
                    continue;
                };
                write_snippet(
                    tx,
                    key,
                    SnippetRecord {
                        deleted_at: None,
                        ..record
                    },
                )?;
                restored += 1;
            }
            Ok(restored)
        })
    }

    /// Erases snippets permanently on every device.
    pub fn purge_snippets(&self, key: &ItemKey, ids: &[String]) -> Result<usize, SnippetError> {
        self.snippet_write(|tx| {
            let now = crate::now_ms();
            let mut purged = 0;
            for id in ids {
                if let Some(record) = record(tx, id)?.filter(|record| !record.purged) {
                    purge(tx, key, record, now)?;
                    purged += 1;
                }
            }
            Ok(purged)
        })
    }

    /// Purges snippets that have been in Recently Deleted for the full window.
    pub fn purge_expired_snippets(&self, key: &ItemKey, now_ms: i64) -> Result<usize, StoreError> {
        let mut conn = self.conn()?;
        let tx = write_tx(&mut conn)?;
        let ids: Vec<String> = tx
            .prepare(
                "SELECT id FROM snippets WHERE purged=0 AND deleted_at IS NOT NULL \
                 AND deleted_at<=?1",
            )?
            .query_map([now_ms.saturating_sub(RECENTLY_DELETED_MS)], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        for id in &ids {
            if let Some(record) = record(&tx, id)? {
                purge(&tx, key, record, now_ms)?;
            }
        }
        tx.commit()?;
        Ok(ids.len())
    }

    /// Files snippets in a collection (`None` = Unfiled), appended in order.
    pub fn move_snippets(
        &self,
        key: &ItemKey,
        ids: &[String],
        collection_id: Option<&str>,
    ) -> Result<usize, SnippetError> {
        self.snippet_write(|tx| {
            if let Some(collection_id) = collection_id {
                live_collection(tx, collection_id)?;
            }
            let mut moved = 0;
            for id in ids {
                let Some(record) = record(tx, id)?.filter(|record| !record.purged) else {
                    continue;
                };
                if record.collection_id.as_deref() != collection_id {
                    move_to(tx, key, record, collection_id)?;
                    moved += 1;
                }
            }
            Ok(moved)
        })
    }

    /// Places a snippet right after `after_id` in its collection, or first.
    pub fn reorder_snippet(
        &self,
        key: &ItemKey,
        id: &str,
        after_id: Option<&str>,
    ) -> Result<(), SnippetError> {
        self.snippet_write(|tx| {
            let record = live(tx, id)?;
            let low = match after_id {
                Some(after_id) => {
                    let after = live(tx, after_id)?;
                    if after.collection_id != record.collection_id {
                        return Err(SnippetError::InvalidCollection);
                    }
                    Some(after.order_key)
                }
                None => None,
            };
            let high: Option<String> = tx.query_row(
                "SELECT MIN(order_key) FROM snippets WHERE collection_id IS ?1 AND id<>?2 \
                 AND (?3 IS NULL OR order_key>?3)",
                params![record.collection_id, id, low],
                |r| r.get(0),
            )?;
            let order_key = order_key_between(low.as_deref(), high.as_deref());
            write_snippet(
                tx,
                key,
                SnippetRecord {
                    order_key,
                    ..record
                },
            )?;
            Ok(())
        })
    }

    /// Pins a snippet after `after_id` in the Pinned section (at the end
    /// without one), or unpins it.
    pub fn set_snippet_pinned(
        &self,
        key: &ItemKey,
        id: &str,
        pinned: bool,
        after_id: Option<&str>,
    ) -> Result<(), SnippetError> {
        self.snippet_write(|tx| {
            let record = live(tx, id)?;
            let pin_order = match (pinned, after_id) {
                (false, _) => None,
                (true, None) => Some(last_pin_order(tx)?),
                (true, Some(after_id)) => {
                    let low = live(tx, after_id)?
                        .pin_order
                        .ok_or(SnippetError::NotFound)?;
                    let high: Option<f64> = tx.query_row(
                        "SELECT MIN(pin_order) FROM snippets WHERE pinned=1 AND id<>?1 \
                         AND pin_order>?2",
                        params![id, low],
                        |r| r.get(0),
                    )?;
                    Some(high.map_or(low + 1.0, |high| low + (high - low) / 2.0))
                }
            };
            write_snippet(
                tx,
                key,
                SnippetRecord {
                    pinned,
                    pin_order,
                    ..record
                },
            )?;
            Ok(())
        })
    }

    /// Ids of template snippets whose body `refers` to snippet `id` by its
    /// abbreviation. The template parser stays with the caller.
    pub fn snippet_references(
        &self,
        id: &str,
        refers: &dyn Fn(&str) -> bool,
    ) -> Result<Vec<String>, StoreError> {
        let conn = self.conn()?;
        let Some(abbreviation) = record(&conn, id)?.and_then(|record| record.abbreviation) else {
            return Ok(Vec::new());
        };
        Ok(reference_candidates(&conn, &abbreviation, id)?
            .into_iter()
            .filter(|(_, text)| refers(text))
            .map(|(id, _)| id)
            .collect())
    }

    /// Renames (or clears) an abbreviation. With `rewrite`, every template
    /// body it changes gets a new revision in the same transaction; returns
    /// how many were rewritten.
    pub fn rename_snippet_abbreviation(
        &self,
        key: &ItemKey,
        id: &str,
        abbreviation: Option<&str>,
        rewrite: Option<BodyRewrite<'_>>,
    ) -> Result<usize, SnippetError> {
        self.snippet_write(|tx| {
            let record = live(tx, id)?;
            let wanted = abbreviation
                .map(str::trim)
                .filter(|wanted| !wanted.is_empty());
            check_abbreviation(tx, id, record.abbreviation.as_deref(), wanted)?;
            let previous = record.abbreviation.clone();
            write_snippet(
                tx,
                key,
                SnippetRecord {
                    abbreviation: wanted.map(str::to_owned),
                    ..record
                },
            )?;
            let mut rewritten = 0;
            if let (Some(rewrite), Some(previous)) = (rewrite, previous) {
                for (reference, text) in reference_candidates(tx, &previous, id)? {
                    let Some(updated) = rewrite(&text).filter(|updated| *updated != text) else {
                        continue;
                    };
                    if updated.len() > MAX_BODY_BYTES {
                        return Err(SnippetError::TooLarge);
                    }
                    write_body(tx, key, &reference, updated, None, None)?;
                    rewritten += 1;
                }
            }
            publish_copies(tx, key)?;
            Ok(rewritten)
        })
    }

    /// Live collections in display order.
    pub fn snippet_collections(&self) -> Result<Vec<SnippetCollection>, StoreError> {
        Ok(self
            .conn()?
            .prepare(
                "SELECT c.id,c.name,c.icon,c.accent,c.order_key,(SELECT COUNT(*) FROM snippets s \
                 WHERE s.collection_id=c.id AND s.purged=0 AND s.deleted_at IS NULL) \
                 FROM snippet_collections c WHERE c.deleted_at IS NULL ORDER BY c.order_key,c.id",
            )?
            .query_map([], |r| {
                Ok(SnippetCollection {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    icon: r.get(2)?,
                    accent: r.get(3)?,
                    order_key: r.get(4)?,
                    snippet_count: r.get(5)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?)
    }

    pub fn save_snippet_collection(
        &self,
        key: &ItemKey,
        draft: &CollectionDraft,
    ) -> Result<SnippetCollection, SnippetError> {
        let id = self.snippet_write(|tx| {
            let name = draft.name.trim();
            if name.is_empty()
                || !single_line(name, MAX_COLLECTION_NAME_CHARS)
                || draft.icon.trim().is_empty()
                || !single_line(&draft.icon, MAX_COLLECTION_STYLE_CHARS)
                || draft.accent.trim().is_empty()
                || !single_line(&draft.accent, MAX_COLLECTION_STYLE_CHARS)
            {
                return Err(SnippetError::InvalidCollection);
            }
            let existing = match &draft.id {
                Some(id) => Some(live_collection(tx, id)?),
                None => None,
            };
            let live: Vec<(String, String)> = tx
                .prepare("SELECT id,name FROM snippet_collections WHERE deleted_at IS NULL")?
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<rusqlite::Result<_>>()?;
            let folded = abbreviation::normalize(name);
            if let Some((collection_id, _)) = live.iter().find(|(id, other)| {
                Some(id) != draft.id.as_ref() && abbreviation::normalize(other) == folded
            }) {
                return Err(SnippetError::CollectionNameTaken {
                    collection_id: collection_id.clone(),
                });
            }
            if existing.is_none() && live.len() >= MAX_COLLECTIONS {
                return Err(SnippetError::LimitReached);
            }
            let (id, order_key) = match existing {
                Some(existing) => (existing.id, existing.order_key),
                None => {
                    let last: Option<String> =
                        tx.query_row("SELECT MAX(order_key) FROM snippet_collections", [], |r| {
                            r.get(0)
                        })?;
                    (
                        uuid::Uuid::new_v4().to_string(),
                        order_key_between(last.as_deref(), None),
                    )
                }
            };
            organization::write(
                tx,
                key,
                Record::SnippetCollection(SnippetCollectionRecord {
                    id: id.clone(),
                    name: name.to_owned(),
                    icon: draft.icon.trim().to_owned(),
                    accent: draft.accent.trim().to_owned(),
                    order_key,
                    deleted_at: None,
                }),
            )?;
            Ok(id)
        })?;
        self.snippet_collections()?
            .into_iter()
            .find(|collection| collection.id == id)
            .ok_or(SnippetError::NotFound)
    }

    /// Deletes a collection; its snippets move to Unfiled or, with
    /// `delete_snippets`, to Recently Deleted.
    pub fn delete_snippet_collection(
        &self,
        key: &ItemKey,
        id: &str,
        delete_snippets: bool,
    ) -> Result<(), SnippetError> {
        self.snippet_write(|tx| {
            let collection = live_collection(tx, id)?;
            let now = crate::now_ms();
            for record in filed_in(tx, id)? {
                if !delete_snippets {
                    move_to(tx, key, record, None)?;
                } else if record.deleted_at.is_none() {
                    mark_deleted(tx, key, record, now)?;
                }
            }
            organization::write(
                tx,
                key,
                Record::SnippetCollection(SnippetCollectionRecord {
                    deleted_at: Some(now),
                    ..collection
                }),
            )?;
            Ok(())
        })
    }

    /// Moves every snippet of `from` into `into` and deletes `from`.
    pub fn merge_snippet_collections(
        &self,
        key: &ItemKey,
        from: &str,
        into: &str,
    ) -> Result<usize, SnippetError> {
        self.snippet_write(|tx| {
            if from == into {
                return Err(SnippetError::InvalidCollection);
            }
            let source = live_collection(tx, from)?;
            live_collection(tx, into)?;
            let records = filed_in(tx, from)?;
            let moved = records.len();
            for record in records {
                move_to(tx, key, record, Some(into))?;
            }
            organization::write(
                tx,
                key,
                Record::SnippetCollection(SnippetCollectionRecord {
                    deleted_at: Some(crate::now_ms()),
                    ..source
                }),
            )?;
            Ok(moved)
        })
    }

    /// Ranked snippet matches for a typed query (spec A-3): exact
    /// abbreviation, abbreviation prefix, title, then full text.
    pub fn quick_search_snippets(
        &self,
        query: &str,
        limit: u32,
    ) -> Result<Vec<SnippetHit>, StoreError> {
        let query = query.trim();
        if query.is_empty() {
            return Ok(Vec::new());
        }
        let limit = copypaste_ipc::clamp_page(limit, copypaste_ipc::DEFAULT_SEARCH_PAGE);
        let conn = self.conn()?;
        let mut tiers: HashMap<String, (SearchTier, usize)> = HashMap::new();
        let mut found = |id: String, tier: SearchTier| {
            let position = tiers.len();
            tiers.entry(id).or_insert((tier, position));
        };
        if !query.chars().any(char::is_whitespace) {
            let wanted = abbreviation::normalize(query);
            let rows: Vec<(String, String)> = conn
                .prepare(
                    "SELECT id,abbreviation_key FROM snippets WHERE abbreviation_key>=?1 \
                     AND abbreviation_key<?2 AND deleted_at IS NULL AND purged=0 \
                     ORDER BY abbreviation_key,id LIMIT ?3",
                )?
                .query_map(params![wanted, format!("{wanted}\u{10FFFF}"), limit], |r| {
                    Ok((r.get(0)?, r.get(1)?))
                })?
                .collect::<rusqlite::Result<_>>()?;
            for (id, abbreviation_key) in rows {
                if let Some(tier) = ranking::abbreviation_tier(&wanted, &abbreviation_key) {
                    found(id, tier);
                }
            }
        }
        if let Some(expression) = super::search::sanitize_fts5_query(query) {
            let mut statement = conn.prepare(
                "SELECT f.id FROM snippet_fts f JOIN snippets s ON s.id=f.id \
                 WHERE snippet_fts MATCH ?1 AND s.purged=0 AND s.deleted_at IS NULL \
                 ORDER BY rank LIMIT ?2",
            )?;
            for (expression, tier) in [
                (format!("title : ({expression})"), SearchTier::Title),
                (expression, SearchTier::FullText),
            ] {
                let ids = statement
                    .query_map(params![expression, limit], |r| r.get::<_, String>(0))?
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                for id in ids {
                    found(id, tier);
                }
            }
        }
        if tiers.is_empty() {
            return Ok(Vec::new());
        }
        let placeholders = vec!["?"; tiers.len()].join(",");
        let ids: Vec<&String> = tiers.keys().collect();
        let mut hits = conn
            .prepare(&format!("{SUMMARY_SELECT} WHERE s.id IN ({placeholders})"))?
            .query_map(params_from_iter(ids), summary)?
            .map(|row| {
                row.map(|snippet| {
                    let (tier, position) = tiers[&snippet.id];
                    let signals = RankSignals {
                        tier,
                        pinned: snippet.pinned,
                        last_used_at: snippet.last_used_at,
                        position,
                    };
                    (signals, SnippetHit { snippet, tier })
                })
            })
            .collect::<rusqlite::Result<Vec<_>>>()?;
        hits.sort_by(|a, b| ranking::compare(&a.0, &b.0));
        Ok(hits
            .into_iter()
            .take(limit as usize)
            .map(|(_, hit)| hit)
            .collect())
    }

    /// Groups of live snippets sharing a normalized abbreviation, as left by
    /// concurrent offline edits; each group needs the user's choice.
    pub fn duplicate_snippet_abbreviations(&self) -> Result<Vec<Vec<String>>, StoreError> {
        let rows: Vec<(String, String)> = self
            .conn()?
            .prepare(
                "SELECT abbreviation_key,id FROM snippets WHERE deleted_at IS NULL AND purged=0 \
                 AND abbreviation_key IN (SELECT abbreviation_key FROM snippets \
                 WHERE abbreviation_key IS NOT NULL AND deleted_at IS NULL AND purged=0 \
                 GROUP BY abbreviation_key HAVING COUNT(*)>1) ORDER BY abbreviation_key,id",
            )?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let mut groups: Vec<Vec<String>> = Vec::new();
        let mut seen = HashSet::new();
        for (abbreviation_key, id) in rows {
            if seen.insert(abbreviation_key) {
                groups.push(Vec::new());
            }
            if let Some(group) = groups.last_mut() {
                group.push(id);
            }
        }
        Ok(groups)
    }

    /// Bumps the local usage counters; `false` for an unknown snippet.
    pub fn record_snippet_use(&self, id: &str, at_ms: i64) -> Result<bool, StoreError> {
        Ok(self.conn()?.execute(
            "INSERT INTO snippet_usage(id,last_used_at,use_count) SELECT ?1,?2,1 \
             WHERE EXISTS(SELECT 1 FROM snippets WHERE id=?1 AND purged=0) \
             ON CONFLICT(id) DO UPDATE SET last_used_at=MAX(last_used_at,excluded.last_used_at),\
             use_count=use_count+1",
            params![id, at_ms],
        )? > 0)
    }

    /// Snippets counted against the limits: live and recently deleted.
    pub fn snippet_count(&self) -> Result<usize, StoreError> {
        let conn = self.conn()?;
        snippet_total(&conn)
    }

    /// Writes the records of conflict copies an apply projected without a
    /// key. Cheap when there are none: one range seek.
    pub fn publish_snippet_conflicts(&self, key: &ItemKey) -> Result<usize, StoreError> {
        let pending: bool = self.conn()?.query_row(
            "SELECT EXISTS(SELECT 1 FROM snippets p WHERE p.id>=?1 AND p.id<?2 \
             AND NOT EXISTS(SELECT 1 FROM organization_records r WHERE r.id=p.record_id))",
            [conflict::COPY_RANGE.0, conflict::COPY_RANGE.1],
            |r| r.get(0),
        )?;
        if !pending {
            return Ok(0);
        }
        let mut conn = self.conn()?;
        let tx = write_tx(&mut conn)?;
        let published = publish_copies(&tx, key)?;
        tx.commit()?;
        Ok(published)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_keys_always_fall_strictly_between_their_neighbours() {
        let low = order_key_between(None, None);
        let mut high = order_key_between(Some(&low), None);
        assert!(low < high);
        for _ in 0..200 {
            let key = order_key_between(Some(&low), Some(&high));
            assert!(low < key && key < high, "{low} {key} {high}");
            assert!(!key.ends_with('0') && key.len() <= 128);
            high = key;
        }
        let first = order_key_between(None, Some(&low));
        assert!(first < low && !first.ends_with('0'));
    }
}
