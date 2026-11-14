//! Replicated organization records and their transactional query projections.
//!
//! Each tag, membership, pin and library record is an independent LWW record.
//! Existing P2P and encrypted cloud transports carry them without changing
//! clip timestamps. Kinds added after tags, memberships and pins are stored
//! with `kind_class = 2`, so a sender can withhold them from older peers that
//! would decline them on every session.
use std::collections::BTreeMap;

use copypaste_ipc::{
    HistoryTag, HistoryTagMutation, HistoryTags, MAX_HISTORY_TAGS, MAX_ITEM_TAG_IDS, MAX_TAG_CLIPS,
    MAX_TAG_NAME_CHARS,
};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{connection::write_tx, IncomingItem, Store, StoreError};

mod clip_notes;
mod module_records;
mod preferences;
mod rules;
mod snippets;
mod transforms;

pub use clip_notes::{ClipNoteRecord, ModuleOrigin, Provenance, ProvenanceKind, TransformRef};
pub use module_records::{
    ModuleRecord, ModuleRecordPage, MAX_MODULE_RECORDS, MAX_MODULE_RECORD_KEY_BYTES,
    MAX_MODULE_RECORD_PAGE, MAX_MODULE_RECORD_VALUE_BYTES,
};
pub use preferences::{PreferenceRecord, SetMemberRecord};
pub use rules::{ContentRuleRecord, RuleOrigin, RulePlacementRecord};
pub use snippets::{SnippetBodyRecord, SnippetCollectionRecord, SnippetRecord};
pub use transforms::PipelineRecord;

pub(crate) const CONTENT_TYPE: &str = "application/vnd.copypaste.organization+json";
const PREFIX: &str = "org:v1:";
/// Tags, memberships and pins: what every build with organization records reads.
const LEGACY_KIND_CLASS: i64 = 1;
const LIBRARY_KIND_CLASS: i64 = 2;
const LEGACY_RECORD_BYTES: usize = 2048;
/// The largest kind; each kind's own bound is checked once it is parsed.
const MAX_RECORD_BYTES: usize = snippets::BODY_BYTES;
const _: () = assert!(module_records::MODULE_RECORD_BYTES <= MAX_RECORD_BYTES);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Record {
    Tag {
        id: String,
        name: String,
        deleted: bool,
    },
    Membership {
        clip_id: String,
        tag_id: String,
        assigned: bool,
    },
    Pin {
        clip_id: String,
        pinned: bool,
        order: Option<f64>,
    },
    Snippet(SnippetRecord),
    SnippetBody(SnippetBodyRecord),
    SnippetCollection(SnippetCollectionRecord),
    ContentRule(ContentRuleRecord),
    ContentRulePlacement(RulePlacementRecord),
    Pipeline(PipelineRecord),
    Preference(PreferenceRecord),
    SetMember(SetMemberRecord),
    ClipNote(ClipNoteRecord),
    ModuleRecord(ModuleRecord),
}

/// The identity of one library record, for reading its projection back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordKey<'a> {
    Snippet(&'a str),
    SnippetBody(&'a str),
    SnippetCollection(&'a str),
    ContentRule(&'a str),
    ContentRulePlacement(&'a str),
    Pipeline(&'a str),
    Preference { namespace: &'a str, key: &'a str },
    SetMember { set: &'a str, member: &'a str },
    ClipNote(&'a str),
    ModuleRecord { module_id: &'a str, key: &'a str },
}

impl RecordKey<'_> {
    fn key(self) -> String {
        match self {
            Self::Snippet(id) => snippets::snippet_key(id),
            Self::SnippetBody(id) => snippets::body_key(id),
            Self::SnippetCollection(id) => snippets::collection_key(id),
            Self::ContentRule(id) => rules::rule_key(id),
            Self::ContentRulePlacement(id) => rules::placement_key(id),
            Self::Pipeline(id) => transforms::pipeline_key(id),
            Self::Preference { namespace, key } => preferences::preference_key(namespace, key),
            Self::SetMember { set, member } => preferences::member_key(set, member),
            Self::ClipNote(clip_id) => clip_notes::note_key(clip_id),
            Self::ModuleRecord { module_id, key } => {
                module_records::module_record_key(module_id, key)
            }
        }
    }
}

pub(crate) fn is_record_id(id: &str) -> bool {
    id.starts_with(PREFIX)
}

fn record_id(key: &str) -> String {
    format!("{PREFIX}{}", hex::encode(Sha256::digest(key.as_bytes())))
}

/// The id of a clip's synced note record.
pub(crate) fn clip_note_record_id(clip_id: &str) -> String {
    record_id(&clip_notes::note_key(clip_id))
}

fn identifier(id: &str) -> bool {
    !id.is_empty() && id.len() <= 128 && !is_record_id(id)
}

/// Single-line user text: bounded, and free of control characters.
fn line(text: &str, max_chars: usize) -> bool {
    text.chars().count() <= max_chars && !text.chars().any(char::is_control)
}

fn required_line(text: &str, max_chars: usize) -> bool {
    !text.trim().is_empty() && line(text, max_chars)
}

impl Record {
    fn key(&self) -> String {
        match self {
            Self::Tag { id, .. } => format!("tag:{id}"),
            Self::Membership {
                clip_id, tag_id, ..
            } => format!("membership:{}:{clip_id}:{tag_id}", clip_id.len()),
            Self::Pin { clip_id, .. } => format!("pin:{clip_id}"),
            Self::Snippet(r) => snippets::snippet_key(&r.id),
            Self::SnippetBody(r) => snippets::body_key(&r.id),
            Self::SnippetCollection(r) => snippets::collection_key(&r.id),
            Self::ContentRule(r) => rules::rule_key(&r.id),
            Self::ContentRulePlacement(r) => rules::placement_key(&r.id),
            Self::Pipeline(r) => transforms::pipeline_key(&r.id),
            Self::Preference(r) => preferences::preference_key(&r.namespace, &r.key),
            Self::SetMember(r) => preferences::member_key(&r.set, &r.member),
            Self::ClipNote(r) => clip_notes::note_key(&r.clip_id),
            Self::ModuleRecord(r) => module_records::module_record_key(&r.module_id, &r.key),
        }
    }
    fn id(&self) -> String {
        record_id(&self.key())
    }
    fn kind_class(&self) -> i64 {
        match self {
            Self::Tag { .. } | Self::Membership { .. } | Self::Pin { .. } => LEGACY_KIND_CLASS,
            _ => LIBRARY_KIND_CLASS,
        }
    }
    fn max_bytes(&self) -> usize {
        match self {
            Self::Tag { .. } | Self::Membership { .. } | Self::Pin { .. } => LEGACY_RECORD_BYTES,
            Self::Snippet(_) => snippets::SNIPPET_BYTES,
            Self::SnippetBody(_) => snippets::BODY_BYTES,
            Self::SnippetCollection(_) => snippets::COLLECTION_BYTES,
            Self::ContentRule(_) => rules::RULE_BYTES,
            Self::ContentRulePlacement(_) => rules::PLACEMENT_BYTES,
            Self::Pipeline(_) => transforms::PIPELINE_BYTES,
            Self::Preference(_) => preferences::PREFERENCE_BYTES,
            Self::SetMember(_) => preferences::MEMBER_BYTES,
            Self::ClipNote(_) => clip_notes::NOTE_BYTES,
            Self::ModuleRecord(_) => module_records::MODULE_RECORD_BYTES,
        }
    }
    fn valid(&self) -> bool {
        match self {
            Self::Tag { id, name, .. } => {
                identifier(id)
                    && !name.trim().is_empty()
                    && name == name.trim()
                    && name.chars().count() <= MAX_TAG_NAME_CHARS
                    && !name.chars().any(char::is_control)
            }
            Self::Membership {
                clip_id, tag_id, ..
            } => identifier(clip_id) && identifier(tag_id),
            Self::Pin {
                clip_id,
                pinned,
                order,
            } => {
                identifier(clip_id)
                    && (if *pinned {
                        order.is_some_and(f64::is_finite)
                    } else {
                        order.is_none()
                    })
            }
            Self::Snippet(r) => r.valid(),
            Self::SnippetBody(r) => r.valid(),
            Self::SnippetCollection(r) => r.valid(),
            Self::ContentRule(r) => r.valid(),
            Self::ContentRulePlacement(r) => r.valid(),
            Self::Pipeline(r) => r.valid(),
            Self::Preference(r) => r.valid(),
            Self::SetMember(r) => r.valid(),
            Self::ClipNote(r) => r.valid(),
            Self::ModuleRecord(r) => r.valid(),
        }
    }
    /// Whether the local version holds a value that may only tighten (a purge,
    /// a revocation, a concurrent hide) which this version would loosen.
    fn yields_to_local(
        &self,
        tx: &Connection,
        incoming: &IncomingItem<'_>,
    ) -> Result<bool, StoreError> {
        match self {
            Self::Snippet(r) => snippets::keeps_purge(tx, r),
            Self::SnippetBody(r) => snippets::keeps_purged_body(tx, r),
            Self::ClipNote(r) => clip_notes::keeps_hidden(tx, r),
            Self::ContentRule(r) => {
                super::rules::keeps_deletion(tx, r, incoming.origin_device_id, incoming.created_at)
            }
            _ => Ok(false),
        }
    }
    fn project(&self, tx: &Connection, record_id: &str, stamp: i64) -> Result<(), StoreError> {
        match self {
            Self::Tag { id, name, deleted } => {
                tx.execute("INSERT INTO history_tags(id,name,deleted) VALUES (?1,?2,?3) ON CONFLICT(id) DO UPDATE SET name=excluded.name, deleted=excluded.deleted", params![id,name,deleted])?;
            }
            Self::Membership {
                clip_id,
                tag_id,
                assigned,
            } => {
                tx.execute("INSERT INTO history_clip_tags(clip_id,tag_id,assigned) VALUES (?1,?2,?3) ON CONFLICT(clip_id,tag_id) DO UPDATE SET assigned=excluded.assigned", params![clip_id,tag_id,assigned])?;
            }
            Self::Pin {
                clip_id,
                pinned,
                order,
            } => {
                tx.execute("INSERT INTO history_pin_state(clip_id,pinned,pin_order,updated_at) VALUES (?1,?2,?3,?4) ON CONFLICT(clip_id) DO UPDATE SET pinned=excluded.pinned,pin_order=excluded.pin_order,updated_at=excluded.updated_at", params![clip_id,pinned,order,stamp])?;
                apply_pin_projection(tx, clip_id)?;
            }
            Self::Snippet(r) => r.project(tx, record_id, stamp)?,
            Self::SnippetBody(r) => r.project(tx, record_id, stamp)?,
            Self::SnippetCollection(r) => r.project(tx, record_id)?,
            Self::ContentRule(r) => r.project(tx, record_id, stamp)?,
            Self::ContentRulePlacement(r) => r.project(tx, record_id)?,
            Self::Pipeline(r) => r.project(tx, record_id, stamp)?,
            Self::Preference(r) => r.project(tx, record_id, stamp)?,
            Self::SetMember(r) => r.project(tx, record_id, stamp)?,
            Self::ClipNote(r) => r.project(tx, record_id)?,
            Self::ModuleRecord(r) => r.project(tx, record_id, stamp)?,
        }
        Ok(())
    }
}

pub(super) fn apply_pin_projection(tx: &Connection, id: &str) -> Result<(), StoreError> {
    tx.execute("UPDATE clipboard_items SET (pinned,pin_order,pin_updated_at) = (SELECT pinned,pin_order,updated_at FROM history_pin_state WHERE clip_id=?1) WHERE id=?1 AND deleted=0 AND EXISTS (SELECT 1 FROM history_pin_state WHERE clip_id=?1 AND updated_at>=clipboard_items.pin_updated_at)", [id])?;
    Ok(())
}

fn parse(id: &str, text: &str) -> Option<Record> {
    if text.len() > MAX_RECORD_BYTES {
        return None;
    }
    let record: Record = serde_json::from_str(text).ok()?;
    (text.len() <= record.max_bytes() && record.valid() && record.id() == id).then_some(record)
}

/// A tombstone carries no content, so only its id and shape are checked here;
/// the store accepts it only for a clip note (see `clip_notes`).
pub(crate) fn valid_wire(id: &str, content_type: &str, content: &str, deleted: bool) -> bool {
    if content_type != CONTENT_TYPE {
        return !is_record_id(id);
    }
    if deleted {
        return is_record_id(id) && content.is_empty();
    }
    parse(id, content).is_some()
}

pub(super) fn upsert_in_tx(
    tx: &Connection,
    incoming: &IncomingItem<'_>,
) -> Result<bool, StoreError> {
    if incoming.deleted {
        return clip_notes::apply_tombstone(tx, incoming);
    }
    let Some(record) = incoming
        .search_text
        .and_then(|text| parse(incoming.id, text))
    else {
        return Ok(false);
    };
    if record.yields_to_local(tx, incoming)? {
        restamp(tx, incoming.id, incoming.created_at)?;
        return Ok(true);
    }
    // Projected first, so a projection sees the version it replaces.
    record.project(tx, incoming.id, incoming.created_at)?;
    tx.execute("INSERT INTO organization_records(id,content_ciphertext,nonce,content_hash,created_at,origin_device_id,kind_class,deleted) VALUES (?1,?2,?3,?4,?5,?6,?7,0) ON CONFLICT(id) DO UPDATE SET content_ciphertext=excluded.content_ciphertext,nonce=excluded.nonce,content_hash=excluded.content_hash,created_at=excluded.created_at,origin_device_id=excluded.origin_device_id,kind_class=excluded.kind_class,deleted=0", params![incoming.id,incoming.content_ciphertext,incoming.nonce,incoming.content_hash,incoming.created_at,incoming.origin_device_id,record.kind_class()])?;
    Ok(true)
}

/// Writes the local version back, unchanged, as newer than `beaten`: the
/// sealed content binds only the id, so no key is needed, and every peer that
/// took the loosening version receives the restrictive one after it.
fn restamp(tx: &Connection, id: &str, beaten: i64) -> Result<(), StoreError> {
    let local: i64 = tx.query_row(
        "SELECT created_at FROM organization_records WHERE id=?1",
        [id],
        |r| r.get(0),
    )?;
    let stamp = local.max(beaten).saturating_add(1).max(crate::now_ms());
    tx.execute(
        "UPDATE organization_records SET created_at=?2, origin_device_id='' WHERE id=?1",
        params![id, stamp],
    )?;
    Ok(())
}

pub(crate) fn write(
    tx: &Connection,
    key: &crate::ItemKey,
    record: Record,
) -> Result<(), StoreError> {
    if !record.valid() {
        return Err(StoreError::InvalidHistoryQuery);
    }
    let id = record.id();
    let text = serde_json::to_string(&record).map_err(|_| StoreError::InvalidHistoryQuery)?;
    if text.len() > record.max_bytes() {
        return Err(StoreError::InvalidHistoryQuery);
    }
    let hash = super::compute_content_hash(text.as_bytes());
    let previous: Option<(i64, String, bool)> = tx
        .query_row(
            "SELECT created_at,content_hash,deleted FROM organization_records WHERE id=?1",
            [&id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    if previous
        .as_ref()
        .is_some_and(|(_, previous_hash, deleted)| !deleted && previous_hash == &hash)
    {
        return Ok(());
    }
    let pin_stamp = if let Record::Pin { clip_id, .. } = &record {
        tx.query_row(
            "SELECT pin_updated_at FROM clipboard_items WHERE id=?1",
            [clip_id],
            |r| r.get::<_, i64>(0),
        )
        .optional()?
        .unwrap_or(0)
    } else {
        0
    };
    let stamp = previous
        .map_or(crate::now_ms(), |(stamp, _, _)| {
            stamp.saturating_add(1).max(crate::now_ms())
        })
        .max(pin_stamp);
    let nonce = Vec::<u8>::new();
    let bytes = crate::binary::seal(text.as_bytes(), key, &id)
        .map_err(|_| StoreError::InvalidHistoryQuery)?;
    upsert_in_tx(
        tx,
        &IncomingItem {
            id: &id,
            content_ciphertext: Some(&bytes),
            nonce: Some(&nonce),
            content_type: CONTENT_TYPE,
            content_hash: &hash,
            created_at: stamp,
            deleted: false,
            origin_device_id: "",
            app_bundle_id: None,
            app_name: None,
            pinned: false,
            pin_order: None,
            pin_updated_at: 0,
            search_text: Some(&text),
            payload_metadata: None,
        },
    )?;
    Ok(())
}

pub(super) fn write_pin(tx: &Connection, key: &crate::ItemKey, id: &str) -> Result<(), StoreError> {
    let state: Option<(bool, Option<f64>)> = tx
        .query_row(
            "SELECT pinned,pin_order FROM clipboard_items WHERE id=?1 AND deleted=0",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    if let Some((pinned, order)) = state {
        write(
            tx,
            key,
            Record::Pin {
                clip_id: id.into(),
                pinned,
                order,
            },
        )?;
    }
    Ok(())
}

/// The first id after every [`PREFIX`] id in byte order (`;` follows `:`), so
/// legacy copies are found by one primary-key range seek.
const PREFIX_END: &str = "org:v1;";

/// Moves organization records out of the clipboard history.
///
/// Builds without organization records stored a peer's record as an ordinary
/// binary clip under the record's own id, sealed exactly as
/// `organization_records` seals it. Once this build writes a record for that
/// id, the id lives in both tables: a peer session advertises both versions
/// and serves whichever row the read returns first, so the receiver drops it
/// as not matching the summary it planned from, every round. A copy is
/// adopted only when no record holds its id; an existing record was written
/// after the copy and is the version peers already planned against.
fn adopt_legacy_records(tx: &Connection, key: &crate::ItemKey) -> Result<(), StoreError> {
    type Legacy = (String, Option<Vec<u8>>, String, String, i64, bool, String);
    let legacy = tx
        .prepare(
            "SELECT id, content_ciphertext, content_type, content_hash, created_at, deleted, \
             origin_device_id FROM clipboard_items WHERE id >= ?1 AND id < ?2",
        )?
        .query_map([PREFIX, PREFIX_END], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
                r.get(6)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<Legacy>>>()?;
    for (id, sealed, content_type, hash, created_at, deleted, origin) in legacy {
        super::search::delete_fts_row_in_tx(tx, &id)?;
        tx.execute("DELETE FROM clipboard_items WHERE id=?1", [&id])?;
        let held: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM organization_records WHERE id=?1)",
            [&id],
            |r| r.get(0),
        )?;
        let Some(sealed) = sealed.filter(|_| !held && !deleted && content_type == CONTENT_TYPE)
        else {
            continue;
        };
        let Ok(text) = crate::binary::open(&sealed, key, &id) else {
            continue;
        };
        let Ok(text) = std::str::from_utf8(&text) else {
            continue;
        };
        upsert_in_tx(
            tx,
            &IncomingItem {
                id: &id,
                content_ciphertext: Some(&sealed),
                nonce: Some(&[]),
                content_type: CONTENT_TYPE,
                content_hash: &hash,
                created_at,
                deleted: false,
                origin_device_id: &origin,
                app_bundle_id: None,
                app_name: None,
                pinned: false,
                pin_order: None,
                pin_updated_at: 0,
                search_text: Some(text),
                payload_metadata: None,
            },
        )?;
    }
    Ok(())
}

impl Store {
    /// Prepares organization records for synchronization: legacy record
    /// copies leave the clipboard history first, then pins that predate
    /// organization records get one. The order matters, because a seeded pin
    /// record takes the id a legacy copy of a peer's pin record already holds.
    pub fn seed_pin_organization(&self, key: &crate::ItemKey) -> Result<(), StoreError> {
        let mut adopt = true;
        loop {
            let mut conn = self.conn()?;
            let tx = write_tx(&mut conn)?;
            if std::mem::take(&mut adopt) {
                adopt_legacy_records(&tx, key)?;
            }
            let ids = tx.prepare("SELECT id FROM clipboard_items WHERE deleted=0 AND (pinned=1 OR pin_updated_at>0) AND NOT EXISTS (SELECT 1 FROM history_pin_state WHERE clip_id=clipboard_items.id) LIMIT 100")?
                .query_map([],|r|r.get::<_,String>(0))?.collect::<rusqlite::Result<Vec<_>>>()?;
            if ids.is_empty() {
                tx.commit()?;
                return Ok(());
            }
            for id in ids {
                write_pin(&tx, key, &id)?;
            }
            tx.commit()?;
        }
    }

    pub fn history_tags(&self, clip_ids: &[String]) -> Result<HistoryTags, StoreError> {
        if clip_ids.len() > MAX_TAG_CLIPS {
            return Err(StoreError::InvalidHistoryQuery);
        }
        let conn = self.conn()?;
        let tags = catalogue(&conn)?;
        let assignments = assignments(&conn, clip_ids, None)?;
        Ok(HistoryTags { tags, assignments })
    }

    /// Name-ordered tag ids of each clip, bounded per clip for History pages.
    pub fn item_tag_ids(
        &self,
        clip_ids: &[String],
    ) -> Result<BTreeMap<String, Vec<String>>, StoreError> {
        let conn = self.conn()?;
        assignments(&conn, clip_ids, Some(MAX_ITEM_TAG_IDS))
    }

    pub fn mutate_history_tags(
        &self,
        key: &crate::ItemKey,
        mutation: &HistoryTagMutation,
    ) -> Result<(), StoreError> {
        let mut conn = self.conn()?;
        let tx = write_tx(&mut conn)?;
        match mutation {
            HistoryTagMutation::Save { id, name } => {
                let id = match id {
                    Some(id) => {
                        if !tx.query_row(
                            "SELECT EXISTS(SELECT 1 FROM history_tags WHERE id=?1 AND deleted=0)",
                            [id],
                            |r| r.get::<_, bool>(0),
                        )? {
                            return Err(StoreError::InvalidHistoryQuery);
                        }
                        id.clone()
                    }
                    None => {
                        if tx.query_row(
                            "SELECT COUNT(*) FROM history_tags WHERE deleted=0",
                            [],
                            |r| r.get::<_, usize>(0),
                        )? >= MAX_HISTORY_TAGS
                        {
                            return Err(StoreError::InvalidHistoryQuery);
                        }
                        uuid::Uuid::new_v4().to_string()
                    }
                };
                let name = name.trim().to_owned();
                if tx.query_row("SELECT EXISTS(SELECT 1 FROM history_tags WHERE name=?1 COLLATE NOCASE AND deleted=0 AND id<>?2)",params![name,id],|r|r.get::<_,bool>(0))? { return Err(StoreError::InvalidHistoryQuery); }
                write(
                    &tx,
                    key,
                    Record::Tag {
                        id,
                        name,
                        deleted: false,
                    },
                )?;
            }
            HistoryTagMutation::Delete { id } => {
                let name: Option<String> = tx
                    .query_row(
                        "SELECT name FROM history_tags WHERE id=?1 AND deleted=0",
                        [id],
                        |r| r.get(0),
                    )
                    .optional()?;
                if let Some(name) = name {
                    write(
                        &tx,
                        key,
                        Record::Tag {
                            id: id.clone(),
                            name,
                            deleted: true,
                        },
                    )?;
                }
            }
            HistoryTagMutation::Assign {
                clip_ids,
                tag_ids,
                assigned,
            } => {
                if clip_ids.len() > MAX_TAG_CLIPS
                    || tag_ids.len() > MAX_HISTORY_TAGS
                    || clip_ids.len().saturating_mul(tag_ids.len()) > 5000
                {
                    return Err(StoreError::InvalidHistoryQuery);
                }
                for tag_id in tag_ids {
                    if !tx.query_row(
                        "SELECT EXISTS(SELECT 1 FROM history_tags WHERE id=?1 AND deleted=0)",
                        [tag_id],
                        |r| r.get::<_, bool>(0),
                    )? {
                        return Err(StoreError::InvalidHistoryQuery);
                    }
                    for clip_id in clip_ids {
                        if !tx.query_row("SELECT EXISTS(SELECT 1 FROM clipboard_items WHERE id=?1 AND deleted=0 AND pinned=1)",[clip_id],|r|r.get::<_,bool>(0))? { return Err(StoreError::InvalidHistoryQuery); }
                        write(
                            &tx,
                            key,
                            Record::Membership {
                                clip_id: clip_id.clone(),
                                tag_id: tag_id.clone(),
                                assigned: *assigned,
                            },
                        )?;
                    }
                }
            }
        }
        tx.commit()?;
        Ok(())
    }
}

fn assignments(
    conn: &Connection,
    clip_ids: &[String],
    limit: Option<usize>,
) -> Result<BTreeMap<String, Vec<String>>, StoreError> {
    let limit = limit.map_or(-1, |limit| i64::try_from(limit).unwrap_or(i64::MAX));
    let mut statement = conn.prepare("SELECT ct.tag_id FROM history_clip_tags ct JOIN history_tags t ON t.id=ct.tag_id WHERE ct.clip_id=?1 AND ct.assigned=1 AND t.deleted=0 ORDER BY t.name COLLATE NOCASE,t.id LIMIT ?2")?;
    let mut assignments = BTreeMap::new();
    for id in clip_ids {
        assignments.insert(
            id.clone(),
            statement
                .query_map(params![id, limit], |r| r.get(0))?
                .collect::<rusqlite::Result<Vec<String>>>()?,
        );
    }
    Ok(assignments)
}

pub(super) fn catalogue(conn: &Connection) -> Result<Vec<HistoryTag>, StoreError> {
    let tags = conn.prepare("SELECT id,name FROM history_tags WHERE deleted=0 ORDER BY name COLLATE NOCASE,id LIMIT 501")?
            .query_map([], |r| Ok(HistoryTag { id:r.get(0)?,name:r.get(1)? }))?.collect::<rusqlite::Result<Vec<_>>>()?;
    if tags.len() > MAX_HISTORY_TAGS {
        return Err(StoreError::InvalidHistoryQuery);
    }
    Ok(tags)
}

impl Store {
    /// Writes one library record. Tags, memberships and pins have their own
    /// invariants and go through their dedicated operations instead.
    pub fn write_organization_record(
        &self,
        key: &crate::ItemKey,
        record: Record,
    ) -> Result<(), StoreError> {
        self.write_organization_records(key, [record])
    }

    /// Writes library records in one transaction: all of them or none.
    pub fn write_organization_records(
        &self,
        key: &crate::ItemKey,
        records: impl IntoIterator<Item = Record>,
    ) -> Result<(), StoreError> {
        let mut conn = self.conn()?;
        let tx = write_tx(&mut conn)?;
        for record in records {
            if record.kind_class() == LEGACY_KIND_CLASS {
                return Err(StoreError::InvalidHistoryQuery);
            }
            write(&tx, key, record)?;
        }
        tx.commit()?;
        Ok(())
    }

    /// The stamp of the stored version of one library record: the base an
    /// edit records, so a concurrent restrictive change is recognised.
    pub fn organization_record_stamp(&self, key: RecordKey<'_>) -> Result<Option<i64>, StoreError> {
        let conn = self.conn()?;
        Ok(conn
            .query_row(
                "SELECT created_at FROM organization_records WHERE id=?1 AND deleted=0",
                [record_id(&key.key())],
                |r| r.get(0),
            )
            .optional()?)
    }

    /// The projected value of one library record, as every query sees it.
    pub fn organization_record(&self, key: RecordKey<'_>) -> Result<Option<Record>, StoreError> {
        let conn = self.conn()?;
        read(&conn, key)
    }

    /// Tombstones the synced note of `clip_id`; `false` when it has none.
    pub fn tombstone_clip_note(&self, clip_id: &str) -> Result<bool, StoreError> {
        let mut conn = self.conn()?;
        let tx = write_tx(&mut conn)?;
        let changed = clip_notes::tombstone(&tx, clip_id)?;
        tx.commit()?;
        Ok(changed)
    }

    /// Drops notes whose clip has been missing for the orphan window.
    pub fn prune_orphan_clip_notes(&self, now_ms: i64) -> Result<usize, StoreError> {
        let mut conn = self.conn()?;
        let tx = write_tx(&mut conn)?;
        let pruned = clip_notes::prune_orphans(&tx, now_ms)?;
        tx.commit()?;
        Ok(pruned)
    }
}

/// [`Store::organization_record`] inside a caller's transaction.
pub(crate) fn read(conn: &Connection, key: RecordKey<'_>) -> Result<Option<Record>, StoreError> {
    Ok(match key {
        RecordKey::Snippet(id) => snippets::snippet(conn, id)?.map(Record::Snippet),
        RecordKey::SnippetBody(id) => snippets::body(conn, id)?.map(Record::SnippetBody),
        RecordKey::SnippetCollection(id) => {
            snippets::collection(conn, id)?.map(Record::SnippetCollection)
        }
        RecordKey::ContentRule(id) => rules::rule(conn, id)?.map(Record::ContentRule),
        RecordKey::ContentRulePlacement(id) => {
            rules::placement(conn, id)?.map(Record::ContentRulePlacement)
        }
        RecordKey::Pipeline(id) => transforms::pipeline(conn, id)?.map(Record::Pipeline),
        RecordKey::Preference { namespace, key } => {
            preferences::preference(conn, namespace, key)?.map(Record::Preference)
        }
        RecordKey::SetMember { set, member } => {
            preferences::member(conn, set, member)?.map(Record::SetMember)
        }
        RecordKey::ClipNote(id) => clip_notes::note(conn, id)?.map(Record::ClipNote),
        RecordKey::ModuleRecord { module_id, key } => {
            module_records::module_record(conn, module_id, key)?.map(Record::ModuleRecord)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{item, store, T0};
    use super::*;

    fn kind_classes(store: &Store) -> Vec<i64> {
        store
            .conn()
            .unwrap()
            .prepare("SELECT kind_class FROM organization_records ORDER BY kind_class")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap()
    }

    #[test]
    fn library_kinds_are_written_in_their_own_class_and_reject_unknown_fields() {
        let store = store();
        let key = crate::Keyring::from_secret(&[5; 32]).item_key();
        let clip = store.insert(item("pinned", T0)).unwrap();
        store.set_pinned_synced(&clip.id, true, &key).unwrap();
        let preference = Record::Preference(PreferenceRecord {
            namespace: "transforms".into(),
            key: "default".into(),
            value_json: "null".into(),
        });
        store
            .write_organization_record(&key, preference.clone())
            .unwrap();
        assert_eq!(
            kind_classes(&store),
            [LEGACY_KIND_CLASS, LIBRARY_KIND_CLASS]
        );
        assert!(store
            .write_organization_record(
                &key,
                Record::Pin {
                    clip_id: clip.id,
                    pinned: false,
                    order: None,
                }
            )
            .is_err());

        let text = serde_json::to_string(&preference).unwrap();
        assert!(parse(&preference.id(), &text).is_some());
        let extended = text.replacen('{', r#"{"added":1,"#, 1);
        assert!(parse(&preference.id(), &extended).is_none());
    }
}
