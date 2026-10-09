//! Encrypted, incremental module embeddings and bounded fragment reads.

use super::{connection::write_tx, Store, StoreError};
use rusqlite::{params, OptionalExtension};
use std::collections::BTreeMap;

const FRAGMENT_BYTES: usize = 4096;

pub struct SemanticWork {
    pub item_id: String,
    pub content_hash: String,
    pub text: String,
    pub offset: usize,
    pub next_offset: usize,
    pub complete: bool,
}

/// The model identity binds continuation cursors; scores are recomputed from
/// live, content-hash-bound records before every page, never from UI previews.
pub struct SemanticMatches {
    pub scope: String,
    pub scores: Vec<(String, f64)>,
}

impl Store {
    pub fn semantic_work(&self, scope: &str) -> Result<Option<SemanticWork>, StoreError> {
        let conn = self.conn()?;
        let row = conn.query_row(
            "SELECT ci.id, ci.content_hash, CASE WHEN d.content_hash = ci.content_hash THEN d.next_offset ELSE 0 END, \
             length(CAST(fts.content_text AS BLOB)), \
             substr(CAST(fts.content_text AS BLOB), \
               CASE WHEN d.content_hash = ci.content_hash THEN d.next_offset + 1 ELSE 1 END, ?2) \
             FROM clipboard_items ci JOIN clipboard_fts fts ON fts.rowid = ci.fts_rowid \
             LEFT JOIN module_search_documents d ON d.item_id = ci.id AND d.scope = ?1 \
             WHERE ci.deleted = 0 AND (ci.content_type = 'text' OR ci.content_type LIKE 'text/%') \
             AND (d.item_id IS NULL OR d.content_hash != ci.content_hash OR d.complete = 0) \
             ORDER BY ci.created_at DESC, ci.id DESC LIMIT 1",
            params![scope, FRAGMENT_BYTES as i64],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, usize>(2)?, row.get::<_, usize>(3)?, row.get::<_, Vec<u8>>(4)?)),
        ).optional()?;
        let Some((item_id, content_hash, offset, total, mut bytes)) = row else {
            return Ok(None);
        };
        if let Err(error) = std::str::from_utf8(&bytes) {
            if error.error_len().is_some() {
                return Err(StoreError::InvalidHistoryQuery);
            }
            bytes.truncate(error.valid_up_to());
        }
        let text = String::from_utf8(bytes).map_err(|_| StoreError::InvalidHistoryQuery)?;
        let complete = offset + text.len() >= total;
        let mut advance = text.len();
        // Overlap adjoining fragments so context across their boundary survives.
        if !complete && advance > 256 {
            advance -= 256;
            while !text.is_char_boundary(advance) {
                advance -= 1;
            }
        }
        if !complete && advance == 0 {
            return Err(StoreError::InvalidHistoryQuery);
        }
        Ok(Some(SemanticWork {
            item_id,
            content_hash,
            text,
            offset,
            next_offset: offset + advance,
            complete,
        }))
    }

    /// A late worker may never index a deleted or changed clip. The caller also
    /// holds the module lifecycle lease through this publication.
    pub fn save_semantic_work(
        &self,
        module_id: &str,
        scope: &str,
        work: &SemanticWork,
        vectors: &[Vec<f32>],
    ) -> Result<bool, StoreError> {
        if vectors.is_empty()
            || vectors.len() > 128
            || vectors.iter().any(|vector| {
                vector.is_empty()
                    || vector.len() > 4096
                    || vector.iter().any(|value| !value.is_finite())
            })
        {
            return Err(StoreError::InvalidHistoryQuery);
        }
        let mut conn = self.conn()?;
        let tx = write_tx(&mut conn)?;
        let live: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM clipboard_items WHERE id = ?1 AND content_hash = ?2 AND deleted = 0 AND fts_rowid IS NOT NULL)", params![work.item_id, work.content_hash], |row| row.get(0))?;
        if !live {
            return Ok(false);
        }
        tx.execute("DELETE FROM module_search_documents WHERE scope = ?1 AND item_id = ?2 AND content_hash != ?3", params![scope, work.item_id, work.content_hash])?;
        tx.execute("INSERT OR IGNORE INTO module_search_documents(scope, module_id, item_id, content_hash) VALUES (?1, ?2, ?3, ?4)", params![scope, module_id, work.item_id, work.content_hash])?;
        let offset: usize = tx.query_row(
            "SELECT next_offset FROM module_search_documents WHERE scope = ?1 AND item_id = ?2",
            params![scope, work.item_id],
            |row| row.get(0),
        )?;
        if offset != work.offset {
            return Ok(false);
        }
        let next_chunk: i64 = tx.query_row("SELECT COALESCE(MAX(chunk) + 1, 0) FROM module_search_vectors WHERE scope = ?1 AND item_id = ?2", params![scope, work.item_id], |row| row.get(0))?;
        for (index, vector) in vectors.iter().enumerate() {
            let bytes: Vec<_> = vector
                .iter()
                .flat_map(|value| value.to_le_bytes())
                .collect();
            tx.execute("INSERT INTO module_search_vectors(scope, item_id, chunk, vector) VALUES (?1, ?2, ?3, ?4)", params![scope, work.item_id, next_chunk + index as i64, bytes])?;
        }
        tx.execute("UPDATE module_search_documents SET next_offset = ?3, complete = ?4 WHERE scope = ?1 AND item_id = ?2", params![scope, work.item_id, work.next_offset as i64, work.complete])?;
        tx.commit()?;
        Ok(true)
    }

    pub fn semantic_matches(
        &self,
        scope: &str,
        query: &[f32],
        minimum: f32,
    ) -> Result<SemanticMatches, StoreError> {
        if query.is_empty()
            || query.len() > 4096
            || query.iter().any(|value| !value.is_finite())
            || !minimum.is_finite()
            || !(0.0..=1.0).contains(&minimum)
        {
            return Err(StoreError::InvalidHistoryQuery);
        }
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT v.item_id, v.vector FROM module_search_vectors v \
            JOIN module_search_documents d ON d.scope = v.scope AND d.item_id = v.item_id \
            JOIN clipboard_items ci ON ci.id = d.item_id \
            WHERE v.scope = ?1 AND ci.deleted = 0 AND ci.fts_rowid IS NOT NULL AND ci.content_hash = d.content_hash \
            AND (ci.content_type = 'text' OR ci.content_type LIKE 'text/%')",
        )?;
        let mut rows = stmt.query([scope])?;
        let mut scores = BTreeMap::<String, f64>::new();
        while let Some(row) = rows.next()? {
            let bytes: Vec<u8> = row.get(1)?;
            if bytes.len() != query.len() * 4 {
                return Err(StoreError::InvalidHistoryQuery);
            }
            let mut score = 0.0;
            for (bytes, value) in bytes.chunks_exact(4).zip(query) {
                let component = f32::from_le_bytes(
                    bytes
                        .try_into()
                        .map_err(|_| StoreError::InvalidHistoryQuery)?,
                );
                if !component.is_finite() {
                    return Err(StoreError::InvalidHistoryQuery);
                }
                score += f64::from(component) * f64::from(*value);
            }
            if score >= f64::from(minimum) {
                let id: String = row.get(0)?;
                let best = scores.entry(id).or_insert(score);
                *best = best.max(score).min(1.0);
            }
        }
        let best = scores.values().copied().fold(f64::from(minimum), f64::max);
        scores.retain(|_, score| *score >= best - 0.10);
        Ok(SemanticMatches {
            scope: scope.into(),
            scores: scores.into_iter().collect(),
        })
    }

    pub fn clear_semantic_index(&self, module_id: &str) -> Result<(), StoreError> {
        self.conn()?.execute(
            "DELETE FROM module_search_documents WHERE module_id = ?1",
            [module_id],
        )?;
        Ok(())
    }

    pub fn prune_semantic_index(&self, module_id: &str, scope: &str) -> Result<(), StoreError> {
        self.conn()?.execute("DELETE FROM module_search_documents WHERE module_id = ?1 AND \
            (scope != ?2 OR NOT EXISTS(SELECT 1 FROM clipboard_items ci WHERE ci.id = item_id AND ci.deleted = 0 AND ci.content_hash = module_search_documents.content_hash))", params![module_id, scope])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::test_support::{item, store, T0};

    #[test]
    fn incremental_unicode_fragments_cover_the_entire_clip_and_bound_memory() {
        let s = store();
        let text = format!("{} endingmarker", "Україна 🏡 ".repeat(1500));
        let row = s.insert(item(&text, T0)).unwrap();
        let mut previous = 0;
        let mut seen_end = false;
        while let Some(work) = s.semantic_work("model-a").unwrap() {
            assert_eq!(work.item_id, row.id);
            assert_eq!(work.offset, previous);
            assert!(work.text.len() <= FRAGMENT_BYTES);
            assert!(text[work.offset..].starts_with(&work.text));
            seen_end |= work.text.contains("endingmarker");
            previous = work.next_offset;
            assert!(s
                .save_semantic_work("module-a", "model-a", &work, &[vec![1.0, 0.0]])
                .unwrap());
        }
        assert!(seen_end);
        let matches = s.semantic_matches("model-a", &[1.0, 0.0], 0.8).unwrap();
        assert_eq!(matches.scores.len(), 1);
        assert_eq!(matches.scores[0].0, row.id);
    }

    #[test]
    fn late_work_cannot_index_deleted_or_changed_content() {
        let s = store();
        let row = s.insert(item("rent for October", T0)).unwrap();
        let work = s.semantic_work("model-a").unwrap().unwrap();
        assert!(s
            .save_semantic_work("module-a", "model-a", &work, &[vec![1.0, 0.0]])
            .unwrap());
        s.delete(&row.id).unwrap();
        assert!(s
            .semantic_matches("model-a", &[1.0, 0.0], 0.8)
            .unwrap()
            .scores
            .is_empty());
        assert!(!s
            .save_semantic_work("module-a", "model-a", &work, &[vec![1.0, 0.0]])
            .unwrap());
        s.clear_semantic_index("module-a").unwrap();
        let row = s.insert(item("changed content", T0 + 1)).unwrap();
        let work = s.semantic_work("model-a").unwrap().unwrap();
        s.conn()
            .unwrap()
            .execute(
                "UPDATE clipboard_items SET content_hash = 'different' WHERE id = ?1",
                [&row.id],
            )
            .unwrap();
        assert!(!s
            .save_semantic_work("module-a", "model-a", &work, &[vec![1.0, 0.0]])
            .unwrap());
    }

    #[test]
    fn model_switch_rebuilds_index_and_rejects_invalid_query_vectors() {
        let s = store();
        s.insert(item("apartment lease", T0)).unwrap();
        let work = s.semantic_work("model-a").unwrap().unwrap();
        s.save_semantic_work("module-a", "model-a", &work, &[vec![1.0, 0.0]])
            .unwrap();
        assert!(s.semantic_work("model-a").unwrap().is_none());
        assert!(s.semantic_work("model-b").unwrap().is_some());
        s.prune_semantic_index("module-a", "model-b").unwrap();
        assert!(s
            .semantic_matches("model-a", &[1.0, 0.0], 0.8)
            .unwrap()
            .scores
            .is_empty());
        assert!(s.semantic_matches("model-a", &[f32::NAN], 0.8).is_err());
    }
}
