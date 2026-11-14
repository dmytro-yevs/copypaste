//! Snippet metadata, snippet body and snippet collection records.
//!
//! Metadata and body are separate records so that a title edit and a body edit
//! made on two devices both survive. A purge is terminal: once any applied
//! metadata version carried `purged`, the projection stays purged and the body
//! projects as empty whatever arrives later.
//!
//! The projected `deleted_at` is the effective one: a delete loses to a body
//! version stamped after it, so an edit made after a remote delete restores
//! the snippet. A body replaced by a concurrent version is kept as a conflict
//! copy, projected here and published by the next keyed write.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use super::{identifier, line, record_id, required_line, StoreError};
use crate::snippets::{abbreviation, conflict};

pub(super) const SNIPPET_BYTES: usize = 4 * 1024;
pub(super) const BODY_BYTES: usize = 320 * 1024;
pub(super) const COLLECTION_BYTES: usize = 2 * 1024;
const REVISION_HEX: usize = 32;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnippetRecord {
    pub id: String,
    pub collection_id: Option<String>,
    pub title: String,
    pub abbreviation: Option<String>,
    pub template: bool,
    pub pinned: bool,
    pub pin_order: Option<f64>,
    pub order_key: String,
    pub concealed: bool,
    pub deleted_at: Option<i64>,
    pub purged: bool,
    pub created_at: i64,
    pub created_device: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnippetBodyRecord {
    pub id: String,
    /// 128-bit random revision, lowercase hex.
    pub revision: String,
    /// The revisions this body descends from, newest first.
    pub ancestors: Vec<String>,
    pub text: String,
    pub rich_html: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnippetCollectionRecord {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub accent: String,
    pub order_key: String,
    pub deleted_at: Option<i64>,
}

pub(super) fn snippet_key(id: &str) -> String {
    format!("snippet:{id}")
}

pub(super) fn body_key(id: &str) -> String {
    format!("snippet_body:{id}")
}

pub(super) fn collection_key(id: &str) -> String {
    format!("snippet_collection:{id}")
}

fn order_key(key: &str) -> bool {
    required_line(key, 128)
}

fn revision(value: &str) -> bool {
    value.len() == REVISION_HEX
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

impl SnippetRecord {
    pub(super) fn valid(&self) -> bool {
        identifier(&self.id)
            && self.collection_id.as_deref().is_none_or(identifier)
            && line(&self.title, 256)
            && self
                .abbreviation
                .as_deref()
                .is_none_or(|abbreviation| abbreviation::validate(abbreviation).is_ok())
            && (if self.pinned {
                self.pin_order.is_some_and(f64::is_finite)
            } else {
                self.pin_order.is_none()
            })
            && order_key(&self.order_key)
            && line(&self.created_device, 128)
            && (!self.purged || (self.title.is_empty() && self.abbreviation.is_none()))
    }

    pub(super) fn project(
        &self,
        tx: &Connection,
        record_id: &str,
        stamp: i64,
    ) -> Result<(), StoreError> {
        let body_stamp: Option<i64> = tx
            .query_row(
                "SELECT body_stamp FROM snippet_bodies WHERE id=?1",
                [&self.id],
                |r| r.get(0),
            )
            .optional()?;
        let deleted_at = self
            .deleted_at
            .filter(|_| body_stamp.is_none_or(|body| body <= stamp));
        tx.execute(
            "INSERT INTO snippets(id,record_id,collection_id,title,abbreviation,abbreviation_key,\
             template,pinned,pin_order,order_key,concealed,deleted_at,purged,meta_stamp,created_at,\
             created_device) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16) \
             ON CONFLICT(id) DO UPDATE SET record_id=excluded.record_id,\
             collection_id=excluded.collection_id,title=excluded.title,\
             abbreviation=excluded.abbreviation,abbreviation_key=excluded.abbreviation_key,\
             template=excluded.template,pinned=excluded.pinned,pin_order=excluded.pin_order,\
             order_key=excluded.order_key,concealed=excluded.concealed,\
             deleted_at=excluded.deleted_at,purged=excluded.purged,meta_stamp=excluded.meta_stamp,\
             created_at=excluded.created_at,created_device=excluded.created_device",
            params![
                self.id,
                record_id,
                self.collection_id,
                self.title,
                self.abbreviation,
                self.abbreviation.as_deref().map(abbreviation::normalize),
                self.template,
                self.pinned,
                self.pin_order,
                self.order_key,
                self.concealed,
                deleted_at,
                self.purged,
                stamp,
                self.created_at,
                self.created_device,
            ],
        )?;
        if self.purged {
            tx.execute(
                "UPDATE snippet_bodies SET body_text='', body_rich=NULL \
                 WHERE id=?1 AND (body_text<>'' OR body_rich IS NOT NULL)",
                [&self.id],
            )?;
        }
        refresh_search(tx, &self.id)
    }
}

impl SnippetBodyRecord {
    pub(super) fn valid(&self) -> bool {
        identifier(&self.id)
            && revision(&self.revision)
            && self.ancestors.len() <= conflict::MAX_ANCESTORS
            && self.ancestors.iter().all(|ancestor| revision(ancestor))
    }

    pub(super) fn project(
        &self,
        tx: &Connection,
        record_id: &str,
        stamp: i64,
    ) -> Result<(), StoreError> {
        let previous = body(tx, &self.id)?;
        let purged = is_purged(tx, &self.id)?;
        let (text, rich) = if purged {
            ("", None)
        } else {
            (self.text.as_str(), self.rich_html.as_deref())
        };
        let ancestors =
            serde_json::to_string(&self.ancestors).map_err(|_| StoreError::InvalidHistoryQuery)?;
        tx.execute(
            "INSERT INTO snippet_bodies(id,record_id,revision,ancestors_json,body_text,body_rich,\
             body_stamp) VALUES (?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(id) DO UPDATE SET \
             record_id=excluded.record_id,revision=excluded.revision,\
             ancestors_json=excluded.ancestors_json,body_text=excluded.body_text,\
             body_rich=excluded.body_rich,body_stamp=excluded.body_stamp",
            params![
                self.id,
                record_id,
                self.revision,
                ancestors,
                text,
                rich,
                stamp
            ],
        )?;
        tx.execute(
            "UPDATE snippets SET deleted_at=NULL WHERE id=?1 AND deleted_at IS NOT NULL \
             AND purged=0 AND meta_stamp<?2",
            params![self.id, stamp],
        )?;
        if let Some(previous) = previous.filter(|previous| previous.revision != self.revision) {
            if !purged {
                on_replaced(tx, record_id, &previous, self)?;
            }
        }
        refresh_search(tx, &self.id)
    }
}

impl SnippetCollectionRecord {
    pub(super) fn valid(&self) -> bool {
        identifier(&self.id)
            && required_line(&self.name, 64)
            && required_line(&self.icon, 64)
            && required_line(&self.accent, 64)
            && order_key(&self.order_key)
    }

    pub(super) fn project(&self, tx: &Connection, record_id: &str) -> Result<(), StoreError> {
        tx.execute(
            "INSERT INTO snippet_collections(id,record_id,name,icon,accent,order_key,deleted_at) \
             VALUES (?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(id) DO UPDATE SET \
             record_id=excluded.record_id,name=excluded.name,icon=excluded.icon,\
             accent=excluded.accent,order_key=excluded.order_key,deleted_at=excluded.deleted_at",
            params![
                self.id,
                record_id,
                self.name,
                self.icon,
                self.accent,
                self.order_key,
                self.deleted_at
            ],
        )?;
        Ok(())
    }
}

fn version(body: &SnippetBodyRecord) -> conflict::BodyVersion<'_> {
    conflict::BodyVersion {
        revision: &body.revision,
        ancestors: &body.ancestors,
        text: &body.text,
        rich_html: body.rich_html.as_deref(),
    }
}

/// Called when an applied body replaces a different revision of the same
/// snippet, before its record is replaced. The applied version won the merge,
/// so a concurrent `previous` is the loser and is projected as a copy. The
/// copy has no record yet: sealing one needs the item key, which the apply
/// transaction does not hold.
fn on_replaced(
    tx: &Connection,
    body_record_id: &str,
    previous: &SnippetBodyRecord,
    incoming: &SnippetBodyRecord,
) -> Result<(), StoreError> {
    if !conflict::concurrent(&version(previous), &version(incoming)) {
        return Ok(());
    }
    let copy_id = conflict::copy_id(&previous.revision);
    if snippet(tx, &copy_id)?.is_some() {
        return Ok(());
    }
    let (stamp, origin): (i64, String) = tx
        .query_row(
            "SELECT created_at,origin_device_id FROM organization_records WHERE id=?1",
            [body_record_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?
        .unwrap_or_else(|| (crate::now_ms(), String::new()));
    let (device_id, device_name): (String, Option<String>) = tx.query_row(
        "SELECT d.id,(SELECT name FROM sync_device_name WHERE device_id=d.id) FROM \
         (SELECT CASE WHEN ?1<>'' THEN ?1 ELSE COALESCE((SELECT value FROM sync_device_state \
         WHERE key='device_id'),'') END AS id) d",
        [&origin],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    // A body seen before its metadata: unknown flags are taken as concealed.
    let original = snippet(tx, &incoming.id)?;
    let copy = SnippetRecord {
        id: copy_id.clone(),
        collection_id: original.as_ref().and_then(|s| s.collection_id.clone()),
        title: conflict::copy_title(
            original.as_ref().map_or("", |s| s.title.as_str()),
            device_name.as_deref().unwrap_or("unknown device"),
            stamp,
            crate::storage::snippets::MAX_TITLE_CHARS,
        ),
        abbreviation: None,
        template: original.as_ref().is_some_and(|s| s.template),
        pinned: false,
        pin_order: None,
        order_key: original
            .as_ref()
            .map_or_else(|| "V".to_owned(), |s| s.order_key.clone()),
        concealed: original.as_ref().is_none_or(|s| s.concealed),
        deleted_at: None,
        purged: false,
        created_at: stamp,
        created_device: device_id.chars().take(128).collect(),
    };
    let body = SnippetBodyRecord {
        id: copy_id.clone(),
        revision: previous.revision.clone(),
        ancestors: Vec::new(),
        text: previous.text.clone(),
        rich_html: previous.rich_html.clone(),
    };
    copy.project(tx, &record_id(&snippet_key(&copy_id)), stamp)?;
    body.project(tx, &record_id(&body_key(&copy_id)), stamp)
}

fn is_purged(tx: &Connection, id: &str) -> Result<bool, StoreError> {
    Ok(tx
        .query_row("SELECT purged FROM snippets WHERE id=?1", [id], |r| {
            r.get::<_, bool>(0)
        })
        .optional()?
        .unwrap_or(false))
}

pub(super) fn keeps_purge(tx: &Connection, incoming: &SnippetRecord) -> Result<bool, StoreError> {
    Ok(!incoming.purged && is_purged(tx, &incoming.id)?)
}

/// A body edited after a purge is answered with the local stub, so its text
/// is not stored. Without a local stub it is stored, projected empty, and
/// replaced once the purging device writes its stub back above it.
pub(super) fn keeps_purged_body(
    tx: &Connection,
    incoming: &SnippetBodyRecord,
) -> Result<bool, StoreError> {
    if incoming.revision == conflict::PURGED_REVISION || !is_purged(tx, &incoming.id)? {
        return Ok(false);
    }
    Ok(tx
        .query_row(
            "SELECT revision=?2 FROM snippet_bodies WHERE id=?1",
            params![incoming.id, conflict::PURGED_REVISION],
            |r| r.get::<_, bool>(0),
        )
        .optional()?
        .unwrap_or(false))
}

/// Keeps the snippet's full-text row equal to its projection: title and
/// abbreviation always, the body only when it is not concealed, nothing once
/// purged.
fn refresh_search(tx: &Connection, id: &str) -> Result<(), StoreError> {
    type Row = (
        Option<i64>,
        String,
        Option<String>,
        bool,
        bool,
        Option<String>,
    );
    let row: Option<Row> = tx
        .query_row(
            "SELECT s.fts_rowid,s.title,s.abbreviation,s.concealed,s.purged,b.body_text \
             FROM snippets s LEFT JOIN snippet_bodies b ON b.id=s.id WHERE s.id=?1",
            [id],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                ))
            },
        )
        .optional()?;
    let Some((old_rowid, title, abbreviation, concealed, purged, body)) = row else {
        return Ok(());
    };
    if let Some(rowid) = old_rowid {
        tx.execute("DELETE FROM snippet_fts WHERE rowid=?1", [rowid])?;
    }
    let rowid = if purged {
        None
    } else {
        let body = if concealed {
            String::new()
        } else {
            body.unwrap_or_default()
        };
        tx.execute(
            "INSERT INTO snippet_fts(id,title,abbreviation,body_text) VALUES (?1,?2,?3,?4)",
            params![id, title, abbreviation, body],
        )?;
        Some(tx.last_insert_rowid())
    };
    tx.execute(
        "UPDATE snippets SET fts_rowid=?2 WHERE id=?1",
        params![id, rowid],
    )?;
    Ok(())
}

pub(super) fn snippet(conn: &Connection, id: &str) -> Result<Option<SnippetRecord>, StoreError> {
    Ok(conn
        .query_row(
            "SELECT id,collection_id,title,abbreviation,template,pinned,pin_order,order_key,\
             concealed,deleted_at,purged,created_at,created_device FROM snippets WHERE id=?1",
            [id],
            |r| {
                Ok(SnippetRecord {
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
                    purged: r.get(10)?,
                    created_at: r.get(11)?,
                    created_device: r.get(12)?,
                })
            },
        )
        .optional()?)
}

pub(super) fn body(conn: &Connection, id: &str) -> Result<Option<SnippetBodyRecord>, StoreError> {
    let row: Option<(String, String, String, Option<String>)> = conn
        .query_row(
            "SELECT revision,ancestors_json,body_text,body_rich FROM snippet_bodies WHERE id=?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()?;
    row.map(|(revision, ancestors, text, rich_html)| {
        Ok(SnippetBodyRecord {
            id: id.to_owned(),
            revision,
            ancestors: serde_json::from_str(&ancestors)
                .map_err(|_| StoreError::InvalidHistoryQuery)?,
            text,
            rich_html,
        })
    })
    .transpose()
}

pub(super) fn collection(
    conn: &Connection,
    id: &str,
) -> Result<Option<SnippetCollectionRecord>, StoreError> {
    Ok(conn
        .query_row(
            "SELECT id,name,icon,accent,order_key,deleted_at FROM snippet_collections WHERE id=?1",
            [id],
            |r| {
                Ok(SnippetCollectionRecord {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    icon: r.get(2)?,
                    accent: r.get(3)?,
                    order_key: r.get(4)?,
                    deleted_at: r.get(5)?,
                })
            },
        )
        .optional()?)
}
