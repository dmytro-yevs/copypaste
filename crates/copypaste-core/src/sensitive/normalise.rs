//! Normalisation (manifest I3 / §5.1).
//!
//! Small on purpose, and separate on purpose: this runs once per scan, before
//! any rule sees the text, and it is the only place in the module that can move
//! a byte offset. Anything that ever returns spans has to read exactly this
//! file to know what those offsets index into.

use std::borrow::Cow;

use unicode_normalization::UnicodeNormalization;

/// NFKC-normalise before matching.
///
/// Without this, `Ａ` (U+FF21 FULLWIDTH LATIN CAPITAL A) and every other
/// compatibility form renders as ASCII but bypasses every ASCII character
/// class. All offsets, if this module ever returns any, belong to the
/// normalised string.
///
/// NFKC is the identity on ASCII (§5.1), so ASCII input — the overwhelming
/// majority of clipboard traffic — skips the pass and the allocation entirely.
pub(super) fn normalise(text: &str) -> Cow<'_, str> {
    if text.is_ascii() {
        Cow::Borrowed(text)
    } else {
        Cow::Owned(text.nfkc().collect())
    }
}

// ---------------------------------------------------------------------------
// Tests — §5.1 / I3
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use regex::Regex;

    use super::*;
    use crate::sensitive::engine::test_support::{detector, BENIGN_CORPUS};
    use crate::sensitive::rules::rule;

    /// The bypass NFKC closes: full-width Latin renders as ASCII but matches no
    /// ASCII character class.
    #[test]
    fn nfkc_normalised_input_detects_secrets() {
        let det = detector();
        let fullwidth = "\u{FF21}\u{FF2B}\u{FF29}\u{FF21}IOSFODNN7EXAMPLE";
        assert_ne!(fullwidth, "AKIAIOSFODNN7EXAMPLE");
        // without normalisation the raw bytes match nothing
        assert!(!Regex::new(rule("aws_access_key").pattern)
            .unwrap()
            .is_match(fullwidth));
        // with it, the same key is found
        assert!(det.is_sensitive(fullwidth));
        assert_eq!(det.scan(fullwidth).unwrap().rule, "aws_access_key");
    }

    #[test]
    fn nfkc_bypass_closed_for_full_width_digits_and_cards() {
        let det = detector();
        // 4111111111111111 written with FULLWIDTH DIGIT ZERO..NINE
        let fullwidth: String = "4111111111111111"
            .chars()
            .map(|c| char::from_u32(u32::from(c) - u32::from('0') + 0xFF10).unwrap())
            .collect();
        assert_ne!(fullwidth, "4111111111111111");
        assert_eq!(det.scan(&fullwidth).unwrap().rule, "credit_card");
    }

    /// §5.1: NFKC is the identity on ASCII — which is what lets callers index
    /// spans into the original string, and what makes the fast path sound.
    #[test]
    fn normalise_is_the_identity_on_ascii() {
        assert!(matches!(
            normalise("AKIAIOSFODNN7EXAMPLE"),
            Cow::Borrowed(_)
        ));
        assert_eq!(normalise("AKIAIOSFODNN7EXAMPLE"), "AKIAIOSFODNN7EXAMPLE");
        for text in BENIGN_CORPUS {
            assert_eq!(normalise(text), *text);
        }
    }

    /// §7.8: v1's `nfkc_zwj_in_jwt_normalises_away` asserted nothing about ZWJ
    /// — its body tested a clean ASCII JWT. This is the real test. NFKC does
    /// **not** strip default-ignorable code points, so a ZWJ spliced into a
    /// token still defeats the regex. The bypass is open, by decision:
    /// stripping default-ignorables before matching would change every offset
    /// this module might ever return, and the failure direction is a false
    /// negative (I1's preferred direction), not data loss.
    #[test]
    fn zwj_bypass_is_documented_and_still_open() {
        let det = detector();
        let spliced = "AKIA\u{200D}IOSFODNN7EXAMPLE";
        assert_eq!(normalise(spliced), spliced, "NFKC keeps ZWJ");
        assert!(!det.is_sensitive(spliced), "known gap, tracked in §7.8");
    }
}
