use std::ops::Range;

use super::SpannedFinding;

const REDACTION: &str = "***REDACTED***";

/// Replace spans in an NFKC-normalised string. Bounds snap outward to UTF-8
/// character boundaries, and overlapping or adjacent spans share one marker
/// (manifest I9).
pub fn redact_spans(normalised: &str, spans: &[Range<usize>]) -> String {
    let mut snapped: Vec<_> = spans
        .iter()
        .filter_map(|span| snap(normalised, span.clone()))
        .collect();
    if snapped.is_empty() {
        return normalised.to_owned();
    }

    snapped.sort_unstable_by_key(|span| (span.start, span.end));
    let mut merged: Vec<Range<usize>> = Vec::with_capacity(snapped.len());
    for span in snapped {
        if let Some(last) = merged.last_mut().filter(|last| span.start <= last.end) {
            last.end = last.end.max(span.end);
        } else {
            merged.push(span);
        }
    }

    let removed: usize = merged.iter().map(|span| span.end - span.start).sum();
    let mut redacted = String::with_capacity(
        normalised.len() - removed + merged.len().saturating_mul(REDACTION.len()),
    );
    let mut cursor = 0;
    for span in merged {
        redacted.push_str(&normalised[cursor..span.start]);
        redacted.push_str(REDACTION);
        cursor = span.end;
    }
    redacted.push_str(&normalised[cursor..]);
    redacted
}

pub(super) fn redact_findings(normalised: &str, findings: &[SpannedFinding]) -> String {
    let spans: Vec<_> = findings.iter().map(SpannedFinding::byte_range).collect();
    redact_spans(normalised, &spans)
}

fn snap(text: &str, span: Range<usize>) -> Option<Range<usize>> {
    if span.start == span.end {
        return None;
    }
    let (mut start, mut end) = if span.start < span.end {
        (span.start.min(text.len()), span.end.min(text.len()))
    } else {
        (span.end.min(text.len()), span.start.min(text.len()))
    };
    while start > 0 && !text.is_char_boundary(start) {
        start -= 1;
    }
    while end < text.len() && !text.is_char_boundary(end) {
        end += 1;
    }
    (start < end).then_some(start..end)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sensitive::Detector;

    #[test]
    fn no_spans_leave_input_unchanged() {
        assert_eq!(redact_spans("ordinary text", &[]), "ordinary text");
    }

    #[test]
    fn start_middle_end_and_disjoint_spans_preserve_surroundings() {
        let at_start = 0..6;
        let in_middle = 2..8;
        let at_end = 5..11;
        assert_eq!(
            redact_spans("secret tail", std::slice::from_ref(&at_start)),
            "***REDACTED*** tail"
        );
        assert_eq!(
            redact_spans("a secret z", std::slice::from_ref(&in_middle)),
            "a ***REDACTED*** z"
        );
        assert_eq!(
            redact_spans("head secret", std::slice::from_ref(&at_end)),
            "head ***REDACTED***"
        );
        assert_eq!(
            redact_spans("aaXXbbYYcc", &[2..4, 6..8]),
            "aa***REDACTED***bb***REDACTED***cc"
        );
    }

    #[test]
    fn overlap_and_adjacency_merge_to_one_marker() {
        assert_eq!(redact_spans("abcdef", &[0..4, 2..6]), "***REDACTED***");
        assert_eq!(
            redact_spans("abcdef", &[0..2, 2..4, 4..6]),
            "***REDACTED***"
        );
    }

    #[test]
    fn mid_codepoint_bounds_snap_outward() {
        let mid_codepoint = 1..2;
        assert_eq!(
            redact_spans("héllo", std::slice::from_ref(&mid_codepoint)),
            "h***REDACTED***llo"
        );
    }

    #[test]
    fn emoji_and_multibyte_surroundings_stay_intact() {
        let text = "🔑clé=hunter2🚀";
        let start = text.find("hunter2").unwrap();
        let secret = start..start + "hunter2".len();
        assert_eq!(
            redact_spans(text, std::slice::from_ref(&secret)),
            "🔑clé=***REDACTED***🚀"
        );
    }

    #[test]
    fn detector_redacts_embedded_and_keyword_secrets() {
        let detector = Detector::new().unwrap();
        let redacted = detector.redact("normal text AKIAIOSFODNN7EXAMPLE survives");
        assert_eq!(redacted, "normal text ***REDACTED*** survives");

        let redacted = detector.redact("prefix password=hunter2 suffix");
        assert!(!redacted.contains("hunter2"));
        assert!(redacted.contains("prefix "));
        assert!(redacted.contains(" suffix"));

        let redacted = detector.redact("Send to alice@example.com");
        assert_eq!(redacted, "Send to ***REDACTED***");
    }
}
