//! Joining lines, unwrapping hard-wrapped paragraphs and prefixing lines.

use super::{is_empty_line, Lines};
use crate::catalogue::{Category, Context, Flags, Output, TransformDef};
use crate::error::{check_output_len, TransformError};
use crate::params::{ParamSpec, Params};

static JOIN_PARAMS: [ParamSpec; 3] = [
    ParamSpec::choice("separator", &["space", "comma", "none", "custom"], "space"),
    ParamSpec::text("custom_separator", ""),
    ParamSpec::flag("drop_empty", true),
];

static UNWRAP_PARAMS: [ParamSpec; 1] = [ParamSpec::flag("dehyphenate", true)];

static PREFIX_PARAMS: [ParamSpec; 3] = [
    ParamSpec::choice(
        "style",
        &["dash", "asterisk", "number", "quote", "custom"],
        "dash",
    ),
    ParamSpec::text("custom_prefix", ""),
    ParamSpec::flag("skip_empty", true),
];

pub(super) static JOIN: TransformDef = TransformDef {
    id: "lines.join",
    version: 1,
    category: Category::Lines,
    params: &JOIN_PARAMS,
    flags: Flags::PLAIN,
    modifier: None,
    example: ("red\ngreen\n\nblue", "red green blue"),
    run: run_join,
};

pub(super) static UNWRAP: TransformDef = TransformDef {
    id: "lines.unwrap",
    version: 1,
    category: Category::Lines,
    params: &UNWRAP_PARAMS,
    flags: Flags::PLAIN,
    modifier: None,
    example: ("exam-\nple text\nhere\n\nNext", "example text here\n\nNext"),
    run: run_unwrap,
};

pub(super) static PREFIX: TransformDef = TransformDef {
    id: "lines.prefix",
    version: 1,
    category: Category::Lines,
    params: &PREFIX_PARAMS,
    flags: Flags::PLAIN,
    modifier: None,
    example: ("apples\npears", "- apples\n- pears"),
    run: run_prefix,
};

fn run_join(input: &str, params: &Params, ctx: &Context) -> Result<Output, TransformError> {
    let separator = match params.choice("separator") {
        "comma" => ", ",
        "none" => "",
        "custom" => params.text("custom_separator"),
        _ => " ",
    };
    let drop_empty = params.flag("drop_empty");
    let lines = Lines::split(input);
    let mut out = String::with_capacity(input.len());
    let kept = lines
        .lines
        .iter()
        .filter(|l| !(drop_empty && is_empty_line(l, true)));
    for (i, line) in kept.enumerate() {
        ctx.tick(i)?;
        if i > 0 {
            out.push_str(separator);
        }
        out.push_str(line);
        check_output_len(out.len())?;
    }
    Ok(Output::new(out))
}

const HYPHENS: [char; 3] = ['-', '\u{2010}', '\u{ad}'];

/// Appends a continuation line to a paragraph. A word broken by a hyphen
/// at the line end is rejoined; with `dehyphenate` and a lowercase start on
/// the next line, the hyphen is dropped (`exam-` + `ple` → `example`).
fn append(paragraph: &mut String, next: &str, dehyphenate: bool) {
    let mut chars = paragraph.chars().rev();
    let broken = matches!(
        (chars.next(), chars.next()),
        (Some(h), Some(before)) if HYPHENS.contains(&h) && before.is_alphabetic()
    );
    if !broken {
        paragraph.push(' ');
    } else if dehyphenate && next.chars().next().is_some_and(char::is_lowercase) {
        paragraph.pop();
    }
    paragraph.push_str(next);
}

fn run_unwrap(input: &str, params: &Params, ctx: &Context) -> Result<Output, TransformError> {
    let dehyphenate = params.flag("dehyphenate");
    let lines = Lines::split(input);
    let mut out: Vec<String> = Vec::new();
    let mut paragraph: Option<String> = None;
    for (i, line) in lines.lines.iter().enumerate() {
        ctx.tick(i)?;
        if is_empty_line(line, true) {
            out.extend(paragraph.take());
            out.push(String::new());
            continue;
        }
        match paragraph.as_mut() {
            Some(p) => append(p, line.trim(), dehyphenate),
            None => paragraph = Some(line.trim_end().to_owned()),
        }
    }
    out.extend(paragraph);
    Ok(Output::new(lines.join(out, ctx)?))
}

fn run_prefix(input: &str, params: &Params, ctx: &Context) -> Result<Output, TransformError> {
    let style = params.choice("style");
    let skip_empty = params.flag("skip_empty");
    let lines = Lines::split(input);
    let mut number = 0usize;
    let prefixed = lines.lines.iter().map(|line| {
        if skip_empty && is_empty_line(line, true) {
            return (*line).to_owned();
        }
        number += 1;
        match style {
            "asterisk" => format!("* {line}"),
            "number" => format!("{number}. {line}"),
            "quote" => format!("> {line}"),
            "custom" => format!("{}{line}", params.text("custom_prefix")),
            _ => format!("- {line}"),
        }
    });
    Ok(Output::new(lines.join(prefixed, ctx)?))
}
