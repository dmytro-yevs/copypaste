//! Cached local identity and display names used by history attribution and sync.

mod error;
mod name_refresh;
pub use name_refresh::run_name_refresh;

pub use error::MetaError;

use std::collections::HashMap;
use std::sync::RwLock;

use copypaste_core::{origin_or, Store, StoreError, StoredItem};
use copypaste_ipc::DeviceClass;

/// Where one item came from, as a user reads it.
///
/// The id is the identity and the name is a label: it is whatever a peer said
/// in its hello, two devices may report the same one, and `device_name` is
/// `None` rather than a guess whenever this device has never been told one —
/// the ordinary case for an item that arrived through a cloud account from a
/// third device that was never paired directly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Origin {
    pub device_id: String,
    pub device_name: Option<String>,
    pub device_class: DeviceClass,
}

/// This device's sync identity.
#[derive(Debug)]
pub struct Meta {
    store: Store,
    device_id: String,
    device_name: RwLock<String>,
    device_class: DeviceClass,
}

impl Meta {
    pub fn open_system(store: &Store) -> Result<Self, MetaError> {
        let system = copypaste_core::device_name::SystemDeviceName::current();
        Self::from_identity(store, store.system_device_identity(&system)?)
    }

    #[cfg(test)]
    pub fn open(store: &Store, name_hint: &str) -> Result<Self, MetaError> {
        Self::from_identity(store, store.device_identity(name_hint)?)
    }

    fn from_identity(
        store: &Store,
        identity: copypaste_core::storage::DeviceIdentity,
    ) -> Result<Self, MetaError> {
        Ok(Self {
            store: store.clone(),
            device_id: identity.device_id,
            device_name: RwLock::new(identity.device_name),
            device_class: copypaste_p2p::DeviceProfile::current().device_class,
        })
    }

    pub fn refresh_system_name(
        &self,
        system: &copypaste_core::device_name::SystemDeviceName,
    ) -> Result<bool, StoreError> {
        let mut current = self.device_name.write().unwrap_or_else(|p| p.into_inner());
        let name = self.store.system_device_identity(system)?.device_name;
        let changed = *current != name;
        *current = name;
        Ok(changed)
    }

    pub fn publish_device_name(&self, publish: impl FnOnce(&str)) {
        let name = self.device_name.read().unwrap_or_else(|p| p.into_inner());
        publish(&name);
    }

    #[must_use]
    pub fn device_id(&self) -> &str {
        &self.device_id
    }

    #[must_use]
    pub fn device_name(&self) -> String {
        self.device_name
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    #[must_use]
    pub fn device_class(&self) -> DeviceClass {
        self.device_class
    }

    /// Replace the stored device name. Cosmetic; takes effect on the next hello.
    pub fn set_device_name(&self, name: &str) -> Result<(), StoreError> {
        let mut current = self.device_name.write().unwrap_or_else(|p| p.into_inner());
        *current = self.store.set_device_name(&self.device_id, name)?;
        Ok(())
    }

    pub fn record_device_name(&self, device_id: &str, name: &str) -> Result<(), MetaError> {
        Ok(self.store.record_device_name(device_id, name)?)
    }

    pub fn record_device_class(
        &self,
        device_id: &str,
        device_class: DeviceClass,
    ) -> Result<(), MetaError> {
        Ok(self.store.record_device_class(device_id, device_class)?)
    }

    /// This device, as an origin.
    #[must_use]
    pub fn here(&self) -> Origin {
        Origin {
            device_id: self.device_id.clone(),
            device_name: Some(self.device_name()),
            device_class: self.device_class,
        }
    }

    /// The origin of each row, keyed by item id, resolved to a name where one
    /// is known.
    ///
    /// One name query for a whole page rather than one per row: a page is up to
    /// 1 000 items and this is on the path of every `list` and every `search`.
    /// A row with no stored origin was captured here ([`origin_or`]).
    pub fn origins_for(&self, rows: &[StoredItem]) -> Result<HashMap<String, Origin>, MetaError> {
        let ids: Vec<String> = rows
            .iter()
            .map(|row| origin_or(&row.origin_device_id, &self.device_id).to_string())
            .collect();
        let names = self.store.device_names(&ids)?;
        let classes = self.store.device_classes(&ids)?;
        Ok(rows
            .iter()
            .zip(ids)
            .map(|(row, device_id)| {
                let device_name = names.get(&device_id).cloned();
                (
                    row.id.clone(),
                    Origin {
                        device_class: if device_id == self.device_id {
                            self.device_class
                        } else {
                            classes
                                .get(&device_id)
                                .copied()
                                .unwrap_or(DeviceClass::Unknown)
                        },
                        device_id,
                        device_name,
                    },
                )
            })
            .collect())
    }

    /// The origin of one row.
    pub fn origin_of(&self, row: &StoredItem) -> Result<Origin, MetaError> {
        let device_id = origin_or(&row.origin_device_id, &self.device_id).to_string();
        let names = self.store.device_names(std::slice::from_ref(&device_id))?;
        let classes = self
            .store
            .device_classes(std::slice::from_ref(&device_id))?;
        let device_name = names.get(&device_id).cloned();
        Ok(Origin {
            device_class: if device_id == self.device_id {
                self.device_class
            } else {
                classes
                    .get(&device_id)
                    .copied()
                    .unwrap_or(DeviceClass::Unknown)
            },
            device_id,
            device_name,
        })
    }

    pub fn oldest_version_ms(&self) -> Result<Option<i64>, MetaError> {
        Ok(self.store.oldest_version_ms()?)
    }

    pub fn state(&self, key: &str) -> Result<Option<String>, MetaError> {
        Ok(self.store.state(key)?)
    }

    pub fn set_state(&self, key: &str, value: &str) -> Result<(), MetaError> {
        Ok(self.store.set_state(key, value)?)
    }

    pub fn set_state_all(&self, entries: &[(&str, &str)]) -> Result<(), MetaError> {
        Ok(self.store.set_state_all(entries)?)
    }

    pub fn clear_state(&self, keys: &[&str]) -> Result<(), MetaError> {
        Ok(self.store.clear_state(keys)?)
    }

    pub fn state_ms(&self, key: &str) -> Result<i64, MetaError> {
        Ok(self.store.state_ms(key)?)
    }

    pub fn set_state_ms(&self, key: &str, ms: i64) -> Result<(), MetaError> {
        Ok(self.store.set_state_ms(key, ms)?)
    }
}

#[cfg(test)]
mod tests {
    use crate::testutil::{add, test_state};

    #[test]
    fn automatic_refresh_and_manual_override_reach_the_published_name() {
        use copypaste_core::device_name::SystemDeviceName;
        let (state, _dir) = test_state("Before");
        let next = SystemDeviceName::from_sources(Some("After"), None);
        assert!(state.meta.refresh_system_name(&next).unwrap());
        state
            .meta
            .publish_device_name(|name| assert_eq!(name, "After"));
        assert!(!state.meta.refresh_system_name(&next).unwrap());
        state.meta.set_device_name("Personal").unwrap();
        assert!(!state.meta.refresh_system_name(&next).unwrap());
        state
            .meta
            .publish_device_name(|name| assert_eq!(name, "Personal"));
        assert_eq!(state.store.current_device_name().unwrap(), "Personal");
    }

    #[test]
    fn an_item_captured_here_is_attributed_to_this_device_by_name() {
        let (state, _dir) = test_state("alpha");
        let id = add(&state, "mine");
        let row = state.store.version(&id).unwrap().unwrap();

        let origin = state.meta.origin_of(&row).unwrap();
        assert_eq!(origin.device_id, state.meta.device_id());
        assert_eq!(origin.device_class, state.meta.device_class());
        assert_eq!(
            origin.device_name.as_deref(),
            Some(state.meta.device_name().as_str())
        );
    }

    /// The case the type exists for: a row that came from somewhere else must
    /// not read as local, and its name arrives later than its id does.
    #[test]
    fn a_peer_item_reports_the_peer_and_its_name_once_that_name_is_known() {
        let (state, _dir) = test_state("alpha");
        let source = crate::sync::store_source(&state);
        assert!(copypaste_p2p::sync::SyncSource::apply(
            &source,
            copypaste_p2p::protocol::SyncItem {
                item_id: "theirs".into(),
                content: "from the phone".into(),
                binary_content: Vec::new(),
                payload_metadata: None,
                source_app_bundle_id: None,
                source_app_name: None,
                content_type: "text".into(),
                created_at: 1_000,
                deleted: false,
                content_hash: "hash-theirs".into(),
                origin_device_id: "device-b".into(),
                pinned: false,
                pin_order: None,
                pin_updated_at: 0,
            }
        )
        .unwrap());
        let row = state.store.version("theirs").unwrap().unwrap();

        let origin = state.meta.origin_of(&row).unwrap();
        assert_eq!(origin.device_id, "device-b");
        assert_eq!(origin.device_name, None);
        assert_eq!(origin.device_class, copypaste_ipc::DeviceClass::Unknown);
        assert_ne!(origin.device_id, state.meta.device_id());

        state
            .meta
            .record_device_name("device-b", "  Phone  ")
            .unwrap();
        state
            .meta
            .record_device_class("device-b", copypaste_ipc::DeviceClass::Phone)
            .unwrap();
        assert_eq!(
            state.meta.origin_of(&row).unwrap().device_name.as_deref(),
            Some("Phone")
        );
        assert_eq!(
            state.meta.origin_of(&row).unwrap().device_class,
            copypaste_ipc::DeviceClass::Phone
        );
    }

    /// A page is resolved in one query, and every row asked for comes back.
    #[test]
    fn every_row_in_a_page_gets_an_answer() {
        let (state, _dir) = test_state("alpha");
        add(&state, "a");
        add(&state, "b");
        let rows = state.store.list(10, 0).unwrap();

        let origins = state.meta.origins_for(&rows).unwrap();
        assert_eq!(origins.len(), rows.len());
        for row in &rows {
            assert_eq!(origins[&row.id].device_id, state.meta.device_id());
        }
        assert!(state.meta.origins_for(&[]).unwrap().is_empty());
    }

    #[test]
    fn a_device_identity_survives_a_restart_and_two_devices_differ() {
        let (a, dir) = test_state("alpha");
        let (b, _db) = test_state("beta");
        assert_ne!(a.meta.device_id(), b.meta.device_id());
        assert_eq!(a.meta.device_name(), "alpha");

        let id = a.meta.device_id().to_string();
        drop(a);
        let (restarted, _dir) =
            crate::testutil::reopen(dir, crate::cloud::Cloud::new(None), "alpha");
        assert_eq!(restarted.meta.device_id(), id);
    }
}
