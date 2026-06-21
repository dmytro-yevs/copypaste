//! `SyncBadgeState` enum and the canonical badge-derivation logic.
//!
//! This is the only module in `methods/` that contains real logic (as opposed to
//! pure data declarations). Keeping it isolated gives it a clear tested home and
//! makes it easy to find the derivation rules.

use serde::{Deserialize, Serialize};

// ── SyncBadgeState — canonical daemon-computed badge state ──────────────────

/// The canonical sync-badge state computed once by the daemon and delivered
/// over IPC so every consumer (macOS UI, Android) renders an identical badge
/// without each re-implementing the derivation logic.
///
/// ## Motivation (CopyPaste-merc)
///
/// Before this type, macOS `SyncStatusChip.tsx` and Android `SyncStatusBadge.kt`
/// each re-derived the badge from raw IPC fields using local constants
/// (`RECENT_SYNC_MS = 300_000` on each platform) that could drift independently.
/// The badge could disagree on a daemon crash (macOS sees IPC-unreachable →
/// `Offline`; Android only sees OS-network → `NetworkOffline`).
///
/// Now the daemon is the **single source of truth**. Consumers that receive
/// `badge_state` in the `get_sync_status` response must render it directly and
/// must NOT re-derive the state from raw fields. A thin backward-compat
/// fallback is permitted only for responses from daemons older than this field.
///
/// ## Variants
///
/// | Variant          | Dot colour       | Meaning                                              |
/// |------------------|------------------|------------------------------------------------------|
/// | `Synced`         | green            | At least one peer exchanged data within 5 minutes.   |
/// | `Syncing`        | green (pulse)    | A sync round-trip is actively in flight.             |
/// | `Idle`           | grey             | Configured but no recent sync (devices may be off).  |
/// | `Offline`        | red              | Daemon detects no usable sync path.                  |
/// | `Error`          | red              | Sync backend returned an explicit error.             |
/// | `Misconfigured`  | amber            | Cloud URL set but credentials incomplete/invalid.    |
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncBadgeState {
    /// At least one peer/backend exchanged data within [`SYNC_BADGE_RECENT_MS`].
    Synced,
    /// A sync round-trip is actively in flight (future: when daemon exposes this).
    Syncing,
    /// Sync is configured but no recent successful exchange. Peers may be off.
    Idle,
    /// Daemon cannot reach any sync backend — IPC unreachable or no network path.
    Offline,
    /// Sync backend returned an explicit error (auth failure, RLS, relay down).
    Error,
    /// Cloud URL is set but credentials are missing or invalid
    /// (`supabase_configured == false` while `supabase_url` is non-empty).
    Misconfigured,
}

/// How recent a last-sync timestamp must be (milliseconds) for the daemon to
/// consider the link "synced". Single source of truth — replaces the per-platform
/// `RECENT_SYNC_MS` constants (macOS 300_000 and Android 5 * 60 * 1_000 L) that
/// were duplicated and could drift independently.
pub const SYNC_BADGE_RECENT_MS: u64 = 5 * 60 * 1_000; // 5 minutes

/// Compute the [`SyncBadgeState`] from raw daemon-side signals.
///
/// This is the single place where the badge derivation lives. The daemon calls
/// this and embeds the result in the `get_sync_status` response so consumers
/// never need to re-derive it.
///
/// # Arguments
///
/// * `passphrase_set` — whether a sync key is loaded (P2P or cloud).
/// * `supabase_url_set` — whether a Supabase project URL is configured.
/// * `supabase_configured` — URL + anon key both present (or `SUPABASE_URL` env).
/// * `signed_in` — whether GoTrue auth succeeded.
/// * `last_sync_ms` — timestamp of the last successful exchange (epoch ms), or
///   `None` when never synced.
/// * `now_ms` — current wall-clock time (epoch ms). Pass `None` to use
///   `std::time::SystemTime::now()` automatically.
///
/// To signal an active in-flight sync round-trip, use
/// [`compute_sync_badge_state_with_inflight`] instead. This function is kept
/// for backward-compatibility with existing callers and delegates with
/// `in_flight = false`.
pub fn compute_sync_badge_state(
    passphrase_set: bool,
    supabase_url_set: bool,
    supabase_configured: bool,
    signed_in: bool,
    last_sync_ms: Option<i64>,
    now_ms: Option<u64>,
) -> SyncBadgeState {
    // Delegate to the extended variant with in_flight=false so the existing
    // daemon caller continues to compile and behave identically (CopyPaste-1jms.22).
    compute_sync_badge_state_with_inflight(
        passphrase_set,
        supabase_url_set,
        supabase_configured,
        signed_in,
        last_sync_ms,
        now_ms,
        false,
    )
}

/// Extended variant of [`compute_sync_badge_state`] that adds an `in_flight`
/// signal for when a sync round-trip is actively in progress.
///
/// When `in_flight` is `true` and no recent sync has already been recorded,
/// this returns [`SyncBadgeState::Syncing`] (green pulse) instead of falling
/// through to the `Error`/`Offline`/`Idle` branches. The `Syncing` state is
/// transient: the caller is responsible for setting `in_flight` back to
/// `false` once the round-trip completes or fails.
///
/// The daemon should adopt this function once it threads an `Arc<AtomicBool>`
/// in-flight flag through the cloud-poll, relay-receive, and P2P loops.
///
/// # Arguments
///
/// Same as [`compute_sync_badge_state`], plus:
///
/// * `in_flight` — `true` while a cloud poll, relay push, or P2P handshake is
///   actively running.
pub fn compute_sync_badge_state_with_inflight(
    passphrase_set: bool,
    supabase_url_set: bool,
    supabase_configured: bool,
    signed_in: bool,
    last_sync_ms: Option<i64>,
    now_ms: Option<u64>,
    in_flight: bool,
) -> SyncBadgeState {
    // Resolve current time — allows tests to inject a deterministic value.
    let now = now_ms.unwrap_or_else(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    });

    // Misconfig: cloud URL set but credentials absent/incomplete → amber.
    // Check this BEFORE the "no sync configured" path so a partially-configured
    // Supabase setup shows amber rather than the misleading grey idle dot.
    if supabase_url_set && !supabase_configured {
        return SyncBadgeState::Misconfigured;
    }

    // Recent sync: compare last_sync_ms against the 5-minute threshold.
    let recently_synced = last_sync_ms
        .map(|ts| ts > 0 && now.saturating_sub(ts as u64) <= SYNC_BADGE_RECENT_MS)
        .unwrap_or(false);

    if recently_synced {
        return SyncBadgeState::Synced;
    }

    // Active round-trip in progress and no recent completed sync → Syncing
    // (green pulse). Placed after Synced so a completed sync wins over an
    // in-flight one: if last_sync_ms is recent the round-trip is wrapping up
    // and Synced is the more accurate label.
    if in_flight {
        return SyncBadgeState::Syncing;
    }

    // Auth error: cloud is configured and URL is valid but GoTrue session failed.
    if supabase_configured && !signed_in {
        return SyncBadgeState::Error;
    }

    // No sync path configured at all AND no recent activity → Offline.
    // "No path" means neither a passphrase (P2P/relay) nor a Supabase URL.
    if !passphrase_set && !supabase_url_set {
        return SyncBadgeState::Offline;
    }

    // Configured but stale — idle grey.
    SyncBadgeState::Idle
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper: a fixed "now" far enough from any test timestamp.
    const NOW_MS: u64 = 1_000_000_000_000; // 2001-09-09 in ms
                                           // "5 minutes ago minus 1 s" — inside the RECENT window.
    const RECENT_MS: i64 = (NOW_MS - SYNC_BADGE_RECENT_MS + 1_000) as i64;
    // "5 minutes ago plus 1 s" — outside the RECENT window.
    const STALE_MS: i64 = (NOW_MS - SYNC_BADGE_RECENT_MS - 1_000) as i64;

    #[test]
    fn badge_state_synced_when_recent_sync() {
        let state = compute_sync_badge_state(
            true, // passphrase_set
            true, // supabase_url_set
            true, // supabase_configured
            true, // signed_in
            Some(RECENT_MS),
            Some(NOW_MS),
        );
        assert_eq!(state, SyncBadgeState::Synced);
    }

    #[test]
    fn badge_state_idle_when_stale_sync_but_configured() {
        let state = compute_sync_badge_state(
            true, // passphrase_set
            true, // supabase_url_set
            true, // supabase_configured
            true, // signed_in
            Some(STALE_MS),
            Some(NOW_MS),
        );
        assert_eq!(state, SyncBadgeState::Idle);
    }

    #[test]
    fn badge_state_idle_when_never_synced_but_passphrase_set() {
        let state = compute_sync_badge_state(
            true,  // passphrase_set — a sync path exists
            false, // supabase_url_set
            false, // supabase_configured
            false, // signed_in
            None,  // never synced
            Some(NOW_MS),
        );
        // passphrase_set = true means a P2P sync path is configured → Idle, not Offline.
        assert_eq!(state, SyncBadgeState::Idle);
    }

    #[test]
    fn badge_state_offline_when_nothing_configured() {
        let state = compute_sync_badge_state(
            false, // passphrase_set
            false, // supabase_url_set
            false, // supabase_configured
            false, // signed_in
            None,  // never synced
            Some(NOW_MS),
        );
        assert_eq!(state, SyncBadgeState::Offline);
    }

    #[test]
    fn badge_state_misconfigured_when_url_set_but_not_configured() {
        // Cloud URL is set but anon key / credentials are missing.
        let state = compute_sync_badge_state(
            false, // passphrase_set
            true,  // supabase_url_set
            false, // supabase_configured — anon key absent
            false, // signed_in
            None,
            Some(NOW_MS),
        );
        assert_eq!(state, SyncBadgeState::Misconfigured);
    }

    #[test]
    fn badge_state_error_when_configured_but_not_signed_in() {
        // URL + anon key present, but GoTrue auth failed (signed_in = false).
        let state = compute_sync_badge_state(
            false, // passphrase_set
            true,  // supabase_url_set
            true,  // supabase_configured
            false, // signed_in — auth failure
            Some(STALE_MS),
            Some(NOW_MS),
        );
        assert_eq!(state, SyncBadgeState::Error);
    }

    #[test]
    fn badge_state_synced_takes_priority_over_error() {
        // Even when signed_in=false, a RECENT sync means Synced (key rotation in
        // flight, or config changing mid-session).
        let state = compute_sync_badge_state(
            true,  // passphrase_set
            true,  // supabase_url_set
            true,  // supabase_configured
            false, // signed_in — but recent exchange happened
            Some(RECENT_MS),
            Some(NOW_MS),
        );
        assert_eq!(state, SyncBadgeState::Synced);
    }

    // ── compute_sync_badge_state_with_inflight tests (CopyPaste-1jms.22) ──────

    #[test]
    fn badge_state_syncing_when_in_flight_and_no_recent_sync() {
        // The primary acceptance criterion: in_flight=true with no recent sync
        // must return Syncing (green pulse).
        let state = compute_sync_badge_state_with_inflight(
            true, // passphrase_set
            true, // supabase_url_set
            true, // supabase_configured
            true, // signed_in
            None, // no prior sync
            Some(NOW_MS),
            true, // in_flight — round-trip actively running
        );
        assert_eq!(state, SyncBadgeState::Syncing);
    }

    #[test]
    fn badge_state_synced_wins_over_in_flight_when_recently_synced() {
        // A completed recent sync takes priority over an in-flight flag: the
        // round-trip is wrapping up and Synced is the more accurate label.
        let state = compute_sync_badge_state_with_inflight(
            true,
            true,
            true,
            true,
            Some(RECENT_MS),
            Some(NOW_MS),
            true, // in_flight set — but recently_synced wins
        );
        assert_eq!(state, SyncBadgeState::Synced);
    }

    #[test]
    fn badge_state_in_flight_false_behaves_identically_to_original() {
        // in_flight=false must not change the derivation — ensures backward
        // compatibility between compute_sync_badge_state and the _with_inflight
        // variant.
        // Each tuple is (passphrase_set, url_set, configured, signed_in, last_sync,
        // expected_badge).  The six-element anonymous tuple is deliberately
        // kept inline here — a named type would add noise without clarity for a
        // single test-internal table.
        #[allow(clippy::type_complexity)]
        let cases: &[(bool, bool, bool, bool, Option<i64>, SyncBadgeState)] = &[
            (
                true,
                true,
                true,
                true,
                Some(RECENT_MS),
                SyncBadgeState::Synced,
            ),
            (true, true, true, true, Some(STALE_MS), SyncBadgeState::Idle),
            (false, false, false, false, None, SyncBadgeState::Offline),
            (
                false,
                true,
                false,
                false,
                None,
                SyncBadgeState::Misconfigured,
            ),
            (
                false,
                true,
                true,
                false,
                Some(STALE_MS),
                SyncBadgeState::Error,
            ),
        ];
        for (passphrase_set, url_set, configured, signed_in, last_sync, expected) in cases {
            let via_new = compute_sync_badge_state_with_inflight(
                *passphrase_set,
                *url_set,
                *configured,
                *signed_in,
                *last_sync,
                Some(NOW_MS),
                false, // in_flight=false → should match the old function
            );
            let via_old = compute_sync_badge_state(
                *passphrase_set,
                *url_set,
                *configured,
                *signed_in,
                *last_sync,
                Some(NOW_MS),
            );
            assert_eq!(via_new, *expected, "new fn mismatch");
            assert_eq!(via_old, *expected, "old fn mismatch");
            assert_eq!(
                via_new, via_old,
                "parity between old and new(in_flight=false)"
            );
        }
    }

    #[test]
    fn sync_badge_state_serialises_to_snake_case() {
        let cases = [
            (SyncBadgeState::Synced, r#""synced""#),
            (SyncBadgeState::Syncing, r#""syncing""#),
            (SyncBadgeState::Idle, r#""idle""#),
            (SyncBadgeState::Offline, r#""offline""#),
            (SyncBadgeState::Error, r#""error""#),
            (SyncBadgeState::Misconfigured, r#""misconfigured""#),
        ];
        for (variant, expected) in &cases {
            let s = serde_json::to_string(variant).unwrap();
            assert_eq!(&s, expected, "variant serialisation mismatch");
        }
    }
}
