//! Markdown to plain text. Block syntax is handled per line (fences,
//! headings, quotes, list markers, tables, rules); inline syntax (links,
//! images, code spans, emphasis, escapes) is removed while the text stays.
//! Fenced code keeps every byte. Line endings follow the input, and the
//! output is never longer than the input.

use crate::catalogue::Context;
use crate::error::TransformError;

/// How far a link's text or destination may reach before it is treated as
/// literal brackets, which keeps unmatched brackets linear.
const MAX_LINK_TEXT: usize = 1024;
const MAX_LINK_DESTINATION: usize = 4096;

/// Link text nested deeper than this is kept literally.
const MAX_NESTING: usize = 16;

#[derive(Clone, Copy, Debug)]
pub(super) struct Options {
    /// Write links as `text (url)` instead of `text`.
    pub link_urls: bool,
    pub list_markers: bool,
}

pub(super) fn to_plain(
    input: &str,
    options: Options,
    ctx: &Context,
) -> Result<String, TransformError> {
    let eol = if input.contains("\r\n") { "\r\n" } else { "\n" };
    let lines: Vec<&str> = input
        .split('\n')
        .map(|l| l.strip_suffix('\r').unwrap_or(l))
        .collect();
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    let mut fence: Option<(char, usize)> = None;
    let mut table = false;
    let mut paragraph = false;
    let mut index = 0;
    while index < lines.len() {
        ctx.tick(index)?;
        let line = lines[index];
        index += 1;
        let continues_paragraph = std::mem::take(&mut paragraph);
        if let Some((ch, len)) = fence {
            if fence_marker(line)
                .is_some_and(|(c, n, rest)| c == ch && n >= len && rest.trim().is_empty())
            {
                fence = None;
            } else {
                out.push(line.to_owned());
            }
            continue;
        }
        if let Some((ch, len, _)) = fence_marker(line) {
            fence = Some((ch, len));
            continue;
        }
        if table {
            if line.contains('|') && !line.trim().is_empty() {
                out.push(table_row(line, options, ctx)?);
                continue;
            }
            table = false;
        }
        if line.contains('|') && lines.get(index).is_some_and(|next| is_delimiter_row(next)) {
            out.push(table_row(line, options, ctx)?);
            index += 1;
            table = true;
            continue;
        }
        if is_reference_definition(line) {
            continue;
        }
        if is_setext_underline(line) && continues_paragraph {
            continue;
        }
        if is_thematic_break(line) {
            out.push(String::new());
            continue;
        }
        paragraph = !line.trim().is_empty() && heading_text(line).is_none();
        out.push(block_line(line, options, ctx)?);
    }
    Ok(out.join(eol))
}

/// A fence opener or closer: its character, length and the rest.
fn fence_marker(line: &str) -> Option<(char, usize, &str)> {
    let trimmed = indent_up_to_three(line)?;
    let ch = trimmed.chars().next().filter(|c| matches!(c, '`' | '~'))?;
    let len = trimmed.chars().take_while(|&c| c == ch).count();
    (len >= 3).then(|| (ch, len, &trimmed[len..]))
}

fn indent_up_to_three(line: &str) -> Option<&str> {
    let spaces = line.len() - line.trim_start_matches(' ').len();
    (spaces <= 3).then(|| &line[spaces..])
}

fn is_delimiter_row(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.contains('-')
        && trimmed.contains(['|', ':'])
        && trimmed
            .chars()
            .all(|c| matches!(c, '|' | '-' | ':' | ' ' | '\t'))
}

fn is_reference_definition(line: &str) -> bool {
    let Some(rest) = indent_up_to_three(line).and_then(|l| l.strip_prefix('[')) else {
        return false;
    };
    rest.split_once("]:").is_some_and(|(label, dest)| {
        !label.is_empty() && !label.contains(']') && !dest.trim().is_empty()
    })
}

fn is_setext_underline(line: &str) -> bool {
    indent_up_to_three(line).is_some_and(|l| {
        let l = l.trim_end();
        !l.is_empty() && (l.chars().all(|c| c == '=') || l.chars().all(|c| c == '-'))
    })
}

fn is_thematic_break(line: &str) -> bool {
    let Some(l) = indent_up_to_three(line) else {
        return false;
    };
    let mut marks = l.chars().filter(|c| !c.is_whitespace());
    let first = marks.next();
    matches!(first, Some('-' | '*' | '_')) && {
        let rest: Vec<char> = marks.collect();
        rest.len() >= 2 && rest.iter().all(|&c| Some(c) == first)
    }
}

/// Cells of a table row joined by tabs.
fn table_row(line: &str, options: Options, ctx: &Context) -> Result<String, TransformError> {
    let trimmed = line.trim();
    let trimmed = trimmed.strip_prefix('|').unwrap_or(trimmed);
    let trimmed = trimmed.strip_suffix('|').unwrap_or(trimmed);
    let mut cells = Vec::new();
    let mut cell = String::new();
    let mut chars = trimmed.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => match chars.next() {
                Some('|') => cell.push('|'),
                Some(other) => {
                    cell.push('\\');
                    cell.push(other);
                }
                None => cell.push('\\'),
            },
            '|' => cells.push(std::mem::take(&mut cell)),
            c => cell.push(c),
        }
    }
    cells.push(cell);
    let cells = cells
        .iter()
        .map(|c| inline(c.trim(), options, ctx, 0))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(cells.join("\t"))
}

/// One ordinary line: quote, heading and list syntax off, then inline.
fn block_line(line: &str, options: Options, ctx: &Context) -> Result<String, TransformError> {
    let mut rest = line;
    let mut quoted = false;
    while let Some(after) = indent_up_to_three(rest).and_then(|l| l.strip_prefix('>')) {
        rest = after.strip_prefix(' ').unwrap_or(after);
        quoted = true;
    }
    let mut head = String::new();
    if let Some(text) = heading_text(rest) {
        rest = text;
    } else if let Some((indent, marker, text)) = list_item(rest) {
        if options.list_markers {
            head.push_str(indent);
            head.push_str(marker);
            head.push(' ');
        } else if !quoted {
            head.push_str(indent);
        }
        rest = text;
    }
    let body = rest.trim_end_matches(' ');
    let body = body
        .strip_suffix('\\')
        .filter(|b| !b.ends_with('\\'))
        .unwrap_or(body);
    head.push_str(&inline(body, options, ctx, 0)?);
    Ok(head)
}

fn heading_text(line: &str) -> Option<&str> {
    let l = indent_up_to_three(line)?;
    let hashes = l.chars().take_while(|&c| c == '#').count();
    if !(1..=6).contains(&hashes) {
        return None;
    }
    let rest = &l[hashes..];
    if !(rest.is_empty() || rest.starts_with([' ', '\t'])) {
        return None;
    }
    let rest = rest.trim();
    let closed = rest.trim_end_matches('#');
    Some(if closed.is_empty() || closed.ends_with([' ', '\t']) {
        closed.trim_end()
    } else {
        rest
    })
}

/// Indentation, marker and text of a list item line.
fn list_item(line: &str) -> Option<(&str, &str, &str)> {
    let text = line.trim_start_matches([' ', '\t']);
    let indent = &line[..line.len() - text.len()];
    let marker_len = if text.starts_with(['-', '*', '+']) {
        1
    } else {
        let digits = text.chars().take_while(char::is_ascii_digit).count();
        let delimited = (1..=9).contains(&digits) && text[digits..].starts_with(['.', ')']);
        if !delimited {
            return None;
        }
        digits + 1
    };
    let after = &text[marker_len..];
    if !after.starts_with([' ', '\t']) {
        return None;
    }
    Some((
        indent,
        &text[..marker_len],
        after.trim_start_matches([' ', '\t']),
    ))
}

/// An emphasis delimiter run awaiting a partner.
struct Delimiter {
    ch: char,
    /// Byte range of the run in the output buffer.
    start: usize,
    len: usize,
    can_open: bool,
    can_close: bool,
}

/// Removes inline Markdown syntax from one line.
fn inline(
    text: &str,
    options: Options,
    ctx: &Context,
    nesting: usize,
) -> Result<String, TransformError> {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut out = String::with_capacity(text.len());
    let mut delimiters: Vec<Delimiter> = Vec::new();
    let mut no_closer: Vec<usize> = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        ctx.tick(i)?;
        let (at, c) = chars[i];
        match c {
            '\\' if chars
                .get(i + 1)
                .is_some_and(|(_, n)| n.is_ascii_punctuation()) =>
            {
                out.push(chars[i + 1].1);
                i += 2;
            }
            '`' => {
                let run = chars[i..].iter().take_while(|(_, c)| *c == '`').count();
                let closer = (!no_closer.contains(&run))
                    .then(|| find_backtick_run(&chars, i + run, run))
                    .flatten();
                match closer {
                    Some(close) => {
                        let code = &text[chars[i + run].0..chars[close].0];
                        let stripped = code
                            .strip_prefix(' ')
                            .and_then(|c| c.strip_suffix(' '))
                            .filter(|c| !c.trim().is_empty())
                            .unwrap_or(code);
                        out.push_str(stripped);
                        i = close + run;
                    }
                    None => {
                        no_closer.push(run);
                        out.push_str(&"`".repeat(run));
                        i += run;
                    }
                }
            }
            '!' | '[' if nesting < MAX_NESTING => {
                let image = c == '!';
                let open = if image { i + 1 } else { i };
                let bracket = chars.get(open).is_some_and(|(_, c)| *c == '[');
                match bracket.then(|| link(text, &chars, open)).flatten() {
                    Some(found) => {
                        let label = inline(&text[found.text.clone()], options, ctx, nesting + 1)?;
                        out.push_str(&label);
                        let url = found.destination.map(|d| text[d].trim());
                        if let Some(url) = url
                            .filter(|u| options.link_urls && !image && !u.is_empty() && *u != label)
                        {
                            out.push_str(" (");
                            out.push_str(url);
                            out.push(')');
                        }
                        i = found.end;
                    }
                    None => {
                        out.push(c);
                        i += 1;
                    }
                }
            }
            '<' => match autolink(&text[at..]) {
                Some((inner, len)) => {
                    out.push_str(inner);
                    i += text[at..at + len].chars().count();
                }
                None => {
                    out.push(c);
                    i += 1;
                }
            },
            '*' | '_' | '~' => {
                let run = chars[i..].iter().take_while(|(_, x)| *x == c).count();
                let prev = i.checked_sub(1).map(|p| chars[p].1);
                let next = chars.get(i + run).map(|(_, c)| *c);
                let start = out.len();
                out.push_str(&c.to_string().repeat(run));
                let space = |c: Option<char>| c.is_none_or(char::is_whitespace);
                let alnum = |c: Option<char>| c.is_some_and(char::is_alphanumeric);
                let mut can_open = !space(next);
                let mut can_close = !space(prev);
                if c == '_' {
                    can_open &= !alnum(prev);
                    can_close &= !alnum(next);
                }
                if c != '~' || run == 2 {
                    delimiters.push(Delimiter {
                        ch: c,
                        start,
                        len: run,
                        can_open,
                        can_close,
                    });
                }
                i += run;
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    Ok(remove_emphasis(out, &delimiters))
}

fn find_backtick_run(chars: &[(usize, char)], from: usize, run: usize) -> Option<usize> {
    let mut i = from;
    while i < chars.len() {
        if chars[i].1 == '`' {
            let len = chars[i..].iter().take_while(|(_, c)| *c == '`').count();
            if len == run {
                return Some(i);
            }
            i += len;
        } else {
            i += 1;
        }
    }
    None
}

struct Link {
    /// Byte ranges into the line.
    text: std::ops::Range<usize>,
    destination: Option<std::ops::Range<usize>>,
    /// Char index just past the link.
    end: usize,
}

/// `[text](destination "title")`, `[text][ref]` or `[text][]` at `open`.
fn link(text: &str, chars: &[(usize, char)], open: usize) -> Option<Link> {
    let byte = |i: usize| chars.get(i).map_or(text.len(), |(b, _)| *b);
    let mut depth = 0usize;
    let mut close = None;
    let mut i = open;
    while i < chars.len().min(open + MAX_LINK_TEXT) {
        match chars[i].1 {
            '\\' => i += 1,
            '[' => depth += 1,
            ']' if depth <= 1 => {
                close = Some(i);
                break;
            }
            ']' => depth -= 1,
            _ => {}
        }
        i += 1;
    }
    let close = close?;
    let label = byte(open + 1)..byte(close);
    match chars.get(close + 1).map(|(_, c)| *c) {
        Some('(') => {
            let mut parens = 0usize;
            let end = chars
                .iter()
                .enumerate()
                .skip(close + 1)
                .take(MAX_LINK_DESTINATION)
                .find(|(_, (_, c))| {
                    match c {
                        '(' => parens += 1,
                        ')' => parens -= 1,
                        _ => {}
                    }
                    parens == 0
                })
                .map(|(i, _)| i)?;
            let inner = &text[byte(close + 2)..byte(end)];
            let start = byte(close + 2);
            let dest = destination_span(inner);
            Some(Link {
                text: label,
                destination: Some(start + dest.start..start + dest.end),
                end: end + 1,
            })
        }
        Some('[') => {
            let end = chars
                .iter()
                .enumerate()
                .skip(close + 2)
                .take(MAX_LINK_TEXT)
                .find(|(_, (_, c))| *c == ']')
                .map(|(i, _)| i)?;
            Some(Link {
                text: label,
                destination: None,
                end: end + 1,
            })
        }
        _ => None,
    }
}

/// The URL inside `(...)`, without angle brackets or a title.
fn destination_span(inner: &str) -> std::ops::Range<usize> {
    let lead = inner.len() - inner.trim_start().len();
    let rest = &inner[lead..];
    if let Some(bracketed) = rest.strip_prefix('<') {
        if let Some(end) = bracketed.find('>') {
            return lead + 1..lead + 1 + end;
        }
    }
    let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
    lead..lead + end
}

/// `<scheme:...>` or `<user@host>`: the inner text and the byte length.
fn autolink(text: &str) -> Option<(&str, usize)> {
    let end = text.find('>')?;
    let inner = &text[1..end];
    let scheme = inner.split_once(':').is_some_and(|(s, _)| {
        s.len() >= 2
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-'))
    });
    let email = inner.contains('@') && !inner.starts_with('@');
    let clean = !inner.is_empty() && !inner.contains(|c: char| c.is_whitespace() || c == '<');
    (clean && (scheme || email)).then_some((inner, end + 1))
}

/// Deletes matched emphasis delimiter runs; unmatched ones stay literal.
fn remove_emphasis(text: String, delimiters: &[Delimiter]) -> String {
    let mut remove = vec![0usize; delimiters.len()];
    let mut opened: Vec<usize> = Vec::new();
    for (index, d) in delimiters.iter().enumerate() {
        if d.can_close {
            if let Some(pos) = opened.iter().rposition(|&o| delimiters[o].ch == d.ch) {
                let opener = opened[pos];
                let n = delimiters[opener].len.min(d.len);
                remove[opener] = n;
                remove[index] = n;
                opened.truncate(pos);
                continue;
            }
        }
        if d.can_open {
            opened.push(index);
        }
    }
    if remove.iter().all(|&n| n == 0) {
        return text;
    }
    let mut out = String::with_capacity(text.len());
    let mut copied = 0;
    for (d, &n) in delimiters.iter().zip(&remove) {
        if n == 0 {
            continue;
        }
        out.push_str(&text[copied..d.start]);
        out.push_str(&text[d.start + n..d.start + d.len]);
        copied = d.start + d.len;
    }
    out.push_str(&text[copied..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEFAULT: Options = Options {
        link_urls: true,
        list_markers: true,
    };

    fn plain(text: &str, options: Options) -> String {
        to_plain(text, options, &Context::new()).unwrap()
    }

    #[test]
    fn blocks_lose_their_syntax_and_fences_keep_bytes() {
        let input = "Title\n=====\n> # Quoted *head*\n\n```rust\nlet x = *p; // [a](b)\n```\n---\n[ref]: https://x.test";
        assert_eq!(
            plain(input, DEFAULT),
            "Title\nQuoted head\n\nlet x = *p; // [a](b)\n"
        );
        assert_eq!(
            plain("| a | b |\n|---|:-:|\n| `1\\|` | **2** |\nafter", DEFAULT),
            "a\tb\n1|\t2\nafter"
        );
    }

    #[test]
    fn inline_syntax_and_list_markers() {
        let options = Options {
            link_urls: false,
            list_markers: false,
        };
        assert_eq!(
            plain("- see [**docs**](https://x.test \"t\") and ![logo](l.png)\n1. snake_case_name and 2 * 3 * 4", options),
            "see docs and logo\nsnake_case_name and 2 * 3 * 4"
        );
        assert_eq!(
            plain(
                "* <https://a.test> `code` \\*lit\\* ~~gone~~ [x][r]\r\nnext  ",
                DEFAULT
            ),
            "* https://a.test code *lit* gone x\r\nnext"
        );
    }
}
