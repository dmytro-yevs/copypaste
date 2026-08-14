//! What the generator refuses to build.
//!
//! Every rule here exists because something was spelled once and did the wrong
//! thing silently, so the check is that the spelling is impossible rather than
//! that the reviewer noticed.

use std::collections::BTreeSet;

use anyhow::{bail, Context, Result};
use regex::Regex;

use crate::schema::{OverlayRule, RawRule, SelectedRule, Validator};

pub fn validate_decisions(rule: &SelectedRule) -> Result<()> {
    require_decision(
        &rule.name,
        "pattern",
        rule.pattern_override.is_some(),
        &rule.pattern_decision,
    )?;
    require_decision(
        &rule.name,
        "entropy",
        rule.entropy_override.is_some(),
        &rule.entropy_decision,
    )?;
    require_decision(
        &rule.name,
        "keywords",
        rule.keywords_override.is_some(),
        &rule.keywords_decision,
    )?;
    require_decision(
        &rule.name,
        "secret_shape_allowlist",
        rule.secret_shape_allowlist_from.is_some(),
        &rule.secret_shape_allowlist_decision,
    )?;
    require_decision(
        &rule.name,
        "placeholder_stopwords",
        rule.placeholder_stopwords_from.is_some(),
        &rule.placeholder_stopwords_decision,
    )?;
    require_decision(
        &rule.name,
        "never_auto_delete",
        rule.never_auto_delete,
        &rule.never_auto_delete_decision,
    )?;
    require_decision(
        &rule.name,
        "anchor_only",
        rule.anchor_only,
        &rule.anchor_only_decision,
    )?;
    refuse_inert_fields(
        &rule.name,
        rule.anchor_only,
        rule.never_auto_delete,
        rule.placeholder_stopwords_from.as_ref(),
        rule.placeholder_stopwords_minimum,
    )?;
    if !rule.use_rule_allowlists
        && rule
            .allowlist_decision
            .as_deref()
            .unwrap_or("")
            .trim()
            .is_empty()
    {
        bail!(
            "{} disables rule allowlists without allowlist_decision",
            rule.name
        );
    }
    // Upstream tunes a threshold for a repository scanner, which prefers a false
    // positive; this detector deletes, so it prefers a false negative (§5.5).
    // `cloudflare_api_token` inherited 2.0 with no number and no decision
    // anywhere in that file, and deleted a 40-character README template at 2.233
    // (DMY-162). A rule whose value is the whole question states its own.
    if matches!(rule.validator, Validator::ValueStrength) && rule.entropy_override.is_none() {
        bail!(
            "{} gates on its captured value and may not inherit an upstream entropy threshold",
            rule.name
        );
    }
    Ok(())
}

/// A local rule has no upstream anything to inherit, so every field it sets is
/// its own to justify.
pub fn validate_overlay(overlay: &OverlayRule) -> Result<()> {
    // Refusing deletion for a rule above the floor is a product decision about a
    // whole class of user data, so it is not spellable without one.
    require_decision(
        &overlay.name,
        "never_auto_delete",
        overlay.never_auto_delete,
        &overlay.never_auto_delete_decision,
    )?;
    // The threshold is the whole gate between a README example and a credential
    // for the context-anchored rules.
    require_decision(
        &overlay.name,
        "entropy",
        overlay.entropy.is_some(),
        &overlay.entropy_decision,
    )?;
    // The same refusal `validate_decisions` applies to upstream-sourced rules.
    // `generic_password_kv` stated no threshold at all and deleted twelve of
    // twelve ordinary `.env` templates (DMY-162); an overlay is the spelling
    // that had no guard.
    if matches!(overlay.validator, Validator::ValueStrength) && overlay.entropy.is_none() {
        bail!(
            "{} gates on its captured value and must state an entropy threshold",
            overlay.name
        );
    }
    require_decision(
        &overlay.name,
        "secret_shape_allowlist",
        overlay.secret_shape_allowlist_from.is_some(),
        &overlay.secret_shape_allowlist_decision,
    )?;
    require_decision(
        &overlay.name,
        "placeholder_stopwords",
        overlay.placeholder_stopwords_from.is_some(),
        &overlay.placeholder_stopwords_decision,
    )?;
    require_decision(
        &overlay.name,
        "anchor_only",
        overlay.anchor_only,
        &overlay.anchor_only_decision,
    )?;
    refuse_inert_fields(
        &overlay.name,
        overlay.anchor_only,
        overlay.never_auto_delete,
        overlay.placeholder_stopwords_from.as_ref(),
        overlay.placeholder_stopwords_minimum,
    )?;
    Ok(())
}

/// The two refusals a selected rule and an overlay share.
///
/// `anchor_only` asks what a *deletable* match licenses, and a restricted rule
/// never reaches that side of the gate; spelling both reads as a second refusal
/// and is none. A minimum with no list to count is a number that does nothing.
fn refuse_inert_fields(
    name: &str,
    anchor_only: bool,
    never_auto_delete: bool,
    stopwords_from: Option<&String>,
    minimum: Option<usize>,
) -> Result<()> {
    if anchor_only && never_auto_delete {
        bail!("{name} is never_auto_delete, so anchor_only decides nothing");
    }
    if minimum.is_some() && stopwords_from.is_none() {
        bail!("{name} sets a placeholder stopword minimum with no list to count");
    }
    Ok(())
}

fn require_decision(
    name: &str,
    field: &str,
    changed: bool,
    decision: &Option<String>,
) -> Result<()> {
    if changed
        != decision
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
    {
        bail!("{name} must pair {field} with its decision");
    }
    Ok(())
}

pub fn validate_source_rule(rule: &RawRule) -> Result<()> {
    if rule.regex.is_none() {
        bail!("selected Gitleaks rule {} has no content regex", rule.id);
    }
    if rule.path.is_some() {
        bail!("selected Gitleaks rule {} requires path semantics", rule.id);
    }
    if !rule.extra.is_empty() {
        bail!(
            "selected Gitleaks rule {} has unsupported fields: {:?}",
            rule.id,
            rule.extra.keys()
        );
    }
    let _ = &rule.tags;
    Ok(())
}

pub fn validate_allowlist_overrides(selected: &SelectedRule, sources: &[&RawRule]) -> Result<()> {
    let available = sources
        .iter()
        .flat_map(|rule| rule.allowlists.iter())
        .flat_map(|allowlist| allowlist.regexes.iter())
        .collect::<Vec<_>>();
    for replacement in &selected.allowlist_regex_overrides {
        if replacement.decision.trim().is_empty() {
            bail!(
                "{} has an allowlist regex override without a decision",
                selected.name
            );
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

pub fn validate_rule(
    name: &str,
    confidence: f32,
    pattern: &str,
    secret_group: usize,
) -> Result<()> {
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

pub fn insert_name(names: &mut BTreeSet<String>, name: &str) -> Result<()> {
    if !names.insert(name.to_owned()) {
        bail!("duplicate generated rule name {name}");
    }
    Ok(())
}
