use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use anyhow::{bail, Context, Result};
use regex::Regex;
use serde::Deserialize;
use sha2::{Digest, Sha256};

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
    Luhn,
    Iban,
    SsnStructure,
    PhoneShape,
}

#[derive(Debug, Deserialize)]
struct GitleaksConfig {
    #[allow(dead_code)]
    title: String,
    #[serde(rename = "minVersion")]
    #[allow(dead_code)]
    min_version: String,
    allowlist: Option<RawAllowlist>,
    rules: Vec<RawRule>,
}

#[derive(Debug, Deserialize)]
struct RawRule {
    id: String,
    #[allow(dead_code)]
    description: Option<String>,
    regex: Option<String>,
    #[serde(default)]
    entropy: f64,
    #[serde(default, rename = "secretGroup")]
    secret_group: usize,
    #[serde(default)]
    keywords: Vec<String>,
    #[serde(default)]
    allowlists: Vec<RawAllowlist>,
    path: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(flatten)]
    extra: BTreeMap<String, toml::Value>,
}

#[derive(Debug, Clone, Deserialize)]
struct RawAllowlist {
    #[allow(dead_code)]
    description: Option<String>,
    condition: Option<String>,
    #[serde(default)]
    commits: Vec<String>,
    #[serde(default)]
    paths: Vec<String>,
    #[serde(default, rename = "regexTarget")]
    regex_target: String,
    #[serde(default)]
    regexes: Vec<String>,
    #[serde(default)]
    stopwords: Vec<String>,
    #[serde(flatten)]
    extra: BTreeMap<String, toml::Value>,
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

pub fn read_selection(root: &Path) -> Result<Selection> {
    let path = root.join("config/sensitive-rules.toml");
    let text = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("parse {}", path.display()))
}

pub fn load_inputs(root: &Path) -> Result<Inputs> {
    let selection = read_selection(root)?;
    verify_file(root, &selection.source.vendored_config, &selection.source.config_sha256)?;
    verify_file(root, &selection.source.vendored_license, &selection.source.license_sha256)?;
    if selection.source.unsupported_context_decision.trim().is_empty() {
        bail!("source.unsupported_context_decision must explain path/commit handling");
    }
    if selection.source.regex_dialect_decision.trim().is_empty() {
        bail!("source.regex_dialect_decision must explain RE2-to-Rust handling");
    }
    if selection.source.license != "MIT" {
        bail!("unreviewed upstream license {}", selection.source.license);
    }
    if !selection.source.use_global_allowlist
        && selection.source.global_allowlist_decision.trim().is_empty()
    {
        bail!("source.global_allowlist_decision must explain the disabled global allowlist");
    }
    let config_path = root.join(&selection.source.vendored_config);
    let config_text = fs::read_to_string(&config_path)?;
    let config: GitleaksConfig = toml::from_str(&config_text)
        .with_context(|| format!("parse {}", config_path.display()))?;
    resolve(selection, config)
}

fn resolve(selection: Selection, config: GitleaksConfig) -> Result<Inputs> {
    let by_id: BTreeMap<_, _> = config.rules.iter().map(|rule| (rule.id.as_str(), rule)).collect();
    if by_id.len() != config.rules.len() {
        bail!("vendored Gitleaks config contains duplicate rule IDs");
    }
    let global_allowlists = if selection.source.use_global_allowlist {
        config
            .allowlist
            .as_ref()
            .map(|allowlist| resolve_allowlist(allowlist, &[]))
            .transpose()?
            .flatten()
            .into_iter()
            .collect()
    } else {
        Vec::new()
    };
    let mut names = BTreeSet::new();
    let mut selected_ids = BTreeSet::new();
    let mut rules = Vec::new();

    for selected in &selection.rules {
        validate_decisions(selected)?;
        if selected.upstream_ids.is_empty() {
            bail!("{} selects no upstream rule ID", selected.name);
        }
        let mut sources = Vec::new();
        for id in &selected.upstream_ids {
            if !selected_ids.insert(id.clone()) {
                bail!("upstream rule ID {id} is selected more than once");
            }
            let source = by_id
                .get(id.as_str())
                .with_context(|| format!("selected Gitleaks rule {id} is absent"))?;
            validate_source_rule(source)?;
            sources.push(*source);
        }
        let pattern = match &selected.pattern_override {
            Some(pattern) => pattern.clone(),
            None if sources.len() == 1 => sources[0].regex.clone().unwrap(),
            None => bail!("{} merges multiple upstream rules without pattern_override", selected.name),
        };
        let entropy = effective_entropy(selected, &sources)?;
        let keywords = selected
            .keywords_override
            .clone()
            .unwrap_or_else(|| merge_keywords(&sources));
        let mut allowlists = Vec::new();
        if selected.use_rule_allowlists {
            validate_allowlist_overrides(selected, &sources)?;
            for source in &sources {
                for raw in &source.allowlists {
                    if let Some(allowlist) =
                        resolve_allowlist(raw, &selected.allowlist_regex_overrides)?
                    {
                        allowlists.push(allowlist);
                    }
                }
            }
        }
        let secret_group = selected
            .secret_group
            .unwrap_or_else(|| sources.first().map_or(0, |source| source.secret_group));
        validate_rule(&selected.name, selected.confidence, &pattern, secret_group)?;
        insert_name(&mut names, &selected.name)?;
        rules.push(Rule {
            upstream_ids: selected.upstream_ids.clone(),
            name: selected.name.clone(),
            category: selected.category,
            confidence: selected.confidence,
            pattern,
            validator: selected.validator,
            secret_group,
            entropy,
            keywords,
            allowlists,
        });
    }

    for overlay in &selection.overlay {
        validate_rule(&overlay.name, overlay.confidence, &overlay.pattern, overlay.secret_group)?;
        insert_name(&mut names, &overlay.name)?;
        rules.push(Rule {
            upstream_ids: Vec::new(),
            name: overlay.name.clone(),
            category: overlay.category,
            confidence: overlay.confidence,
            pattern: overlay.pattern.clone(),
            validator: overlay.validator,
            secret_group: overlay.secret_group,
            entropy: None,
            keywords: Vec::new(),
            allowlists: Vec::new(),
        });
    }

    Ok(Inputs {
        selection,
        global_allowlists,
        rules,
        selected_ids: selected_ids.into_iter().collect(),
    })
}

fn validate_decisions(rule: &SelectedRule) -> Result<()> {
    require_decision(&rule.name, "pattern", rule.pattern_override.is_some(), &rule.pattern_decision)?;
    require_decision(&rule.name, "entropy", rule.entropy_override.is_some(), &rule.entropy_decision)?;
    require_decision(&rule.name, "keywords", rule.keywords_override.is_some(), &rule.keywords_decision)?;
    if !rule.use_rule_allowlists && rule.allowlist_decision.as_deref().unwrap_or("").trim().is_empty() {
        bail!("{} disables rule allowlists without allowlist_decision", rule.name);
    }
    Ok(())
}

fn require_decision(name: &str, field: &str, changed: bool, decision: &Option<String>) -> Result<()> {
    if changed != decision.as_deref().is_some_and(|value| !value.trim().is_empty()) {
        bail!("{name} must pair {field}_override with {field}_decision");
    }
    Ok(())
}

fn validate_source_rule(rule: &RawRule) -> Result<()> {
    if rule.regex.is_none() {
        bail!("selected Gitleaks rule {} has no content regex", rule.id);
    }
    if rule.path.is_some() {
        bail!("selected Gitleaks rule {} requires path semantics", rule.id);
    }
    if !rule.extra.is_empty() {
        bail!("selected Gitleaks rule {} has unsupported fields: {:?}", rule.id, rule.extra.keys());
    }
    let _ = &rule.tags;
    Ok(())
}

fn effective_entropy(selected: &SelectedRule, sources: &[&RawRule]) -> Result<Option<f64>> {
    if let Some(value) = selected.entropy_override {
        return Ok((value > 0.0).then_some(value));
    }
    let first = sources[0].entropy;
    if sources.iter().any(|source| source.entropy != first) {
        bail!("{} merges differing entropy thresholds without an override", selected.name);
    }
    Ok((first > 0.0).then_some(first))
}

fn merge_keywords(sources: &[&RawRule]) -> Vec<String> {
    let mut seen = BTreeSet::new();
    sources
        .iter()
        .flat_map(|source| source.keywords.iter())
        .filter(|keyword| seen.insert(keyword.to_lowercase()))
        .cloned()
        .collect()
}

fn validate_allowlist_overrides(selected: &SelectedRule, sources: &[&RawRule]) -> Result<()> {
    let available = sources
        .iter()
        .flat_map(|rule| rule.allowlists.iter())
        .flat_map(|allowlist| allowlist.regexes.iter())
        .collect::<Vec<_>>();
    for replacement in &selected.allowlist_regex_overrides {
        if replacement.decision.trim().is_empty() {
            bail!("{} has an allowlist regex override without a decision", selected.name);
        }
        let count = available
            .iter()
            .filter(|pattern| pattern.as_str() == replacement.source)
            .count();
        if count != 1 {
            bail!(
                "{} allowlist override source matched {count} upstream regexes",
                selected.name
            );
        }
    }
    Ok(())
}

fn resolve_allowlist(
    raw: &RawAllowlist,
    overrides: &[RegexOverride],
) -> Result<Option<Allowlist>> {
    if !raw.extra.is_empty() {
        bail!("allowlist has unsupported fields: {:?}", raw.extra.keys());
    }
    let condition = match raw.condition.as_deref().unwrap_or("OR") {
        "OR" => Condition::Any,
        "AND" => Condition::All,
        other => bail!("unsupported allowlist condition {other}"),
    };
    let has_context = !raw.paths.is_empty() || !raw.commits.is_empty();
    if has_context && matches!(condition, Condition::All) {
        return Ok(None);
    }
    let target = match raw.regex_target.as_str() {
        "" => Target::Secret,
        "match" => Target::Match,
        "line" => Target::Line,
        other => bail!("unsupported allowlist regexTarget {other}"),
    };
    let regexes = raw
        .regexes
        .iter()
        .map(|pattern| {
            overrides
                .iter()
                .find(|replacement| replacement.source == *pattern)
                .map_or_else(|| pattern.clone(), |replacement| replacement.replacement.clone())
        })
        .collect::<Vec<_>>();
    for pattern in &regexes {
        Regex::new(pattern).with_context(|| format!("compile allowlist regex {pattern:?}"))?;
    }
    if raw.regexes.is_empty() && raw.stopwords.is_empty() {
        return Ok(None);
    }
    Ok(Some(Allowlist {
        condition,
        target,
        regexes,
        stopwords: raw.stopwords.clone(),
    }))
}

fn validate_rule(name: &str, confidence: f32, pattern: &str, secret_group: usize) -> Result<()> {
    if !(0.0..=1.0).contains(&confidence) {
        bail!("{name} confidence is outside 0..=1");
    }
    let regex = Regex::new(pattern).with_context(|| format!("compile rule {name}"))?;
    let capture_count = regex.captures_len().saturating_sub(1);
    if secret_group > capture_count {
        bail!("{name} secret_group {secret_group} exceeds {capture_count} captures");
    }
    Ok(())
}

fn insert_name(names: &mut BTreeSet<String>, name: &str) -> Result<()> {
    if !names.insert(name.to_owned()) {
        bail!("duplicate generated rule name {name}");
    }
    Ok(())
}

fn verify_file(root: &Path, relative: &str, expected: &str) -> Result<()> {
    let path = root.join(relative);
    let bytes = fs::read(&path).with_context(|| format!("read {}", path.display()))?;
    let actual = sha256(&bytes);
    if actual != expected {
        bail!("{} SHA-256 mismatch: expected {expected}, got {actual}", path.display());
    }
    Ok(())
}

pub fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn yes() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pinned_config_exposes_the_global_content_allowlist() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(2).unwrap();
        let text = fs::read_to_string(root.join("config/gitleaks/gitleaks.toml")).unwrap();
        let config: GitleaksConfig = toml::from_str(&text).unwrap();
        let allowlist = config.allowlist.expect("global allowlist table");
        assert_eq!(allowlist.regexes.len(), 13);
        assert_eq!(allowlist.stopwords.len(), 2);
    }
}
