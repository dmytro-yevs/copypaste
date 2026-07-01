//! [`PairingCoordinator`] — owns the live [`PairingState`] plus the channel
//! used to deliver the user's accept/reject decision into the in-flight
//! handshake task. See the `pairing` module doc comment for the full
//! protocol/security rationale.

use std::sync::Mutex;

use tokio::sync::oneshot;

use super::state::{PairingRole, PairingState};

/// Coordinator owning the live [`PairingState`] plus the channel used to deliver
/// the user's accept/reject decision into the in-flight handshake task.
///
/// `state` is the observable machine ([`crate::pair_get_sas`] reads it).
/// `pending` holds the [`oneshot::Sender<bool>`] that the handshake's `confirm`
/// callback is awaiting — [`crate::pair_confirm_sas`] fires it. Both inner
/// fields are guarded by plain `std::sync::Mutex` because every critical section
/// is a trivial take/replace with no `.await` (a verbatim port of the daemon's
/// `PairingCoordinator`).
#[derive(Default)]
pub struct PairingCoordinator {
    state: Mutex<StateSlot>,
    pending: Mutex<Option<Pending>>,
}

/// `std::sync::Mutex` cannot hold a non-`Default` enum behind `#[derive(Default)]`
/// directly, so wrap it.
struct StateSlot(PairingState);

impl Default for StateSlot {
    fn default() -> Self {
        StateSlot(PairingState::Idle)
    }
}

/// In-flight confirmation channel for the single active pairing.
struct Pending {
    /// Fired by [`PairingCoordinator::deliver_decision`] with the user's
    /// accept(`true`)/reject(`false`) decision; awaited by the handshake's
    /// `confirm` callback.
    confirm_tx: oneshot::Sender<bool>,
}

impl PairingCoordinator {
    /// Construct an idle coordinator.
    pub fn new() -> Self {
        Self::default()
    }

    /// Snapshot the current state.
    pub fn snapshot(&self) -> PairingState {
        self.lock_state().0.clone()
    }

    /// Attempt to claim the machine for a new pairing as `role`.
    ///
    /// Returns `true` and transitions to `Initiating` when no pairing is
    /// genuinely in progress. Specifically:
    /// - From `Idle`: claims immediately.
    /// - From a stale **terminal** state (`Confirmed`/`Rejected`/`Aborted`/
    ///   `TimedOut`): auto-resets (clears any stale pending channel) and claims,
    ///   so the UI never needs to call `pair_reset()` explicitly after a terminal
    ///   outcome before retrying.
    /// - From an **active** state (`Initiating`/`AwaitingSas`): returns `false`
    ///   so the caller can reject the truly concurrent request.
    pub fn try_begin(&self, role: PairingRole) -> bool {
        let mut slot = self.lock_state();
        if slot.0.is_active() {
            return false; // genuine in-progress pairing — refuse
        }
        // Idle OR stale terminal: clear any stale pending channel and claim.
        let _ = self.lock_pending().take();
        slot.0 = PairingState::Initiating { role };
        true
    }

    /// Transition `Initiating → AwaitingSas`, storing the derived SAS and the
    /// confirmation channel. Called from the handshake's `confirm` callback.
    ///
    /// Returns the receiver the callback awaits for the user's decision.
    pub fn enter_awaiting_sas(&self, sas: String, role: PairingRole) -> oneshot::Receiver<bool> {
        let (confirm_tx, confirm_rx) = oneshot::channel();
        {
            let mut slot = self.lock_state();
            slot.0 = PairingState::AwaitingSas { sas, role };
        }
        *self.lock_pending() = Some(Pending { confirm_tx });
        confirm_rx
    }

    /// Deliver the user's accept/reject decision into the waiting handshake.
    ///
    /// Returns `true` when a decision was delivered (a pairing was awaiting),
    /// `false` when no pairing is awaiting confirmation. Does NOT itself move to
    /// a terminal state — the handshake task records the outcome via
    /// [`finish`](Self::finish) once both sides have exchanged frame 10a.
    pub fn deliver_decision(&self, accept: bool) -> bool {
        let pending = self.lock_pending().take();
        match pending {
            Some(p) => {
                // Receiver dropped (handshake task already gone) is benign.
                let _ = p.confirm_tx.send(accept);
                true
            }
            None => false,
        }
    }

    /// Abort an in-flight pairing: drop the confirmation channel (which makes the
    /// handshake `confirm` await resolve to a rejection / error) and move to
    /// `Aborted`. No-op when already terminal/idle.
    pub fn abort(&self) {
        // Dropping the sender resolves the handshake's await with `Err(Recv)`,
        // which the callback maps to a rejection so keys drop/zeroize.
        let _ = self.lock_pending().take();
        let mut slot = self.lock_state();
        if slot.0.is_active() {
            slot.0 = PairingState::Aborted;
        }
    }

    /// Record the terminal outcome reported by the handshake task and clear any
    /// pending confirmation channel.
    pub fn finish(&self, outcome: PairingState) {
        debug_assert!(outcome.is_terminal());
        let _ = self.lock_pending().take();
        self.lock_state().0 = outcome;
    }

    /// Reset to `Idle` (called once the caller has observed a terminal state, so
    /// a fresh pairing may begin).
    pub fn reset(&self) {
        let _ = self.lock_pending().take();
        self.lock_state().0 = PairingState::Idle;
    }

    fn lock_state(&self) -> std::sync::MutexGuard<'_, StateSlot> {
        // Poisoned mutex (a panic mid-mutation) is recovered: the slot holds
        // only a small enum and an optional channel — reusing it after a panic
        // is safe and keeps pairing functional.
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn lock_pending(&self) -> std::sync::MutexGuard<'_, Option<Pending>> {
        self.pending.lock().unwrap_or_else(|p| p.into_inner())
    }
}
