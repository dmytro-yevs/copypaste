use std::sync::Arc;
use std::time::Duration;

use backon::{ConstantBuilder, Retryable as _};
use serde::Deserialize;
use tauri::{AppHandle, Emitter as _, Listener as _, Manager as _, Runtime};
use tokio::sync::{Mutex, Notify};

use super::menu::TrayMenu;
use crate::backend::{Backend as _, SelectedBackend};
use crate::service::push::EVENT_PUSH_STATE;

pub const EVENT_CHANGED: &str = "private-mode-changed";

const POLL_INTERVAL: Duration = Duration::from_millis(250);
const RESYNC_TIMEOUT: Duration = Duration::from_secs(30);
const STARTUP_RETRIES: usize = 119;
const BACKSTOP_RETRIES: usize = 1;

#[derive(Debug, Default)]
struct Confirmation {
    candidate: Option<bool>,
    matches: u8,
}

impl Confirmation {
    fn observe(&mut self, observed: Option<bool>) -> Option<bool> {
        let Some(observed) = observed else {
            self.candidate = None;
            self.matches = 0;
            return None;
        };

        if self.candidate == Some(observed) {
            self.matches += 1;
        } else {
            self.candidate = Some(observed);
            self.matches = 1;
        }

        (self.matches >= 2).then_some(observed)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Update {
    pub value: bool,
    pub broadcast: bool,
}

pub(super) fn toggle_update(previous: bool, applied: Option<bool>) -> Update {
    Update {
        value: applied.unwrap_or(previous),
        broadcast: true,
    }
}

fn reconciliation_update(current: bool, confirmed: bool) -> Option<Update> {
    (current != confirmed).then_some(Update {
        value: confirmed,
        broadcast: true,
    })
}

pub(super) fn spawn<R: Runtime>(app: AppHandle<R>, menu: Arc<TrayMenu<R>>, backstop: Duration) {
    let wake = Arc::new(Notify::new());
    let on_push = Arc::clone(&wake);
    app.listen(EVENT_PUSH_STATE, move |event| {
        if serde_json::from_str::<PushState>(event.payload()).is_ok_and(|state| state.live) {
            on_push.notify_one();
        }
    });

    let on_change = Arc::clone(&menu);
    app.listen(EVENT_CHANGED, move |event| {
        if let Ok(enabled) = serde_json::from_str::<bool>(event.payload()) {
            on_change.set_private_mode(enabled);
        }
    });

    tauri::async_runtime::spawn(async move {
        reconcile(&app, &menu, STARTUP_RETRIES).await;
        loop {
            tokio::select! {
                () = wake.notified() => {}
                () = tokio::time::sleep(backstop) => {}
            }
            reconcile(&app, &menu, BACKSTOP_RETRIES).await;
        }
    });
}

#[derive(Deserialize)]
struct PushState {
    live: bool,
}

#[derive(Debug)]
struct NotConfirmed;

async fn reconcile<R: Runtime>(app: &AppHandle<R>, menu: &TrayMenu<R>, retries: usize) {
    let (current, revision) = menu.private_mode_snapshot();
    let confirmation = Arc::new(Mutex::new(Confirmation::default()));
    let app = app.clone();
    let probe = || {
        let app = app.clone();
        let confirmation = Arc::clone(&confirmation);
        async move {
            let observed = app
                .state::<SelectedBackend>()
                .get_config()
                .await
                .ok()
                .map(|applied| applied.config.private_mode);
            confirmation
                .lock()
                .await
                .observe(observed)
                .ok_or(NotConfirmed)
        }
    };
    let policy = ConstantBuilder::new()
        .with_delay(POLL_INTERVAL)
        .with_max_times(retries);
    let confirmed = tokio::time::timeout(RESYNC_TIMEOUT, probe.retry(policy)).await;
    let Ok(Ok(confirmed)) = confirmed else {
        return;
    };

    let Some(update) = reconciliation_update(current, confirmed) else {
        return;
    };
    if !menu.reconcile_private_mode(update.value, revision) {
        return;
    }
    if update.broadcast {
        let _ = app.emit(EVENT_CHANGED, update.value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_mismatch_waits_for_confirmation_before_correcting() {
        let mut confirmation = Confirmation::default();

        assert_eq!(confirmation.observe(Some(true)), None);
        let confirmed = confirmation.observe(Some(true));

        assert_eq!(confirmed, Some(true));
        assert_eq!(
            reconciliation_update(false, confirmed.unwrap()),
            Some(Update {
                value: true,
                broadcast: true,
            })
        );
    }

    #[test]
    fn transient_disagreement_restarts_the_confirmation_streak() {
        let mut confirmation = Confirmation::default();

        assert_eq!(confirmation.observe(Some(true)), None);
        assert_eq!(confirmation.observe(Some(false)), None);
        assert_eq!(confirmation.observe(None), None);
        assert_eq!(confirmation.observe(Some(true)), None);
        assert_eq!(confirmation.observe(Some(true)), Some(true));
    }

    #[test]
    fn backend_failure_corrects_and_broadcasts_the_previous_state() {
        assert_eq!(
            toggle_update(true, None),
            Update {
                value: true,
                broadcast: true,
            }
        );
    }

    #[test]
    fn matching_reconciliation_does_not_rewrite_the_visible_state() {
        assert_eq!(reconciliation_update(true, true), None);
    }

    #[test]
    fn resync_schedule_is_bounded_and_uses_the_manifest_cadence() {
        assert_eq!(POLL_INTERVAL, Duration::from_millis(250));
        assert_eq!(RESYNC_TIMEOUT, Duration::from_secs(30));
        assert_eq!(STARTUP_RETRIES, 119);
        assert_eq!(BACKSTOP_RETRIES, 1);
    }
}
