//! UPPERCASE, lowercase and swap case: full Unicode mappings whose output
//! length may differ from the input (`ß` → `SS`).

use unicode_segmentation::UnicodeSegmentation;

use crate::catalogue::{Category, Context, Flags, Output, TransformDef};
use crate::error::TransformError;
use crate::params::{ParamSpec, Params};

static LOCALE: [ParamSpec; 1] = [ParamSpec::choice(
    "locale",
    &["invariant", "system"],
    "invariant",
)];

pub(super) static UPPER: TransformDef = TransformDef {
    id: "case.upper",
    version: 1,
    category: Category::Case,
    params: &LOCALE,
    flags: Flags::PLAIN,
    modifier: Some("upper"),
    example: ("Straße", "STRASSE"),
    run: run_upper,
};

pub(super) static LOWER: TransformDef = TransformDef {
    id: "case.lower",
    version: 1,
    category: Category::Case,
    params: &LOCALE,
    flags: Flags::PLAIN,
    modifier: Some("lower"),
    example: ("ΟΔΟΣ Hello", "οδο\u{3c2} hello"),
    run: run_lower,
};

pub(super) static SWAP: TransformDef = TransformDef {
    id: "case.swap",
    version: 1,
    category: Category::Case,
    params: &[],
    flags: Flags::PLAIN,
    modifier: Some("swap"),
    example: ("Hello World", "hELLO wORLD"),
    run: run_swap,
};

/// The device locale when the `locale` parameter asks for it.
fn device_locale<'c>(params: &Params, ctx: &'c Context) -> Option<&'c str> {
    if params.choice("locale") == "system" {
        ctx.locale()
    } else {
        None
    }
}

fn run_upper(input: &str, params: &Params, ctx: &Context) -> Result<Output, TransformError> {
    Ok(Output::new(super::upper_in(
        input,
        device_locale(params, ctx),
    )))
}

fn run_lower(input: &str, params: &Params, ctx: &Context) -> Result<Output, TransformError> {
    Ok(Output::new(super::lower_in(
        input,
        device_locale(params, ctx),
    )))
}

/// Swaps the case of each grapheme cluster by its first character, so
/// emoji sequences, flags and combining marks pass through whole.
fn run_swap(input: &str, _: &Params, ctx: &Context) -> Result<Output, TransformError> {
    let mut out = String::with_capacity(input.len());
    for (i, grapheme) in input.graphemes(true).enumerate() {
        ctx.tick(i)?;
        let first = grapheme.chars().next().unwrap_or_default();
        if first.is_lowercase() {
            out.push_str(&super::upper(grapheme));
        } else if first.is_uppercase() {
            out.push_str(&super::lower(grapheme));
        } else {
            out.push_str(grapheme);
        }
    }
    Ok(Output::new(out))
}
