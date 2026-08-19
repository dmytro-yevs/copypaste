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
    use crate::sensitive::spec::{AllowlistTarget, Category, RuleSpec, Validator};

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
            "generic_api_key",
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
        assert!(rule
            .allowlists
            .iter()
            .any(|allowlist| !allowlist.stopwords.is_empty()));
        assert!(rule
            .allowlists
            .iter()
            .any(|allowlist| allowlist.target == AllowlistTarget::Line));
    }

    /// A context anchor proves which field matched, never that the value is a
    /// credential. These are above the floor on the strength of the anchor
    /// alone, so each must capture its value and gate that value on both its
    /// strength and its randomness — never on the whole match, which carries the
    /// anchor, and `AWS_SECRET_ACCESS_KEY=` contains `secret`.
    ///
    /// Each number is pinned to its value, not to a floor. `cloudflare_api_token`
    /// had no number of its own and inherited upstream's 2.0, which a floor test
    /// accepted while the rule deleted a 40-character README template at 2.233
    /// (DMY-162). §5.6 states how each was measured.
    #[test]
    fn context_anchored_rules_gate_their_captured_value() {
        for (name, entropy) in [
            ("azure_storage_key", 4.8),
            ("cloudflare_api_token", 4.3),
            ("aws_secret_access_key", 4.3),
            ("dotenv_secret", 4.4),
            // `password=` and `api_key=` are the same evidence at the same
            // strength, and this rule stated no threshold at all. Its number is
            // an order lower because its values include human-chosen passwords:
            // §9.1's `password=hunter2` measures 2.807 and fixes the ceiling, so
            // 2.0 reaches the repetitive templates and no further (§5.6).
            ("generic_password_kv", 2.0),
            // The `api_key` family, split out of the rule above and carrying
            // its number unchanged: the value population is the same one.
            ("api_key_kv", 2.0),
        ] {
            let rule = rule(name);
            assert!(rule.secret_group > 0, "{name} does not capture its value");
            assert_eq!(rule.validator, Validator::ValueStrength, "{name}");
            assert_eq!(rule.entropy, Some(entropy), "{name}");
            assert!(rule.never_auto_delete, "{name} may still delete");
        }
    }

    /// No rule may separate a placeholder from a credential by matching *one*
    /// word against the value. Both spellings of that — a `contains` stopword
    /// list and a `starts_with` prefix list — suppress the rule outright, and a
    /// suppressed 0.90-0.99 rule is a credential that is neither flagged, masked
    /// nor kept out of the index. That is the failure §5.6's fail-closed
    /// amendment exists for, and randomness answers the question instead.
    #[test]
    fn context_anchored_rules_carry_no_word_list_over_their_value() {
        for name in [
            "azure_storage_key",
            "aws_secret_access_key",
            "dotenv_secret",
            "generic_password_kv",
            "api_key_kv",
        ] {
            for allowlist in rule(name).allowlists {
                assert!(
                    allowlist.target != AllowlistTarget::Secret || allowlist.stopwords.is_empty(),
                    "{name} suppresses its value by word match"
                );
            }
        }
    }

    /// The one exception, and what makes it safe: the list is gitleaks' own,
    /// governed by `VENDORED_CONFIG_SHA256` rather than restated here, and it
    /// suppresses only on a *count* of distinct markers. One marker is what
    /// §9.1 binds as still detected — a real token containing or beginning with
    /// `todo`, `your`, `dummy` or `sample`. The minimum is measured against this
    /// rule's own value population, which is machine-issued and fixed at 40
    /// characters; `api_key_kv`'s was not, and the list took real values with it
    /// (§5.6, DMY-162).
    #[test]
    fn the_counted_word_list_is_vendored_and_above_one() {
        let counted: Vec<_> = rule("cloudflare_api_token")
            .allowlists
            .iter()
            .filter(|allowlist| !allowlist.stopwords.is_empty())
            .collect();
        assert_eq!(counted.len(), 1);
        assert_eq!(counted[0].target, AllowlistTarget::Secret);
        assert_eq!(counted[0].stopword_minimum, 3);
        assert_eq!(counted[0].stopwords.len(), 1_446, "the list is restated");
        assert!(counted[0].regexes.is_empty(), "a second gate was borrowed");
        // Borrowed, not copied: the same words upstream applies to these lines.
        let upstream = rule("generic_api_key")
            .allowlists
            .iter()
            .find(|allowlist| !allowlist.stopwords.is_empty())
            .expect("generic_api_key keeps its vendored stopwords");
        assert_eq!(counted[0].stopwords, upstream.stopwords);
    }

    /// Which rules a restricted neighbour may overrule. A keyword and a value of
    /// no shape is one judgement read twice; a unique literal is separate
    /// evidence and still deletes over the same bytes (§4.2, DMY-162). Pinned as
    /// a set, because the failure was a veto written over every rule at once.
    ///
    /// `heroku_api_key` is the shape this band cannot protect: `anchor_only`
    /// withholds only where a restricted match covers the same bytes, and a bare
    /// `heroku …: <UUID>` has no neighbour, so the rule deleted it. The band is
    /// for a rule that must still delete *somewhere*; a rule whose value proves
    /// nothing anywhere belongs in the restricted band instead (DMY-162).
    #[test]
    fn the_anchor_only_band_is_exactly_the_rules_with_no_shape_of_their_own() {
        let mut anchor_only: Vec<_> = RULES
            .iter()
            .filter(|rule| rule.anchor_only)
            .map(|rule| rule.name)
            .collect();
        anchor_only.sort_unstable();
        assert_eq!(anchor_only, ["generic_api_key"]);
        assert!(rule("heroku_api_key").never_auto_delete);
        for name in [
            "github_classic_pat",
            "hashicorp_vault",
            "jwt",
            "slack_token",
            "openai_legacy",
            "aws_access_key",
            "private_key",
            "db_conn_string",
        ] {
            assert!(!rule(name).anchor_only, "{name} lost its own evidence");
        }
    }

    /// The one question randomness cannot answer: a value whose characters are
    /// nearly all distinct measures like a credential because by that gate it
    /// *is* one. `MY_API_TOKEN=sk_test_abcdefghijklmnopqrstuvwx` measures 4.515
    /// and was deleted (DMY-162), while `stripe_live` refuses the same `sk_test_`
    /// prefix on purpose. Gitleaks answers it with a shape rather than a word
    /// list, and this asserts the literal is *its* one — borrowed from the
    /// vendored config, not a second copy that can drift from it.
    ///
    /// The four are the rules whose values are machine-issued, where a value
    /// drawn without a digit or a symbol is 1 in 280 to 1 in 57 million. The two
    /// keyword-KV rules are excluded: their values are what a person typed, for
    /// which all-letter is ordinary, and both are `never_auto_delete`, so the
    /// guard prevented no deletion and only sent a real password or key to
    /// `clipboard_fts` (§5.6, I4).
    #[test]
    fn context_anchored_rules_carry_the_vendored_secret_shape_guard() {
        for name in [
            "azure_storage_key",
            "cloudflare_api_token",
            "aws_secret_access_key",
            "dotenv_secret",
        ] {
            assert_eq!(
                secret_shape(rule(name)),
                Some(&["^[a-zA-Z_.-]+$"][..]),
                "{name}"
            );
        }
        for name in ["generic_password_kv", "api_key_kv"] {
            assert_eq!(secret_shape(rule(name)), None, "{name}");
        }
        // Upstream's own capture group admits `=`, so this rule sees
        // `<86 letters>==` where the allowlist was written for the value alone
        // and matched nothing — the one spelling the other five reject and this
        // rule deleted (DMY-162). Widened here and nowhere else.
        assert_eq!(
            secret_shape(rule("generic_api_key")),
            Some(&["^[a-zA-Z_.-]+={0,3}$"][..])
        );
    }

    fn secret_shape(spec: &'static RuleSpec) -> Option<&'static [&'static str]> {
        spec.allowlists
            .iter()
            .find(|allowlist| {
                allowlist.target == AllowlistTarget::Secret && allowlist.stopwords.is_empty()
            })
            .map(|allowlist| allowlist.regexes)
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
