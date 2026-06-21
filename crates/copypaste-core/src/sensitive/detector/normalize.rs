use unicode_normalization::UnicodeNormalization;

/// NFKC-normalise input so Unicode bypass tricks (full-width AKIA, ZWJ in JWTs,
/// compatibility ligatures) collapse to their ASCII canonical form before regex matching.
///
/// Matched byte ranges are therefore valid against the *normalised* string, not the
/// caller-supplied original. Callers that need ranges against the original (e.g. the
/// `redact` helper) should redact against the same normalised string returned by
/// `nfkc_normalize`.
pub fn nfkc_normalize(text: &str) -> String {
    text.nfkc().collect()
}
