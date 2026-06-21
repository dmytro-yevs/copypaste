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
