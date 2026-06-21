use super::super::patterns::{pattern_name, patterns};
use std::ops::Range;

/// Returns true if a `generic_password_kv` match value is strong enough to be a real
/// credential, suppressing benign prose like `password: foo` or `// api_key=demo`.
///
/// Strong = any one of:
///   - value length ≥ 10 characters (Unicode scalar values, not bytes)
///   - contains a special char `[!@#$%^&*+/=]`
///   - mix of letter AND digit
pub(super) fn is_credential_value_strong(value: &str) -> bool {
    // Count chars, not bytes: a multibyte secret (e.g. CJK/accented) must be
    // gated on its character length, otherwise a short multibyte value would
    // be over-counted by `.len()` (byte length) and mis-classified as strong.
    if value.chars().count() >= 10 {
        return true;
    }
    let mut has_letter = false;
    let mut has_digit = false;
    let mut has_special = false;
    for b in value.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' => has_letter = true,
            b'0'..=b'9' => has_digit = true,
            b'!' | b'@' | b'#' | b'$' | b'%' | b'^' | b'&' | b'*' | b'+' | b'/' | b'=' => {
                has_special = true
            }
            _ => {}
        }
    }
    has_special || (has_letter && has_digit)
}

/// Returns true when a given pattern index produced a match that should be discarded
/// (e.g. a `generic_password_kv` match whose captured value is too weak to be a secret).
///
/// `generic_bearer` FP suppression is handled via confidence lowering (0.65, below the
/// 0.70 auto-wipe floor) rather than a post-match filter, because the pattern's 20-char
/// minimum already ensures any matched bearer value passes `is_credential_value_strong`
/// (≥10 chars → true), so a filter here would be a no-op.
pub(super) fn match_is_false_positive(
    pattern_idx: usize,
    full_match: &str,
    text: &str,
    range: &Range<usize>,
) -> bool {
    if pattern_name(pattern_idx) != "generic_password_kv" {
        return false;
    }
    // Re-run with captures to extract the value group from the same byte range.
    // Cheap: same pattern, restricted to the matched slice.
    let re = &patterns()[pattern_idx];
    if let Some(caps) = re.captures(&text[range.clone()]) {
        if let Some(v) = caps.get(1) {
            return !is_credential_value_strong(v.as_str());
        }
    }
    // Fallback: validate the whole match if we couldn't pull the capture.
    !is_credential_value_strong(full_match)
}
