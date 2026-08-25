//! What the two TOML files may say, and the resolved shape handed to the
//! renderer. Data only: every rule about what a selection *must* say lives in
//! [`crate::refusals`].

use std::collections::BTreeMap;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Selection {
    pub source: Source,
    #[serde(default)]
    pub rules: Vec<SelectedRule>,
    #[serde(default)]
    pub overlay: Vec<OverlayRule>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub release: String,
    pub commit: String,
    pub license: String,
    pub config_url: String,
    pub license_url: String,
    pub config_sha256: String,
    pub license_sha256: String,
    pub vendored_config: String,
    pub vendored_license: String,
    pub generated_rust: String,
    pub generated_publication_redaction: String,
    pub publication_redaction_rules: Vec<String>,
    pub publication_redaction_decision: String,
    pub unsupported_context_decision: String,
    pub regex_dialect_decision: String,
    #[serde(default)]
    pub use_global_allowlist: bool,
    pub global_allowlist_decision: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectedRule {
    pub upstream_ids: Vec<String>,
    pub name: String,
    pub category: Category,
    pub confidence: f32,
    pub pattern_override: Option<String>,
    pub pattern_decision: Option<String>,
    pub entropy_override: Option<f64>,
    pub entropy_decision: Option<String>,
    pub keywords_override: Option<Vec<String>>,
    pub keywords_decision: Option<String>,
    #[serde(default = "yes")]
    pub use_rule_allowlists: bool,
    pub allowlist_decision: Option<String>,
    #[serde(default)]
    pub allowlist_regex_overrides: Vec<RegexOverride>,
    pub secret_shape_allowlist_from: Option<String>,
    pub secret_shape_allowlist_decision: Option<String>,
    pub placeholder_stopwords_from: Option<String>,
    pub placeholder_stopwords_minimum: Option<usize>,
    pub placeholder_stopwords_decision: Option<String>,
    #[serde(default)]
    pub never_auto_delete: bool,
    pub never_auto_delete_decision: Option<String>,
    #[serde(default)]
    pub anchor_only: bool,
    pub anchor_only_decision: Option<String>,
    #[serde(default)]
    pub validator: Validator,
    pub secret_group: Option<usize>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegexOverride {
    pub source: String,
    pub replacement: String,
    pub decision: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OverlayRule {
    pub name: String,
    pub category: Category,
    pub confidence: f32,
    pub pattern: String,
    #[serde(default)]
    pub validator: Validator,
    #[serde(default)]
    pub secret_group: usize,
    #[serde(default)]
    pub never_auto_delete: bool,
    pub never_auto_delete_decision: Option<String>,
    #[serde(default)]
    pub anchor_only: bool,
    pub anchor_only_decision: Option<String>,
    pub secret_shape_allowlist_from: Option<String>,
    pub secret_shape_allowlist_decision: Option<String>,
    pub placeholder_stopwords_from: Option<String>,
    pub placeholder_stopwords_minimum: Option<usize>,
    pub placeholder_stopwords_decision: Option<String>,
    pub entropy: Option<f64>,
    pub entropy_decision: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Credential,
    Financial,
    PersonalId,
    Infrastructure,
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Validator {
    #[default]
    None,
    ValueStrength,
    CardNumber,
    Iban,
    SsnStructure,
    PhoneShape,
}

#[derive(Debug, Deserialize)]
pub struct GitleaksConfig {
    #[allow(dead_code)]
    pub title: String,
    #[serde(rename = "minVersion")]
    #[allow(dead_code)]
    pub min_version: String,
    pub allowlist: Option<RawAllowlist>,
    pub rules: Vec<RawRule>,
}

#[derive(Debug, Deserialize)]
pub struct RawRule {
    pub id: String,
    #[allow(dead_code)]
    pub description: Option<String>,
    pub regex: Option<String>,
    #[serde(default)]
    pub entropy: f64,
    #[serde(default, rename = "secretGroup")]
    pub secret_group: usize,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default)]
    pub allowlists: Vec<RawAllowlist>,
    pub path: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, toml::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawAllowlist {
    #[allow(dead_code)]
    pub description: Option<String>,
    pub condition: Option<String>,
    #[serde(default)]
    pub commits: Vec<String>,
    #[serde(default)]
    pub paths: Vec<String>,
    #[serde(default, rename = "regexTarget")]
    pub regex_target: String,
    #[serde(default)]
    pub regexes: Vec<String>,
    #[serde(default)]
    pub stopwords: Vec<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, toml::Value>,
}

pub struct Inputs {
    pub selection: Selection,
    pub global_allowlists: Vec<Allowlist>,
    pub rules: Vec<Rule>,
    pub selected_ids: Vec<String>,
}

pub struct Rule {
    pub upstream_ids: Vec<String>,
    pub name: String,
    pub category: Category,
    pub confidence: f32,
    pub pattern: String,
    pub validator: Validator,
    pub secret_group: usize,
    pub never_auto_delete: bool,
    pub anchor_only: bool,
    pub entropy: Option<f64>,
    pub keywords: Vec<String>,
    pub allowlists: Vec<Allowlist>,
}

#[derive(Clone)]
pub struct Allowlist {
    pub condition: Condition,
    pub target: Target,
    pub regexes: Vec<String>,
    pub stopwords: Vec<String>,
    pub stopword_minimum: usize,
}

#[derive(Clone, Copy)]
pub enum Condition {
    Any,
    All,
}

#[derive(Clone, Copy)]
pub enum Target {
    Secret,
    Match,
    Line,
}

fn yes() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use super::*;

    #[test]
    fn pinned_config_exposes_the_global_content_allowlist() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .unwrap();
        let text = fs::read_to_string(root.join("config/gitleaks/gitleaks.toml")).unwrap();
        let config: GitleaksConfig = toml::from_str(&text).unwrap();
        let allowlist = config.allowlist.expect("global allowlist table");
        assert_eq!(allowlist.regexes.len(), 13);
        assert_eq!(allowlist.stopwords.len(), 2);
    }
}
