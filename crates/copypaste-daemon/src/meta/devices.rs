//! Which device an item came from, and what that device calls itself.
//!
//! `sync_item_origin` has held the origin *id* since peer sync landed, because
//! the merge tie-break orders on it. What it could not answer is the question a
//! user asks — "which of my devices was this?" — because an id is a UUID.
//! `sync_device_name` is the other half: one row per device this one has ever
//! completed a session with, plus this device itself.
//!
//! # A name is cosmetic and untrusted
//!
//! It is whatever the peer said in its hello. Two devices may report the same
//! name, a device may change its name, and nothing keys off it. The id remains
//! the identity; the name is a label, and [`Origin::device_name`] is `None`
//! rather than a guess whenever this device has never been told one — which is
//! the ordinary case for an item that arrived through a cloud account from a
//! third device that was never paired directly.

use std::collections::HashMap;

use rusqlite::params;

use super::{Meta, MetaError};

/// Where one item came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Origin {
    pub device_id: String,
    pub device_name: Option<String>,
}

impl Meta {
    /// Remember what a device calls itself.
    ///
    /// Unlike [`Meta::record_origin`], this **is** an update: an origin is
    /// where an item was born and restamping it destroys the merge tie-break,
    /// whereas a device name is a label its owner is free to change. Called
    /// after every completed session, from both the initiating and the
    /// responding side.
    pub fn record_device_name(&self, device_id: &str, name: &str) -> Result<(), MetaError> {
        let name = name.trim();
        if device_id.is_empty() || name.is_empty() {
            return Ok(());
        }
        let conn = self.lock()?;
        conn.execute(
            "INSERT INTO sync_device_name (device_id, name) VALUES (?1, ?2) \
             ON CONFLICT(device_id) DO UPDATE SET name = excluded.name",
            params![device_id, name],
        )?;
        Ok(())
    }

    /// The origin of each of `item_ids`, resolved to a name where one is known.
    ///
    /// One query for a whole page rather than one per row: a page is up to
    /// 1 000 items and this is on the path of every `list` and every `search`.
    ///
    /// An item with no row in `sync_item_origin` was captured here — the table
    /// only gains a row for something that arrived from a peer or was ingested
    /// after the origin feature existed — so the fallback is this device, and
    /// it is applied here rather than left to the caller. `Meta::local_version`
    /// and `Meta::row_to_version` make the same substitution for the same
    /// reason.
    pub fn origins_for(&self, item_ids: &[String]) -> Result<HashMap<String, Origin>, MetaError> {
        let mut found = HashMap::with_capacity(item_ids.len());
        if item_ids.is_empty() {
            return Ok(found);
        }

        {
            let conn = self.lock()?;
            // One statement per batch size, as `Meta::fetch` does: the batch is
            // a page, and a page is clamped by the server.
            let placeholders = std::iter::repeat_n("?", item_ids.len())
                .collect::<Vec<_>>()
                .join(",");
            let sql = format!(
                "SELECT o.item_id, o.origin_device_id, d.name \
                   FROM sync_item_origin o \
                   LEFT JOIN sync_device_name d ON d.device_id = o.origin_device_id \
                  WHERE o.item_id IN ({placeholders})"
            );
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map(rusqlite::params_from_iter(item_ids.iter()), |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    Origin {
                        device_id: row.get(1)?,
                        device_name: row.get::<_, Option<String>>(2)?,
                    },
                ))
            })?;
            for row in rows {
                let (item_id, origin) = row?;
                found.insert(item_id, origin);
            }
        }

        for item_id in item_ids {
            found.entry(item_id.clone()).or_insert_with(|| self.here());
        }
        Ok(found)
    }

    /// The origin of one item.
    pub fn origin_of(&self, item_id: &str) -> Result<Origin, MetaError> {
        let id = item_id.to_string();
        Ok(self
            .origins_for(std::slice::from_ref(&id))?
            .remove(&id)
            .unwrap_or_else(|| self.here()))
    }

    /// This device, as an origin.
    pub fn here(&self) -> Origin {
        Origin {
            device_id: self.device_id().to_string(),
            device_name: Some(self.device_name().to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::meta::testutil::{fixture, insert};

    #[test]
    fn an_item_with_no_recorded_origin_belongs_to_this_device_by_name() {
        let f = fixture();
        insert(&f.store, "mine", "hash-mine", 1_000, false);

        let origin = f.meta.origin_of("mine").unwrap();
        assert_eq!(origin.device_id, f.meta.device_id());
        assert_eq!(origin.device_name.as_deref(), Some(f.meta.device_name()));
    }

    /// The case the whole file is for: a row that came from somewhere else must
    /// not read as local.
    #[test]
    fn a_peer_item_reports_the_peer_and_its_name_once_that_name_is_known() {
        let f = fixture();
        insert(&f.store, "theirs", "hash-theirs", 1_000, false);
        f.meta.record_origin("theirs", "device-b").unwrap();

        // Before any session with device-b: the id is known, the name is not,
        // and the item must not be attributed to this device.
        let origin = f.meta.origin_of("theirs").unwrap();
        assert_eq!(origin.device_id, "device-b");
        assert_eq!(origin.device_name, None);
        assert_ne!(origin.device_id, f.meta.device_id());

        f.meta.record_device_name("device-b", "  Phone  ").unwrap();
        let origin = f.meta.origin_of("theirs").unwrap();
        assert_eq!(origin.device_name.as_deref(), Some("Phone"));
    }

    /// A device name is a label its owner may change, unlike an origin.
    #[test]
    fn a_device_name_is_updated_where_an_origin_would_not_be() {
        let f = fixture();
        f.meta.record_device_name("device-b", "Old").unwrap();
        f.meta.record_device_name("device-b", "New").unwrap();
        insert(&f.store, "theirs", "hash-theirs", 1_000, false);
        f.meta.record_origin("theirs", "device-b").unwrap();
        assert_eq!(
            f.meta.origin_of("theirs").unwrap().device_name.as_deref(),
            Some("New")
        );
    }

    #[test]
    fn a_blank_id_or_name_is_ignored_rather_than_stored() {
        let f = fixture();
        f.meta.record_device_name("", "Nameless").unwrap();
        f.meta.record_device_name("device-b", "   ").unwrap();
        insert(&f.store, "theirs", "hash-theirs", 1_000, false);
        f.meta.record_origin("theirs", "device-b").unwrap();
        assert_eq!(f.meta.origin_of("theirs").unwrap().device_name, None);
    }

    /// A page is resolved in one query, and every id asked for comes back —
    /// including the ones with no origin row, which is most of them.
    #[test]
    fn every_requested_item_gets_an_answer() {
        let f = fixture();
        insert(&f.store, "a", "hash-a", 1_000, false);
        insert(&f.store, "b", "hash-b", 2_000, false);
        f.meta.record_origin("b", "device-b").unwrap();
        f.meta.record_device_name("device-b", "Phone").unwrap();

        let ids = vec![
            "a".to_string(),
            "b".to_string(),
            "never-existed".to_string(),
        ];
        let origins = f.meta.origins_for(&ids).unwrap();
        assert_eq!(origins.len(), 3);
        assert_eq!(origins["a"].device_id, f.meta.device_id());
        assert_eq!(origins["b"].device_name.as_deref(), Some("Phone"));
        assert_eq!(origins["never-existed"].device_id, f.meta.device_id());
    }

    #[test]
    fn an_empty_request_does_not_touch_the_database() {
        let f = fixture();
        assert!(f.meta.origins_for(&[]).unwrap().is_empty());
    }
}
