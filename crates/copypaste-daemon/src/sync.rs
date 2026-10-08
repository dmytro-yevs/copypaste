//! The one construction of the shared sync view.
//!
//! [`copypaste_core::StoreSource`] is what both transports read and write
//! through, and what the Android app builds over the same core store. All this
//! adds is the two things only a daemon has: this device's identity, cached in
//! [`crate::meta`], and the cloud upload floor that has to be pulled back
//! whenever a version lands with a stamp older than it.

use std::sync::Arc;

use copypaste_core::StoreSource;

use crate::AppState;

/// A source over this daemon's history.
pub fn store_source(state: &Arc<AppState>) -> StoreSource {
    let settings = Arc::downgrade(state);
    let clipboard = Arc::downgrade(state);
    StoreSource::with_retention_settings(
        state.store.clone(),
        Arc::clone(&state.keyring),
        state.meta.device_id().to_string(),
        state.meta.device_name(),
        move || {
            settings.upgrade().map_or_else(
                || copypaste_ipc::ConfigData {
                    sync_enabled: false,
                    ..Default::default()
                },
                |state| state.settings.get().clone(),
            )
        },
    )
    .on_clip_received(move |row| {
        let Some(clipboard) = clipboard.upgrade() else {
            return;
        };
        // Share capture authority so a local read is persisted before comparing
        // its stamp, and a settings change cannot race a native write.
        clipboard.settings.with_capture_authority(|settings, _| {
            let mut native = clipboard.clipboard();
            clipboard.instant_clipboard.apply(
                &clipboard.store,
                &clipboard.keyring,
                row,
                || settings,
                |payload| native.write_payload(&row.id, payload, &row.content_type),
            );
        });
    })
}

/// [`store_source`] for the *peer* transport, which additionally has to pull
/// the cloud upload floor back.
///
/// An item that arrived from a peer carries the sender's stamp, routinely older
/// than this device's upload cursor, so without this it would never be
/// forwarded to the account. The cloud transport deliberately does **not** take
/// this hook: a row it just downloaded is already in the account, and lowering
/// the floor onto it would re-offer it for upload every round.
///
/// Built per operation rather than held on [`AppState`]: the hook owns an
/// `Arc<AppState>`, and a source parked on the state it points back at is a
/// reference cycle that never drops.
pub fn peer_source(state: &Arc<AppState>) -> StoreSource {
    let hooked = Arc::clone(state);
    store_source(state).on_applied(move |created_at| hooked.modules.note_version(created_at))
}

#[cfg(test)]
mod tests {
    use copypaste_core::RemoteVersion;
    use copypaste_ipc::ConfigPatch;

    use super::*;

    #[test]
    fn receiving_sources_share_the_native_writer_and_live_device_switch() {
        let (state, _dir, writes) = crate::testutil::test_state_watching_clipboard("receiver");
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
        assert!(store_source(&state).apply_version(&remote(100)).unwrap());
        assert_eq!(
            writes.entries(),
            [crate::testutil::WrittenPayload::Text(
                "received text".into()
            )]
        );
        assert!(!peer_source(&state).apply_version(&remote(100)).unwrap());
        assert_eq!(writes.count(), 1);
        state
            .settings
            .apply(
                &state.meta,
                &ConfigPatch {
                    instant_clipboard: Some(false),
                    ..Default::default()
                },
            )
            .unwrap();
        assert!(peer_source(&state).apply_version(&remote(200)).unwrap());
        assert_eq!(writes.count(), 1);
        assert!(state.settings.get().sync_enabled);
        state
            .settings
            .apply(
                &state.meta,
                &ConfigPatch {
                    instant_clipboard: Some(true),
                    ..Default::default()
                },
            )
            .unwrap();
        assert!(store_source(&state).apply_version(&remote(300)).unwrap());
        assert_eq!(writes.count(), 2);
    }
}

/// Compose the same sync capabilities for production and isolated daemon tests.
pub fn install_module_services(
    state: &Arc<AppState>,
) -> Result<(), copypaste_modules::ModuleError> {
    let weak_state = Arc::downgrade(state);
    let weak_settings = Arc::downgrade(state);
    state
        .modules
        .set_sync_services(Arc::new(copypaste_modules::SyncServices::new(
            state.store.clone(),
            copypaste_sync::store::StoreView::new(
                store_source(state),
                state.meta.device_id().to_string(),
            ),
            move || {
                weak_settings
                    .upgrade()
                    .is_some_and(|state| state.is_ready() && state.settings.get().sync_enabled)
            },
            move |stamp| {
                if let Some(state) = weak_state.upgrade() {
                    state.p2p.node().cursors().note_local(stamp);
                    state.p2p.wake();
                    state.note_remote_change();
                }
            },
        )))?;
    Ok(())
}
