//! JSON string literals: escaping text into one and decoding one back.

use std::borrow::Cow;
use std::fmt::Write;

use crate::error::{check_output_len, TransformError};

/// Appends `text` to `out` as a JSON string literal. Only the quote, the
/// backslash and control characters are escaped; everything else is kept.
pub(crate) fn push_quoted(out: &mut String, text: &str) -> Result<(), TransformError> {
    out.push('"');
    let mut chunk = 0;
    for (i, byte) in text.bytes().enumerate() {
        let escape = match byte {
            b'"' => "\\\"",
            b'\\' => "\\\\",
            b'\n' => "\\n",
            b'\r' => "\\r",
            b'\t' => "\\t",
            0x08 => "\\b",
            0x0c => "\\f",
            0..=0x1f => "",
            _ => continue,
        };
        out.push_str(&text[chunk..i]);
        if escape.is_empty() {
            let _ = write!(out, "\\u{byte:04x}");
        } else {
            out.push_str(escape);
        }
        chunk = i + 1;
        check_output_len(out.len())?;
    }
    out.push_str(&text[chunk..]);
    out.push('"');
    check_output_len(out.len())
}

/// `text` as a JSON string literal.
pub(crate) fn quoted(text: &str) -> Result<String, TransformError> {
    let mut out = String::with_capacity(text.len() + 2);
    push_quoted(&mut out, text)?;
    Ok(out)
}

fn hex4(text: &str, at: usize) -> Option<u32> {
    let digits = text.get(at..at + 4)?;
    if digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        u32::from_str_radix(digits, 16).ok()
    } else {
        None
    }
}

/// Decodes the escapes in the body of a string literal. On failure returns
/// the byte offset of the escape that cannot be decoded, a lone surrogate
/// included.
pub(crate) fn unescape(content: &str) -> Result<String, usize> {
    let bytes = content.as_bytes();
    let mut out = String::with_capacity(content.len());
    let mut chunk = 0;
    while let Some(found) = content[chunk..].find('\\') {
        let at = chunk + found;
        out.push_str(&content[chunk..at]);
        let (c, len) = match bytes.get(at + 1) {
            Some(b'"') => ('"', 2),
            Some(b'\\') => ('\\', 2),
            Some(b'/') => ('/', 2),
            Some(b'b') => ('\u{8}', 2),
            Some(b'f') => ('\u{c}', 2),
            Some(b'n') => ('\n', 2),
            Some(b'r') => ('\r', 2),
            Some(b't') => ('\t', 2),
            Some(b'u') => {
                let high = hex4(content, at + 2).ok_or(at)?;
                if (0xd800..0xdc00).contains(&high) {
                    let low = content[at + 6..]
                        .starts_with("\\u")
                        .then(|| hex4(content, at + 8))
                        .flatten()
                        .filter(|low| (0xdc00..0xe000).contains(low))
                        .ok_or(at)?;
                    let code = 0x10000 + ((high - 0xd800) << 10) + (low - 0xdc00);
                    (char::from_u32(code).ok_or(at)?, 12)
                } else {
                    (char::from_u32(high).ok_or(at)?, 6)
                }
            }
            _ => return Err(at),
        };
        out.push(c);
        chunk = at + len;
    }
    out.push_str(&content[chunk..]);
    Ok(out)
}

/// The decoded name of a key token, for duplicate checks and sorting. A key
/// that cannot be decoded compares by its raw body.
pub(crate) fn decode_key(raw: &str) -> Cow<'_, str> {
    let body = &raw[1..raw.len() - 1];
    if body.contains('\\') {
        unescape(body).map_or(Cow::Borrowed(body), Cow::Owned)
    } else {
        Cow::Borrowed(body)
    }
}

/// The offset of the first quote in `content` that no backslash escapes.
pub(crate) fn unescaped_quote(content: &str) -> Option<usize> {
    let bytes = content.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 2,
            b'"' => return Some(i),
            _ => i += 1,
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoting_escapes_only_what_json_requires() {
        assert_eq!(
            quoted("a\"b\\c\n\u{1}/é\u{2028}").unwrap(),
            "\"a\\\"b\\\\c\\n\\u0001/é\u{2028}\""
        );
    }

    #[test]
    fn unescape_joins_surrogate_pairs_and_refuses_lone_ones() {
        assert_eq!(unescape(r"😀 é\/").unwrap(), "😀 é/");
        assert_eq!(unescape(r"ab\ud83d"), Err(2));
        assert_eq!(unescape(r"\x41"), Err(0));
    }
}
