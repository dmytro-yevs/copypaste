//! Filtered, cursor-paged reads over the complete retained history.
//!
//! This is deliberately a store query rather than a client-side filter over
//! [`super::Store::list_from`]: a matching row after the first lazy page must
//! remain reachable. The opaque cursor includes a digest of normalized query
//! options, so changing a filter cannot accidentally resume another result set.

use std::collections::BTreeMap;

use rusqlite::{params_from_iter, types::Value};
use sha2::{Digest, Sha256};

use super::model::{item_columns_ci, row_to_item, ItemColumns, StoreError, StoredItem};
use super::search::sanitize_fts5_query;
use super::store::Store;

pub use copypaste_ipc::{
    HistoryDeviceFacet, HistoryFacets, HistoryQuery, HistorySort, HistorySourceAppFacet,
};

/// Where a filtered history query stopped.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct HistoryCursor {
    fingerprint: String,
    sort: HistorySort,
    pinned: bool,
    pin_order: Option<f64>,
    created_at: i64,
    id: String,
    relevance: Option<f64>,
}

impl HistoryCursor {
    /// An opaque transport representation.
    #[must_use]
    pub fn token(&self) -> String {
        hex::encode(serde_json::to_vec(self).unwrap_or_default())
    }

    /// Parse a cursor only when it belongs to `query`.
    ///
    /// # Errors
    ///
    /// [`StoreError::InvalidCursor`] when the marker was malformed or belongs
    /// to another filter/order set.
    pub fn parse_for(token: &str, query: &HistoryQuery) -> Result<Self, StoreError> {
        let bytes = hex::decode(token).map_err(|_| StoreError::InvalidCursor)?;
        let cursor: Self = serde_json::from_slice(&bytes).map_err(|_| StoreError::InvalidCursor)?;
        (cursor.fingerprint == query_fingerprint(query))
            .then_some(cursor)
            .ok_or(StoreError::InvalidCursor)
    }
}

/// A result page and its query-bound continuation marker.
#[derive(Debug, Clone)]
pub struct HistoryPage {
    pub items: Vec<StoredItem>,
    pub next: Option<HistoryCursor>,
}

#[derive(serde::Serialize)]
struct FingerprintQuery<'a> {
    search: Option<String>,
    content_classes: Vec<&'static str>,
    semantic_kinds: Vec<&'static str>,
    pinned_only: bool,
    origin_device_id: &'a Option<String>,
    source_app_bundle_id: &'a Option<String>,
    sort: HistorySort,
}

fn class_key(class: copypaste_ipc::ContentClass) -> &'static str {
    match class {
        copypaste_ipc::ContentClass::Text => "text",
        copypaste_ipc::ContentClass::Image => "image",
        copypaste_ipc::ContentClass::File => "file",
        copypaste_ipc::ContentClass::Other => "other",
    }
}

fn normalized_classes(query: &HistoryQuery) -> Vec<&'static str> {
    let mut classes = query
        .content_classes
        .iter()
        .copied()
        .map(class_key)
        .collect::<Vec<_>>();
    classes.sort_unstable();
    classes.dedup();
    classes
}

fn normalized_semantic_kinds(query: &HistoryQuery) -> Vec<&'static str> {
    let mut kinds = query
        .semantic_kinds
        .iter()
        .copied()
        .map(copypaste_ipc::SemanticKind::as_str)
        .collect::<Vec<_>>();
    kinds.sort_unstable();
    kinds.dedup();
    kinds
}

fn normalized_search(query: &HistoryQuery) -> Option<String> {
    query.search.as_deref().and_then(sanitize_fts5_query)
}

fn query_fingerprint(query: &HistoryQuery) -> String {
    let normalized = FingerprintQuery {
        search: normalized_search(query),
        content_classes: normalized_classes(query),
        semantic_kinds: normalized_semantic_kinds(query),
        pinned_only: query.pinned_only,
        origin_device_id: &query.origin_device_id,
        source_app_bundle_id: &query.source_app_bundle_id,
        sort: query.sort,
    };
    let bytes = serde_json::to_vec(&normalized).unwrap_or_default();
    hex::encode(Sha256::digest(bytes))
}

fn class_clause(classes: &[&str]) -> Option<&'static str> {
    match classes {
        [] => None,
        ["text"] => Some("(ci.content_type = 'text' OR ci.content_type LIKE 'text/%')"),
        ["image"] => Some("ci.content_type LIKE 'image/%'"),
        ["file"] => Some("ci.content_type = 'file'"),
        ["other"] => Some("NOT (ci.content_type = 'text' OR ci.content_type LIKE 'text/%' OR ci.content_type LIKE 'image/%' OR ci.content_type = 'file')"),
        _ => Some("("),
    }
}

fn push_class_clause(sql: &mut String, classes: &[&str]) {
    let Some(clause) = class_clause(classes) else {
        return;
    };
    if clause != "(" {
        sql.push_str(" AND ");
        sql.push_str(clause);
        return;
    }

    sql.push_str(" AND (");
    for (index, class) in classes.iter().enumerate() {
        if index != 0 {
            sql.push_str(" OR ");
        }
        sql.push_str(match *class {
            "text" => "(ci.content_type = 'text' OR ci.content_type LIKE 'text/%')",
            "image" => "ci.content_type LIKE 'image/%'",
            "file" => "ci.content_type = 'file'",
            "other" => "NOT (ci.content_type = 'text' OR ci.content_type LIKE 'text/%' OR ci.content_type LIKE 'image/%' OR ci.content_type = 'file')",
            _ => unreachable!("normalized content class"),
        });
    }
    sql.push(')');
}

fn cursor_of(
    item: &StoredItem,
    fingerprint: String,
    sort: HistorySort,
    relevance: Option<f64>,
) -> HistoryCursor {
    HistoryCursor {
        fingerprint,
        sort,
        pinned: item.pinned,
        pin_order: item.pin_order,
        created_at: item.created_at,
        id: item.id.clone(),
        relevance,
    }
}

fn pin_order_value(pin_order: Option<f64>) -> Value {
    pin_order.map_or(Value::Null, Value::Real)
}

/// Continue after a row in the pinned run, then include the complete unpinned
/// run. Pinned order is independent of the selected unpinned sort.
fn push_after_pinned(
    sql: &mut String,
    values: &mut Vec<Value>,
    cursor: &HistoryCursor,
    qualifier: &str,
) {
    let pinned = format!("{qualifier}pinned");
    let pin_order = format!("{qualifier}pin_order");
    let created_at = format!("{qualifier}created_at");
    let id = format!("{qualifier}id");
    sql.push_str(&format!(
        " AND ({pinned} = 0 OR ({pinned} = 1 AND \
         (({pin_order} IS NOT NULL AND (? IS NULL OR {pin_order} > ?)) \
          OR ({pin_order} IS ? AND {created_at} < ?) \
          OR ({pin_order} IS ? AND {created_at} = ? AND {id} < ?))))"
    ));
    let pin_order = pin_order_value(cursor.pin_order);
    values.extend([
        pin_order.clone(),
        pin_order.clone(),
        pin_order.clone(),
        Value::Integer(cursor.created_at),
        pin_order,
        Value::Integer(cursor.created_at),
        Value::Text(cursor.id.clone()),
    ]);
}

impl Store {
    /// List selectable source and origin facets from the complete live history.
    ///
    /// Labels are mandatory: unknown device/app metadata is deliberately not
    /// returned as a selectable raw identifier. `local_device_id` maps the
    /// storage representation for local captures (an empty origin id) to the
    /// stable id clients receive on [`copypaste_ipc::Item`].
    pub fn history_facets(
        &self,
        local_device_id: &str,
        local_device_name: &str,
        local_device_class: copypaste_ipc::DeviceClass,
    ) -> Result<HistoryFacets, StoreError> {
        let conn = self.conn()?;
        let mut origins = Vec::new();
        let mut origin_stmt = conn.prepare(
            "SELECT ci.origin_device_id, dn.name, dc.value \
             FROM clipboard_items ci \
             LEFT JOIN sync_device_name dn ON dn.device_id = ci.origin_device_id \
             LEFT JOIN sync_device_state dc ON dc.key = 'device_class:' || ci.origin_device_id \
             WHERE ci.deleted = 0 \
             GROUP BY ci.origin_device_id, dn.name, dc.value",
        )?;
        let origin_rows = origin_stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<String>>(2)?,
            ))
        })?;
        for row in origin_rows {
            let (stored_id, known_label, known_class) = row?;
            let (id, label) = if stored_id.is_empty() {
                (local_device_id, Some(local_device_name))
            } else {
                (stored_id.as_str(), known_label.as_deref())
            };
            if !id.is_empty() {
                if let Some(label) = label.filter(|label| !label.trim().is_empty()) {
                    origins.push(HistoryDeviceFacet {
                        id: id.to_owned(),
                        label: label.to_owned(),
                        device_class: if id == local_device_id {
                            local_device_class
                        } else {
                            known_class.as_deref().map_or(
                                copypaste_ipc::DeviceClass::Unknown,
                                copypaste_ipc::DeviceClass::from_wire_name,
                            )
                        },
                    });
                }
            }
        }

        // The most recently captured label wins when an app renamed itself;
        // the stable bundle/package id remains the selected value.
        let mut apps = BTreeMap::new();
        let mut app_stmt = conn.prepare(
            "SELECT id, app_bundle_id, app_name, payload_metadata FROM clipboard_items \
             WHERE deleted = 0 AND app_bundle_id IS NOT NULL AND app_name IS NOT NULL \
             ORDER BY app_bundle_id ASC, created_at DESC, id DESC",
        )?;
        let app_rows = app_stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })?;
        for row in app_rows {
            let (item_id, id, label, payload_metadata) = row?;
            if !id.is_empty() && !label.trim().is_empty() {
                let has_icon = payload_metadata
                    .as_deref()
                    .and_then(|value| serde_json::from_str::<crate::PayloadMetadata>(value).ok())
                    .is_some_and(|metadata| metadata.source_app_icon.is_some());
                let entry = apps.entry(id).or_insert_with(|| (label, None));
                if entry.1.is_none() && has_icon {
                    entry.1 = Some(item_id);
                }
            }
        }
        let mut source_apps = apps
            .into_iter()
            .map(|(id, (label, icon_item_id))| HistorySourceAppFacet {
                id,
                label,
                icon_item_id,
            })
            .collect::<Vec<_>>();
        origins.sort_by(|left, right| left.label.cmp(&right.label).then(left.id.cmp(&right.id)));
        source_apps
            .sort_by(|left, right| left.label.cmp(&right.label).then(left.id.cmp(&right.id)));
        Ok(HistoryFacets {
            origin_devices: origins,
            source_apps,
        })
    }

    /// Read a byte-bounded page of the whole history after applying `query`.
    ///
    /// Search runs against FTS before filtering and pagination. Every other
    /// predicate runs in the same SQLite read; no loaded subset is ever used as
    /// the source of a filter. Relevance ordering requires a nonempty text
    /// query because binary rows have no plaintext search index.
    pub fn query_history_bounded(
        &self,
        query: &HistoryQuery,
        after: Option<&HistoryCursor>,
        limit: u32,
        budget: usize,
    ) -> Result<HistoryPage, StoreError> {
        self.query_history_bounded_for_device(query, after, limit, budget, None)
    }

    /// Device-aware counterpart of [`Self::query_history_bounded`].
    ///
    /// Local captures are stored with an empty origin id, while IPC correctly
    /// presents the real local device id. The transport owner supplies that id
    /// here so selecting the local facet filters the same identity users see.
    pub fn query_history_bounded_for_device(
        &self,
        query: &HistoryQuery,
        after: Option<&HistoryCursor>,
        limit: u32,
        budget: usize,
        local_device_id: Option<&str>,
    ) -> Result<HistoryPage, StoreError> {
        if limit == 0 {
            return Ok(HistoryPage {
                items: Vec::new(),
                next: None,
            });
        }
        if after.is_some_and(|cursor| {
            cursor.fingerprint != query_fingerprint(query) || cursor.sort != query.sort
        }) {
            return Err(StoreError::InvalidCursor);
        }

        let search = normalized_search(query);
        if query.sort == HistorySort::Relevance && search.is_none() {
            return Err(StoreError::InvalidHistoryQuery);
        }
        let classes = normalized_classes(query);
        let semantic_kinds = normalized_semantic_kinds(query);
        let fingerprint = query_fingerprint(query);
        let mut values = Vec::<Value>::new();
        let mut filters = String::from("ci.deleted = 0");
        if let Some(search) = &search {
            // The FTS table is maintained only for text captures, but an
            // existing database can carry a stale legacy row. Keep this guard
            // aligned with `Store::search`: a text query must never surface an
            // image, file, or future binary merely because it has an index row.
            filters.push_str(
                " AND clipboard_fts MATCH ? \
                 AND (ci.content_type = 'text' OR ci.content_type LIKE 'text/%')",
            );
            values.push(Value::Text(search.clone()));
        }
        push_class_clause(&mut filters, &classes);
        if !semantic_kinds.is_empty() {
            filters.push_str(
                " AND copypaste_semantic_kind(ci.content_type, clipboard_fts.content_text) IN (",
            );
            for (index, kind) in semantic_kinds.iter().enumerate() {
                if index != 0 {
                    filters.push_str(", ");
                }
                filters.push('?');
                values.push(Value::Text((*kind).to_owned()));
            }
            filters.push(')');
        }
        if query.pinned_only {
            filters.push_str(" AND ci.pinned = 1");
        }
        if let Some(origin) = &query.origin_device_id {
            if Some(origin.as_str()) == local_device_id {
                filters.push_str(" AND ci.origin_device_id = ''");
            } else {
                filters.push_str(" AND ci.origin_device_id = ?");
                values.push(Value::Text(origin.clone()));
            }
        }
        if let Some(source) = &query.source_app_bundle_id {
            filters.push_str(" AND ci.app_bundle_id = ?");
            values.push(Value::Text(source.clone()));
        }

        let mut sql = String::new();
        if query.sort == HistorySort::Relevance {
            sql.push_str("SELECT * FROM (SELECT ");
            sql.push_str(item_columns_ci!());
            sql.push_str(", bm25(clipboard_fts) AS relevance FROM clipboard_fts JOIN clipboard_items ci ON ci.id = clipboard_fts.id WHERE ");
            sql.push_str(&filters);
            sql.push_str(") ranked WHERE 1 = 1");
            if let Some(cursor) = after {
                if cursor.pinned {
                    push_after_pinned(&mut sql, &mut values, cursor, "");
                } else {
                    let relevance = cursor.relevance.ok_or(StoreError::InvalidCursor)?;
                    sql.push_str(" AND pinned = 0 AND (relevance > ? OR (relevance = ? AND (created_at < ? OR (created_at = ? AND id < ?))))");
                    values.extend([
                        Value::Real(relevance),
                        Value::Real(relevance),
                        Value::Integer(cursor.created_at),
                        Value::Integer(cursor.created_at),
                        Value::Text(cursor.id.clone()),
                    ]);
                }
            }
            sql.push_str(
                " ORDER BY pinned DESC, pin_order ASC, \
                 CASE WHEN pinned = 0 THEN relevance END ASC, \
                 created_at DESC, id DESC LIMIT ?",
            );
        } else {
            sql.push_str("SELECT ");
            sql.push_str(item_columns_ci!());
            sql.push_str(" FROM clipboard_items ci");
            if search.is_some() || !semantic_kinds.is_empty() {
                sql.push_str(" JOIN clipboard_fts ON ci.id = clipboard_fts.id");
            }
            sql.push_str(" WHERE ");
            sql.push_str(&filters);
            if let Some(cursor) = after {
                if cursor.pinned {
                    push_after_pinned(&mut sql, &mut values, cursor, "ci.");
                } else {
                    match query.sort {
                        HistorySort::Newest => {
                            sql.push_str(
                                " AND ci.pinned = 0 AND (ci.created_at < ? OR (ci.created_at = ? AND ci.id < ?))",
                            );
                        }
                        HistorySort::Oldest => {
                            sql.push_str(
                                " AND ci.pinned = 0 AND (ci.created_at > ? OR (ci.created_at = ? AND ci.id > ?))",
                            );
                        }
                        HistorySort::Relevance => unreachable!("handled above"),
                    }
                    values.extend([
                        Value::Integer(cursor.created_at),
                        Value::Integer(cursor.created_at),
                        Value::Text(cursor.id.clone()),
                    ]);
                }
            }
            match query.sort {
                HistorySort::Newest => sql.push_str(
                    " ORDER BY ci.pinned DESC, ci.pin_order ASC, \
                     ci.created_at DESC, ci.id DESC LIMIT ?",
                ),
                HistorySort::Oldest => sql.push_str(
                    " ORDER BY ci.pinned DESC, ci.pin_order ASC, \
                     CASE WHEN ci.pinned = 1 THEN ci.created_at END DESC, \
                     CASE WHEN ci.pinned = 0 THEN ci.created_at END ASC, \
                     CASE WHEN ci.pinned = 1 THEN ci.id END DESC, \
                     CASE WHEN ci.pinned = 0 THEN ci.id END ASC LIMIT ?",
                ),
                HistorySort::Relevance => unreachable!("handled above"),
            }
        }
        values.push(Value::Integer(i64::from(limit).saturating_add(1)));

        let conn = self.conn()?;
        let mut stmt = conn.prepare(&sql)?;
        let columns = ItemColumns::resolve(&stmt)?;
        let mut rows = stmt.query(params_from_iter(values.iter()))?;
        let mut items = Vec::with_capacity(limit as usize);
        let mut bytes = 0usize;
        let mut more = false;
        let mut last_relevance = None;
        while let Some(row) = rows.next()? {
            if items.len() == limit as usize {
                more = true;
                break;
            }
            let item = row_to_item(row, &columns)?;
            let item_bytes = item
                .content_ciphertext
                .len()
                .saturating_add(item.payload_metadata.as_ref().map_or(0, String::len));
            if !items.is_empty() && bytes.saturating_add(item_bytes) > budget {
                more = true;
                break;
            }
            bytes = bytes.saturating_add(item_bytes);
            if query.sort == HistorySort::Relevance {
                last_relevance = Some(row.get::<_, f64>("relevance")?);
            }
            items.push(item);
        }
        let next = more
            .then(|| {
                items
                    .last()
                    .map(|item| cursor_of(item, fingerprint, query.sort, last_relevance))
            })
            .flatten();
        Ok(HistoryPage { items, next })
    }
}

#[cfg(test)]
mod tests {
    use base64::{engine::general_purpose::STANDARD, Engine as _};

    use super::*;
    use crate::storage::test_support::{item, store, T0};

    fn walk(store: &Store, query: &HistoryQuery) -> Vec<String> {
        walk_with_page_size(store, query, 2)
    }

    fn walk_with_page_size(store: &Store, query: &HistoryQuery, page_size: u32) -> Vec<String> {
        let mut cursor = None;
        let mut ids = Vec::new();
        loop {
            let page = store
                .query_history_bounded(query, cursor.as_ref(), page_size, usize::MAX)
                .unwrap();
            ids.extend(page.items.iter().map(|item| item.id.clone()));
            let Some(next) = page.next else { break };
            cursor = Some(next);
        }
        ids
    }

    #[test]
    fn filters_the_complete_history_before_paging() {
        let store = store();
        let text = store.insert(item("needle text", T0)).unwrap();
        let image = store
            .insert(super::super::NewItem {
                content_type: "image/png".into(),
                search_text: None,
                content_hash: "image".into(),
                id: "image".into(),
                content_ciphertext: vec![1],
                nonce: vec![1],
                created_at: T0 + 2,
                app_bundle_id: Some("com.example.camera".into()),
                app_name: None,
                payload_metadata: None,
            })
            .unwrap();
        let file = store
            .insert(super::super::NewItem {
                content_type: "file".into(),
                search_text: None,
                content_hash: "file".into(),
                id: "file".into(),
                content_ciphertext: vec![1],
                nonce: vec![1],
                created_at: T0 + 1,
                app_bundle_id: Some("com.example.files".into()),
                app_name: None,
                payload_metadata: Some(
                    r#"{"filename":"report.pdf","mime_type":"application/pdf"}"#.into(),
                ),
            })
            .unwrap();
        let other = store
            .insert(super::super::NewItem {
                content_type: "application/x-future".into(),
                search_text: None,
                content_hash: "other".into(),
                id: "other".into(),
                content_ciphertext: vec![1],
                nonce: vec![1],
                created_at: T0 + 3,
                app_bundle_id: None,
                app_name: None,
                payload_metadata: None,
            })
            .unwrap();
        store
            .conn()
            .unwrap()
            .execute(
                "UPDATE clipboard_items SET origin_device_id = ?2 WHERE id = ?1",
                rusqlite::params![other.id, "remote-device"],
            )
            .unwrap();
        store.set_pinned(&file.id, true).unwrap();

        let query = HistoryQuery {
            content_classes: vec![copypaste_ipc::ContentClass::File],
            pinned_only: true,
            source_app_bundle_id: Some("com.example.files".into()),
            ..HistoryQuery::default()
        };
        assert_eq!(walk(&store, &query), vec![file.id]);
        let image_query = HistoryQuery {
            content_classes: vec![copypaste_ipc::ContentClass::Image],
            ..HistoryQuery::default()
        };
        assert_eq!(walk(&store, &image_query), vec![image.id]);
        let text_query = HistoryQuery {
            content_classes: vec![copypaste_ipc::ContentClass::Text],
            ..HistoryQuery::default()
        };
        assert_eq!(walk(&store, &text_query), vec![text.id]);
        let other_query = HistoryQuery {
            content_classes: vec![copypaste_ipc::ContentClass::Other],
            origin_device_id: Some("remote-device".into()),
            ..HistoryQuery::default()
        };
        assert_eq!(walk(&store, &other_query), vec![other.id]);
    }

    #[test]
    fn filters_existing_text_rows_by_semantic_kind_before_paging() {
        let store = store();
        let plain = store.insert(item("meeting notes", T0)).unwrap();
        let link = store
            .insert(item("https://example.com/docs", T0 + 1))
            .unwrap();
        let email = store.insert(item("person@example.com", T0 + 2)).unwrap();
        let color = store.insert(item("oklch(50% 0.1 30)", T0 + 3)).unwrap();
        let code = store
            .insert(item("fn main() { println!(\"hello\"); }", T0 + 4))
            .unwrap();

        for (kind, expected) in [
            (copypaste_ipc::SemanticKind::PlainText, plain.id),
            (copypaste_ipc::SemanticKind::Link, link.id),
            (copypaste_ipc::SemanticKind::Email, email.id),
            (copypaste_ipc::SemanticKind::Color, color.id),
            (copypaste_ipc::SemanticKind::Code, code.id),
        ] {
            let query = HistoryQuery {
                semantic_kinds: vec![kind],
                ..HistoryQuery::default()
            };
            assert_eq!(walk(&store, &query), vec![expected], "{kind:?}");
        }
    }

    #[test]
    fn search_paginates_deterministically_and_rejects_a_different_query_cursor() {
        let store = store();
        let first = store.insert(item("needle first", T0)).unwrap();
        let second = store.insert(item("needle second", T0 + 1)).unwrap();
        let third = store.insert(item("needle third", T0 + 2)).unwrap();
        let query = HistoryQuery {
            search: Some("needle".into()),
            sort: HistorySort::Newest,
            ..HistoryQuery::default()
        };
        let page = store
            .query_history_bounded(&query, None, 2, usize::MAX)
            .unwrap();
        assert_eq!(
            page.items.iter().map(|item| &item.id).collect::<Vec<_>>(),
            vec![&third.id, &second.id]
        );
        let cursor = page.next.unwrap();
        let rest = store
            .query_history_bounded(&query, Some(&cursor), 2, usize::MAX)
            .unwrap();
        assert_eq!(
            rest.items.iter().map(|item| &item.id).collect::<Vec<_>>(),
            vec![&first.id]
        );
        let other = HistoryQuery {
            search: Some("other".into()),
            ..query.clone()
        };
        assert!(matches!(
            store.query_history_bounded(&other, Some(&cursor), 2, usize::MAX),
            Err(StoreError::InvalidCursor)
        ));
    }

    #[test]
    fn every_sort_keeps_pins_first_in_pin_order_across_page_boundaries() {
        let store = store();
        let oldest = store.insert(item("needle oldest", T0)).unwrap();
        let first_pin = store.insert(item("needle first pin", T0 + 1)).unwrap();
        let second_pin = store.insert(item("needle second pin", T0 + 2)).unwrap();
        let newest = store.insert(item("needle newest", T0 + 3)).unwrap();
        store.set_pinned(&second_pin.id, true).unwrap();
        store.set_pinned(&first_pin.id, true).unwrap();

        for sort in [
            HistorySort::Newest,
            HistorySort::Oldest,
            HistorySort::Relevance,
        ] {
            let query = HistoryQuery {
                search: Some("needle".into()),
                sort,
                ..HistoryQuery::default()
            };
            let ids = walk_with_page_size(&store, &query, 1);
            assert_eq!(
                &ids[..2],
                &[second_pin.id.clone(), first_pin.id.clone()],
                "sort: {sort:?}"
            );
            assert_eq!(ids.len(), 4, "sort: {sort:?}");
            assert!(ids.contains(&oldest.id), "sort: {sort:?}");
            assert!(ids.contains(&newest.id), "sort: {sort:?}");
        }
    }

    #[test]
    fn relevance_requires_search_and_orders_ranked_results_with_a_cursor() {
        let store = store();
        store.insert(item("needle needle", T0)).unwrap();
        store.insert(item("needle", T0 + 1)).unwrap();
        store.insert(item("needle unrelated", T0 + 2)).unwrap();
        let query = HistoryQuery {
            search: Some("needle".into()),
            sort: HistorySort::Relevance,
            ..HistoryQuery::default()
        };
        let ids = walk(&store, &query);
        assert_eq!(ids.len(), 3);
        assert_eq!(
            ids.iter().collect::<std::collections::HashSet<_>>().len(),
            3
        );
        assert!(matches!(
            store.query_history_bounded(
                &HistoryQuery {
                    sort: HistorySort::Relevance,
                    ..HistoryQuery::default()
                },
                None,
                2,
                usize::MAX
            ),
            Err(StoreError::InvalidHistoryQuery)
        ));
    }

    #[test]
    fn search_never_surfaces_a_live_non_text_row_with_a_stale_fts_entry() {
        let store = store();
        let text = store.insert(item("needle text", T0)).unwrap();
        let image = store
            .insert(super::super::NewItem {
                content_type: "image/png".into(),
                search_text: None,
                content_hash: "stale-image".into(),
                id: "stale-image".into(),
                content_ciphertext: vec![1],
                nonce: vec![1],
                created_at: T0 + 1,
                app_bundle_id: None,
                app_name: None,
                payload_metadata: None,
            })
            .unwrap();
        super::super::test_support::plant_fts_row(&store, &image.id, "needle image");

        for sort in [
            HistorySort::Newest,
            HistorySort::Oldest,
            HistorySort::Relevance,
        ] {
            let query = HistoryQuery {
                search: Some("needle".into()),
                sort,
                ..HistoryQuery::default()
            };
            let page = store
                .query_history_bounded(&query, None, 10, usize::MAX)
                .unwrap();
            assert_eq!(
                page.items.iter().map(|item| &item.id).collect::<Vec<_>>(),
                vec![&text.id],
                "sort: {sort:?}"
            );
        }
    }

    #[test]
    fn oldest_order_and_cursor_tokens_are_query_bound() {
        let store = store();
        let oldest = store.insert(item("old", T0)).unwrap();
        let newest = store.insert(item("new", T0 + 1)).unwrap();
        let query = HistoryQuery {
            sort: HistorySort::Oldest,
            ..HistoryQuery::default()
        };
        let page = store
            .query_history_bounded(&query, None, 1, usize::MAX)
            .unwrap();
        assert_eq!(page.items[0].id, oldest.id);
        let token = page.next.unwrap().token();
        let cursor = HistoryCursor::parse_for(&token, &query).unwrap();
        let next = store
            .query_history_bounded(&query, Some(&cursor), 1, usize::MAX)
            .unwrap();
        assert_eq!(next.items[0].id, newest.id);
        assert!(matches!(
            HistoryCursor::parse_for(
                &token,
                &HistoryQuery {
                    sort: HistorySort::Newest,
                    ..HistoryQuery::default()
                }
            ),
            Err(StoreError::InvalidCursor)
        ));
    }

    #[test]
    fn facets_cover_unloaded_history_with_labels_and_stable_ids() {
        let store = store();
        let mut local_old = item("local old", T0);
        local_old.app_bundle_id = Some("com.example.editor".into());
        local_old.app_name = Some("Old Editor".into());
        store.insert(local_old).unwrap();

        let mut remote = item("remote", T0 + 1);
        remote.app_bundle_id = Some("com.example.viewer".into());
        remote.app_name = Some("Viewer".into());
        let remote = store.insert(remote).unwrap();
        store
            .conn()
            .unwrap()
            .execute(
                "UPDATE clipboard_items SET origin_device_id = ?2 WHERE id = ?1",
                rusqlite::params![remote.id, "remote-device"],
            )
            .unwrap();
        store.record_device_name("remote-device", "Phone").unwrap();
        store
            .record_device_class("remote-device", copypaste_ipc::DeviceClass::Phone)
            .unwrap();

        let mut local_new = item("local new", T0 + 2);
        local_new.app_bundle_id = Some("com.example.editor".into());
        local_new.app_name = Some("Editor".into());
        let icon_png = STANDARD
            .decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNgYAAAAAMAASsJTYQAAAAASUVORK5CYII=")
            .unwrap();
        local_new.payload_metadata =
            crate::PayloadMetadata::new(None, crate::SourceAppIconMetadata::new(&icon_png, 1, 1))
                .and_then(|metadata| metadata.to_json(copypaste_ipc::content_type::TEXT));
        let local_new = store.insert(local_new).unwrap();

        let facets = store
            .history_facets("local-device", "Laptop", copypaste_ipc::DeviceClass::Laptop)
            .unwrap();
        assert_eq!(
            facets.origin_devices,
            vec![
                HistoryDeviceFacet {
                    id: "local-device".into(),
                    label: "Laptop".into(),
                    device_class: copypaste_ipc::DeviceClass::Laptop,
                },
                HistoryDeviceFacet {
                    id: "remote-device".into(),
                    label: "Phone".into(),
                    device_class: copypaste_ipc::DeviceClass::Phone,
                },
            ]
        );
        assert_eq!(
            facets.source_apps,
            vec![
                HistorySourceAppFacet {
                    id: "com.example.editor".into(),
                    label: "Editor".into(),
                    icon_item_id: Some(local_new.id),
                },
                HistorySourceAppFacet {
                    id: "com.example.viewer".into(),
                    label: "Viewer".into(),
                    icon_item_id: None,
                },
            ]
        );

        let local_query = HistoryQuery {
            origin_device_id: Some("local-device".into()),
            ..HistoryQuery::default()
        };
        let local_page = store
            .query_history_bounded_for_device(
                &local_query,
                None,
                10,
                usize::MAX,
                Some("local-device"),
            )
            .unwrap();
        assert_eq!(local_page.items.len(), 2);
        assert!(local_page
            .items
            .iter()
            .all(|item| item.origin_device_id.is_empty()));
    }

    #[test]
    fn keyset_ties_at_one_item_pages_never_repeat_or_skip() {
        let store = store();
        let created_at = T0 + 50;
        let ids = ["tie-a", "tie-b", "tie-c"];
        for id in ids {
            store
                .insert(super::super::NewItem {
                    id: id.into(),
                    content_ciphertext: format!("ct:{id}").into_bytes(),
                    nonce: vec![1],
                    content_type: "text".into(),
                    content_hash: format!("hash:{id}"),
                    search_text: Some(format!("needle {id}")),
                    created_at,
                    app_bundle_id: None,
                    app_name: None,
                    payload_metadata: None,
                })
                .unwrap();
        }

        for (sort, expected) in [
            (HistorySort::Newest, vec!["tie-c", "tie-b", "tie-a"]),
            (HistorySort::Oldest, vec!["tie-a", "tie-b", "tie-c"]),
        ] {
            let query = HistoryQuery {
                sort,
                ..HistoryQuery::default()
            };
            assert_eq!(
                walk_with_page_size(&store, &query, 1),
                expected,
                "sort: {sort:?}"
            );
        }

        let ranks = store
            .conn()
            .unwrap()
            .prepare("SELECT bm25(clipboard_fts) FROM clipboard_fts WHERE clipboard_fts MATCH ?1")
            .unwrap()
            .query_map(["needle"], |row| row.get::<_, f64>(0))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        assert_eq!(ranks.len(), ids.len());
        assert!(ranks.windows(2).all(|pair| pair[0] == pair[1]));

        let relevance = HistoryQuery {
            search: Some("needle".into()),
            sort: HistorySort::Relevance,
            ..HistoryQuery::default()
        };
        assert_eq!(
            walk_with_page_size(&store, &relevance, 1),
            vec!["tie-c", "tie-b", "tie-a"]
        );
    }
}
