use std::sync::OnceLock;

/// Validate a credit card number using the Luhn algorithm.
pub fn luhn_valid(s: &str) -> bool {
    let digits: Vec<u32> = s
        .chars()
        .filter(|c| c.is_ascii_digit())
        // Audit LOW #7: `to_digit(10).unwrap()` is structurally safe (filter
        // only admits ASCII digits) but `unwrap_or(0)` removes the bare
        // unwrap from a security-relevant path. Cannot fire in practice.
        .map(|c| c.to_digit(10).unwrap_or(0))
        .collect();
    if digits.len() < 13 || digits.len() > 19 {
        return false;
    }
    let sum: u32 = digits
        .iter()
        .rev()
        .enumerate()
        .map(|(i, &d)| {
            if i % 2 == 1 {
                let v = d * 2;
                if v > 9 {
                    v - 9
                } else {
                    v
                }
            } else {
                d
            }
        })
        .sum();
    sum.is_multiple_of(10)
}

/// Strip whitespace and `-`, then Luhn-validate. Mirrors the public
/// `luhn_valid` helper but inlined here to avoid an extra
/// allocation+digit-filter pass on the per-candidate hot path.
pub(super) fn luhn_valid_strict(s: &str) -> bool {
    let digits: Vec<u32> = s
        .chars()
        .filter(|c| c.is_ascii_digit())
        // Audit LOW #7: `.to_digit(10).unwrap()` is safe in this branch
        // (the preceding filter only admits ASCII digits) but `unwrap_or(0)`
        // removes the smell entirely. A `0` could only appear if an
        // ASCII-digit char somehow rejected base-10 decode, which is
        // impossible — the `0` is a safety net, not an active value.
        .map(|c| c.to_digit(10).unwrap_or(0))
        .collect();
    if digits.len() < 13 || digits.len() > 19 {
        return false;
    }
    let sum: u32 = digits
        .iter()
        .rev()
        .enumerate()
        .map(|(i, &d)| {
            if i % 2 == 1 {
                let v = d * 2;
                if v > 9 {
                    v - 9
                } else {
                    v
                }
            } else {
                d
            }
        })
        .sum();
    sum.is_multiple_of(10)
}

/// Returns true iff the input contains at least one candidate digit run
/// (13–19 ASCII digits, optionally separated by single `-` or whitespace)
/// that Luhn-validates as a credit-card number.
///
/// Uses a static `OnceLock<Regex>` so the candidate scanner is compiled once
/// per process. The pattern is anchored on word boundaries to skip mid-token
/// hits like `xid=4111111111111111foobar`.
pub(super) fn contains_luhn_valid_card_run(text: &str) -> bool {
    static CARD_RUN_RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = CARD_RUN_RE.get_or_init(|| {
        // `\b(?:\d[\s-]?){13,19}\d\b` — between 13 and 19 digits with
        // optional single space or hyphen between each, plus a final
        // digit (so total = 14..=20 digits). The leading run already
        // matches one digit so we accept totals 13..=19 effectively;
        // the explicit Luhn `digits.len() < 13 || > 19` clamp filters.
        //
        // Graceful fallback: if the regex crate ever rejects this pattern
        // (e.g. after a semver bump changes syntax), degrade to a never-match
        // regex rather than panicking on the first clipboard capture.
        regex::Regex::new(r"\b(?:\d[\s-]?){12,18}\d\b")
            // `[^\s\S]` is the canonical never-match regex for the `regex` crate:
            // it requires a character that is neither whitespace nor non-whitespace,
            // which is impossible. Lookahead (`(?!x)x`) is not supported by `regex`.
            .unwrap_or_else(|_| regex::Regex::new(r"[^\s\S]").expect("never-match regex is valid"))
    });
    for m in re.find_iter(text) {
        if luhn_valid_strict(m.as_str()) {
            return true;
        }
    }
    false
}
