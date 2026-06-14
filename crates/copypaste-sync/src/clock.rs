/// Lamport logical clock.
///
/// **NOT on the daemon production path** (CopyPaste-j6r/ojhe): the live daemon
/// does not advance a `LamportClock`. It stamps `lamport_ts` via
/// `copypaste_core::next_lamport_ts` (`max(prev + 1, now_ms)`) and resolves
/// conflicts through [`crate::merge::resolve`]. This type is retained for the
/// session protocol + its tests; see the crate-root docs before reviving it.
///
/// Rules (Lamport 1978):
///  * On every local event: `tick()` — increments by 1.
///  * On receiving a message carrying clock value `r`: `observe(r)` — sets
///    clock to `max(local, r) + 1`.
///
/// The clock is *not* thread-safe by itself; callers that need concurrent
/// access must wrap it in a `Mutex` / `RwLock`.
#[derive(Debug, Default, Clone)]
pub struct LamportClock {
    value: u64,
}

impl LamportClock {
    /// Create a new clock starting at 0.
    pub fn new() -> Self {
        Self { value: 0 }
    }

    /// Create a clock at the given initial value (used when restoring
    /// persisted state across sessions).
    pub fn from_value(value: u64) -> Self {
        Self { value }
    }

    /// Return the current clock value without advancing it.
    pub fn get(&self) -> u64 {
        self.value
    }

    /// Advance the clock for a local event and return the new value.
    ///
    /// Uses `saturating_add` to prevent overflow panic at `u64::MAX`.
    /// At saturation, the clock remains at `u64::MAX` and a warning is logged once.
    pub fn tick(&mut self) -> u64 {
        if self.value == u64::MAX {
            warn_saturated();
            return self.value;
        }
        self.value = self.value.saturating_add(1);
        self.value
    }

    /// Advance the clock upon receiving a message timestamped `received`.
    ///
    /// Sets `value = max(local, received).saturating_add(1)` per the Lamport rule,
    /// with saturation to avoid panic at `u64::MAX`.
    /// Returns the new value (which should be stamped on the reply).
    pub fn observe(&mut self, received: u64) -> u64 {
        let base = self.value.max(received);
        if base == u64::MAX {
            warn_saturated();
            self.value = u64::MAX;
            return self.value;
        }
        self.value = base.saturating_add(1);
        self.value
    }
}

/// Log a single warning when the Lamport clock saturates at `u64::MAX`.
///
/// Uses `OnceLock` to ensure the warning is only emitted once per process
/// (preventing log spam if the clock stays at saturation).
fn warn_saturated() {
    use std::sync::OnceLock;
    static WARNED: OnceLock<()> = OnceLock::new();
    WARNED.get_or_init(|| {
        tracing::warn!(
            "lamport clock saturated at u64::MAX — subsequent ticks/observes are no-ops"
        );
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_clock_starts_at_zero() {
        let c = LamportClock::new();
        assert_eq!(c.get(), 0);
    }

    #[test]
    fn tick_increments_by_one() {
        let mut c = LamportClock::new();
        assert_eq!(c.tick(), 1);
        assert_eq!(c.tick(), 2);
        assert_eq!(c.get(), 2);
    }

    #[test]
    fn observe_uses_max_plus_one_when_received_larger() {
        let mut c = LamportClock::new();
        c.tick(); // value = 1
        let new_val = c.observe(10); // max(1, 10) + 1 = 11
        assert_eq!(new_val, 11);
        assert_eq!(c.get(), 11);
    }

    #[test]
    fn observe_increments_local_when_local_larger() {
        let mut c = LamportClock::from_value(20);
        let new_val = c.observe(5); // max(20, 5) + 1 = 21
        assert_eq!(new_val, 21);
    }

    #[test]
    fn observe_equal_clocks_increments() {
        let mut c = LamportClock::from_value(5);
        let new_val = c.observe(5); // max(5, 5) + 1 = 6
        assert_eq!(new_val, 6);
    }

    #[test]
    fn from_value_restores_state() {
        let c = LamportClock::from_value(42);
        assert_eq!(c.get(), 42);
    }

    // --- Saturation tests (edge-cases CRITICAL #3) ---

    #[test]
    fn tick_saturates_at_u64_max() {
        let mut c = LamportClock::from_value(u64::MAX);
        // Should NOT panic on overflow.
        let v = c.tick();
        assert_eq!(v, u64::MAX, "tick at u64::MAX must stay at u64::MAX");
        assert_eq!(c.get(), u64::MAX);
        // Repeated ticks remain at MAX.
        let v2 = c.tick();
        assert_eq!(v2, u64::MAX);
    }

    #[test]
    fn tick_one_below_max_saturates() {
        let mut c = LamportClock::from_value(u64::MAX - 1);
        // First tick reaches exactly MAX.
        assert_eq!(c.tick(), u64::MAX);
        // Next tick should saturate, not panic.
        assert_eq!(c.tick(), u64::MAX);
    }

    #[test]
    fn observe_saturates_at_u64_max() {
        let mut c = LamportClock::from_value(u64::MAX);
        // observe with local already at MAX — must not panic.
        let v = c.observe(u64::MAX);
        assert_eq!(v, u64::MAX, "observe at u64::MAX must stay at u64::MAX");
        assert_eq!(c.get(), u64::MAX);
    }

    #[test]
    fn observe_received_max_saturates() {
        let mut c = LamportClock::from_value(5);
        // observe a peer at MAX — must not panic.
        let v = c.observe(u64::MAX);
        assert_eq!(v, u64::MAX);
        assert_eq!(c.get(), u64::MAX);
    }

    #[test]
    fn observe_just_below_max_saturates() {
        let mut c = LamportClock::from_value(u64::MAX - 1);
        // max(u64::MAX-1, u64::MAX-1) + 1 = u64::MAX
        let v = c.observe(u64::MAX - 1);
        assert_eq!(v, u64::MAX);
        // Next observe should saturate.
        assert_eq!(c.observe(0), u64::MAX);
    }
}
