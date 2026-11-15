//! Find and replace, match extraction and character removal.
//!
//! User patterns run on the linear-time `regex` engine with explicit
//! compiled-size limits, so a pathological pattern costs time proportional
//! to the input; the run's deadline still applies between matches.

use std::collections::HashSet;
use std::net::{Ipv4Addr, Ipv6Addr};

use regex::{Regex, RegexBuilder};
use unicode_segmentation::UnicodeSegmentation;

use crate::catalogue::{Context, Output};
use crate::error::{check_output_len, Location, TransformError, Warning};
use crate::url::scan::http_urls;

/// Compiled program limits for user patterns.
const SIZE_LIMIT: usize = 2 * 1024 * 1024;
const DFA_SIZE_LIMIT: usize = 4 * 1024 * 1024;

pub(super) struct Pattern<'p> {
    pub text: &'p str,
    pub literal: bool,
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub multiline: bool,
}

fn compile(pattern: &Pattern) -> Result<Regex, TransformError> {
    let source = if pattern.literal {
        regex::escape(pattern.text)
    } else {
        pattern.text.to_owned()
    };
    let wrapped = if pattern.whole_word {
        format!(r"\b(?:{source})\b")
    } else {
        source.clone()
    };
    let build = |source: &str| {
        RegexBuilder::new(source)
            .case_insensitive(!pattern.case_sensitive)
            .multi_line(pattern.multiline)
            .crlf(true)
            .size_limit(SIZE_LIMIT)
            .dfa_size_limit(DFA_SIZE_LIMIT)
            .build()
    };
    build(&wrapped).map_err(|err| {
        // The wrapper can hide where the user's own pattern went wrong.
        let err = match (pattern.whole_word, err) {
            (true, regex::Error::Syntax(_)) => build(&source).err(),
            (_, err) => Some(err),
        };
        let column = match &err {
            Some(regex::Error::CompiledTooBig(_)) => {
                return TransformError::InvalidInput {
                    code: "regex.too_expensive",
                    location: None,
                }
            }
            Some(regex::Error::Syntax(message)) => caret_column(message),
            _ => None,
        };
        let offset = column.map(|c| {
            pattern
                .text
                .char_indices()
                .nth(c)
                .map_or(pattern.text.len(), |(i, _)| i)
        });
        TransformError::InvalidInput {
            code: "regex.invalid_pattern",
            location: offset.map(|o| Location::of(pattern.text, o)),
        }
    })
}

/// Zero-based character column of the first `^` under a one-line pattern
/// in the parser's message (`    pattern` / `    ^^^`).
fn caret_column(message: &str) -> Option<usize> {
    let mut lines = message.lines().skip(1);
    let pattern_line = lines.next()?;
    let carets = lines.next()?;
    if !pattern_line.starts_with("    ") || !carets.trim().starts_with('^') {
        return None;
    }
    carets.find('^')?.checked_sub(4)
}

pub(super) fn replace(
    input: &str,
    pattern: &Pattern,
    replacement: &str,
    ctx: &Context,
) -> Result<Output, TransformError> {
    if pattern.text.is_empty() {
        return Ok(Output::new(input.to_owned()).warn(Warning::new("find.empty_pattern")));
    }
    let regex = compile(pattern)?;
    let expand = !pattern.literal && replacement.contains('$');
    let mut out = String::with_capacity(input.len());
    let mut copied = 0;
    if expand {
        for (i, caps) in regex.captures_iter(input).enumerate() {
            ctx.tick(i)?;
            let found = caps.get(0).map_or(0..0, |m| m.range());
            out.push_str(&input[copied..found.start]);
            caps.expand(replacement, &mut out);
            copied = found.end;
            check_output_len(out.len())?;
        }
    } else {
        for (i, found) in regex.find_iter(input).enumerate() {
            ctx.tick(i)?;
            out.push_str(&input[copied..found.start()]);
            out.push_str(replacement);
            copied = found.end();
            check_output_len(out.len())?;
        }
    }
    out.push_str(&input[copied..]);
    Ok(Output::new(out))
}

pub(super) fn extract(
    input: &str,
    preset: &str,
    custom: &str,
    dedupe: bool,
    ctx: &Context,
) -> Result<Output, TransformError> {
    let candidates: Box<dyn Iterator<Item = &str>> = match preset {
        "urls" => Box::new(http_urls(input).map(|r| &input[r])),
        "custom" => {
            if custom.is_empty() {
                return Ok(Output::new(String::new()).warn(Warning::new("find.empty_pattern")));
            }
            let regex = compile(&Pattern {
                text: custom,
                literal: false,
                case_sensitive: true,
                whole_word: false,
                multiline: true,
            })?;
            Box::new(
                regex
                    .find_iter(input)
                    .map(|m| m.as_str())
                    .collect::<Vec<_>>()
                    .into_iter(),
            )
        }
        preset => {
            let (source, valid): (&str, fn(&str) -> bool) = match preset {
                "emails" => (
                    r"[A-Za-z0-9._%+-]+@[A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)*\.[A-Za-z]{2,}",
                    |_| true,
                ),
                "numbers" => (r"-?[0-9]+(?:\.[0-9]+)?", |_| true),
                "ipv4" => (r"\b[0-9]{1,3}(?:\.[0-9]{1,3}){3}\b", |s| {
                    s.parse::<Ipv4Addr>().is_ok()
                }),
                "ipv6" => (
                    r"[0-9A-Fa-f]{0,4}(?::[0-9A-Fa-f]{0,4}){2,7}(?:\.[0-9]{1,3}){0,3}",
                    |s| s.parse::<Ipv6Addr>().is_ok(),
                ),
                "hex_colors" => (
                    r"#(?:[0-9a-fA-F]{8}|[0-9a-fA-F]{6}|[0-9a-fA-F]{3,4})\b",
                    |_| true,
                ),
                _ => (
                    r"\b[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}\b",
                    |_| true,
                ),
            };
            let regex = Regex::new(source).map_err(|_| TransformError::InvalidInput {
                code: "regex.invalid_pattern",
                location: None,
            })?;
            let found: Vec<&str> = regex
                .find_iter(input)
                .map(|m| m.as_str())
                .filter(|s| valid(s))
                .collect();
            Box::new(found.into_iter())
        }
    };
    let mut seen = HashSet::new();
    let mut out = String::new();
    for (i, found) in candidates.enumerate() {
        ctx.tick(i)?;
        if dedupe && !seen.insert(found) {
            continue;
        }
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(found);
        check_output_len(out.len())?;
    }
    let mut output = Output::new(out);
    if output.text.is_empty() {
        output = output.warn(Warning::new("extract.no_matches"));
    }
    Ok(output)
}

pub(super) fn remove(
    input: &str,
    class: &str,
    chars: &str,
    ctx: &Context,
) -> Result<Output, TransformError> {
    let class_pattern = match class {
        "digits" => r"\p{Nd}+",
        "punctuation" => r"[\p{P}$+<=>^`|~]+",
        "non_ascii" => r"[^\x00-\x7F]+",
        "emoji" => return Ok(Output::new(remove_emoji(input, ctx)?)),
        _ => {
            let set: HashSet<char> = chars.chars().collect();
            let mut out = String::with_capacity(input.len());
            for (i, c) in input.chars().enumerate() {
                ctx.tick(i)?;
                if !set.contains(&c) {
                    out.push(c);
                }
            }
            return Ok(Output::new(out));
        }
    };
    let regex = Regex::new(class_pattern).map_err(|_| TransformError::InvalidInput {
        code: "regex.invalid_pattern",
        location: None,
    })?;
    ctx.check()?;
    Ok(Output::new(regex.replace_all(input, "").into_owned()))
}

/// Drops whole grapheme clusters that render as emoji: emoji-presentation
/// characters, text symbols forced to emoji style, flags and keycaps.
fn remove_emoji(input: &str, ctx: &Context) -> Result<String, TransformError> {
    let emoji = Regex::new(
        r"\p{Emoji_Presentation}|\p{Extended_Pictographic}\x{FE0F}|[\x{1F1E6}-\x{1F1FF}]|\x{20E3}",
    )
    .map_err(|_| TransformError::InvalidInput {
        code: "regex.invalid_pattern",
        location: None,
    })?;
    let mut out = String::with_capacity(input.len());
    for (i, grapheme) in input.graphemes(true).enumerate() {
        ctx.tick(i)?;
        if grapheme.is_ascii() || !emoji.is_match(grapheme) {
            out.push_str(grapheme);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pattern(text: &str, whole_word: bool) -> Pattern<'_> {
        Pattern {
            text,
            literal: false,
            case_sensitive: true,
            whole_word,
            multiline: true,
        }
    }

    #[test]
    fn invalid_patterns_point_into_the_users_pattern() {
        for whole_word in [false, true] {
            let err = compile(&pattern("ab(cd", whole_word)).unwrap_err();
            assert_eq!(
                err,
                TransformError::InvalidInput {
                    code: "regex.invalid_pattern",
                    location: Some(Location::of("ab(cd", 2)),
                }
            );
        }
    }

    #[test]
    fn pathological_patterns_stay_linear() {
        let input = "a".repeat(100_000);
        let started = std::time::Instant::now();
        let out = replace(&input, &pattern("(a+)+$", false), "x", &Context::new()).unwrap();
        assert_eq!(out.text, "x");
        assert!(started.elapsed() < std::time::Duration::from_secs(2));
    }
}
