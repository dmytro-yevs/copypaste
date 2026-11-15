//! HTML to plain text and to Markdown.
//!
//! Plain text uses the same html2text configuration as the history's
//! derived plain text. Markdown walks the parsed DOM: soft line breaks and
//! runs of whitespace become one space, `pre` keeps every byte inside a
//! fence, and elements without a Markdown form contribute their text.

use html2text::render::TrivialDecorator;
use html2text::{Comment, Document, Element, Handle};

use crate::catalogue::Context;
use crate::error::{check_output_len, TransformError};

/// Below this depth content is kept as plain text instead of recursing.
const MAX_DEPTH: usize = 128;

/// Elements whose content is never visible text.
const HIDDEN: [&str; 12] = [
    "head", "script", "style", "template", "title", "meta", "link", "iframe", "object", "svg",
    "canvas", "select",
];

/// Blocks separated by a blank line.
const PARAGRAPHS: [&str; 4] = ["p", "dl", "figure", "details"];

/// Blocks that only start a new line.
const LINES: [&str; 18] = [
    "div",
    "section",
    "article",
    "header",
    "footer",
    "main",
    "nav",
    "aside",
    "figcaption",
    "dt",
    "dd",
    "summary",
    "caption",
    "address",
    "center",
    "form",
    "fieldset",
    "legend",
];

fn unreadable() -> TransformError {
    TransformError::InvalidInput {
        code: "html.unreadable",
        location: None,
    }
}

pub(super) fn to_plain(input: &str, ctx: &Context) -> Result<String, TransformError> {
    ctx.check()?;
    let rendered = html2text::config::with_decorator(TrivialDecorator::new())
        .raw_mode(true)
        .unicode_strikeout(false)
        .string_from_read(input.as_bytes(), input.len().max(1))
        .map_err(|_| unreadable())?;
    Ok(rendered.trim_end_matches('\n').to_owned())
}

pub(super) fn to_markdown(input: &str, ctx: &Context) -> Result<String, TransformError> {
    ctx.check()?;
    let dom = html2text::config::plain()
        .parse_html(input.as_bytes())
        .map_err(|_| unreadable())?;
    let mut writer = Writer::new(ctx, false);
    for child in dom.document.children.borrow().iter() {
        if matches!(child.data, Element { .. }) {
            writer.node(child, 0)?;
        }
    }
    Ok(writer.out.trim_end().to_owned())
}

/// What a DOM node is, as far as the converters care.
enum Kind<'n> {
    Element(&'n str),
    Text,
    Other,
}

fn kind(node: &Handle) -> Kind<'_> {
    match &node.data {
        Element { name, .. } => Kind::Element(&name.local),
        Comment { .. } | Document => Kind::Other,
        _ => Kind::Text,
    }
}

fn attr(node: &Handle, name: &str) -> Option<String> {
    match &node.data {
        Element { attrs, .. } => attrs
            .borrow()
            .iter()
            .find(|a| &*a.name.local == name)
            .map(|a| a.value.trim().to_owned()),
        _ => None,
    }
}

/// The characters of a text node. The DOM keeps them private, so they are
/// read back through the serializer, which escapes only `& < >` and NBSP.
fn text_of(node: &Handle) -> String {
    let mut bytes = Vec::new();
    if node.serialize(&mut bytes).is_err() {
        return String::new();
    }
    let escaped = String::from_utf8_lossy(&bytes);
    let mut out = String::with_capacity(escaped.len());
    let mut rest = &*escaped;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        let (entity, ch) = [
            ("&amp;", '&'),
            ("&lt;", '<'),
            ("&gt;", '>'),
            ("&nbsp;", '\u{a0}'),
        ]
        .into_iter()
        .find(|(entity, _)| rest.starts_with(entity))
        .unwrap_or(("&", '&'));
        out.push(ch);
        rest = &rest[entity.len()..];
    }
    out.push_str(rest);
    out
}

/// All text under `node` exactly as written, with `<br>` as a line break.
/// Iterative, so any nesting depth is fine.
fn raw_text(node: &Handle, ctx: &Context) -> Result<String, TransformError> {
    let mut out = String::new();
    let mut stack: Vec<Handle> = node.children.borrow().iter().rev().cloned().collect();
    let mut visited = 0usize;
    while let Some(next) = stack.pop() {
        visited += 1;
        ctx.tick(visited)?;
        match kind(&next) {
            Kind::Text => out.push_str(&text_of(&next)),
            Kind::Element("br") => out.push('\n'),
            Kind::Element(name) if !HIDDEN.contains(&name) => {
                stack.extend(next.children.borrow().iter().rev().cloned());
            }
            _ => {}
        }
        check_output_len(out.len())?;
    }
    Ok(out)
}

fn is_html_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r' | '\x0c')
}

fn longest_run(text: &str, of: char) -> usize {
    text.split(|c| c != of)
        .map(|run| run.len())
        .max()
        .unwrap_or(0)
}

/// A link or image destination, bracketed when it holds spaces or parens.
fn destination(url: &str) -> String {
    if url.contains(|c: char| c.is_whitespace() || matches!(c, '(' | ')' | '<' | '>')) {
        format!("<{}>", url.replace('<', "%3C").replace('>', "%3E"))
    } else {
        url.to_owned()
    }
}

struct Writer<'c, 'a> {
    ctx: &'c Context<'a>,
    out: String,
    /// Continuation prefix of each open quote or list item.
    prefix: Vec<String>,
    /// List marker that replaces the innermost prefix on the next line.
    marker: Option<String>,
    /// Line breaks owed before the next content (2 = blank line).
    pending: u8,
    /// The owed single break came from `<br>`.
    hard_break: bool,
    line_open: bool,
    space: bool,
    /// Inline rendering (link text, emphasis, cells): blocks become spaces.
    inline: bool,
    /// Inline rendering began with whitespace.
    lead: bool,
    lists: usize,
    visited: usize,
}

impl<'c, 'a> Writer<'c, 'a> {
    fn new(ctx: &'c Context<'a>, inline: bool) -> Self {
        Self {
            ctx,
            out: String::new(),
            prefix: Vec::new(),
            marker: None,
            pending: 0,
            hard_break: false,
            line_open: false,
            space: false,
            inline,
            lead: false,
            lists: 0,
            visited: 0,
        }
    }

    fn newline(&mut self, n: u8) {
        if self.inline {
            self.space = true;
            return;
        }
        self.pending = self.pending.max(n);
        if self.pending > 1 {
            self.hard_break = false;
        }
        self.space = false;
    }

    fn line_break(&mut self) {
        if self.inline {
            self.space = true;
        } else if self.pending >= 1 && self.hard_break {
            self.pending = 2;
            self.hard_break = false;
        } else {
            self.pending = self.pending.max(1);
            self.hard_break = self.pending == 1;
            self.space = false;
        }
    }

    /// Writes owed line breaks and the line prefix before new content.
    fn begin(&mut self) {
        if self.pending > 0 && !self.out.is_empty() {
            if self.hard_break {
                self.out.push_str("  ");
            }
            self.out.push('\n');
            if self.pending > 1 {
                self.out.push_str(self.prefix.concat().trim_end());
                self.out.push('\n');
            }
            self.line_open = false;
        }
        self.pending = 0;
        self.hard_break = false;
        if !self.line_open {
            let last = self.prefix.len().checked_sub(1);
            let marker = self.marker.take();
            for (i, prefix) in self.prefix.iter().enumerate() {
                match &marker {
                    Some(marker) if Some(i) == last => self.out.push_str(marker),
                    _ => self.out.push_str(prefix),
                }
            }
            self.line_open = true;
            if self.space && self.out.is_empty() {
                self.lead = true;
            }
            self.space = false;
        }
    }

    /// [`Self::begin`] plus a collapsed space owed from earlier text.
    fn begin_inline(&mut self) {
        if self.pending > 0 || !self.line_open {
            self.begin();
        } else if self.space {
            self.out.push(' ');
            self.space = false;
        }
    }

    fn raw(&mut self, text: &str) {
        self.begin_inline();
        self.out.push_str(text);
    }

    fn text(&mut self, text: &str) {
        let mut chars = text.chars().peekable();
        while let Some(c) = chars.next() {
            if is_html_space(c) {
                self.space = true;
                continue;
            }
            let at_line_start = self.pending > 0 || !self.line_open;
            self.begin_inline();
            let prev = self.out.chars().next_back();
            let next = chars.peek().copied();
            let alnum = |c: Option<char>| c.is_some_and(char::is_alphanumeric);
            let escape = match c {
                '\\' | '`' | '*' | '[' | ']' => true,
                '_' => !(alnum(prev) && alnum(next)),
                '#' | '>' => at_line_start,
                '<' => {
                    next.is_some_and(|n| n.is_ascii_alphabetic() || matches!(n, '/' | '!' | '?'))
                }
                _ => false,
            };
            if escape {
                self.out.push('\\');
            }
            self.out.push(c);
        }
    }

    /// Renders `node`'s children inline. Leading whitespace becomes this
    /// writer's pending space; the flag says whether whitespace trailed.
    fn inline_children(
        &mut self,
        node: &Handle,
        depth: usize,
    ) -> Result<(String, bool), TransformError> {
        let mut sub = Writer::new(self.ctx, true);
        sub.children(node, depth)?;
        if sub.lead || (sub.out.is_empty() && sub.space) {
            self.space = true;
        }
        Ok((sub.out, sub.space))
    }

    fn children(&mut self, node: &Handle, depth: usize) -> Result<(), TransformError> {
        for child in node.children.borrow().iter() {
            self.node(child, depth + 1)?;
        }
        Ok(())
    }

    fn node(&mut self, node: &Handle, depth: usize) -> Result<(), TransformError> {
        self.visited += 1;
        self.ctx.tick(self.visited)?;
        check_output_len(self.out.len())?;
        let name = match kind(node) {
            Kind::Text => {
                self.text(&text_of(node));
                return Ok(());
            }
            Kind::Other => return Ok(()),
            Kind::Element(name) => name,
        };
        match name {
            _ if HIDDEN.contains(&name) => {}
            _ if depth > MAX_DEPTH => self.text(&raw_text(node, self.ctx)?),
            "br" => self.line_break(),
            "hr" => self.block(2, |w| {
                w.raw("---");
                Ok(())
            })?,
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => self.heading(node, name, depth)?,
            "strong" | "b" => self.wrapped(node, depth, "**", "**")?,
            "em" | "i" => self.wrapped(node, depth, "*", "*")?,
            "del" | "s" | "strike" => self.wrapped(node, depth, "~~", "~~")?,
            "code" | "kbd" | "samp" | "tt" => self.code(node)?,
            "a" => self.link(node, depth)?,
            "img" => self.image(node),
            "pre" => self.pre(node)?,
            "ul" | "ol" => self.list(node, name == "ol", depth)?,
            "li" => self.item(node, "- ".to_owned(), depth)?,
            "blockquote" => self.quote(node, depth)?,
            "table" => self.table(node, depth)?,
            _ if PARAGRAPHS.contains(&name) => self.block(2, |w| w.children(node, depth))?,
            _ if LINES.contains(&name) => self.block(1, |w| w.children(node, depth))?,
            _ => self.children(node, depth)?,
        }
        Ok(())
    }

    fn block(
        &mut self,
        gap: u8,
        body: impl FnOnce(&mut Self) -> Result<(), TransformError>,
    ) -> Result<(), TransformError> {
        self.newline(gap);
        body(self)?;
        self.newline(gap);
        Ok(())
    }

    fn heading(&mut self, node: &Handle, name: &str, depth: usize) -> Result<(), TransformError> {
        if self.inline {
            return self.children(node, depth);
        }
        let level = usize::from(name.as_bytes()[1] - b'0');
        self.newline(2);
        let (body, _) = self.inline_children(node, depth)?;
        if !body.is_empty() {
            self.raw(&format!("{} {}", "#".repeat(level), body));
        }
        self.newline(2);
        Ok(())
    }

    fn wrapped(
        &mut self,
        node: &Handle,
        depth: usize,
        open: &str,
        close: &str,
    ) -> Result<(), TransformError> {
        let (body, trail) = self.inline_children(node, depth)?;
        if !body.is_empty() {
            self.raw(&format!("{open}{body}{close}"));
        }
        self.space |= trail;
        Ok(())
    }

    fn code(&mut self, node: &Handle) -> Result<(), TransformError> {
        let raw = raw_text(node, self.ctx)?;
        let collapsed = raw.split(is_html_space).filter(|w| !w.is_empty());
        let code = collapsed.collect::<Vec<_>>().join(" ");
        if raw.starts_with(is_html_space) {
            self.space = true;
        }
        if !code.is_empty() {
            let fence = "`".repeat(longest_run(&code, '`') + 1);
            let pad = if code.starts_with('`') || code.ends_with('`') {
                " "
            } else {
                ""
            };
            self.raw(&format!("{fence}{pad}{code}{pad}{fence}"));
        }
        self.space |= raw.ends_with(is_html_space);
        Ok(())
    }

    fn link(&mut self, node: &Handle, depth: usize) -> Result<(), TransformError> {
        let href = attr(node, "href").unwrap_or_default();
        let (body, trail) = self.inline_children(node, depth)?;
        let scripted = href
            .get(..11)
            .is_some_and(|s| s.eq_ignore_ascii_case("javascript:"));
        if href.is_empty() || scripted {
            if !body.is_empty() {
                self.raw(&body);
            }
        } else if body.is_empty() {
            self.raw(&format!("<{}>", href.replace('>', "%3E")));
        } else {
            self.raw(&format!("[{body}]({})", destination(&href)));
        }
        self.space |= trail;
        Ok(())
    }

    fn image(&mut self, node: &Handle) {
        let alt = attr(node, "alt").unwrap_or_default();
        let alt = alt
            .replace('\\', "\\\\")
            .replace('[', "\\[")
            .replace(']', "\\]");
        match attr(node, "src").filter(|src| !src.is_empty()) {
            Some(src) => self.raw(&format!("![{alt}]({})", destination(&src))),
            None if !alt.is_empty() => self.raw(&alt),
            None => {}
        }
    }

    fn pre(&mut self, node: &Handle) -> Result<(), TransformError> {
        let code = raw_text(node, self.ctx)?;
        let code = code.strip_suffix('\n').unwrap_or(&code);
        if self.inline {
            let fence = "`".repeat(longest_run(code, '`') + 1);
            self.raw(&format!("{fence}{}{fence}", code.replace('\n', " ")));
            return Ok(());
        }
        let language = node
            .children
            .borrow()
            .iter()
            .find_map(|child| attr(child, "class"))
            .and_then(|class| {
                class.split_whitespace().find_map(|c| {
                    c.strip_prefix("language-")
                        .or_else(|| c.strip_prefix("lang-"))
                        .map(str::to_owned)
                })
            })
            .unwrap_or_default();
        let fence = "`".repeat(longest_run(code, '`').max(2) + 1);
        self.newline(2);
        self.raw(&format!("{fence}{language}"));
        for line in code.split('\n') {
            self.pending = 1;
            self.raw(line);
        }
        self.pending = 1;
        self.raw(&fence);
        self.newline(2);
        Ok(())
    }

    fn list(&mut self, node: &Handle, ordered: bool, depth: usize) -> Result<(), TransformError> {
        if self.inline {
            return self.children(node, depth);
        }
        let gap = if self.lists > 0 { 1 } else { 2 };
        let mut number = attr(node, "start")
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(1);
        self.newline(gap);
        self.lists += 1;
        for child in node.children.borrow().iter() {
            if matches!(kind(child), Kind::Element("li")) {
                let marker = if ordered {
                    format!("{number}. ")
                } else {
                    "- ".to_owned()
                };
                number = number.saturating_add(1);
                self.item(child, marker, depth + 1)?;
            } else {
                self.node(child, depth + 1)?;
            }
        }
        self.lists -= 1;
        self.newline(gap);
        Ok(())
    }

    fn item(&mut self, node: &Handle, marker: String, depth: usize) -> Result<(), TransformError> {
        if self.inline {
            return self.children(node, depth);
        }
        self.newline(1);
        self.prefix.push(" ".repeat(marker.chars().count()));
        self.marker = Some(marker);
        let result = self.children(node, depth);
        self.newline(1);
        self.prefix.pop();
        self.marker = None;
        result
    }

    fn quote(&mut self, node: &Handle, depth: usize) -> Result<(), TransformError> {
        if self.inline {
            return self.children(node, depth);
        }
        self.newline(2);
        self.prefix.push("> ".to_owned());
        let result = self.children(node, depth);
        self.newline(2);
        self.prefix.pop();
        result
    }

    fn table(&mut self, node: &Handle, depth: usize) -> Result<(), TransformError> {
        if self.inline {
            return self.children(node, depth);
        }
        let mut rows = Vec::new();
        collect_rows(node, depth, &mut rows)?;
        self.newline(2);
        for (index, row) in rows.iter().enumerate() {
            let mut cells = Vec::new();
            for cell in row.children.borrow().iter() {
                if matches!(kind(cell), Kind::Element("td" | "th")) {
                    let mut sub = Writer::new(self.ctx, true);
                    sub.children(cell, depth + 2)?;
                    cells.push(sub.out.trim().replace('|', "\\|"));
                }
            }
            if cells.is_empty() {
                continue;
            }
            self.pending = self.pending.max(1);
            self.raw(&format!("| {} |", cells.join(" | ")));
            if index == 0 {
                self.pending = 1;
                self.raw(&format!("|{}", " --- |".repeat(cells.len())));
            }
        }
        self.newline(2);
        Ok(())
    }
}

/// `tr` rows of a table, through `thead`, `tbody` and `tfoot` but not into
/// nested tables.
fn collect_rows(node: &Handle, depth: usize, rows: &mut Vec<Handle>) -> Result<(), TransformError> {
    if depth > MAX_DEPTH {
        return Ok(());
    }
    for child in node.children.borrow().iter() {
        match kind(child) {
            Kind::Element("tr") => rows.push(child.clone()),
            Kind::Element("thead" | "tbody" | "tfoot") => collect_rows(child, depth + 1, rows)?,
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn md(html: &str) -> String {
        to_markdown(html, &Context::new()).unwrap()
    }

    #[test]
    fn nested_lists_quotes_and_tables_keep_their_structure() {
        assert_eq!(
            md("<ul><li>a<ul><li>b</li></ul></li><li>c</li></ul><ol start=3><li>x</li></ol>"),
            "- a\n  - b\n- c\n\n3. x"
        );
        assert_eq!(
            md("<blockquote><p>one</p><p>two</p></blockquote><p>after</p>"),
            "> one\n>\n> two\n\nafter"
        );
        assert_eq!(
            md("<table><tr><th>k</th><th>v</th></tr><tr><td>a|b</td><td><b>1</b></td></tr></table>"),
            "| k | v |\n| --- | --- |\n| a\\|b | **1** |"
        );
    }

    #[test]
    fn inline_spacing_and_escapes() {
        assert_eq!(
            md("<p>a<b> bold </b>c <i>it</i>, <code>x`y</code> snake_case *star*</p>"),
            "a **bold** c *it*, ``x`y`` snake_case \\*star\\*"
        );
        assert_eq!(md("<p>one<br>two</p>"), "one  \ntwo");
        assert_eq!(md("<script>x()</script><p>&lt;b&gt; &amp;</p>"), "\\<b> &");
    }

    #[test]
    fn deep_nesting_degrades_to_text() {
        let html = format!("{}deep <b>text</b>", "<div><span>".repeat(MAX_DEPTH * 4));
        assert_eq!(md(&html), "deep text");
        assert_eq!(to_plain(&html, &Context::new()).unwrap(), "deep text");
    }
}
