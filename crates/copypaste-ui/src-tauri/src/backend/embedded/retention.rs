use std::sync::{Arc, Weak};
use std::time::Duration;

use super::open::Inner;

const SWEEP_INTERVAL: Duration = Duration::from_secs(1);

pub(super) fn sweep(inner: &Inner) {
    let ttl = Duration::from_secs(inner.settings().sensitive_ttl_secs);
    match copypaste_core::sweep_sensitive(
        &inner.state.store,
        &inner.state.detector,
        &inner.state.keyring.item_key(),
        ttl,
        copypaste_core::now_ms(),
    ) {
        Ok(0) => {}
        Ok(removed) => inner.publish_items(false, u32::try_from(removed).unwrap_or(u32::MAX)),
        Err(error) => tracing::warn!(?error, "the sensitive-item sweep failed"),
    }
}

pub(super) fn start(inner: &Arc<Inner>) {
    let Ok(runtime) = tokio::runtime::Handle::try_current() else {
        tracing::warn!("the embedded retention timer could not start without a runtime");
        return;
    };
    let inner = Arc::downgrade(inner);
    runtime.spawn(run(inner));
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
    async fn disabled_auto_wipe_emits_no_event() {
        let (backend, _clipboard, _dir) = backend();
        backend.add(SECRET).await.unwrap();
        let mut events = backend.watch().await.unwrap();

        sweep(&backend.inner);

        assert!(events.try_recv().is_err());
    }
}
