//! Word segmentation for the code cases (camelCase, snake_case and kin).
//!
//! Tokens break at whitespace and punctuation, at a lower→upper transition,
//! at the end of an acronym (`HTTPServer` → `HTTP`, `Server`) and before an
//! uppercase letter that follows a digit. Digits attach to the preceding
//! token; a digit run that starts a token is a token of its own. Letters
//! without case (CJK and similar) break only at whitespace and punctuation.
//! An apostrophe between letters is dropped without a break. The unit is the
//! grapheme cluster, so combining marks stay with their base.

use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Class {
    Upper,
    Lower,
    Digit,
    Uncased,
    Apostrophe,
    Separator,
}

fn class(grapheme: &str) -> Class {
    let c = grapheme.chars().next().unwrap_or(' ');
    if c.is_uppercase() || is_titlecase(c) {
        Class::Upper
    } else if c.is_lowercase() {
        Class::Lower
    } else if c.is_numeric() {
        Class::Digit
    } else if c.is_alphabetic() {
        Class::Uncased
    } else if c == '\'' || c == '\u{2019}' {
        Class::Apostrophe
    } else {
        Class::Separator
    }
}

/// The four titlecase digraphs of Latin Extended-B.
fn is_titlecase(c: char) -> bool {
    matches!(c, '\u{01C5}' | '\u{01C8}' | '\u{01CB}' | '\u{01F2}')
}

fn is_letter_or_digit(class: Class) -> bool {
    !matches!(class, Class::Apostrophe | Class::Separator)
}

#[derive(Default)]
struct Builder<'a> {
    tokens: Vec<String>,
    /// Graphemes of the open token with their classes.
    open: Vec<(&'a str, Class)>,
}

impl<'a> Builder<'a> {
    fn close(&mut self) {
        if !self.open.is_empty() {
            self.tokens
                .push(self.open.drain(..).map(|(g, _)| g).collect());
        }
    }

    fn push(&mut self, grapheme: &'a str, class: Class) {
        let last = self.open.last().map(|&(_, c)| c);
        let leading_digits = self.open.iter().all(|&(_, c)| c == Class::Digit);
        let breaks = match (last, class) {
            (None, _) => false,
            (Some(_), Class::Digit) => false,
            (Some(_), _) if leading_digits => true,
            (Some(Class::Lower | Class::Uncased | Class::Digit), Class::Upper) => true,
            (Some(Class::Upper), Class::Lower) => {
                let uppers = self
                    .open
                    .iter()
                    .rev()
                    .take_while(|&&(_, c)| c == Class::Upper)
                    .count();
                if uppers >= 2 {
                    let head = self.open.pop();
                    self.close();
                    self.open.extend(head);
                }
                false
            }
            _ => false,
        };
        if breaks {
            self.close();
        }
        self.open.push((grapheme, class));
    }
}

/// Splits `text` into identifier tokens.
pub(crate) fn tokens(text: &str) -> Vec<String> {
    let graphemes: Vec<(&str, Class)> = text.graphemes(true).map(|g| (g, class(g))).collect();
    let mut builder = Builder::default();
    for (i, &(grapheme, class)) in graphemes.iter().enumerate() {
        match class {
            Class::Separator => builder.close(),
            Class::Apostrophe => {
                let joined = i > 0
                    && is_letter_or_digit(graphemes[i - 1].1)
                    && graphemes
                        .get(i + 1)
                        .is_some_and(|&(_, c)| is_letter_or_digit(c));
                if !joined {
                    builder.close();
                }
            }
            _ => builder.push(grapheme, class),
        }
    }
    builder.close();
    builder.tokens
}

/// Whether `word` is written in capitals: two or more uppercase letters and
/// no lowercase one (`URL2`, `HTTP`, but not `I` or `Http`).
pub(crate) fn is_all_caps(word: &str) -> bool {
    let mut uppers = 0;
    for c in word.chars() {
        if c.is_lowercase() {
            return false;
        }
        if c.is_uppercase() {
            uppers += 1;
        }
    }
    uppers >= 2
}

#[cfg(test)]
mod tests {
    use super::*;

    fn split(text: &str) -> Vec<String> {
        tokens(text)
    }

    #[test]
    fn splits_transitions_acronyms_and_digits() {
        assert_eq!(
            split("parseHTTPResponse2XX"),
            ["parse", "HTTP", "Response2", "XX"]
        );
        assert_eq!(split("parseURL2Text"), ["parse", "URL2", "Text"]);
        assert_eq!(split("HTTPServer"), ["HTTP", "Server"]);
        assert_eq!(split("2fast  for-you"), ["2", "fast", "for", "you"]);
        assert_eq!(split("__don't.stop__"), ["dont", "stop"]);
    }

    #[test]
    fn uncased_scripts_break_only_at_separators() {
        assert_eq!(split("東京 タワー_x"), ["東京", "タワー", "x"]);
        assert_eq!(split("cafe\u{301}Noir"), ["cafe\u{301}", "Noir"]);
    }

    #[test]
    fn all_caps_needs_two_capitals() {
        assert!(is_all_caps("URL2"));
        assert!(!is_all_caps("I"));
        assert!(!is_all_caps("Http"));
    }
}
