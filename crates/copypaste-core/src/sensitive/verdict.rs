//! The aggregate rule: what one item's findings license.
//!
//! [`super::engine`] owns the compiled rules and the spans they produce; this
//! owns the decision taken over those spans, which is the gate between
//! detection and destruction (manifest I2, §4.2).

use super::finding::{Severity, SpannedFinding};

/// True when `findings` license **deleting** the item.
///
/// `findings` is every validated match at [`Severity::Restricted`] or worse,
/// ordered by `start` ascending.
///
/// An anchor-only match licenses deletion only where no restricted match covers
/// the same bytes. As a plain disjunction the band was inert: `generic_api_key`
/// judged the *same value* through the *same* anchor on weaker gates and
/// reinstated every deletion it had refused (DMY-162). A rule that matched a
/// unique literal is separate evidence at any offset and still deletes.
pub(super) fn deletes(findings: &[SpannedFinding], anchor_only: &[&'static str]) -> bool {
    let restricted = RestrictedSpans::from_ordered(findings);
    findings
        .iter()
        .filter(|finding| finding.severity >= Severity::HighConfidence)
        .any(|high| !anchor_only.contains(&high.rule) || !restricted.covers(high))
}

/// The restricted spans in `start` order, each carrying the running maximum of
/// `end` over everything at or before it.
///
/// Asking the question by scanning the findings again is quadratic in them, and
/// a `.env`-shaped paste — the input this rule exists for — is exactly the one
/// where neither loop can leave early: 4 MiB of `CLOUDFLARE_API_TOKEN=` lines
/// cost 6.0 s against `scan_all`'s 1.3 s, and `sweep_sensitive` pays it once per
/// expired row on every pass for the life of the item (DMY-162).
struct RestrictedSpans {
    starts: Vec<usize>,
    max_end: Vec<usize>,
}

impl RestrictedSpans {
    /// `findings` must be ordered by `start` ascending, which
    /// [`super::engine::Detector::scan_all`] guarantees: `max_end` is a prefix
    /// maximum over that order and means nothing without it.
    fn from_ordered(findings: &[SpannedFinding]) -> Self {
        let mut starts = Vec::new();
        let mut max_end = Vec::new();
        let mut running = 0;
        for finding in findings
            .iter()
            .filter(|finding| finding.severity == Severity::Restricted)
        {
            running = running.max(finding.end);
            starts.push(finding.start);
            max_end.push(running);
        }
        Self { starts, max_end }
    }

    /// Whether some restricted span shares a byte with `span`.
    ///
    /// The same `restricted.start < span.end && span.start < restricted.end` the
    /// naive form asks: the binary search takes the prefix satisfying the first
    /// half, and its running maximum answers the second for all of it at once.
    fn covers(&self, span: &SpannedFinding) -> bool {
        let prefix = self.starts.partition_point(|start| *start < span.end);
        prefix > 0 && self.max_end[prefix - 1] > span.start
    }
}

/// The naive form of [`deletes`], kept as the reference the ordered lookup is
/// checked against. Test-only: production has one implementation.
#[cfg(test)]
pub(in crate::sensitive) fn deletes_naive(
    findings: &[SpannedFinding],
    anchor_only: &[&'static str],
) -> bool {
    findings
        .iter()
        .filter(|finding| finding.severity >= Severity::HighConfidence)
        .any(|high| {
            let high_is_anchor_only = anchor_only.contains(&high.rule);
            !findings
                .iter()
                .any(|other| withholds(other, high, high_is_anchor_only))
        })
}

/// Whether `restricted` refuses deletion of what `high` matched. One judgement
/// twice, and I1 takes the recoverable outcome — but only where `high` rests on
/// the same context anchor and nothing else. Vetoing on overlap alone took
/// auto-wipe from `GITHUB_TOKEN=ghp_…`, `VAULT_TOKEN=hvs.…` and `AUTH_TOKEN=`
/// with a JWT, two of which §9.1 binds as auto-wipes (DMY-162).
///
/// Overlap, not containment: `generic_api_key` consumes the trailing newline its
/// neighbour stops before, so the two spans differ by a byte on the same value.
#[cfg(test)]
pub(in crate::sensitive) fn withholds(
    restricted: &SpannedFinding,
    high: &SpannedFinding,
    high_is_anchor_only: bool,
) -> bool {
    high_is_anchor_only
        && restricted.severity == Severity::Restricted
        && restricted.start < high.end
        && high.start < restricted.end
}

#[cfg(test)]
mod tests {
    use super::*;

    const ANCHOR_ONLY: &[&str] = &["generic_api_key"];

    fn span(severity: Severity, rule: &'static str, start: usize, end: usize) -> SpannedFinding {
        SpannedFinding {
            rule,
            category: "credential",
            confidence: 0.99,
            severity,
            start,
            end,
        }
    }

    fn restricted(start: usize, end: usize) -> SpannedFinding {
        span(Severity::Restricted, "credit_card", start, end)
    }

    fn high(start: usize, end: usize) -> SpannedFinding {
        span(Severity::HighConfidence, "credit_card", start, end)
    }

    /// Overlap, not containment, and not adjacency. `generic_api_key` consumes
    /// the trailing newline `cloudflare_api_token` stops before, so the two
    /// spans over one value differ by a byte; a containment test would miss it
    /// and delete. Two matches that merely touch are separate values.
    #[test]
    fn withholding_takes_any_shared_byte_and_no_fewer() {
        assert!(withholds(&restricted(0, 61), &high(0, 61), true));
        assert!(withholds(&restricted(0, 61), &high(0, 62), true));
        assert!(withholds(&restricted(48, 147), &high(48, 148), true));
        assert!(withholds(&restricted(10, 20), &high(0, 61), true));
        assert!(!withholds(&restricted(0, 61), &high(61, 100), true));
        assert!(!withholds(&restricted(61, 100), &high(0, 61), true));
        // Only the restricted band withholds; two deletable rules over one span
        // are still deletable.
        assert!(!withholds(&high(0, 61), &high(0, 61), true));
        // And overlap is no longer sufficient: a rule that matched a unique
        // literal is separate evidence over the very same bytes.
        assert!(!withholds(&restricted(0, 61), &high(0, 61), false));
    }

    /// The ordered lookup answers what the nested scan answers, on the shape
    /// that made the nested scan quadratic: thousands of `.env` lines where
    /// every high match is anchor-only and every one of them is vetoed, so
    /// neither loop can leave early.
    ///
    /// Synthetic spans rather than a scanned document, so the corpus can be
    /// large enough to matter without a regex bill, and deterministic — a
    /// wall-clock assertion here would be flaky on a shared runner.
    #[test]
    fn the_ordered_lookup_answers_what_the_nested_scan_answers() {
        let mut findings = Vec::new();
        for line in 0..4_000 {
            let start = line * 62;
            findings.push(restricted(start, start + 61));
            findings.push(span(
                Severity::HighConfidence,
                "generic_api_key",
                start,
                start + 62,
            ));
        }
        assert!(!deletes(&findings, ANCHOR_ONLY));
        assert_eq!(
            deletes(&findings, ANCHOR_ONLY),
            deletes_naive(&findings, ANCHOR_ONLY)
        );

        // One unvetoed line anywhere flips the verdict, and the two forms must
        // flip together wherever it is: first, last and in the middle.
        for line in [0, 2_000, 3_999] {
            let mut corpus = findings.clone();
            corpus.remove(line * 2);
            assert!(deletes(&corpus, ANCHOR_ONLY), "line {line}");
            assert_eq!(
                deletes(&corpus, ANCHOR_ONLY),
                deletes_naive(&corpus, ANCHOR_ONLY)
            );
        }
    }

    /// The boundaries the prefix maximum has to preserve, which a per-item scan
    /// got for free: a restricted span that starts later than a high one can
    /// still cover it, and one that starts earlier can end before it begins.
    #[test]
    fn the_ordered_lookup_agrees_on_every_span_arrangement() {
        let arrangements: &[&[SpannedFinding]] = &[
            &[],
            &[high(0, 10)],
            &[restricted(0, 10)],
            // Disjoint: independent evidence, so the item still deletes.
            &[
                restricted(0, 10),
                span(Severity::HighConfidence, "generic_api_key", 20, 30),
            ],
            // Touching, not overlapping.
            &[
                restricted(0, 10),
                span(Severity::HighConfidence, "generic_api_key", 10, 20),
            ],
            // A long restricted span before a short high one inside it.
            &[
                restricted(0, 100),
                span(Severity::HighConfidence, "generic_api_key", 40, 50),
            ],
            // The covering restricted span starts *after* an earlier, shorter
            // one that does not reach the high match: the prefix maximum, not
            // the last element, is what answers this.
            &[
                restricted(0, 5),
                restricted(10, 100),
                span(Severity::HighConfidence, "generic_api_key", 90, 95),
            ],
            // A high match with a unique literal of its own is never vetoed.
            &[restricted(0, 100), high(40, 50)],
        ];
        for findings in arrangements {
            assert_eq!(
                deletes(findings, ANCHOR_ONLY),
                deletes_naive(findings, ANCHOR_ONLY),
                "{findings:?}"
            );
        }
    }
}
