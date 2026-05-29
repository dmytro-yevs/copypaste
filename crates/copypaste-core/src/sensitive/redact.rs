//! Log-safe redaction of sensitive content.
//!
//! Replaces every [`PatternMatch`] range in the original text with `***REDACTED***`.
//! Overlapping matches are merged so the replacement string is never duplicated.

use super::detector::PatternMatch;

const REDACTED: &str = "***REDACTED***";

/// Replace all matched ranges in `text` with `***REDACTED***`.
///
/// Ranges are merged when they overlap or are adjacent, so a single
/// `***REDACTED***` placeholder covers the entire span.
pub fn redact(text: &str, matches: &[PatternMatch]) -> String {
    if matches.is_empty() {
        return text.to_owned();
    }

    // Collect and sort byte ranges; then merge overlapping spans.
    let mut spans: Vec<(usize, usize)> = matches
        .iter()
        .map(|m| (m.matched_range.start, m.matched_range.end))
        .collect();
    spans.sort_unstable();

    let merged = merge_spans(spans);

    // Build output by stitching literal segments with REDACTED placeholders.
    // Span bounds are snapped to UTF-8 char boundaries before slicing: a
    // caller passing un-normalised offsets that land mid-codepoint would
    // otherwise panic on `text[..]`. Snapping outward keeps the redaction
    // conservative (it can only ever cover *more* of the matched bytes).
    let mut out = String::with_capacity(text.len());
    let mut cursor = 0usize;
    for (start, end) in merged {
        let start = floor_char_boundary(text, start);
        let end = ceil_char_boundary(text, end);
        if start > cursor {
            out.push_str(&text[cursor..start]);
        }
        out.push_str(REDACTED);
        cursor = end;
    }
    if cursor < text.len() {
        out.push_str(&text[cursor..]);
    }
    out
}

/// Largest char boundary `<= i` (clamped to `text.len()`).
fn floor_char_boundary(text: &str, i: usize) -> usize {
    let mut i = i.min(text.len());
    while i > 0 && !text.is_char_boundary(i) {
        i -= 1;
    }
    i
}

/// Smallest char boundary `>= i` (clamped to `text.len()`).
fn ceil_char_boundary(text: &str, i: usize) -> usize {
    let len = text.len();
    let mut i = i.min(len);
    while i < len && !text.is_char_boundary(i) {
        i += 1;
    }
    i
}

fn merge_spans(sorted: Vec<(usize, usize)>) -> Vec<(usize, usize)> {
    let mut merged: Vec<(usize, usize)> = Vec::with_capacity(sorted.len());
    for (s, e) in sorted {
        if let Some(last) = merged.last_mut() {
            if s <= last.1 {
                // Overlapping or adjacent — extend the current span.
                if e > last.1 {
                    last.1 = e;
                }
                continue;
            }
        }
        merged.push((s, e));
    }
    merged
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::super::detector::{PatternMatch, SensitiveCategory};
    use super::*;

    fn make_match(start: usize, end: usize, name: &'static str) -> PatternMatch {
        PatternMatch {
            pattern_name: name,
            confidence: 0.9,
            category: SensitiveCategory::Credential,
            matched_range: start..end,
        }
    }

    #[test]
    fn no_matches_returns_original() {
        let text = "Hello, world!";
        assert_eq!(redact(text, &[]), text);
    }

    #[test]
    fn single_match_at_start() {
        let text = "SECRET rest";
        //          0123456
        let m = make_match(0, 6, "test");
        assert_eq!(redact(text, &[m]), "***REDACTED*** rest");
    }

    #[test]
    fn single_match_in_middle() {
        let text = "prefix SECRET suffix";
        let m = make_match(7, 13, "test");
        assert_eq!(redact(text, &[m]), "prefix ***REDACTED*** suffix");
    }

    #[test]
    fn single_match_at_end() {
        let text = "prefix SECRET";
        let m = make_match(7, 13, "test");
        assert_eq!(redact(text, &[m]), "prefix ***REDACTED***");
    }

    #[test]
    fn two_non_overlapping_matches() {
        let text = "AAAA middle BBBB";
        let m1 = make_match(0, 4, "a");
        let m2 = make_match(12, 16, "b");
        assert_eq!(
            redact(text, &[m1, m2]),
            "***REDACTED*** middle ***REDACTED***"
        );
    }

    #[test]
    fn overlapping_matches_merged() {
        let text = "ABCDEF rest";
        let m1 = make_match(0, 4, "a"); // ABCD
        let m2 = make_match(2, 6, "b"); // CDEF — overlaps with m1
        let result = redact(text, &[m1, m2]);
        // Both spans [0,4) and [2,6) merge to [0,6) → one REDACTED
        assert_eq!(result, "***REDACTED*** rest");
        assert_eq!(result.matches(super::REDACTED).count(), 1);
    }

    #[test]
    fn adjacent_matches_merged() {
        let text = "AABBCC";
        let m1 = make_match(0, 2, "a");
        let m2 = make_match(2, 4, "b"); // starts where m1 ends
        let m3 = make_match(4, 6, "c");
        let result = redact(text, &[m1, m2, m3]);
        assert_eq!(result, "***REDACTED***");
    }

    #[test]
    fn full_text_match() {
        let text = "FULLSECRET";
        let m = make_match(0, 10, "test");
        assert_eq!(redact(text, &[m]), "***REDACTED***");
    }

    #[test]
    fn match_bounds_mid_codepoint_do_not_panic() {
        // "héllo" — 'é' is 2 bytes (0xC3 0xA9) occupying byte indices 1..3.
        // A range of [1, 2) lands *inside* the 'é' codepoint on both ends.
        // Naive `text[1..2]` slicing would panic; snapping to char
        // boundaries must instead redact the whole 'é'.
        let text = "héllo";
        let m = make_match(1, 2, "test");
        let result = redact(text, &[m]);
        assert!(result.starts_with('h'));
        assert!(result.contains(super::REDACTED));
        assert!(result.ends_with("llo"));
        // 'é' is fully covered; the raw codepoint must not survive.
        assert!(!result.contains('é'));
    }

    #[test]
    fn multibyte_match_redacts_full_token() {
        // Emoji + multibyte text around a secret-looking span.
        let text = "🔑clé=hunter2🚀";
        // Redact the "hunter2" run; offsets computed from the byte layout.
        let start = text.find("hunter2").unwrap();
        let end = start + "hunter2".len();
        let m = make_match(start, end, "test");
        let result = redact(text, &[m]);
        assert!(!result.contains("hunter2"));
        assert!(result.contains("🔑clé="));
        assert!(result.contains('🚀'));
        assert!(result.contains(super::REDACTED));
    }

    #[test]
    fn integration_with_detector() {
        use super::super::detector::SensitiveDetector;
        let text = "Access key: AKIAIOSFODNN7EXAMPLE and then normal text";
        let d = SensitiveDetector::new();
        let matches = d.detect(text);
        let redacted = redact(text, &matches);
        assert!(!redacted.contains("AKIAIOSFODNN7EXAMPLE"));
        assert!(redacted.contains("***REDACTED***"));
        assert!(redacted.contains("normal text"));
    }

    #[test]
    fn integration_password_kv_redacted() {
        use super::super::detector::SensitiveDetector;
        let text = "DB settings: password=hunter2 host=localhost";
        let d = SensitiveDetector::new();
        let matches = d.detect(text);
        let redacted = redact(text, &matches);
        assert!(!redacted.contains("hunter2"));
        assert!(redacted.contains("***REDACTED***"));
    }
}
