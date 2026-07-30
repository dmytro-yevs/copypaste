//! The false-positive gates a regex match must survive before it counts.
//!
//! Every function here answers "the shape matched — is it actually a secret?",
//! and every one of them exists because a v1 rule without it deleted user data
//! or missed a real credential. They are pure and total: no allocation-free
//! promises, no fast paths, no second copy of the same algorithm (§7.3).
//!
//! [`super::spec::Validator`] names which of these a rule runs;
//! [`super::engine`] does the dispatch.

/// Characters in the §5.3 "special character" set. Note `$`, `#`, `%`, `/` and
/// `=` are *strength* signals here, which is why the code-shape rejection below
/// cannot simply blacklist punctuation.
const STRENGTH_SPECIALS: &str = "!@#$%^&*+/=";

/// Code punctuation that a machine-issued credential essentially never
/// contains, but that source code and templates are full of. v2 addition —
/// see `value_is_strong`.
const CODE_SHAPED_CHARS: &[char] = &['(', ')', '\'', '"', '`', '<', '>'];

/// Whole-value placeholder stopwords (gitleaks' allowlist model, §5.6 — v1 had
/// *no* allowlist of any kind and called it its biggest structural gap).
///
/// Matched against the **entire** value, case-insensitively, never as a
/// substring: a substring match could suppress a real credential that happens
/// to contain one of these letter sequences.
const VALUE_STOPWORDS: &[&str] = &[
    "changeme",
    "change_me",
    "yourkey",
    "your_key",
    "yourapikey",
    "your_api_key",
    "yoursecret",
    "your_secret",
    "yourpassword",
    "your_password",
    "placeholder",
    "redacted",
    "insert_key_here",
    "notarealsecret",
    "example",
    "todo",
    "tbd",
    "n/a",
    "none",
    "null",
    "nil",
    "undefined",
];

/// Value-strength gate — the manifest's only post-match validator for keyword
/// rules (§5.3), plus the two v2 additions §7.1 and §7.7 ask for.
///
/// The three manifest criteria, verbatim. A value is strong if **any** holds:
///
/// 1. `value.chars().count() >= 10` — **characters, not bytes**;
/// 2. contains one of `! @ # $ % ^ & * + / =`;
/// 3. contains at least one ASCII letter **and** at least one ASCII digit.
///
/// > **The char-vs-byte gate.** A 9-character CJK value (`私的秘密言葉確認鍵`)
/// > is 27 bytes. A byte-length gate would call it strong; the char gate
/// > correctly calls it weak. Pinned by `multibyte_value_gated_on_chars_not_bytes`.
///
/// Two rejections run *before* those criteria:
///
/// * **Code shape.** The benign corpus contains
///   `const password = prompt('enter password:');`. Group 1 captures
///   `prompt('enter` — 13 characters, so criterion 1 calls it strong and v1
///   auto-wiped it. That entry is one of the two FPs v1's 5 % budget silently
///   absorbed (§7.7). A value containing `(`, `)`, `'`, `"`, `` ` ``, `<`, `>`,
///   or opening with `$`/`${`/`{{` is source code or a template reference, not
///   a literal secret. Biasing toward "not a secret" is the direction CLAUDE.md
///   rule 4 and manifest I1 both require.
/// * **Stopwords** (§5.6).
pub(super) fn value_is_strong(value: &str) -> bool {
    if value.starts_with('$') || value.contains("${") || value.contains("{{") {
        return false;
    }
    if value.contains(CODE_SHAPED_CHARS) {
        return false;
    }
    let lowered = value.to_lowercase();
    if VALUE_STOPWORDS.contains(&lowered.as_str()) {
        return false;
    }
    value.chars().count() >= 10
        || value.chars().any(|c| STRENGTH_SPECIALS.contains(c))
        || (value.chars().any(|c| c.is_ascii_alphabetic())
            && value.chars().any(|c| c.is_ascii_digit()))
}

/// Luhn checksum over a candidate digit run. Manifest §5.4.
///
/// **One implementation** (§7.3): v1 shipped `luhn_valid` and
/// `luhn_valid_strict` as byte-for-byte the same algorithm, justified by an
/// allocation saving that did not exist — both allocated, both ran the same
/// digit filter.
///
/// The `13 ..= 19` clamp is load-bearing: it rejects short and long numeric
/// runs outright, independently of the checksum.
pub(super) fn luhn_valid(candidate: &str) -> bool {
    let digits: Vec<u32> = candidate
        .bytes()
        .filter(u8::is_ascii_digit)
        .map(|b| u32::from(b - b'0'))
        .collect();
    if !(13..=19).contains(&digits.len()) {
        return false;
    }
    let sum: u32 = digits
        .iter()
        .rev()
        .enumerate()
        .map(|(i, &d)| {
            if i % 2 == 1 {
                let doubled = d * 2;
                if doubled > 9 {
                    doubled - 9
                } else {
                    doubled
                }
            } else {
                d
            }
        })
        .sum();
    sum.is_multiple_of(10)
}

/// SSN group structure, per §4.2: group 1 in 001–899, group 2 in 01–99,
/// group 3 in 0001–9999, no all-zero group.
///
/// This only trims obvious non-SSNs; the rule stays below the auto-wipe floor
/// regardless, because a *real* SSN is still the user's own data (§4.2).
pub(super) fn ssn_structure_plausible(matched: &str) -> bool {
    let groups: Vec<u32> = matched
        .split(|c: char| !c.is_ascii_digit())
        .filter(|g| !g.is_empty())
        .filter_map(|g| g.parse::<u32>().ok())
        .collect();
    match groups.as_slice() {
        [area, group, serial] => {
            (1..=899).contains(area) && (1..=99).contains(group) && (1..=9999).contains(serial)
        }
        _ => false,
    }
}

/// A phone number must look like one. See the `phone_us` rule comment: the
/// benign corpus entry `the order number is 1234567890` is a bare digit run,
/// and §7.7 requires zero corpus FPs.
pub(super) fn phone_is_formatted(matched: &str) -> bool {
    matched.starts_with('+')
        || matched
            .chars()
            .any(|c| matches!(c, '(' | ')' | '-' | '.') || c.is_whitespace())
}

// ---------------------------------------------------------------------------
// Tests — §5.3 the value-strength gate, §5.4 Luhn, §4.2 SSN, §7.7 phone shape
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sensitive::engine::test_support::{all_rules, detector, fired};
    use crate::sensitive::Severity;

    #[test]
    fn value_strength_follows_the_manifest_criteria() {
        // 1 — ten characters
        assert!(value_is_strong("abcdefghij"));
        assert!(!value_is_strong("abcdefghi"));
        // 2 — a special character
        assert!(value_is_strong("!abcdef"));
        assert!(value_is_strong("ab=cdef"));
        // 3 — letter and digit
        assert!(value_is_strong("hunter2"));
        assert!(!value_is_strong("hunter"));
        assert!(!value_is_strong("123456"));
    }

    /// The char-vs-byte gate. `私的秘密言葉確認鍵` is 9 chars / 27 bytes: a
    /// byte-length gate would wrongly call it strong.
    #[test]
    fn multibyte_value_gated_on_chars_not_bytes() {
        let nine = "私的秘密言葉確認鍵";
        let ten = "私的秘密言葉確認鍵値";
        assert_eq!(nine.len(), 27);
        assert_eq!(nine.chars().count(), 9);
        assert!(!value_is_strong(nine));
        assert!(value_is_strong(ten));

        let det = detector();
        assert!(!det.is_sensitive(&format!("password: {nine}")));
        assert!(det.is_sensitive(&format!("password: {ten}")));
    }

    /// v2 addition (§7.7): code and template shapes are not secrets.
    #[test]
    fn code_shaped_values_are_weak() {
        assert!(!value_is_strong("prompt('enter"));
        assert!(!value_is_strong("getEnv();"));
        assert!(!value_is_strong("${API_KEY}"));
        assert!(!value_is_strong("$MY_SECRET_VAR"));
        assert!(!value_is_strong("{{ vault_password }}"));
        assert!(!value_is_strong("<your-key-here>"));
        // stopwords, matched whole-value only
        assert!(!value_is_strong("changeme"));
        assert!(!value_is_strong("PLACEHOLDER"));
        // …and never as a substring, so a real credential survives
        assert!(value_is_strong("changeme7f3a91bc2d"));
    }

    /// §7.1: v1 ran `dotenv_secret` at 0.80 with no validator, so
    /// `API_KEY=changeme` auto-wiped.
    #[test]
    fn dotenv_secret_is_value_gated() {
        let det = detector();
        for weak in [
            "API_KEY=changeme",
            "DB_PASSWORD=xxx",
            "MY_TOKEN=TODO",
            "APP_KEY=${FOO}",
        ] {
            assert!(
                !det.is_sensitive(weak),
                "{weak:?} -> {:?}",
                all_rules(&det, weak)
            );
        }
        let strong = "export DATADOG_API_KEY=8f14e45fceea167a5a36dedd4bea2543";
        assert!(fired(&det, strong, "dotenv_secret"));
        assert_eq!(det.scan(strong).unwrap().severity, Severity::HighConfidence);
    }

    /// §5.4's test-fixture bug: the original negative fixture `4242424242422`
    /// was *accidentally Luhn-valid* (digit sum 50), so the "must not match"
    /// test passed vacuously. **Assert your negative fixtures are negative.**
    #[test]
    fn negative_card_fixtures_are_actually_negative() {
        assert!(
            luhn_valid("4242424242422"),
            "the old fixture really was valid"
        );
        assert!(
            !luhn_valid("4242424242421"),
            "the replacement must be invalid"
        );
        assert!(luhn_valid("4111111111111111"));
        // the 13..=19 clamp, independent of the checksum
        assert!(!luhn_valid("42424242424"));
        assert!(!luhn_valid("41111111111111111111"));
    }

    #[test]
    fn ssn_structure_rejects_impossible_groups() {
        assert!(ssn_structure_plausible("123-45-6789"));
        assert!(ssn_structure_plausible("012 31 2024"));
        assert!(!ssn_structure_plausible("000-45-6789"));
        assert!(!ssn_structure_plausible("123-00-6789"));
        assert!(!ssn_structure_plausible("123-45-0000"));
        assert!(!ssn_structure_plausible("900-45-6789"));
    }

    #[test]
    fn phone_shape_rejects_bare_digit_runs() {
        assert!(phone_is_formatted("(555) 867-5309"));
        assert!(phone_is_formatted("+1 555 867 5309"));
        assert!(phone_is_formatted("555-867-5309"));
        assert!(!phone_is_formatted("1234567890"));
    }
}
