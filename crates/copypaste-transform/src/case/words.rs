//! Title Case and Sentence case over UAX #29 words, so an apostrophe inside
//! a word never starts a new one (`don't` → `Don't`).
//!
//! "Keep acronyms" leaves words written in capitals alone, but only when the
//! text has lowercase letters somewhere: text that is all capitals is
//! converted, not kept.

use unicode_segmentation::UnicodeSegmentation;

use super::{capitalize, lower};
use crate::catalogue::{Category, Context, Flags, Output, TransformDef};
use crate::error::TransformError;
use crate::params::{ParamSpec, Params};
use crate::segment::is_all_caps;

static TITLE_PARAMS: [ParamSpec; 2] = [
    ParamSpec::choice("style", &["headline", "all_words"], "headline"),
    ParamSpec::flag("keep_acronyms", true),
];

static SENTENCE_PARAMS: [ParamSpec; 1] = [ParamSpec::flag("keep_acronyms", true)];

pub(super) static TITLE: TransformDef = TransformDef {
    id: "case.title",
    version: 1,
    category: Category::Case,
    params: &TITLE_PARAMS,
    flags: Flags::PLAIN,
    modifier: Some("title"),
    example: ("the lord of the rings", "The Lord of the Rings"),
    run: run_title,
};

pub(super) static SENTENCE: TransformDef = TransformDef {
    id: "case.sentence",
    version: 1,
    category: Category::Case,
    params: &SENTENCE_PARAMS,
    flags: Flags::PLAIN,
    modifier: Some("sentence"),
    example: ("hello WORLD. how are you?", "Hello WORLD. How are you?"),
    run: run_sentence,
};

/// English words a headline keeps in lowercase unless first or last.
const MINOR_WORDS: [&str; 20] = [
    "a", "an", "and", "as", "at", "but", "by", "for", "in", "nor", "of", "off", "on", "or", "per",
    "so", "the", "to", "via", "vs",
];

fn is_word(segment: &str) -> bool {
    segment.chars().next().is_some_and(char::is_alphanumeric)
}

/// Whether capitalised words are kept for this input.
fn keeps_acronyms(input: &str, params: &Params) -> bool {
    params.flag("keep_acronyms") && input.chars().any(char::is_lowercase)
}

fn run_title(input: &str, params: &Params, ctx: &Context) -> Result<Output, TransformError> {
    let headline = params.choice("style") == "headline";
    let keep = keeps_acronyms(input, params);
    let mut out = String::with_capacity(input.len());
    for (n, line) in input.split_inclusive('\n').enumerate() {
        ctx.tick(n)?;
        let segments: Vec<&str> = line.split_word_bounds().collect();
        let first = segments.iter().position(|s| is_word(s));
        let last = segments.iter().rposition(|s| is_word(s));
        let mut after_colon = false;
        for (i, segment) in segments.iter().enumerate() {
            if !is_word(segment) {
                if !segment.trim().is_empty() {
                    after_colon = segment.contains(':');
                }
                out.push_str(segment);
                continue;
            }
            let edge = Some(i) == first || Some(i) == last || after_colon;
            after_colon = false;
            if keep && is_all_caps(segment) {
                out.push_str(segment);
            } else if headline && !edge && MINOR_WORDS.contains(&lower(segment).as_str()) {
                out.push_str(&lower(segment));
            } else {
                out.push_str(&capitalize(segment));
            }
        }
    }
    Ok(Output::new(out))
}

fn ends_sentence(segment: &str) -> bool {
    segment.contains(['.', '!', '?', '\n', '\u{3002}', '\u{FF01}', '\u{FF1F}'])
}

fn run_sentence(input: &str, params: &Params, ctx: &Context) -> Result<Output, TransformError> {
    let keep = keeps_acronyms(input, params);
    let mut out = String::with_capacity(input.len());
    let mut sentence_start = true;
    for (i, segment) in input.split_word_bounds().enumerate() {
        ctx.tick(i)?;
        if !is_word(segment) {
            sentence_start |= ends_sentence(segment);
            out.push_str(segment);
            continue;
        }
        if keep && is_all_caps(segment) {
            out.push_str(segment);
        } else if sentence_start {
            out.push_str(&capitalize(segment));
        } else {
            out.push_str(&lower(segment));
        }
        sentence_start = false;
    }
    Ok(Output::new(out))
}
