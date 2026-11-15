//! Trimming, whitespace collapsing and empty-line removal. Whitespace is the
//! Unicode White_Space property, so NBSP and the ideographic space count.

use super::{is_empty_line, Lines};
use crate::catalogue::{Category, Context, Flags, Output, TransformDef};
use crate::error::TransformError;
use crate::params::{ParamSpec, Params};

static TRIM_EACH_PARAMS: [ParamSpec; 1] =
    [ParamSpec::choice("sides", &["both", "start", "end"], "end")];

static COLLAPSE_PARAMS: [ParamSpec; 1] = [ParamSpec::flag("keep_line_breaks", true)];

static REMOVE_EMPTY_PARAMS: [ParamSpec; 1] = [ParamSpec::flag("whitespace_only", true)];

pub(super) static TRIM: TransformDef = TransformDef {
    id: "lines.trim",
    version: 1,
    category: Category::Lines,
    params: &[],
    flags: Flags::PLAIN,
    modifier: Some("trim"),
    example: ("\u{a0} hello world \n", "hello world"),
    run: run_trim,
};

pub(super) static TRIM_EACH: TransformDef = TransformDef {
    id: "lines.trim_each",
    version: 1,
    category: Category::Lines,
    params: &TRIM_EACH_PARAMS,
    flags: Flags::PLAIN,
    modifier: None,
    example: ("one  \ntwo\t\n", "one\ntwo\n"),
    run: run_trim_each,
};

pub(super) static COLLAPSE: TransformDef = TransformDef {
    id: "lines.collapse_whitespace",
    version: 1,
    category: Category::Lines,
    params: &COLLAPSE_PARAMS,
    flags: Flags::PLAIN,
    modifier: Some("collapse"),
    example: ("a  \t b\nc   d", "a b\nc d"),
    run: run_collapse,
};

pub(super) static REMOVE_EMPTY: TransformDef = TransformDef {
    id: "lines.remove_empty",
    version: 1,
    category: Category::Lines,
    params: &REMOVE_EMPTY_PARAMS,
    flags: Flags::PLAIN,
    modifier: None,
    example: ("a\n\n  \nb\n", "a\nb\n"),
    run: run_remove_empty,
};

fn run_trim(input: &str, _: &Params, _: &Context) -> Result<Output, TransformError> {
    Ok(Output::new(input.trim().to_owned()))
}

fn run_trim_each(input: &str, params: &Params, ctx: &Context) -> Result<Output, TransformError> {
    let trim: fn(&str) -> &str = match params.choice("sides") {
        "both" => str::trim,
        "start" => str::trim_start,
        _ => str::trim_end,
    };
    let lines = Lines::split(input);
    let text = lines.join(lines.lines.iter().map(|l| trim(l)), ctx)?;
    Ok(Output::new(text))
}

fn is_line_break(c: char) -> bool {
    matches!(c, '\n' | '\r')
}

fn run_collapse(input: &str, params: &Params, ctx: &Context) -> Result<Output, TransformError> {
    let keep_breaks = params.flag("keep_line_breaks");
    let mut out = String::with_capacity(input.len());
    let mut in_run = false;
    for (i, c) in input.chars().enumerate() {
        ctx.tick(i)?;
        let collapsible = c.is_whitespace() && !(keep_breaks && is_line_break(c));
        if collapsible {
            if !in_run {
                out.push(' ');
            }
            in_run = true;
        } else {
            out.push(c);
            in_run = false;
        }
    }
    Ok(Output::new(out))
}

fn run_remove_empty(input: &str, params: &Params, ctx: &Context) -> Result<Output, TransformError> {
    let blank_counts = params.flag("whitespace_only");
    let lines = Lines::split(input);
    let kept = lines
        .lines
        .iter()
        .filter(|l| !is_empty_line(l, blank_counts));
    Ok(Output::new(lines.join(kept, ctx)?))
}
