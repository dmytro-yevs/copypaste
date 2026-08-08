//! This device's sync identity, and where an item came from.
//!
//! Everything else this module used to be is now `copypaste_core::storage`: the
//! summaries a session advertises, the last-write-wins write, the cursor table
//! and the device-name registry are all `Store` methods, and
//! [`copypaste_core::StoreSource`] is the one `SyncSource` both platforms
//! construct. What is left here is the daemon's cache of the identity — read
//! once at start, because every hello and every merge key 4 needs it — and
//! [`Origin`], which is attribution for a *user*, not for the merge.
//!
//! Note the shape that went with it: this module used to open the SQLCipher
//! file on a second connection to read columns `StoredItem` did not carry. Two
//! writers on one file, and two answers to "what is in this device's history".

mod error;

pub use error::MetaError;

use std::collections::HashMap;

use copypaste_core::{origin_or, Store, StoredItem};

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
}

/// This device's sync identity.
#[derive(Debug)]
pub struct Meta {
    store: Store,
    device_id: String,
    device_name: String,
}

impl Meta {
    /// Resolve this device's identity from the history database, minting it on
    /// first run.
    ///
    /// `name_hint` is used only when no name has been stored yet: the device
    /// name is cosmetic and peer-visible, so it stays put across a hostname
    /// change rather than churning on every restart.
    pub fn open(store: &Store, name_hint: &str) -> Result<Self, MetaError> {
        let identity = store.device_identity(name_hint)?;
        Ok(Self {
            store: store.clone(),
            device_id: identity.device_id,
            device_name: identity.device_name,
        })
    }

    #[must_use]
    pub fn device_id(&self) -> &str {
        &self.device_id
    }

    #[must_use]
    pub fn device_name(&self) -> &str {
        &self.device_name
    }

    /// Replace the stored device name. Cosmetic; takes effect on the next hello.
    pub fn set_device_name(&mut self, name: &str) -> Result<(), MetaError> {
        self.device_name = self.store.set_device_name(&self.device_id, name)?;
        Ok(())
    }

    pub fn record_device_name(&self, device_id: &str, name: &str) -> Result<(), MetaError> {
        Ok(self.store.record_device_name(device_id, name)?)
    }

    /// This device, as an origin.
    #[must_use]
    pub fn here(&self) -> Origin {
        Origin {
            device_id: self.device_id.clone(),
            device_name: Some(self.device_name.clone()),
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
        Ok(rows
            .iter()
            .zip(ids)
            .map(|(row, device_id)| {
                let device_name = names.get(&device_id).cloned();
                (
                    row.id.clone(),
                    Origin {
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
        let device_name = names.get(&device_id).cloned();
        Ok(Origin {
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
    fn an_item_captured_here_is_attributed_to_this_device_by_name() {
        let (state, _dir) = test_state("alpha");
        let id = add(&state, "mine");
        let row = state.store.version(&id).unwrap().unwrap();

        let origin = state.meta.origin_of(&row).unwrap();
        assert_eq!(origin.device_id, state.meta.device_id());
        assert_eq!(
            origin.device_name.as_deref(),
            Some(state.meta.device_name())
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
        assert_ne!(origin.device_id, state.meta.device_id());

        state
            .meta
            .record_device_name("device-b", "  Phone  ")
            .unwrap();
        assert_eq!(
            state.meta.origin_of(&row).unwrap().device_name.as_deref(),
            Some("Phone")
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
