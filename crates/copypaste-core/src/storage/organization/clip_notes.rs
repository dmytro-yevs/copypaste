//! Synced per-clip notes: hidden from integrations, keep, a synced expiry and
//! provenance.
//!
//! The only kind that is tombstoned, together with its clip. A note may arrive
//! before its clip, so a note without a live clip is stored (the sender's
//! cursor advances) and marked orphan; inserting the clip clears the mark, and
//! orphans older than [`ORPHAN_TTL_MS`] are pruned. Hiding wins over a
//! concurrent unhide that was based on an older version, and the earlier of
//! two synced deadlines wins unless the newer version keeps the clip.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use super::super::expiry;
use super::{identifier, line, IncomingItem, StoreError, CONTENT_TYPE, LIBRARY_KIND_CLASS};

pub(super) const NOTE_BYTES: usize = 4 * 1024;
const ORPHAN_TTL_MS: i64 = 30 * 24 * 60 * 60 * 1000;
const MAX_SNAPSHOT_STEPS: usize = 16;
const PROVENANCE_CHARS: usize = 64;
const MODULE_LABEL_CHARS: usize = 80;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClipNoteRecord {
    pub clip_id: String,
    pub hidden_from_integrations: bool,
    /// Stamp of the version that last changed `hidden_from_integrations`.
    pub hidden_changed_at: i64,
    /// Stamp of the note version this one was edited from.
    pub base_stamp: i64,
    pub keep: bool,
    pub expires_at: Option<i64>,
    pub expiry_duration_ms: Option<i64>,
    pub provenance: Option<Provenance>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Provenance {
    pub kind: ProvenanceKind,
    pub from_clip_id: Option<String>,
    pub transform_snapshot: Option<Vec<TransformRef>>,
    pub pipeline_name: Option<String>,
    /// The module that wrote the clip; set exactly for [`ProvenanceKind::Module`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub module: Option<ModuleOrigin>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleOrigin {
    pub module_id: String,
    /// What the module says the write was, shown with its name.
    pub label: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvenanceKind {
    Manual,
    Edited,
    Transformed,
    Module,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransformRef {
    pub transform_id: String,
    pub version: u32,
}

pub(super) fn note_key(clip_id: &str) -> String {
    format!("clip_note:{clip_id}")
}

impl Provenance {
    fn valid(&self) -> bool {
        let short = |text: &Option<String>| {
            text.as_deref()
                .is_none_or(|text| line(text, PROVENANCE_CHARS))
        };
        self.from_clip_id.as_deref().is_none_or(identifier)
            && self.transform_snapshot.as_ref().is_none_or(|steps| {
                steps.len() <= MAX_SNAPSHOT_STEPS
                    && steps
                        .iter()
                        .all(|step| identifier(&step.transform_id) && step.transform_id.len() <= 64)
            })
            && short(&self.pipeline_name)
            && (self.kind == ProvenanceKind::Module) == self.module.is_some()
            && self.module.as_ref().is_none_or(|module| {
                identifier(&module.module_id)
                    && module.module_id.len() <= 96
                    && super::required_line(&module.label, MODULE_LABEL_CHARS)
            })
    }
}

impl ClipNoteRecord {
    pub(super) fn valid(&self) -> bool {
        identifier(&self.clip_id)
            && (!self.keep || self.expires_at.is_none())
            && self.expiry_duration_ms.is_none_or(|duration| duration >= 0)
            && self.provenance.as_ref().is_none_or(Provenance::valid)
    }

    pub(super) fn project(&self, tx: &Connection, record_id: &str) -> Result<(), StoreError> {
        let provenance = self
            .provenance
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|_| StoreError::InvalidHistoryQuery)?;
        let local: Option<(bool, Option<i64>, Option<i64>)> = tx
            .query_row(
                "SELECT keep,expires_at,expiry_duration_ms FROM clip_notes WHERE clip_id=?1",
                [&self.clip_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        let expiry = expiry::merge_synced(
            local
                .filter(|(keep, _, _)| !keep)
                .map(|(_, expires_at, duration_ms)| expiry::SyncedExpiry {
                    expires_at,
                    duration_ms,
                })
                .unwrap_or_default(),
            expiry::SyncedExpiry {
                expires_at: self.expires_at,
                duration_ms: self.expiry_duration_ms,
            },
            self.keep,
        );
        tx.execute(
            "INSERT INTO clip_notes(clip_id,record_id,hidden_from_integrations,hidden_changed_at,base_stamp,\
             keep,expires_at,expiry_duration_ms,provenance_json,orphan_since) \
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,CASE WHEN EXISTS(SELECT 1 FROM clipboard_items \
             WHERE id=?1 AND deleted=0) THEN NULL ELSE ?10 END) ON CONFLICT(clip_id) DO UPDATE SET \
             record_id=excluded.record_id,hidden_from_integrations=excluded.hidden_from_integrations,\
             hidden_changed_at=excluded.hidden_changed_at,base_stamp=excluded.base_stamp,\
             keep=excluded.keep,expires_at=excluded.expires_at,\
             expiry_duration_ms=excluded.expiry_duration_ms,\
             provenance_json=excluded.provenance_json,\
             orphan_since=CASE WHEN excluded.orphan_since IS NULL THEN NULL \
             ELSE COALESCE(clip_notes.orphan_since,excluded.orphan_since) END",
            params![
                self.clip_id,
                record_id,
                self.hidden_from_integrations,
                self.hidden_changed_at,
                self.base_stamp,
                self.keep,
                expiry.expires_at,
                expiry.duration_ms,
                provenance,
                crate::now_ms(),
            ],
        )?;
        expiry::note_applied(tx, self)
    }
}

/// An unhide that did not see the local hide is concurrent with it, and the
/// hide wins.
pub(super) fn keeps_hidden(tx: &Connection, incoming: &ClipNoteRecord) -> Result<bool, StoreError> {
    if incoming.hidden_from_integrations {
        return Ok(false);
    }
    let local: Option<(bool, i64)> = tx
        .query_row(
            "SELECT hidden_from_integrations,hidden_changed_at FROM clip_notes WHERE clip_id=?1",
            [&incoming.clip_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    Ok(local.is_some_and(|(hidden, changed_at)| hidden && incoming.base_stamp < changed_at))
}

/// Stores a tombstone for a note id. Any other live record under that id is
/// never tombstoned: only notes may be.
pub(super) fn apply_tombstone(
    tx: &Connection,
    incoming: &IncomingItem<'_>,
) -> Result<bool, StoreError> {
    let other_live: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM organization_records r WHERE r.id=?1 AND r.deleted=0 \
         AND NOT EXISTS(SELECT 1 FROM clip_notes n WHERE n.record_id=r.id))",
        [incoming.id],
        |r| r.get(0),
    )?;
    if other_live {
        return Ok(false);
    }
    tx.execute(
        "INSERT INTO organization_records(id,content_ciphertext,nonce,content_hash,created_at,\
         origin_device_id,kind_class,deleted) VALUES (?1,X'',X'',?2,?3,?4,?5,1) \
         ON CONFLICT(id) DO UPDATE SET content_ciphertext=X'',nonce=X'',\
         content_hash=excluded.content_hash,created_at=excluded.created_at,\
         origin_device_id=excluded.origin_device_id,kind_class=excluded.kind_class,deleted=1",
        params![
            incoming.id,
            incoming.content_hash,
            incoming.created_at,
            incoming.origin_device_id,
            LIBRARY_KIND_CLASS,
        ],
    )?;
    tx.execute("DELETE FROM clip_notes WHERE record_id=?1", [incoming.id])?;
    Ok(true)
}

pub(super) fn tombstone(tx: &Connection, clip_id: &str) -> Result<bool, StoreError> {
    let id = super::record_id(&note_key(clip_id));
    let live: Option<(i64, String)> = tx
        .query_row(
            "SELECT created_at,content_hash FROM organization_records WHERE id=?1 AND deleted=0",
            [&id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let Some((stamp, hash)) = live else {
        return Ok(false);
    };
    apply_tombstone(
        tx,
        &IncomingItem {
            id: &id,
            content_ciphertext: None,
            nonce: None,
            content_type: CONTENT_TYPE,
            content_hash: &hash,
            created_at: stamp.saturating_add(1).max(crate::now_ms()),
            deleted: true,
            origin_device_id: "",
            app_bundle_id: None,
            app_name: None,
            pinned: false,
            pin_order: None,
            pin_updated_at: 0,
            search_text: None,
            payload_metadata: None,
        },
    )
}

/// Hard-deletes orphan notes past the window, record included: no tombstone,
/// because the clip they describe never reached this device.
pub(super) fn prune_orphans(tx: &Connection, now_ms: i64) -> Result<usize, StoreError> {
    let cutoff = now_ms.saturating_sub(ORPHAN_TTL_MS);
    tx.execute(
        "DELETE FROM organization_records WHERE id IN (SELECT record_id FROM clip_notes \
         WHERE orphan_since IS NOT NULL AND orphan_since<=?1)",
        [cutoff],
    )?;
    Ok(tx.execute(
        "DELETE FROM clip_notes WHERE orphan_since IS NOT NULL AND orphan_since<=?1",
        [cutoff],
    )?)
}

pub(super) fn note(conn: &Connection, clip_id: &str) -> Result<Option<ClipNoteRecord>, StoreError> {
    type Row = (
        bool,
        i64,
        i64,
        bool,
        Option<i64>,
        Option<i64>,
        Option<String>,
    );
    let row: Option<Row> = conn
        .query_row(
            "SELECT hidden_from_integrations,hidden_changed_at,base_stamp,keep,expires_at,\
             expiry_duration_ms,provenance_json FROM clip_notes WHERE clip_id=?1",
            [clip_id],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                ))
            },
        )
        .optional()?;
    row.map(
        |(
            hidden_from_integrations,
            hidden_changed_at,
            base_stamp,
            keep,
            expires_at,
            duration,
            provenance,
        )| {
            Ok(ClipNoteRecord {
                clip_id: clip_id.to_owned(),
                hidden_from_integrations,
                hidden_changed_at,
                base_stamp,
                keep,
                expires_at,
                expiry_duration_ms: duration,
                provenance: provenance
                    .as_deref()
                    .map(serde_json::from_str)
                    .transpose()
                    .map_err(|_| StoreError::InvalidHistoryQuery)?,
            })
        },
    )
    .transpose()
}
