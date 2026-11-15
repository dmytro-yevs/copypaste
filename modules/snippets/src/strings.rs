//! Runtime strings shown in the app, in the twelve app locales. The table is
//! generated from the app's own translations (`scripts/generate-strings.py`).
//! Messages the app never had (host and storage errors) stay English, as the
//! runtime kept them before snippets became a module.

use crate::strings_table::{LOCALES, TABLE};

/// The strings of one locale (`uk`, `pt-BR`, `zh_Hans`); English otherwise.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Strings {
    column: usize,
    language: &'static str,
}

impl Strings {
    pub(crate) fn new(locale: &str) -> Self {
        let language = locale
            .split(['-', '_'])
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        let column = LOCALES
            .iter()
            .position(|candidate| *candidate == language)
            .unwrap_or(0);
        Self {
            column,
            language: LOCALES[column],
        }
    }

    /// The message `key`, or the key itself when the table lacks it.
    pub(crate) fn get(&self, key: &'static str) -> &'static str {
        self.lookup(key).unwrap_or(key)
    }

    fn lookup(&self, key: &str) -> Option<&'static str> {
        TABLE
            .binary_search_by(|(candidate, _)| (*candidate).cmp(key))
            .ok()
            .map(|at| TABLE[at].1[self.column])
    }

    /// `key` with each `{name}` replaced.
    pub(crate) fn fill(&self, key: &'static str, values: &[(&str, &str)]) -> String {
        fill(self.get(key), values)
    }

    /// The variant of an ICU `select` message, `other` when unknown.
    pub(crate) fn select(&self, key: &str, option: &str, values: &[(&str, &str)]) -> String {
        let text = self
            .lookup(&format!("{key}#{option}"))
            .or_else(|| self.lookup(&format!("{key}#other")))
            .unwrap_or_default();
        fill(text, values)
    }

    /// The variant of an ICU `plural` message for `count`, with `{count}`
    /// and `values` filled in.
    pub(crate) fn plural(&self, key: &str, count: usize, values: &[(&str, &str)]) -> String {
        let text = self
            .lookup(&format!("{key}#={count}"))
            .or_else(|| self.lookup(&format!("{key}#{}", self.category(count))))
            .or_else(|| self.lookup(&format!("{key}#other")))
            .unwrap_or_default();
        let count = count.to_string();
        let mut all = vec![("count", count.as_str())];
        all.extend_from_slice(values);
        fill(text, &all)
    }

    /// The CLDR plural category of an integer in this language.
    fn category(&self, count: usize) -> &'static str {
        let (last, last_two) = (count % 10, count % 100);
        let few = (2..=4).contains(&last) && !(12..=14).contains(&last_two);
        match self.language {
            "ja" | "ko" | "zh" => "other",
            "fr" if count <= 1 => "one",
            "uk" if last == 1 && last_two != 11 => "one",
            "uk" if few => "few",
            "uk" => "many",
            "pl" if count == 1 => "one",
            "pl" if few => "few",
            "pl" => "many",
            _ if count == 1 => "one",
            _ => "other",
        }
    }
}

/// `template` with each `{name}` replaced.
pub(crate) fn fill(template: &str, values: &[(&str, &str)]) -> String {
    values
        .iter()
        .fold(template.to_owned(), |text, (name, value)| {
            text.replace(&format!("{{{name}}}"), value)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locales_fall_back_and_plurals_pick_the_language_category() {
        assert_eq!(Strings::new("uk_UA").get("snippetsScopeAll"), "Усі");
        assert_eq!(Strings::new("xx").get("snippetsScopeAll"), "All");
        assert!(TABLE.windows(2).all(|pair| pair[0].0 < pair[1].0));
        let uk = Strings::new("uk");
        assert_eq!(
            uk.plural("snippetsDeletedTitle", 3, &[]),
            "Видалено 3 фрагменти"
        );
        assert_eq!(
            uk.plural("snippetsDeletedTitle", 5, &[]),
            "Видалено 5 фрагментів"
        );
        assert_eq!(
            Strings::new("en").plural("snippetsSelected", 1, &[]),
            "1 selected"
        );
        assert_eq!(
            Strings::new("en").select("snippetsLint", "unknownModifier", &[("detail", "x")]),
            "Unknown modifier “x”."
        );
    }
}
