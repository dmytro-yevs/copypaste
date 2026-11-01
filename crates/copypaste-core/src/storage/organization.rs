//! Replicated organization records and their transactional query projections.
//!
//! Each tag, membership, and pin has an independent LWW record. Existing P2P
//! and encrypted cloud transports carry them without changing clip timestamps.
use std::collections::BTreeMap;

use copypaste_ipc::{
    HistoryTag, HistoryTagMutation, HistoryTags, MAX_HISTORY_TAGS, MAX_TAG_CLIPS,
    MAX_TAG_NAME_CHARS,
};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{connection::write_tx, IncomingItem, Store, StoreError};

pub(crate) const CONTENT_TYPE: &str = "application/vnd.copypaste.organization+json";
const PREFIX: &str = "org:v1:";
const MAX_RECORD_BYTES: usize = 2048;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Record {
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
}

pub(crate) fn is_record_id(id: &str) -> bool {
    id.starts_with(PREFIX)
}

impl Record {
    fn id(&self) -> String {
        let key = match self {
            Self::Tag { id, .. } => format!("tag:{id}"),
            Self::Membership {
                clip_id, tag_id, ..
            } => format!("membership:{}:{clip_id}:{tag_id}", clip_id.len()),
            Self::Pin { clip_id, .. } => format!("pin:{clip_id}"),
        };
        format!("{PREFIX}{}", hex::encode(Sha256::digest(key.as_bytes())))
    }
    fn valid(&self) -> bool {
        fn identifier(id: &str) -> bool {
            !id.is_empty() && id.len() <= 128 && !is_record_id(id)
        }
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
        }
    }
    fn project(&self, tx: &Connection, stamp: i64) -> Result<(), StoreError> {
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
        }
        Ok(())
    }
}

pub(super) fn apply_pin_projection(tx: &Connection, id: &str) -> Result<(), StoreError> {
    tx.execute("UPDATE clipboard_items SET (pinned,pin_order,pin_updated_at) = (SELECT pinned,pin_order,updated_at FROM history_pin_state WHERE clip_id=?1) WHERE id=?1 AND deleted=0 AND EXISTS (SELECT 1 FROM history_pin_state WHERE clip_id=?1 AND updated_at>=clipboard_items.pin_updated_at)", [id])?;
    Ok(())
}

pub(crate) fn valid_wire(id: &str, content_type: &str, content: &str, deleted: bool) -> bool {
    if content_type != CONTENT_TYPE {
        return !is_record_id(id);
    }
    !deleted
        && content.len() <= MAX_RECORD_BYTES
        && serde_json::from_str::<Record>(content)
            .is_ok_and(|record| record.valid() && record.id() == id)
}

pub(super) fn upsert_in_tx(
    tx: &Connection,
    incoming: &IncomingItem<'_>,
) -> Result<bool, StoreError> {
    let Some(text) = incoming.search_text else {
        return Ok(false);
    };
    if !valid_wire(incoming.id, incoming.content_type, text, incoming.deleted) {
        return Ok(false);
    }
    let record: Record = serde_json::from_str(text).map_err(|_| StoreError::InvalidHistoryQuery)?;
    tx.execute("INSERT INTO organization_records(id,content_ciphertext,nonce,content_hash,created_at,origin_device_id) VALUES (?1,?2,?3,?4,?5,?6) ON CONFLICT(id) DO UPDATE SET content_ciphertext=excluded.content_ciphertext,nonce=excluded.nonce,content_hash=excluded.content_hash,created_at=excluded.created_at,origin_device_id=excluded.origin_device_id", params![incoming.id,incoming.content_ciphertext,incoming.nonce,incoming.content_hash,incoming.created_at,incoming.origin_device_id])?;
    record.project(tx, incoming.created_at)?;
    Ok(true)
}

fn write(tx: &Connection, key: &crate::ItemKey, record: Record) -> Result<(), StoreError> {
    if !record.valid() {
        return Err(StoreError::InvalidHistoryQuery);
    }
    let id = record.id();
    let text = serde_json::to_string(&record).map_err(|_| StoreError::InvalidHistoryQuery)?;
    let hash = super::compute_content_hash(text.as_bytes());
    let previous: Option<(i64, String)> = tx
        .query_row(
            "SELECT created_at,content_hash FROM organization_records WHERE id=?1",
            [&id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    if previous
        .as_ref()
        .is_some_and(|(_, previous_hash)| previous_hash == &hash)
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
        .map_or(crate::now_ms(), |(stamp, _)| {
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

impl Store {
    pub fn seed_pin_organization(&self, key: &crate::ItemKey) -> Result<(), StoreError> {
        loop {
            let mut conn = self.conn()?;
            let tx = write_tx(&mut conn)?;
            let ids = tx.prepare("SELECT id FROM clipboard_items WHERE deleted=0 AND (pinned=1 OR pin_updated_at>0) AND NOT EXISTS (SELECT 1 FROM history_pin_state WHERE clip_id=clipboard_items.id) LIMIT 100")?
                .query_map([],|r|r.get::<_,String>(0))?.collect::<rusqlite::Result<Vec<_>>>()?;
            if ids.is_empty() {
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
        let mut assignments = BTreeMap::new();
        let mut statement = conn.prepare("SELECT ct.tag_id FROM history_clip_tags ct JOIN history_tags t ON t.id=ct.tag_id WHERE ct.clip_id=?1 AND ct.assigned=1 AND t.deleted=0 ORDER BY t.name COLLATE NOCASE,t.id")?;
        for id in clip_ids {
            assignments.insert(
                id.clone(),
                statement
                    .query_map([id], |r| r.get(0))?
                    .collect::<rusqlite::Result<Vec<String>>>()?,
            );
        }
        Ok(HistoryTags { tags, assignments })
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

pub(super) fn catalogue(conn: &Connection) -> Result<Vec<HistoryTag>, StoreError> {
    let tags = conn.prepare("SELECT id,name FROM history_tags WHERE deleted=0 ORDER BY name COLLATE NOCASE,id LIMIT 501")?
            .query_map([], |r| Ok(HistoryTag { id:r.get(0)?,name:r.get(1)? }))?.collect::<rusqlite::Result<Vec<_>>>()?;
    if tags.len() > MAX_HISTORY_TAGS {
        return Err(StoreError::InvalidHistoryQuery);
    }
    Ok(tags)
}
