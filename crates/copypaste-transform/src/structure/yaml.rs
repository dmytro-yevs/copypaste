//! YAML to JSON and JSON to YAML.
//!
//! YAML is read with saphyr: pure Rust, no tags that construct anything,
//! and alias expansion capped by its node budget. A stream must hold exactly
//! one document. Plain numbers keep their written digits where JSON allows
//! them. JSON becomes block-style YAML, quoting any string that a YAML 1.1
//! or 1.2 reader could take for something else.

use std::fmt::Write;

use saphyr::{LoadableYamlNode, MarkedYaml, Scalar, ScanError, YamlData};

use super::json::reader::{strip_bom, Container, Event, Reader, Sink, MAX_DEPTH};
use super::json::string::{quoted, unescape};
use super::json::writer::Writer;
use super::{indent_unit, INDENT};
use crate::catalogue::{Category, Context, Flags, Output, TransformDef};
use crate::error::{check_output_len, Location, TransformError, Warning};
use crate::params::{ParamSpec, Params};

static TO_JSON_PARAMS: [ParamSpec; 1] = [INDENT];

pub(super) static TO_JSON: TransformDef = TransformDef {
    id: "structure.yaml_to_json",
    version: 1,
    category: Category::Structure,
    params: &TO_JSON_PARAMS,
    flags: Flags::PLAIN,
    modifier: None,
    example: (
        "name: demo\nport: 8080\ntags:\n  - a\n  - b",
        "{\n  \"name\": \"demo\",\n  \"port\": 8080,\n  \"tags\": [\n    \"a\",\n    \"b\"\n  ]\n}",
    ),
    run: run_to_json,
};

pub(super) static FROM_JSON: TransformDef = TransformDef {
    id: "structure.json_to_yaml",
    version: 1,
    category: Category::Structure,
    params: &[],
    flags: Flags::PLAIN,
    modifier: None,
    example: (
        "{\"name\":\"demo\",\"on\":\"yes\",\"tags\":[\"a\",\"b\"]}",
        "name: demo\n\"on\": \"yes\"\ntags:\n  - a\n  - b",
    ),
    run: run_from_json,
};

/// Characters between checkpoints when mapping saphyr's character indices
/// back to byte offsets.
const STRIDE: usize = 256;

/// Maps character indices to byte offsets in O(1) for ASCII text and in
/// at most [`STRIDE`] steps otherwise.
struct CharOffsets<'a> {
    text: &'a str,
    checkpoints: Option<Vec<usize>>,
}

impl<'a> CharOffsets<'a> {
    fn new(text: &'a str) -> Self {
        let checkpoints = (!text.is_ascii()).then(|| {
            text.char_indices()
                .step_by(STRIDE)
                .map(|(i, _)| i)
                .collect()
        });
        Self { text, checkpoints }
    }

    fn byte(&self, index: usize) -> usize {
        let len = self.text.len();
        match &self.checkpoints {
            None => index.min(len),
            Some(checkpoints) => {
                let Some(&base) = checkpoints.get(index / STRIDE) else {
                    return len;
                };
                self.text[base..]
                    .char_indices()
                    .nth(index % STRIDE)
                    .map_or(len, |(i, _)| base + i)
            }
        }
    }

    fn error(&self, code: &'static str, index: usize) -> TransformError {
        TransformError::invalid_input(code, self.text, self.byte(index))
    }
}

fn scan_error(offsets: &CharOffsets, error: &ScanError) -> TransformError {
    let info = error.info();
    let code = if info.contains("duplicated key") {
        "yaml.duplicate_key"
    } else if info.contains("alias expansion") {
        "yaml.alias_limit"
    } else {
        "yaml.syntax"
    };
    offsets.error(code, error.marker().index())
}

fn run_to_json(input: &str, params: &Params, ctx: &Context) -> Result<Output, TransformError> {
    let text = strip_bom(input);
    let offsets = CharOffsets::new(text);
    let documents =
        MarkedYaml::load_from_str(text).map_err(|error| scan_error(&offsets, &error))?;
    ctx.check()?;
    let document = match documents.as_slice() {
        [] => return Err(TransformError::invalid_input("yaml.empty", text, 0)),
        [document] => document,
        [_, second, ..] => {
            return Err(offsets.error("yaml.multiple_documents", second.span.start.index()));
        }
    };
    let mut converter = Converter {
        offsets: &offsets,
        ctx,
        count: 0,
        warnings: Vec::new(),
    };
    let mut writer = Writer::new(Some(indent_unit(params)), text.len());
    if matches!(document.data, YamlData::BadValue) {
        writer.event(Event::Value("null"))?;
    } else {
        converter.node(document, 0, &mut writer)?;
    }
    Ok(Output {
        text: writer.finish(),
        warnings: converter.warnings,
        sensitive: false,
    })
}

struct Converter<'o, 'c> {
    offsets: &'o CharOffsets<'o>,
    ctx: &'c Context<'c>,
    count: usize,
    warnings: Vec<Warning>,
}

impl Converter<'_, '_> {
    fn node(
        &mut self,
        node: &MarkedYaml<'_>,
        depth: usize,
        sink: &mut dyn Sink,
    ) -> Result<(), TransformError> {
        self.ctx.tick(self.count)?;
        self.count += 1;
        let start = node.span.start.index();
        if depth > MAX_DEPTH {
            return Err(self.offsets.error("yaml.too_deep", start));
        }
        match &node.data {
            YamlData::Value(scalar) => {
                let token = self.scalar(node, scalar)?;
                sink.event(Event::Value(&token))?;
            }
            YamlData::Representation(text, ..) => {
                sink.event(Event::Value(&quoted(text)?))?;
            }
            YamlData::Sequence(items) => {
                sink.event(Event::Open(Container::Array))?;
                for item in items {
                    self.node(item, depth + 1, sink)?;
                }
                sink.event(Event::Close(Container::Array))?;
            }
            YamlData::Mapping(members) => {
                sink.event(Event::Open(Container::Object))?;
                for (key, value) in members {
                    let name = self.key(key)?;
                    sink.event(Event::Key(&quoted(&name)?))?;
                    self.node(value, depth + 1, sink)?;
                }
                sink.event(Event::Close(Container::Object))?;
            }
            YamlData::Tagged(_, inner) => {
                self.tag_ignored(start);
                self.node(inner, depth + 1, sink)?;
            }
            YamlData::Alias(_) | YamlData::BadValue => {
                return Err(self.offsets.error("yaml.invalid_value", start));
            }
        }
        Ok(())
    }

    fn tag_ignored(&mut self, index: usize) {
        if self.warnings.len() < super::json::reader::MAX_WARNINGS {
            let text = self.offsets.text;
            let location = Location::of(text, self.offsets.byte(index));
            self.warnings
                .push(Warning::at("yaml.tag_ignored", location));
        }
    }

    /// The JSON token for a resolved scalar.
    fn scalar(&self, node: &MarkedYaml<'_>, scalar: &Scalar<'_>) -> Result<String, TransformError> {
        match scalar {
            Scalar::Null => Ok("null".to_owned()),
            Scalar::Boolean(b) => Ok(b.to_string()),
            Scalar::String(s) => quoted(s),
            Scalar::Integer(_) | Scalar::FloatingPoint(_) => self.number(node, scalar),
        }
    }

    /// The written digits of a number when they are a plain YAML number of
    /// the same value, else the value's own rendering.
    fn number(&self, node: &MarkedYaml<'_>, scalar: &Scalar<'_>) -> Result<String, TransformError> {
        let start = self.offsets.byte(node.span.start.index());
        let end = self.offsets.byte(node.span.end.index());
        let written = self
            .offsets
            .text
            .get(start..end)
            .and_then(|raw| json_number(raw.trim()));
        match *scalar {
            Scalar::Integer(i) => Ok(written
                .filter(|w| w.parse::<i64>() == Ok(i))
                .unwrap_or_else(|| i.to_string())),
            Scalar::FloatingPoint(f) if f.is_finite() => Ok(written
                .filter(|w| w.parse::<f64>() == Ok(*f))
                .unwrap_or_else(|| format!("{:?}", *f))),
            _ => Err(self
                .offsets
                .error("yaml.non_finite_number", node.span.start.index())),
        }
    }

    /// A mapping key as JSON object member name.
    fn key(&mut self, key: &MarkedYaml<'_>) -> Result<String, TransformError> {
        match &key.data {
            YamlData::Value(Scalar::String(s)) => Ok(s.to_string()),
            YamlData::Value(Scalar::Null) => Ok("null".to_owned()),
            YamlData::Value(Scalar::Boolean(b)) => Ok(b.to_string()),
            YamlData::Value(scalar) => self.number(key, scalar),
            YamlData::Representation(text, ..) => Ok(text.to_string()),
            YamlData::Tagged(_, inner) => {
                self.tag_ignored(key.span.start.index());
                self.key(inner)
            }
            _ => Err(self
                .offsets
                .error("yaml.complex_key", key.span.start.index())),
        }
    }
}

/// `raw` as a JSON number if it is a decimal YAML 1.2 core-schema number:
/// a `+` sign and leading zeros go, and a bare `.` gains a digit.
fn json_number(raw: &str) -> Option<String> {
    let (negative, body) = match raw.as_bytes().first()? {
        b'-' => (true, &raw[1..]),
        b'+' => (false, &raw[1..]),
        _ => (false, raw),
    };
    let (mantissa, exponent) = match body.find(['e', 'E']) {
        Some(i) => (&body[..i], Some(&body[i..])),
        None => (body, None),
    };
    let (int, fraction) = match mantissa.find('.') {
        Some(i) => (&mantissa[..i], Some(&mantissa[i + 1..])),
        None => (mantissa, None),
    };
    let digits = |s: &str| s.bytes().all(|b| b.is_ascii_digit());
    if !digits(int) || !fraction.is_none_or(digits) {
        return None;
    }
    if int.is_empty() && fraction.is_none_or(str::is_empty) {
        return None;
    }
    if let Some(exponent) = exponent {
        let power = exponent[1..]
            .strip_prefix(['+', '-'])
            .unwrap_or(&exponent[1..]);
        if power.is_empty() || !digits(power) {
            return None;
        }
    }
    let mut out = String::with_capacity(raw.len() + 2);
    if negative {
        out.push('-');
    }
    let int = int.trim_start_matches('0');
    out.push_str(if int.is_empty() { "0" } else { int });
    if let Some(fraction) = fraction {
        out.push('.');
        out.push_str(if fraction.is_empty() { "0" } else { fraction });
    }
    out.push_str(exponent.unwrap_or_default());
    Some(out)
}

fn run_from_json(input: &str, _: &Params, ctx: &Context) -> Result<Output, TransformError> {
    let text = strip_bom(input);
    let mut reader = Reader::new(text, false);
    let mut emitter = Emitter {
        out: String::with_capacity(text.len()),
        frames: Vec::new(),
        documents: 0,
    };
    reader.drive(&mut emitter, ctx)?;
    Ok(Output {
        text: emitter.out,
        warnings: reader.into_warnings(),
        sensitive: false,
    })
}

struct Frame {
    container: Container,
    /// Column of this collection's entries.
    indent: usize,
    /// The first entry continues the current line (after `- `).
    inline_first: bool,
    /// Written before `{}` or `[]` when the collection is empty.
    empty_prefix: &'static str,
    count: usize,
}

/// Writes reader events as block-style YAML, one document per value.
struct Emitter {
    out: String,
    frames: Vec<Frame>,
    documents: usize,
}

impl Emitter {
    fn entry(&mut self) {
        let Some(frame) = self.frames.last_mut() else {
            return;
        };
        if frame.count > 0 || !frame.inline_first {
            self.out.push('\n');
            let indent = frame.indent;
            self.out.extend(std::iter::repeat_n(' ', indent));
        }
        frame.count += 1;
    }

    /// Starts a value in its parent's position; returns the column, line
    /// placement and empty prefix a collection there would use.
    fn begin_value(&mut self) -> (usize, bool, &'static str) {
        match self.frames.last().map(|f| (f.container, f.indent)) {
            None => {
                if self.documents > 0 {
                    self.out.push_str("\n---\n");
                }
                (0, true, "")
            }
            Some((Container::Object, indent)) => (indent + 2, false, " "),
            Some((Container::Array, indent)) => {
                self.entry();
                self.out.push_str("- ");
                (indent + 2, true, "")
            }
        }
    }

    fn end_value(&mut self) {
        if self.frames.is_empty() {
            self.documents += 1;
        }
    }
}

impl Sink for Emitter {
    fn event(&mut self, event: Event<'_>) -> Result<(), TransformError> {
        match event {
            Event::Open(container) => {
                let (indent, inline_first, empty_prefix) = self.begin_value();
                self.frames.push(Frame {
                    container,
                    indent,
                    inline_first,
                    empty_prefix,
                    count: 0,
                });
            }
            Event::Close(container) => {
                if let Some(frame) = self.frames.pop().filter(|f| f.count == 0) {
                    self.out.push_str(frame.empty_prefix);
                    self.out.push_str(match container {
                        Container::Object => "{}",
                        Container::Array => "[]",
                    });
                }
                self.end_value();
            }
            Event::Key(raw) => {
                self.entry();
                push_scalar(&mut self.out, raw)?;
                self.out.push(':');
            }
            Event::Value(raw) => {
                let in_object = self
                    .frames
                    .last()
                    .is_some_and(|f| f.container == Container::Object);
                if in_object {
                    self.out.push(' ');
                } else {
                    self.begin_value();
                }
                push_scalar(&mut self.out, raw)?;
                self.end_value();
            }
        }
        check_output_len(self.out.len())
    }
}

/// Appends a JSON token as a YAML scalar. Numbers and literals mean the
/// same in YAML; strings go plain only when that cannot change their type.
fn push_scalar(out: &mut String, raw: &str) -> Result<(), TransformError> {
    if !raw.starts_with('"') {
        out.push_str(raw);
        return Ok(());
    }
    let text = unescape(&raw[1..raw.len() - 1]).map_err(|_| TransformError::InvalidInput {
        code: "json.invalid_escape",
        location: None,
    })?;
    if is_plain(&text) {
        out.push_str(&text);
    } else {
        push_double_quoted(out, &text);
    }
    Ok(())
}

/// Characters a double-quoted YAML scalar must escape (outside the named
/// escapes) because YAML does not allow them printed.
fn non_printable(c: char) -> bool {
    c < ' '
        || ('\u{7f}'..='\u{9f}').contains(&c)
        || matches!(c, '\u{feff}' | '\u{fffe}' | '\u{ffff}')
}

fn push_double_quoted(out: &mut String, text: &str) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if non_printable(c) || matches!(c, '\u{2028}' | '\u{2029}') => {
                let _ = write!(out, "\\u{:04x}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// Whether `text` can be written as a plain scalar and read back as the
/// same string by YAML 1.1 and 1.2 readers.
fn is_plain(text: &str) -> bool {
    let Some(first) = text.chars().next() else {
        return false;
    };
    if first.is_whitespace() || text.ends_with(char::is_whitespace) {
        return false;
    }
    if "-?:,[]{}#&*!|>'\"%@`".contains(first) || text.starts_with("...") {
        return false;
    }
    if text.ends_with(':') || text.contains(": ") || text.contains(" #") {
        return false;
    }
    if text
        .chars()
        .any(|c| c.is_control() || non_printable(c) || matches!(c, '\u{2028}' | '\u{2029}'))
    {
        return false;
    }
    !resolves_as_non_string(text)
}

/// Whether a plain `text` could read back as null, a boolean, a number, a
/// date or a merge key under the YAML 1.1 or 1.2 core schema.
fn resolves_as_non_string(text: &str) -> bool {
    const WORDS: [&str; 11] = [
        "null", "~", "true", "false", "yes", "no", "on", "off", "y", "n", "<<",
    ];
    if WORDS.iter().any(|w| w.eq_ignore_ascii_case(text)) {
        return true;
    }
    let body = text.strip_prefix(['+', '-']).unwrap_or(text);
    if [".inf", ".nan"]
        .iter()
        .any(|w| w.eq_ignore_ascii_case(body))
    {
        return true;
    }
    let numeric_start = body.starts_with(|c: char| c.is_ascii_digit())
        || (body.starts_with('.') && body[1..].starts_with(|c: char| c.is_ascii_digit()));
    numeric_start
        && body
            .bytes()
            .all(|b| b.is_ascii_hexdigit() || b"xXoObB_.:+-eE".contains(&b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yaml_numbers_become_json_numbers() {
        assert_eq!(json_number("+007").as_deref(), Some("7"));
        assert_eq!(json_number(".5").as_deref(), Some("0.5"));
        assert_eq!(json_number("-1.e+3").as_deref(), Some("-1.0e+3"));
        assert_eq!(json_number("1.10").as_deref(), Some("1.10"));
        assert_eq!(json_number("0x1f"), None);
        assert_eq!(json_number("."), None);
    }

    #[test]
    fn ambiguous_strings_are_quoted() {
        for text in [
            "yes",
            "No",
            "1.5",
            "2024-01-01",
            "12:30",
            "",
            " a",
            "a: b",
            "-x",
            ".inf",
        ] {
            assert!(!is_plain(text), "{text:?}");
        }
        for text in ["demo", "Київ", "a-b", "https://x.test/a#b", "1st"] {
            assert!(is_plain(text), "{text:?}");
        }
    }

    #[test]
    fn char_offsets_map_to_bytes() {
        let text = format!("{}é{}x", "a".repeat(300), "b".repeat(300));
        let offsets = CharOffsets::new(&text);
        assert_eq!(offsets.byte(300), 300);
        assert_eq!(offsets.byte(301), 302);
        assert_eq!(offsets.byte(601), 602);
        assert_eq!(offsets.byte(10_000), text.len());
    }
}
