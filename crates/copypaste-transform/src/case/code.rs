//! Identifier cases built from [`crate::segment::tokens`]: camelCase,
//! PascalCase, snake_case, kebab-case and CONSTANT_CASE.

use super::{capitalize, lower, upper};
use crate::catalogue::{Category, Context, Flags, Output, TransformDef};
use crate::error::TransformError;
use crate::params::{ParamSpec, Params};
use crate::segment::{is_all_caps, tokens};

static MIXED_PARAMS: [ParamSpec; 2] = [
    ParamSpec::flag("per_line", true),
    ParamSpec::flag("keep_acronyms", false),
];

static JOINED_PARAMS: [ParamSpec; 1] = [ParamSpec::flag("per_line", true)];

pub(super) static CAMEL: TransformDef = TransformDef {
    id: "case.camel",
    version: 1,
    category: Category::Case,
    params: &MIXED_PARAMS,
    flags: Flags::PLAIN,
    modifier: Some("camel"),
    example: ("user account ID", "userAccountId"),
    run: run_camel,
};

pub(super) static PASCAL: TransformDef = TransformDef {
    id: "case.pascal",
    version: 1,
    category: Category::Case,
    params: &MIXED_PARAMS,
    flags: Flags::PLAIN,
    modifier: Some("pascal"),
    example: ("user account ID", "UserAccountId"),
    run: run_pascal,
};

pub(super) static SNAKE: TransformDef = TransformDef {
    id: "case.snake",
    version: 1,
    category: Category::Case,
    params: &JOINED_PARAMS,
    flags: Flags::PLAIN,
    modifier: Some("snake"),
    example: ("parseHTTPResponse2XX", "parse_http_response2_xx"),
    run: run_snake,
};

pub(super) static KEBAB: TransformDef = TransformDef {
    id: "case.kebab",
    version: 1,
    category: Category::Case,
    params: &JOINED_PARAMS,
    flags: Flags::PLAIN,
    modifier: Some("kebab"),
    example: ("Hello World Again", "hello-world-again"),
    run: run_kebab,
};

pub(super) static CONSTANT: TransformDef = TransformDef {
    id: "case.constant",
    version: 1,
    category: Category::Case,
    params: &JOINED_PARAMS,
    flags: Flags::PLAIN,
    modifier: Some("constant"),
    example: ("maxRetryCount", "MAX_RETRY_COUNT"),
    run: run_constant,
};

/// Acronyms that "keep acronyms" writes in capitals even when the input
/// does not (`user id` → `UserID`).
const ACRONYMS: [&str; 26] = [
    "api", "ascii", "cpu", "css", "csv", "db", "dns", "gpu", "html", "http", "https", "id", "io",
    "ip", "json", "jwt", "pdf", "sdk", "sql", "ssh", "tcp", "tls", "ui", "uri", "url", "uuid",
];

fn is_acronym(token: &str) -> bool {
    is_all_caps(token) || ACRONYMS.contains(&lower(token).as_str())
}

/// Converts every line on its own, or the whole text as one identifier,
/// keeping each line's terminator.
fn convert(
    input: &str,
    params: &Params,
    ctx: &Context,
    build: impl Fn(Vec<String>) -> String,
) -> Result<Output, TransformError> {
    if !params.flag("per_line") {
        return Ok(Output::new(build(tokens(input))));
    }
    let mut out = String::with_capacity(input.len());
    for (i, line) in input.split_inclusive('\n').enumerate() {
        ctx.tick(i)?;
        let body = line.trim_end_matches(['\r', '\n']);
        out.push_str(&build(tokens(body)));
        out.push_str(&line[body.len()..]);
    }
    Ok(Output::new(out))
}

fn mixed(tokens: Vec<String>, keep_acronyms: bool, lower_first: bool) -> String {
    let mut out = String::new();
    for (i, token) in tokens.iter().enumerate() {
        if i == 0 && lower_first {
            out.push_str(&lower(token));
        } else if keep_acronyms && is_acronym(token) {
            out.push_str(&upper(token));
        } else {
            out.push_str(&capitalize(token));
        }
    }
    out
}

fn joined(tokens: Vec<String>, separator: &str, map: fn(&str) -> String) -> String {
    tokens
        .iter()
        .map(|t| map(t))
        .collect::<Vec<_>>()
        .join(separator)
}

fn run_camel(input: &str, params: &Params, ctx: &Context) -> Result<Output, TransformError> {
    let keep = params.flag("keep_acronyms");
    convert(input, params, ctx, |t| mixed(t, keep, true))
}

fn run_pascal(input: &str, params: &Params, ctx: &Context) -> Result<Output, TransformError> {
    let keep = params.flag("keep_acronyms");
    convert(input, params, ctx, |t| mixed(t, keep, false))
}

fn run_snake(input: &str, params: &Params, ctx: &Context) -> Result<Output, TransformError> {
    convert(input, params, ctx, |t| joined(t, "_", lower))
}

fn run_kebab(input: &str, params: &Params, ctx: &Context) -> Result<Output, TransformError> {
    convert(input, params, ctx, |t| joined(t, "-", lower))
}

fn run_constant(input: &str, params: &Params, ctx: &Context) -> Result<Output, TransformError> {
    convert(input, params, ctx, |t| joined(t, "_", upper))
}
