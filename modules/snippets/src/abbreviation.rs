//! Abbreviation normalization and validation.
//!
//! An abbreviation is stored as entered and compared in its NFKC case-folded
//! form, so "Sig" and "ｓｉｇ" collide while "sig" and "sig2" do not.

use icu_casemap::CaseMapperBorrowed;
use icu_normalizer::ComposingNormalizerBorrowed;
use unicode_segmentation::UnicodeSegmentation;

pub const MAX_GRAPHEMES: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbbreviationError {
    Empty,
    TooLong,
    /// Whitespace or a control character.
    Separator,
}

pub fn validate(abbreviation: &str) -> Result<(), AbbreviationError> {
    if abbreviation.is_empty() {
        return Err(AbbreviationError::Empty);
    }
    if abbreviation
        .chars()
        .any(|ch| ch.is_whitespace() || ch.is_control())
    {
        return Err(AbbreviationError::Separator);
    }
    if abbreviation.graphemes(true).nth(MAX_GRAPHEMES).is_some() {
        return Err(AbbreviationError::TooLong);
    }
    Ok(())
}

/// The matching form. Folding can leave text unnormalized, so NFKC runs on
/// both sides of it.
pub fn normalize(text: &str) -> String {
    let nfkc = ComposingNormalizerBorrowed::new_nfkc();
    let normalized = nfkc.normalize(text);
    let folded = CaseMapperBorrowed::new().fold_string(&normalized);
    nfkc.normalize(&folded).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compatibility_and_case_variants_share_one_key() {
        assert_eq!(normalize("Sig"), normalize("sig"));
        assert_eq!(normalize("ｓｉｇ"), "sig");
        assert_eq!(normalize("STRASSE"), normalize("straße"));
        assert_ne!(normalize("sig"), normalize("sig2"));
    }

    #[test]
    fn length_counts_grapheme_clusters_and_separators_are_refused() {
        assert_eq!(validate("👨‍👩‍👧‍👦".repeat(32).as_str()), Ok(()));
        assert_eq!(
            validate("a".repeat(33).as_str()),
            Err(AbbreviationError::TooLong)
        );
        assert_eq!(validate("a b"), Err(AbbreviationError::Separator));
        assert_eq!(validate("a\u{7}"), Err(AbbreviationError::Separator));
        assert_eq!(validate(""), Err(AbbreviationError::Empty));
    }
}
