//! Snippet templates: syntax, lint and a single-pass renderer.
//!
//! Dates are formatted with CLDR data so output is identical on every OS for
//! the same locale, zone and instant. The renderer never detects the local
//! zone or reads the clipboard or other snippets itself; the caller passes
//! them in.

#![forbid(unsafe_code)]

mod dates;
mod fields;
mod lint;
mod nested;
mod random;
mod render;
mod syntax;

pub use fields::{fields, FieldKind, TemplateField};
pub use lint::{lint, Issue, Lint, LintContext, Severity};
pub use nested::{NestedBody, NestedSnippet, NoSnippets, Origin, SnippetRef, SnippetResolver};
pub use render::{
    render, ClipboardSource, Modifiers, NoModifiers, RenderContext, RenderError, Rendered, Required,
};
pub use syntax::{parse, Template};

/// Largest expanded output; a larger expansion is refused.
pub const MAX_OUTPUT_BYTES: usize = 1024 * 1024;

/// Most distinct fields in one expansion, nested snippets included.
pub const MAX_FIELDS: usize = 10;

/// Deepest nested snippet that is still expanded; the top level is depth 0.
pub const MAX_DEPTH: usize = 3;

/// Highest `clipboard index`: the Nth previous text history item.
pub const MAX_CLIPBOARD_INDEX: u8 = 50;

/// Most options of one choice field.
pub const MAX_CHOICE_OPTIONS: usize = 50;

/// Longest field name, in characters.
pub const MAX_FIELD_NAME_CHARS: usize = 40;
