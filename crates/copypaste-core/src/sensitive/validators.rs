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
///   secret" is what AGENTS.md rule 4 and manifest I1 require. Neither the
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

/// A payment card, not merely a Luhn-valid digit run (§5.4).
///
/// Luhn accepts about one arbitrary 13-19 digit run in ten and the card rule
/// sits at 0.99, so an ISBN with a quantity beside it or a column of amounts was
/// hard-deleted: a measured 10.8 % of a realistic numeric corpus (DMY-162).
/// `card-validate` supplies the issuer ranges and per-brand lengths. §7.3 names
/// taking Luhn from a crate as the preferred outcome; this crate carries both.
pub(super) fn card_number_valid(matched: &str) -> bool {
    // Grouping and separator uniformity are the candidate pattern's job, not a
    // second check here: one alternative per spelling is what makes the
    // four-digit leading group structural. Order the rest cheapest-first — every
    // digit run in ordinary text reaches this function, and `Validate::from`
    // walks twelve brand regexes where the clamp is a length test. The clamp
    // stays ours: §5.4 pins it, and it holds whatever a brand table says.
    let digits: String = matched.chars().filter(char::is_ascii_digit).collect();
    (13..=19).contains(&digits.len())
        && card_validate::Validate::is_luhn_valid(&digits)
        && card_validate::Validate::from(&digits).is_ok()
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
    use crate::sensitive::engine::test_support::{
        all_rules, detector, fired, noise, rep, rule_severity, ALNUM, BASE64, LETTERS,
    };
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
        let strong = format!("export DATADOG_API_KEY={}", noise(40, ALNUM));
        assert!(fired(&det, &strong, "dotenv_secret"));
        assert_eq!(
            rule_severity(&det, &strong, "dotenv_secret"),
            Some(Severity::Restricted)
        );
        // The cost of 4.4, stated as a test rather than left to be discovered:
        // a 16-symbol alphabet cannot exceed 4 bits, so no hex value reaches
        // this rule any more. It is still detected, and still withheld, by
        // `generic_api_key` at 0.75 (§5.6).
        let hex = "export DATADOG_API_KEY=8f14e45fceea167a5a36dedd4bea2543";
        assert!(!fired(&det, hex, "dotenv_secret"));
        assert!(fired(&det, hex, "generic_api_key"));
        assert!(det.is_sensitive(hex));
    }

    /// §5.4's test-fixture bug: the original negative fixture `4242424242422`
    /// was *accidentally Luhn-valid* (digit sum 50), so the "must not match"
    /// test passed vacuously. **Assert your negative fixtures are negative.**
    #[test]
    fn negative_card_fixtures_are_actually_negative() {
        let luhn = card_validate::Validate::is_luhn_valid;
        assert!(luhn("4242424242422"), "the old fixture really was valid");
        assert!(!luhn("4242424242421"), "the replacement must be invalid");
        assert!(!card_number_valid("4242424242421"));
        assert!(card_number_valid("4111111111111111"));
        // The 13..=19 clamp, independent of the checksum and of any brand table.
        // Both fixtures pass Luhn, so the clamp is the only thing rejecting them.
        for outside_the_clamp in ["42424242420", "41111111111111111115"] {
            assert!(luhn(outside_the_clamp), "{outside_the_clamp}");
            assert!(!card_number_valid(outside_the_clamp));
        }
    }

    /// The gate DMY-162 added: a Luhn-valid digit run is not a card. Both
    /// fixtures below pass the checksum and belong to no issuer range, which is
    /// what a book's ISBN and a warehouse order id look like.
    #[test]
    fn luhn_valid_runs_outside_every_issuer_range_are_not_cards() {
        let luhn = card_validate::Validate::is_luhn_valid;
        for not_a_card in ["9780132350883", "1234567890123452"] {
            assert!(luhn(not_a_card), "fixture must be Luhn-valid: {not_a_card}");
            assert!(!card_number_valid(not_a_card), "{not_a_card}");
        }
    }

    /// Every brand the manifest's card contract has to keep, in the bare and the
    /// grouped spelling. Separators are stripped before the brand and length
    /// rules run, so the two spellings must agree.
    ///
    /// The positive control for the restricted band: a real card is still
    /// detected, still classified, still kept out of the index and previews —
    /// and still never deleted. `may_auto_wipe` is asserted false here for the
    /// same fixtures, so a change that quietly restores deletion fails.
    #[test]
    fn real_cards_are_detected_bare_and_grouped() {
        let det = detector();
        for (bare, grouped) in [
            ("4111111111111111", "4111 1111 1111 1111"),
            ("5555555555554444", "5555 5555 5555 4444"),
            ("378282246310005", "3782 822463 10005"),
            ("30569309025904", "3056 930902 5904"),
            ("6011111111111117", "6011-1111-1111-1117"),
            ("3530111333300000", "3530-1113-3330-0000"),
        ] {
            for form in [bare, grouped] {
                assert!(card_number_valid(form), "card rejected: {form}");
                assert!(fired(&det, form, "credit_card"), "rule missed {form}");
                assert!(det.is_sensitive(form), "{form} was not withheld");
                assert!(!det.may_auto_wipe(form), "{form} became deletable");
                assert_eq!(det.scan(form).unwrap().severity, Severity::Restricted);
            }
        }
        // Embedded in ordinary text, which is Audit MED #6's case and the one
        // that decides whether the *item* is withheld.
        let embedded = "Customer card: 4111 1111 1111 1111 — expires 12/26";
        assert!(fired(&det, embedded, "credit_card"));
        assert!(det.is_sensitive(embedded));
        assert!(!det.may_auto_wipe(embedded));
    }

    /// Ordinary 16-digit order ids that happen to start inside an issuer range
    /// and happen to pass Luhn — the mutation that rejected the first attempt at
    /// this fix, where 71 of 600 were hard-deleted.
    ///
    /// Deliberately *not* tautological: every prefix here is one the brand table
    /// accepts, every id is generated to be Luhn-valid, and each is therefore
    /// indistinguishable from a card by anything the digits carry. The test is
    /// that none of them can be deleted — not that none of them is detected,
    /// which no implementation could honestly claim.
    fn issuer_prefixed_order_ids() -> Vec<String> {
        let mut state = 0x9e37_79b9_7f4a_7c15_u64;
        let mut next_digit = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            char::from(b'0' + (state % 10) as u8)
        };
        let mut ids = Vec::new();
        for prefix in ["4", "51", "35", "60", "37"] {
            for _ in 0..120 {
                let mut id = String::from(prefix);
                while id.len() < 15 {
                    id.push(next_digit());
                }
                id.push(luhn_check_digit(&id));
                ids.push(id);
            }
        }
        ids
    }

    /// The digit that makes `body` Luhn-valid. A negative fixture that fails the
    /// checksum would prove nothing (§5.4's vacuously-passing `4242424242422`).
    fn luhn_check_digit(body: &str) -> char {
        let sum: u32 = body
            .bytes()
            .rev()
            .enumerate()
            .map(|(index, byte)| {
                let digit = u32::from(byte - b'0');
                if index % 2 == 0 {
                    let doubled = digit * 2;
                    if doubled > 9 {
                        doubled - 9
                    } else {
                        doubled
                    }
                } else {
                    digit
                }
            })
            .sum();
        char::from(b'0' + ((10 - sum % 10) % 10) as u8)
    }

    #[test]
    fn issuer_prefixed_order_ids_are_never_deletable() {
        let det = detector();
        let ids = issuer_prefixed_order_ids();
        assert_eq!(ids.len(), 600, "the mutation must stay large enough");
        assert!(
            ids.iter()
                .filter(|id| card_validate::Validate::is_luhn_valid(id))
                .count()
                == ids.len(),
            "every mutation entry must be Luhn-valid, or it proves nothing"
        );
        let accepted = ids.iter().filter(|id| card_number_valid(id)).count();
        // The number the old policy would have deleted: every id the validator
        // accepts was `HighConfidence` at 0.99 before the restricted band.
        eprintln!("{accepted} of {} order ids classify as cards", ids.len());
        assert!(
            accepted > 100,
            "only {accepted} of {} entries reach the card validator; the mutation \
             stopped exercising the classifier",
            ids.len()
        );

        let deletable: Vec<_> = ids
            .iter()
            .filter(|id| det.may_auto_wipe(&format!("order {id}")))
            .take(5)
            .collect();
        assert!(
            deletable.is_empty(),
            "{} of {} issuer-prefixed order ids would be deleted, e.g. {deletable:#?}",
            ids.iter()
                .filter(|id| det.may_auto_wipe(&format!("order {id}")))
                .count(),
            ids.len()
        );
    }

    /// Deterministic stand-in for the numeric data people copy all day: a column
    /// of amounts one per line, a tab-separated reading, an ISBN with a quantity
    /// beside it, and order or tracking ids. Every entry is structurally not a
    /// card — the separator is a newline or a tab, or the leading digits belong
    /// to no issuer — so none may ever reach `HighConfidence`, whatever the
    /// checksum says.
    ///
    /// Before DMY-162 the candidate scanner joined across newlines and tabs and
    /// the only gate was Luhn, so roughly one entry in eight was classified
    /// `credit_card` at 0.99 and hard-deleted whenever auto-wipe was on.
    fn realistic_numeric_corpus() -> Vec<String> {
        let mut state = 0x2545_f491_4f6c_dd1d_u64;
        let mut digits = move |count: u32, lead: char| {
            let mut out = String::from(lead);
            for _ in 1..count {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                out.push(char::from(b'0' + (state % 10) as u8));
            }
            out
        };
        let mut corpus = Vec::new();
        for _ in 0..120 {
            // amounts, one per line
            corpus.push(format!(
                "{}\n{}\n{}\n{}",
                digits(4, '1'),
                digits(4, '2'),
                digits(4, '3'),
                digits(4, '4')
            ));
            // a tab-separated meter reading
            corpus.push(format!(
                "reading\t{}\t{}\t{}\t{}",
                digits(4, '9'),
                digits(4, '8'),
                digits(4, '7'),
                digits(4, '6')
            ));
            // ISBN-13 with a quantity after it
            corpus.push(format!(
                "ISBN 978-{}-{}-{} qty {}",
                digits(3, '0'),
                digits(2, '1'),
                digits(6, '2'),
                digits(2, '1')
            ));
            // an order id, bare and grouped
            corpus.push(format!("order {}", digits(16, '1')));
            corpus.push(format!(
                "tracking {} {} {} {}",
                digits(4, '0'),
                digits(4, '9'),
                digits(4, '7'),
                digits(4, '1')
            ));
        }
        corpus
    }

    #[test]
    fn realistic_numeric_data_is_never_classified_for_deletion() {
        let det = detector();
        let corpus = realistic_numeric_corpus();
        let wiped: Vec<_> = corpus
            .iter()
            .filter(|text| det.may_auto_wipe(text))
            .take(5)
            .collect();
        assert!(
            wiped.is_empty(),
            "{} of {} realistic numeric samples would be deleted, e.g. {wiped:#?}",
            corpus.iter().filter(|t| det.may_auto_wipe(t)).count(),
            corpus.len()
        );
        assert!(corpus.len() >= 500, "the corpus must stay large enough");
    }

    /// The grouping rule on its own, stated as the property it defends: a card
    /// is written as a four-digit leading group followed by groups joined by one
    /// uniform separator, and a column of numbers is not a card however the
    /// checksum lands.
    ///
    /// The last two entries are the ones an optional separator lets through.
    /// Every digit here comes from the manifest's own `4111 1111 1111 1111`
    /// fixture, so the checksum passes and the grouping is the only thing that
    /// can reject them.
    #[test]
    fn card_candidates_require_a_four_digit_group_and_one_uniform_separator() {
        let det = detector();
        for text in [
            "4111\n1111\n1111\n1111",
            "4111\t1111\t1111\t1111",
            "4111 1111-1111 1111",
            "4111  1111  1111  1111",
            "41111111 11111111",
            "4111 111111111111",
            "41111111-11111111",
            "4111-111111111111",
        ] {
            assert!(
                !fired(&det, text, "credit_card"),
                "credit_card fired on {text:?}"
            );
            assert!(!det.is_sensitive(text), "{text:?}");
        }
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
        assert_eq!(det.scan(line).unwrap().severity, Severity::Restricted);
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

    /// Ordinary values from a README or an `.env.example`, each written at the
    /// length and over the alphabet its rule's pattern demands, paired with the
    /// value the rule must capture.
    fn context_anchored_placeholders() -> Vec<(&'static str, String, String)> {
        let mut cases = Vec::new();
        let mut add = |rule: &'static str, text: String, value: &str| {
            cases.push((rule, text, value.to_string()));
        };
        for value in [
            // The exact template DMY-162 reproduced, at 40. It measures 2.233
            // and inherited upstream's 2.0, so it was deleted.
            "YOUR_TOKEN_HERE_XXXXXXXXXXXXXXXXXXXXXXXX",
            "REPLACE_WITH_YOUR_CLOUDFLARE_API_TOKEN_X",
            "YOUR_CLOUDFLARE_API_TOKEN_GOES_HERE_HERE",
            "your-cloudflare-api-token-goes-right-her",
            "TODO_SET_THIS_BEFORE_DEPLOY_TODO_SET_THI",
            "CHANGEME_CHANGEME_CHANGEME_CHANGEME_CHAN",
        ] {
            add(
                "cloudflare_api_token",
                format!("CLOUDFLARE_API_TOKEN={value}"),
                value,
            );
        }
        for value in [
            "REPLACE/WITH/YOUR/AWS/SECRET/ACCESS/KEYX",
            "EXAMPLE/SECRET/KEY/VALUE/PLEASE/CHANGE/M",
            "your+aws+secret+access+key+goes+right+he",
            "abcdefghijabcdefghijabcdefghijabcdefghij",
        ] {
            add(
                "aws_secret_access_key",
                format!("aws_secret_access_key = {value}"),
                value,
            );
        }
        for value in [
            "PutYourOwnAzureStorageAccountKeyHereBeforeDeployPutYourOwnAzureStorageAccountKeyHereOk",
            "01234567890123456789012345678901234567890123456789012345678901234567890123456789012345",
        ] {
            add("azure_storage_key", format!("AccountKey={value}=="), value);
        }
        for value in [
            "REPLACE_ME_WITH_THE_REAL_VALUE_PLEASE_OK",
            "changeme-please-before-you-deploy",
            "put-the-real-value-in-your-env-file",
            "YOUR_API_TOKEN_HERE",
            "TODO_before_release",
        ] {
            add("dotenv_secret", format!("MY_API_TOKEN={value}"), value);
        }
        // The shape guard rather than the threshold. Every value below measures
        // *above* its rule's number — the pangram at 4.753 against 4.3 and 4.4 —
        // and is rejected for carrying no digit and no symbol, which is the
        // question entropy cannot answer (§5.6). `sk_test_…` is DMY-162's last
        // named false positive at 4.515: a credential's variety by every
        // threshold, while the neighbouring `stripe_live` refuses that prefix on
        // purpose because test keys occur in public examples. `noise` over
        // letters alone is the highest-variety value the guard admits.
        for value in [
            "TheQuickBrownFoxJumpsOverTheLazyDogAbcde".to_string(),
            "Deploy-Your-Own-Cloudflare-Token-Here-Ab".to_string(),
            noise(40, LETTERS),
        ] {
            add(
                "cloudflare_api_token",
                format!("CLOUDFLARE_API_TOKEN={value}"),
                &value,
            );
        }
        for value in [
            "ReplaceWithYourAwsSecretAccessKeyPleaseX".to_string(),
            noise(40, LETTERS),
        ] {
            add(
                "aws_secret_access_key",
                format!("aws_secret_access_key = {value}"),
                &value,
            );
        }
        for value in [
            "sk_test_abcdefghijklmnopqrstuvwx".to_string(),
            "TheQuickBrownFoxJumpsOverTheLazyDogAbcde".to_string(),
            noise(40, LETTERS),
        ] {
            add("dotenv_secret", format!("MY_API_TOKEN={value}"), &value);
        }
        // No word at all. A repetitive value is a placeholder whatever it
        // spells, which is what dropping the word list bought.
        add(
            "azure_storage_key",
            format!("AccountKey={}==", rep('A', 86)),
            &rep('A', 86),
        );
        add(
            "cloudflare_api_token",
            format!("CLOUDFLARE_API_TOKEN={}", rep('b', 40)),
            &rep('b', 40),
        );
        cases
    }

    /// §5.4's lesson, applied to these rules: **assert your negative fixtures
    /// are actually negative.** §9.2's Cloudflare row was written with a
    /// 38-character value where the pattern requires exactly 40, so it could
    /// never fire the rule it was meant to constrain, and the test that quoted
    /// it passed on the length. Every placeholder must reach its rule and be
    /// rejected by the value gate, not by the shape.
    #[test]
    fn context_anchored_negatives_reach_their_rule_before_the_value_gate() {
        for (name, text, value) in context_anchored_placeholders() {
            let spec = crate::sensitive::rules::rule(name);
            let pattern = regex::Regex::new(spec.pattern).expect("rule pattern compiles");
            let captured = pattern
                .captures(&text)
                .unwrap_or_else(|| panic!("{name} never matches {text}"))
                .get(spec.secret_group)
                .map(|group| group.as_str().to_string());
            assert_eq!(captured, Some(value), "{name} captured something else");
        }
    }

    /// The rules that earn their place above the floor with a context anchor
    /// rather than a distinctive token. The anchor proves which *field* was
    /// matched and says nothing about the *value*, so a README example
    /// classified at 0.90-0.99 and was deleted.
    ///
    /// Two gates answer that, both taken from what the value *is* rather than
    /// what it spells: its randomness, and gitleaks' own secret shape. Every
    /// placeholder below is repetitive, a template, or spelled without a digit
    /// or a symbol, and none of them names a word the code knows.
    #[test]
    fn context_anchored_placeholders_are_not_credentials() {
        let det = detector();
        let mut texts: Vec<String> = context_anchored_placeholders()
            .into_iter()
            .map(|(_, text, _)| text)
            .collect();
        texts.extend([
            format!("AccountKey=your{}==", rep('A', 82)),
            format!("CLOUDFLARE_API_TOKEN=your{}", rep('b', 36)),
            format!("aws_secret_access_key = your{}", rep('c', 36)),
            format!("AccountKey=fake{}==", rep('A', 82)),
            format!("CLOUDFLARE_API_TOKEN=sample{}", rep('b', 34)),
        ]);
        for placeholder in texts {
            assert!(
                det.scan_all(&placeholder).is_empty(),
                "placeholder classified: {placeholder} -> {:?}",
                all_rules(&det, &placeholder)
            );
        }
        // `azure_storage_key`'s guard is load-bearing above its own 4.8: 86
        // random letters measure 5.002 and the rule still refuses them. So does
        // `generic_api_key` now, whose capture takes the trailing `==` and left
        // the same template deletable through the one rule that missed it.
        let all_letters = format!("AccountKey={}==", noise(86, LETTERS));
        assert!(
            det.scan_all(&all_letters).is_empty(),
            "{all_letters} -> {:?}",
            all_rules(&det, &all_letters)
        );
        // …while values with a credential's randomness are still classified and
        // still withheld. The band takes the deletion, not the detection.
        for (real, rule) in [
            (
                format!("AccountKey={}==", noise(86, BASE64)),
                "azure_storage_key",
            ),
            (
                format!("CLOUDFLARE_API_TOKEN={}", noise(40, ALNUM)),
                "cloudflare_api_token",
            ),
            (
                format!("MY_API_TOKEN={}", noise(40, ALNUM)),
                "dotenv_secret",
            ),
            (
                "aws_secret_access_key = wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".to_string(),
                "aws_secret_access_key",
            ),
        ] {
            assert!(det.is_sensitive(&real), "{real}");
            assert!(!det.may_auto_wipe(&real), "{real}");
            assert_eq!(
                rule_severity(&det, &real, rule),
                Some(Severity::Restricted),
                "{real}"
            );
        }
    }

    /// The two gates `generic_password_kv` gained, and what they cost.
    ///
    /// Neither is a word list: a value spelled from four or fewer effective
    /// symbols is repetitive, and one spelled with no digit and no symbol is a
    /// template whatever it says (§5.6). The second is the more expensive of the
    /// two here, because this rule's values include human-chosen passwords —
    /// stated in the config decision and pinned by the quoted case below, which
    /// keeps its quotes in the capture and is still detected.
    #[test]
    fn the_password_rule_rejects_repetitive_and_shapeless_values() {
        let det = detector();
        // No rule at all, not merely not this one: a template that slips the
        // split into the sibling rule is still a false positive.
        for template in [
            "TWILIO_API_KEY=ACxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx",
            "MAILGUN_API_KEY=key-0000000000000000000000000000000",
            "api_key: xxxxxxxxxxxxxxxxxxxx",
            "password: abcdefghij",
            "api_key = your-own-key-goes-here",
        ] {
            assert!(
                det.scan_all(template).is_empty(),
                "{template} -> {:?}",
                all_rules(&det, template)
            );
        }
        // §9.1's own fixtures, which fix the threshold's ceiling at 2.807, and
        // the 10-character boundary that `password: abcdefghij` used to pin —
        // now pinned by the CJK pair, which is the fixture carrying the
        // bytes-versus-chars defect anyway.
        for (credential, rule) in [
            ("password=hunter2", "generic_password_kv"),
            ("secret = !abcdef", "generic_password_kv"),
            ("db_password=S3cur3Pass!word", "generic_password_kv"),
            ("password: 私的秘密言葉確認鍵値", "generic_password_kv"),
            ("my_api_key = \"correcthorsebatterystaple\"", "api_key_kv"),
        ] {
            assert!(
                fired(&det, credential, rule),
                "{credential} -> {:?}",
                all_rules(&det, credential)
            );
            assert!(det.is_sensitive(credential), "{credential}");
        }
        assert!(!det.is_sensitive("password: 私的秘密言葉確認鍵"));
    }

    /// A real credential this rule is the only one to see. It is withheld from
    /// the index, from sync and from previews, and it is never deleted: the
    /// keyword proves the field and nothing here proves the value, which is
    /// §4.2's rule and the reason the four beside it are restricted too.
    #[test]
    fn a_real_password_value_is_withheld_and_never_deleted() {
        let det = detector();
        for text in [
            format!("password={}", noise(40, ALNUM)),
            format!("db_password={}", noise(32, BASE64)),
            format!("client_secret={}", noise(32, ALNUM)),
        ] {
            assert!(fired(&det, &text, "generic_password_kv"), "{text}");
            assert!(det.is_sensitive(&text), "{text}");
            assert!(!det.may_auto_wipe(&text), "{text}");
        }
    }

    /// Every §9.1 keyword-KV positive in one list, with the rule that owns it.
    ///
    /// The `api_key` family carries the vendored placeholder vocabulary and the
    /// rest do not, which is the whole of the split. Borrowing that list onto the
    /// undivided rule suppresses eight of these outright — `value`, `acces` and
    /// `word` are all stopwords — and a suppressed 0.75 rule is a credential
    /// that is neither flagged, nor masked, nor kept out of `clipboard_fts`
    /// (§5.6 fail-closed). Those eight are what a later "one rule would do"
    /// takes out, so they are pinned here with the rest.
    #[test]
    fn every_binding_keyword_kv_positive_is_still_detected() {
        let det = detector();
        for (text, rule) in [
            ("access_token=abc123XYZlongvalue99", "generic_password_kv"),
            ("access_token: gh_access_abc123XYZ", "generic_password_kv"),
            (
                "export access_token=abc123XYZlongvalue99",
                "generic_password_kv",
            ),
            ("client_secret=Sup3rS3cr3tV@lue!", "generic_password_kv"),
            (
                "refresh_token=rt_abc123XYZlong_value",
                "generic_password_kv",
            ),
            (
                "refresh_token = rt_PROD_abc123XYZlongval",
                "generic_password_kv",
            ),
            ("db_password=S3cur3Pass!word", "generic_password_kv"),
            ("password=hunter2", "generic_password_kv"),
            ("secret = !abcdef", "generic_password_kv"),
            ("password: 私的秘密言葉確認鍵値", "generic_password_kv"),
            (
                "access-token: rt_abc123XYZlong_value",
                "generic_password_kv",
            ),
            ("refresh.token=rt_abc123XYZlongval", "generic_password_kv"),
            ("auth token = abc123XYZlongvalue99", "generic_password_kv"),
            ("auth-token = abc123XYZlongvalue99", "generic_password_kv"),
            ("client secret = Sup3rS3cr3tV@lue!", "generic_password_kv"),
            ("db-password: S3cur3Pass!word", "generic_password_kv"),
            ("api-key: abc123XYZlong", "api_key_kv"),
            ("api.key = abc123XYZlong", "api_key_kv"),
            ("api key: abc123XYZlong", "api_key_kv"),
            ("my_api_key = \"correcthorsebatterystaple\"", "api_key_kv"),
            // One marker, and the reason the count starts at two: `value` is a
            // stopword, and §9.1 binds the quoted `.env` form as detected.
            ("export MY_API_KEY=\"S3cr3tValue123\"", "api_key_kv"),
        ] {
            assert!(
                fired(&det, text, rule),
                "{text} -> {:?}",
                all_rules(&det, text)
            );
            assert!(det.is_sensitive(text), "{text} would reach the index");
            assert_eq!(
                rule_severity(&det, text, rule),
                Some(Severity::Restricted),
                "{text}"
            );
            assert!(!det.may_auto_wipe(text), "{text}");
        }
    }

    /// The one template no threshold this rule may carry can reach, and the
    /// one-marker credentials that is true of.
    ///
    /// `Replace_With_Your_Real_Token_Value123456` measures **4.354** against a
    /// threshold of 4.3, so §5.6's 4.225 template ceiling holds only for the
    /// single-case word population it was drawn from. Raising the threshold
    /// instead would *create* deletions: the restricted match that vetoes
    /// `generic_api_key` disappears with it (§4.2, F1). Counting distinct
    /// vendored markers separates the two populations — four here, at most one
    /// in every value §9.1 binds.
    #[test]
    fn the_cloudflare_prose_template_is_counted_out_and_a_marked_token_is_not() {
        let det = detector();
        let template = "CLOUDFLARE_API_KEY=Replace_With_Your_Real_Token_Value123456";
        assert!(
            det.scan_all(template).is_empty(),
            "{template} -> {:?}",
            all_rules(&det, template)
        );
        assert!(!det.is_sensitive(template));
        assert!(!det.may_auto_wipe(template));
        for marker in ["your", "todo", "dummy", "sample"] {
            let text = format!(
                "CLOUDFLARE_API_TOKEN={marker}{}",
                noise(40 - marker.len(), ALNUM)
            );
            assert!(
                fired(&det, &text, "cloudflare_api_token"),
                "{text} -> {:?}",
                all_rules(&det, &text)
            );
            assert!(det.is_sensitive(&text), "{text}");
            assert!(!det.may_auto_wipe(&text), "{text}");
        }
    }

    /// `generic_api_key` keeps its own band, its own 3.5 and its own detection.
    /// Only the padding its capture swallows moved, so a value the vendored
    /// shape guard was written to reject no longer escapes it.
    #[test]
    fn the_upstream_generic_rule_still_detects_and_still_deletes() {
        let det = detector();
        for text in [
            "credential = aB3dE5gH7jK9".to_string(),
            format!("token: {}", noise(32, ALNUM)),
            format!("credential={}", noise(40, BASE64)),
        ] {
            assert!(fired(&det, &text, "generic_api_key"), "{text}");
            assert_eq!(
                rule_severity(&det, &text, "generic_api_key"),
                Some(Severity::HighConfidence),
                "{text}"
            );
            assert!(det.may_auto_wipe(&text), "{text}");
        }
        assert!(!fired(&det, "credential = aaaaaaaaaaaa", "generic_api_key"));
        assert!(!fired(&det, "credential = AbCdEfGhIjKl", "generic_api_key"));
    }

    /// The guarantee the restricted band buys, on inputs where one of these
    /// four is the only rule that fires: a correct match on a real credential
    /// is kept out of the index, out of sync and out of previews, and still
    /// cannot delete the item. Nothing in a human-readable value proves it is a
    /// credential, so an unforeseen template clearing the threshold must cost
    /// searchability rather than the data (I1, §5.6, DMY-162).
    #[test]
    fn a_context_anchored_credential_is_withheld_and_never_deleted() {
        let det = detector();
        let key = noise(86, BASE64);
        for (text, rule) in [
            (
                "aws_secret_access_key = wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".to_string(),
                "aws_secret_access_key",
            ),
            (
                format!("aws_secret_access_key = sample{}", noise(34, ALNUM)),
                "aws_secret_access_key",
            ),
            (
                format!("AccountKey={}todo{}==", &key[..41], &key[45..]),
                "azure_storage_key",
            ),
            (format!("SERVICE_PWD={}", noise(40, ALNUM)), "dotenv_secret"),
        ] {
            assert_eq!(all_rules(&det, &text), [rule], "{text}");
            assert!(det.is_sensitive(&text), "{text}");
            assert!(!det.may_auto_wipe(&text), "{text}");
        }
        // Both rules a Cloudflare line fires are in the band, so the same holds.
        let cloudflare = format!("CLOUDFLARE_API_TOKEN=todo{}", noise(36, ALNUM));
        assert_eq!(
            all_rules(&det, &cloudflare),
            ["cloudflare_api_token", "dotenv_secret"]
        );
        assert!(det.is_sensitive(&cloudflare));
        assert!(!det.may_auto_wipe(&cloudflare));
    }

    /// The control that rejected both spellings of a word list. A real
    /// credential that merely *carries* a placeholder word was suppressed
    /// outright by the `contains` form — not made inert, but dropped from every
    /// finding, so it was neither flagged, nor masked, nor kept out of
    /// `clipboard_fts`. The `starts_with` form that replaced it moved the same
    /// failure to values that *begin* with one, which is rarer and no safer:
    /// both fail open, and §5.6 requires a suppressing gate to fail closed.
    ///
    /// There is no list now. Each value below is credential-shaped and carries
    /// `your`, `todo`, `dummy`, `sample` or `example` — at the start for the
    /// first three — and every one must be detected on its randomness alone.
    #[test]
    fn real_credentials_carrying_a_placeholder_word_are_still_detected() {
        let det = detector();
        // Spliced from one block, not concatenated from two `noise` calls:
        // `noise` is seeded per call, so two calls return the *same* string and
        // the doubled value measures 4.590 where a real 86-character key
        // measures above 4.9. A positive fixture has to be shaped like the
        // thing it stands for.
        let key = noise(86, BASE64);
        let azure_word_inside = format!("AccountKey={}todo{}==", &key[..41], &key[45..]);
        for (text, rule) in [
            (
                format!("AccountKey=your{}==", noise(82, BASE64)),
                "azure_storage_key",
            ),
            (
                format!("CLOUDFLARE_API_TOKEN=todo{}", noise(36, ALNUM)),
                "cloudflare_api_token",
            ),
            (
                format!("aws_secret_access_key = sample{}", noise(34, ALNUM)),
                "aws_secret_access_key",
            ),
            (
                format!("DATADOG_API_TOKEN=dummy{}", noise(32, ALNUM)),
                "dotenv_secret",
            ),
            (azure_word_inside, "azure_storage_key"),
            (
                // AWS's own published secret key, which ends EXAMPLEKEY and
                // which §9.1 binds. Nothing carves `example` out any more.
                "aws_secret_access_key = wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".to_string(),
                "aws_secret_access_key",
            ),
        ] {
            assert!(
                fired(&det, &text, rule),
                "{rule} suppressed by a placeholder word: {text} -> {:?}",
                all_rules(&det, &text)
            );
            assert!(det.is_sensitive(&text), "{text} would reach the index");
            assert_eq!(
                rule_severity(&det, &text, rule),
                Some(Severity::Restricted),
                "{text}"
            );
        }
    }

    /// The keyword spellings §3.2 rule 23 left out. `api-key:`, `access.token:`
    /// and `auth token =` are the ordinary YAML, CLI-flag and prose forms and
    /// reached no rule at all, because there is no bare `key` or `token` keyword
    /// to fall back on. `client secret` and `db-password` are here to pin that
    /// the bare `secret` and `password` keywords still cover them. The
    /// value-strength gate is untouched, so the weak column stays undetected.
    #[test]
    fn delimiter_spellings_of_the_password_keyword_are_covered() {
        let det = detector();
        for (strong, rule) in [
            ("api-key: abc123XYZlong", "api_key_kv"),
            ("api.key = abc123XYZlong", "api_key_kv"),
            ("api key: abc123XYZlong", "api_key_kv"),
            (
                "access-token: rt_abc123XYZlong_value",
                "generic_password_kv",
            ),
            ("refresh.token=rt_abc123XYZlongval", "generic_password_kv"),
            ("auth token = abc123XYZlongvalue99", "generic_password_kv"),
            ("auth-token = abc123XYZlongvalue99", "generic_password_kv"),
            // already covered by the bare `secret` and `password` keywords
            ("client secret = Sup3rS3cr3tV@lue!", "generic_password_kv"),
            ("db-password: S3cur3Pass!word", "generic_password_kv"),
        ] {
            assert!(
                fired(&det, strong, rule),
                "{strong} -> {:?}",
                all_rules(&det, strong)
            );
            assert!(det.is_sensitive(strong), "{strong}");
        }
        for weak in [
            "api-key: see the wiki",
            "client secret: ask ops",
            "db-password: ${VAULT_DB}",
        ] {
            assert!(
                !det.is_sensitive(weak),
                "{weak} -> {:?}",
                all_rules(&det, weak)
            );
        }
    }

    #[test]
    fn gitlab_pat_is_detected() {
        let det = detector();
        let token = format!("glpat-{}", noise(20, ALNUM));
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
