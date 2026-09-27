use crate::AppState;
use copypaste_core::device_name::SystemDeviceName;
use std::sync::Arc;
use tokio::sync::watch;

pub async fn run_name_refresh(state: Arc<AppState>, mut shutdown: watch::Receiver<bool>) {
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
    loop {
        tokio::select! {
            biased;
            _ = shutdown.changed() => break,
            _ = interval.tick() => {}
        }
        if *shutdown.borrow() {
            break;
        }
        let state = Arc::clone(&state);
        let result = tokio::task::spawn_blocking(move || {
            if state
                .meta
                .refresh_system_name(&SystemDeviceName::current())?
            {
                state
                    .meta
                    .publish_device_name(|name| state.p2p.node().set_device_name(name));
            }
            Ok::<_, copypaste_core::StoreError>(())
        })
        .await;
        if !matches!(result, Ok(Ok(()))) {
            tracing::warn!("could not refresh the system device name");
        }
    }
}
