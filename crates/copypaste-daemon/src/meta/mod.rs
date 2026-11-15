//! Refreshes the shared device identity from the operating system name.

mod name_refresh;
pub use name_refresh::run_name_refresh;

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

        let origin = state.meta.origin_of(&row);
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
        let source = crate::testutil::calls::store_source(&state);
        assert!(copypaste_p2p::sync::SyncSource::apply(
            source.as_ref(),
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

        let origin = state.meta.origin_of(&row);
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
            state.meta.origin_of(&row).device_name.as_deref(),
            Some("Phone")
        );
        assert_eq!(
            state.meta.origin_of(&row).device_class,
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

        let origins = state.meta.origins_for(&rows);
        assert_eq!(origins.len(), rows.len());
        for row in &rows {
            assert_eq!(origins[&row.id].device_id, state.meta.device_id());
        }
        assert!(state.meta.origins_for(&[]).is_empty());
    }

    #[test]
    fn a_device_identity_survives_a_restart_and_two_devices_differ() {
        let (a, dir) = test_state("alpha");
        let (b, _db) = test_state("beta");
        assert_ne!(a.meta.device_id(), b.meta.device_id());
        assert_eq!(a.meta.device_name(), "alpha");

        let id = a.meta.device_id().to_string();
        drop(a);
        let (restarted, _dir) = crate::testutil::reopen(dir, "alpha");
        assert_eq!(restarted.meta.device_id(), id);
    }
}
