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
    /// Describe, for a client about to render a preview, what inert matches that
    /// preview contains. A credential in it withholds the whole result.
    ///
    /// **Scoped to the preview.** Ranking and redacting the whole text and
    /// letting the caller truncate to `LIST_PREVIEW_BYTES` ran every rule over
    /// hundreds of megabytes per page to describe a kilobyte of it.
    ///
    /// Re-asking the *whole-item* question here would keep that cost and buy
    /// nothing: capture answers it, [`super::purge_indexed_secrets`] corrects it
    /// for a changed ruleset, and the unredacted `content` this travels beside is
    /// withheld by neither. A match past the boundary changes the label, not what
    /// the response discloses.
    pub fn inert_finding_metadata(&self, text: &str) -> Option<SensitiveFinding> {
        // Slice before copying, or a 4 MiB item still costs a 4 MiB clone to
        // describe a kilobyte. The slack lets `bound_preview` make the real cut
        // on a grapheme boundary rather than this one on a codepoint boundary.
        let mut end = text
            .len()
            .min(2 * copypaste_ipc::limits::LIST_PREVIEW_BYTES);
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        let mut preview = text[..end].to_owned();
        bound_preview(&mut preview);
        let normalised = normalise(&preview);
        let findings = self.scan_all_normalised(&normalised);
        // Withheld, not partially described: a preview carrying a live credential
        // must not come back labelled by the inert email beside it.
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
        // Redaction can only lengthen, so the bound is reapplied after it.
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

    /// The span cap, not the preview bound: the addresses are short enough that
    /// far more than `MAX_SURFACED_SENSITIVE_SPANS` of them fit inside the
    /// prefix, so the cap is the only thing that can be doing the capping.
    #[test]
    fn attacker_controlled_match_counts_are_capped_without_leaking_the_rest() {
        let detector = Detector::new().unwrap();
        let text = std::iter::repeat_n("a@b.co", 400)
            .collect::<Vec<_>>()
            .join(" ");
        assert!(text.len() > copypaste_ipc::limits::LIST_PREVIEW_BYTES);

        let finding = detector.inert_finding_metadata(&text).unwrap();
        assert_eq!(finding.spans.len(), MAX_SURFACED_SENSITIVE_SPANS);
        assert!(finding.spans_truncated);
        assert!(!finding.redacted_preview.contains("a@b.co"));
        assert!(finding.redacted_preview.len() <= copypaste_ipc::limits::LIST_PREVIEW_BYTES);
    }

    #[test]
    fn high_confidence_matches_never_get_partial_preview_metadata() {
        let detector = Detector::new().unwrap();
        assert!(detector
            .inert_finding_metadata("prefix AKIAIOSFODNN7EXAMPLE suffix")
            .is_none());
    }

    /// A credential *inside* the preview withholds the whole result, so the
    /// label can never describe a preview by the inert match beside a live one.
    /// One past the boundary is not withheld and does not need to be: the
    /// unredacted `content` in the same response carries it either way, and the
    /// item-level answer is the capture flag plus the startup rescan.
    #[test]
    fn a_credential_in_the_preview_withholds_but_the_boundary_is_the_preview() {
        let detector = Detector::new().unwrap();
        let filler = "alice@example.com ".repeat(copypaste_ipc::limits::LIST_PREVIEW_BYTES / 4);
        assert!(filler.len() > copypaste_ipc::limits::LIST_PREVIEW_BYTES);

        assert!(detector
            .inert_finding_metadata(&format!("AKIAIOSFODNN7EXAMPLE {filler}"))
            .is_none());
        let past = detector
            .inert_finding_metadata(&format!("{filler} AKIAIOSFODNN7EXAMPLE"))
            .expect("described by its preview");
        assert_eq!(past.label, "email");
        assert!(!past.redacted_preview.contains("AKIA"));
    }

    /// The amplification DMY-162 names: the description is of the preview, so a
    /// 4 MiB item must not cost a ranked scan and a redaction of 4 MiB to
    /// produce a kilobyte of it. Asserted as a ratio against the same content at
    /// preview size, because an absolute time is not a property of the code.
    #[test]
    fn describing_a_large_item_costs_what_describing_its_preview_costs() {
        let detector = Detector::new().unwrap();
        let unit = "mail alice@example.com or bob@example.org, host 10.0.0.1:5432\n";
        let small = unit.repeat(copypaste_ipc::limits::LIST_PREVIEW_BYTES / unit.len() + 1);
        let large = unit.repeat(4 * 1024 * 1024 / unit.len() + 1);
        assert!(large.len() >= 4 * 1024 * 1024);

        let time = |text: &str| {
            let started = std::time::Instant::now();
            for _ in 0..20 {
                assert!(detector.inert_finding_metadata(text).is_some());
            }
            started.elapsed()
        };
        // Warm the lazy DFA cache so the first call is not the one compared.
        time(&small);
        let small_cost = time(&small);
        let large_cost = time(&large);

        eprintln!("preview {small_cost:?} vs 4 MiB {large_cost:?} over 20 runs");
        assert!(
            large_cost < small_cost * 8,
            "describing 4 MiB cost {large_cost:?} against {small_cost:?} for its preview"
        );
    }
}
