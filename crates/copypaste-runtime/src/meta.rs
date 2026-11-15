//! Cached local identity and display names used by history attribution and sync.

use std::collections::{BTreeSet, HashMap};
use std::sync::RwLock;

use copypaste_core::{origin_or, Store, StoreError, StoredItem};
use copypaste_ipc::DeviceClass;

/// Where one item came from, as a user reads it.
///
/// The id is the identity and the name is a label: it is whatever a peer said
/// in its hello, and `device_name` is `None` rather than a guess whenever this
/// device has never been told one.
pub use copypaste_core::history_item::ItemOrigin as Origin;

/// This device's sync identity. The name is live: a rename reaches status,
/// facets and origin labels without a restart.
#[derive(Debug)]
pub struct Meta {
    store: Store,
    device_id: String,
    device_name: RwLock<String>,
    device_class: DeviceClass,
}

impl Meta {
    /// The identity named after the operating system's device name. Android
    /// passes its name explicitly to [`Meta::open`].
    #[cfg(not(target_os = "android"))]
    pub fn open_system(store: &Store) -> Result<Self, StoreError> {
        let system = copypaste_core::device_name::SystemDeviceName::current();
        Ok(Self::from_identity(
            store,
            store.system_device_identity(&system)?,
        ))
    }

    /// The identity minted with `name_hint` when this database has none yet.
    pub fn open(store: &Store, name_hint: &str) -> Result<Self, StoreError> {
        Ok(Self::from_identity(
            store,
            store.device_identity(name_hint)?,
        ))
    }

    fn from_identity(store: &Store, identity: copypaste_core::storage::DeviceIdentity) -> Self {
        Self {
            store: store.clone(),
            device_id: identity.device_id,
            device_name: RwLock::new(identity.device_name),
            device_class: copypaste_p2p::DeviceProfile::current().device_class,
        }
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

    /// Publish the name while holding it, so a concurrent rename cannot be
    /// overtaken by an older value.
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

    pub fn record_device_name(&self, device_id: &str, name: &str) -> Result<(), StoreError> {
        self.store.record_device_name(device_id, name)
    }

    pub fn record_device_class(
        &self,
        device_id: &str,
        device_class: DeviceClass,
    ) -> Result<(), StoreError> {
        self.store.record_device_class(device_id, device_class)
    }

    /// The origin of each row, keyed by item id.
    ///
    /// Two bounded metadata reads for a whole page rather than one per row,
    /// and none for a page captured entirely here. A failed lookup costs a
    /// label, never the page.
    pub fn origins_for(&self, rows: &[StoredItem]) -> HashMap<String, Origin> {
        let device_ids: Vec<&str> = rows
            .iter()
            .map(|row| origin_or(&row.origin_device_id, &self.device_id))
            .collect();
        let remote: Vec<String> = device_ids
            .iter()
            .filter(|id| **id != self.device_id)
            .map(|id| (*id).to_owned())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let (names, classes) = if remote.is_empty() {
            (HashMap::new(), HashMap::new())
        } else {
            let names = self.store.device_names(&remote).unwrap_or_else(|error| {
                tracing::warn!(?error, "could not resolve origin device names");
                HashMap::new()
            });
            let classes = self.store.device_classes(&remote).unwrap_or_else(|error| {
                tracing::warn!(?error, "could not resolve origin device classes");
                HashMap::new()
            });
            (names, classes)
        };
        let local_name = self.device_name();
        rows.iter()
            .zip(device_ids)
            .map(|(row, device_id)| {
                let origin = if device_id == self.device_id {
                    Origin {
                        device_id: device_id.to_owned(),
                        device_name: Some(local_name.clone()),
                        device_class: self.device_class,
                    }
                } else {
                    Origin {
                        device_name: names.get(device_id).cloned(),
                        device_class: classes
                            .get(device_id)
                            .copied()
                            .unwrap_or(DeviceClass::Unknown),
                        device_id: device_id.to_owned(),
                    }
                };
                (row.id.clone(), origin)
            })
            .collect()
    }

    /// The origin of one row.
    pub fn origin_of(&self, row: &StoredItem) -> Origin {
        self.origins_for(std::slice::from_ref(row))
            .remove(&row.id)
            .expect("every row has an origin")
    }
}
