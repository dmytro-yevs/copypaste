//! Inspector counts for the preview header and the bounded preview prefix.

use serde::Serialize;
use unicode_segmentation::UnicodeSegmentation;

use crate::PREVIEW_BYTES;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Counts {
    /// User-perceived characters (extended grapheme clusters).
    pub characters: usize,
    /// UAX #29 words.
    pub words: usize,
    /// Lines; a final line break does not start another line.
    pub lines: usize,
    pub bytes: usize,
    pub code_points: usize,
    pub utf16_units: usize,
}

pub fn counts(text: &str) -> Counts {
    Counts {
        characters: text.graphemes(true).count(),
        words: text.unicode_words().count(),
        lines: text.lines().count(),
        bytes: text.len(),
        code_points: text.chars().count(),
        utf16_units: text.chars().map(char::len_utf16).sum(),
    }
}

/// The longest prefix of `text` within [`PREVIEW_BYTES`] that ends on a
/// character boundary.
pub fn preview_prefix(text: &str) -> &str {
    if text.len() <= PREVIEW_BYTES {
        return text;
    }
    let mut end = PREVIEW_BYTES;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_graphemes_words_and_lines() {
        let c = counts("Hi 👩‍💻 there\r\nok\n");
        assert_eq!(c.characters, 14);
        assert_eq!(c.words, 3);
        assert_eq!(c.lines, 2);
        assert_eq!(c.code_points, 17);
        assert_eq!(c.utf16_units, 19);
    }

    #[test]
    fn preview_prefix_stops_on_a_char_boundary() {
        let text = format!("{}é", "a".repeat(PREVIEW_BYTES - 1));
        assert_eq!(preview_prefix(&text).len(), PREVIEW_BYTES - 1);
        assert_eq!(preview_prefix("short"), "short");
    }
}
