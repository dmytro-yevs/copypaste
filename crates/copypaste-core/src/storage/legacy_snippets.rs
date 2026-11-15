//! The snippet library as the app kept it before snippets became a module,
//! read once by the module that takes it over.

use rusqlite::params;

use super::{Store, StoreError};

pub const MAX_LEGACY_SNIPPET_PAGE: u32 = 100;

#[derive(Debug, Clone, PartialEq)]
pub struct LegacySnippetRow {
    pub id: String,
    pub collection_id: Option<String>,
    pub title: String,
    pub abbreviation: Option<String>,
    pub template: bool,
    pub pinned: bool,
    pub pin_order: Option<f64>,
    pub order_key: String,
    pub concealed: bool,
    pub deleted_at_ms: Option<i64>,
    pub created_at_ms: i64,
    pub created_device: String,
    pub revision: String,
    pub ancestors: Vec<String>,
    pub text: String,
    pub rich_html: Option<String>,
    pub use_count: u64,
    pub last_used_at_ms: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyCollectionRow {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub accent: String,
    pub order_key: String,
    pub deleted_at_ms: Option<i64>,
}

/// Collections come with the first page only.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LegacySnippetPage {
    pub collections: Vec<LegacyCollectionRow>,
    pub snippets: Vec<LegacySnippetRow>,
    /// The last id returned when more may follow.
    pub next: Option<String>,
}

impl Store {
    /// Unpurged snippets with their bodies and local use, ordered by id,
    /// after `after`.
    pub fn legacy_snippet_page(
        &self,
        after: Option<&str>,
        limit: u32,
    ) -> Result<LegacySnippetPage, StoreError> {
        let limit = limit.clamp(1, MAX_LEGACY_SNIPPET_PAGE) as usize;
        let conn = self.conn()?;
        let collections = if after.is_none() {
            conn.prepare(
                "SELECT id, name, icon, accent, order_key, deleted_at FROM snippet_collections \
                 ORDER BY order_key, id",
            )?
            .query_map([], |r| {
                Ok(LegacyCollectionRow {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    icon: r.get(2)?,
                    accent: r.get(3)?,
                    order_key: r.get(4)?,
                    deleted_at_ms: r.get(5)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?
        } else {
            Vec::new()
        };
        let mut statement = conn.prepare(
            "SELECT s.id, s.collection_id, s.title, s.abbreviation, s.template, s.pinned, \
             s.pin_order, s.order_key, s.concealed, s.deleted_at, s.created_at, s.created_device, \
             b.revision, b.ancestors_json, b.body_text, b.body_rich, u.use_count, u.last_used_at \
             FROM snippets s JOIN snippet_bodies b ON b.id = s.id \
             LEFT JOIN snippet_usage u ON u.id = s.id \
             WHERE s.purged = 0 AND s.id > ?1 ORDER BY s.id LIMIT ?2",
        )?;
        let mut snippets = statement
            .query_map(params![after.unwrap_or(""), limit as i64 + 1], |r| {
                let ancestors: String = r.get(13)?;
                Ok(LegacySnippetRow {
                    id: r.get(0)?,
                    collection_id: r.get(1)?,
                    title: r.get(2)?,
                    abbreviation: r.get(3)?,
                    template: r.get(4)?,
                    pinned: r.get(5)?,
                    pin_order: r.get(6)?,
                    order_key: r.get(7)?,
                    concealed: r.get(8)?,
                    deleted_at_ms: r.get(9)?,
                    created_at_ms: r.get(10)?,
                    created_device: r.get(11)?,
                    revision: r.get(12)?,
                    ancestors: serde_json::from_str(&ancestors).unwrap_or_default(),
                    text: r.get(14)?,
                    rich_html: r.get(15)?,
                    use_count: r.get::<_, Option<i64>>(16)?.unwrap_or(0).max(0) as u64,
                    last_used_at_ms: r.get(17)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let next = (snippets.len() > limit).then(|| {
            snippets.truncate(limit);
            snippets.last().map(|row| row.id.clone())
        });
        Ok(LegacySnippetPage {
            collections,
            snippets,
            next: next.flatten(),
        })
    }
}
