//! Percent-encoding of a URL component or a whole URL, and decoding.

use percent_encoding::{utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};

use super::check_growth;
use crate::catalogue::{Category, Context, Flags, Output, TransformDef};
use crate::error::{check_output_len, TransformError};
use crate::params::{ParamSpec, Params};

static COMPONENT_PARAMS: [ParamSpec; 1] =
    [ParamSpec::choice("space", &["percent", "plus"], "percent")];

static DECODE_PARAMS: [ParamSpec; 1] = [ParamSpec::flag("plus_as_space", true)];

pub(super) static ENCODE_COMPONENT: TransformDef = TransformDef {
    id: "encode.url_encode",
    version: 1,
    category: Category::Encode,
    params: &COMPONENT_PARAMS,
    flags: Flags::PLAIN,
    modifier: Some("urlencode"),
    example: ("a b&c=ü", "a%20b%26c%3D%C3%BC"),
    run: run_encode_component,
};

pub(super) static ENCODE_FULL: TransformDef = TransformDef {
    id: "encode.url_encode_full",
    version: 1,
    category: Category::Encode,
    params: &[],
    flags: Flags::PLAIN,
    modifier: None,
    example: (
        "https://x.test/a b?q=ü&r=50%25",
        "https://x.test/a%20b?q=%C3%BC&r=50%25",
    ),
    run: run_encode_full,
};

pub(super) static DECODE: TransformDef = TransformDef {
    id: "encode.url_decode",
    version: 1,
    category: Category::Encode,
    params: &DECODE_PARAMS,
    flags: Flags::PLAIN,
    modifier: None,
    example: ("a%20b+c%3D%C3%BC", "a b c=ü"),
    run: run_decode,
};

/// Everything but the RFC 3986 unreserved characters.
const COMPONENT: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');

/// RFC 3986 reserved characters, which keep their meaning in a full URL.
const RESERVED: &[u8] = b":/?#[]@!$&'()*+,;=";

fn run_encode_component(
    input: &str,
    params: &Params,
    ctx: &Context,
) -> Result<Output, TransformError> {
    let plus = params.choice("space") == "plus";
    let mut out = String::with_capacity(input.len());
    for (i, piece) in utf8_percent_encode(input, COMPONENT).enumerate() {
        ctx.tick(i)?;
        check_growth(i, &out)?;
        if plus && piece == "%20" {
            out.push('+');
        } else {
            out.push_str(piece);
        }
    }
    Ok(Output::new(out))
}

/// Encodes only bytes that cannot appear in a URL; existing `%XX`
/// sequences are kept, so encoding twice changes nothing.
fn run_encode_full(input: &str, _: &Params, ctx: &Context) -> Result<Output, TransformError> {
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(input.len());
    for (i, &byte) in bytes.iter().enumerate() {
        ctx.tick(i)?;
        check_growth(i, &out)?;
        let keep = byte.is_ascii_alphanumeric()
            || b"-._~".contains(&byte)
            || RESERVED.contains(&byte)
            || (byte == b'%' && hex_pair(bytes, i + 1).is_some());
        if keep {
            out.push(char::from(byte));
        } else {
            push_escape(&mut out, byte);
        }
    }
    check_output_len(out.len())?;
    Ok(Output::new(out))
}

fn push_escape(out: &mut String, byte: u8) {
    const DIGITS: &[u8; 16] = b"0123456789ABCDEF";
    out.push('%');
    out.push(char::from(DIGITS[usize::from(byte >> 4)]));
    out.push(char::from(DIGITS[usize::from(byte & 0xf)]));
}

/// The byte written as two hex digits at `bytes[at..at + 2]`.
fn hex_pair(bytes: &[u8], at: usize) -> Option<u8> {
    let digit = |i: usize| bytes.get(i).and_then(|&b| char::from(b).to_digit(16));
    let value = (digit(at)? << 4) | digit(at + 1)?;
    u8::try_from(value).ok()
}

/// A `%` without two hex digits fails at its offset; decoded bytes that
/// are not UTF-8 fail at the escape that produced the first bad byte.
fn run_decode(input: &str, params: &Params, ctx: &Context) -> Result<Output, TransformError> {
    let plus_as_space = params.flag("plus_as_space");
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        ctx.tick(i)?;
        match bytes[i] {
            b'%' => {
                let byte = hex_pair(bytes, i + 1)
                    .ok_or_else(|| TransformError::invalid_input("url.malformed", input, i))?;
                out.push(byte);
                i += 3;
                continue;
            }
            b'+' if plus_as_space => out.push(b' '),
            byte => out.push(byte),
        }
        i += 1;
    }
    String::from_utf8(out).map(Output::new).map_err(|e| {
        let offset = input_offset(bytes, e.utf8_error().valid_up_to());
        TransformError::invalid_input("url.not_utf8", input, offset)
    })
}

/// The input offset of decoded byte `target`.
fn input_offset(bytes: &[u8], target: usize) -> usize {
    let (mut i, mut produced) = (0, 0);
    while i < bytes.len() && produced < target {
        i += if bytes[i] == b'%' { 3 } else { 1 };
        produced += 1;
    }
    i
}

#[cfg(test)]
mod tests {
    use crate::{apply, Context, Location, Step, TransformError};

    #[test]
    fn malformed_escape_points_at_its_percent_sign() {
        let step = Step::current("encode.url_decode").unwrap();
        assert_eq!(
            apply(&step, "%E0%A4%A", &Context::new()),
            Err(TransformError::InvalidInput {
                code: "url.malformed",
                location: Some(Location {
                    offset: 6,
                    line: 1,
                    column: 7
                }),
            })
        );
    }
}
