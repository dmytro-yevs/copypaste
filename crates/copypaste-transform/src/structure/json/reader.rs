//! A strict RFC 8259 reader that yields events over the original text.
//!
//! Numbers, strings and keys are handed out as the exact source slices, so
//! nothing is ever round-tripped through a float or re-escaped. Several
//! top-level values on separate lines are read as JSON Lines. Lenient mode
//! also accepts comments and trailing commas, each with a warning.

use std::borrow::Cow;
use std::collections::HashSet;

use super::string::decode_key;
use crate::catalogue::Context;
use crate::error::{Location, TransformError, Warning};

/// Deepest nesting of objects and arrays a document may have.
pub(crate) const MAX_DEPTH: usize = 512;

/// Most warnings kept for one run; later ones are dropped.
pub(crate) const MAX_WARNINGS: usize = 100;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Container {
    Object,
    Array,
}

/// One step through a document. `Key` and `Value` carry raw JSON tokens:
/// a quoted string, a number or a literal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Event<'a> {
    Open(Container),
    Close(Container),
    Key(&'a str),
    Value(&'a str),
}

/// Receives the events of one or more documents in order.
pub(crate) trait Sink {
    fn event(&mut self, event: Event<'_>) -> Result<(), TransformError>;
}

/// Accepts every event; used to validate without producing output.
pub(crate) struct Discard;

impl Sink for Discard {
    fn event(&mut self, _: Event<'_>) -> Result<(), TransformError> {
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    Document,
    ValueOrClose,
    KeyOrClose,
    Key,
    ArrayValue,
    Colon,
    MemberValue,
    AfterValue,
}

struct Frame<'a> {
    container: Container,
    keys: HashSet<Cow<'a, str>>,
}

pub(crate) struct Reader<'a> {
    text: &'a str,
    bytes: &'a [u8],
    pos: usize,
    lenient: bool,
    state: State,
    stack: Vec<Frame<'a>>,
    documents: usize,
    warnings: Vec<Warning>,
}

impl<'a> Reader<'a> {
    /// A reader over `text` with any leading byte order mark already removed.
    pub(crate) fn new(text: &'a str, lenient: bool) -> Self {
        Self {
            text,
            bytes: text.as_bytes(),
            pos: 0,
            lenient,
            state: State::Document,
            stack: Vec::new(),
            documents: 0,
            warnings: Vec::new(),
        }
    }

    pub(crate) fn into_warnings(self) -> Vec<Warning> {
        self.warnings
    }

    /// Feeds every event to `sink`, checking `ctx` as it goes.
    pub(crate) fn drive(
        &mut self,
        sink: &mut dyn Sink,
        ctx: &Context,
    ) -> Result<(), TransformError> {
        let mut count = 0;
        while let Some(event) = self.next_event()? {
            ctx.tick(count)?;
            count += 1;
            sink.event(event)?;
        }
        Ok(())
    }

    fn error(&self, code: &'static str, offset: usize) -> TransformError {
        TransformError::invalid_input(code, self.text, offset)
    }

    fn warn(&mut self, code: &'static str, offset: usize) {
        if self.warnings.len() < MAX_WARNINGS {
            self.warnings
                .push(Warning::at(code, Location::of(self.text, offset)));
        }
    }

    fn unexpected(&self) -> TransformError {
        if self.pos >= self.bytes.len() {
            self.error("json.unexpected_end", self.bytes.len())
        } else {
            self.error("json.unexpected_character", self.pos)
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    /// Skips whitespace (and comments when lenient); reports whether a line
    /// break was crossed.
    fn skip_space(&mut self) -> Result<bool, TransformError> {
        let mut newline = false;
        while let Some(byte) = self.peek() {
            match byte {
                b' ' | b'\t' | b'\r' => self.pos += 1,
                b'\n' => {
                    newline = true;
                    self.pos += 1;
                }
                b'/' if self.lenient => newline |= self.skip_comment()?,
                _ => break,
            }
        }
        Ok(newline)
    }

    fn skip_comment(&mut self) -> Result<bool, TransformError> {
        let start = self.pos;
        let rest = &self.text[start..];
        if rest.starts_with("//") {
            self.pos = rest.find('\n').map_or(self.bytes.len(), |i| start + i);
            self.warn("json.comment", start);
            Ok(false)
        } else if let Some(body) = rest.strip_prefix("/*") {
            let end = body
                .find("*/")
                .ok_or_else(|| self.error("json.unexpected_end", self.bytes.len()))?;
            self.pos = start + 2 + end + 2;
            self.warn("json.comment", start);
            Ok(body[..end].contains('\n'))
        } else {
            Err(self.error("json.unexpected_character", start))
        }
    }

    pub(crate) fn next_event(&mut self) -> Result<Option<Event<'a>>, TransformError> {
        loop {
            match self.state {
                State::Document => {
                    let newline = self.skip_space()?;
                    if self.pos >= self.bytes.len() {
                        return if self.documents == 0 {
                            Err(self.error("json.empty", 0))
                        } else {
                            Ok(None)
                        };
                    }
                    if self.documents > 0 && !newline {
                        return Err(self.error("json.unexpected_character", self.pos));
                    }
                    return self.value().map(Some);
                }
                State::ValueOrClose | State::KeyOrClose => {
                    self.skip_space()?;
                    let close = if self.state == State::ValueOrClose {
                        b']'
                    } else {
                        b'}'
                    };
                    if self.peek() == Some(close) {
                        return Ok(Some(self.close()));
                    }
                    return if self.state == State::ValueOrClose {
                        self.value().map(Some)
                    } else {
                        self.key().map(Some)
                    };
                }
                State::Key | State::ArrayValue => {
                    self.skip_space()?;
                    let close = if self.state == State::ArrayValue {
                        b']'
                    } else {
                        b'}'
                    };
                    if self.peek() == Some(close) {
                        if !self.lenient {
                            return Err(self.error("json.trailing_comma", self.pos));
                        }
                        self.warn("json.trailing_comma", self.pos);
                        return Ok(Some(self.close()));
                    }
                    return if self.state == State::ArrayValue {
                        self.value().map(Some)
                    } else {
                        self.key().map(Some)
                    };
                }
                State::Colon => {
                    self.skip_space()?;
                    if self.peek() != Some(b':') {
                        return Err(self.unexpected());
                    }
                    self.pos += 1;
                    self.state = State::MemberValue;
                }
                State::MemberValue => {
                    self.skip_space()?;
                    return self.value().map(Some);
                }
                State::AfterValue => {
                    self.skip_space()?;
                    let container = self.stack.last().map(|f| f.container);
                    match (self.peek(), container) {
                        (Some(b','), Some(Container::Object)) => {
                            self.pos += 1;
                            self.state = State::Key;
                        }
                        (Some(b','), Some(Container::Array)) => {
                            self.pos += 1;
                            self.state = State::ArrayValue;
                        }
                        (Some(b'}'), Some(Container::Object))
                        | (Some(b']'), Some(Container::Array)) => return Ok(Some(self.close())),
                        _ => return Err(self.unexpected()),
                    }
                }
            }
        }
    }

    fn close(&mut self) -> Event<'a> {
        self.pos += 1;
        let container = self.stack.pop().map_or(Container::Array, |f| f.container);
        self.after_value();
        Event::Close(container)
    }

    fn after_value(&mut self) {
        if self.stack.is_empty() {
            self.documents += 1;
            self.state = State::Document;
        } else {
            self.state = State::AfterValue;
        }
    }

    fn open(&mut self, container: Container) -> Result<Event<'a>, TransformError> {
        if self.stack.len() >= MAX_DEPTH {
            return Err(self.error("json.too_deep", self.pos));
        }
        self.pos += 1;
        self.stack.push(Frame {
            container,
            keys: HashSet::new(),
        });
        self.state = match container {
            Container::Object => State::KeyOrClose,
            Container::Array => State::ValueOrClose,
        };
        Ok(Event::Open(container))
    }

    fn value(&mut self) -> Result<Event<'a>, TransformError> {
        let token = match self.peek() {
            Some(b'{') => return self.open(Container::Object),
            Some(b'[') => return self.open(Container::Array),
            Some(b'"') => self.string()?,
            Some(b'-' | b'0'..=b'9') => self.number()?,
            Some(b't' | b'f' | b'n') => self.literal()?,
            _ => return Err(self.unexpected()),
        };
        self.after_value();
        Ok(Event::Value(token))
    }

    fn key(&mut self) -> Result<Event<'a>, TransformError> {
        if self.peek() != Some(b'"') {
            return Err(self.unexpected());
        }
        let start = self.pos;
        let raw = self.string()?;
        let name = decode_key(raw);
        if let Some(frame) = self.stack.last_mut() {
            if !frame.keys.insert(name) {
                self.warn("json.duplicate_key", start);
            }
        }
        self.state = State::Colon;
        Ok(Event::Key(raw))
    }

    /// A string token starting at the opening quote, escapes checked.
    fn string(&mut self) -> Result<&'a str, TransformError> {
        let start = self.pos;
        let mut i = start + 1;
        loop {
            match self.bytes.get(i) {
                None => return Err(self.error("json.unexpected_end", self.bytes.len())),
                Some(b'"') => break,
                Some(b'\\') => {
                    i += match self.bytes.get(i + 1) {
                        Some(b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't') => 2,
                        Some(b'u')
                            if self
                                .bytes
                                .get(i + 2..i + 6)
                                .is_some_and(|h| h.iter().all(u8::is_ascii_hexdigit)) =>
                        {
                            6
                        }
                        _ => return Err(self.error("json.invalid_escape", i)),
                    };
                }
                Some(&byte) if byte < 0x20 => {
                    return Err(self.error("json.control_character", i));
                }
                Some(_) => i += 1,
            }
        }
        self.pos = i + 1;
        Ok(&self.text[start..self.pos])
    }

    fn digits(&self, mut i: usize) -> usize {
        while self.bytes.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
        i
    }

    fn number(&mut self) -> Result<&'a str, TransformError> {
        let start = self.pos;
        let invalid = |at| Err(self.error("json.invalid_number", at));
        let mut i = start;
        if self.bytes[i] == b'-' {
            i += 1;
        }
        match self.bytes.get(i) {
            Some(b'0') => i += 1,
            Some(b'1'..=b'9') => i = self.digits(i),
            _ => return invalid(i),
        }
        if self.bytes.get(i) == Some(&b'.') {
            let end = self.digits(i + 1);
            if end == i + 1 {
                return invalid(end);
            }
            i = end;
        }
        if matches!(self.bytes.get(i), Some(b'e' | b'E')) {
            i += 1;
            if matches!(self.bytes.get(i), Some(b'+' | b'-')) {
                i += 1;
            }
            let end = self.digits(i);
            if end == i {
                return invalid(end);
            }
            i = end;
        }
        if self
            .bytes
            .get(i)
            .is_some_and(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'+' | b'-'))
        {
            return invalid(start);
        }
        self.pos = i;
        Ok(&self.text[start..i])
    }

    fn literal(&mut self) -> Result<&'a str, TransformError> {
        let rest = &self.text[self.pos..];
        let word = ["true", "false", "null"]
            .into_iter()
            .find(|w| rest.starts_with(w))
            .filter(|w| {
                !rest
                    .as_bytes()
                    .get(w.len())
                    .is_some_and(u8::is_ascii_alphanumeric)
            })
            .ok_or_else(|| self.error("json.unexpected_character", self.pos))?;
        let start = self.pos;
        self.pos += word.len();
        Ok(&self.text[start..self.pos])
    }
}

/// `text` without a leading byte order mark.
pub(crate) fn strip_bom(text: &str) -> &str {
    text.strip_prefix('\u{feff}').unwrap_or(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn events(text: &str) -> Result<Vec<Event<'_>>, TransformError> {
        let mut reader = Reader::new(text, false);
        let mut out = Vec::new();
        while let Some(event) = reader.next_event()? {
            out.push(event);
        }
        Ok(out)
    }

    #[test]
    fn numbers_and_strings_come_out_as_written() {
        let got = events(r#"{"ab":[12345678901234567890,1.10,-0e+5,"\/"]}"#).unwrap();
        assert_eq!(got[1], Event::Key(r#""ab""#));
        assert_eq!(got[3], Event::Value("12345678901234567890"));
        assert_eq!(got[4], Event::Value("1.10"));
        assert_eq!(got[5], Event::Value("-0e+5"));
        assert_eq!(got[6], Event::Value(r#""\/""#));
    }

    #[test]
    fn errors_point_at_the_offending_byte() {
        let code_at = |text| match events(text) {
            Err(TransformError::InvalidInput { code, location }) => {
                (code, location.map(|l| (l.line, l.column)))
            }
            other => panic!("expected an error, got {other:?}"),
        };
        assert_eq!(code_at("{\"a\":1,}"), ("json.trailing_comma", Some((1, 8))));
        assert_eq!(code_at("[01]"), ("json.invalid_number", Some((1, 2))));
        assert_eq!(code_at("[1.]"), ("json.invalid_number", Some((1, 4))));
        assert_eq!(code_at("\"a\\x\""), ("json.invalid_escape", Some((1, 3))));
        assert_eq!(
            code_at("[\n tru]"),
            ("json.unexpected_character", Some((2, 2)))
        );
        assert_eq!(
            code_at("{\"a\" 1}"),
            ("json.unexpected_character", Some((1, 6)))
        );
        assert_eq!(code_at("[1"), ("json.unexpected_end", Some((1, 3))));
        assert_eq!(code_at("1 2"), ("json.unexpected_character", Some((1, 3))));
        assert_eq!(code_at("  "), ("json.empty", Some((1, 1))));
    }

    #[test]
    fn nesting_is_limited() {
        let deep = "[".repeat(MAX_DEPTH + 1);
        assert_eq!(events(&deep).unwrap_err().code(), "json.too_deep");
        let fine = format!("{}{}", "[".repeat(MAX_DEPTH), "]".repeat(MAX_DEPTH));
        assert!(events(&fine).is_ok());
    }
}
