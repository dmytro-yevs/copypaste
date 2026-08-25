//! Reading the selection and the vendored config, and resolving the two into
//! the [`Inputs`] the renderer emits.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};

use crate::borrow::{
    borrowed_placeholder_stopwords, borrowed_secret_shape, effective_entropy, merge_keywords,
    resolve_allowlist,
};
use crate::refusals::{
    insert_name, validate_allowlist_overrides, validate_decisions, validate_overlay, validate_rule,
    validate_source_rule,
};
use crate::schema::{GitleaksConfig, Inputs, Rule, Selection};

pub fn read_selection(root: &Path) -> Result<Selection> {
    let path = root.join("config/sensitive-rules.toml");
    let text = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("parse {}", path.display()))
}

pub fn load_inputs(root: &Path) -> Result<Inputs> {
    let selection = read_selection(root)?;
    verify_file(
        root,
        &selection.source.vendored_config,
        &selection.source.config_sha256,
    )?;
    verify_file(
        root,
        &selection.source.vendored_license,
        &selection.source.license_sha256,
    )?;
    if selection
        .source
        .unsupported_context_decision
        .trim()
        .is_empty()
    {
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
    let config: GitleaksConfig =
        toml::from_str(&config_text).with_context(|| format!("parse {}", config_path.display()))?;
    let inputs = resolve(selection, config)?;
    validate_publication_redaction(&inputs)?;
    Ok(inputs)
}

fn validate_publication_redaction(inputs: &Inputs) -> Result<()> {
    let source = &inputs.selection.source;
    if source.publication_redaction_decision.trim().is_empty() {
        bail!("source.publication_redaction_decision must explain the publication boundary");
    }
    if source.publication_redaction_rules.is_empty() {
        bail!("source.publication_redaction_rules must select at least one rule");
    }
    let mut selected = BTreeSet::new();
    for name in &source.publication_redaction_rules {
        if !selected.insert(name) {
            bail!("publication redaction rule {name} is selected more than once");
        }
        let rule = inputs
            .rules
            .iter()
            .find(|rule| rule.name == *name)
            .with_context(|| format!("publication redaction rule {name} is absent"))?;
        if rule.secret_group != 0 {
            bail!("publication redaction rule {name} does not redact its complete match");
        }
        for unsupported in ["(?i", "(?m", "(?s", "(?-", "(?P<", "\\p{"] {
            if rule.pattern.contains(unsupported) {
                bail!("publication redaction rule {name} uses non-ECMAScript syntax {unsupported}");
            }
        }
    }
    Ok(())
}

fn resolve(selection: Selection, config: GitleaksConfig) -> Result<Inputs> {
    let by_id: BTreeMap<_, _> = config
        .rules
        .iter()
        .map(|rule| (rule.id.as_str(), rule))
        .collect();
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
            None => bail!(
                "{} merges multiple upstream rules without pattern_override",
                selected.name
            ),
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
        if let Some(from) = &selected.secret_shape_allowlist_from {
            allowlists.push(borrowed_secret_shape(&selected.name, from, &by_id)?);
        }
        if let Some(from) = &selected.placeholder_stopwords_from {
            allowlists.push(borrowed_placeholder_stopwords(
                &selected.name,
                from,
                selected.placeholder_stopwords_minimum,
                &by_id,
            )?);
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
            never_auto_delete: selected.never_auto_delete,
            anchor_only: selected.anchor_only,
            entropy,
            keywords,
            allowlists,
        });
    }

    for overlay in &selection.overlay {
        validate_rule(
            &overlay.name,
            overlay.confidence,
            &overlay.pattern,
            overlay.secret_group,
        )?;
        validate_overlay(overlay)?;
        let mut allowlists = Vec::new();
        if let Some(from) = &overlay.secret_shape_allowlist_from {
            allowlists.push(borrowed_secret_shape(&overlay.name, from, &by_id)?);
        }
        if let Some(from) = &overlay.placeholder_stopwords_from {
            allowlists.push(borrowed_placeholder_stopwords(
                &overlay.name,
                from,
                overlay.placeholder_stopwords_minimum,
                &by_id,
            )?);
        }
        insert_name(&mut names, &overlay.name)?;
        rules.push(Rule {
            upstream_ids: Vec::new(),
            name: overlay.name.clone(),
            category: overlay.category,
            confidence: overlay.confidence,
            pattern: overlay.pattern.clone(),
            validator: overlay.validator,
            secret_group: overlay.secret_group,
            never_auto_delete: overlay.never_auto_delete,
            anchor_only: overlay.anchor_only,
            entropy: overlay.entropy,
            keywords: Vec::new(),
            allowlists,
        });
    }

    Ok(Inputs {
        selection,
        global_allowlists,
        rules,
        selected_ids: selected_ids.into_iter().collect(),
    })
}

fn verify_file(root: &Path, relative: &str, expected: &str) -> Result<()> {
    let path = root.join(relative);
    let bytes = fs::read(&path).with_context(|| format!("read {}", path.display()))?;
    let actual = sha256(&bytes);
    if actual != expected {
        bail!(
            "{} SHA-256 mismatch: expected {expected}, got {actual}",
            path.display()
        );
    }
    Ok(())
}

pub fn sha256(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_is_lowercase_hex() {
        let digest = sha256(b"");

        assert_eq!(
            digest,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(digest.len(), 64);
        assert!(digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)));
    }
}
