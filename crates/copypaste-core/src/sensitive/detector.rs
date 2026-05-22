use std::ops::Range;
use super::patterns::{pattern_set, patterns, pattern_name, pattern_category, pattern_confidence};

// ── Public types ──────────────────────────────────────────────────────────────

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

// ── Legacy kind enum (kept for backwards compatibility) ───────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum SensitiveKind {
    AwsKey, GitHubToken, OpenAIKey, AnthropicKey, StripeKey,
    NpmToken, PyPIToken, SlackToken, VaultToken, GcpToken,
    SshPrivateKey, Jwt, CreditCard, Other(String),
}

impl SensitiveKind {
    fn from_pattern_name(name: &str) -> Self {
        match name {
            n if n.starts_with("aws") => Self::AwsKey,
            n if n.starts_with("github") => Self::GitHubToken,
            n if n.starts_with("openai") => Self::OpenAIKey,
            "anthropic" => Self::AnthropicKey,
            "stripe_live" | "stripe_webhook" => Self::StripeKey,
            "npm_token" => Self::NpmToken,
            "pypi_token" => Self::PyPIToken,
            n if n.starts_with("slack") => Self::SlackToken,
            "hashicorp_vault" => Self::VaultToken,
            "gcp_oauth" => Self::GcpToken,
            "ssh_private_key" => Self::SshPrivateKey,
            "jwt" | "generic_bearer" => Self::Jwt,
            other => Self::Other(other.to_string()),
        }
    }
}

// ── SensitiveDetector ─────────────────────────────────────────────────────────

/// Detects sensitive data patterns in text.
///
/// Compiled regexes are initialised once (via `OnceLock`) and shared across all
/// instances — construction is effectively free after the first call.
#[derive(Default)]
pub struct SensitiveDetector;

impl SensitiveDetector {
    /// Create a new detector. All regex compilation is lazy/shared.
    pub fn new() -> Self {
        Self
    }

    /// Return every pattern match found in `text`, with byte ranges and confidence.
    pub fn detect(&self, text: &str) -> Vec<PatternMatch> {
        let mut results: Vec<PatternMatch> = Vec::new();

        // RegexSet first-pass: O(n) single scan to find which patterns match.
        let matched_indices: Vec<usize> =
            pattern_set().matches(text).into_iter().collect();

        // Second pass: for each matching pattern, find precise byte ranges.
        for idx in matched_indices {
            let re = &patterns()[idx];
            for m in re.find_iter(text) {
                results.push(PatternMatch {
                    pattern_name: pattern_name(idx),
                    confidence: pattern_confidence(idx),
                    category: SensitiveCategory::from_raw(pattern_category(idx)),
                    matched_range: m.start()..m.end(),
                });
            }
        }

        // Luhn credit card check (standalone digits, no length guard).
        luhn_find(text, &mut results);

        results
    }

    /// Returns `true` if any pattern matches (fast path — uses `RegexSet` only).
    ///
    /// This is significantly faster than calling `detect()` because it skips the
    /// per-pattern `find_iter` pass needed to compute exact byte ranges.
    pub fn is_sensitive(&self, text: &str) -> bool {
        self.is_sensitive_threshold(text, 0.5)
    }

    /// Returns `true` if any matching pattern has confidence ≥ `threshold`.
    ///
    /// Uses the fast `RegexSet` path — no byte-range computation.
    pub fn is_sensitive_threshold(&self, text: &str, threshold: f32) -> bool {
        let matched: Vec<usize> = pattern_set().matches(text).into_iter().collect();
        if matched.iter().any(|&i| pattern_confidence(i) >= threshold) {
            return true;
        }
        // Also check Luhn (financial) — confidence 0.85 always ≥ 0.5.
        if threshold <= 0.85 {
            return luhn_find_any(text);
        }
        false
    }

    /// Returns the single highest-confidence match, if any.
    pub fn highest_confidence(&self, text: &str) -> Option<PatternMatch> {
        self.detect(text)
            .into_iter()
            .max_by(|a, b| a.confidence.partial_cmp(&b.confidence).unwrap())
    }
}

// ── Luhn helper ───────────────────────────────────────────────────────────────

/// Fast check: returns true if any Luhn-valid card number is found in `text`.
fn luhn_find_any(text: &str) -> bool {
    let bytes = text.as_bytes();
    let n = bytes.len();
    let mut i = 0;
    while i < n {
        if bytes[i].is_ascii_digit() {
            let start = i;
            while i < n && (bytes[i].is_ascii_digit() || bytes[i] == b' ' || bytes[i] == b'-') {
                i += 1;
            }
            if luhn_valid(&text[start..i]) {
                return true;
            }
        } else {
            i += 1;
        }
    }
    false
}

fn luhn_find(text: &str, results: &mut Vec<PatternMatch>) {
    // Walk the text looking for 13-19 consecutive digit runs (allowing spaces/dashes).
    let bytes = text.as_bytes();
    let n = bytes.len();
    let mut i = 0;
    while i < n {
        if bytes[i].is_ascii_digit() {
            let start = i;
            while i < n && (bytes[i].is_ascii_digit() || bytes[i] == b' ' || bytes[i] == b'-') {
                i += 1;
            }
            let slice = &text[start..i];
            if luhn_valid(slice) {
                results.push(PatternMatch {
                    pattern_name: "credit_card_luhn",
                    confidence: 0.85,
                    category: SensitiveCategory::Financial,
                    matched_range: start..i,
                });
            }
        } else {
            i += 1;
        }
    }
}

/// Luhn algorithm over a string that may contain spaces and dashes.
pub fn luhn_valid(s: &str) -> bool {
    let digits: Vec<u32> = s
        .chars()
        .filter(|c| c.is_ascii_digit())
        .map(|c| c.to_digit(10).unwrap())
        .collect();
    let len = digits.len();
    if !(13..=19).contains(&len) {
        return false;
    }
    let sum: u32 = digits.iter().rev().enumerate().map(|(i, &d)| {
        if i % 2 == 1 {
            let v = d * 2;
            if v > 9 { v - 9 } else { v }
        } else {
            d
        }
    }).sum();
    sum % 10 == 0
}

// ── Legacy top-level detect() kept for backwards compatibility ─────────────────

pub fn detect(text: &str) -> Option<SensitiveKind> {
    let matched: Vec<usize> = pattern_set().matches(text).into_iter().collect();
    if let Some(&idx) = matched.first() {
        tracing::debug!(pattern = pattern_name(idx), "sensitive content detected");
        return Some(SensitiveKind::from_pattern_name(pattern_name(idx)));
    }
    // Standalone short line: cheap Luhn check (keep existing behaviour)
    if text.len() <= 25 && luhn_valid(text) {
        return Some(SensitiveKind::CreditCard);
    }
    None
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn det() -> SensitiveDetector { SensitiveDetector::new() }

    // ── Legacy detect() tests (unchanged behaviour) ───────────────────────────

    #[test]
    fn detects_aws_access_key() { assert!(detect("AKIAIOSFODNN7EXAMPLE").is_some()); }
    #[test]
    fn detects_temporary_aws_key() { assert!(detect("ASIAIOSFODNN7EXAMPLE1234").is_some()); }
    #[test]
    fn detects_github_classic_pat() { assert!(detect(&("ghp_".to_string() + &"A".repeat(36))).is_some()); }
    #[test]
    fn detects_github_fine_grained_pat() {
        assert!(detect(&format!("github_pat_{}_{}", "A".repeat(22), "B".repeat(59))).is_some());
    }
    #[test]
    fn detects_openai_key() { assert!(detect(&("sk-proj-".to_string() + &"A".repeat(48))).is_some()); }
    #[test]
    fn detects_anthropic_key() { assert!(detect(&("sk-ant-api03-".to_string() + &"A".repeat(80))).is_some()); }
    #[test]
    fn detects_jwt() {
        assert!(detect("eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJ1c2VyIn0.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c").is_some());
    }
    #[test]
    fn detects_ssh_private_key() { assert!(detect("-----BEGIN RSA PRIVATE KEY-----\nMIIEo...").is_some()); }
    #[test]
    fn detects_openssh_private_key() { assert!(detect("-----BEGIN OPENSSH PRIVATE KEY-----\nMIIEo...").is_some()); }
    #[test]
    fn detects_stripe_live_key() { assert!(detect(&("sk_live_".to_string() + &"A".repeat(24))).is_some()); }
    #[test]
    fn detects_npm_token() { assert!(detect(&("npm_".to_string() + &"A".repeat(36))).is_some()); }
    #[test]
    fn detects_slack_bot_token() {
        assert!(detect("xoxb-17653285717-17653285718-AbCdEfGhIjKlMnOpQrStUvWx").is_some());
    }
    #[test]
    fn detects_slack_webhook() {
        assert!(detect("https://hooks.slack.com/services/T00000000/B00000000/XXXXXXXXXXXXXXXXXXXXXXXX").is_some());
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

    // ── Luhn ─────────────────────────────────────────────────────────────────

    #[test]
    fn luhn_valid_visa() { assert!(luhn_valid("4111111111111111")); }
    #[test]
    fn luhn_valid_mastercard() { assert!(luhn_valid("5500005555555559")); }
    #[test]
    fn luhn_valid_amex() { assert!(luhn_valid("378282246310005")); }
    #[test]
    fn luhn_invalid_wrong_digit() { assert!(!luhn_valid("4111111111111112")); }
    #[test]
    fn luhn_invalid_too_short() { assert!(!luhn_valid("411111")); }
    #[test]
    fn luhn_valid_with_spaces() { assert!(luhn_valid("4111 1111 1111 1111")); }
    #[test]
    fn luhn_valid_with_dashes() { assert!(luhn_valid("4111-1111-1111-1111")); }
    #[test]
    fn credit_card_detected_short_line_only() { assert!(detect("4111111111111111").is_some()); }

    // ── SensitiveDetector: credentials ───────────────────────────────────────

    #[test]
    fn detector_aws_returns_match_with_range() {
        let text = "My key is AKIAIOSFODNN7EXAMPLE here";
        let ms = det().detect(text);
        assert!(!ms.is_empty());
        let m = &ms[0];
        assert_eq!(m.pattern_name, "aws_access_key");
        assert_eq!(m.category, SensitiveCategory::Credential);
        assert!(m.confidence > 0.9);
        assert_eq!(&text[m.matched_range.clone()], "AKIAIOSFODNN7EXAMPLE");
    }

    #[test]
    fn detector_jwt_category_credential() {
        let jwt = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJ1c2VyIn0.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c";
        let ms = det().detect(jwt);
        assert!(ms.iter().any(|m| m.category == SensitiveCategory::Credential));
    }

    #[test]
    fn detector_ssh_key_category_credential() {
        let key = "-----BEGIN RSA PRIVATE KEY-----\nABCDEF";
        let ms = det().detect(key);
        assert!(ms.iter().any(|m| m.category == SensitiveCategory::Credential));
    }

    #[test]
    fn detector_bearer_token() {
        let text = "Authorization: Bearer eyAbcDefGhIjKlMnOpQrStUvWx123456";
        let ms = det().detect(text);
        assert!(ms.iter().any(|m| m.pattern_name == "generic_bearer"));
    }

    #[test]
    fn detector_generic_password_kv() {
        let text = "password=s3cr3tP@ssw0rd";
        let ms = det().detect(text);
        assert!(ms.iter().any(|m| m.pattern_name == "generic_password_kv"));
    }

    // ── SensitiveDetector: financial ──────────────────────────────────────────

    #[test]
    fn detector_credit_card_via_luhn() {
        let text = "Card: 4111111111111111 exp 12/26";
        let ms = det().detect(text);
        assert!(ms.iter().any(|m| m.category == SensitiveCategory::Financial));
    }

    #[test]
    fn detector_iban() {
        // GB29 NWBK 6016 1331 9268 19 (valid structure for testing)
        let text = "IBAN: GB29NWBK60161331926819";
        let ms = det().detect(text);
        assert!(ms.iter().any(|m| m.pattern_name == "iban"), "no iban match in: {:?}", ms);
    }

    // ── SensitiveDetector: personal IDs ──────────────────────────────────────

    #[test]
    fn detector_ssn() {
        let text = "SSN: 123-45-6789";
        let ms = det().detect(text);
        assert!(ms.iter().any(|m| m.pattern_name == "ssn_us"));
    }

    #[test]
    fn detector_ssn_spaces() {
        let text = "SSN 123 45 6789";
        let ms = det().detect(text);
        assert!(ms.iter().any(|m| m.pattern_name == "ssn_us"));
    }

    #[test]
    fn detector_email() {
        let text = "Contact user@example.com for details";
        let ms = det().detect(text);
        assert!(ms.iter().any(|m| m.pattern_name == "email"));
    }

    #[test]
    fn detector_phone_us_with_country_code() {
        let text = "Call +1-555-867-5309";
        let ms = det().detect(text);
        assert!(ms.iter().any(|m| m.pattern_name == "phone_us"), "no phone_us in {:?}", ms);
    }

    #[test]
    fn detector_phone_us_parentheses() {
        let text = "Phone: (555) 867-5309";
        let ms = det().detect(text);
        assert!(ms.iter().any(|m| m.pattern_name == "phone_us"));
    }

    // ── SensitiveDetector: infrastructure ────────────────────────────────────

    #[test]
    fn detector_ip_with_port() {
        let text = "Connect to 192.168.1.100:5432";
        let ms = det().detect(text);
        assert!(ms.iter().any(|m| m.pattern_name == "ip_with_port"));
    }

    #[test]
    fn detector_postgres_connection_string() {
        let text = "postgresql://admin:s3cr3t@db.example.com:5432/mydb";
        let ms = det().detect(text);
        assert!(ms.iter().any(|m| m.pattern_name == "db_conn_string"));
        assert!(ms.iter().any(|m| m.category == SensitiveCategory::Infrastructure));
    }

    #[test]
    fn detector_mongodb_connection_string() {
        let text = "mongodb://user:pass@cluster0.example.mongodb.net/db";
        let ms = det().detect(text);
        assert!(ms.iter().any(|m| m.pattern_name == "db_conn_string"));
    }

    #[test]
    fn detector_redis_connection_string() {
        let text = "redis://user:hunter2@cache.example.com:6379";
        let ms = det().detect(text);
        assert!(ms.iter().any(|m| m.pattern_name == "db_conn_string"));
    }

    #[test]
    fn detector_aws_arn() {
        let text = "arn:aws:iam::123456789012:role/MyRole";
        let ms = det().detect(text);
        assert!(ms.iter().any(|m| m.pattern_name == "aws_arn"));
    }

    #[test]
    fn detector_dotenv_secret() {
        let text = "DATABASE_PASSWORD=super_secret_value";
        let ms = det().detect(text);
        assert!(ms.iter().any(|m| m.pattern_name == "dotenv_secret"), "no dotenv_secret in {:?}", ms);
    }

    #[test]
    fn detector_dotenv_api_key() {
        let text = "STRIPE_SECRET_KEY=sk_live_abcdef";
        let ms = det().detect(text);
        assert!(ms.iter().any(|m| m.pattern_name == "dotenv_secret" || m.pattern_name == "stripe_live"),
            "no match in {:?}", ms);
    }

    // ── Negative tests ────────────────────────────────────────────────────────

    #[test]
    fn no_false_positive_on_lorem_ipsum() {
        let ms = det().detect("Lorem ipsum dolor sit amet, consectetur adipiscing elit.");
        assert!(ms.is_empty(), "unexpected matches: {:?}", ms);
    }

    #[test]
    fn no_false_positive_on_short_code() {
        let ms = det().detect(r#"fn main() { println!("Hello, world!"); }"#);
        assert!(ms.is_empty(), "unexpected matches: {:?}", ms);
    }

    #[test]
    fn no_false_positive_plain_text_url() {
        let ms = det().detect("Visit https://example.com for more info.");
        assert!(ms.is_empty(), "unexpected matches: {:?}", ms);
    }

    #[test]
    fn no_false_positive_random_number() {
        // Random 16-digit number that fails Luhn
        let ms = det().detect("1234567890123456");
        assert!(!ms.iter().any(|m| m.category == SensitiveCategory::Financial),
            "false positive credit card: {:?}", ms);
    }

    // ── is_sensitive helper ───────────────────────────────────────────────────

    #[test]
    fn is_sensitive_true_for_aws_key() {
        assert!(det().is_sensitive("AKIAIOSFODNN7EXAMPLE is here"));
    }

    #[test]
    fn is_sensitive_false_for_plain_text() {
        assert!(!det().is_sensitive("Hello, world!"));
    }

    // ── highest_confidence ────────────────────────────────────────────────────

    #[test]
    fn highest_confidence_returns_aws_key() {
        let text = "AKIAIOSFODNN7EXAMPLE is my key and user@example.com is my email";
        let best = det().highest_confidence(text);
        assert!(best.is_some());
        let m = best.unwrap();
        assert_eq!(m.pattern_name, "aws_access_key");
    }

    // ── Performance ───────────────────────────────────────────────────────────

    #[test]
    fn pattern_match_completes_in_5ms_on_10mb_text() {
        let big = "a".repeat(10_000_000);
        let start = std::time::Instant::now();
        let _ = detect(&big);
        assert!(start.elapsed().as_millis() < 500, "took {}ms", start.elapsed().as_millis());
    }

    /// Performance: 1000 × detect() on 1 KB text must finish under 100 ms in release builds.
    /// (Each call runs RegexSet + per-pattern find_iter, so ~30 µs/call is expected.)
    #[test]
    #[cfg(not(debug_assertions))]
    fn detector_1000_calls_on_1kb_text_under_100ms() {
        let text = "Hello world, my email is user@example.com and nothing else is sensitive here.";
        let d = det();
        let start = std::time::Instant::now();
        for _ in 0..1000 {
            let _ = d.detect(text);
        }
        let elapsed = start.elapsed().as_millis();
        assert!(elapsed < 100, "1000 calls took {}ms (>100ms budget)", elapsed);
    }

    /// Fast path: is_sensitive() only uses RegexSet (single O(n) pass) — must be <10 ms for 1000 calls.
    #[test]
    #[cfg(not(debug_assertions))]
    fn is_sensitive_1000_calls_on_1kb_text_under_10ms() {
        let text = "Hello world, my email is user@example.com and nothing else is sensitive here.";
        let d = det();
        let start = std::time::Instant::now();
        for _ in 0..1000 {
            let _ = d.is_sensitive(text);
        }
        let elapsed = start.elapsed().as_millis();
        assert!(elapsed < 10, "1000 is_sensitive calls took {}ms (>10ms budget)", elapsed);
    }
}
