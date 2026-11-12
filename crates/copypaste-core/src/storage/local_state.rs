//! Typed access to per-clip local state, the read-time expiry filter and the
//! store side of the deadline scheduler.
//!
//! An expired clip is hidden from every list and search the moment its
//! deadline passes, whether or not a purge has run yet. Pinned and kept clips
//! never expire. The scheduler asks this module for the next deadline and runs
//! its housekeeping: the long-horizon prunes and the purge of sealed originals.

use std::collections::HashMap;

use copypaste_ipc::payload::{
    ClipProvenance, ItemAnnotations, ProvenanceKind as WireKind, TransformStamp,
};
use rusqlite::{params, Connection, OptionalExtension};

use super::connection::write_tx;
use super::organization::{Provenance, ProvenanceKind};
use super::{Store, StoreError};

/// How long ids purged by expiry, rule conflicts and orphan notes are kept.
pub const RETAIN_MS: i64 = 30 * 24 * 60 * 60 * 1000;

/// Wall-clock milliseconds as SQL, so every query reads one clock without a
/// parameter of its own.
macro_rules! sql_now_ms {
    () => {
        "CAST((julianday('now') - 2440587.5) * 86400000.0 AS INTEGER)"
    };
}

/// Keeps a `clipboard_items` row only while it is not expired. `$alias` is
/// the table prefix (`""` or `"ci."`); pinned and kept clips never expire.
macro_rules! unexpired {
    ($alias:literal) => {
        concat!(
            "(",
            $alias,
            "pinned = 1 OR (NOT EXISTS (SELECT 1 FROM clip_local_state xs WHERE xs.clip_id = ",
            $alias,
            "id AND xs.expires_at <= ",
            $crate::storage::local_state::sql_now_ms!(),
            " AND NOT EXISTS (SELECT 1 FROM clip_notes xk WHERE xk.clip_id = xs.clip_id AND xk.keep = 1)) \
             AND NOT EXISTS (SELECT 1 FROM clip_notes xn WHERE xn.clip_id = ",
            $alias,
            "id AND xn.keep = 0 AND xn.expires_at <= ",
            $crate::storage::local_state::sql_now_ms!(),
            ")))"
        )
    };
}

pub(crate) use {sql_now_ms, unexpired};

/// One clip's row in `clip_local_state`: never synced, removed with the clip.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClipLocalState {
    pub clip_id: String,
    pub expires_at: Option<i64>,
    pub expiry_duration_ms: Option<i64>,
    pub expiry_mono_ms: Option<i64>,
    pub rule_secret: bool,
    pub rule_hidden_from_ai: bool,
    pub applied_rules_json: Option<String>,
    pub eval_flags: i64,
    pub capture_channel: Option<String>,
    pub rules_revision: Option<i64>,
}

/// What one housekeeping sweep removed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LocalSweep {
    pub expired_ids: usize,
    pub rule_conflicts: usize,
    pub rule_originals: usize,
    pub orphan_notes: usize,
}

/// Per-kind change markers of the synced library, compared before and after
/// an apply to learn which kinds a peer or the cloud changed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LibraryRevision {
    pub snippets: i64,
    pub rules: i64,
    pub pipelines: (i64, i64),
    pub preferences: (i64, i64),
    pub set_members: (i64, i64),
    pub grants: (i64, i64),
}

/// Writes `state` in the caller's transaction, only for a live clip.
pub(crate) fn write(tx: &Connection, state: &ClipLocalState) -> Result<bool, StoreError> {
    Ok(tx.execute(
        "INSERT INTO clip_local_state(clip_id,expires_at,expiry_duration_ms,expiry_mono_ms,\
         rule_secret,rule_hidden_from_ai,applied_rules_json,eval_flags,capture_channel,\
         rules_revision) SELECT ?1,?2,?3,?4,?5,?6,?7,?8,?9,?10 WHERE EXISTS(SELECT 1 FROM \
         clipboard_items WHERE id=?1 AND deleted=0) ON CONFLICT(clip_id) DO UPDATE SET \
         expires_at=excluded.expires_at,expiry_duration_ms=excluded.expiry_duration_ms,\
         expiry_mono_ms=excluded.expiry_mono_ms,rule_secret=excluded.rule_secret,\
         rule_hidden_from_ai=excluded.rule_hidden_from_ai,\
         applied_rules_json=excluded.applied_rules_json,eval_flags=excluded.eval_flags,\
         capture_channel=excluded.capture_channel,rules_revision=excluded.rules_revision",
        params![
            state.clip_id,
            state.expires_at,
            state.expiry_duration_ms,
            state.expiry_mono_ms,
            state.rule_secret,
            state.rule_hidden_from_ai,
            state.applied_rules_json,
            state.eval_flags,
            state.capture_channel,
            state.rules_revision,
        ],
    )? == 1)
}

pub(crate) fn read(conn: &Connection, clip_id: &str) -> Result<Option<ClipLocalState>, StoreError> {
    Ok(conn
        .prepare_cached(
            "SELECT expires_at,expiry_duration_ms,expiry_mono_ms,rule_secret,rule_hidden_from_ai,\
             applied_rules_json,eval_flags,capture_channel,rules_revision FROM clip_local_state \
             WHERE clip_id=?1",
        )?
        .query_row([clip_id], |row| {
            Ok(ClipLocalState {
                clip_id: clip_id.to_owned(),
                expires_at: row.get(0)?,
                expiry_duration_ms: row.get(1)?,
                expiry_mono_ms: row.get(2)?,
                rule_secret: row.get(3)?,
                rule_hidden_from_ai: row.get(4)?,
                applied_rules_json: row.get(5)?,
                eval_flags: row.get(6)?,
                capture_channel: row.get(7)?,
                rules_revision: row.get(8)?,
            })
        })
        .optional()?)
}

/// The annotations of one clip, bounded for the wire, or `None` when it has
/// nothing beyond the defaults. One indexed lookup per table.
pub(crate) fn annotations(
    conn: &Connection,
    clip_id: &str,
) -> Result<Option<ItemAnnotations>, StoreError> {
    type Row = (
        Option<i64>,
        bool,
        bool,
        bool,
        bool,
        Option<i64>,
        Option<String>,
        Option<String>,
    );
    let row: Row = conn
        .prepare_cached(
            "SELECT s.expires_at, COALESCE(s.rule_secret,0), COALESCE(s.rule_hidden_from_ai,0), \
             COALESCE(n.hidden_from_ai,0), COALESCE(n.keep,0), n.expires_at, n.provenance_json, \
             (SELECT json_group_array(tag_id) FROM (SELECT r.tag_id FROM clip_rule_tags r \
              JOIN history_tags t ON t.id = r.tag_id AND t.deleted = 0 \
              WHERE r.clip_id = q.id ORDER BY r.tag_id LIMIT ?2)) \
             FROM (SELECT ?1 AS id) q \
             LEFT JOIN clip_local_state s ON s.clip_id = q.id \
             LEFT JOIN clip_notes n ON n.clip_id = q.id AND n.orphan_since IS NULL",
        )?
        .query_row(
            params![clip_id, copypaste_ipc::MAX_ITEM_TAG_IDS as i64],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                ))
            },
        )?;
    let (local_expiry, rule_secret, rule_hidden, hidden, keep, note_expiry, provenance, tags) = row;
    let annotations = ItemAnnotations {
        expires_at: if keep {
            None
        } else {
            local_expiry.into_iter().chain(note_expiry).min()
        },
        hidden_from_ai: hidden || rule_hidden,
        rule_secret,
        provenance: provenance
            .as_deref()
            .and_then(|json| serde_json::from_str::<Provenance>(json).ok())
            .map(wire_provenance),
        rule_tag_ids: tags
            .as_deref()
            .and_then(|json| serde_json::from_str(json).ok())
            .unwrap_or_default(),
        keep,
    };
    Ok((annotations != ItemAnnotations::default()).then(|| annotations.bounded()))
}

fn wire_provenance(provenance: Provenance) -> ClipProvenance {
    ClipProvenance {
        kind: match provenance.kind {
            ProvenanceKind::Manual => WireKind::Manual,
            ProvenanceKind::Edited => WireKind::Edited,
            ProvenanceKind::Transformed => WireKind::Transformed,
            ProvenanceKind::AiTool => WireKind::AiTool,
        },
        from_clip_id: provenance.from_clip_id,
        transform_snapshot: provenance
            .transform_snapshot
            .unwrap_or_default()
            .into_iter()
            .map(|step| TransformStamp {
                transform_id: step.transform_id,
                version: step.version,
            })
            .collect(),
        pipeline_name: provenance.pipeline_name,
        grant_name: provenance.grant_name,
    }
}

impl Store {
    /// Writes the local state of a live clip. Answers `false`, writing nothing,
    /// when the clip is gone.
    pub fn write_clip_local_state(&self, state: &ClipLocalState) -> Result<bool, StoreError> {
        let mut conn = self.conn()?;
        let tx = write_tx(&mut conn)?;
        let written = write(&tx, state)?;
        tx.commit()?;
        Ok(written)
    }

    pub fn clip_local_state(&self, clip_id: &str) -> Result<Option<ClipLocalState>, StoreError> {
        read(&*self.conn()?, clip_id)
    }

    /// Annotations of the given clips; clips with none are absent.
    pub fn item_annotations(
        &self,
        clip_ids: &[String],
    ) -> Result<HashMap<String, ItemAnnotations>, StoreError> {
        let conn = self.conn()?;
        let mut found = HashMap::new();
        for id in clip_ids {
            if let Some(annotations) = annotations(&conn, id)? {
                found.insert(id.clone(), annotations);
            }
        }
        Ok(found)
    }

    /// The earliest store-held deadline strictly after `now_ms`: a clip expiry,
    /// a sealed original's purge, or the end of a 30-day retention.
    pub fn next_local_deadline(&self, now_ms: i64) -> Result<Option<i64>, StoreError> {
        Ok(self
            .conn()?
            .prepare_cached(
                "SELECT MIN(t) FROM (\
                 SELECT MIN(expires_at) AS t FROM clip_local_state WHERE expires_at > ?1 \
                 UNION ALL SELECT MIN(expires_at) FROM clip_notes \
                   WHERE keep = 0 AND orphan_since IS NULL AND expires_at > ?1 \
                 UNION ALL SELECT MIN(purge_at) FROM rule_originals WHERE purge_at > ?1 \
                 UNION ALL SELECT MIN(expired_at) + ?2 FROM expired_ids \
                   WHERE expired_at > ?1 - ?2 \
                 UNION ALL SELECT MIN(created_at) + ?2 FROM content_rule_conflicts \
                   WHERE created_at > ?1 - ?2 \
                 UNION ALL SELECT MIN(orphan_since) + ?2 FROM clip_notes \
                   WHERE orphan_since > ?1 - ?2)",
            )?
            .query_row(params![now_ms, RETAIN_MS], |row| row.get(0))?)
    }

    /// Removes what has outlived its deadline: expired-id records, rule
    /// conflicts and orphan notes after 30 days, sealed originals at their
    /// purge time. Clip expiry itself is the caller's.
    pub fn sweep_local_state(&self, now_ms: i64) -> Result<LocalSweep, StoreError> {
        let cutoff = now_ms.saturating_sub(RETAIN_MS);
        let mut conn = self.conn()?;
        let tx = write_tx(&mut conn)?;
        let sweep = LocalSweep {
            expired_ids: tx.execute("DELETE FROM expired_ids WHERE expired_at <= ?1", [cutoff])?,
            rule_conflicts: tx.execute(
                "DELETE FROM content_rule_conflicts WHERE created_at <= ?1",
                [cutoff],
            )?,
            rule_originals: tx
                .execute("DELETE FROM rule_originals WHERE purge_at <= ?1", [now_ms])?,
            orphan_notes: 0,
        };
        tx.commit()?;
        drop(conn);
        Ok(LocalSweep {
            orphan_notes: self.prune_orphan_clip_notes(now_ms)?,
            ..sweep
        })
    }

    /// Change markers per library kind; cheap enough to read once per
    /// coalesced apply batch.
    pub fn library_revision(&self) -> Result<LibraryRevision, StoreError> {
        let conn = self.conn()?;
        let (snippets, rules) = conn.query_row(
            "SELECT snippets, rules FROM resource_revisions WHERE singleton = 0",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let marker = |table: &str| -> Result<(i64, i64), StoreError> {
            Ok(conn
                .prepare_cached(&format!(
                    "SELECT COUNT(*), COALESCE(MAX(r.created_at), 0) FROM {table} p \
                     JOIN organization_records r ON r.id = p.record_id"
                ))?
                .query_row([], |row| Ok((row.get(0)?, row.get(1)?)))?)
        };
        Ok(LibraryRevision {
            snippets,
            rules,
            pipelines: marker("transform_pipelines")?,
            preferences: marker("library_preferences")?,
            set_members: marker("library_set_members")?,
            grants: marker("ai_grants")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{item, store, T0};
    use super::*;

    fn expire(store: &Store, clip_id: &str, expires_at: i64) {
        assert!(store
            .write_clip_local_state(&ClipLocalState {
                clip_id: clip_id.to_owned(),
                expires_at: Some(expires_at),
                ..Default::default()
            })
            .unwrap());
    }

    #[test]
    fn an_expired_clip_is_absent_from_every_list_and_search_before_its_purge() {
        let store = store();
        let now = crate::now_ms();
        let live = store.insert(item("alpha live", T0)).unwrap().id;
        let expired = store.insert(item("alpha expired", T0 + 1)).unwrap().id;
        let pinned = store.insert(item("alpha pinned", T0 + 2)).unwrap().id;
        let later = store.insert(item("alpha later", T0 + 3)).unwrap().id;
        expire(&store, &expired, now - 1);
        expire(&store, &pinned, now - 1);
        expire(&store, &later, now + 60_000);
        store.set_pinned(&pinned, true).unwrap();

        let mut listed: Vec<String> = store
            .list_from_bounded(None, 50, usize::MAX)
            .unwrap()
            .items
            .into_iter()
            .map(|row| row.id)
            .collect();
        listed.sort();
        let mut visible = vec![live.clone(), pinned.clone(), later.clone()];
        visible.sort();
        assert_eq!(listed, visible, "list");

        let mut searched: Vec<String> = store
            .search("alpha", 50)
            .unwrap()
            .into_iter()
            .map(|row| row.id)
            .collect();
        searched.sort();
        assert_eq!(searched, visible, "search");

        let mut queried: Vec<String> = store
            .query_history_bounded(&Default::default(), None, 50, usize::MAX)
            .unwrap()
            .items
            .into_iter()
            .map(|row| row.id)
            .collect();
        queried.sort();
        assert_eq!(queried, visible, "history query");
        assert!(store.get(&expired).unwrap().is_some(), "not purged yet");
    }

    #[test]
    fn annotations_merge_local_and_synced_state_and_keep_clears_expiry() {
        let store = store();
        let id = store.insert(item("annotated", T0)).unwrap().id;
        assert_eq!(
            store
                .item_annotations(std::slice::from_ref(&id))
                .unwrap()
                .len(),
            0
        );
        store
            .write_clip_local_state(&ClipLocalState {
                clip_id: id.clone(),
                expires_at: Some(T0 + 10),
                rule_secret: true,
                ..Default::default()
            })
            .unwrap();
        let annotations = store.item_annotations(std::slice::from_ref(&id)).unwrap()[&id].clone();
        assert_eq!(annotations.expires_at, Some(T0 + 10));
        assert!(annotations.rule_secret && !annotations.keep);
        assert!(!store
            .write_clip_local_state(&ClipLocalState {
                clip_id: "missing".into(),
                ..Default::default()
            })
            .unwrap());
    }

    #[test]
    fn the_next_deadline_skips_past_values_and_housekeeping_prunes_due_rows() {
        let store = store();
        let now = T0 + RETAIN_MS * 2;
        let id = store.insert(item("expiring", T0)).unwrap().id;
        assert_eq!(store.next_local_deadline(now).unwrap(), None);
        expire(&store, &id, now - 5);
        assert_eq!(store.next_local_deadline(now).unwrap(), None);
        {
            let conn = store.conn().unwrap();
            conn.execute(
                "INSERT INTO expired_ids(clip_id, expired_at) VALUES ('old', ?1), ('new', ?2)",
                params![now - RETAIN_MS, now - 10],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO rule_originals(clip_id, content_ciphertext, nonce, content_type, \
                 purge_at) VALUES (?1, X'00', X'00', 'text', ?2)",
                params![id, now],
            )
            .unwrap();
        }
        assert_eq!(
            store.next_local_deadline(now).unwrap(),
            Some(now - 10 + RETAIN_MS)
        );
        let swept = store.sweep_local_state(now).unwrap();
        assert_eq!((swept.expired_ids, swept.rule_originals), (1, 1));
        expire(&store, &id, now + 1_000);
        assert_eq!(store.next_local_deadline(now).unwrap(), Some(now + 1_000));
    }
}
