//! Inline redaction of secrets in tool results.
//!
//! Spans are found with the same recognizers capture uses: the credential
//! classifier on each token-like run, private-key blocks, and the payment
//! card detector on digit runs. A short text that is a one-time code is
//! redacted whole. Each span becomes a marker that names only its kind.

use copypaste_core::rules::detect::{detect, DetectedKind};
use copypaste_ipc::content_type;

/// Opens the marker that replaces an inline secret; the kind name and `]`
/// follow, for example `[redacted: api key]`.
pub(super) const MARKER_PREFIX: &str = "[redacted: ";
const PRIVATE_KEY_BEGIN: &str = "-----BEGIN ";
const PRIVATE_KEY_HEADER_END: &str = "PRIVATE KEY-----";
const PRIVATE_KEY_END: &str = "-----END ";
/// Shortest run the credential classifier accepts.
const MIN_TOKEN_BYTES: usize = 20;
/// One-time codes are recognized only in short texts, as the detector does.
const MAX_CODE_TEXT_CHARS: usize = 200;

pub(super) struct Redacted {
    pub(super) text: String,
    /// Characters of the original text that markers replaced.
    pub(super) redacted_chars: usize,
    pub(super) total_chars: usize,
}

impl Redacted {
    /// More than half the text was secret: the whole item is treated as one.
    pub(super) fn mostly_secret(&self) -> bool {
        self.redacted_chars * 2 > self.total_chars
    }
}

fn token_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.')
}

fn private_keys(text: &str, spans: &mut Vec<(usize, usize, &'static str)>) {
    let mut from = 0;
    while let Some(found) = text[from..].find(PRIVATE_KEY_BEGIN) {
        let start = from + found;
        let line_end = text[start..].find('\n').map_or(text.len(), |at| start + at);
        if !text[start..line_end].contains(PRIVATE_KEY_HEADER_END) {
            from = start + PRIVATE_KEY_BEGIN.len();
            continue;
        }
        let end = text[line_end..]
            .find(PRIVATE_KEY_END)
            .map(|at| line_end + at)
            .and_then(|footer| {
                text[footer..]
                    .find(PRIVATE_KEY_HEADER_END)
                    .map(|at| footer + at + PRIVATE_KEY_HEADER_END.len())
            })
            .unwrap_or(text.len());
        spans.push((start, end, "private key"));
        from = end;
    }
}

fn tokens(text: &str, spans: &mut Vec<(usize, usize, &'static str)>) {
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if !token_byte(bytes[index]) {
            index += 1;
            continue;
        }
        let start = index;
        while index < bytes.len() && token_byte(bytes[index]) {
            index += 1;
        }
        let run = &text[start..index];
        if run.len() >= MIN_TOKEN_BYTES
            && copypaste_core::classify_credentials(content_type::TEXT, run)
                .is_some_and(|privacy| privacy.secret)
        {
            spans.push((start, index, "token"));
        }
    }
}

/// Digit runs of 13 to 19 digits, single spaces or hyphens allowed between
/// digits, that the detector accepts as a card number.
fn payment_cards(text: &str, spans: &mut Vec<(usize, usize, &'static str)>) {
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        let boundary = index == 0 || !bytes[index - 1].is_ascii_alphanumeric();
        if !bytes[index].is_ascii_digit() || !boundary {
            index += 1;
            continue;
        }
        let start = index;
        let mut end = index;
        let mut digits = 0;
        while end < bytes.len() {
            if bytes[end].is_ascii_digit() {
                digits += 1;
                end += 1;
            } else if matches!(bytes[end], b' ' | b'-')
                && bytes.get(end + 1).is_some_and(u8::is_ascii_digit)
            {
                end += 1;
            } else {
                break;
            }
        }
        if (13..=19).contains(&digits)
            && detect(&text[start..end])
                .kinds
                .contains(DetectedKind::PaymentCard)
        {
            spans.push((start, end, "payment card"));
        }
        index = end.max(start + 1);
    }
}

/// `text` with every recognized secret replaced by its marker.
pub(super) fn redact(text: &str) -> Redacted {
    let total_chars = text.chars().count();
    if total_chars <= MAX_CODE_TEXT_CHARS && detect(text).kinds.contains(DetectedKind::OneTimeCode)
    {
        return Redacted {
            text: format!("{MARKER_PREFIX}one-time code]"),
            redacted_chars: total_chars,
            total_chars,
        };
    }
    let mut spans = Vec::new();
    private_keys(text, &mut spans);
    tokens(text, &mut spans);
    payment_cards(text, &mut spans);
    spans.sort_unstable();
    let mut output = String::with_capacity(text.len());
    let mut redacted_chars = 0;
    let mut cursor = 0;
    for (start, end, kind) in spans {
        if start < cursor {
            continue;
        }
        output.push_str(&text[cursor..start]);
        output.push_str(MARKER_PREFIX);
        output.push_str(kind);
        output.push(']');
        redacted_chars += text[start..end].chars().count();
        cursor = end;
    }
    output.push_str(&text[cursor..]);
    Redacted {
        text: output,
        redacted_chars,
        total_chars,
    }
}

/// At most `max_chars` of `text`. When `text` was itself cut short, a
/// token-like run touching the end is dropped, since a secret split by the
/// cut is no longer recognizable.
pub(super) fn bounded(text: &str, max_chars: usize, cut_before: bool) -> (String, bool) {
    let mut end = text
        .char_indices()
        .nth(max_chars)
        .map_or(text.len(), |(at, _)| at);
    let truncated = cut_before || end < text.len();
    if truncated {
        let bytes = text.as_bytes();
        while end > 0 && token_byte(bytes[end - 1]) {
            end -= 1;
        }
    }
    (text[..end].to_owned(), truncated)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inline_secrets_become_kind_markers_and_mostly_secret_text_is_flagged() {
        let token = format!("ghp_{}", "Ab3d".repeat(10));
        let card = "4111 1111 1111 1111";
        let words = "remember to rotate the deploy credentials before the release";
        let text = format!("{words}: deploy with {token} and pay with {card} today");
        let redacted = redact(&text);
        assert_eq!(
            redacted.text,
            format!("{words}: deploy with [redacted: token] and pay with [redacted: payment card] today")
        );
        assert!(!redacted.mostly_secret());
        assert!(redact(&format!("key: {token}")).mostly_secret());

        let key = format!(
            "-----BEGIN {0} PRIVATE KEY-----\nAAAA\n-----END {0} PRIVATE KEY-----",
            "OPENSSH"
        );
        assert_eq!(
            redact(&format!("a\n{key}\nb")).text,
            "a\n[redacted: private key]\nb"
        );
        assert!(redact("Your code is 482913").mostly_secret());
        assert_eq!(redact("plain words 2026").text, "plain words 2026");
    }

    #[test]
    fn a_bounded_preview_drops_a_run_cut_at_its_end() {
        assert_eq!(bounded("abc def", 5, false), ("abc ".into(), true));
        assert_eq!(bounded("abc def", 50, true), ("abc ".into(), true));
        assert_eq!(bounded("abc def", 50, false), ("abc def".into(), false));
    }
}
