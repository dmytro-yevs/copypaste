//! JWT decoding: header and payload as indented JSON, exactly as encoded
//! (key order, numbers and escapes kept). The signature is never checked.

use base64::alphabet;
use base64::engine::general_purpose::{GeneralPurpose, GeneralPurposeConfig};
use base64::engine::DecodePaddingMode;
use base64::Engine;
use serde_json::{Map, Value};

use crate::catalogue::{Context, Output};
use crate::error::{TransformError, Warning};

/// URL-safe alphabet with or without padding.
const BASE64URL: GeneralPurpose = GeneralPurpose::new(
    &alphabet::URL_SAFE,
    GeneralPurposeConfig::new().with_decode_padding_mode(DecodePaddingMode::Indifferent),
);

pub(super) fn decode(input: &str, ctx: &Context) -> Result<Output, TransformError> {
    let start = input.len() - input.trim_start().len();
    let mut token = input.trim();
    let mut offset = start;
    if token
        .get(..7)
        .is_some_and(|p| p.eq_ignore_ascii_case("bearer "))
    {
        let rest = token[7..].trim_start();
        offset += token.len() - rest.len();
        token = rest;
    }
    let segments: Vec<&str> = token.split('.').collect();
    match segments.len() {
        3 => {}
        5 => {
            return Err(TransformError::invalid_input(
                "jwt.encrypted",
                input,
                offset,
            ))
        }
        _ => {
            return Err(TransformError::invalid_input(
                "jwt.malformed",
                input,
                offset,
            ))
        }
    }
    let header = segment(segments[0], "jwt.header_invalid", input, offset)?;
    let payload_offset = offset + segments[0].len() + 1;
    let payload = segment(segments[1], "jwt.payload_invalid", input, payload_offset)?;
    ctx.check()?;

    let mut text = String::with_capacity(header.0.len() + payload.0.len() + 64);
    text.push_str("{\n  \"header\": ");
    reindent(&header.0, 1, &mut text);
    text.push_str(",\n  \"payload\": ");
    reindent(&payload.0, 1, &mut text);
    text.push_str("\n}");

    let mut warnings = vec![Warning::new("jwt.signature_not_verified")];
    let now = jiff::Timestamp::now().as_second() as f64;
    let claim = |name: &str| payload.1.get(name).and_then(Value::as_f64);
    if claim("exp").is_some_and(|exp| exp <= now) {
        warnings.push(Warning::new("jwt.expired"));
    }
    if claim("nbf").is_some_and(|nbf| nbf > now) {
        warnings.push(Warning::new("jwt.not_yet_valid"));
    }
    Ok(Output {
        text,
        warnings,
        sensitive: true,
    })
}

/// One segment's JSON object, as written and parsed.
fn segment(
    encoded: &str,
    code: &'static str,
    input: &str,
    offset: usize,
) -> Result<(String, Map<String, Value>), TransformError> {
    let invalid = || TransformError::invalid_input(code, input, offset);
    let bytes = BASE64URL.decode(encoded).map_err(|_| invalid())?;
    let json = String::from_utf8(bytes).map_err(|_| invalid())?;
    match serde_json::from_str::<Value>(&json) {
        Ok(Value::Object(map)) => Ok((json, map)),
        _ => Err(invalid()),
    }
}

/// Re-indents valid JSON by two spaces per level, starting at `level`,
/// without touching any token.
fn reindent(json: &str, level: usize, out: &mut String) {
    let newline = |out: &mut String, level: usize| {
        out.push('\n');
        out.push_str(&"  ".repeat(level));
    };
    let mut level = level;
    let mut in_string = false;
    let mut escaped = false;
    let mut chars = json.chars().peekable();
    while let Some(c) = chars.next() {
        if in_string {
            out.push(c);
            match c {
                _ if escaped => escaped = false,
                '\\' => escaped = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match c {
            '"' => {
                in_string = true;
                out.push(c);
            }
            '{' | '[' => {
                out.push(c);
                while chars.peek().is_some_and(char::is_ascii_whitespace) {
                    chars.next();
                }
                if let Some(close) = chars.next_if(|n| matches!(n, '}' | ']')) {
                    out.push(close);
                } else {
                    level += 1;
                    newline(out, level);
                }
            }
            '}' | ']' => {
                level = level.saturating_sub(1);
                newline(out, level);
                out.push(c);
            }
            ',' => {
                out.push(c);
                newline(out, level);
            }
            ':' => out.push_str(": "),
            c if c.is_ascii_whitespace() => {}
            c => out.push(c),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn token(header: &str, payload: &str) -> String {
        let encode = |s: &str| base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(s);
        format!("{}.{}.sig", encode(header), encode(payload))
    }

    #[test]
    fn claims_keep_their_spelling_and_order() {
        let jwt = token(
            r#"{"typ":"JWT","alg":"none"}"#,
            r#"{"n":12345678901234567890,"a":[],"s":"\u00e9"}"#,
        );
        let out = decode(&format!("Bearer {jwt}"), &Context::new()).unwrap();
        assert_eq!(
            out.text,
            "{\n  \"header\": {\n    \"typ\": \"JWT\",\n    \"alg\": \"none\"\n  },\n  \"payload\": {\n    \"n\": 12345678901234567890,\n    \"a\": [],\n    \"s\": \"\\u00e9\"\n  }\n}"
        );
        assert!(out.sensitive);
    }

    #[test]
    fn a_bad_segment_is_named_with_its_offset() {
        let jwt = token(r#"{"alg":"none"}"#, "not json");
        let err = decode(&jwt, &Context::new()).unwrap_err();
        assert_eq!(err.code(), "jwt.payload_invalid");
        let TransformError::InvalidInput { location, .. } = err else {
            unreachable!()
        };
        assert_eq!(location.unwrap().offset, jwt.find('.').unwrap() + 1);
    }
}
