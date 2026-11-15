//! Base64 encode and a tolerant decode.

use ::base64::alphabet::{self, Alphabet};
use ::base64::engine::general_purpose::GeneralPurpose;
use ::base64::engine::{DecodePaddingMode, GeneralPurposeConfig};
use ::base64::Engine;

use super::decoded_text;
use crate::catalogue::{Category, Context, Flags, Output, TransformDef};
use crate::error::{check_output_len, TransformError};
use crate::params::{ParamSpec, Params};

static ENCODE_PARAMS: [ParamSpec; 3] = [
    ParamSpec::choice("variant", &["standard", "url_safe"], "standard"),
    ParamSpec::flag("padding", true),
    ParamSpec::flag("wrap", false),
];

static DECODE_PARAMS: [ParamSpec; 1] = [ParamSpec::choice("output", &["text", "hex"], "text")];

pub(super) static ENCODE: TransformDef = TransformDef {
    id: "encode.base64_encode",
    version: 1,
    category: Category::Encode,
    params: &ENCODE_PARAMS,
    flags: Flags::PLAIN,
    modifier: Some("base64"),
    example: ("hello world", "aGVsbG8gd29ybGQ="),
    run: run_encode,
};

pub(super) static DECODE: TransformDef = TransformDef {
    id: "encode.base64_decode",
    version: 1,
    category: Category::Encode,
    params: &DECODE_PARAMS,
    flags: Flags::PLAIN,
    modifier: None,
    example: ("aGVsbG8gd29ybGQ", "hello world"),
    run: run_decode,
};

/// Line length of wrapped output, as in MIME.
const WRAP: usize = 76;

/// Input bytes encoded between cancellation checks; a multiple of 3, so
/// chunks join without inner padding.
const CHUNK: usize = 48 * 1024;

fn engine(alphabet: &Alphabet, padding: bool) -> GeneralPurpose {
    GeneralPurpose::new(
        alphabet,
        GeneralPurposeConfig::new()
            .with_encode_padding(padding)
            .with_decode_padding_mode(DecodePaddingMode::RequireNone)
            .with_decode_allow_trailing_bits(true),
    )
}

fn run_encode(input: &str, params: &Params, ctx: &Context) -> Result<Output, TransformError> {
    let alphabet = match params.choice("variant") {
        "url_safe" => &alphabet::URL_SAFE,
        _ => &alphabet::STANDARD,
    };
    let engine = engine(alphabet, params.flag("padding"));
    let encoded_len = input.len().div_ceil(3) * 4;
    let wrap = params.flag("wrap");
    check_output_len(encoded_len + if wrap { encoded_len / WRAP } else { 0 })?;
    let mut encoded = String::with_capacity(encoded_len);
    for chunk in input.as_bytes().chunks(CHUNK) {
        ctx.check()?;
        engine.encode_string(chunk, &mut encoded);
    }
    if !wrap || encoded.len() <= WRAP {
        return Ok(Output::new(encoded));
    }
    let mut out = String::with_capacity(encoded.len() + encoded.len() / WRAP);
    for (i, line) in encoded.as_bytes().chunks(WRAP).enumerate() {
        ctx.tick(i)?;
        if i > 0 {
            out.push('\n');
        }
        out.extend(line.iter().map(|&b| char::from(b)));
    }
    Ok(Output::new(out))
}

/// Accepts the standard and URL-safe alphabets (even mixed), optional
/// padding, whitespace anywhere and a leading `data:` URI header.
fn run_decode(input: &str, params: &Params, ctx: &Context) -> Result<Output, TransformError> {
    let start = data_uri_payload(input);
    let mut symbols = Vec::with_capacity(input.len() - start);
    let mut padding = 0;
    for (i, &byte) in input.as_bytes().iter().enumerate().skip(start) {
        ctx.tick(i)?;
        let symbol = match byte {
            b if b.is_ascii_whitespace() => continue,
            b'=' if padding < 2 && !symbols.is_empty() => {
                padding += 1;
                continue;
            }
            b'-' => b'+',
            b'_' => b'/',
            b if b.is_ascii_alphanumeric() || b == b'+' || b == b'/' => b,
            _ => return Err(TransformError::invalid_input("base64.invalid", input, i)),
        };
        if padding > 0 {
            return Err(TransformError::invalid_input("base64.invalid", input, i));
        }
        symbols.push(symbol);
    }
    if symbols.len() % 4 == 1 {
        return Err(TransformError::invalid_input(
            "base64.length",
            input,
            input.len(),
        ));
    }
    ctx.check()?;
    let bytes = engine(&alphabet::STANDARD, false)
        .decode(&symbols)
        .map_err(|_| TransformError::invalid_input("base64.length", input, input.len()))?;
    drop(symbols);
    let text = match params.choice("output") {
        "hex" => super::hex::encode(&bytes, false, false, ctx)?,
        _ => decoded_text(bytes)?,
    };
    Ok(Output::new(text))
}

/// Byte offset where the Base64 payload starts: after a `data:...,` header,
/// else after leading whitespace (which the decoder skips anyway).
fn data_uri_payload(input: &str) -> usize {
    let lead = input.len() - input.trim_start().len();
    let rest = &input.as_bytes()[lead..];
    if rest.len() >= 5 && rest[..5].eq_ignore_ascii_case(b"data:") {
        if let Some(comma) = rest.iter().position(|&b| b == b',') {
            return lead + comma + 1;
        }
    }
    lead
}

#[cfg(test)]
mod tests {
    use crate::{apply, Context, Step, TransformError};

    #[test]
    fn binary_payload_reports_its_size_and_decodes_to_hex() {
        let step = Step::current("encode.base64_decode").unwrap();
        assert_eq!(
            apply(&step, "/9j/4A==", &Context::new()),
            Err(TransformError::BinaryData { bytes: 4 })
        );
        let hex = step.param("output", "hex");
        assert_eq!(
            apply(&hex, "/9j/4A==", &Context::new()).unwrap().text,
            "ffd8ffe0"
        );
    }
}
