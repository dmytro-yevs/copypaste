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

fn lower(text: &str) -> String {
    CaseMapper::new()
        .lowercase_to_string(text, &Default::default())
        .into_owned()
}

fn upper(text: &str) -> String {
    CaseMapper::new()
        .uppercase_to_string(text, &Default::default())
        .into_owned()
}

/// Titlecases the first character of `word` and lowercases the rest.
fn capitalize(word: &str) -> String {
    let mut options = TitlecaseOptions::default();
    options.leading_adjustment = Some(LeadingAdjustment::None);
    options.trailing_case = Some(TrailingCase::Lower);
    CaseMapper::new()
        .titlecase_segment_with_only_case_data_to_string(word, &Default::default(), options)
        .into_owned()
}
