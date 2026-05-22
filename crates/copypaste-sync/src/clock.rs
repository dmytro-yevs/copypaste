/// Lamport logical clock.
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
    pub fn tick(&mut self) -> u64 {
        self.value += 1;
        self.value
    }

    /// Advance the clock upon receiving a message timestamped `received`.
    ///
    /// Sets `value = max(local, received) + 1` per the Lamport rule.
    /// Returns the new value (which should be stamped on the reply).
    pub fn observe(&mut self, received: u64) -> u64 {
        self.value = self.value.max(received) + 1;
        self.value
    }
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
}
