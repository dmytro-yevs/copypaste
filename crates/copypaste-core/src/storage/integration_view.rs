//! The history an integration (an optional module) may read.
//!
//! Visibility is one SQL predicate added to the ordinary history filters, so
//! paging, facets and single-item reads hide the same clips: secret,
//! transient, credential, hidden from integrations (synced note or local
//! rule), from another device unless included, from an excluded device or
//! source app, of a kind outside the view, or unpinned and older than the
//! lookback. Deleted and expired clips are already gone from every read.

use std::collections::HashSet;

use rusqlite::{params_from_iter, types::Value};
use serde::Serialize;

use super::history::HistoryFacets;
use super::local_state::{sql_now_ms, unexpired};
use super::{Store, StoreError};

const DAY_MS: i64 = 24 * 60 * 60 * 1000;
/// Known device names one read returns.
const MAX_DEVICE_NAMES: i64 = 512;

/// Content kinds an integration sees. Links are plain text the semantic
/// classifier recognises as a link; rich text is any other `text/*` type.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct IntegrationKinds {
    pub text: bool,
    pub link: bool,
    pub rich_text: bool,
    pub image: bool,
    pub file: bool,
}

/// What one integration request may see.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct IntegrationView {
    /// This device; local captures store an empty origin.
    pub local_device_id: String,
    pub include_other_devices: bool,
    pub excluded_devices: Vec<String>,
    pub excluded_source_apps: Vec<String>,
    pub kinds: IntegrationKinds,
    /// Unpinned clips older than this many days are invisible.
    pub lookback_days: Option<u32>,
}

/// Classifies plain text by its indexed content, as History does.
const LINK: &str = "copypaste_semantic_kind(ci.content_type, \
    (SELECT f.content_text FROM clipboard_fts f WHERE f.rowid = ci.fts_rowid)) IS 'link'";

fn json_list(values: &[String]) -> Result<Value, StoreError> {
    serde_json::to_string(values)
        .map(Value::Text)
        .map_err(|_| StoreError::InvalidHistoryQuery)
}

impl IntegrationView {
    /// Appends the visibility predicate to a `WHERE` clause over `ci`,
    /// numbering its parameters after `values`.
    pub(super) fn push_filter(
        &self,
        filters: &mut String,
        values: &mut Vec<Value>,
    ) -> Result<(), StoreError> {
        let mut param = |value: Value| {
            values.push(value);
            format!("?{}", values.len())
        };
        filters.push_str(
            " AND (ci.payload_metadata IS NULL OR (json_valid(ci.payload_metadata) \
             AND IFNULL(json_extract(ci.payload_metadata, '$.privacy.secret'), 0) = 0 \
             AND IFNULL(json_extract(ci.payload_metadata, '$.privacy.transient'), 0) = 0 \
             AND IFNULL(json_extract(ci.payload_metadata, '$.credentials'), 0) = 0)) \
             AND NOT EXISTS (SELECT 1 FROM clip_notes n WHERE n.clip_id = ci.id \
               AND n.hidden_from_integrations = 1) \
             AND NOT EXISTS (SELECT 1 FROM clip_local_state s WHERE s.clip_id = ci.id \
               AND (s.rule_secret = 1 OR s.rule_hidden_from_integrations = 1))",
        );
        // Bound only when used: SQLite counts parameters by the highest number.
        let local = if !self.include_other_devices || !self.excluded_devices.is_empty() {
            param(Value::Text(self.local_device_id.clone()))
        } else {
            String::new()
        };
        if !self.include_other_devices {
            filters.push_str(&format!(" AND ci.origin_device_id IN ('', {local})"));
        }
        if !self.excluded_devices.is_empty() {
            let devices = param(json_list(&self.excluded_devices)?);
            filters.push_str(&format!(
                " AND (CASE WHEN ci.origin_device_id = '' THEN {local} \
                 ELSE ci.origin_device_id END) NOT IN (SELECT value FROM json_each({devices}))"
            ));
        }
        if !self.excluded_source_apps.is_empty() {
            let apps = param(json_list(&self.excluded_source_apps)?);
            filters.push_str(&format!(
                " AND (ci.app_bundle_id IS NULL OR ci.app_bundle_id NOT IN \
                 (SELECT value FROM json_each({apps})))"
            ));
        }
        if let Some(days) = self.lookback_days {
            let window = param(Value::Integer(i64::from(days).saturating_mul(DAY_MS)));
            filters.push_str(&format!(
                " AND (ci.pinned = 1 OR ci.created_at >= {} - {window})",
                sql_now_ms!()
            ));
        }
        let kinds = self.kinds;
        let mut alternatives = Vec::new();
        match (kinds.text, kinds.link) {
            (true, true) => alternatives.push("ci.content_type = 'text'".to_owned()),
            (true, false) => {
                alternatives.push(format!("(ci.content_type = 'text' AND NOT {LINK})"))
            }
            (false, true) => alternatives.push(format!("(ci.content_type = 'text' AND {LINK})")),
            (false, false) => {}
        }
        for (shown, clause) in [
            (kinds.rich_text, "ci.content_type LIKE 'text/%'"),
            (kinds.image, "ci.content_type LIKE 'image/%'"),
            (kinds.file, "ci.content_type = 'file'"),
        ] {
            if shown {
                alternatives.push(clause.to_owned());
            }
        }
        if alternatives.is_empty() {
            filters.push_str(" AND 0");
        } else {
            filters.push_str(&format!(" AND ({})", alternatives.join(" OR ")));
        }
        Ok(())
    }

    /// Binds a history cursor to this view as well as to its query.
    pub(super) fn fingerprint(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

impl Store {
    /// Whether the live clip `id` is visible in `view`. Unknown and hidden
    /// ids answer alike.
    pub fn integration_visible(
        &self,
        id: &str,
        view: &IntegrationView,
    ) -> Result<bool, StoreError> {
        let mut filters = String::from(concat!(
            "ci.id = ?1 AND ci.deleted = 0 AND ",
            unexpired!("ci.")
        ));
        let mut values = vec![Value::Text(id.to_owned())];
        view.push_filter(&mut filters, &mut values)?;
        let conn = self.conn()?;
        Ok(conn.query_row(
            &format!("SELECT EXISTS(SELECT 1 FROM clipboard_items ci WHERE {filters})"),
            params_from_iter(values.iter()),
            |row| row.get(0),
        )?)
    }

    /// The source app and device facets that have a clip visible in `view`.
    pub fn integration_facets(
        &self,
        view: &IntegrationView,
        local_device_name: &str,
        local_device_class: copypaste_ipc::DeviceClass,
    ) -> Result<HistoryFacets, StoreError> {
        let mut facets =
            self.history_facets(&view.local_device_id, local_device_name, local_device_class)?;
        let mut filters = String::from(concat!("ci.deleted = 0 AND ", unexpired!("ci.")));
        let mut values = Vec::new();
        view.push_filter(&mut filters, &mut values)?;
        let local = values.len() + 1;
        values.push(Value::Text(view.local_device_id.clone()));
        let conn = self.conn()?;
        let distinct = |column: &str| -> Result<HashSet<String>, StoreError> {
            let mut statement = conn.prepare(&format!(
                "SELECT DISTINCT {column} FROM clipboard_items ci WHERE {filters} \
                 AND {column} IS NOT NULL"
            ))?;
            let rows = statement.query_map(params_from_iter(values.iter()), |row| row.get(0))?;
            Ok(rows.collect::<rusqlite::Result<_>>()?)
        };
        let devices = distinct(&format!(
            "(CASE WHEN ci.origin_device_id = '' THEN ?{local} ELSE ci.origin_device_id END)"
        ))?;
        let apps = distinct("ci.app_bundle_id")?;
        facets
            .origin_devices
            .retain(|facet| devices.contains(&facet.id));
        facets.source_apps.retain(|facet| apps.contains(&facet.id));
        facets.tags.clear();
        Ok(facets)
    }

    /// Every device this one knows a name for, by id; bounded.
    pub fn known_device_names(&self) -> Result<Vec<(String, String)>, StoreError> {
        let conn = self.conn()?;
        let mut statement = conn.prepare_cached(
            "SELECT device_id, name FROM sync_device_name ORDER BY device_id LIMIT ?1",
        )?;
        let rows = statement.query_map([MAX_DEVICE_NAMES], |row| Ok((row.get(0)?, row.get(1)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{item, store};
    use super::*;
    use crate::storage::organization::{ClipNoteRecord, Record};
    use crate::storage::HistoryQuery;

    fn view() -> IntegrationView {
        IntegrationView {
            local_device_id: "me".into(),
            kinds: IntegrationKinds {
                text: true,
                link: true,
                ..IntegrationKinds::default()
            },
            ..IntegrationView::default()
        }
    }

    #[test]
    fn hidden_secret_foreign_excluded_and_old_clips_stay_invisible() {
        let store = store();
        let keyring = crate::Keyring::from_secret(&[3; 32]);
        let key = keyring.item_key();
        let preview = keyring.preview_key();
        let keys = (&key, &preview);
        let now = crate::now_ms();
        let visible = store.insert(item("plain words", now)).unwrap().id;
        let hidden = store.insert(item("hidden words", now + 1)).unwrap().id;
        store
            .write_organization_record(
                &key,
                Record::ClipNote(ClipNoteRecord {
                    clip_id: hidden.clone(),
                    hidden_from_integrations: true,
                    hidden_changed_at: 1,
                    base_stamp: 0,
                    keep: false,
                    expires_at: None,
                    expiry_duration_ms: None,
                    provenance: None,
                }),
            )
            .unwrap();
        let mut secret = item("secret words", now + 2);
        secret.payload_metadata = Some(r#"{"privacy":{"secret":true}}"#.into());
        let secret = store.insert(secret).unwrap().id;
        let foreign = store.insert(item("foreign words", now + 3)).unwrap().id;
        store
            .conn()
            .unwrap()
            .execute(
                "UPDATE clipboard_items SET origin_device_id = 'peer' WHERE id = ?1",
                [&foreign],
            )
            .unwrap();
        let mut app = item("app words", now + 4);
        app.app_bundle_id = Some("com.example.vault".into());
        let app = store.insert(app).unwrap().id;
        let old = store
            .insert(item("old words", now - 3 * DAY_MS))
            .unwrap()
            .id;
        let pinned_old = store
            .insert(item("pinned old words", now - 4 * DAY_MS))
            .unwrap()
            .id;
        store.set_pinned(&pinned_old, true).unwrap();

        let view = IntegrationView {
            excluded_source_apps: vec!["com.example.vault".into()],
            lookback_days: Some(1),
            ..view()
        };
        let page = store
            .query_integration_history(&HistoryQuery::default(), &view, None, 50, usize::MAX, keys)
            .unwrap();
        let mut ids = page
            .items
            .iter()
            .map(|row| row.row.id.clone())
            .collect::<Vec<_>>();
        ids.sort();
        let mut expected = vec![visible.clone(), pinned_old.clone()];
        expected.sort();
        assert_eq!(ids, expected);
        for id in [&hidden, &secret, &foreign, &app, &old] {
            assert!(!store.integration_visible(id, &view).unwrap(), "{id}");
        }
        assert!(store.integration_visible(&visible, &view).unwrap());

        let others = IntegrationView {
            include_other_devices: true,
            ..view.clone()
        };
        assert!(store.integration_visible(&foreign, &others).unwrap());
        let excluded = IntegrationView {
            excluded_devices: vec!["peer".into()],
            ..others
        };
        assert!(!store.integration_visible(&foreign, &excluded).unwrap());
        let link = store
            .insert(item("https://example.com/page", now + 5))
            .unwrap()
            .id;
        let kinds = |text, link| IntegrationView {
            kinds: IntegrationKinds {
                text,
                link,
                ..IntegrationKinds::default()
            },
            ..view.clone()
        };
        assert!(!store
            .integration_visible(&link, &kinds(true, false))
            .unwrap());
        assert!(store
            .integration_visible(&link, &kinds(false, true))
            .unwrap());
        assert!(!store
            .integration_visible(&visible, &kinds(false, true))
            .unwrap());
        let no_kinds = IntegrationView {
            kinds: IntegrationKinds::default(),
            ..view
        };
        assert!(!store.integration_visible(&visible, &no_kinds).unwrap());
    }

    #[test]
    fn a_cursor_resumes_only_the_view_it_was_issued_for() {
        let store = store();
        let keyring = crate::Keyring::from_secret(&[3; 32]);
        let (key, preview) = (keyring.item_key(), keyring.preview_key());
        let keys = (&key, &preview);
        let now = crate::now_ms();
        for index in 0..3 {
            store
                .insert(item(&format!("clip {index}"), now + index))
                .unwrap();
        }
        let query = HistoryQuery::default();
        let first = store
            .query_integration_history(&query, &view(), None, 1, usize::MAX, keys)
            .unwrap();
        let cursor = first.next.unwrap();
        let token = cursor.token();
        assert!(super::super::HistoryCursor::parse_for(&token, &query).is_err());
        let resumed = super::super::HistoryCursor::parse_scoped(&token, &query, &view()).unwrap();
        let next = store
            .query_integration_history(&query, &view(), Some(&resumed), 5, usize::MAX, keys)
            .unwrap();
        assert_eq!(next.items.len(), 2);
        let other = IntegrationView {
            include_other_devices: true,
            ..view()
        };
        assert!(super::super::HistoryCursor::parse_scoped(&token, &query, &other).is_err());
    }
}
