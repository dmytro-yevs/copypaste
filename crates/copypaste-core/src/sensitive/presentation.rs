use copypaste_ipc::limits::bound_preview;
use copypaste_ipc::{SensitiveFinding, SensitiveSpan};

use super::engine::compare_rank;
use super::normalise::normalise;
use super::redact::redact_findings;
use super::{Detector, Severity};

/// A response must stay bounded even when clipboard text produces thousands
/// of validated matches. The preview still redacts every match.
pub const MAX_SURFACED_SENSITIVE_SPANS: usize = 64;

impl Detector {
    /// Convert inert detector matches into the additive client DTO. Any
    /// high-confidence match withholds the whole result.
    pub fn inert_finding_metadata(&self, text: &str) -> Option<SensitiveFinding> {
        let normalised = normalise(text);
        let findings = self.scan_all_normalised(&normalised);
        // Anything the whole-item gate would withhold takes the whole result
        // with it, deletable or not: partial metadata about a withheld item is
        // still a description of it.
        if findings.iter().any(|f| f.severity > Severity::Flag) {
            return None;
        }
        let label = findings
            .iter()
            .max_by(|a, b| compare_rank(a, b))?
            .rule
            .to_owned();
        let mut spans_truncated = findings.len() > MAX_SURFACED_SENSITIVE_SPANS;
        let spans = findings
            .iter()
            .take(MAX_SURFACED_SENSITIVE_SPANS)
            .filter_map(
                |finding| match (finding.start.try_into(), finding.end.try_into()) {
                    (Ok(start), Ok(end)) => Some(SensitiveSpan { start, end }),
                    _ => {
                        spans_truncated = true;
                        None
                    }
                },
            )
            .collect();
        let mut redacted_preview = redact_findings(&normalised, &findings);
        bound_preview(&mut redacted_preview);
        if redacted_preview.len() > copypaste_ipc::limits::LIST_PREVIEW_BYTES {
            let mut end = copypaste_ipc::limits::LIST_PREVIEW_BYTES;
            while !redacted_preview.is_char_boundary(end) {
                end -= 1;
            }
            redacted_preview.truncate(end);
        }
        Some(SensitiveFinding {
            label,
            spans,
            spans_truncated,
            redacted_preview,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inert_matches_convert_to_a_stable_bounded_redacted_dto() {
        let detector = Detector::new().unwrap();
        let text = "Email alice@example.com or bob@example.com";
        assert!(!detector.is_sensitive(text));

        let finding = detector.inert_finding_metadata(text).unwrap();
        assert_eq!(finding.label, "email");
        assert_eq!(finding.spans.len(), 2);
        assert!(!finding.spans_truncated);
        assert_eq!(
            finding.redacted_preview,
            "Email ***REDACTED*** or ***REDACTED***"
        );
    }

    #[test]
    fn attacker_controlled_match_counts_are_capped_without_leaking_the_rest() {
        let detector = Detector::new().unwrap();
        let text = std::iter::repeat_n("alice@example.com", MAX_SURFACED_SENSITIVE_SPANS + 10)
            .collect::<Vec<_>>()
            .join(" ");

        let finding = detector.inert_finding_metadata(&text).unwrap();
        assert_eq!(finding.spans.len(), MAX_SURFACED_SENSITIVE_SPANS);
        assert!(finding.spans_truncated);
        assert!(!finding.redacted_preview.contains("@example.com"));
        assert!(finding.redacted_preview.len() <= copypaste_ipc::limits::LIST_PREVIEW_BYTES);
    }

    #[test]
    fn high_confidence_matches_never_get_partial_preview_metadata() {
        let detector = Detector::new().unwrap();
        assert!(detector
            .inert_finding_metadata("prefix AKIAIOSFODNN7EXAMPLE suffix")
            .is_none());
    }
}
