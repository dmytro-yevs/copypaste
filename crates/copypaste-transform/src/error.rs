//! Failures and warnings. Neither ever carries input or output text, so both
//! are safe to log and to send over IPC.

use serde::Serialize;

use crate::MAX_OUTPUT_BYTES;

/// A position in the input: byte offset plus 1-based line and column, where
/// the column counts Unicode scalar values from the start of the line.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Location {
    pub offset: usize,
    pub line: usize,
    pub column: usize,
}

impl Location {
    /// The location of byte `offset` in `text`, clamped to the text and
    /// moved back to a character boundary.
    pub fn of(text: &str, offset: usize) -> Self {
        let mut offset = offset.min(text.len());
        while !text.is_char_boundary(offset) {
            offset -= 1;
        }
        let before = &text[..offset];
        let line_start = before.rfind('\n').map_or(0, |i| i + 1);
        Self {
            offset,
            line: before.bytes().filter(|&b| b == b'\n').count() + 1,
            column: before[line_start..].chars().count() + 1,
        }
    }
}

/// A non-fatal note about a result (lossy, duplicate keys, unknown escapes).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Warning {
    /// Stable dotted code such as `json.duplicate_key`; the UI localizes it.
    pub code: &'static str,
    pub location: Option<Location>,
}

impl Warning {
    pub fn new(code: &'static str) -> Self {
        Self {
            code,
            location: None,
        }
    }

    pub fn at(code: &'static str, location: Location) -> Self {
        Self {
            code,
            location: Some(location),
        }
    }
}

/// Why a parameter value was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ParamIssue {
    Unknown,
    WrongType,
    NotAnOption,
    OutOfRange,
    TooLong,
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum TransformError {
    #[error("unknown transform {id}")]
    UnknownTransform { id: String },
    #[error("unknown modifier {name}")]
    UnknownModifier { name: String },
    #[error("transform {id} version {version} is not supported (current {current})")]
    UnsupportedVersion {
        id: String,
        version: u32,
        current: u32,
    },
    #[error("parameter {name} is invalid: {issue:?}")]
    InvalidParam { name: String, issue: ParamIssue },
    #[error("input of {bytes} bytes is too large to transform (limit {limit})")]
    InputTooLarge { bytes: usize, limit: usize },
    #[error("output is too large (limit {limit} bytes)")]
    OutputTooLarge { limit: usize },
    /// The input cannot be transformed; `code` is a stable dotted code such
    /// as `json.trailing_comma` and `location` points into the input.
    #[error("invalid input: {code}")]
    InvalidInput {
        code: &'static str,
        location: Option<Location>,
    },
    #[error("transform was cancelled")]
    Cancelled,
    #[error("transform exceeded its time budget")]
    BudgetExceeded,
}

impl TransformError {
    /// A stable code for the UI and the runtime log.
    pub fn code(&self) -> &'static str {
        match self {
            Self::UnknownTransform { .. } => "unknown_transform",
            Self::UnknownModifier { .. } => "unknown_modifier",
            Self::UnsupportedVersion { .. } => "unsupported_version",
            Self::InvalidParam { .. } => "invalid_param",
            Self::InputTooLarge { .. } => "input_too_large",
            Self::OutputTooLarge { .. } => "output_too_large",
            Self::InvalidInput { code, .. } => code,
            Self::Cancelled => "cancelled",
            Self::BudgetExceeded => "budget_exceeded",
        }
    }

    pub fn invalid_input(code: &'static str, text: &str, offset: usize) -> Self {
        Self::InvalidInput {
            code,
            location: Some(Location::of(text, offset)),
        }
    }
}

/// Fails once a growing output would pass [`MAX_OUTPUT_BYTES`]. Transforms
/// whose output can outgrow the input call it while they build.
pub(crate) fn check_output_len(len: usize) -> Result<(), TransformError> {
    if len > MAX_OUTPUT_BYTES {
        return Err(TransformError::OutputTooLarge {
            limit: MAX_OUTPUT_BYTES,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn location_counts_lines_and_scalar_columns() {
        let text = "ab\nçd}";
        assert_eq!(
            Location::of(text, 6),
            Location {
                offset: 6,
                line: 2,
                column: 3
            }
        );
        assert_eq!(Location::of(text, 4).offset, 3);
    }
}
