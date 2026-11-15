//! Editor lint: every problem a body has, with the span to underline.
//!
//! Problems inside a nested snippet's own body belong to that snippet. Only
//! what changes this expansion (a cycle, depth, the field cap or a nested
//! cursor) is reported here, on the reference that leads to it.

use std::ops::Range;

use icu_locale::Locale;
use serde::{Deserialize, Serialize};

use crate::dates;
use crate::fields::{FieldRegistry, Registered};
use crate::nested::{Chain, NestedBody, Origin, SnippetResolver};
use crate::render::Modifiers;
use crate::syntax::{Directive, Template};

/// One problem with a placeholder, shared by lint and render warnings.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum Issue {
    Unclosed,
    Malformed,
    UnknownPlaceholder {
        name: String,
    },
    UnknownParameter {
        key: String,
    },
    DuplicateParameter {
        key: String,
    },
    MissingParameter {
        key: String,
    },
    InvalidValue {
        key: String,
    },
    ConflictingParameters,
    UnknownLocale,
    UnknownTimeZone,
    InvalidPattern,
    DateOutOfRange,
    UnknownModifier {
        id: String,
    },
    ModifierFailed {
        id: String,
    },
    EmptyClipboard {
        index: u8,
    },
    ExtraCursor,
    NestedCursor,
    TooManyFields {
        name: String,
    },
    ConflictingField {
        name: String,
    },
    ChoiceDefaultNotOption {
        name: String,
    },
    /// Labels (abbreviation, else id) from the first repeated snippet to the
    /// reference that closes the loop.
    Cycle {
        chain: Vec<String>,
    },
    DepthExceeded,
    /// More nested snippets than one expansion inserts.
    NestingLimit,
    UnknownSnippet,
    /// Template is off but the body holds placeholders.
    ContainsPlaceholders,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Hint,
    Warning,
    Error,
}

impl Issue {
    pub fn severity(&self) -> Severity {
        match self {
            Self::ContainsPlaceholders => Severity::Hint,
            Self::EmptyClipboard { .. }
            | Self::ExtraCursor
            | Self::NestedCursor
            | Self::TooManyFields { .. }
            | Self::ConflictingField { .. }
            | Self::ChoiceDefaultNotOption { .. }
            | Self::UnknownSnippet => Severity::Warning,
            _ => Severity::Error,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lint {
    pub severity: Severity,
    /// Byte range in the linted body.
    pub span: Range<usize>,
    pub issue: Issue,
}

pub struct LintContext<'a> {
    /// BCP 47 tag of the app locale, used to check date patterns.
    pub locale: &'a str,
    pub snippets: &'a dyn SnippetResolver,
    pub modifiers: &'a dyn Modifiers,
    /// The snippet being edited, so a reference back to it is a cycle.
    pub origin: Option<&'a Origin>,
}

/// Lints a body. With Template off the only lint is the hint that the body
/// holds placeholders; the hint never switches Template on.
pub fn lint(template: &Template, template_on: bool, ctx: &LintContext<'_>) -> Vec<Lint> {
    if !template_on {
        return template
            .placeholders()
            .find(|p| p.parsed.is_ok())
            .map(|p| lint_of(p.span.clone(), Issue::ContainsPlaceholders))
            .into_iter()
            .collect();
    }
    let mut walk = Walk {
        ctx,
        locale: Locale::try_from_str(ctx.locale).unwrap_or(Locale::UNKNOWN),
        chain: Chain::new(ctx.origin),
        fields: FieldRegistry::default(),
        cursors: 0,
        lints: Vec::new(),
    };
    walk.template(template, None);
    walk.lints
}

fn lint_of(span: Range<usize>, issue: Issue) -> Lint {
    Lint {
        severity: issue.severity(),
        span,
        issue,
    }
}

struct Walk<'a> {
    ctx: &'a LintContext<'a>,
    locale: Locale,
    chain: Chain,
    fields: FieldRegistry,
    cursors: usize,
    lints: Vec<Lint>,
}

impl Walk<'_> {
    /// `anchor` is the top-level reference span while walking a nested body.
    fn template(&mut self, template: &Template, anchor: Option<&Range<usize>>) {
        for placeholder in template.placeholders() {
            let span = anchor.unwrap_or(&placeholder.span).clone();
            let parsed = match &placeholder.parsed {
                Ok(parsed) => parsed,
                Err(issue) => {
                    if anchor.is_none() {
                        self.push(span, issue.clone());
                    }
                    continue;
                }
            };
            if anchor.is_none() {
                for id in parsed
                    .modifiers
                    .iter()
                    .filter(|id| !self.ctx.modifiers.contains(id))
                {
                    self.push(span.clone(), Issue::UnknownModifier { id: id.clone() });
                }
            }
            match &parsed.directive {
                Directive::Date(spec) if anchor.is_none() => {
                    if let Err(issue) = dates::check_pattern(spec, &self.locale) {
                        self.push(span, issue);
                    }
                }
                Directive::Cursor if anchor.is_some() => self.push(span, Issue::NestedCursor),
                Directive::Cursor => {
                    self.cursors += 1;
                    if self.cursors > 1 {
                        self.push(span, Issue::ExtraCursor);
                    }
                }
                Directive::Field(spec) => {
                    let name = spec.name.clone();
                    match self.fields.register(spec) {
                        Registered::OverLimit => self.push(span, Issue::TooManyFields { name }),
                        Registered::Conflicting(_) if anchor.is_none() => {
                            self.push(span, Issue::ConflictingField { name })
                        }
                        _ if anchor.is_none() && spec.default_is_invalid() => {
                            self.push(span, Issue::ChoiceDefaultNotOption { name })
                        }
                        _ => {}
                    }
                }
                Directive::Snippet(reference) => {
                    match self.chain.enter(reference, self.ctx.snippets) {
                        Err(Issue::UnknownSnippet) if anchor.is_some() => {}
                        Err(issue) => self.push(span, issue),
                        Ok(body) => {
                            if let NestedBody::Template(nested) = body {
                                self.template(&nested, Some(&span));
                            }
                            self.chain.leave();
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn push(&mut self, span: Range<usize>, issue: Issue) {
        let lint = lint_of(span, issue);
        if !self.lints.contains(&lint) {
            self.lints.push(lint);
        }
    }
}
