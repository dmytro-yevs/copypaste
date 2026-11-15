//! Line transforms. Each keeps the input's line-ending style (LF or CRLF,
//! taken from the first line break) and whether it ends with a newline.

mod compose;
mod order;
mod whitespace;

use crate::catalogue::{Catalogue, Context};
use crate::error::{check_output_len, TransformError};

pub(crate) fn register(catalogue: &mut Catalogue) {
    for def in [
        &whitespace::TRIM,
        &whitespace::TRIM_EACH,
        &whitespace::COLLAPSE,
        &whitespace::REMOVE_EMPTY,
        &order::SORT,
        &order::DEDUPE,
        &order::REVERSE,
        &compose::JOIN,
        &compose::UNWRAP,
        &compose::PREFIX,
    ] {
        catalogue.add(def);
    }
}

/// A text split into lines without their terminators.
pub(crate) struct Lines<'a> {
    pub(crate) lines: Vec<&'a str>,
    eol: &'static str,
    final_newline: bool,
}

impl<'a> Lines<'a> {
    pub(crate) fn split(text: &'a str) -> Self {
        let crlf = text.find('\n').is_some_and(|i| text[..i].ends_with('\r'));
        let final_newline = text.ends_with('\n');
        let body = text.strip_suffix('\n').unwrap_or(text);
        let lines = if text.is_empty() {
            Vec::new()
        } else if crlf {
            body.split('\n')
                .map(|l| l.strip_suffix('\r').unwrap_or(l))
                .collect()
        } else {
            body.split('\n').collect()
        };
        Self {
            lines,
            eol: if crlf { "\r\n" } else { "\n" },
            final_newline,
        }
    }

    /// Joins `lines` with this text's line ending, adding a final newline
    /// when the input had one and at least one line remains.
    pub(crate) fn join<S: AsRef<str>>(
        &self,
        lines: impl IntoIterator<Item = S>,
        ctx: &Context,
    ) -> Result<String, TransformError> {
        let mut out = String::new();
        let mut count = 0;
        for (i, line) in lines.into_iter().enumerate() {
            count += 1;
            ctx.tick(i)?;
            if i > 0 {
                out.push_str(self.eol);
            }
            out.push_str(line.as_ref());
            check_output_len(out.len())?;
        }
        if self.final_newline && count > 0 {
            out.push_str(self.eol);
        }
        Ok(out)
    }
}

/// Whether `line` holds nothing, or only whitespace when `blank_counts`.
pub(crate) fn is_empty_line(line: &str, blank_counts: bool) -> bool {
    if blank_counts {
        line.trim().is_empty()
    } else {
        line.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_and_join_round_trip_endings() {
        let ctx = Context::new();
        for text in ["a\r\nb", "a\nb\n", "", "\n", "x"] {
            let lines = Lines::split(text);
            assert_eq!(lines.join(&lines.lines, &ctx).unwrap(), text);
        }
        assert_eq!(Lines::split("a\r\nb\r\n").lines, ["a", "b"]);
    }
}
