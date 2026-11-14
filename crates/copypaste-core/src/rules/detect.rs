//! Content kind detection that rule conditions ask for.
//!
//! Deterministic and local. It examines a bounded prefix, reuses the credential
//! and semantic classifiers, and runs only when a condition needs a kind.

use serde::{Deserialize, Serialize};

/// Bytes of text examined by detection (content-rules R-15).
pub const DETECTION_PREFIX_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DetectedKind {
    Credential,
    Url,
    Email,
    Phone,
    Code,
    Color,
    Json,
    FilePath,
    Number,
    OneTimeCode,
    PaymentCard,
}

impl DetectedKind {
    pub const ALL: [Self; 11] = [
        Self::Credential,
        Self::Url,
        Self::Email,
        Self::Phone,
        Self::Code,
        Self::Color,
        Self::Json,
        Self::FilePath,
        Self::Number,
        Self::OneTimeCode,
        Self::PaymentCard,
    ];

    const fn bit(self) -> u16 {
        1 << self as u16
    }
}

/// A set of detected kinds; one item may have several.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DetectedKinds(u16);

impl DetectedKinds {
    #[must_use]
    pub const fn contains(self, kind: DetectedKind) -> bool {
        self.0 & kind.bit() != 0
    }

    pub fn insert(&mut self, kind: DetectedKind) {
        self.0 |= kind.bit();
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    #[must_use]
    pub const fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }

    pub fn iter(self) -> impl Iterator<Item = DetectedKind> {
        DetectedKind::ALL
            .into_iter()
            .filter(move |kind| self.contains(*kind))
    }
}

impl FromIterator<DetectedKind> for DetectedKinds {
    fn from_iter<I: IntoIterator<Item = DetectedKind>>(iter: I) -> Self {
        let mut kinds = Self::default();
        for kind in iter {
            kinds.insert(kind);
        }
        kinds
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Detection {
    pub kinds: DetectedKinds,
    /// The text was longer than the prefix; kinds describe the prefix only.
    pub partial: bool,
}

/// Longest prefix of `text` that fits in `max` bytes on a char boundary.
pub(crate) fn prefix(text: &str, max: usize) -> (&str, bool) {
    if text.len() <= max {
        return (text, false);
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    (&text[..end], true)
}

/// Detect the kinds of a plain-text representation.
#[must_use]
pub fn detect(text: &str) -> Detection {
    use copypaste_ipc::{content_type::TEXT, SemanticKind};

    let (sample, partial) = prefix(text, DETECTION_PREFIX_BYTES);
    let mut kinds = DetectedKinds::default();
    let credential = crate::classify_credentials(TEXT, sample);
    if credential.is_some_and(|privacy| privacy.secret) {
        kinds.insert(DetectedKind::Credential);
    }
    if credential.is_none() {
        let semantic = crate::classify_semantic(TEXT, sample).map(|class| class.kind);
        let kind = match semantic {
            Some(SemanticKind::Link) => Some(DetectedKind::Url),
            Some(SemanticKind::Email) => Some(DetectedKind::Email),
            Some(SemanticKind::Phone) => Some(DetectedKind::Phone),
            Some(SemanticKind::Code) => Some(DetectedKind::Code),
            Some(SemanticKind::Color) => Some(DetectedKind::Color),
            Some(SemanticKind::Json) => Some(DetectedKind::Json),
            Some(SemanticKind::Path) => Some(DetectedKind::FilePath),
            Some(SemanticKind::Credentials) => Some(DetectedKind::Credential),
            Some(SemanticKind::PlainText) | None => None,
        };
        if let Some(kind) = kind {
            kinds.insert(kind);
        }
    }
    let value = sample.trim();
    if is_number(value) {
        kinds.insert(DetectedKind::Number);
    }
    if is_one_time_code(value) {
        kinds.insert(DetectedKind::OneTimeCode);
    }
    if contains_payment_card(sample) {
        kinds.insert(DetectedKind::PaymentCard);
    }
    Detection { kinds, partial }
}

fn is_number(value: &str) -> bool {
    let digits = value.strip_prefix(['+', '-']).unwrap_or(value);
    let bytes = digits.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 64
        && bytes[0].is_ascii_digit()
        && bytes[bytes.len() - 1].is_ascii_digit()
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b',' | b'.' | b' ' | b'\'' | b'_'))
        && !bytes
            .windows(2)
            .any(|pair| !pair[0].is_ascii_digit() && !pair[1].is_ascii_digit())
}

fn is_code_digits(token: &str) -> bool {
    (4..=8).contains(&token.len()) && token.bytes().all(|byte| byte.is_ascii_digit())
}

/// A bare 4–8 digit code (optionally split once by a space or hyphen), or a
/// short message that names a code and carries one.
fn is_one_time_code(value: &str) -> bool {
    let joined: String = value
        .chars()
        .filter(|ch| !matches!(ch, ' ' | '-'))
        .collect();
    if value.len() <= 9 && value.matches([' ', '-']).count() <= 1 && is_code_digits(&joined) {
        return true;
    }
    if value.chars().count() > 200 {
        return false;
    }
    let lower = value.to_lowercase();
    ["code", "otp", "passcode", "verification", "pin", "код"]
        .iter()
        .any(|word| lower.contains(word))
        && value
            .split(|ch: char| !ch.is_ascii_digit())
            .any(is_code_digits)
}

/// A 13–19 digit run (single spaces or hyphens allowed between digits) with a
/// plausible network prefix that passes the Luhn check.
fn contains_payment_card(text: &str) -> bool {
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if !bytes[index].is_ascii_digit() || index > 0 && bytes[index - 1].is_ascii_alphanumeric() {
            index += 1;
            continue;
        }
        let mut digits = Vec::with_capacity(19);
        let mut cursor = index;
        while cursor < bytes.len() && digits.len() <= 19 {
            if bytes[cursor].is_ascii_digit() {
                digits.push(bytes[cursor] - b'0');
                cursor += 1;
            } else if matches!(bytes[cursor], b' ' | b'-')
                && bytes.get(cursor + 1).is_some_and(u8::is_ascii_digit)
            {
                cursor += 1;
            } else {
                break;
            }
        }
        let bounded = !bytes
            .get(cursor)
            .is_some_and(|byte| byte.is_ascii_alphanumeric());
        if bounded
            && (13..=19).contains(&digits.len())
            && (2..=6).contains(&digits[0])
            && luhn(&digits)
        {
            return true;
        }
        index = cursor.max(index + 1);
    }
    false
}

fn luhn(digits: &[u8]) -> bool {
    let sum: u32 = digits
        .iter()
        .rev()
        .enumerate()
        .map(|(position, &digit)| {
            let digit = u32::from(digit);
            if position % 2 == 1 {
                let doubled = digit * 2;
                if doubled > 9 {
                    doubled - 9
                } else {
                    doubled
                }
            } else {
                digit
            }
        })
        .sum();
    sum.is_multiple_of(10)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(text: &str) -> Vec<DetectedKind> {
        detect(text).kinds.iter().collect()
    }

    #[test]
    fn detects_cards_codes_and_numbers() {
        assert!(kinds("card 4111 1111 1111 1111 exp").contains(&DetectedKind::PaymentCard));
        assert!(!kinds("4111 1111 1111 1112").contains(&DetectedKind::PaymentCard));
        assert!(!kinds("order 41111111111111119999").contains(&DetectedKind::PaymentCard));
        assert_eq!(
            kinds("123456"),
            vec![DetectedKind::Number, DetectedKind::OneTimeCode]
        );
        assert!(kinds("Your verification code is 482913").contains(&DetectedKind::OneTimeCode));
        assert!(kinds("1,234.50").contains(&DetectedKind::Number));
        assert!(!kinds("1,,2").contains(&DetectedKind::Number));
        assert_eq!(kinds("https://example.org/a"), vec![DetectedKind::Url]);
    }

    #[test]
    fn detection_is_bounded_and_reports_partial() {
        let text = "a".repeat(DETECTION_PREFIX_BYTES + 10);
        assert!(detect(&text).partial);
        let (head, cut) = prefix("éé", 3);
        assert_eq!((head, cut), ("é", true));
    }
}
