//! Generated sensitive rules and structural acceptance checks.
//!
//! Edit `config/sensitive-rules.toml`, then run the generator. The generated
//! module is the indivisible data-table exemption to the source-size limit.

pub(super) use super::rules_generated::{GLOBAL_ALLOWLISTS, RULES};

#[cfg(test)]
pub(super) fn rule(name: &str) -> &'static super::spec::RuleSpec {
    RULES
        .iter()
        .find(|rule| rule.name == name)
        .unwrap_or_else(|| panic!("no rule named {name}"))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::sensitive::engine::test_support::{detector, fired};
    use crate::sensitive::finding::AUTOWIPE_CONFIDENCE_FLOOR;
    use crate::sensitive::rules_generated::{
        GITLEAKS_COMMIT, GITLEAKS_VERSION, SELECTED_GITLEAKS_RULE_IDS, VENDORED_CONFIG_SHA256,
    };
    use crate::sensitive::spec::{AllowlistTarget, Category};

    #[test]
    fn generated_source_and_rule_id_parity_are_pinned() {
        assert_eq!(GITLEAKS_VERSION, "v8.30.1");
        assert_eq!(GITLEAKS_COMMIT, "83d9cd684c87d95d656c1458ef04895a7f1cbd8e");
        assert_eq!(
            VENDORED_CONFIG_SHA256,
            "e163e53b9e7e8a8511e77271e2b323ed057759542a6d988258afe3a1fa329caf"
        );
        let generated: BTreeSet<_> = RULES
            .iter()
            .flat_map(|rule| rule.upstream_ids.iter().copied())
            .collect();
        let selected: BTreeSet<_> = SELECTED_GITLEAKS_RULE_IDS.iter().copied().collect();
        assert_eq!(generated, selected);
        assert_eq!(generated.len(), SELECTED_GITLEAKS_RULE_IDS.len());
    }

    #[test]
    fn rule_names_are_unique() {
        let names: BTreeSet<_> = RULES.iter().map(|rule| rule.name).collect();
        assert_eq!(names.len(), RULES.len());
    }

    #[test]
    fn fp_risk_patterns_below_autowipe_floor() {
        for name in [
            "discord_bot_token",
            "twilio_signing_key_sid",
            "iban",
            "ssn_us",
            "generic_bearer",
            "http_basic_auth",
            "ip_with_port",
            "email",
            "phone_us",
            "passport",
            "aws_arn",
        ] {
            assert!(
                rule(name).confidence < AUTOWIPE_CONFIDENCE_FLOOR,
                "{name} is at or above the auto-wipe floor"
            );
        }
    }

    #[test]
    fn no_rule_sits_exactly_on_the_floor() {
        for rule in RULES {
            assert!(
                (rule.confidence - AUTOWIPE_CONFIDENCE_FLOOR).abs() > f32::EPSILON,
                "{} sits exactly on the floor",
                rule.name
            );
            assert!((0.0..=1.0).contains(&rule.confidence), "{}", rule.name);
        }
    }

    #[test]
    fn prefixed_token_rules_are_word_anchored() {
        for name in [
            "sendgrid_api_key",
            "terraform_cloud_token",
            "cloudflare_api_token",
            "twilio_signing_key_sid",
            "discord_bot_token",
            "gitlab_pat",
        ] {
            assert!(rule(name).pattern.contains(r"\b"), "{name} lost its anchor");
        }
        assert!(!rule("aws_access_key").pattern.ends_with(r"\b"));
        assert!(!rule("azure_storage_key").pattern.contains(r"\b"));
    }

    #[test]
    fn p2_ozzt_rules_are_high_confidence_credentials() {
        for name in [
            "azure_storage_key",
            "azure_sas_token",
            "gcp_service_account_key",
            "cloudflare_api_token",
            "sendgrid_api_key",
            "terraform_cloud_token",
        ] {
            let rule = rule(name);
            assert_eq!(rule.category, Category::Credential, "{name}");
            assert!(rule.confidence >= 0.90, "{name}");
        }
    }

    #[test]
    fn upstream_generic_rule_keeps_entropy_stopwords_and_allowlists() {
        let rule = rule("generic_api_key");
        assert_eq!(rule.entropy, Some(3.5));
        assert!(rule.keywords.contains(&"credential"));
        assert!(
            rule.allowlists
                .iter()
                .any(|allowlist| !allowlist.stopwords.is_empty())
        );
        assert!(
            rule.allowlists
                .iter()
                .any(|allowlist| allowlist.target == AllowlistTarget::Line)
        );
    }

    #[test]
    fn local_overlay_rules_have_no_upstream_identity() {
        for name in [
            "credit_card",
            "iban",
            "ssn_us",
            "email",
            "phone_us",
            "passport",
        ] {
            assert!(rule(name).upstream_ids.is_empty(), "{name}");
        }
    }

    #[test]
    fn azure_sas_markers_match_in_either_order() {
        let detector = detector();
        let raw_sig = format!("{}+/=", "A".repeat(41));
        let encoded_sig = format!("{}%2B%2F%3D", "b".repeat(41));
        for sas in [
            format!("sv=2024-11-04&sig={raw_sig}"),
            format!("sig={encoded_sig}&amp;sv=2024-11-04"),
            format!("sv=2024-11-04&sp=rw&se=2030-01-01&sig={encoded_sig}"),
            format!("sig={raw_sig}&amp;sp=rw&amp;se=2030-01-01&amp;sv=2024-11-04"),
        ] {
            assert!(fired(&detector, &sas, "azure_sas_token"), "{sas}");
            assert!(detector.may_auto_wipe(&sas), "{sas}");
        }
    }

    #[test]
    fn azure_sas_requires_both_exact_markers_in_one_query() {
        let detector = detector();
        let signature = "A".repeat(40);
        for benign in [
            "sv=2024-11-04&sig=too-short".to_string(),
            format!("sig={signature}&sv=not-a-version"),
            format!("signature={signature}&sv=2024-11-04"),
            format!("sig={signature} sv=2024-11-04"),
            format!("sig={signature}#fragment&sv=2024-11-04"),
        ] {
            assert!(!fired(&detector, &benign, "azure_sas_token"), "{benign}");
        }
    }
}
