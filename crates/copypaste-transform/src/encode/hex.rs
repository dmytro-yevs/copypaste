//! Hex encode and decode of UTF-8 bytes.

use super::decoded_text;
use crate::catalogue::{Category, Context, Flags, Output, TransformDef};
use crate::error::{check_output_len, TransformError};
use crate::params::{ParamSpec, Params};

static ENCODE_PARAMS: [ParamSpec; 2] = [
    ParamSpec::choice("separator", &["none", "space"], "none"),
    ParamSpec::choice("case", &["lower", "upper"], "lower"),
];

pub(super) static ENCODE: TransformDef = TransformDef {
    id: "encode.hex_encode",
    version: 1,
    category: Category::Encode,
    params: &ENCODE_PARAMS,
    flags: Flags::PLAIN,
    modifier: Some("hex"),
    example: ("Hi!", "486921"),
    run: run_encode,
};

pub(super) static DECODE: TransformDef = TransformDef {
    id: "encode.hex_decode",
    version: 1,
    category: Category::Encode,
    params: &[],
    flags: Flags::PLAIN,
    modifier: None,
    example: ("0x48 69 21", "Hi!"),
    run: run_decode,
};

fn run_encode(input: &str, params: &Params, ctx: &Context) -> Result<Output, TransformError> {
    let spaced = params.choice("separator") == "space";
    let upper = params.choice("case") == "upper";
    encode(input.as_bytes(), spaced, upper, ctx).map(Output::new)
}

/// Two digits per byte, optionally separated by single spaces.
pub(super) fn encode(
    bytes: &[u8],
    spaced: bool,
    upper: bool,
    ctx: &Context,
) -> Result<String, TransformError> {
    let digits: &[u8; 16] = if upper {
        b"0123456789ABCDEF"
    } else {
        b"0123456789abcdef"
    };
    let per_byte = if spaced { 3 } else { 2 };
    check_output_len((bytes.len() * per_byte).saturating_sub(usize::from(spaced)))?;
    let mut out = String::with_capacity(bytes.len() * per_byte);
    for (i, &byte) in bytes.iter().enumerate() {
        ctx.tick(i)?;
        if spaced && i > 0 {
            out.push(' ');
        }
        out.push(char::from(digits[usize::from(byte >> 4)]));
        out.push(char::from(digits[usize::from(byte & 0xf)]));
    }
    Ok(out)
}

/// Ignores whitespace and a `0x` prefix before any group of digits.
fn run_decode(input: &str, _: &Params, ctx: &Context) -> Result<Output, TransformError> {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len() / 2);
    let mut high: Option<u8> = None;
    let mut i = 0;
    while i < bytes.len() {
        ctx.tick(i)?;
        let byte = bytes[i];
        if byte.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        let group_start = i == 0 || bytes[i - 1].is_ascii_whitespace();
        if group_start && byte == b'0' && matches!(bytes.get(i + 1), Some(b'x' | b'X')) {
            i += 2;
            continue;
        }
        let nibble = char::from(byte)
            .to_digit(16)
            .ok_or_else(|| TransformError::invalid_input("hex.invalid", input, i))?;
        let nibble = u8::try_from(nibble).unwrap_or_default();
        match high.take() {
            Some(h) => out.push((h << 4) | nibble),
            None => high = Some(nibble),
        }
        i += 1;
    }
    if high.is_some() {
        return Err(TransformError::invalid_input(
            "hex.odd_length",
            input,
            input.len(),
        ));
    }
    decoded_text(out).map(Output::new)
}

#[cfg(test)]
mod tests {
    use crate::{apply, Context, Step, TransformError, MAX_OUTPUT_BYTES};

    #[test]
    fn output_over_the_limit_is_refused_before_it_is_built() {
        let input = "a".repeat(MAX_OUTPUT_BYTES / 3 + 2);
        let step = Step::current("encode.hex_encode")
            .unwrap()
            .param("separator", "space");
        assert_eq!(
            apply(&step, &input, &Context::new()),
            Err(TransformError::OutputTooLarge {
                limit: MAX_OUTPUT_BYTES
            })
        );
    }
}
