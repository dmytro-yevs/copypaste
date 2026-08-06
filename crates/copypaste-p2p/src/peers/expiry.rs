use super::file::State;
use super::PAIRING_CODE_TTL;

pub(super) fn ttl_ms() -> i64 {
    i64::try_from(PAIRING_CODE_TTL.as_millis()).unwrap_or(i64::MAX)
}

pub(super) fn is_expired(state: &State, pairing_id: &str, now_ms: i64) -> bool {
    state
        .pending
        .get(pairing_id)
        .is_some_and(|deadline| *deadline <= now_ms)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pending(pairing_id: &str, deadline: i64) -> State {
        let mut state = State::default();
        state.pending.insert(pairing_id.to_string(), deadline);
        state
    }

    #[test]
    fn the_deadline_is_the_last_instant_the_code_still_works() {
        let state = pending("waiting", 1_000);
        assert!(!is_expired(&state, "waiting", 999));
        assert!(is_expired(&state, "waiting", 1_000));
        assert!(is_expired(&state, "waiting", 1_001));
    }

    #[test]
    fn a_pairing_with_no_deadline_never_ages_out() {
        assert!(!is_expired(&State::default(), "established", i64::MAX));
    }

    #[test]
    fn the_ttl_is_the_configured_window_in_milliseconds() {
        assert_eq!(ttl_ms(), PAIRING_CODE_TTL.as_millis() as i64);
    }
}
