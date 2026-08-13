//! One sync round at a time, per transport.
//!
//! A poll loop and a user asking for a round now were never kept apart, so the
//! two ran against one history: two pushes of the same window, two pulls behind
//! one cursor, and two completions racing to commit the upload floor — which
//! each compares against the floor *its own* scan started from.
//!
//! The two callers want different things from a busy gate, which is the whole
//! API here. A poll loop **skips** ([`RoundGate::try_enter`]): it will tick
//! again. An explicit request **queues** ([`RoundGate::enter`]), because the
//! user asked after the running round read its scan.

use std::sync::Arc;

use tokio::sync::{Mutex, OwnedMutexGuard};

/// The permit to run one round. Dropping it lets the next caller in.
pub type RoundGuard = OwnedMutexGuard<()>;

/// Serialises the rounds of one transport.
#[derive(Clone, Default)]
pub struct RoundGate {
    lock: Arc<Mutex<()>>,
}

impl RoundGate {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Take the permit, or `None` when a round is already running.
    #[must_use]
    pub fn try_enter(&self) -> Option<RoundGuard> {
        Arc::clone(&self.lock).try_lock_owned().ok()
    }

    /// Take the permit, waiting for any running round to finish first.
    pub async fn enter(&self) -> RoundGuard {
        Arc::clone(&self.lock).lock_owned().await
    }

    #[must_use]
    pub fn is_running(&self) -> bool {
        self.lock.try_lock().is_err()
    }
}

impl std::fmt::Debug for RoundGate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RoundGate")
            .field("running", &self.is_running())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_second_round_cannot_start_while_one_is_running() {
        let gate = RoundGate::new();
        let held = gate.enter().await;
        assert!(gate.try_enter().is_none(), "two rounds ran at once");
        assert!(gate.is_running());
        drop(held);
        assert!(gate.try_enter().is_some());
    }

    /// The difference between the two callers: the poll loop gives up and the
    /// explicit request waits, so a user-driven round always runs against the
    /// history as it stands after their request.
    #[tokio::test]
    async fn an_explicit_request_queues_where_a_poll_skips() {
        let gate = RoundGate::new();
        let held = gate.enter().await;
        let queued = tokio::spawn({
            let gate = gate.clone();
            async move {
                let _guard = gate.enter().await;
                true
            }
        });
        assert!(gate.try_enter().is_none());
        drop(held);
        assert!(queued.await.unwrap());
    }
}
