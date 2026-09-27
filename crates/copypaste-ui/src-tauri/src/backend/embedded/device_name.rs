use super::open::Inner;
use std::sync::Arc;

pub(super) fn start(inner: &Arc<Inner>) {
    let weak = Arc::downgrade(inner);
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
        interval.tick().await;
        loop {
            interval.tick().await;
            let Some(inner) = weak.upgrade() else {
                break;
            };
            let update = Arc::clone(&inner);
            let result = tokio::task::spawn_blocking(move || {
                let inner = update;
                let changed = inner.state.refresh_system_name()?;
                if changed {
                    if let Some(node) = inner.node.get() {
                        inner
                            .state
                            .publish_device_name(|name| node.set_device_name(name));
                        #[cfg(target_os = "android")]
                        crate::network_discovery::reconcile(
                            inner.state.device_name(),
                            node.pairing_ids(),
                            inner.settings().lan_visibility,
                        );
                    }
                }
                Ok::<_, crate::backend::BackendError>(changed)
            })
            .await;
            if !matches!(result, Ok(Ok(_))) {
                tracing::warn!("could not refresh the system device name");
            }
        }
    });
}
