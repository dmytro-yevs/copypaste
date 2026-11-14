//! Backslash escapes (C and JSON style) and `\u` Unicode escapes.

use std::fmt::Write as _;

use super::check_growth;
use crate::catalogue::{Category, Context, Flags, Output, TransformDef};
use crate::error::{check_output_len, Location, TransformError, Warning};
use crate::params::{ParamSpec, Params};

static UNICODE_ESCAPE_PARAMS: [ParamSpec; 1] = [ParamSpec::choice(
    "astral",
    &["surrogate_pair", "braces"],
    "surrogate_pair",
)];

pub(super) static BACKSLASH_ESCAPE: TransformDef = TransformDef {
    id: "encode.backslash_escape",
    version: 1,
    category: Category::Encode,
    params: &[],
    flags: Flags::PLAIN,
    modifier: Some("escape"),
    example: ("say \"hi\"\n\tC:\\dir", "say \\\"hi\\\"\\n\\tC:\\\\dir"),
    run: run_backslash_escape,
};

pub(super) static BACKSLASH_UNESCAPE: TransformDef = TransformDef {
    id: "encode.backslash_unescape",
    version: 1,
    category: Category::Encode,
    params: &[],
    flags: Flags::PLAIN,
    modifier: None,
    example: ("say \\\"hi\\\"\\n\\x41\\u00e9", "say \"hi\"\nAé"),
    run: run_backslash_unescape,
};

pub(super) static UNICODE_ESCAPE: TransformDef = TransformDef {
    id: "encode.unicode_escape",
    version: 1,
    category: Category::Encode,
    params: &UNICODE_ESCAPE_PARAMS,
    flags: Flags::PLAIN,
    modifier: None,
    example: ("café 😀", "caf\\u00e9 \\ud83d\\ude00"),
    run: run_unicode_escape,
};

pub(super) static UNICODE_UNESCAPE: TransformDef = TransformDef {
    id: "encode.unicode_unescape",
    version: 1,
    category: Category::Encode,
    params: &[],
    flags: Flags::PLAIN,
    modifier: None,
    example: ("caf\\u00e9 \\u{1F600}", "café 😀"),
    run: run_unicode_unescape,
};

/// An escape kept as written because nothing defines it.
const UNKNOWN: &str = "escape.unknown";
/// A known escape whose value is unusable (lone surrogate, bad digits,
/// `\x` bytes that are not UTF-8); also kept as written.
const INVALID: &str = "escape.invalid";

fn run_backslash_escape(input: &str, _: &Params, ctx: &Context) -> Result<Output, TransformError> {
    let mut out = String::with_capacity(input.len());
    for (i, c) in input.chars().enumerate() {
        ctx.tick(i)?;
        check_growth(i, &out)?;
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_ascii_control() => {
                let _ = write!(out, "\\u{:04x}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    check_output_len(out.len())?;
    Ok(Output::new(out))
}

fn run_unicode_escape(
    input: &str,
    params: &Params,
    ctx: &Context,
) -> Result<Output, TransformError> {
    let braces = params.choice("astral") == "braces";
    let mut out = String::with_capacity(input.len());
    let mut units = [0u16; 2];
    for (i, c) in input.chars().enumerate() {
        ctx.tick(i)?;
        check_growth(i, &out)?;
        if c.is_ascii() {
            out.push(c);
        } else if braces && u32::from(c) > 0xffff {
            let _ = write!(out, "\\u{{{:x}}}", u32::from(c));
        } else {
            for unit in c.encode_utf16(&mut units) {
                let _ = write!(out, "\\u{unit:04x}");
            }
        }
    }
    check_output_len(out.len())?;
    Ok(Output::new(out))
}

/// Collects the first warning of each code, located in the input.
struct Warnings<'a> {
    input: &'a str,
    list: Vec<Warning>,
}

impl<'a> Warnings<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            input,
            list: Vec::new(),
        }
    }

    fn add(&mut self, code: &'static str, offset: usize) {
        if !self.list.iter().any(|w| w.code == code) {
            self.list
                .push(Warning::at(code, Location::of(self.input, offset)));
        }
    }

    fn finish(self, text: String) -> Output {
        Output {
            text,
            warnings: self.list,
            sensitive: false,
        }
    }
}

fn run_backslash_unescape(
    input: &str,
    _: &Params,
    ctx: &Context,
) -> Result<Output, TransformError> {
    let mut out = String::with_capacity(input.len());
    let mut warnings = Warnings::new(input);
    let mut at = 0;
    let mut iteration = 0;
    while let Some(found) = input[at..].find('\\') {
        ctx.tick(iteration)?;
        iteration += 1;
        let slash = at + found;
        out.push_str(&input[at..slash]);
        let rest = &input[slash + 1..];
        let simple = match rest.as_bytes().first() {
            Some(b'n') => Some('\n'),
            Some(b'r') => Some('\r'),
            Some(b't') => Some('\t'),
            Some(b'\\') => Some('\\'),
            Some(b'"') => Some('"'),
            Some(b'\'') => Some('\''),
            Some(b'/') => Some('/'),
            Some(b'b') => Some('\u{8}'),
            Some(b'f') => Some('\u{c}'),
            Some(b'v') => Some('\u{b}'),
            Some(b'a') => Some('\u{7}'),
            Some(b'0') if !rest[1..].starts_with(|c: char| c.is_digit(8)) => Some('\0'),
            _ => None,
        };
        let consumed = if let Some(c) = simple {
            out.push(c);
            1
        } else if rest.starts_with('u') {
            unicode(rest, &mut out, &mut warnings, slash)
        } else if rest.starts_with('x') {
            hex_bytes(input, slash, &mut out, &mut warnings)
        } else {
            warnings.add(UNKNOWN, slash);
            out.push('\\');
            0
        };
        at = slash + 1 + consumed;
    }
    out.push_str(&input[at..]);
    Ok(warnings.finish(out))
}

/// Decodes the `\u` escape at `rest` (just after its backslash) into `out`
/// and returns the bytes consumed after the backslash. An unusable escape
/// is copied as written.
fn unicode(rest: &str, out: &mut String, warnings: &mut Warnings, slash: usize) -> usize {
    match unicode_value(rest) {
        Some((len, c)) => {
            out.push(c);
            len
        }
        None => {
            warnings.add(INVALID, slash);
            out.push('\\');
            0
        }
    }
}

/// `u{1F600}`, `u00e9` or a surrogate pair `ud83d\ude00`.
fn unicode_value(rest: &str) -> Option<(usize, char)> {
    let body = rest.strip_prefix('u')?;
    if let Some(braced) = body.strip_prefix('{') {
        let digits = hex_digits(braced, 6);
        if digits == 0 || !braced[digits..].starts_with('}') {
            return None;
        }
        let value = u32::from_str_radix(&braced[..digits], 16).ok()?;
        return char::from_u32(value).map(|c| (digits + 3, c));
    }
    let high = four_hex(body)?;
    if let Some(c) = char::from_u32(high) {
        return Some((5, c));
    }
    let low = body[4..].strip_prefix("\\u").and_then(four_hex)?;
    if !(0xd800..0xdc00).contains(&high) || !(0xdc00..0xe000).contains(&low) {
        return None;
    }
    let c = char::from_u32(0x10000 + ((high - 0xd800) << 10) + (low - 0xdc00))?;
    Some((11, c))
}

fn hex_digits(text: &str, max: usize) -> usize {
    text.bytes()
        .take(max)
        .take_while(u8::is_ascii_hexdigit)
        .count()
}

fn four_hex(text: &str) -> Option<u32> {
    (hex_digits(text, 4) == 4)
        .then(|| u32::from_str_radix(&text[..4], 16).ok())
        .flatten()
}

/// Decodes a run of `\xHH` escapes starting at `slash` as UTF-8 bytes and
/// returns the bytes consumed after the first backslash.
fn hex_bytes(input: &str, slash: usize, out: &mut String, warnings: &mut Warnings) -> usize {
    let mut bytes = Vec::new();
    let mut end = slash;
    while input[end..].starts_with("\\x") && hex_digits(&input[end + 2..], 2) == 2 {
        bytes.push(u8::from_str_radix(&input[end + 2..end + 4], 16).unwrap_or_default());
        end += 4;
    }
    if bytes.is_empty() {
        warnings.add(INVALID, slash);
        out.push('\\');
        return 0;
    }
    match String::from_utf8(bytes) {
        Ok(text) => out.push_str(&text),
        Err(_) => {
            warnings.add(INVALID, slash);
            out.push_str(&input[slash..end]);
        }
    }
    end - slash - 1
}

/// Decodes `\u` escapes only. Any other backslash and the character after
/// it are copied, so an escaped backslash (`\\u0041`) stays as written.
fn run_unicode_unescape(input: &str, _: &Params, ctx: &Context) -> Result<Output, TransformError> {
    let mut out = String::with_capacity(input.len());
    let mut warnings = Warnings::new(input);
    let mut at = 0;
    let mut iteration = 0;
    while let Some(found) = input[at..].find('\\') {
        ctx.tick(iteration)?;
        iteration += 1;
        let slash = at + found;
        out.push_str(&input[at..slash]);
        let rest = &input[slash + 1..];
        let consumed = if rest.starts_with('u') {
            unicode(rest, &mut out, &mut warnings, slash)
        } else {
            let next = rest.chars().next().map_or(0, char::len_utf8);
            out.push('\\');
            out.push_str(&rest[..next]);
            next
        };
        at = slash + 1 + consumed;
    }
    out.push_str(&input[at..]);
    Ok(warnings.finish(out))
}

#[cfg(test)]
mod tests {
    use crate::{apply, Context, Step};

    #[test]
    fn unknown_escapes_stay_and_warn_once_at_the_first() {
        let step = Step::current("encode.backslash_unescape").unwrap();
        let out = apply(&step, "a\\qb\\zc\\n", &Context::new()).unwrap();
        assert_eq!(out.text, "a\\qb\\zc\n");
        assert_eq!(out.warnings.len(), 1);
        assert_eq!(out.warnings[0].code, "escape.unknown");
        assert_eq!(out.warnings[0].location.map(|l| l.column), Some(2));
    }
}
