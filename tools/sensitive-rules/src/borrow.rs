//! Taking a shape, a word list, a threshold or a keyword set out of the
//! vendored gitleaks config, so the pinned checksum governs it and the two
//! spellings cannot drift apart.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{bail, Context, Result};
use regex::Regex;

use crate::schema::{
    Allowlist, Condition, RawAllowlist, RawRule, RegexOverride, SelectedRule, Target,
};

/// The one secret-target, regex-only allowlist an upstream rule carries.
pub fn borrowed_secret_shape(
    name: &str,
    from: &str,
    by_id: &BTreeMap<&str, &RawRule>,
) -> Result<Allowlist> {
    let source = by_id
        .get(from)
        .with_context(|| format!("{name} borrows a secret allowlist from absent rule {from}"))?;
    let mut found = Vec::new();
    for raw in &source.allowlists {
        let shapes_the_secret = raw.regex_target.is_empty()
            && !raw.regexes.is_empty()
            && raw.stopwords.is_empty()
            && raw.paths.is_empty()
            && raw.commits.is_empty();
        if !shapes_the_secret {
            continue;
        }
        if let Some(allowlist) = resolve_allowlist(raw, &[])? {
            found.push(allowlist);
        }
    }
    if found.len() != 1 {
        bail!(
            "{name} borrowing from {from} matched {} secret-shape allowlists",
            found.len()
        );
    }
    Ok(found.remove(0))
}

/// The one stopword-carrying allowlist an upstream rule declares. Only the words
/// travel: the sibling regex in that entry suppresses on a different target, and
/// a second suppressor the borrowing rule has not measured may not come along.
///
/// `minimum` is how many distinct words the value must carry. One is upstream's
/// own reading; above one is what separates a prose template from a credential
/// that happens to spell a marker, and it needs its own measurement (§5.6).
pub fn borrowed_placeholder_stopwords(
    name: &str,
    from: &str,
    minimum: Option<usize>,
    by_id: &BTreeMap<&str, &RawRule>,
) -> Result<Allowlist> {
    let minimum = minimum.unwrap_or(1);
    if minimum == 0 {
        bail!("{name} placeholder stopwords must require at least one match");
    }
    let source = by_id
        .get(from)
        .with_context(|| format!("{name} borrows placeholder stopwords from absent rule {from}"))?;
    let found: Vec<_> = source
        .allowlists
        .iter()
        .filter(|raw| !raw.stopwords.is_empty() && raw.paths.is_empty() && raw.commits.is_empty())
        .collect();
    if found.len() != 1 {
        bail!(
            "{name} borrowing from {from} matched {} stopword allowlists",
            found.len()
        );
    }
    // Verbatim, in upstream's order. The engine lowercases and deduplicates at
    // construction, so distinctness is owned there; reordering here would make
    // the borrow a *copy* that no longer matches the rule it was taken from, and
    // the generated table would carry it twice.
    Ok(Allowlist {
        condition: Condition::Any,
        target: Target::Secret,
        regexes: Vec::new(),
        stopwords: found[0].stopwords.clone(),
        stopword_minimum: minimum,
    })
}

pub fn resolve_allowlist(
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
                .map_or_else(
                    || pattern.clone(),
                    |replacement| replacement.replacement.clone(),
                )
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
        stopword_minimum: 1,
    }))
}

pub fn effective_entropy(selected: &SelectedRule, sources: &[&RawRule]) -> Result<Option<f64>> {
    if let Some(value) = selected.entropy_override {
        return Ok((value > 0.0).then_some(value));
    }
    let first = sources[0].entropy;
    if sources.iter().any(|source| source.entropy != first) {
        bail!(
            "{} merges differing entropy thresholds without an override",
            selected.name
        );
    }
    Ok((first > 0.0).then_some(first))
}

pub fn merge_keywords(sources: &[&RawRule]) -> Vec<String> {
    let mut seen = BTreeSet::new();
    sources
        .iter()
        .flat_map(|source| source.keywords.iter())
        .filter(|keyword| seen.insert(keyword.to_lowercase()))
        .cloned()
        .collect()
}
