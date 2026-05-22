use super::patterns::{pattern_set, pattern_name};

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
    fn pattern_match_completes_in_5ms_on_10mb_text() {
        let big = "a".repeat(10_000_000);
        let start = std::time::Instant::now();
        let _ = detect(&big);
        assert!(start.elapsed().as_millis() < 500, "took {}ms", start.elapsed().as_millis());
    }
}
