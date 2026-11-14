//! Translated manifest strings. English fields stay the fallback; a locale map
//! only overrides strings that already exist in the manifest.
use crate::{ModuleFieldValue, ModuleManifest};
use std::collections::BTreeMap;

/// `locale -> string key -> translation`. Keys name manifest strings:
/// `title`, `description`, `commands/<id>/title`, `commands/<id>/description`,
/// `commands/<id>/arguments/<field>/title`, `preferences/<field>/title`, and
/// `.../<field>/options/<choice>/title` for choice titles.
pub type ModuleLocalizations = BTreeMap<String, BTreeMap<String, String>>;

pub const MAX_LOCALES: usize = 64;
const TITLE_BYTES: usize = 160;
const DESCRIPTION_BYTES: usize = 2048;

type Visitor<'a> = dyn FnMut(&str, &mut String, usize) + 'a;

fn visit(manifest: &mut ModuleManifest, f: &mut Visitor<'_>) {
    f("title", &mut manifest.title, TITLE_BYTES);
    f("description", &mut manifest.description, DESCRIPTION_BYTES);
    visit_fields("preferences", &mut manifest.preferences, f);
    for command in &mut manifest.commands {
        let key = format!("commands/{}", command.id);
        f(&format!("{key}/title"), &mut command.title, TITLE_BYTES);
        f(
            &format!("{key}/description"),
            &mut command.description,
            DESCRIPTION_BYTES,
        );
        visit_fields(&format!("{key}/arguments"), &mut command.arguments, f);
    }
}

fn visit_fields(prefix: &str, fields: &mut [crate::ModuleField], f: &mut Visitor<'_>) {
    for field in fields {
        let key = format!("{prefix}/{}", field.id);
        f(&format!("{key}/title"), &mut field.title, TITLE_BYTES);
        if let ModuleFieldValue::Choices { options, .. } = &mut field.value {
            for option in options {
                f(
                    &format!("{key}/options/{}/title", option.id),
                    &mut option.title,
                    TITLE_BYTES,
                );
            }
        }
    }
}

fn valid_locale(locale: &str) -> bool {
    let mut parts = locale.split('-');
    locale.len() <= 35
        && parts.next().is_some_and(|language| {
            (2..=3).contains(&language.len()) && language.bytes().all(|b| b.is_ascii_lowercase())
        })
        && parts.all(|part| {
            (1..=8).contains(&part.len()) && part.bytes().all(|b| b.is_ascii_alphanumeric())
        })
}

pub(crate) fn validate(manifest: &ModuleManifest) -> Result<(), String> {
    if manifest.localizations.is_empty() {
        return Ok(());
    }
    let mut limits = BTreeMap::new();
    visit(&mut manifest.clone(), &mut |key, _, limit| {
        limits.insert(key.to_owned(), limit);
    });
    if manifest.localizations.len() > MAX_LOCALES
        || manifest.localizations.iter().any(|(locale, strings)| {
            !valid_locale(locale)
                || strings.iter().any(|(key, value)| {
                    limits.get(key).is_none_or(|limit| {
                        value.len() > *limit || (*limit == TITLE_BYTES && value.trim().is_empty())
                    })
                })
        })
    {
        return Err("The module translations are invalid.".into());
    }
    Ok(())
}

impl ModuleManifest {
    /// A copy with strings for `locale` (`uk`, `uk-UA`, or `uk_UA`): the exact
    /// tag first, then each shorter prefix, then the English originals.
    #[must_use]
    pub fn localized(&self, locale: &str) -> Self {
        let mut localized = self.clone();
        let requested = locale.replace('_', "-").to_ascii_lowercase();
        let mut chain = Vec::new();
        let mut tag = requested.as_str();
        loop {
            if let Some(strings) = self
                .localizations
                .iter()
                .find(|(candidate, _)| candidate.eq_ignore_ascii_case(tag))
                .map(|(_, strings)| strings)
            {
                chain.push(strings);
            }
            match tag.rfind('-') {
                Some(end) => tag = &tag[..end],
                None => break,
            }
        }
        visit(&mut localized, &mut |key, value, _| {
            if let Some(translation) = chain.iter().find_map(|strings| strings.get(key)) {
                value.clone_from(translation);
            }
        });
        localized
    }
}
