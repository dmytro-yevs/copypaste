use std::sync::Weak;

use tokio_util::sync::CancellationToken;

use super::super::open::Inner;

pub(super) async fn until_cancelled<T>(
    cancel: &CancellationToken,
    future: impl std::future::Future<Output = T>,
) -> Option<T> {
    tokio::select! {
        biased;
        _ = cancel.cancelled() => None,
        value = future => Some(value),
    }
}

pub(super) async fn poll(inner: Weak<Inner>, shutdown: CancellationToken) {
    loop {
        let Some(owner) = inner.upgrade() else {
            break;
        };
        let wait = owner
            .cloud
            .driver()
            .map_or(std::time::Duration::from_secs(60), |driver| {
                driver.poll_interval()
            });
        tokio::select! {
            biased;
            _ = shutdown.cancelled() => break,
            _ = owner.cloud.wake.notified() => {},
            _ = tokio::time::sleep(wait) => {},
        }
        drop(owner);
        let Some(owner) = inner.upgrade() else {
            break;
        };
        if owner.settings().sync_enabled {
            // Skips rather than queues: a tick that collides with a round in
            // flight would push the same window and race it to commit the
            // upload floor, and the loop ticks again anyway.
            let _ = owner.cloud.poll_round(&owner).await;
        }
    }
}
