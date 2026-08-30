use std::sync::{Arc, Weak};
use std::time::Duration;

use super::open::Inner;

const SWEEP_INTERVAL: Duration = Duration::from_secs(1);

pub(super) fn sweep(inner: &Inner) {
    let (mutation_started, removed) = inner.state.store.with_retention(|| {
        let ttl = Duration::from_secs(inner.settings().sensitive_ttl_secs);
        // Same bound as daemon wipe: floor must not land above tombstone stamps.
        let mutation_started = copypaste_core::now_ms();
        let removed = copypaste_core::sweep_sensitive(
            &inner.state.store,
            &inner.state.detector,
            &inner.state.keyring.item_key(),
            ttl,
            mutation_started,
        );
        (mutation_started, removed)
    });
    match removed {
        Ok(0) => {}
        Ok(removed) => {
            inner.note_version_written(mutation_started);
            inner.note_local_version(mutation_started);
            inner.publish_items(false, u32::try_from(removed).unwrap_or(u32::MAX));
        }
        Err(error) => tracing::warn!(?error, "the sensitive-item sweep failed"),
    }
}

pub(super) fn start(inner: &Arc<Inner>) {
    let inner = Arc::downgrade(inner);
    tauri::async_runtime::spawn(run(inner));
}

async fn run(inner: Weak<Inner>) {
    let mut interval = tokio::time::interval(SWEEP_INTERVAL);
    interval.tick().await;
    loop {
        interval.tick().await;
        let Some(inner) = inner.upgrade() else {
            break;
        };
        sweep(&inner);
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::backend;
    use super::*;
    use crate::backend::Backend;
    use copypaste_ipc::EventKind;

    const SECRET: &str = "AKIAIOSFODNN7EXAMPLE";

    #[test]
    fn a_no_runtime_open_starts_periodic_sensitive_retention() {
        let (backend, _clipboard, _dir) = backend();
        tauri::async_runtime::block_on(backend.set_config(copypaste_ipc::ConfigPatch {
            sensitive_ttl_secs: Some(1),
            ..Default::default()
        }))
        .unwrap();

        let mut events = backend.inner.events.subscribe();
        let created_at = copypaste_core::now_ms().saturating_sub(2_000);
        let item = copypaste_core::ingest_into(
            &backend.inner.state.store,
            &backend.inner.state.detector,
            &backend.inner.state.keyring,
            SECRET,
            copypaste_ipc::content_type::TEXT,
            created_at,
            &backend.inner.settings(),
        )
        .unwrap()
        .into_item();
        assert!(item.is_sensitive);
        assert!(backend.inner.state.store.get(&item.id).unwrap().is_some());

        let event = tauri::async_runtime::block_on(async {
            tokio::time::timeout(Duration::from_secs(3), events.recv())
                .await
                .expect("the periodic sweep did not run")
                .unwrap()
        });
        assert_eq!(event.event, EventKind::Items);
        assert_eq!(event.item_count, 0);
        assert!(!event.captured);
        assert_eq!(event.swept, 1);
        assert!(backend.inner.state.store.get(&item.id).unwrap().is_none());
    }

    #[tokio::test]
    async fn an_expired_secret_emits_a_non_capture_sweep_event() {
        let (backend, _clipboard, _dir) = backend();
        backend
            .set_config(copypaste_ipc::ConfigPatch {
                sensitive_ttl_secs: Some(1),
                ..Default::default()
            })
            .await
            .unwrap();
        let item = backend.add(SECRET).await.unwrap();
        let mut events = backend.watch().await.unwrap();

        std::thread::sleep(Duration::from_millis(1_100));
        sweep(&backend.inner);

        let event = events.recv().await.unwrap();
        assert_eq!(event.event, EventKind::Items);
        assert_eq!(event.item_count, 0);
        assert!(!event.captured);
        assert_eq!(event.swept, 1);
        assert!(backend.get(&item.id).await.is_err());
    }

    #[tokio::test]
    async fn a_sensitive_sweep_pulls_the_embedded_cloud_upload_floor_back() {
        use super::super::cloud::KEY_UPLOAD_FLOOR;

        let (backend, _clipboard, _dir) = backend();
        backend
            .set_config(copypaste_ipc::ConfigPatch {
                sensitive_ttl_secs: Some(1),
                ..Default::default()
            })
            .await
            .unwrap();
        let created_at = copypaste_core::now_ms().saturating_sub(2_000);
        let item = copypaste_core::ingest_into(
            &backend.inner.state.store,
            &backend.inner.state.detector,
            &backend.inner.state.keyring,
            SECRET,
            copypaste_ipc::content_type::TEXT,
            created_at,
            &backend.inner.settings(),
        )
        .unwrap()
        .into_item();
        let ahead = copypaste_core::now_ms().saturating_add(60_000);
        backend
            .inner
            .state
            .store
            .set_state_ms(KEY_UPLOAD_FLOOR, ahead)
            .unwrap();

        sweep(&backend.inner);

        let floor = backend
            .inner
            .state
            .store
            .state_ms(KEY_UPLOAD_FLOOR)
            .unwrap();
        assert!(floor < ahead, "the sweep left the upload floor ahead");
        assert!(
            backend
                .inner
                .state
                .store
                .versions_since(floor, 100)
                .unwrap()
                .iter()
                .any(|row| row.deleted && row.id == item.id),
            "the wipe tombstone was not offered"
        );
    }

    #[tokio::test]
    async fn disabled_auto_wipe_emits_no_event() {
        let (backend, _clipboard, _dir) = backend();
        backend
            .set_config(copypaste_ipc::ConfigPatch {
                sensitive_ttl_secs: Some(0),
                ..Default::default()
            })
            .await
            .unwrap();
        let item = copypaste_core::ingest_into(
            &backend.inner.state.store,
            &backend.inner.state.detector,
            &backend.inner.state.keyring,
            SECRET,
            copypaste_ipc::content_type::TEXT,
            copypaste_core::now_ms().saturating_sub(120_000),
            &backend.inner.settings(),
        )
        .unwrap()
        .into_item();
        let mut events = backend.watch().await.unwrap();

        sweep(&backend.inner);

        assert!(events.try_recv().is_err());
        assert!(backend.inner.state.store.get(&item.id).unwrap().is_some());
    }
}
