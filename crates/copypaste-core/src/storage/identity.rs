//! This device's sync identity, and what each device calls itself.
//!
//! # A name is cosmetic and untrusted
//!
//! It is whatever a peer said in its hello. Two devices may report the same
//! name, a device may change its name, and nothing keys off it. The id remains
//! the identity; a name is a label, and [`Store::device_names`] simply has no
//! entry whenever this device has never been told one — which is the ordinary
//! case for an item that arrived through a cloud account from a third device
//! that was never paired directly.

use std::collections::{BTreeSet, HashMap};

use rusqlite::{params, Connection, OptionalExtension};

use super::model::StoreError;
use super::store::Store;

/// Key of the persisted device id in `sync_device_state`.
const KEY_DEVICE_ID: &str = "device_id";
/// Key of the persisted device name.
const KEY_DEVICE_NAME: &str = "device_name";

/// Who this device is, on both sync transports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceIdentity {
    pub device_id: String,
    pub device_name: String,
}

impl Store {
    /// Resolve this device's identity, minting it on first run.
    ///
    /// `name_hint` is used only when no name has been stored yet: the device
    /// name is cosmetic and peer-visible, so it stays put across a hostname
    /// change rather than churning on every restart.
    ///
    /// This device is entered in the name registry like any other, so resolving
    /// an origin needs no special case for "the local one".
    pub fn device_identity(&self, name_hint: &str) -> Result<DeviceIdentity, StoreError> {
        let conn = self.conn()?;
        let device_id = load_or_set(&conn, KEY_DEVICE_ID, || uuid::Uuid::new_v4().to_string())?;
        let device_name = load_or_set(&conn, KEY_DEVICE_NAME, || {
            sanitise_name(name_hint).unwrap_or_else(|| "CopyPaste device".to_string())
        })?;
        drop(conn);

        self.record_device_name(&device_id, &device_name)?;
        Ok(DeviceIdentity {
            device_id,
            device_name,
        })
    }

    /// Replace the stored device name, returning the sanitised form actually
    /// stored. Cosmetic; takes effect on the next hello.
    pub fn set_device_name(&self, device_id: &str, name: &str) -> Result<String, StoreError> {
        let name = sanitise_name(name).ok_or(StoreError::InvalidDeviceName)?;
        {
            let conn = self.conn()?;
            conn.execute(
                "INSERT INTO sync_device_state (key, value) VALUES (?1, ?2) \
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![KEY_DEVICE_NAME, &name],
            )?;
        }
        self.record_device_name(device_id, &name)?;
        Ok(name)
    }

    /// Read the local device name without changing first-run identity state.
    pub fn current_device_name(&self) -> Result<String, StoreError> {
        let conn = self.conn()?;
        conn.query_row(
            "SELECT value FROM sync_device_state WHERE key = ?1",
            [KEY_DEVICE_NAME],
            |row| row.get(0),
        )
        .optional()?
        .filter(|name: &String| !name.is_empty())
        .ok_or(StoreError::NotFound)
    }

    /// Remember what a device calls itself.
    ///
    /// Unlike an origin — which is where an item was born, and restamping it
    /// would destroy the merge tie-break's determinism across three devices —
    /// this **is** an update: a device name is a label its owner is free to
    /// change. Called after every completed session, from both the initiating
    /// and the responding side.
    pub fn record_device_name(&self, device_id: &str, name: &str) -> Result<(), StoreError> {
        let name = name.trim();
        if device_id.is_empty() || name.is_empty() {
            return Ok(());
        }
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO sync_device_name (device_id, name) VALUES (?1, ?2) \
             ON CONFLICT(device_id) DO UPDATE SET name = excluded.name",
            params![device_id, name],
        )?;
        Ok(())
    }

    /// The name each of `device_ids` reported, for the ones this device knows.
    ///
    /// One query for a whole page rather than one per row: a page is up to
    /// 1 000 items and this is on the path of every `list` and every `search`.
    /// An id with no row is absent from the map rather than guessed at.
    ///
    /// Callers pass one id per *row*, and a page is overwhelmingly rows from one
    /// or two devices, so the ids are deduplicated before the statement is
    /// built. The result is keyed by device id and cannot see the difference; a
    /// 1 000-row page was otherwise a 1 000-placeholder `IN` list, prepared
    /// afresh because its text changes with the count.
    pub fn device_names(
        &self,
        device_ids: &[String],
    ) -> Result<HashMap<String, String>, StoreError> {
        let unique: BTreeSet<&str> = device_ids.iter().map(String::as_str).collect();
        let mut found = HashMap::with_capacity(unique.len());
        if unique.is_empty() {
            return Ok(found);
        }
        let conn = self.conn()?;
        let placeholders = std::iter::repeat_n("?", unique.len())
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "SELECT device_id, name FROM sync_device_name WHERE device_id IN ({placeholders})"
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(unique.iter()), |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        for row in rows {
            let (id, name) = row?;
            found.insert(id, name);
        }
        Ok(found)
    }
}

fn load_or_set(
    conn: &Connection,
    key: &str,
    mint: impl FnOnce() -> String,
) -> Result<String, StoreError> {
    let existing: Option<String> = conn
        .query_row(
            "SELECT value FROM sync_device_state WHERE key = ?1",
            [key],
            |r| r.get(0),
        )
        .optional()?;
    if let Some(value) = existing.filter(|v| !v.is_empty()) {
        return Ok(value);
    }
    let value = mint();
    conn.execute(
        "INSERT INTO sync_device_state (key, value) VALUES (?1, ?2) \
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, &value],
    )?;
    Ok(value)
}

/// A device name goes on the wire and into a peer's UI, so it is bounded and
/// stripped here rather than at each use.
///
/// `MAX_DEVICE_NAME_BYTES` is a *byte* bound and the name is user-supplied
/// UTF-8, so the truncation walks characters.
fn sanitise_name(raw: &str) -> Option<String> {
    let cleaned: String = raw
        .trim()
        .chars()
        .filter(|c| !c.is_control())
        .take(copypaste_p2p::protocol::MAX_DEVICE_NAME_BYTES / 4)
        .collect();
    (!cleaned.is_empty()).then_some(cleaned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::test_support::{store, KEY};
    use tempfile::TempDir;

    #[test]
    fn a_device_identity_is_minted_once_and_then_reused() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("copypaste-v2.db");

        let id = {
            let s = Store::open(&path, &KEY).unwrap();
            let first = s.device_identity("laptop").unwrap();
            assert!(!first.device_id.is_empty());
            assert_eq!(first.device_name, "laptop");
            first.device_id
        };

        // A different hint must not move the identity: peers key off it.
        let s = Store::open(&path, &KEY).unwrap();
        let second = s.device_identity("something else").unwrap();
        assert_eq!(second.device_id, id);
        assert_eq!(second.device_name, "laptop");
    }

    #[test]
    fn this_device_is_in_the_name_registry_like_any_other() {
        let s = store();
        let me = s.device_identity("laptop").unwrap();
        let names = s.device_names(std::slice::from_ref(&me.device_id)).unwrap();
        assert_eq!(names.get(&me.device_id).map(String::as_str), Some("laptop"));
    }

    #[test]
    fn a_device_name_is_updated_where_an_origin_would_not_be() {
        let s = store();
        s.record_device_name("device-b", "Old").unwrap();
        s.record_device_name("device-b", "  New  ").unwrap();
        let names = s.device_names(&["device-b".to_string()]).unwrap();
        assert_eq!(names["device-b"], "New");
    }

    #[test]
    fn a_blank_id_or_name_is_ignored_rather_than_stored() {
        let s = store();
        s.record_device_name("", "Nameless").unwrap();
        s.record_device_name("device-b", "   ").unwrap();
        assert!(s
            .device_names(&["device-b".to_string(), String::new()])
            .unwrap()
            .is_empty());
    }

    /// A page is one id per row and almost always one or two distinct devices.
    /// Repeats must collapse before the `IN` list is built, and must not change
    /// what comes back.
    #[test]
    fn repeated_ids_are_asked_for_once_and_answered_the_same() {
        let s = store();
        s.record_device_name("device-a", "Alpha").unwrap();
        s.record_device_name("device-b", "Beta").unwrap();

        let mut page: Vec<String> = Vec::new();
        for _ in 0..1_000 {
            page.push("device-a".to_string());
            page.push("device-b".to_string());
            page.push("never-seen".to_string());
        }

        let names = s.device_names(&page).unwrap();
        assert_eq!(names.len(), 2);
        assert_eq!(names["device-a"], "Alpha");
        assert_eq!(names["device-b"], "Beta");

        // Same answer as the one-of-each call it collapses to.
        let distinct = s
            .device_names(&[
                "device-a".to_string(),
                "device-b".to_string(),
                "never-seen".to_string(),
            ])
            .unwrap();
        assert_eq!(names, distinct);
    }

    #[test]
    fn an_unknown_device_has_no_name_rather_than_a_guess() {
        let s = store();
        assert!(s
            .device_names(&["never-seen".to_string()])
            .unwrap()
            .is_empty());
        assert!(s.device_names(&[]).unwrap().is_empty());
    }

    #[test]
    fn renaming_moves_both_the_identity_and_the_registry_row() {
        let s = store();
        let me = s.device_identity("laptop").unwrap();
        assert_eq!(
            s.set_device_name(&me.device_id, " desk\nmac ").unwrap(),
            "deskmac"
        );
        assert_eq!(s.device_identity("ignored").unwrap().device_name, "deskmac");
        assert_eq!(
            s.device_names(std::slice::from_ref(&me.device_id)).unwrap()[&me.device_id],
            "deskmac"
        );
    }

    #[test]
    fn a_device_name_is_bounded_and_stripped() {
        assert_eq!(sanitise_name("  laptop\n ").as_deref(), Some("laptop"));
        assert_eq!(sanitise_name(""), None);
        assert!(sanitise_name(&"x".repeat(500)).unwrap().len() <= 128);
        assert!(matches!(
            store().set_device_name("device-a", " \n\t "),
            Err(StoreError::InvalidDeviceName)
        ));
    }

    #[test]
    fn the_current_device_name_follows_a_persisted_rename() {
        let store = store();
        let identity = store.device_identity("Old name").unwrap();
        store
            .set_device_name(&identity.device_id, "New name")
            .unwrap();
        assert_eq!(store.current_device_name().unwrap(), "New name");
    }
}
