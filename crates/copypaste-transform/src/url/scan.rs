//! Finds http(s) URLs in free text and rewrites them in place, leaving every
//! byte outside the URLs untouched.

use std::ops::Range;

use crate::catalogue::Context;
use crate::error::{check_output_len, TransformError};

/// Byte ranges of the http(s) URLs in `text`, in order.
pub(crate) fn http_urls(text: &str) -> impl Iterator<Item = Range<usize>> + '_ {
    let mut from = 0;
    std::iter::from_fn(move || {
        while let Some(start) = next_scheme(text, from) {
            let range = start..url_end(text, start);
            from = range.end.max(start + 1);
            if has_host(&text[range.clone()]) {
                return Some(range);
            }
        }
        from = text.len();
        None
    })
}

/// Replaces every URL for which `rewrite` returns a new spelling. The
/// closure gets the URL and its byte offset in `text`.
pub(crate) fn rewrite_urls(
    text: &str,
    ctx: &Context,
    mut rewrite: impl FnMut(&str, usize) -> Option<String>,
) -> Result<String, TransformError> {
    let mut out = String::with_capacity(text.len());
    let mut copied = 0;
    for (i, range) in http_urls(text).enumerate() {
        ctx.tick(i)?;
        if let Some(new) = rewrite(&text[range.clone()], range.start) {
            out.push_str(&text[copied..range.start]);
            out.push_str(&new);
            copied = range.end;
            check_output_len(out.len())?;
        }
    }
    out.push_str(&text[copied..]);
    Ok(out)
}

fn next_scheme(text: &str, from: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    (from..bytes.len()).find(|&i| {
        let rest = &bytes[i..];
        let scheme = [&b"http://"[..], b"https://"]
            .iter()
            .any(|s| rest.len() >= s.len() && rest[..s.len()].eq_ignore_ascii_case(s));
        scheme && (i == 0 || !bytes[i - 1].is_ascii_alphanumeric())
    })
}

/// End of the URL starting at `start`: the first space, quote or angle
/// bracket, minus trailing punctuation and unbalanced closing brackets that
/// belong to the surrounding prose.
fn url_end(text: &str, start: usize) -> usize {
    let end = text[start..]
        .char_indices()
        .find(|(_, c)| c.is_whitespace() || c.is_control() || is_delimiter(*c))
        .map_or(text.len(), |(i, _)| start + i);
    let mut url = &text[start..end];
    while let Some(last) = url.chars().next_back() {
        let unbalanced = |open: char| url.matches(open).count() < url.matches(last).count();
        let trailing = match last {
            '.' | ',' | ':' | ';' | '!' | '?' | '\'' | '*' => true,
            ')' => unbalanced('('),
            ']' => unbalanced('['),
            '}' => unbalanced('{'),
            _ => false,
        };
        if !trailing {
            break;
        }
        url = &url[..url.len() - last.len_utf8()];
    }
    start + url.len()
}

/// Characters that never belong to a URL written in prose, including
/// full-width punctuation, which CJK text puts right after a link.
fn is_delimiter(c: char) -> bool {
    matches!(
        c,
        '<' | '>'
            | '"'
            | '`'
            | '。'
            | '、'
            | '，'
            | '！'
            | '？'
            | '：'
            | '；'
            | '「'
            | '」'
            | '『'
            | '』'
            | '（'
            | '）'
    )
}

fn has_host(url: &str) -> bool {
    url.split_once("://")
        .is_some_and(|(_, rest)| !rest.is_empty() && !rest.starts_with(['/', '?', '#']))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found(text: &str) -> Vec<&str> {
        http_urls(text).map(|r| &text[r]).collect()
    }

    #[test]
    fn prose_punctuation_and_brackets_stay_outside() {
        assert_eq!(
            found("See (https://a.test/x_(y)), then HTTP://b.test/?q=1. Or <https://c.test>"),
            [
                "https://a.test/x_(y)",
                "HTTP://b.test/?q=1",
                "https://c.test"
            ]
        );
        assert_eq!(found("見てhttps://a.test/p。次"), ["https://a.test/p"]);
        assert!(found("xhttps://a.test https:// ftp://a.test").is_empty());
    }
}
