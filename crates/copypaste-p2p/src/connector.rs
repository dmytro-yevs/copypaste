//! Per-peer dial scheduling helpers for a proactive P2P connector loop.
//!
//! The daemon runs a connector that periodically dials paired-but-not-connected
//! peers. Two failure modes motivated this module, both extracted here as pure,
//! deterministic state machines so they can be unit-tested without a running
//! daemon, tokio runtime, or real sockets:
//!
//! * **Stale-sink dedup (M1).** The connector keys live connections by cert
//!   fingerprint and skips a peer it already has a sink for. But a *closed*
//!   sink — the peer's connection task has exited yet the cleanup pass that
//!   evicts it from the map has not run — must NOT block a legitimate
//!   reconnect. [`should_dial_peer`] treats a closed sink as absent so the
//!   connector force-replaces a dead connection instead of waiting for the
//!   reaper.
//!
//! * **Dwell-gated backoff reset (M3).** Resetting the per-peer backoff on ANY
//!   successful TCP/TLS connect lets a *flapping* peer (connects, immediately
//!   drops, repeat) churn at the connector tick rate, because each brief
//!   success wipes the accumulated backoff. [`DialBackoff`] instead only resets
//!   the backoff once a connection has stayed healthy for at least
//!   [`MIN_HEALTHY_DWELL`]; a connection that drops before that dwell elapses
//!   keeps advancing the backoff schedule.

use std::time::{Duration, Instant};

/// Per-peer backoff schedule (seconds) applied after a failed/short-lived dial.
///
/// The dialer advances one step on each consecutive failure (or flap) and only
/// resets to the first step once a connection has been healthy for
/// [`MIN_HEALTHY_DWELL`], so an offline or flapping peer is retried with
/// increasing spacing (2s → 5s → 10s → 30s → 60s, capped) rather than hammered
/// every tick. The gentle early ramp redials within a couple of seconds after a
/// transient blip while still backing off to 60s for a genuinely-offline peer.
pub const BACKOFF_STEPS: [u64; 5] = [2, 5, 10, 30, 60];

/// Minimum time a connection must remain healthy before its peer's backoff is
/// reset to the first step. Sized comfortably above the connector tick so a
/// peer that connects and immediately drops (a flap) never resets its backoff,
/// while a genuinely stable link resets within a couple of ticks.
pub const MIN_HEALTHY_DWELL: Duration = Duration::from_secs(10);

/// Decide whether the connector should dial `peer_fingerprint`.
///
/// `existing_sink_is_healthy` is the result of looking the peer up in the live
/// sink map and asking whether that sink's channel is still open
/// (`!sender.is_closed()`):
///
/// * `None` — no sink registered for this peer → dial.
/// * `Some(true)` — a healthy live connection exists → skip (don't churn it).
/// * `Some(false)` — a *stale* sink (channel closed, peer task already exited
///   but not yet reaped) → dial; the caller force-replaces it.
///
/// This is the M1 fix: the accept path already refused to overwrite a healthy
/// sink yet replaced a closed one, but the connector only checked presence
/// (`contains_key`), so a stale-but-unreaped sink permanently blocked
/// reconnection. Gating on health here closes that gap.
#[must_use]
pub fn should_dial_peer(existing_sink_is_healthy: Option<bool>) -> bool {
    match existing_sink_is_healthy {
        None => true,        // nothing connected → dial
        Some(false) => true, // stale/dead sink → force-replace via redial
        Some(true) => false, // healthy live connection → leave it alone
    }
}

/// Per-peer dial state tracked by the connector loop across ticks.
///
/// Encapsulates the exponential backoff schedule plus the dwell tracking needed
/// to gate backoff resets (M3). All transitions take an explicit `now` so the
/// state machine is fully deterministic under test.
#[derive(Debug, Clone, Default)]
pub struct DialBackoff {
    /// Index into [`BACKOFF_STEPS`] used for the *next* failure delay.
    backoff_idx: usize,
    /// Earliest instant we may dial this peer again. `None` = dial now.
    next_attempt: Option<Instant>,
    /// When the current connection was established, if connected. `None` while
    /// disconnected. Used to measure healthy dwell for the backoff-reset gate.
    connected_since: Option<Instant>,
}

impl DialBackoff {
    /// Fresh state for a newly-seen peer: dial immediately, first backoff step.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether the backoff window permits dialing at `now`.
    #[must_use]
    pub fn may_dial(&self, now: Instant) -> bool {
        match self.next_attempt {
            Some(next) => now >= next,
            None => true,
        }
    }

    /// Record that a dial succeeded and a connection is now live at `now`.
    ///
    /// This intentionally does **not** reset the backoff: a brief success from a
    /// flapping peer must not wipe accumulated backoff. The reset only happens
    /// later, via [`maybe_reset_after_dwell`](Self::maybe_reset_after_dwell),
    /// once the connection has proven healthy for [`MIN_HEALTHY_DWELL`]. We do
    /// clear `next_attempt` because, while connected, there is no pending retry.
    pub fn record_connected(&mut self, now: Instant) {
        self.connected_since = Some(now);
        self.next_attempt = None;
    }

    /// If a connection has been continuously healthy since
    /// [`record_connected`](Self::record_connected) for at least
    /// [`MIN_HEALTHY_DWELL`], reset the backoff schedule to the first step.
    ///
    /// Returns `true` if a reset happened. Idempotent: once reset, calling again
    /// is a no-op (the index is already zero). The caller invokes this each tick
    /// while it observes the peer still has a healthy live sink.
    pub fn maybe_reset_after_dwell(&mut self, now: Instant) -> bool {
        let Some(since) = self.connected_since else {
            return false;
        };
        if now.duration_since(since) >= MIN_HEALTHY_DWELL && self.backoff_idx != 0 {
            self.backoff_idx = 0;
            return true;
        }
        false
    }

    /// Record that the live connection dropped. Preserves the backoff index so a
    /// flap (drop before dwell elapsed) keeps escalating on the next failure.
    pub fn record_disconnected(&mut self) {
        self.connected_since = None;
    }

    /// Record a failed dial at `now`: schedule the next attempt after the
    /// current backoff step (plus ±25% jitter to prevent thundering-herd
    /// reconnect storms — CopyPaste-rh27), then advance the step (capped at
    /// the last entry). Returns the nominal backoff delay in seconds (before
    /// jitter); the actual sleep duration includes the jitter offset.
    ///
    /// # Jitter
    ///
    /// Multiple peers that fail concurrently (e.g. a network blip drops every
    /// P2P connection simultaneously) would otherwise ALL schedule their next
    /// dial at `now + step`, causing a synchronised reconnect storm. Adding
    /// ±25% randomised jitter desynchronises them so retries are spread across
    /// `[0.75×step, 1.25×step]`.
    pub fn record_failure(&mut self, now: Instant) -> u64 {
        self.record_failure_with_rng(now, &mut rand::thread_rng())
    }

    /// Test-seam variant of [`record_failure`] that accepts an explicit RNG so
    /// jitter behaviour can be verified deterministically in unit tests.
    pub(crate) fn record_failure_with_rng<R: rand::Rng>(
        &mut self,
        now: Instant,
        rng: &mut R,
    ) -> u64 {
        self.connected_since = None;
        let step = BACKOFF_STEPS[self.backoff_idx.min(BACKOFF_STEPS.len() - 1)];
        // Jitter: sample from [0, step/2) ms then subtract step/4 ms to centre
        // the window at zero, producing a signed offset in [−step/4, +step/4) s.
        // We work in milliseconds to keep integer arithmetic; step is in seconds.
        let step_ms = step * 1_000;
        let half_window_ms = step_ms / 4; // 25% of step
                                          // gen_range(0..2*half_window) − half_window → [−half_window, +half_window)
        let raw: i64 = rng.gen_range(0..=(2 * half_window_ms as i64));
        let jitter_ms: i64 = raw - half_window_ms as i64;
        let jittered_ms = (step_ms as i64 + jitter_ms).max(0) as u64;
        self.next_attempt = Some(now + Duration::from_millis(jittered_ms));
        self.backoff_idx = (self.backoff_idx + 1).min(BACKOFF_STEPS.len() - 1);
        step
    }

    /// Current backoff index — exposed for diagnostics and tests.
    #[must_use]
    pub fn backoff_idx(&self) -> usize {
        self.backoff_idx
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    // ── M1: stale-sink dedup ────────────────────────────────────────────────

    #[test]
    fn should_dial_when_no_sink_present() {
        assert!(should_dial_peer(None));
    }

    #[test]
    fn should_not_dial_when_sink_is_healthy() {
        assert!(!should_dial_peer(Some(true)));
    }

    #[test]
    fn should_dial_to_replace_stale_sink() {
        // M1: a closed-but-unreaped sink must NOT block reconnection.
        assert!(
            should_dial_peer(Some(false)),
            "a stale (closed) sink must be force-replaced by redialing"
        );
    }

    #[test]
    fn backoff_schedule_has_gentle_early_ramp() {
        // A transient blip must redial quickly (2s) and only ramp toward the
        // 60s cap if the peer is genuinely offline, so a brief failure does not
        // cost up to a minute before the next dial.
        assert_eq!(BACKOFF_STEPS, [2, 5, 10, 30, 60]);
    }

    // ── CopyPaste-rh27: jitter in per-peer backoff ──────────────────────────

    /// rh27: jitter must spread retry times across [0.75×step, 1.25×step].
    ///
    /// Uses the seeded `record_failure_with_rng` test-seam with a fixed-seed
    /// SmallRng so the result is deterministic while still exercising the
    /// jitter arithmetic (a seed of 0 produces jitter ≠ 0 for the 2s step).
    #[test]
    fn rh27_backoff_jitter_stays_within_25_percent_of_step() {
        let t0 = Instant::now();

        // Run 100 failures with a seeded RNG; all must be in [0.75×step, 1.25×step].
        for seed in 0u64..100 {
            let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
            let mut s = DialBackoff::new();
            s.record_failure_with_rng(t0, &mut rng);

            let step_ms = BACKOFF_STEPS[0] * 1_000;
            let min_ms = step_ms * 3 / 4;
            let max_ms = step_ms * 5 / 4;

            let next = s
                .next_attempt
                .expect("next_attempt must be set after failure");
            let actual_ms = next.duration_since(t0).as_millis() as u64;
            assert!(
                actual_ms >= min_ms && actual_ms <= max_ms,
                "seed={seed}: jittered delay {actual_ms}ms must be in [{min_ms}, {max_ms}]"
            );
        }
    }

    /// rh27: jitter must produce at least two distinct delay values across a
    /// large sample — confirming it is not a degenerate constant function.
    #[test]
    fn rh27_backoff_jitter_produces_distinct_delays() {
        let t0 = Instant::now();
        let mut seen_ms: std::collections::HashSet<u64> = std::collections::HashSet::new();

        for seed in 0u64..50 {
            let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
            let mut s = DialBackoff::new();
            s.record_failure_with_rng(t0, &mut rng);
            let next = s.next_attempt.unwrap();
            seen_ms.insert(next.duration_since(t0).as_millis() as u64);
        }

        assert!(
            seen_ms.len() >= 2,
            "CopyPaste-rh27: jitter must produce multiple distinct delay values, \
             got only {} distinct value(s): {:?}",
            seen_ms.len(),
            seen_ms,
        );
    }

    /// rh27: the nominal step value returned by `record_failure` must be the
    /// table entry (unchanged by jitter) so callers that log the "delay" see
    /// a human-readable rounded value, not the jittered millisecond count.
    #[test]
    fn rh27_record_failure_returns_nominal_step_not_jittered_value() {
        let t0 = Instant::now();
        let mut s = DialBackoff::new();
        let returned = s.record_failure(t0);
        assert_eq!(
            returned, BACKOFF_STEPS[0],
            "record_failure must return the nominal step, not the jittered duration"
        );
    }

    // ── M3: dwell-gated backoff reset ───────────────────────────────────────

    #[test]
    fn flapping_peer_keeps_escalating_backoff() {
        // A peer that connects then immediately drops (before MIN_HEALTHY_DWELL)
        // must NOT reset its backoff — otherwise it churns at the tick rate.
        let mut s = DialBackoff::new();
        let t0 = Instant::now();

        // First failure → step 0, idx advances to 1.
        assert_eq!(s.record_failure(t0), BACKOFF_STEPS[0]);
        assert_eq!(s.backoff_idx(), 1);

        // Connector dials again and the connect SUCCEEDS, but the link is a flap:
        // it comes up then drops well before the dwell window elapses.
        let t1 = t0 + Duration::from_secs(6);
        s.record_connected(t1);
        // A reset check 2s into the connection: dwell NOT yet met → no reset.
        assert!(!s.maybe_reset_after_dwell(t1 + Duration::from_secs(2)));
        assert_eq!(s.backoff_idx(), 1, "backoff must survive a sub-dwell flap");
        s.record_disconnected();

        // Next failure escalates from where we left off (idx 1 → BACKOFF_STEPS[1]).
        let t2 = t1 + Duration::from_secs(3);
        assert_eq!(
            s.record_failure(t2),
            BACKOFF_STEPS[1],
            "flap must not have reset the backoff"
        );
        assert_eq!(s.backoff_idx(), 2);
    }

    #[test]
    fn disconnect_before_dwell_blocks_later_reset() {
        // Regression for audit finding D: the connector must call
        // `record_disconnected` when it observes the sink gone, so a sub-dwell
        // flap clears `connected_since`. Otherwise the OLD connect instant lingers
        // and a later `maybe_reset_after_dwell` measures wall-time from it and
        // wrongly resets the backoff even though the new connection never dwelled.
        let mut s = DialBackoff::new();
        let t0 = Instant::now();

        // Escalate the backoff with a failure (idx 0 → 1).
        s.record_failure(t0);
        assert_eq!(s.backoff_idx(), 1);

        // A connection comes up, then drops before MIN_HEALTHY_DWELL elapses.
        // The connector observes the gone sink on a later tick and records the
        // disconnect (this is the call the production loop was missing).
        let connect_at = t0 + Duration::from_secs(1);
        s.record_connected(connect_at);
        s.record_disconnected();

        // Long after the original connect instant — far beyond MIN_HEALTHY_DWELL
        // of wall-time — a reset check must STILL be a no-op, because the link
        // never actually dwelled (connected_since was cleared).
        assert!(
            !s.maybe_reset_after_dwell(connect_at + MIN_HEALTHY_DWELL + Duration::from_secs(60)),
            "a sub-dwell flap must not reset backoff via stale wall-time"
        );
        assert_eq!(
            s.backoff_idx(),
            1,
            "backoff must stay escalated after a disconnect-before-dwell"
        );
    }

    #[test]
    fn healthy_dwell_resets_backoff() {
        let mut s = DialBackoff::new();
        let t0 = Instant::now();
        s.record_failure(t0);
        s.record_failure(t0); // idx now 2
        assert_eq!(s.backoff_idx(), 2);

        let connect_at = t0 + Duration::from_secs(100);
        s.record_connected(connect_at);

        // Just before the dwell threshold: no reset.
        assert!(
            !s.maybe_reset_after_dwell(connect_at + MIN_HEALTHY_DWELL - Duration::from_millis(1))
        );
        assert_eq!(s.backoff_idx(), 2);

        // At/after the dwell threshold: reset to the first step.
        assert!(s.maybe_reset_after_dwell(connect_at + MIN_HEALTHY_DWELL));
        assert_eq!(
            s.backoff_idx(),
            0,
            "healthy dwell must reset backoff to step 0"
        );

        // Idempotent: a second call after reset is a no-op.
        assert!(!s.maybe_reset_after_dwell(connect_at + MIN_HEALTHY_DWELL + Duration::from_secs(5)));
    }

    #[test]
    fn maybe_reset_is_noop_while_disconnected() {
        let mut s = DialBackoff::new();
        let t0 = Instant::now();
        s.record_failure(t0);
        // Never connected → connected_since is None → no reset regardless of time.
        assert!(!s.maybe_reset_after_dwell(t0 + Duration::from_secs(3600)));
        assert_eq!(s.backoff_idx(), 1);
    }

    #[test]
    fn may_dial_respects_backoff_window() {
        // Use a zero-jitter RNG fixture so the test is deterministic: seed the
        // SmallRng with a value that produces jitter_ms == 0 for BACKOFF_STEPS[0].
        // Rather than relying on a specific seed, use `record_failure_with_rng`
        // with a deterministic RNG and check the actual `next_attempt`.
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);
        let mut s = DialBackoff::new();
        let t0 = Instant::now();
        assert!(s.may_dial(t0), "fresh state dials immediately");

        let delay = s.record_failure_with_rng(t0, &mut rng);
        assert_eq!(delay, BACKOFF_STEPS[0]);

        // The actual next_attempt includes jitter; read it back and check
        // may_dial at both extremes of the jitter window.
        let next = s.next_attempt.expect("next_attempt set after failure");
        // 1 ms before next_attempt: must NOT be able to dial.
        assert!(
            !s.may_dial(next - Duration::from_millis(1)),
            "must not dial 1ms before next_attempt"
        );
        // At next_attempt: must be able to dial.
        assert!(s.may_dial(next), "must dial at or after next_attempt");
    }

    #[test]
    fn backoff_index_caps_at_last_step() {
        let mut s = DialBackoff::new();
        let t0 = Instant::now();
        for _ in 0..10 {
            s.record_failure(t0);
        }
        assert_eq!(s.backoff_idx(), BACKOFF_STEPS.len() - 1);
        assert_eq!(s.record_failure(t0), BACKOFF_STEPS[BACKOFF_STEPS.len() - 1]);
    }
}
