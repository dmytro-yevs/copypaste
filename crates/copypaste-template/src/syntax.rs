//! Template syntax: `{{name key=value | modifier}}` placeholders in text.
//!
//! Parsing never fails. A placeholder that cannot be understood keeps its
//! source span and the issue, so lint can report it and render can copy it
//! verbatim.

use std::ops::Range;

use crate::dates::{self, DateSpec};
use crate::fields::FieldSpec;
use crate::lint::Issue;
use crate::nested::SnippetRef;
use crate::MAX_CLIPBOARD_INDEX;

/// A parsed template body. Parse it once and render it many times.
#[derive(Debug)]
pub struct Template {
    source: String,
    segments: Vec<Segment>,
}

#[derive(Debug)]
pub(crate) enum Segment {
    Text(String),
    Placeholder(Placeholder),
}

#[derive(Debug)]
pub(crate) struct Placeholder {
    /// Byte range of the whole placeholder, braces included, in the source.
    pub span: Range<usize>,
    pub parsed: Result<Parsed, Issue>,
}

#[derive(Debug)]
pub(crate) struct Parsed {
    pub directive: Directive,
    pub modifiers: Vec<String>,
}

#[derive(Debug)]
pub(crate) enum Directive {
    Date(Box<DateSpec>),
    Clipboard { index: u8, required: bool },
    Cursor,
    Field(FieldSpec),
    Snippet(SnippetRef),
    Uuid,
    Random { min: i64, max: i64 },
}

/// Parses a template body.
pub fn parse(source: &str) -> Template {
    let bytes = source.as_bytes();
    let mut segments = Vec::new();
    let mut text = String::new();
    let mut literal_start = 0;
    let mut index = 0;
    while index < bytes.len() {
        let rest = &bytes[index..];
        if rest.starts_with(b"\\{{") || rest.starts_with(b"\\}}") {
            text.push_str(&source[literal_start..index]);
            text.push_str(&source[index + 1..index + 3]);
            index += 3;
            literal_start = index;
        } else if rest.starts_with(b"{{") {
            text.push_str(&source[literal_start..index]);
            if !text.is_empty() {
                segments.push(Segment::Text(std::mem::take(&mut text)));
            }
            let Some(close) = find_close(bytes, index + 2) else {
                segments.push(Segment::Placeholder(Placeholder {
                    span: index..bytes.len(),
                    parsed: Err(Issue::Unclosed),
                }));
                literal_start = bytes.len();
                break;
            };
            segments.push(Segment::Placeholder(Placeholder {
                span: index..close + 2,
                parsed: parse_inner(&source[index + 2..close]),
            }));
            index = close + 2;
            literal_start = index;
        } else {
            index += 1;
        }
    }
    text.push_str(&source[literal_start..]);
    if !text.is_empty() {
        segments.push(Segment::Text(text));
    }
    Template {
        source: source.to_owned(),
        segments,
    }
}

impl Template {
    /// The body this template was parsed from.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Whether the body holds at least one well-formed placeholder.
    pub fn has_placeholders(&self) -> bool {
        self.placeholders().any(|p| p.parsed.is_ok())
    }

    pub(crate) fn segments(&self) -> &[Segment] {
        &self.segments
    }

    pub(crate) fn placeholders(&self) -> impl Iterator<Item = &Placeholder> {
        self.segments.iter().filter_map(|segment| match segment {
            Segment::Placeholder(placeholder) => Some(placeholder),
            Segment::Text(_) => None,
        })
    }

    pub(crate) fn verbatim(&self, placeholder: &Placeholder) -> &str {
        &self.source[placeholder.span.clone()]
    }
}

/// Finds the `}}` that closes a placeholder opened before `from`. Inside a
/// quoted value `}}` is literal and `\"` and `\\` are escapes.
fn find_close(bytes: &[u8], from: usize) -> Option<usize> {
    let mut quoted = false;
    let mut index = from;
    while index < bytes.len() {
        match bytes[index] {
            b'\\' if quoted => index += 1,
            b'"' => quoted = !quoted,
            b'}' if !quoted && bytes.get(index + 1) == Some(&b'}') => return Some(index),
            _ => {}
        }
        index += 1;
    }
    None
}

struct Cursor<'a> {
    text: &'a str,
    at: usize,
}

impl<'a> Cursor<'a> {
    fn peek(&self) -> Option<char> {
        self.text[self.at..].chars().next()
    }

    fn skip_whitespace(&mut self) {
        while let Some(c) = self.peek().filter(|c| c.is_whitespace()) {
            self.at += c.len_utf8();
        }
    }

    fn eat(&mut self, expected: char) -> bool {
        if self.peek() == Some(expected) {
            self.at += expected.len_utf8();
            true
        } else {
            false
        }
    }

    /// Lowercase ASCII identifier; `extra` lists further allowed characters.
    fn identifier(&mut self, extra: &[char]) -> Option<&'a str> {
        let start = self.at;
        if !self.peek().is_some_and(|c| c.is_ascii_lowercase()) {
            return None;
        }
        while let Some(c) = self
            .peek()
            .filter(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || extra.contains(c))
        {
            self.at += c.len_utf8();
        }
        Some(&self.text[start..self.at])
    }

    fn value(&mut self) -> Option<String> {
        if self.eat('"') {
            let mut value = String::new();
            loop {
                match self.peek()? {
                    '"' => {
                        self.at += 1;
                        return Some(value);
                    }
                    '\\' => {
                        self.at += 1;
                        match self.peek()? {
                            c @ ('"' | '\\') => {
                                value.push(c);
                                self.at += 1;
                            }
                            _ => value.push('\\'),
                        }
                    }
                    c => {
                        value.push(c);
                        self.at += c.len_utf8();
                    }
                }
            }
        }
        let start = self.at;
        while let Some(c) = self
            .peek()
            .filter(|c| !c.is_whitespace() && !matches!(c, '|' | '"' | '='))
        {
            self.at += c.len_utf8();
        }
        (self.at > start).then(|| self.text[start..self.at].to_owned())
    }
}

/// Parameters of one placeholder, consumed as the directive reads them.
pub(crate) struct Params(Vec<(String, String)>);

impl Params {
    pub fn take(&mut self, key: &str) -> Option<String> {
        let index = self.0.iter().position(|(k, _)| k == key)?;
        Some(self.0.remove(index).1)
    }

    pub fn take_bool(&mut self, key: &str) -> Result<Option<bool>, Issue> {
        match self.take(key).as_deref() {
            None => Ok(None),
            Some("true") => Ok(Some(true)),
            Some("false") => Ok(Some(false)),
            Some(_) => Err(Issue::InvalidValue { key: key.into() }),
        }
    }

    pub fn take_i64(&mut self, key: &str) -> Result<i64, Issue> {
        let value = self
            .take(key)
            .ok_or_else(|| Issue::MissingParameter { key: key.into() })?;
        value
            .parse()
            .map_err(|_| Issue::InvalidValue { key: key.into() })
    }

    /// Fails on the first parameter no directive read.
    pub fn finish(self) -> Result<(), Issue> {
        match self.0.into_iter().next() {
            Some((key, _)) => Err(Issue::UnknownParameter { key }),
            None => Ok(()),
        }
    }
}

fn parse_inner(inner: &str) -> Result<Parsed, Issue> {
    let mut cursor = Cursor { text: inner, at: 0 };
    cursor.skip_whitespace();
    let name_start = cursor.at;
    let name = cursor.identifier(&['_']).ok_or(Issue::Malformed)?;
    let mut params = Vec::new();
    loop {
        cursor.skip_whitespace();
        match cursor.peek() {
            None | Some('|') => break,
            Some(_) => {}
        }
        let key = cursor.identifier(&['_']).ok_or(Issue::Malformed)?;
        cursor.skip_whitespace();
        if !cursor.eat('=') {
            return Err(Issue::Malformed);
        }
        cursor.skip_whitespace();
        let value = cursor.value().ok_or(Issue::Malformed)?;
        if params.iter().any(|(k, _)| k == key) {
            return Err(Issue::DuplicateParameter { key: key.into() });
        }
        params.push((key.to_owned(), value));
    }
    let key = inner[name_start..cursor.at].trim_end().to_owned();
    let mut modifiers = Vec::new();
    while cursor.eat('|') {
        cursor.skip_whitespace();
        let id = cursor
            .identifier(&['_', '-', '.'])
            .ok_or(Issue::Malformed)?;
        modifiers.push(id.to_owned());
        cursor.skip_whitespace();
    }
    if cursor.peek().is_some() {
        return Err(Issue::Malformed);
    }
    let mut params = Params(params);
    let directive = directive(name, &mut params, key)?;
    params.finish()?;
    if matches!(directive, Directive::Cursor) && !modifiers.is_empty() {
        return Err(Issue::Malformed);
    }
    Ok(Parsed {
        directive,
        modifiers,
    })
}

fn directive(name: &str, params: &mut Params, key: String) -> Result<Directive, Issue> {
    Ok(match name {
        "date" | "time" | "datetime" => {
            Directive::Date(Box::new(dates::parse_spec(name, params, key)?))
        }
        "clipboard" => {
            let index = match params.take("index") {
                None => 0,
                Some(value) => value
                    .parse::<u8>()
                    .ok()
                    .filter(|index| *index <= MAX_CLIPBOARD_INDEX)
                    .ok_or_else(|| Issue::InvalidValue {
                        key: "index".into(),
                    })?,
            };
            let required = params.take_bool("required")?.unwrap_or(false);
            Directive::Clipboard { index, required }
        }
        "cursor" => Directive::Cursor,
        "field" => Directive::Field(FieldSpec::parse(params)?),
        "snippet" => Directive::Snippet(SnippetRef::parse(params)?),
        "uuid" => Directive::Uuid,
        "random" => {
            let min = params.take_i64("min")?;
            let max = params.take_i64("max")?;
            if min > max {
                return Err(Issue::InvalidValue { key: "max".into() });
            }
            Directive::Random { min, max }
        }
        _ => return Err(Issue::UnknownPlaceholder { name: name.into() }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(template: &Template) -> Vec<String> {
        template
            .segments()
            .iter()
            .map(|segment| match segment {
                Segment::Text(text) => text.clone(),
                Segment::Placeholder(p) => format!("<{}>", template.verbatim(p)),
            })
            .collect()
    }

    #[test]
    fn escapes_and_quoted_braces() {
        let template = parse(r#"a \{{x\}} \y {{field name="}}|," default=b | upper}}"#);
        assert_eq!(
            texts(&template),
            vec![
                r"a {{x}} \y ".to_owned(),
                r#"<{{field name="}}|," default=b | upper}}>"#.to_owned()
            ]
        );
        let placeholder = template.placeholders().next().unwrap();
        let parsed = placeholder.parsed.as_ref().unwrap();
        assert_eq!(parsed.modifiers, vec!["upper"]);
        let Directive::Field(field) = &parsed.directive else {
            panic!("field expected");
        };
        assert_eq!(field.name, "}}|,");
    }

    #[test]
    fn malformed_shapes_are_reported() {
        let issue = |body: &str| {
            parse(body)
                .placeholders()
                .next()
                .unwrap()
                .parsed
                .as_ref()
                .unwrap_err()
                .clone()
        };
        assert_eq!(issue("x {{date"), Issue::Unclosed);
        assert_eq!(issue("{{Date}}"), Issue::Malformed);
        assert_eq!(
            issue("{{date colour=red}}"),
            Issue::UnknownParameter {
                key: "colour".into()
            }
        );
        assert_eq!(
            issue("{{uuid a=1 a=2}}"),
            Issue::DuplicateParameter { key: "a".into() }
        );
        assert_eq!(issue("{{cursor | upper}}"), Issue::Malformed);
        assert_eq!(
            issue("{{random min=5 max=1}}"),
            Issue::InvalidValue { key: "max".into() }
        );
        assert_eq!(
            issue("{{clipboard index=51}}"),
            Issue::InvalidValue {
                key: "index".into()
            }
        );
    }
}
