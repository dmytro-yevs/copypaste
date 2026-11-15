use crate::AppState;
use copypaste_core::device_name::SystemDeviceName;
use std::sync::Arc;
use tokio::sync::watch;

pub async fn run_name_refresh(state: Arc<AppState>, mut shutdown: watch::Receiver<bool>) {
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
    let mut seen = None;
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
            let system = SystemDeviceName::current();
            refresh_on_change(&mut seen, &system, |system| {
                if state.meta.refresh_system_name(system)? {
                    state
                        .meta
                        .publish_device_name(|name| state.p2p.node().set_device_name(name));
                }
                Ok::<_, copypaste_core::StoreError>(())
            })
            .map(|()| seen)
        })
        .await;
        match result {
            Ok(Ok(next)) => seen = next,
            _ => {
                seen = None;
                tracing::warn!("could not refresh the system device name");
            }
        }
    }
}

/// Runs `refresh` only when the OS name differs from the last one applied, so
/// an unchanged name costs no database write transaction.
fn refresh_on_change<E>(
    seen: &mut Option<String>,
    system: &SystemDeviceName,
    refresh: impl FnOnce(&SystemDeviceName) -> Result<(), E>,
) -> Result<(), E> {
    if seen.as_deref() == Some(system.as_str()) {
        return Ok(());
    }
    refresh(system)?;
    *seen = Some(system.as_str().to_owned());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unchanged_system_name_is_not_written_again() {
        let mut seen = None;
        let mut writes = 0;
        let mut tick = |name: &str| {
            let system = SystemDeviceName::from_sources(Some(name), None);
            refresh_on_change(&mut seen, &system, |_| {
                writes += 1;
                Ok::<_, ()>(())
            })
        };
        tick("Laptop").unwrap();
        tick("Laptop").unwrap();
        tick("Renamed").unwrap();
        tick("Renamed").unwrap();
        assert_eq!(writes, 2);
    }
}
