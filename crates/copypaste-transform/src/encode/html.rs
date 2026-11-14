//! HTML character references: minimal or numeric encoding, and decoding
//! of every HTML5 named reference plus decimal and hex numeric ones.

use std::fmt::Write as _;

use markup5ever::data::{C1_REPLACEMENTS, NAMED_ENTITIES};

use super::check_growth;
use crate::catalogue::{Category, Context, Flags, Output, TransformDef};
use crate::error::{check_output_len, TransformError};
use crate::params::{ParamSpec, Params};

static ENCODE_PARAMS: [ParamSpec; 1] = [ParamSpec::choice(
    "scope",
    &["minimal", "non_ascii"],
    "minimal",
)];

pub(super) static ENCODE: TransformDef = TransformDef {
    id: "encode.html_encode",
    version: 1,
    category: Category::Encode,
    params: &ENCODE_PARAMS,
    flags: Flags::PLAIN,
    modifier: Some("html"),
    example: (
        "<a href=\"x\">Tom & Jerry</a>",
        "&lt;a href=&quot;x&quot;&gt;Tom &amp; Jerry&lt;/a&gt;",
    ),
    run: run_encode,
};

pub(super) static DECODE: TransformDef = TransformDef {
    id: "encode.html_decode",
    version: 1,
    category: Category::Encode,
    params: &[],
    flags: Flags::PLAIN,
    modifier: None,
    example: ("&lt;p&gt;caf&eacute; &#8364;5&lt;/p&gt;", "<p>café €5</p>"),
    run: run_decode,
};

/// Longest named reference, `CounterClockwiseContourIntegral;`.
const MAX_NAME: usize = 32;

fn run_encode(input: &str, params: &Params, ctx: &Context) -> Result<Output, TransformError> {
    let numeric = params.choice("scope") == "non_ascii";
    let mut out = String::with_capacity(input.len());
    for (i, c) in input.chars().enumerate() {
        ctx.tick(i)?;
        check_growth(i, &out)?;
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c if numeric && !c.is_ascii() => {
                let _ = write!(out, "&#{};", u32::from(c));
            }
            c => out.push(c),
        }
    }
    check_output_len(out.len())?;
    Ok(Output::new(out))
}

/// Unknown or incomplete references are kept as written.
fn run_decode(input: &str, _: &Params, ctx: &Context) -> Result<Output, TransformError> {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    let mut iteration = 0;
    while let Some(amp) = rest.find('&') {
        ctx.tick(iteration)?;
        iteration += 1;
        out.push_str(&rest[..amp]);
        let after = &rest[amp + 1..];
        match reference(after) {
            Some((consumed, first, second)) => {
                out.push(first);
                out.extend(second);
                rest = &after[consumed..];
            }
            None => {
                out.push('&');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    Ok(Output::new(out))
}

/// Bytes consumed and up to two decoded characters.
type Reference = (usize, char, Option<char>);

/// The reference that starts right after an `&`.
fn reference(after: &str) -> Option<Reference> {
    if let Some(numeric) = after.strip_prefix('#') {
        return numeric_reference(numeric).map(|(len, c)| (len + 1, c, None));
    }
    named_reference(after)
}

/// The longest HTML5 name that prefixes `after`. A name written without
/// its `;` (a legacy form such as `&copy`) counts only when no letter,
/// digit or `=` follows, so query strings like `?a=1&copy=2` stay intact.
fn named_reference(after: &str) -> Option<Reference> {
    let bytes = after.as_bytes();
    let run = bytes
        .iter()
        .take(MAX_NAME)
        .take_while(|b| b.is_ascii_alphanumeric())
        .count();
    let end = if bytes.get(run) == Some(&b';') {
        run + 1
    } else {
        run
    };
    let mut found = None;
    for len in 1..=end {
        let Some(&(first, second)) = NAMED_ENTITIES.get(&after[..len]) else {
            break;
        };
        if first != 0 {
            found = Some((len, first, second));
        }
    }
    let (len, first, second) = found?;
    let terminated = bytes[len - 1] == b';';
    let next = bytes.get(len).copied();
    if !terminated && next.is_some_and(|b| b.is_ascii_alphanumeric() || b == b'=') {
        return None;
    }
    let second = char::from_u32(second).filter(|_| second != 0);
    Some((len, char::from_u32(first)?, second))
}

/// `123;` or `x1F600;` after `&#`; the `;` is optional. Values HTML5 maps
/// elsewhere (NUL, surrogates, out of range, C1 controls) map the same way.
fn numeric_reference(numeric: &str) -> Option<(usize, char)> {
    let (radix, skip) = match numeric.as_bytes().first() {
        Some(b'x' | b'X') => (16, 1),
        _ => (10, 0),
    };
    let digits = numeric[skip..]
        .bytes()
        .take_while(|b| char::from(*b).is_digit(radix))
        .count();
    if digits == 0 {
        return None;
    }
    let value = numeric[skip..skip + digits]
        .bytes()
        .filter_map(|b| char::from(b).to_digit(radix))
        .fold(0u32, |acc, d| {
            acc.saturating_mul(radix).saturating_add(d).min(0x11_0000)
        });
    let len = skip + digits + usize::from(numeric[skip + digits..].starts_with(';'));
    let c = match value {
        0x80..=0x9f => C1_REPLACEMENTS[usize::try_from(value - 0x80).unwrap_or_default()]
            .unwrap_or(char::from_u32(value).unwrap_or(char::REPLACEMENT_CHARACTER)),
        0 => char::REPLACEMENT_CHARACTER,
        v => char::from_u32(v).unwrap_or(char::REPLACEMENT_CHARACTER),
    };
    Some((len, c))
}
