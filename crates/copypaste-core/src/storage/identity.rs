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
use crate::device_name::{sanitise_name, SystemDeviceName};

/// Key of the persisted device id in `sync_device_state`.
const KEY_DEVICE_ID: &str = "device_id";
/// Key of the persisted device name.
const KEY_DEVICE_NAME: &str = "device_name";
const KEY_DEVICE_CLASS_PREFIX: &str = "device_class:";

/// Who this device is, on both sync transports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceIdentity {
    pub device_id: String,
    pub device_name: String,
}

impl Store {
    /// Resolve an automatic name without changing the stable device identity.
    pub fn device_identity(&self, name_hint: &str) -> Result<DeviceIdentity, StoreError> {
        self.system_device_identity(&SystemDeviceName::from_sources(Some(name_hint), None))
    }

    pub fn system_device_identity(
        &self,
        system: &SystemDeviceName,
    ) -> Result<DeviceIdentity, StoreError> {
        let mut conn = self.conn()?;
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let device_id = load_or_set(&tx, KEY_DEVICE_ID, || uuid::Uuid::new_v4().to_string())?;
        let previous = read_state(&tx, KEY_DEVICE_NAME)?;
        let source = read_state(&tx, "device_name_source")?;
        let automatic = source.as_deref() != Some("manual");
        let device_name = if automatic {
            system.as_str().to_string()
        } else {
            previous
                .clone()
                .unwrap_or_else(|| system.as_str().to_string())
        };
        if source.is_none() {
            write_state(&tx, "device_name_source", "automatic")?;
        }
        if previous.as_deref() != Some(&device_name) {
            write_state(&tx, KEY_DEVICE_NAME, &device_name)?;
        }
        write_registry(&tx, &device_id, &device_name)?;
        tx.commit()?;
        Ok(DeviceIdentity {
            device_id,
            device_name,
        })
    }

    /// A user rename permanently takes precedence over subsequent OS changes.
    pub fn set_device_name(&self, device_id: &str, name: &str) -> Result<String, StoreError> {
        let name = sanitise_name(name).ok_or(StoreError::InvalidDeviceName)?;
        let mut conn = self.conn()?;
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        write_state(&tx, KEY_DEVICE_NAME, &name)?;
        write_state(&tx, "device_name_source", "manual")?;
        write_registry(&tx, device_id, &name)?;
        tx.commit()?;
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

    /// Remember the authenticated form factor reported by a synced device.
    pub fn record_device_class(
        &self,
        device_id: &str,
        device_class: copypaste_ipc::DeviceClass,
    ) -> Result<(), StoreError> {
        if device_id.is_empty() || device_class == copypaste_ipc::DeviceClass::Unknown {
            return Ok(());
        }
        let key = format!("{KEY_DEVICE_CLASS_PREFIX}{device_id}");
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO sync_device_state (key, value) VALUES (?1, ?2) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, device_class.wire_name()],
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

    pub fn device_classes(
        &self,
        device_ids: &[String],
    ) -> Result<HashMap<String, copypaste_ipc::DeviceClass>, StoreError> {
        let unique: BTreeSet<&str> = device_ids.iter().map(String::as_str).collect();
        let mut found = HashMap::with_capacity(unique.len());
        if unique.is_empty() {
            return Ok(found);
        }
        let keys = unique
            .iter()
            .map(|id| format!("{KEY_DEVICE_CLASS_PREFIX}{id}"))
            .collect::<Vec<_>>();
        let placeholders = std::iter::repeat_n("?", keys.len())
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!("SELECT key, value FROM sync_device_state WHERE key IN ({placeholders})");
        let conn = self.conn()?;
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(keys.iter()), |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        for row in rows {
            let (key, value) = row?;
            if let Some(device_id) = key.strip_prefix(KEY_DEVICE_CLASS_PREFIX) {
                found.insert(
                    device_id.to_string(),
                    copypaste_ipc::DeviceClass::from_wire_name(&value),
                );
            }
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

fn read_state(conn: &Connection, key: &str) -> Result<Option<String>, StoreError> {
    Ok(conn
        .query_row(
            "SELECT value FROM sync_device_state WHERE key = ?1",
            [key],
            |row| row.get(0),
        )
        .optional()?)
}

fn write_state(conn: &Connection, key: &str, value: &str) -> Result<(), StoreError> {
    conn.execute("INSERT INTO sync_device_state (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value", params![key, value])?;
    Ok(())
}

fn write_registry(conn: &Connection, device_id: &str, name: &str) -> Result<(), StoreError> {
    conn.execute("INSERT INTO sync_device_name (device_id, name) VALUES (?1, ?2) ON CONFLICT(device_id) DO UPDATE SET name = excluded.name WHERE name != excluded.name", params![device_id, name])?;
    Ok(())
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
        assert_eq!(second.device_name, "something else");
    }

    #[test]
    fn automatic_names_update_both_identity_and_registry() {
        let s = store();
        let first = s.device_identity("Phone before").unwrap();
        let next = s.device_identity("Phone after").unwrap();
        assert_eq!(next.device_id, first.device_id);
        assert_eq!(next.device_name, "Phone after");
        assert_eq!(s.current_device_name().unwrap(), "Phone after");
        assert_eq!(
            s.device_names(std::slice::from_ref(&first.device_id))
                .unwrap()[&first.device_id],
            "Phone after"
        );
    }

    #[test]
    fn authenticated_device_classes_persist_by_stable_device_id() {
        let s = store();
        s.record_device_class("phone-id", copypaste_ipc::DeviceClass::Phone)
            .unwrap();
        assert_eq!(
            s.device_classes(&["phone-id".to_string()]).unwrap()["phone-id"],
            copypaste_ipc::DeviceClass::Phone
        );
        s.record_device_class("phone-id", copypaste_ipc::DeviceClass::Tablet)
            .unwrap();
        assert_eq!(
            s.device_classes(&["phone-id".to_string()]).unwrap()["phone-id"],
            copypaste_ipc::DeviceClass::Tablet
        );
    }

    #[test]
    fn manual_name_survives_system_changes_and_reopening() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("identity.db");
        let s = Store::open(&path, &KEY).unwrap();
        let first = s.device_identity("System name").unwrap();
        // Explicitly choosing even the same name makes it a manual override.
        s.set_device_name(&first.device_id, "System name").unwrap();
        assert_eq!(s.device_identity("Renamed OS").unwrap(), first);
        drop(s);
        let reopened = Store::open(&path, &KEY).unwrap();
        assert_eq!(reopened.device_identity("Another OS name").unwrap(), first);
    }

    #[test]
    fn a_rejected_rename_does_not_disable_automatic_names() {
        let s = store();
        let me = s.device_identity("Before").unwrap();
        assert!(s.set_device_name(&me.device_id, " ").is_err());
        assert_eq!(s.device_identity("After").unwrap().device_name, "After");
    }

    #[test]
    fn naming_updates_roll_back_together_on_registry_failure() {
        let s = store();
        let me = s.device_identity("Before").unwrap();
        s.conn().unwrap().execute_batch("CREATE TRIGGER refuse_name BEFORE UPDATE ON sync_device_name BEGIN SELECT RAISE(ABORT, 'refuse'); END;").unwrap();
        assert!(s.set_device_name(&me.device_id, "Manual").is_err());
        assert_eq!(s.current_device_name().unwrap(), "Before");
        assert_eq!(
            s.state("device_name_source").unwrap().as_deref(),
            Some("automatic")
        );
        assert!(s.device_identity("After").is_err());
        assert_eq!(s.current_device_name().unwrap(), "Before");
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
