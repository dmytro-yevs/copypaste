//! Normalisation (manifest I3 / §5.1). Runs once per scan, before any rule
//! sees the text, and is the only place in the module that can move a byte
//! offset — anything that ever returns spans indexes into its output.

use std::borrow::Cow;
use std::sync::OnceLock;

use regex::Regex;
use unicode_normalization::UnicodeNormalization;

/// NFKC-normalise before matching, then strip default-ignorable code points.
/// Without NFKC, `Ａ` (U+FF21 FULLWIDTH LATIN CAPITAL A) renders as ASCII but
/// bypasses every ASCII character class. Without stripping ignorables, a ZWJ
/// spliced into a token defeats every regex (§7.8). ASCII has neither, so it
/// skips the pass and the allocation entirely.
pub(super) fn normalise(text: &str) -> Cow<'_, str> {
    if text.is_ascii() {
        Cow::Borrowed(text)
    } else {
        let nfkc: String = text.nfkc().collect();
        match strip_default_ignorables(&nfkc) {
            Cow::Borrowed(_) => Cow::Owned(nfkc),
            Cow::Owned(stripped) => Cow::Owned(stripped),
        }
    }
}

fn strip_default_ignorables(text: &str) -> Cow<'_, str> {
    default_ignorable_re().replace_all(text, "")
}

fn default_ignorable_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"\p{Default_Ignorable_Code_Point}+")
            .expect("Default_Ignorable_Code_Point is a supported regex property")
    })
}

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

    #[test]
    fn default_ignorables_are_stripped_so_spliced_secrets_are_detected() {
        let det = detector();
        let spliced = "AKIA\u{200D}IOSFODNN7EXAMPLE";
        assert_eq!(normalise(spliced), "AKIAIOSFODNN7EXAMPLE");
        assert_eq!(det.scan(spliced).unwrap().rule, "aws_access_key");
        assert!(det.is_sensitive(spliced));
        assert!(det.may_auto_wipe(spliced));
    }

    #[test]
    fn other_default_ignorables_cannot_splice_a_secret() {
        let det = detector();
        for splice in ['\u{200B}', '\u{200C}', '\u{2060}', '\u{FEFF}', '\u{FE0F}'] {
            let text = format!("AKIA{splice}IOSFODNN7EXAMPLE");
            assert_eq!(normalise(&text), "AKIAIOSFODNN7EXAMPLE", "{splice:?}");
            assert!(det.is_sensitive(&text), "{splice:?}");
        }
    }

    #[test]
    fn stripping_ignorables_does_not_make_inert_pii_wipeable() {
        let det = detector();
        let email = "please email alice\u{200D}.smith@example.com about it";
        let card = "Customer card: 4111\u{200D}111111111111 — expires 12/26";

        assert!(!det.scan_all(email).is_empty());
        assert!(!det.is_sensitive(email));
        assert!(!det.may_auto_wipe(email));

        assert_eq!(det.scan(card).unwrap().rule, "credit_card");
        assert!(det.is_sensitive(card));
        assert!(!det.may_auto_wipe(card));
    }
}
