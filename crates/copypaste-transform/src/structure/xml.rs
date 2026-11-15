//! XML format and minify.
//!
//! quick-xml never resolves entities or fetches anything, and documents that
//! declare an external entity are refused outright. A first pass checks
//! well-formedness and records, per element, whether it holds text, markup
//! or `xml:space="preserve"`. The second pass writes the layout: only
//! whitespace between elements is touched; text, mixed content and
//! preserved elements are copied exactly as written.

use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

use super::json::reader::strip_bom;
use super::{indent_unit, INDENT};
use crate::catalogue::{Category, Context, Flags, Output, TransformDef};
use crate::error::{check_output_len, TransformError};
use crate::params::{ParamSpec, Params};

static FORMAT_PARAMS: [ParamSpec; 1] = [INDENT];

pub(super) static FORMAT: TransformDef = TransformDef {
    id: "structure.xml_format",
    version: 1,
    category: Category::Structure,
    params: &FORMAT_PARAMS,
    flags: Flags::PLAIN,
    modifier: None,
    example: (
        "<list><item id='1'>One</item><item/></list>",
        "<list>\n  <item id=\"1\">One</item>\n  <item/>\n</list>",
    ),
    run: run_format,
};

pub(super) static MINIFY: TransformDef = TransformDef {
    id: "structure.xml_minify",
    version: 1,
    category: Category::Structure,
    params: &[],
    flags: Flags::PLAIN,
    modifier: None,
    example: (
        "<list>\n  <item>One</item>\n</list>",
        "<list><item>One</item></list>",
    ),
    run: run_minify,
};

const HAS_TEXT: u8 = 1;
const HAS_MARKUP: u8 = 2;
const PRESERVE: u8 = 4;

fn run_format(input: &str, params: &Params, ctx: &Context) -> Result<Output, TransformError> {
    layout(strip_bom(input), Some(indent_unit(params)), ctx).map(Output::new)
}

fn run_minify(input: &str, _: &Params, ctx: &Context) -> Result<Output, TransformError> {
    layout(strip_bom(input), None, ctx).map(Output::new)
}

fn reader(text: &str) -> Reader<&[u8]> {
    let mut reader = Reader::from_str(text);
    reader.config_mut().check_comments = true;
    reader
}

fn read_error(text: &str, reader: &Reader<&[u8]>, error: &quick_xml::Error) -> TransformError {
    use quick_xml::errors::IllFormedError;
    let code = match error {
        quick_xml::Error::IllFormed(
            IllFormedError::MismatchedEndTag { .. } | IllFormedError::UnmatchedEndTag(_),
        ) => "xml.mismatched_end_tag",
        quick_xml::Error::InvalidAttr(_) => "xml.invalid_attribute",
        _ => "xml.syntax",
    };
    TransformError::invalid_input(code, text, reader.error_position() as usize)
}

fn is_space(text: &str) -> bool {
    text.bytes()
        .all(|b| matches!(b, b' ' | b'\t' | b'\r' | b'\n'))
}

/// Whether a document type declaration declares an entity whose value lives
/// outside the document (`SYSTEM` or `PUBLIC`).
fn declares_external_entity(doctype: &str) -> bool {
    doctype.match_indices("<!ENTITY").any(|(i, _)| {
        let mut words = doctype[i + "<!ENTITY".len()..].split_ascii_whitespace();
        let name = words.next();
        let keyword = if name == Some("%") {
            words.nth(1)
        } else {
            words.next()
        };
        keyword.is_some_and(|k| k.starts_with("SYSTEM") || k.starts_with("PUBLIC"))
    })
}

fn preserves_space(tag: &BytesStart<'_>) -> bool {
    tag.attributes()
        .flatten()
        .any(|a| a.key.as_ref() == b"xml:space" && a.value.as_ref() == b"preserve")
}

/// Checks well-formedness and returns the content flags of every element
/// that has an end tag, in document order.
fn scan(text: &str, ctx: &Context) -> Result<Vec<u8>, TransformError> {
    let invalid = |code, at| TransformError::invalid_input(code, text, at);
    let mut reader = reader(text);
    let mut flags: Vec<u8> = Vec::new();
    let mut open: Vec<(usize, usize)> = Vec::new();
    let mut roots = 0;
    for count in 0.. {
        ctx.tick(count)?;
        let start = reader.buffer_position() as usize;
        let event = reader
            .read_event()
            .map_err(|e| read_error(text, &reader, &e))?;
        let end = reader.buffer_position() as usize;
        let content = match &event {
            Event::Eof => break,
            Event::Start(tag) | Event::Empty(tag) => {
                if tag.attributes().any(|a| a.is_err()) {
                    return Err(invalid("xml.invalid_attribute", start));
                }
                if open.is_empty() {
                    roots += 1;
                    if roots > 1 {
                        return Err(invalid("xml.multiple_roots", start));
                    }
                }
                HAS_MARKUP
            }
            Event::Text(_) if is_space(&text[start..end]) => 0,
            Event::Text(_) | Event::CData(_) | Event::GeneralRef(_) => {
                if open.is_empty() {
                    return Err(invalid("xml.text_outside_root", start));
                }
                HAS_TEXT
            }
            Event::DocType(_) => {
                if roots > 0 {
                    return Err(invalid("xml.syntax", start));
                }
                if declares_external_entity(&text[start..end]) {
                    return Err(invalid("xml.external_entity", start));
                }
                0
            }
            Event::Comment(_) | Event::PI(_) => HAS_MARKUP,
            Event::Decl(_) | Event::End(_) => 0,
        };
        if let Some(&(index, _)) = open.last() {
            flags[index] |= content;
        }
        match event {
            Event::Start(tag) => {
                flags.push(if preserves_space(&tag) { PRESERVE } else { 0 });
                open.push((flags.len() - 1, start));
            }
            Event::End(_) => {
                open.pop();
            }
            _ => {}
        }
    }
    if let Some(&(_, at)) = open.last() {
        return Err(invalid("xml.unclosed_element", at));
    }
    if roots == 0 {
        return Err(if is_space(text) {
            invalid("xml.empty", 0)
        } else {
            invalid("xml.no_root", text.len())
        });
    }
    Ok(flags)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// Child elements on their own lines; whitespace between them dropped.
    Block,
    /// Text only, kept as written; whitespace-only content dropped.
    Inline { keep_text: bool },
    /// Mixed or preserved content, copied as written. `root` marks the
    /// element whose own tags are still laid out.
    Verbatim { root: bool },
}

struct Layout<'a> {
    text: &'a str,
    out: String,
    unit: Option<&'static str>,
    depth: usize,
}

impl Layout<'_> {
    fn line(&mut self) {
        if let Some(unit) = self.unit {
            if !self.out.is_empty() {
                self.out.push('\n');
            }
            for _ in 0..self.depth {
                self.out.push_str(unit);
            }
        }
    }

    fn utf8<'b>(&self, bytes: &'b [u8]) -> &'b str {
        std::str::from_utf8(bytes).unwrap_or_default()
    }

    /// The tag with single spaces between attributes and every value in
    /// double quotes unless it contains one.
    fn tag(&mut self, tag: &BytesStart<'_>, empty: bool) {
        self.out.push('<');
        self.out.push_str(self.utf8(tag.name().as_ref()));
        for attribute in tag.attributes().flatten() {
            let value = self.utf8(&attribute.value);
            let quote = if value.contains('"') { '\'' } else { '"' };
            self.out.push(' ');
            self.out.push_str(self.utf8(attribute.key.as_ref()));
            self.out.push('=');
            self.out.push(quote);
            self.out.push_str(value);
            self.out.push(quote);
        }
        self.out.push_str(if empty { "/>" } else { ">" });
    }

    fn end_tag(&mut self, name: &[u8]) {
        self.out.push_str("</");
        self.out.push_str(self.utf8(name));
        self.out.push('>');
    }
}

fn layout(text: &str, unit: Option<&'static str>, ctx: &Context) -> Result<String, TransformError> {
    let flags = scan(text, ctx)?;
    let mut reader = reader(text);
    let mut layout = Layout {
        text,
        out: String::with_capacity(text.len()),
        unit,
        depth: 0,
    };
    let mut modes: Vec<Mode> = Vec::new();
    let mut elements = flags.iter();
    for count in 0.. {
        ctx.tick(count)?;
        let start = reader.buffer_position() as usize;
        let event = reader
            .read_event()
            .map_err(|e| read_error(text, &reader, &e))?;
        let raw = &layout.text[start..reader.buffer_position() as usize];
        let mode = modes.last().copied().unwrap_or(Mode::Block);
        match event {
            Event::Eof => break,
            Event::Start(tag) => {
                let flag = elements.next().copied().unwrap_or_default();
                if let Mode::Verbatim { .. } | Mode::Inline { .. } = mode {
                    layout.out.push_str(raw);
                    modes.push(Mode::Verbatim { root: false });
                } else {
                    layout.line();
                    layout.tag(&tag, false);
                    let mixed = flag & HAS_TEXT != 0 && flag & HAS_MARKUP != 0;
                    modes.push(if flag & PRESERVE != 0 || mixed {
                        Mode::Verbatim { root: true }
                    } else if flag & HAS_MARKUP != 0 {
                        layout.depth += 1;
                        Mode::Block
                    } else {
                        Mode::Inline {
                            keep_text: flag & HAS_TEXT != 0,
                        }
                    });
                }
            }
            Event::End(tag) => {
                modes.pop();
                match mode {
                    Mode::Verbatim { root: false } => layout.out.push_str(raw),
                    Mode::Block => {
                        layout.depth -= 1;
                        layout.line();
                        layout.end_tag(tag.name().as_ref());
                    }
                    _ => layout.end_tag(tag.name().as_ref()),
                }
            }
            Event::Empty(tag) => {
                if mode == Mode::Block {
                    layout.line();
                    layout.tag(&tag, true);
                } else {
                    layout.out.push_str(raw);
                }
            }
            Event::Text(_) | Event::CData(_) | Event::GeneralRef(_) => match mode {
                Mode::Block | Mode::Inline { keep_text: false } => {}
                _ => layout.out.push_str(raw),
            },
            Event::Comment(_) | Event::PI(_) | Event::Decl(_) | Event::DocType(_) => {
                if mode == Mode::Block {
                    layout.line();
                }
                layout.out.push_str(raw);
            }
        }
        check_output_len(layout.out.len())?;
    }
    Ok(layout.out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn external_entity_declarations_are_found() {
        assert!(declares_external_entity(
            "<!DOCTYPE x [<!ENTITY e SYSTEM \"file:///etc/passwd\">]>"
        ));
        assert!(declares_external_entity(
            "<!DOCTYPE x [<!ENTITY % p PUBLIC \"-//x\" \"http://x.test\">]>"
        ));
        assert!(!declares_external_entity(
            "<!DOCTYPE x [<!ENTITY e \"SYSTEM\">]>"
        ));
        assert!(!declares_external_entity(
            "<!DOCTYPE html PUBLIC \"-//W3C\" \"x.dtd\">"
        ));
    }
}
