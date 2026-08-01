//! The false-positive gates a regex match must survive before it counts.
//!
//! Each answers "the shape matched — is it actually a secret?", and each exists
//! because a v1 rule without it deleted user data or missed a real credential.
//! No fast paths and no second copy of the same algorithm (§7.3).

/// Characters in the §5.3 "special character" set. Note `$`, `#`, `%`, `/` and
/// `=` are *strength* signals here, which is why the code-shape rejection below
/// cannot simply blacklist punctuation.
const STRENGTH_SPECIALS: &str = "!@#$%^&*+/=";

/// Code punctuation that a machine-issued credential essentially never
/// contains, but that source code and templates are full of. v2 addition —
/// see `value_is_strong`.
const CODE_SHAPED_CHARS: &[char] = &['(', ')', '\'', '"', '`', '<', '>'];

/// Whole-value placeholder stopwords (gitleaks' allowlist model, §5.6 — v1 had
/// none and called that its biggest structural gap). Matched against the
/// **entire** value, case-insensitively, never as a substring: a substring
/// match could suppress a real credential containing one of these sequences.
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
/// rules (§5.3), plus the v2 additions §7.1 and §7.7 ask for.
///
/// A value is strong if **any** of the three manifest criteria holds:
///
/// 1. `value.chars().count() >= 10` — **characters, not bytes**. A 9-character
///    CJK value (`私的秘密言葉確認鍵`) is 27 bytes, and a byte gate would call
///    it strong; pinned by `multibyte_value_gated_on_chars_not_bytes`.
/// 2. contains one of `! @ # $ % ^ & * + / =`;
/// 3. contains at least one ASCII letter **and** at least one ASCII digit.
///
/// The criteria are applied to the value with **balanced surrounding quotes
/// stripped**, because `password="S3cr3tValue"` — the ordinary shape of a
/// credential in `.env`, JSON and YAML — otherwise dies on the code-shape
/// rejection below and is missed entirely. A missed secret reaches
/// `clipboard_fts`, which is the one table not under the item AEAD, and is then
/// syncable. Stripping is deliberately limited to a *balanced* pair, so
/// `prompt('enter` keeps its unbalanced quote and stays rejected.
///
/// Two rejections then run *before* the criteria:
///
/// * **Code shape.** The benign corpus contains
///   `const password = prompt('enter password:');`, where group 1 captures
///   `prompt('enter` — 13 characters, so criterion 1 calls it strong and v1
///   auto-wiped it (one of the two FPs v1's 5 % budget absorbed, §7.7). A value
///   containing `(`, `)`, `'`, `"`, `` ` ``, `<`, `>`, or opening with
///   `$`/`${`/`{{` is code or a template reference, and biasing toward "not a
///   secret" is what CLAUDE.md rule 4 and manifest I1 require. Neither the
///   code-shape nor the stopword rejection is in §5.3; both are v2 additions,
///   so where they contradict §5.3 the manifest wins.
/// * **Stopwords** (§5.6).
pub(super) fn value_is_strong(value: &str) -> bool {
    let value = unquote(value);
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

/// Strip one *balanced* pair of surrounding quotes, plus the trailing `,` or `;`
/// that a JSON or config line leaves on the captured value.
///
/// Balanced only: an opening quote with no closing partner is the
/// `prompt('enter` shape and must survive to be rejected as code.
fn unquote(value: &str) -> &str {
    let trimmed = value.trim_end_matches([',', ';']);
    let mut chars = trimmed.chars();
    match (chars.next(), chars.next_back()) {
        (Some(first), Some(last)) if first == last && matches!(first, '"' | '\'' | '`') => {
            chars.as_str()
        }
        _ => trimmed,
    }
}

/// Luhn checksum over a candidate digit run (§5.4). **One implementation**
/// (§7.3): v1 shipped `luhn_valid` and `luhn_valid_strict` as byte-for-byte the
/// same algorithm. The `13 ..= 19` clamp is load-bearing — it rejects short and
/// long numeric runs independently of the checksum.
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
                if doubled > 9 { doubled - 9 } else { doubled }
            } else {
                d
            }
        })
        .sum();
    sum.is_multiple_of(10)
}

pub(super) fn iban_valid(candidate: &str) -> bool {
    candidate.parse::<iban::Iban>().is_ok()
}

/// SSN group structure, per §4.2: group 1 in 001–899, group 2 in 01–99, group 3
/// in 0001–9999, no all-zero group. Trims obvious non-SSNs only; the rule stays
/// below the auto-wipe floor regardless, because a *real* SSN is still the
/// user's own data (§4.2).
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sensitive::Severity;
    use crate::sensitive::engine::test_support::{all_rules, detector, fired, rep};

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
    fn iban_requires_registered_structure_length_and_checksum() {
        let det = detector();
        for valid in [
            "NO9386011117947",
            "NL91ABNA0417164300",
            "KZ86125KZT5004100100",
            "DE89370400440532013000",
            "GB82WEST12345698765432",
            "AL47212110090000000235698741",
        ] {
            assert!(iban_valid(valid), "valid fixture rejected: {valid}");
            assert!(fired(&det, valid, "iban"), "IBAN rule missed {valid}");
            assert!(!det.may_auto_wipe(valid), "IBAN became wipeable: {valid}");
            let finding = det
                .scan_all(valid)
                .into_iter()
                .find(|finding| finding.rule == "iban")
                .unwrap_or_else(|| panic!("IBAN finding missing for {valid}"));
            assert_eq!(finding.severity, Severity::Flag);
        }

        assert!(matches!(
            "ZZ73123456789012345678".parse::<iban::Iban>(),
            Err(iban::ParseIbanError::UnknownCountry(_))
        ));
        for invalid_length in ["DE5137040044053201300", "DE813704004405320130000"] {
            assert!(matches!(
                invalid_length.parse::<iban::Iban>(),
                Err(iban::ParseIbanError::InvalidBban(_))
            ));
        }
        assert!(matches!(
            "GB93WES112345698765432".parse::<iban::Iban>(),
            Err(iban::ParseIbanError::InvalidBban(_))
        ));
        assert!(matches!(
            "DE88370400440532013000".parse::<iban::Iban>(),
            Err(iban::ParseIbanError::InvalidBaseIban {
                source: iban::ParseBaseIbanError::InvalidChecksum
            })
        ));

        for (reason, invalid) in [
            ("unknown country", "ZZ73123456789012345678"),
            ("country length short", "DE5137040044053201300"),
            ("country length long", "DE813704004405320130000"),
            ("checksum", "DE88370400440532013000"),
            ("country BBAN shape", "GB93WES112345698765432"),
        ] {
            assert!(!iban_valid(invalid), "{reason} fixture is valid: {invalid}");
            assert!(
                !fired(&det, invalid, "iban"),
                "IBAN rule accepted {reason} fixture {invalid}"
            );
        }
    }

    #[test]
    fn iban_extraction_handles_embedded_and_multibyte_context() {
        let det = detector();
        let iban = "DE89370400440532013000";
        for text in [
            format!("beneficiary={iban}; currency=EUR"),
            format!("Рахунок🙂{iban}付款"),
        ] {
            let finding = det
                .scan_all(&text)
                .into_iter()
                .find(|finding| finding.rule == "iban")
                .unwrap_or_else(|| panic!("IBAN finding missing from {text:?}"));
            assert_eq!(&text[finding.start..finding.end], iban);
            assert_eq!(finding.severity, Severity::Flag);
            assert!(!det.may_auto_wipe(&text));
        }

        for glued in [format!("X{iban}"), format!("{iban}X")] {
            assert!(!fired(&det, &glued, "iban"), "matched inside {glued:?}");
        }
    }

    #[test]
    fn iban_shaped_benign_text_produces_no_finding() {
        let det = detector();
        for benign in [
            "The documentation uses DE00123456789012345678 as a placeholder.",
            "Ticket ZZ73123456789012345678 is not a bank account.",
            "Legacy reference DE5137040044053201300 has the wrong length.",
        ] {
            assert!(
                !fired(&det, benign, "iban"),
                "IBAN false positive in {benign:?}"
            );
        }
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

    /// The security fix: a quoted value is the ordinary shape of a credential in
    /// `.env`, JSON and YAML, and the code-shape gate was throwing every one of
    /// them away. A missed secret reaches `clipboard_fts` and syncs.
    #[test]
    fn quoted_credentials_are_detected() {
        let det = detector();
        for text in [
            r#"password="S3cr3tValue""#,
            "password = 'S3cr3tValue'",
            r#""password": "hunter2xyz""#,
            r#"{"api_key": "abc123XYZlong", "region": "us-east-1"}"#,
            r#"password="S3cr3tValue","#,
            "db_password: `S3cr3tValue`",
        ] {
            assert!(det.is_sensitive(text), "missed a quoted credential: {text}");
        }
        // The dotenv form, which shares the same gate.
        assert!(det.is_sensitive(r#"export MY_API_KEY="S3cr3tValue123""#));
    }

    /// The gate the quote-stripping must not undo (§7.7): `prompt('enter` has an
    /// *unbalanced* quote, so it is still code and still rejected.
    #[test]
    fn unbalanced_quotes_stay_code_shaped() {
        let det = detector();
        assert!(!det.is_sensitive("const password = prompt('enter password:');"));
        assert!(!value_is_strong("prompt('enter"));
        assert!(!value_is_strong("getEnv();"));
        assert!(!value_is_strong(r#""changeme""#), "stopword inside quotes");
        assert!(!value_is_strong(r#""foo""#), "still too weak unquoted");
    }

    /// The AWS pair: v2 detected the public `AKIA…` id at 0.99 while the secret
    /// sitting next to it in `~/.aws/credentials` matched no rule at all.
    #[test]
    fn aws_secret_access_key_is_detected_beside_its_id() {
        let det = detector();
        let line = "aws_secret_access_key = wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY";
        assert!(fired(&det, line, "aws_secret_access_key"));
        assert_eq!(det.scan(line).unwrap().severity, Severity::HighConfidence);
        // Quoted and JSON forms of the same thing.
        assert!(det.is_sensitive(
            r#""aws_secret_access_key": "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY""#
        ));
        // The context anchor is mandatory: a bare 40-char run is not a secret.
        assert!(!fired(
            &det,
            "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY",
            "aws_secret_access_key"
        ));
    }

    #[test]
    fn gitlab_pat_is_detected() {
        let det = detector();
        let token = format!("glpat-{}", rep('A', 20));
        assert!(fired(&det, &token, "gitlab_pat"));
        assert_eq!(det.scan(&token).unwrap().severity, Severity::HighConfidence);
        // \b anchor: glued into a longer identifier it is not a token.
        assert!(!fired(&det, &format!("x{token}"), "gitlab_pat"));
    }

    /// `Authorization: Basic` carries base64(user:password), so it is detected
    /// and maskable — but searchable and inert, exactly like `generic_bearer`
    /// (**P2 fb3e**), because it lives in curl examples too.
    #[test]
    fn http_basic_auth_is_detected_but_inert() {
        let det = detector();
        let header = "Authorization: Basic dXNlcjpwYXNzd29yZA==";
        assert!(fired(&det, header, "http_basic_auth"));
        assert!(!det.is_sensitive(header));
        assert_eq!(det.scan(header).unwrap().severity, Severity::Flag);
    }
}
