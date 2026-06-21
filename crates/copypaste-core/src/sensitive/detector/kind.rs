use super::super::patterns::{pattern_name, pattern_set};
use super::fp::match_is_false_positive;
use super::luhn::contains_luhn_valid_card_run;
use super::normalize::nfkc_normalize;

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
    pub(super) fn from_pattern_name(name: &str) -> Self {
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

/// Free-function convenience wrapper around
/// [`super::engine::SensitiveDetector::is_sensitive_for_autowipe`].
///
/// Returns `true` only for high-confidence (>= 0.70) credential matches that
/// should trigger automatic expiry/wipe. Low-confidence heuristics (phone,
/// passport, email) are excluded so routine clipboard content is never silently
/// deleted.
pub fn is_sensitive_for_autowipe(text: &str) -> bool {
    super::engine::SensitiveDetector::new().is_sensitive_for_autowipe(text)
}

pub fn detect(text: &str) -> Option<SensitiveKind> {
    let normalised = nfkc_normalize(text);
    let candidate_indices: Vec<usize> = pattern_set().matches(&normalised).into_iter().collect();
    for &idx in &candidate_indices {
        // For generic_password_kv we must validate value strength to avoid FPs.
        if pattern_name(idx) == "generic_password_kv" {
            let re = &super::super::patterns::patterns()[idx];
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
