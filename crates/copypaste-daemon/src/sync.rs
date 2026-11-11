//! The runtime's peer and cloud sync sources writing received content to this
//! daemon's clipboard.

#[allow(unused_imports)]
use crate::testutil::calls::*;

mod tests {
    use copypaste_core::RemoteVersion;
    use copypaste_ipc::ConfigPatch;

    use super::*;

    #[test]
    fn receiving_sources_share_the_native_writer_and_live_device_switch() {
        let (state, _dir, writes) = crate::testutil::test_state_watching_clipboard("receiver");
        let mut events = state.subscribe_events();
        // The peer source, and the hook-free view the cloud provider receives.
        let peer = Arc::clone(&state.source);
        let cloud = state.source.without_version_hook();
        let remote = |stamp| RemoteVersion {
            item_id: "remote-text",
            content: "received text",
            binary_content: None,
            payload_metadata: None,
            content_type: "text",
            created_at: stamp,
            deleted: false,
            content_hash: None,
            origin_device_id: "sender",
            app_bundle_id: None,
            app_name: None,
        };
        assert!(peer.apply_version(&remote(100)).unwrap());
        let event = events.try_recv().unwrap();
        assert!(event.captured);
        assert_eq!(event.captured_item_id.as_deref(), Some("remote-text"));
        assert_eq!(
            writes.entries(),
            [crate::testutil::WrittenPayload::Text(
                "received text".into()
            )]
        );
        assert!(!cloud.apply_version(&remote(100)).unwrap());
        assert_eq!(writes.count(), 1);
        assert!(events.try_recv().is_err());
        state
            .settings
            .apply(&ConfigPatch {
                instant_clipboard: Some(false),
                ..Default::default()
            })
            .unwrap();
        assert!(cloud.apply_version(&remote(200)).unwrap());
        assert_eq!(writes.count(), 1);
        assert!(events.try_recv().is_err());
        assert!(state.settings.get().sync_enabled);
        state
            .settings
            .apply(&ConfigPatch {
                instant_clipboard: Some(true),
                ..Default::default()
            })
            .unwrap();
        assert!(peer.apply_version(&remote(300)).unwrap());
        assert_eq!(writes.count(), 2);
        assert!(events.try_recv().unwrap().captured);
        assert!(events.try_recv().is_err());
    }
}
