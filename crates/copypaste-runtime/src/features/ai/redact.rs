//! Inline redaction of secrets in tool results.

/// Opens the marker that replaces an inline secret; the kind name and `]`
/// follow, for example `[redacted: api key]`.
#[expect(dead_code, reason = "read once tool results are redacted")]
pub(super) const MARKER_PREFIX: &str = "[redacted: ";
