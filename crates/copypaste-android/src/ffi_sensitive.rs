//! Sensitive-detection FFI exports.
//!
//! Covers: `is_sensitive`, `sensitive_kind`, `sensitive_capture_decision`,
//! `sensitive_expires_at_ms`, `detect_sensitive_spans`, `byte_to_char_offset_android`,
//! and supporting types `SensitiveCaptureDecision` / `SensitiveSpan`.

use copypaste_core::{detect, is_sensitive_for_autowipe};

use crate::panic_boundary;

// ---------------------------------------------------------------------------
// PG-3 (349q): sensitive_capture_decision — single source of truth for whether
// text is sensitive at capture time. Returns `SensitiveCaptureDecision` with
// three fields Kotlin needs to store+mask a sensitive item correctly:
//
//   is_sensitive   — true when confidence >= 0.70 (same as macOS daemon gate)
//   kind           — the SensitiveKind label, or None when not sensitive
//   expires_at_ms  — unix-ms expiry timestamp (now_unix_ms + ttl_secs * 1000),
//                    or None when sensitive_ttl_secs == 0 ("auto-wipe disabled")
//                    or when the text is not sensitive
//
// This replaces the split calls to is_sensitive / sensitive_kind / separate
// expires_at computation that ClipboardService.kt would otherwise need to
// coordinate. One FFI round-trip per capture is cheaper and keeps the logic
// in Rust where it belongs.
//
// SECURITY: the item_id AAD binding (in encrypt_text / decrypt_text) is
// unchanged — callers still pass item_id into the crypto functions. This
// function is PURE (no DB I/O, no file I/O).
//
// PG-4  (ojsh): sensitive_spans — core detector spans for Kotlin masking.
// PG-24 (5tnx): sensitive_expires_at_ms — per-item expires_at from core TTL.
// ---------------------------------------------------------------------------

/// Result of `sensitive_capture_decision` — single-round-trip sensitivity
/// verdict for one clipboard item at capture time.
///
/// Kotlin stores `is_sensitive` and `expires_at_ms` in the DB row and uses
/// `kind` for the badge label. If `is_sensitive` is false, `kind` and
/// `expires_at_ms` are always `None`.
///
/// `expires_at_ms` is `None` when:
///   - `is_sensitive` is false, OR
///   - `sensitive_ttl_secs` is 0 (the "auto-wipe disabled" sentinel).
pub struct SensitiveCaptureDecision {
    /// True when the text triggers the >= 0.70 confidence floor.
    pub is_sensitive: bool,
    /// The canonical sensitive-kind label (e.g. `"AwsKey"`, `"CreditCard"`),
    /// or `None` when the text is not sensitive.
    pub kind: Option<String>,
    /// Unix-millisecond expiry timestamp for this item, or `None` when
    /// auto-wipe is disabled (`sensitive_ttl_secs == 0`) or not sensitive.
    pub expires_at_ms: Option<i64>,
}

/// One matched sensitive span (char-offset, NOT byte-offset).
///
/// `start` and `end` are Unicode scalar-value indices into the NFKC-normalised
/// form of the input text. Kotlin masks `text[start..<end]` with bullet chars.
///
/// NOTE ON NORMALIZATION: `copypaste_core::sensitive::nfkc_normalize` is
/// idempotent on ASCII and almost all practical clipboard text. The only time
/// the normalised string differs from the original is when the text contains
/// full-width Unicode digits/letters (the NFKC form collapses them to ASCII).
/// In that case Kotlin should normalise the text before rendering spans.
pub struct SensitiveSpan {
    /// Start character index (inclusive) in the NFKC-normalised text.
    pub start: u32,
    /// End character index (exclusive) in the NFKC-normalised text.
    pub end: u32,
    /// Confidence score of this match (0.0 – 1.0).
    pub confidence: f32,
    /// Pattern name (e.g. `"aws_access_key"`, `"credit_card"`, `"jwt"`).
    pub pattern_name: String,
}

/// Returns `true` if `text` is sensitive at the HIGH-confidence threshold.
///
/// AB-6a (v0.6.1 threshold parity): this used to flag on `detect(&text).is_some()`
/// — i.e. ANY pattern match, including low-confidence heuristics (phone 0.55,
/// passport 0.55, email 0.60). macOS gates on confidence >= 0.70
/// (`is_sensitive_for_autowipe`), so the two platforms disagreed: mildly-sensitive
/// text that macOS keeps was dropped on Android. We now call the SAME core gate
/// (`is_sensitive_for_autowipe`, the >= 0.70 floor) so the sensitivity verdict is
/// byte-for-byte identical to the daemon's. The Kotlin store policy (store+mask
/// vs drop) is changed in a LATER wave — here we only align the threshold.
///
/// Wrapped in [`panic_boundary::catch`] because the detector runs regex/allocation
/// that could panic; an unwound panic across the JNI boundary aborts the JVM. This
/// export returns a plain `bool`, so a caught panic recovers to `false` (treat as
/// "not sensitive" rather than crash).
pub fn is_sensitive(text: String) -> bool {
    panic_boundary::catch(|| is_sensitive_for_autowipe(&text)).unwrap_or(false)
}

/// Returns the sensitive-kind label for `text`, or `None` if not sensitive.
///
/// PG-23 (l9z8) alignment: `sensitive_kind` now gates at the SAME >= 0.70
/// confidence floor as `is_sensitive_for_autowipe` / `is_sensitive`. Previously
/// it called `detect()` which fires on ANY pattern including low-confidence
/// heuristics (phone 0.55, passport 0.55, email 0.60, IBAN 0.65, SSN 0.65).
/// This produced a divergence where `sensitive_kind` returned `Some("Phone")`
/// while `is_sensitive` returned `false` for the same phone number, confusing
/// Kotlin callers that relied on `sensitive_kind.isNotNull()` as a sensitivity
/// signal.
///
/// The fix: only return a non-null kind for patterns whose confidence is >= 0.70
/// (the SAME autowipe floor). Low-confidence pattern hits that fall below the
/// floor are still available via `detect_sensitive_spans` / `is_sensitive_for_autowipe`
/// but `sensitive_kind` is now purely an informational label that agrees with
/// `is_sensitive`.
///
/// Wrapped in [`panic_boundary::catch`] for the same reason as
/// [`is_sensitive`]. This export returns a plain `Option<String>`, so a caught
/// panic recovers to `None`.
pub fn sensitive_kind(text: String) -> Option<String> {
    panic_boundary::catch(|| {
        // Only report a kind when the text also triggers the auto-wipe gate
        // (confidence >= 0.70). This keeps sensitive_kind and is_sensitive in
        // sync — Kotlin can safely use `sensitive_kind.isNotNull()` as a proxy
        // for is_sensitive.
        if !is_sensitive_for_autowipe(&text) {
            return None;
        }
        detect(&text).map(|k| format!("{:?}", k))
    })
    .unwrap_or(None)
}

/// Compute the sensitivity verdict and auto-wipe expiry for one clipboard item
/// at capture time.
///
/// `now_unix_ms` is the current wall-clock time in Unix milliseconds. Kotlin
/// should pass `System.currentTimeMillis()`. `sensitive_ttl_secs` is from the
/// user-tunable config (defaults to 30 s; 0 = "auto-wipe disabled").
///
/// This is the CORRECT gate for Android capture (PG-3 / 349q). It uses the
/// SAME `is_sensitive_for_autowipe` (>= 0.70 confidence floor) as the macOS
/// daemon, so a phone number (confidence 0.55) is NOT flagged and NOT dropped
/// on Android. Previously ClipboardService.kt checked `is_sensitive` and
/// early-returned, dropping items that macOS keeps.
///
/// PURE — no DB I/O.
pub fn sensitive_capture_decision(
    text: String,
    now_unix_ms: i64,
    sensitive_ttl_secs: u64,
) -> SensitiveCaptureDecision {
    panic_boundary::catch(|| {
        let sensitive = is_sensitive_for_autowipe(&text);
        if !sensitive {
            return SensitiveCaptureDecision {
                is_sensitive: false,
                kind: None,
                expires_at_ms: None,
            };
        }
        let kind = detect(&text).map(|k| format!("{:?}", k));
        // sensitive_ttl_secs == 0 is the "never wipe" sentinel — no expiry.
        let expires_at_ms = if sensitive_ttl_secs == 0 {
            None
        } else {
            Some(now_unix_ms.saturating_add(sensitive_ttl_secs as i64 * 1000))
        };
        SensitiveCaptureDecision {
            is_sensitive: true,
            kind,
            expires_at_ms,
        }
    })
    .unwrap_or(SensitiveCaptureDecision {
        is_sensitive: false,
        kind: None,
        expires_at_ms: None,
    })
}

// ---------------------------------------------------------------------------
// PG-24 (5tnx): sensitive_expires_at_ms
//
// macOS daemon stamps `expires_at = now_ms + sensitive_ttl_local_secs * 1000`
// (daemon.rs:2183) at capture. Android ClipboardRepository.kt:1128-1177 only
// pruned by age in getItems(), leaving expired items alive in suspended apps.
//
// This FFI computes the per-item expiry timestamp from the SAME formula so
// Kotlin stores `expires_at` in the DB row and a WorkManager periodic job can
// sweep stale rows even when the app is suspended.
//
// Returns None when sensitive_ttl_secs == 0 ("auto-wipe disabled" sentinel).
// ---------------------------------------------------------------------------

/// Compute the per-item `expires_at` timestamp (Unix milliseconds) for a
/// sensitive clipboard item, matching the daemon's formula:
///
///   `expires_at = now_unix_ms + sensitive_ttl_secs * 1000`
///
/// Returns `None` when `sensitive_ttl_secs == 0` (the "auto-wipe disabled"
/// sentinel — Kotlin should not write `expires_at` for such items).
///
/// `now_unix_ms` is `System.currentTimeMillis()` from Kotlin.
/// `sensitive_ttl_secs` is the user-tunable `Config.sensitive_ttl_secs`
/// (default 30, from `default_config()`).
///
/// PURE — no DB I/O. Wrapped in `panic_boundary::catch` as a defensive
/// measure; the saturation math cannot panic in practice.
pub fn sensitive_expires_at_ms(now_unix_ms: i64, sensitive_ttl_secs: u64) -> Option<i64> {
    panic_boundary::catch(|| {
        if sensitive_ttl_secs == 0 {
            return None;
        }
        Some(now_unix_ms.saturating_add(sensitive_ttl_secs as i64 * 1000))
    })
    .unwrap_or(None)
}

// ---------------------------------------------------------------------------
// PG-4 (ojsh): detect_sensitive_spans — sensitive byte spans for Kotlin masking
//
// macOS daemon ipc.rs:4460-4487 calls `SensitiveDetector::detect_normalised` and
// maps byte→char offsets for the `sensitive_spans` JSON array used by
// HistoryView.tsx to bullet-mask embedded credentials. Android had no equivalent,
// so a card/IBAN buried in longer non-sensitive text showed unmasked.
//
// This FFI returns the same char-offset spans so Kotlin can mask sub-string
// sensitive matches in the history list. PURE — no DB I/O.
//
// NOTE: spans are over the NFKC-NORMALISED string, not the original. Kotlin
// must use `SensitiveSpan.start/end` as character indices into the normalised
// string returned alongside the spans (or re-normalise the same text before
// masking). Normalization rarely changes the string (only Unicode bypass tricks
// trigger it), so callers can usually index into the original text directly —
// but correctness requires the normalised form.
// ---------------------------------------------------------------------------

/// Detect sensitive spans in `text` and return their char offsets for masking.
///
/// Uses `SensitiveDetector::detect_normalised` (the SAME detector as the macOS
/// daemon's `sensitive_spans` IPC response) to find all pattern matches,
/// including low-confidence hits (phone 0.55, IBAN 0.65) — the masking
/// decision is intentionally broader than the auto-wipe gate. Kotlin masks
/// ALL returned spans regardless of confidence (any credential visible in the
/// history list should be obscured).
///
/// The returned spans are char-offset indices into the NFKC-normalised
/// rendering of `text`. For ASCII text (virtually all practical clipboard
/// content) the normalised form is byte-for-byte identical to the original, so
/// Kotlin can index directly. For unusual Unicode input Kotlin should run
/// `text.normalize(Form.NFKC)` before applying the offsets.
///
/// Returns an empty `Vec` when no sensitive patterns are found. Wrapped in
/// `panic_boundary::catch` — the detector runs regex/allocation that could
/// panic; a caught panic returns an empty span list (safe: no masking applied).
pub fn detect_sensitive_spans(text: String) -> Vec<SensitiveSpan> {
    panic_boundary::catch(|| {
        use copypaste_core::sensitive::nfkc_normalize;
        let normalised = nfkc_normalize(&text);
        let detector = copypaste_core::SensitiveDetector::new();
        detector
            .detect_normalised(&normalised)
            .into_iter()
            .map(|m| {
                let start = byte_to_char_offset_android(&normalised, m.matched_range.start);
                let end = byte_to_char_offset_android(&normalised, m.matched_range.end);
                SensitiveSpan {
                    start,
                    end,
                    confidence: m.confidence,
                    pattern_name: m.pattern_name.to_string(),
                }
            })
            .collect()
    })
    .unwrap_or_default()
}

/// Convert a byte offset in `s` to a char (Unicode scalar value) offset.
///
/// Mirrors the daemon's `byte_to_char_offset` helper (ipc.rs) used to generate
/// the `sensitive_spans` JSON array. A byte offset equal to `s.len()` maps to
/// the char count (one-past-the-end). An out-of-bounds byte offset saturates to
/// the char count. The result is capped at `u32::MAX` for the FFI type; in
/// practice no clipboard item approaches 4 billion chars.
pub fn byte_to_char_offset_android(s: &str, byte_offset: usize) -> u32 {
    // Count the number of chars whose byte offset is strictly less than
    // `byte_offset`. This matches the daemon's `byte_to_char_offset` helper
    // that iterates `char_indices` and counts chars up to the target byte.
    let count = s
        .char_indices()
        .take_while(|(bi, _)| *bi < byte_offset)
        .count();
    count.min(u32::MAX as usize) as u32
}
