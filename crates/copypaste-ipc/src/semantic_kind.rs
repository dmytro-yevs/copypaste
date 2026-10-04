//! Semantic classes derived from the complete content of a text clip.
//!
//! These do not replace [`crate::ContentClass`]. Content class continues to own
//! capture, transport, and size-limit behavior; semantic kind exists only for
//! presentation and full-history filtering.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticKind {
    PlainText,
    Link,
    Email,
    Color,
    Phone,
    Code,
    Json,
    Path,
}

impl SemanticKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PlainText => "plain_text",
            Self::Link => "link",
            Self::Email => "email",
            Self::Color => "color",
            Self::Phone => "phone",
            Self::Code => "code",
            Self::Json => "json",
            Self::Path => "path",
        }
    }
}
