//! Single-pass renderer. A value produced by a placeholder (clipboard text,
//! field input, nested output, modifier output) is appended as text and never
//! parsed again.

use std::collections::{BTreeMap, HashMap};
use std::time::SystemTime;

use icu_locale::Locale;
use jiff::tz::TimeZone;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use unicode_segmentation::UnicodeSegmentation;

use crate::dates;
use crate::fields::{FieldKind, FieldRegistry, Registered};
use crate::lint::Issue;
use crate::nested::{Chain, NestedBody, Origin, SnippetResolver};
use crate::random;
use crate::syntax::{Directive, Parsed, Segment, Template};
use crate::MAX_OUTPUT_BYTES;

/// Deterministic text-to-text transforms usable as `| modifier`.
pub trait Modifiers {
    fn contains(&self, id: &str) -> bool;
    /// `None` when the transform rejects this input.
    fn apply(&self, id: &str, input: &str) -> Option<String>;
}

/// No modifiers at all: every `| modifier` is unknown.
pub struct NoModifiers;

impl Modifiers for NoModifiers {
    fn contains(&self, _id: &str) -> bool {
        false
    }

    fn apply(&self, _id: &str, _input: &str) -> Option<String> {
        None
    }
}

/// Clipboard text for `clipboard index=N`. Index 0 is the system clipboard
/// as captured before expansion, before CopyPaste writes its own output;
/// N ≥ 1 is the Nth previous text history item. Each index is asked once
/// per expansion.
pub trait ClipboardSource {
    fn text(&self, index: u8) -> Option<String>;
}

impl<F: Fn(u8) -> Option<String>> ClipboardSource for F {
    fn text(&self, index: u8) -> Option<String> {
        self(index)
    }
}

/// Everything one expansion reads from outside the template.
pub struct RenderContext<'a> {
    /// The one instant every date placeholder uses, nested ones included.
    pub now: SystemTime,
    /// IANA id of the device zone, or `UTC`. The renderer never detects it.
    pub local_zone: &'a str,
    /// BCP 47 tag of the app locale, the default for date placeholders.
    pub locale: &'a str,
    /// Field values by name. A missing name takes the field's default.
    pub values: &'a BTreeMap<String, String>,
    pub clipboard: &'a dyn ClipboardSource,
    pub snippets: &'a dyn SnippetResolver,
    pub modifiers: &'a dyn Modifiers,
    /// The snippet being expanded, so a reference back to it is a cycle.
    pub origin: Option<&'a Origin>,
}

/// An input that blocks the final paste while empty.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Required {
    Field { name: String },
    Clipboard { index: u8 },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rendered {
    pub text: String,
    /// Byte offset of the first top-level `cursor` in `text`.
    pub cursor: Option<usize>,
    pub warnings: Vec<Issue>,
    pub missing_required: Vec<Required>,
    /// Placeholders copied verbatim because they could not be filled.
    pub unfilled: u32,
}

impl Rendered {
    /// Caret steps from the end of `text` back to the cursor, counted in
    /// grapheme clusters, so CRLF is one step.
    pub fn cursor_graphemes_from_end(&self) -> Option<usize> {
        self.cursor
            .map(|offset| self.text[offset..].graphemes(true).count())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RenderError {
    #[error("expanded output is larger than {MAX_OUTPUT_BYTES} bytes")]
    OutputTooLarge,
    #[error("the instant is outside the supported date range")]
    InstantOutOfRange,
}

/// Expands `template` once.
pub fn render(template: &Template, ctx: &RenderContext<'_>) -> Result<Rendered, RenderError> {
    let instant = Timestamp::try_from(ctx.now).map_err(|_| RenderError::InstantOutOfRange)?;
    let mut renderer = Renderer {
        ctx,
        instant,
        zone: dates::resolve_zone(ctx.local_zone),
        locale: Locale::try_from_str(ctx.locale).unwrap_or(Locale::UNKNOWN),
        chain: Chain::new(ctx.origin),
        fields: FieldRegistry::default(),
        clipboard: HashMap::new(),
        dates: HashMap::new(),
        rendered: Rendered {
            text: String::new(),
            cursor: None,
            warnings: Vec::new(),
            missing_required: Vec::new(),
            unfilled: 0,
        },
    };
    let mut text = String::new();
    renderer.render_into(template, &mut text)?;
    renderer.rendered.text = text;
    Ok(renderer.rendered)
}

enum Outcome {
    Text(String),
    Verbatim(Issue),
    Nothing,
}

struct Renderer<'a> {
    ctx: &'a RenderContext<'a>,
    instant: Timestamp,
    zone: Option<TimeZone>,
    locale: Locale,
    chain: Chain,
    fields: FieldRegistry,
    clipboard: HashMap<u8, Option<String>>,
    dates: HashMap<String, Result<String, Issue>>,
    rendered: Rendered,
}

fn push(out: &mut String, text: &str) -> Result<(), RenderError> {
    if out.len() + text.len() > MAX_OUTPUT_BYTES {
        return Err(RenderError::OutputTooLarge);
    }
    out.push_str(text);
    Ok(())
}

impl Renderer<'_> {
    fn render_into(&mut self, template: &Template, out: &mut String) -> Result<(), RenderError> {
        for segment in template.segments() {
            let placeholder = match segment {
                Segment::Text(text) => {
                    push(out, text)?;
                    continue;
                }
                Segment::Placeholder(placeholder) => placeholder,
            };
            let outcome = match &placeholder.parsed {
                Err(issue) => Outcome::Verbatim(issue.clone()),
                Ok(parsed) => self.placeholder(parsed, out.len())?,
            };
            match outcome {
                Outcome::Text(text) => push(out, &text)?,
                Outcome::Verbatim(issue) => {
                    push(out, template.verbatim(placeholder))?;
                    self.rendered.unfilled += 1;
                    self.rendered.warnings.push(issue);
                }
                Outcome::Nothing => {}
            }
        }
        Ok(())
    }

    /// `offset` is where the value would start in the current output, which
    /// is the final text only at the top level.
    fn placeholder(&mut self, parsed: &Parsed, offset: usize) -> Result<Outcome, RenderError> {
        let ctx = self.ctx;
        if let Some(id) = parsed
            .modifiers
            .iter()
            .find(|id| !ctx.modifiers.contains(id))
        {
            return Ok(Outcome::Verbatim(Issue::UnknownModifier { id: id.clone() }));
        }
        let value = match &parsed.directive {
            Directive::Cursor => {
                if self.chain.depth() == 0 && self.rendered.cursor.is_none() {
                    self.rendered.cursor = Some(offset);
                }
                return Ok(Outcome::Nothing);
            }
            Directive::Date(spec) => {
                let formatted = self
                    .dates
                    .entry(spec.key.clone())
                    .or_insert_with(|| {
                        dates::format(spec, self.instant, self.zone.as_ref(), &self.locale)
                    })
                    .clone();
                match formatted {
                    Ok(text) => text,
                    Err(issue) => return Ok(Outcome::Verbatim(issue)),
                }
            }
            Directive::Clipboard { index, required } => self.clipboard(*index, *required),
            Directive::Field(spec) => match self.fields.register(spec) {
                Registered::OverLimit => {
                    return Ok(Outcome::Verbatim(Issue::TooManyFields {
                        name: spec.name.clone(),
                    }))
                }
                Registered::Known(field) | Registered::Conflicting(field) => {
                    let given = ctx.values.get(&field.name).filter(|value| {
                        field.kind != FieldKind::Choice || field.options.contains(value)
                    });
                    let value = given
                        .or(field.default.as_ref())
                        .cloned()
                        .unwrap_or_default();
                    if value.is_empty() && field.required {
                        self.require(Required::Field { name: field.name });
                    }
                    value
                }
            },
            Directive::Snippet(reference) => match self.chain.enter(reference, ctx.snippets) {
                Err(issue) => return Ok(Outcome::Verbatim(issue)),
                Ok(NestedBody::Literal(text)) => {
                    self.chain.leave();
                    text
                }
                Ok(NestedBody::Template(nested)) => {
                    let mut text = String::new();
                    let result = self.render_into(&nested, &mut text);
                    self.chain.leave();
                    result?;
                    text
                }
            },
            Directive::Uuid => random::uuid(),
            Directive::Random { min, max } => random::integer(*min, *max),
        };
        let mut value = value;
        for id in &parsed.modifiers {
            match ctx.modifiers.apply(id, &value) {
                Some(next) => value = next,
                None => return Ok(Outcome::Verbatim(Issue::ModifierFailed { id: id.clone() })),
            }
        }
        Ok(Outcome::Text(value))
    }

    fn clipboard(&mut self, index: u8, required: bool) -> String {
        let ctx = self.ctx;
        let text = self
            .clipboard
            .entry(index)
            .or_insert_with(|| ctx.clipboard.text(index))
            .clone()
            .unwrap_or_default();
        if text.is_empty() {
            let warning = Issue::EmptyClipboard { index };
            if !self.rendered.warnings.contains(&warning) {
                self.rendered.warnings.push(warning);
            }
            if required {
                self.require(Required::Clipboard { index });
            }
        }
        text
    }

    fn require(&mut self, required: Required) {
        if !self.rendered.missing_required.contains(&required) {
            self.rendered.missing_required.push(required);
        }
    }
}
