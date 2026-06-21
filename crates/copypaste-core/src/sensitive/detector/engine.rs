use super::super::patterns::{
    pattern_category, pattern_confidence, pattern_name, pattern_set, patterns,
};
use super::fp::match_is_false_positive;
use super::luhn::contains_luhn_valid_card_run;
use super::normalize::nfkc_normalize;
use std::ops::Range;

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
        self.detect_normalised(&normalised)
    }

    /// Detect over an *already* NFKC-normalised string. Hot-path entry that
    /// skips re-normalisation; callers must pass a string already run through
    /// [`nfkc_normalize`]. Returned byte ranges are over `normalised`.
    ///
    /// Public so IPC/preview callers that already hold a normalised string (e.g.
    /// `history_page`, which normalises once to map byte→char offsets) can detect
    /// without a redundant second NFKC pass over the same text.
    pub fn detect_normalised(&self, normalised: &str) -> Vec<PatternMatch> {
        let mut results: Vec<PatternMatch> = Vec::new();
        for (i, re) in patterns().iter().enumerate() {
            for m in re.find_iter(normalised) {
                let range = m.range();
                if match_is_false_positive(i, m.as_str(), normalised, &range) {
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
        let ps = pattern_set();
        // Guard: if the RegexSet degraded to empty (all patterns failed to
        // compile), the fast-path would silently return false for every input.
        // Log an error and fall back to the full per-pattern detect() path so
        // sensitive content is still caught.
        if ps.is_empty() {
            tracing::error!(
                "sensitive pattern_set is empty (regex compile failure); \
                 falling back to full detect() path"
            );
            return !self.detect_normalised(&normalised).is_empty();
        }
        let matches: Vec<usize> = ps.matches(&normalised).into_iter().collect();
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
        // Only generic_password_kv candidates remain — validate at least one is
        // strong. `normalised` is already NFKC-normalised, so use the inner
        // entry to avoid a redundant second normalisation pass on the hot path.
        !self.detect_normalised(&normalised).is_empty()
    }

    /// Returns true if any pattern exceeds the confidence threshold.
    pub fn is_sensitive_threshold(&self, text: &str, threshold: f32) -> bool {
        self.detect(text).iter().any(|m| m.confidence >= threshold)
    }

    /// Returns true only if the text contains a **high-confidence** credential
    /// match (confidence >= 0.70) that warrants automatic expiry / wipe.
    ///
    /// This is the **correct gate for the auto-wipe / `sensitive_ttl` path**.
    /// Low-confidence patterns (phone_us 0.55, passport 0.55, email 0.60,
    /// IBAN 0.65, SSN 0.65, discord_bot_token 0.65, twilio_signing_key_sid 0.65,
    /// generic_bearer 0.65, ip_with_port 0.65) are intentionally excluded so
    /// routine phone numbers, bank details, config IPs, or placeholder strings
    /// never trigger silent data deletion.
    ///
    /// High-confidence examples that DO trigger (>= 0.70):
    ///   AWS keys (0.99), JWTs (0.95), OpenAI/Anthropic keys, SSH private keys,
    ///   Stripe/GitHub/npm tokens, Vault tokens (0.95), credit cards (Luhn),
    ///   SendGrid (0.99), Terraform Cloud (0.99), GCP SA key (0.99).
    pub fn is_sensitive_for_autowipe(&self, text: &str) -> bool {
        /// Minimum confidence for a match to trigger automatic expiry/wipe.
        const AUTOWIPE_CONFIDENCE_FLOOR: f32 = 0.70;
        let normalised = nfkc_normalize(text);
        // Fast path: RegexSet tells us which patterns fired; then check confidence.
        let candidate_indices: Vec<usize> =
            pattern_set().matches(&normalised).into_iter().collect();
        if candidate_indices.is_empty() {
            // No regex match at all — check credit cards via Luhn (they bypass
            // the pattern set and have implicit confidence 0.99).
            return contains_luhn_valid_card_run(&normalised);
        }
        for &idx in &candidate_indices {
            if pattern_confidence(idx) < AUTOWIPE_CONFIDENCE_FLOOR {
                continue; // below floor — skip (phone_us 0.55, passport 0.55, email 0.60, ip_with_port 0.65)
            }
            // For generic_password_kv (0.75 >= floor) we still require value strength.
            if pattern_name(idx) == "generic_password_kv" {
                let re = &patterns()[idx];
                if let Some(m) = re.find(&normalised) {
                    if match_is_false_positive(idx, m.as_str(), &normalised, &m.range()) {
                        continue;
                    }
                }
            }
            return true;
        }
        // Patterns fired but all were below the floor — still check Luhn cards.
        contains_luhn_valid_card_run(&normalised)
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
