//! Snippet templates: syntax, lint and a single-pass renderer.
//!
//! Dates are formatted with CLDR data so output is identical on every OS for
//! the same locale, zone and instant. The renderer never detects the local
//! zone or reads the clipboard or other snippets itself; the caller passes
//! them in.

#![forbid(unsafe_code)]

/// Largest expanded output; a larger expansion is refused.
pub const MAX_OUTPUT_BYTES: usize = 1024 * 1024;

/// Most distinct fields in one expansion, nested snippets included.
pub const MAX_FIELDS: usize = 10;
