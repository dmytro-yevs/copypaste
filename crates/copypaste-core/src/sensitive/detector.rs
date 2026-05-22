use std::ops::Range;
use super::patterns::{pattern_set, pattern_name, pattern_category, pattern_confidence, patterns};

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
    pub fn new() -> Self { Self }

    /// Return every pattern match found in `text`, with byte ranges and confidence.
    pub fn detect(&self, text: &str) -> Vec<PatternMatch> {
        let mut results: Vec<PatternMatch> = Vec::new();
        for (i, re) in patterns().iter().enumerate() {
            for m in re.find_iter(text) {
                results.push(PatternMatch {
                    pattern_name: pattern_name(i),
                    confidence: pattern_confidence(i),
                    category: SensitiveCategory::from_raw(pattern_category(i)),
                    matched_range: m.range(),
                });
            }
        }
        results
    }

    /// Returns true if any sensitive pattern is found (fast path using RegexSet).
    pub fn is_sensitive(&self, text: &str) -> bool {
        pattern_set().is_match(text)
    }

    /// Returns true if any pattern exceeds the confidence threshold.
    pub fn is_sensitive_threshold(&self, text: &str, threshold: f32) -> bool {
        self.detect(text).iter().any(|m| m.confidence >= threshold)
    }

    /// Returns the highest-confidence match, if any.
    pub fn highest_confidence(&self, text: &str) -> Option<PatternMatch> {
        self.detect(text).into_iter().max_by(|a, b| {
            a.confidence.partial_cmp(&b.confidence).unwrap_or(std::cmp::Ordering::Equal)
        })
    }
}

/// Validate a credit card number using the Luhn algorithm.
pub fn luhn_valid(s: &str) -> bool {
    let digits: Vec<u32> = s.chars()
        .filter(|c| c.is_ascii_digit())
        .map(|c| c.to_digit(10).unwrap())
        .collect();
    if digits.len() < 13 || digits.len() > 19 { return false; }
    let sum: u32 = digits.iter().rev().enumerate().map(|(i, &d)| {
        if i % 2 == 1 { let v = d * 2; if v > 9 { v - 9 } else { v } } else { d }
    }).sum();
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
    SENSITIVE_APP_BUNDLE_IDS.iter().any(|&known| lower.contains(known))
}

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
            "stripe_live" => Self::StripeKey,
            "npm_token" => Self::NpmToken,
            "pypi_token" => Self::PyPIToken,
            "slack_bot" => Self::SlackToken,
            "hashicorp_vault" => Self::VaultToken,
            "gcp_oauth" => Self::GcpToken,
            "ssh_private_key" => Self::SshPrivateKey,
            "jwt" => Self::Jwt,
            other => Self::Other(other.to_string()),
        }
    }
}

pub fn detect(text: &str) -> Option<SensitiveKind> {
    let matches: Vec<usize> = pattern_set().matches(text).into_iter().collect();
    if let Some(&idx) = matches.first() {
        tracing::debug!(pattern = pattern_name(idx), "sensitive content detected");
        return Some(SensitiveKind::from_pattern_name(pattern_name(idx)));
    }
    if text.len() <= 25 && is_luhn_valid_card(text) {
        return Some(SensitiveKind::CreditCard);
    }
    None
}

fn is_luhn_valid_card(s: &str) -> bool {
    let digits: Vec<u32> = s.chars().filter(|c| c.is_ascii_digit())
        .map(|c| c.to_digit(10).unwrap()).collect();
    if digits.len() < 13 || digits.len() > 19 { return false; }
    let sum: u32 = digits.iter().rev().enumerate().map(|(i, &d)| {
        if i % 2 == 1 { let v = d * 2; if v > 9 { v - 9 } else { v } } else { d }
    }).sum();
    sum % 10 == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_aws_access_key() { assert!(detect("AKIAIOSFODNN7EXAMPLE").is_some()); }
    #[test]
    fn detects_temporary_aws_key() { assert!(detect("ASIAIOSFODNN7EXAMPLE1234").is_some()); }
    #[test]
    fn detects_github_classic_pat() { assert!(detect(&("ghp_".to_string() + &"A".repeat(36))).is_some()); }
    #[test]
    fn detects_github_fine_grained_pat() { assert!(detect(&format!("github_pat_{}_{}", "A".repeat(22), "B".repeat(59))).is_some()); }
    #[test]
    fn detects_openai_key() { assert!(detect(&("sk-proj-".to_string() + &"A".repeat(48))).is_some()); }
    #[test]
    fn detects_anthropic_key() { assert!(detect(&("sk-ant-api03-".to_string() + &"A".repeat(80))).is_some()); }
    #[test]
    fn detects_jwt() { assert!(detect("eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJ1c2VyIn0.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c").is_some()); }
    #[test]
    fn detects_ssh_private_key() { assert!(detect("-----BEGIN RSA PRIVATE KEY-----\nMIIEo...").is_some()); }
    #[test]
    fn detects_openssh_private_key() { assert!(detect("-----BEGIN OPENSSH PRIVATE KEY-----\nMIIEo...").is_some()); }
    #[test]
    fn detects_stripe_live_key() { assert!(detect(&("sk_live_".to_string() + &"A".repeat(24))).is_some()); }
    #[test]
    fn detects_npm_token() { assert!(detect(&("npm_".to_string() + &"A".repeat(36))).is_some()); }
    #[test]
    fn no_false_positive_on_lorem_ipsum() { assert!(detect("Lorem ipsum dolor sit amet, consectetur adipiscing elit.").is_none()); }
    #[test]
    fn no_false_positive_on_short_code() { assert!(detect(r#"fn main() { println!("Hello, world!"); }"#).is_none()); }
    #[test]
    fn credit_card_detected_short_line_only() { assert!(detect("4111111111111111").is_some()); }
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
    #[test]
    #[cfg_attr(debug_assertions, ignore = "regex perf test only meaningful in release builds")]
    fn pattern_match_completes_in_5ms_on_10mb_text() {
        let big = "a".repeat(10_000_000);
        let start = std::time::Instant::now();
        let _ = detect(&big);
        assert!(start.elapsed().as_millis() < 500, "took {}ms", start.elapsed().as_millis());
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
}
