//! Case transforms. Mapping uses ICU4X compiled data in the root locale, so
//! output is identical on every platform; UPPERCASE and lowercase can opt
//! into the device locale (Turkish dotted and dotless i, for example).

mod code;
mod simple;
mod words;

use icu_casemap::options::{LeadingAdjustment, TitlecaseOptions, TrailingCase};
use icu_casemap::CaseMapper;

use crate::catalogue::Catalogue;

pub(crate) fn register(catalogue: &mut Catalogue) {
    for def in [
        &simple::UPPER,
        &simple::LOWER,
        &words::TITLE,
        &words::SENTENCE,
        &code::CAMEL,
        &code::PASCAL,
        &code::SNAKE,
        &code::KEBAB,
        &code::CONSTANT,
        &simple::SWAP,
    ] {
        catalogue.add(def);
    }
}

/// The root locale; `und` always parses, so `None` never happens.
const ROOT: &str = "und";

/// Lowercases `text` in the language of the BCP 47 `tag`, or in the root
/// locale when there is no tag or it does not parse.
fn lower_in(text: &str, tag: Option<&str>) -> String {
    let language = tag
        .and_then(|t| t.parse().ok())
        .or_else(|| ROOT.parse().ok());
    match language {
        Some(language) => CaseMapper::new()
            .lowercase_to_string(text, &language)
            .into_owned(),
        None => text.to_owned(),
    }
}

/// Uppercases like [`lower_in`] lowercases.
fn upper_in(text: &str, tag: Option<&str>) -> String {
    let language = tag
        .and_then(|t| t.parse().ok())
        .or_else(|| ROOT.parse().ok());
    match language {
        Some(language) => CaseMapper::new()
            .uppercase_to_string(text, &language)
            .into_owned(),
        None => text.to_owned(),
    }
}

fn lower(text: &str) -> String {
    lower_in(text, None)
}

fn upper(text: &str) -> String {
    upper_in(text, None)
}

/// Titlecases the first character of `word` and lowercases the rest.
fn capitalize(word: &str) -> String {
    let mut options = TitlecaseOptions::default();
    options.leading_adjustment = Some(LeadingAdjustment::None);
    options.trailing_case = Some(TrailingCase::Lower);
    match ROOT.parse() {
        Ok(root) => CaseMapper::new()
            .titlecase_segment_with_only_case_data_to_string(word, &root, options)
            .into_owned(),
        Err(_) => word.to_owned(),
    }
}
