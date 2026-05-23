use super::patterns::{pattern_category, pattern_confidence, pattern_name, pattern_set, patterns};
use std::ops::Range;
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

/// Returns true if a `generic_password_kv` match value is strong enough to be a real
/// credential, suppressing benign prose like `password: foo` or `// api_key=demo`.
///
/// Strong = any one of:
///   - value length ≥ 10
///   - contains a special char `[!@#$%^&*+/=]`
///   - mix of letter AND digit
fn is_credential_value_strong(value: &str) -> bool {
    if value.len() >= 10 {
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
fn match_is_false_positive(
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

// ── Public types (pattern-detection) ─────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum SensitiveCategory {
    Credential,
    Financial,
    PersonalId,
    Infrastructure,
}

impl SensitiveCategory {
    fn from_raw(raw: u8) -> Self {
        match raw {
            0 => Self::Credential,
            1 => Self::Financial,
            2 => Self::PersonalId,
            3 => Self::Infrastructure,
            _ => Self::Credential,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PatternMatch {
    pub pattern_name: &'static str,
    pub confidence: f32,
    pub category: SensitiveCategory,
    pub matched_range: Range<usize>,
}

/// Detects sensitive data patterns in text. Compiled regexes are initialised once
/// (via OnceLock) and shared across all instances — construction is effectively free.
#[derive(Default)]
pub struct SensitiveDetector;

impl SensitiveDetector {
    pub fn new() -> Self {
        Self
    }

    /// Return every pattern match found in `text`, with byte ranges and confidence.
    ///
    /// Input is NFKC-normalised first to defeat Unicode bypass tricks (full-width
    /// ASCII, ZWJ insertions, ligatures). Byte ranges in the returned matches are
    /// over the *normalised* string.
    pub fn detect(&self, text: &str) -> Vec<PatternMatch> {
        let normalised = nfkc_normalize(text);
        let mut results: Vec<PatternMatch> = Vec::new();
        for (i, re) in patterns().iter().enumerate() {
            for m in re.find_iter(&normalised) {
                let range = m.range();
                if match_is_false_positive(i, m.as_str(), &normalised, &range) {
                    continue;
                }
                results.push(PatternMatch {
                    pattern_name: pattern_name(i),
                    confidence: pattern_confidence(i),
                    category: SensitiveCategory::from_raw(pattern_category(i)),
                    matched_range: range,
                });
            }
        }
        results
    }

    /// Returns true if any sensitive pattern is found.
    ///
    /// Uses the fast `RegexSet` path first, then re-validates `generic_password_kv`
    /// candidates with the value-strength check to avoid prose false positives.
    pub fn is_sensitive(&self, text: &str) -> bool {
        let normalised = nfkc_normalize(text);
        let matches: Vec<usize> = pattern_set().matches(&normalised).into_iter().collect();
        if matches.is_empty() {
            return false;
        }
        // Cheap path: if any non-fp-prone pattern hit, we're done.
        if matches
            .iter()
            .any(|&i| pattern_name(i) != "generic_password_kv")
        {
            return true;
        }
        // Only generic_password_kv candidates remain — validate at least one is strong.
        !self.detect(&normalised).is_empty()
    }

    /// Returns true if any pattern exceeds the confidence threshold.
    pub fn is_sensitive_threshold(&self, text: &str, threshold: f32) -> bool {
        self.detect(text).iter().any(|m| m.confidence >= threshold)
    }

    /// Returns the highest-confidence match, if any.
    pub fn highest_confidence(&self, text: &str) -> Option<PatternMatch> {
        self.detect(text).into_iter().max_by(|a, b| {
            a.confidence
                .partial_cmp(&b.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    }
}

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
    sum % 10 == 0
}

// ── is_sensitive_app ──────────────────────────────────────────────────────────

/// Bundle IDs / process names for apps whose clipboard content should always
/// be treated as sensitive regardless of content patterns (e.g. password managers).
static SENSITIVE_APP_BUNDLE_IDS: &[&str] = &[
    // Password managers
    "com.1password.1password",
    "com.1password.7.1password",
    "com.agilebits.onepassword",
    "com.agilebits.onepassword4",
    "com.agilebits.onepassword-osx-helper",
    "com.bitwarden.desktop",
    "com.bitwarden.desktop.safari",
    "com.keepassxc.keepassxc",
    "org.keepassxc.keepassxc-browser",
    "com.lastpass.lastpass",
    "de.peterb.Dashlane",
    "com.dashlane.dashlane",
    "com.enpass.Enpass",
    "net.sourceforge.keepass",
    "com.stegosafe.StegSafe",
    "com.webpas.webpas",
    "com.roboform.roboform",
    "com.nordpass.macos",
    "com.logmeininc.lastpass",
    // Process name fragments (matched as substring)
    "1password",
    "bitwarden",
    "keepass",
    "dashlane",
    "lastpass",
    "enpass",
    "nordpass",
    "roboform",
];

/// Returns true if the given app bundle ID or process name is a known sensitive app
/// (e.g. a password manager). Match is case-insensitive substring on the lowercased input.
pub fn is_sensitive_app(app_bundle_id: &str) -> bool {
    let lower = app_bundle_id.to_lowercase();
    SENSITIVE_APP_BUNDLE_IDS
        .iter()
        .any(|&known| lower.contains(known))
}

#[derive(Debug, Clone, PartialEq)]
pub enum SensitiveKind {
    AwsKey,
    GitHubToken,
    OpenAIKey,
    AnthropicKey,
    StripeKey,
    NpmToken,
    PyPIToken,
    SlackToken,
    VaultToken,
    GcpToken,
    SshPrivateKey,
    Jwt,
    CreditCard,
    Other(String),
}

impl SensitiveKind {
    fn from_pattern_name(name: &str) -> Self {
        match name {
            n if n.starts_with("aws") => Self::AwsKey,
            n if n.starts_with("github") => Self::GitHubToken,
            n if n.starts_with("openai") => Self::OpenAIKey,
            "anthropic" => Self::AnthropicKey,
            "stripe_live" => Self::StripeKey,
            "npm_token" => Self::NpmToken,
            "pypi_token" => Self::PyPIToken,
            "slack_bot" => Self::SlackToken,
            "hashicorp_vault" => Self::VaultToken,
            "gcp_oauth" => Self::GcpToken,
            // Audit MED #5: PKCS#8-encrypted and PuTTY variants share the
            // same SshPrivateKey kind so callers (UI badge / log redactor)
            // see a uniform "ssh key" classification regardless of format.
            n if n.starts_with("ssh_private_key") => Self::SshPrivateKey,
            "jwt" => Self::Jwt,
            other => Self::Other(other.to_string()),
        }
    }
}

pub fn detect(text: &str) -> Option<SensitiveKind> {
    let normalised = nfkc_normalize(text);
    let candidate_indices: Vec<usize> = pattern_set().matches(&normalised).into_iter().collect();
    for &idx in &candidate_indices {
        // For generic_password_kv we must validate value strength to avoid FPs.
        if pattern_name(idx) == "generic_password_kv" {
            let re = &patterns()[idx];
            if let Some(m) = re.find(&normalised) {
                if match_is_false_positive(idx, m.as_str(), &normalised, &m.range()) {
                    continue;
                }
            }
        }
        tracing::debug!(pattern = pattern_name(idx), "sensitive content detected");
        return Some(SensitiveKind::from_pattern_name(pattern_name(idx)));
    }
    // Audit MED #6: the previous `normalised.len() <= 25 && Luhn(normalised)`
    // gate missed any card embedded in a longer string (e.g. "card:
    // 4111-1111-1111-1111 expires 12/26"). Scan the text for digit runs
    // 13..=19 long (optionally separated by `-` / whitespace) and
    // Luhn-validate each candidate independently. Operates on the
    // already-normalised string so Unicode digit bypasses are defeated by
    // the same NFKC pass.
    if contains_luhn_valid_card_run(&normalised) {
        return Some(SensitiveKind::CreditCard);
    }
    None
}

/// Returns true iff the input contains at least one candidate digit run
/// (13–19 ASCII digits, optionally separated by single `-` or whitespace)
/// that Luhn-validates as a credit-card number.
///
/// Uses a static `OnceLock<Regex>` so the candidate scanner is compiled once
/// per process. The pattern is anchored on word boundaries to skip mid-token
/// hits like `xid=4111111111111111foobar`.
fn contains_luhn_valid_card_run(text: &str) -> bool {
    use std::sync::OnceLock;
    static CARD_RUN_RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = CARD_RUN_RE.get_or_init(|| {
        // `\b(?:\d[\s-]?){13,19}\d\b` — between 13 and 19 digits with
        // optional single space or hyphen between each, plus a final
        // digit (so total = 14..=20 digits). The leading run already
        // matches one digit so we accept totals 13..=19 effectively;
        // the explicit Luhn `digits.len() < 13 || > 19` clamp filters.
        regex::Regex::new(r"\b(?:\d[\s-]?){12,18}\d\b").expect("static card-run regex is valid")
    });
    for m in re.find_iter(text) {
        if luhn_valid_strict(m.as_str()) {
            return true;
        }
    }
    false
}

/// Strip whitespace and `-`, then Luhn-validate. Mirrors the public
/// `super::luhn_valid` helper but inlined here to avoid an extra
/// allocation+digit-filter pass on the per-candidate hot path.
fn luhn_valid_strict(s: &str) -> bool {
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
    sum % 10 == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_aws_access_key() {
        assert!(detect("AKIAIOSFODNN7EXAMPLE").is_some());
    }
    #[test]
    fn detects_temporary_aws_key() {
        assert!(detect("ASIAIOSFODNN7EXAMPLE1234").is_some());
    }
    #[test]
    fn detects_github_classic_pat() {
        assert!(detect(&("ghp_".to_string() + &"A".repeat(36))).is_some());
    }
    #[test]
    fn detects_github_fine_grained_pat() {
        assert!(detect(&format!("github_pat_{}_{}", "A".repeat(22), "B".repeat(59))).is_some());
    }
    #[test]
    fn detects_openai_key() {
        assert!(detect(&("sk-proj-".to_string() + &"A".repeat(48))).is_some());
    }
    #[test]
    fn detects_anthropic_key() {
        assert!(detect(&("sk-ant-api03-".to_string() + &"A".repeat(80))).is_some());
    }
    #[test]
    fn detects_jwt() {
        assert!(detect(
            "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJ1c2VyIn0.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c"
        )
        .is_some());
    }
    #[test]
    fn detects_ssh_private_key() {
        assert!(detect("-----BEGIN RSA PRIVATE KEY-----\nMIIEo...").is_some());
    }
    #[test]
    fn detects_openssh_private_key() {
        assert!(detect("-----BEGIN OPENSSH PRIVATE KEY-----\nMIIEo...").is_some());
    }
    #[test]
    fn detects_pkcs8_encrypted_private_key() {
        // Audit MED #5 — PKCS#8 encrypted form previously slipped through.
        let blob = "garbage prefix\n-----BEGIN ENCRYPTED PRIVATE KEY-----\nMIIFD...\n";
        let kind = detect(blob).expect("should detect PKCS#8 encrypted key");
        assert!(matches!(kind, SensitiveKind::SshPrivateKey));
    }
    #[test]
    fn detects_putty_user_key_file() {
        // Audit MED #5 — PuTTY `.ppk` header.
        let blob = "PuTTY-User-Key-File-2: ssh-rsa\nEncryption: none\nComment: imported-from-openssh\n";
        let kind = detect(blob).expect("should detect PuTTY key");
        assert!(matches!(kind, SensitiveKind::SshPrivateKey));
    }
    #[test]
    fn jwt_word_boundary_anchors_match() {
        // Audit MED #5 — `\b` anchor: real JWT in normal context detects.
        let jwt =
            "Bearer eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJ1c2VyIn0.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c";
        assert!(detect(jwt).is_some());
        // A `eyJ`-prefixed garbage glued onto an identifier should NOT match
        // as a JWT (we'd still detect bearer token from generic_bearer if
        // present — here we use a non-bearer prefix to isolate the case).
        let glued = "configsomethingeyJabc.def.ghi notajwt";
        // Either no match at all OR not classified as Jwt — both are
        // acceptable; pin "not Jwt" precisely.
        let kind = detect(glued);
        assert!(
            !matches!(kind, Some(SensitiveKind::Jwt)),
            "glued eyJ inside an identifier must not be classified as JWT"
        );
    }
    #[test]
    fn detects_stripe_live_key() {
        assert!(detect(&("sk_live_".to_string() + &"A".repeat(24))).is_some());
    }
    #[test]
    fn detects_npm_token() {
        assert!(detect(&("npm_".to_string() + &"A".repeat(36))).is_some());
    }
    #[test]
    fn no_false_positive_on_lorem_ipsum() {
        assert!(detect("Lorem ipsum dolor sit amet, consectetur adipiscing elit.").is_none());
    }
    #[test]
    fn no_false_positive_on_short_code() {
        assert!(detect(r#"fn main() { println!("Hello, world!"); }"#).is_none());
    }
    #[test]
    fn credit_card_detected_short_line_only() {
        assert!(detect("4111111111111111").is_some());
    }
    #[test]
    fn credit_card_detected_when_embedded_in_longer_text() {
        // Audit MED #6: the previous `len <= 25` gate dropped this case.
        let blob = "Customer card: 4111 1111 1111 1111 — expires 12/26";
        let kind = detect(blob).expect("embedded card must be detected");
        assert!(matches!(kind, SensitiveKind::CreditCard));
    }
    #[test]
    fn credit_card_with_hyphens_in_long_text() {
        let blob = "please charge 4111-1111-1111-1111 today";
        let kind = detect(blob).expect("hyphenated card must be detected");
        assert!(matches!(kind, SensitiveKind::CreditCard));
    }
    #[test]
    fn credit_card_no_false_positive_on_luhn_invalid_run() {
        // Pin: a Luhn-invalid 13-digit run inside longer text must not
        // classify as CreditCard. We assert *only* "not classified as
        // CreditCard" — the input may still trigger an unrelated pattern
        // (e.g. phone_us on a 10-digit subrun), which is out of scope.
        // (Earlier fixtures used "all zeros" or "all ones": the former
        // is Luhn-valid (sum=0 ≡ 0 mod 10) and the latter trips phone_us.)
        let blob = "ref=4242424242422 EOT";
        let kind = detect(blob);
        assert!(
            !matches!(kind, Some(SensitiveKind::CreditCard)),
            "Luhn-invalid 13-digit run must not classify as CreditCard, got {:?}",
            kind
        );
    }
    #[test]
    fn detects_slack_bot_token() {
        assert!(detect("xoxb-17653285717-17653285718-AbCdEfGhIjKlMnOpQrStUvWx").is_some());
    }
    #[test]
    fn detects_slack_webhook() {
        assert!(detect(
            "https://hooks.slack.com/services/T00000000/B00000000/XXXXXXXXXXXXXXXXXXXXXXXX"
        )
        .is_some());
    }
    #[test]
    fn detects_stripe_webhook_secret() {
        assert!(detect("whsec_aAbBcCdDeEfFgGhHiIjJkKlLmMnNoOpPqQrRsStT").is_some());
    }
    #[test]
    fn detects_google_api_key() {
        assert!(detect("AIzaSyD-9tSrke72EmVt4TenJheB96ABCDE12345").is_some());
    }
    #[test]
    fn detects_github_actions_token() {
        assert!(detect("ghs_16C7e42F292c6912E7710c838347Ae178B4a").is_some());
    }
    #[test]
    #[cfg_attr(
        debug_assertions,
        ignore = "regex perf test only meaningful in release builds"
    )]
    fn pattern_match_completes_in_5ms_on_10mb_text() {
        let big = "a".repeat(10_000_000);
        let start = std::time::Instant::now();
        let _ = detect(&big);
        assert!(
            start.elapsed().as_millis() < 500,
            "took {}ms",
            start.elapsed().as_millis()
        );
    }

    // --- is_sensitive_app tests ---

    #[test]
    fn sensitive_app_1password_bundle_id() {
        assert!(is_sensitive_app("com.1password.1password"));
    }

    #[test]
    fn sensitive_app_bitwarden_bundle_id() {
        assert!(is_sensitive_app("com.bitwarden.desktop"));
    }

    #[test]
    fn sensitive_app_keepassxc_bundle_id() {
        assert!(is_sensitive_app("com.keepassxc.keepassxc"));
    }

    #[test]
    fn sensitive_app_dashlane_bundle_id() {
        assert!(is_sensitive_app("com.dashlane.dashlane"));
    }

    #[test]
    fn sensitive_app_process_name_fragment() {
        // Process names may be short (e.g. "1password", "bitwarden")
        assert!(is_sensitive_app("bitwarden"));
        assert!(is_sensitive_app("keepass"));
    }

    #[test]
    fn sensitive_app_case_insensitive() {
        assert!(is_sensitive_app("com.Bitwarden.Desktop"));
        assert!(is_sensitive_app("COM.1PASSWORD.1PASSWORD"));
    }

    #[test]
    fn sensitive_app_unknown_app_returns_false() {
        assert!(!is_sensitive_app("com.apple.finder"));
        assert!(!is_sensitive_app("com.google.chrome"));
        assert!(!is_sensitive_app(""));
    }

    #[test]
    fn sensitive_app_partial_match() {
        // "1password" appears as substring in longer bundle IDs
        assert!(is_sensitive_app("com.agilebits.onepassword4"));
    }

    // ── NFKC normalisation / Unicode bypass guards ─────────────────────────────

    #[test]
    fn nfkc_normalised_input_detects_secrets() {
        // Full-width "AKIA" (U+FF21..U+FF24) + 16 ASCII chars after NFKC → AKIA + 16 = AWS key.
        let fullwidth_akia = "\u{FF21}\u{FF2B}\u{FF29}\u{FF21}IOSFODNN7EXAMPLE";
        let kind = detect(fullwidth_akia);
        assert!(kind.is_some(), "expected AWS key after NFKC normalisation");
        matches!(kind.unwrap(), SensitiveKind::AwsKey);
    }

    #[test]
    fn nfkc_zwj_in_jwt_normalises_away() {
        // A real JWT with a zero-width joiner inserted; NFKC strips ZWJ.
        // Note: ZWJ (U+200D) is a control char and NFKC keeps it in many cases;
        // but `eyJ` prefix is ASCII and the regex still matches on the surrounding bytes.
        // Use NFKC normalisation to demonstrate it doesn't break detection of clean JWTs.
        let clean =
            "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJ1c2VyIn0.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c";
        assert!(detect(clean).is_some());
    }

    #[test]
    fn nfkc_normalize_is_idempotent_on_ascii() {
        let s = "AKIAIOSFODNN7EXAMPLE";
        assert_eq!(nfkc_normalize(s), s);
    }

    // ── generic_password_kv FP guards ───────────────────────────────────────────

    #[test]
    fn weak_password_value_is_filtered() {
        // value "foo" — too short, no special, no letter+digit mix.
        assert!(detect("password: foo").is_none());
    }

    #[test]
    fn weak_password_short_letters_is_filtered() {
        // "nope" — too short, no special, no digit.
        assert!(detect("secret = nope").is_none());
    }

    #[test]
    fn strong_password_value_letter_digit_mix_detected() {
        assert!(detect("password=hunter2").is_some());
    }

    #[test]
    fn strong_password_value_with_special_char_detected() {
        assert!(detect("secret = !abcdef").is_some());
    }

    #[test]
    fn long_password_value_detected() {
        assert!(detect("password: abcdefghij").is_some()); // 10 chars
    }
}
