//! System / OS utility FFI exports.
//!
//! Covers: `classify_text_kind` (text-kind classifier), private-mode toggle
//! (`set_private_mode` / `get_private_mode`), and `resolve_stun_public_ip`.

// PG-16 (89ve): text-kind classification re-exported so Kotlin can call it
// instead of re-implementing the classifier in TextKind.kt.
use copypaste_core::text_kind::classify_text;

use crate::{panic_boundary, stun};

// ---------------------------------------------------------------------------
// PG-16 (89ve): Content-type (TextKind) classifier over Android FFI
//
// Android TextKind.kt re-implemented copypaste-core/src/text_kind.rs, causing
// silent classification drift (e.g. `{;` vs `contains(;)&&contains({)` for Code
// detection). This export delegates to `copypaste_core::text_kind::classify_text`
// so Kotlin can call the SINGLE canonical classifier rather than maintaining a
// parallel one. The Kotlin call-site swap in TextKind.kt is a SEPARATE agent
// step (GRADLE-REQUIRED).
//
// Returns the stable uppercase label (e.g. "TEXT", "URL", "CODE") that matches
// `TextKind::label()` in the Rust source.
// ---------------------------------------------------------------------------

/// Classify a text clipboard payload and return its stable uppercase kind label.
///
/// Delegates to `copypaste_core::text_kind::classify_text`, which is the SINGLE
/// canonical classifier both macOS and (after the Kotlin call-site migration)
/// Android will share. This eliminates the silent drift between TextKind.kt's
/// re-implementation and the core logic.
///
/// Returns one of: `"TEXT"`, `"URL"`, `"EMAIL"`, `"PHONE"`, `"COLOR"`, `"JSON"`,
/// `"CODE"`, `"NUMBER"`, `"PATH"`.
///
/// Wrapped in `panic_boundary::catch` — the classifier runs regex/allocation
/// that could panic; a caught panic returns `"TEXT"` (safest fallback: no
/// misclassification, just no decoration chip).
pub fn classify_text_kind(text: String) -> String {
    panic_boundary::catch(|| classify_text(&text).label().to_string())
        .unwrap_or_else(|_| "TEXT".to_string())
}

// ---------------------------------------------------------------------------
// PG-35 (08r1): Private mode FFI — Rust as the source of truth on Android
//
// macOS private mode is daemon-backed (AtomicBool in IpcHandler, persisted to
// disk by `persist_private_mode`). Android was SharedPrefs-only (Settings.kt:795)
// with no Rust path. The architecture note says SharedPrefs is "architecturally
// fine (no daemon)" but the capture path (ClipboardService.kt:887) must check
// the setting before recording any clip — if that check goes through SharedPrefs
// alone, a Rust code path that captures a clip bypasses the guard.
//
// This FFI exposes a Rust-side `AtomicBool` as the authoritative in-process flag.
// Kotlin MUST:
//   1. At startup: call `set_private_mode(prefs.getBoolean("private_mode", false))`
//      to seed the Rust flag from the persisted SharedPrefs value.
//   2. On every user toggle: call `set_private_mode(enabled)` AND persist to
//      SharedPrefs (Rust does not persist; Android has no daemon/disk store here).
//   3. Before capturing any clip: call `get_private_mode()` on the Rust side so
//      any Rust-side capture path honours the same flag.
//
// SECURITY: private mode suppresses capture of sensitive content. The flag MUST
// be seeded from SharedPrefs before the ClipboardService starts accepting clips.
// ---------------------------------------------------------------------------

/// Process-global private-mode flag.
///
/// `true` = private mode ON (suppress clipboard capture).
/// Initialised to `false` (capture on) at process start. Kotlin seeds it at
/// startup from SharedPrefs and keeps it in sync on every toggle.
static PRIVATE_MODE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Set the private-mode flag. Kotlin MUST call this:
///   - At service startup, seeded from SharedPrefs.
///   - On every user toggle (then also persist to SharedPrefs).
///
/// When `enabled` is `true`, clipboard capture MUST be suppressed by the
/// ClipboardService (check `get_private_mode()` before every capture).
///
/// Wrapped in `panic_boundary::catch` — cannot panic in practice; defensive.
pub fn set_private_mode(enabled: bool) {
    panic_boundary::catch(|| {
        PRIVATE_MODE.store(enabled, std::sync::atomic::Ordering::Relaxed);
    })
    .ok(); // void on panic: flag keeps its previous value rather than crashing JVM
}

/// Read the current private-mode flag. Returns `true` when private mode is ON.
///
/// Kotlin MUST check this on the Rust side before passing any clipboard content
/// to a Rust capture path so Rust-initiated captures honour the same toggle as
/// the SharedPrefs check in ClipboardService.kt:887.
pub fn get_private_mode() -> bool {
    panic_boundary::catch(|| PRIVATE_MODE.load(std::sync::atomic::Ordering::Relaxed))
        .unwrap_or(false) // conservative: default to "no private mode" on impossible panic
}

// ---------------------------------------------------------------------------
// PG-28 (8cu0): STUN public-IP resolution over Android FFI
//
// Android performs STUN on the Rust side so the discovered WAN address can be
// threaded into PeerMeta (same as the macOS daemon's public_ip.rs path).
// Kotlin MUST call this on a background/IO thread (it blocks for up to 5 s)
// and MUST gate the call behind `AppConfig.collect_public_ip` — exactly as
// the daemon gates `resolve_public_ip` behind `AppConfig::collect_public_ip`.
// The result (a public IPv4 string) is non-secret and may be stored in the
// devices table, but MUST NOT be logged at info level unnecessarily.

/// Discover this device's public (WAN) IPv4 address via a STUN Binding
/// Request to `stun.l.google.com:19302`.
///
/// **Blocking** — runs a UDP exchange with up to a 5-second timeout. Kotlin
/// MUST call this on an IO dispatcher, NOT the main thread. Returns `null`
/// on any failure (network unreachable, timeout, parse error).
///
/// Kotlin MUST gate this call behind the `collect_public_ip` setting (parity
/// with the macOS daemon's `AppConfig::collect_public_ip` gate). Exposing via
/// FFI so the same result feeds `PeerMeta.public_ip` during pairing (ABI 18).
pub fn resolve_stun_public_ip() -> Option<String> {
    panic_boundary::catch(stun::resolve_public_ip).unwrap_or(None)
}

// ---------------------------------------------------------------------------
// CopyPaste-km61: Expose SYNC_BADGE_RECENT_MS as a UniFFI getter
//
// Android DevicesOnlineState.kt previously hardcoded RECENT_SYNC_MS = 5 * 60 * 1_000L,
// a duplication of copypaste_ipc::SYNC_BADGE_RECENT_MS. Exposing the constant via FFI
// makes Kotlin read the SINGLE source of truth at runtime — if the Rust constant changes,
// Android picks it up on the next app build without a code change.
// ---------------------------------------------------------------------------

/// Mirrors `copypaste_ipc::SYNC_BADGE_RECENT_MS` (crates/copypaste-ipc/src/methods/badge.rs).
/// Both MUST stay equal — if either changes, update the other and re-export via FFI.
/// copypaste-android intentionally does not depend on copypaste-ipc to keep the FFI crate
/// lean; this constant is a local mirror.
const SYNC_BADGE_RECENT_MS_LOCAL: i64 = 5 * 60 * 1_000; // 5 minutes — single value

/// Return the RECENT_SYNC_MS recency window (milliseconds) from the Rust source of truth.
///
/// Mirrors `copypaste_ipc::SYNC_BADGE_RECENT_MS`. Kotlin MUST call this once at startup
/// and use the returned value wherever RECENT_SYNC_MS was previously hardcoded, ensuring
/// both platforms agree on the "recently synced" window. Returns 300_000 (5 minutes) at
/// compile time; only changes if the Rust constant is updated.
pub fn sync_badge_recent_ms() -> i64 {
    SYNC_BADGE_RECENT_MS_LOCAL
}

// ---------------------------------------------------------------------------
// CopyPaste-1jms.23: Canonical Android sync-badge state (Rust parity)
//
// The Kotlin `resolveSyncBadgeState` heuristic re-derives badge state from raw
// signals, but on macOS the daemon returns an authoritative `badge_state` string
// over IPC. This function is the CANONICAL Rust implementation of that same
// priority logic so Android can get a daemon-authoritative string from FFI and
// then surface it via `IpcSyncBadgeState.fromIpcString` → `toSyncBadgeState`.
//
// Priority order (matches Kotlin resolveSyncBadgeState + macOS daemon):
//  1. is_auth_error → "error"        (auth failure = DaemonUnreachable, red)
//  2. is_syncing    → "syncing"      (in-flight → Connected, green)
//  3. count > 0 AND recent sync AND has_internet → "synced" (Connected, green)
//  4. !has_internet → "offline"      (NetworkOffline, red)
//  5. Otherwise     → "idle"         (grey — no hard error, no recent sync)
//
// NOTE: is_auth_error is checked FIRST so an explicit auth failure is never
// masked by is_syncing (an in-flight retry after an auth error should still
// surface red, not green). This diverges from the Kotlin fallback heuristic
// (which has no is_auth_error signal) and is the whole point of this FFI:
// the daemon knows about auth failures that the heuristic cannot observe.
// ---------------------------------------------------------------------------

/// Compute the canonical Android sync-badge state string.
///
/// Returns one of: `"synced"`, `"syncing"`, `"idle"`, `"offline"`, `"error"`.
/// These are the same wire values as [`IpcSyncBadgeState`] in Kotlin, so the
/// caller can pass the result directly to `IpcSyncBadgeState.fromIpcString`.
///
/// # Parameters
/// - `online_count`: number of peers currently online (from DevicesOnlineState).
/// - `last_activity_ms`: wall-clock ms of most-recent successful sync (0 = never).
/// - `recent_sync_ms`: recency window in ms (mirrors `RECENT_SYNC_MS = 5 * 60 * 1000`).
/// - `has_internet`: true when OS reports a validated internet connection.
/// - `is_auth_error`: true when the last sync attempt hit an auth failure
///   (HTTP 401/403, bad credentials, RLS error). Takes highest priority — an
///   auth-failed device MUST show red regardless of other signals.
/// - `is_syncing`: true while a sync operation is actively in-flight.
/// - `now_ms`: current wall-clock time in ms (pass `System.currentTimeMillis()`).
///
/// Wrapped in `panic_boundary::catch` — no external I/O; cannot panic in practice.
pub fn compute_android_sync_badge_state(
    online_count: i64,
    last_activity_ms: i64,
    recent_sync_ms: i64,
    has_internet: bool,
    is_auth_error: bool,
    is_syncing: bool,
    now_ms: i64,
) -> String {
    panic_boundary::catch(|| {
        // 1. Auth error takes absolute priority — even over an in-flight retry.
        if is_auth_error {
            return "error".to_string();
        }
        // 2. Actively syncing → "syncing" (Connected, green).
        if is_syncing {
            return "syncing".to_string();
        }
        // 3. Recent successful sync with at least one peer → "synced" (Connected, green).
        let recent_enough = last_activity_ms > 0 && (now_ms - last_activity_ms) <= recent_sync_ms;
        if online_count > 0 && recent_enough {
            return "synced".to_string();
        }
        // 4. No validated OS internet → "offline" (NetworkOffline, red).
        if !has_internet {
            return "offline".to_string();
        }
        // 5. OS online, no auth error, no recent sync → "idle" (grey).
        "idle".to_string()
    })
    // On an impossible panic: return "idle" (grey, non-alarming fallback).
    .unwrap_or_else(|_| "idle".to_string())
}

#[cfg(test)]
mod sync_badge_tests {
    use super::{
        compute_android_sync_badge_state, sync_badge_recent_ms, SYNC_BADGE_RECENT_MS_LOCAL,
    };

    const RECENT_MS: i64 = 5 * 60 * 1_000; // 5 min
    const NOW: i64 = 1_000_000_000;

    fn badge(count: i64, last_ms: i64, internet: bool, auth_err: bool, syncing: bool) -> String {
        compute_android_sync_badge_state(
            count, last_ms, RECENT_MS, internet, auth_err, syncing, NOW,
        )
    }

    #[test]
    fn auth_error_returns_error_regardless_of_other_signals() {
        // Even with a recent sync and internet, auth error wins.
        assert_eq!("error", badge(1, NOW - 1_000, true, true, false));
        // Also when syncing concurrently — auth error takes priority over syncing.
        assert_eq!("error", badge(1, NOW - 1_000, true, true, true));
        // Also when offline.
        assert_eq!("error", badge(0, 0, false, true, false));
    }

    #[test]
    fn syncing_returns_syncing_when_no_auth_error() {
        assert_eq!("syncing", badge(0, 0, true, false, true));
        assert_eq!("syncing", badge(0, 0, false, false, true));
    }

    #[test]
    fn synced_when_count_positive_and_recent_sync() {
        let last = NOW - RECENT_MS + 1_000; // within window
        assert_eq!("synced", badge(1, last, true, false, false));
    }

    #[test]
    fn offline_when_no_internet_and_no_recent_sync() {
        assert_eq!("offline", badge(0, 0, false, false, false));
    }

    #[test]
    fn idle_when_online_but_no_recent_sync() {
        assert_eq!("idle", badge(0, 0, true, false, false));
    }

    #[test]
    fn idle_when_stale_sync_despite_positive_count() {
        let stale = NOW - RECENT_MS - 1_000; // outside window
        assert_eq!("idle", badge(1, stale, true, false, false));
    }

    // ── CopyPaste-km61: sync_badge_recent_ms getter ───────────────────────────

    /// sync_badge_recent_ms() must return the SAME value as the SYNC_BADGE_RECENT_MS_LOCAL
    /// constant. If either is changed independently, this test catches the drift.
    #[test]
    fn sync_badge_recent_ms_matches_local_constant() {
        assert_eq!(
            sync_badge_recent_ms(),
            SYNC_BADGE_RECENT_MS_LOCAL,
            "sync_badge_recent_ms() must equal SYNC_BADGE_RECENT_MS_LOCAL (300_000)"
        );
    }

    /// sync_badge_recent_ms() must return 300_000 (5 minutes in ms).
    /// This is the CANONICAL badge-recency window shared with the Kotlin RECENT_SYNC_MS
    /// constant and macOS SyncStatusChip. Any change to this value MUST be coordinated
    /// across copypaste_ipc::SYNC_BADGE_RECENT_MS, this constant, and DevicesOnlineState.kt.
    #[test]
    fn sync_badge_recent_ms_is_five_minutes() {
        const FIVE_MINUTES_MS: i64 = 5 * 60 * 1_000;
        assert_eq!(
            sync_badge_recent_ms(),
            FIVE_MINUTES_MS,
            "badge-recency window must be 5 minutes (300_000 ms)"
        );
    }
}
