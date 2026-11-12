//! Cheap content detectors behind the palette's "Suggested" row. They run
//! only when a palette opens with a source, look at the first
//! [`PREVIEW_BYTES`] once, never decode speculatively in layers, and suggest
//! only transforms that are registered.

use base64::alphabet;
use base64::engine::general_purpose::GeneralPurpose;
use base64::engine::{DecodePaddingMode, GeneralPurposeConfig};
use base64::Engine;
use serde::{Deserialize, Serialize};

use crate::catalogue::{apply, catalogue, Context, Step};
use crate::counts::preview_prefix;
use crate::PREVIEW_BYTES;

/// Most suggestions returned.
pub const MAX_SUGGESTIONS: usize = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Detector {
    Jwt,
    Timestamp,
    Json,
    Base64,
    TrackingUrl,
    Html,
    Markdown,
}

impl Detector {
    /// Every detector, in priority order.
    pub const ALL: [Self; 7] = [
        Self::Jwt,
        Self::Timestamp,
        Self::Json,
        Self::Base64,
        Self::TrackingUrl,
        Self::Html,
        Self::Markdown,
    ];

    /// The transform this detector suggests.
    pub fn transform_id(self) -> &'static str {
        match self {
            Self::Jwt => "dev.jwt_decode",
            Self::Timestamp => "dev.timestamp_to_date",
            Self::Json => "structure.json_format",
            Self::Base64 => "encode.base64_decode",
            Self::TrackingUrl => "url.remove_tracking",
            Self::Html => "markup.html_to_markdown",
            Self::Markdown => "markup.markdown_to_plain",
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct DetectOptions {
    /// Detectors the user turned off.
    pub disabled: Vec<Detector>,
    /// The source carries an HTML flavor.
    pub has_html: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Suggestion {
    pub detector: Detector,
    pub transform_id: &'static str,
}

/// Up to [`MAX_SUGGESTIONS`] transforms that fit `input`, best first.
pub fn detect(input: &str, options: &DetectOptions) -> Vec<Suggestion> {
    let text = preview_prefix(input);
    let trimmed = text.trim();
    Detector::ALL
        .into_iter()
        .filter(|d| !options.disabled.contains(d))
        .filter(|d| catalogue().get(d.transform_id()).is_some())
        .filter(|d| matches(*d, text, trimmed, options))
        .take(MAX_SUGGESTIONS)
        .map(|detector| Suggestion {
            detector,
            transform_id: detector.transform_id(),
        })
        .collect()
}

fn matches(detector: Detector, text: &str, trimmed: &str, options: &DetectOptions) -> bool {
    match detector {
        Detector::Jwt => is_jwt(trimmed),
        Detector::Timestamp => is_timestamp(trimmed),
        Detector::Json => is_json(trimmed),
        Detector::Base64 => is_base64_text(trimmed),
        Detector::TrackingUrl => has_tracking(text),
        Detector::Html => options.has_html,
        Detector::Markdown => has_markdown(text),
    }
}

/// Standard or URL-safe alphabet; padding optional.
fn lenient(alphabet: &alphabet::Alphabet) -> GeneralPurpose {
    GeneralPurpose::new(
        alphabet,
        GeneralPurposeConfig::new()
            .with_decode_padding_mode(DecodePaddingMode::Indifferent)
            .with_decode_allow_trailing_bits(true),
    )
}

fn decode_base64(text: &str) -> Option<Vec<u8>> {
    let url_safe = text.contains(['-', '_']);
    let alphabet = if url_safe {
        &alphabet::URL_SAFE
    } else {
        &alphabet::STANDARD
    };
    lenient(alphabet).decode(text).ok()
}

fn is_jwt(text: &str) -> bool {
    let mut parts = text.split('.');
    let (Some(header), Some(payload), Some(_), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return false;
    };
    let base64url = |s: &str| {
        !s.is_empty()
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    };
    if !base64url(header) || !base64url(payload) {
        return false;
    }
    lenient(&alphabet::URL_SAFE)
        .decode(header)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .is_some_and(|value| value.get("alg").is_some())
}

/// Seconds, milliseconds, microseconds or nanoseconds since the epoch, by
/// digit count, landing between 1970-01-01 and 2100-12-31.
fn is_timestamp(text: &str) -> bool {
    const LAST_SECOND: u64 = 4_133_980_799;
    if !text.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    let divisor: u64 = match text.len() {
        9 | 10 => 1,
        12 | 13 => 1_000,
        15 | 16 => 1_000_000,
        18 | 19 => 1_000_000_000,
        _ => return false,
    };
    text.parse::<u64>()
        .is_ok_and(|value| value / divisor <= LAST_SECOND)
}

fn is_json(text: &str) -> bool {
    text.starts_with(['{', '[']) && serde_json::from_str::<serde::de::IgnoredAny>(text).is_ok()
}

/// A run of Base64 (lines allowed) that decodes to readable UTF-8. Short
/// runs are skipped because ordinary words fit the alphabet.
fn is_base64_text(text: &str) -> bool {
    let compact: String = text.split_ascii_whitespace().collect();
    if compact.len() < 12 || compact.len() > PREVIEW_BYTES {
        return false;
    }
    let alphabet_only = compact
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'-' | b'_' | b'='));
    alphabet_only
        && decode_base64(&compact)
            .and_then(|bytes| String::from_utf8(bytes).ok())
            .is_some_and(|decoded| {
                !decoded.is_empty()
                    && decoded
                        .chars()
                        .all(|c| !c.is_control() || c.is_ascii_whitespace())
            })
}

/// An http(s) URL that the tracking cleaner would change.
fn has_tracking(text: &str) -> bool {
    if !(text.contains("http://") || text.contains("https://")) || !text.contains('?') {
        return false;
    }
    let Some(step) = Step::current(Detector::TrackingUrl.transform_id()) else {
        return false;
    };
    apply(&step, text, &Context::new()).is_ok_and(|output| output.text != text)
}

/// A heading, a code fence, an inline link or strong emphasis.
fn has_markdown(text: &str) -> bool {
    let block = text.lines().any(|line| {
        let line = line.trim_start();
        let hashes = line.bytes().take_while(|&b| b == b'#').count();
        ((1..=6).contains(&hashes) && line[hashes..].starts_with(' ')) || line.starts_with("```")
    });
    block || text.contains("](http") || has_strong(text)
}

/// Text between a pair of `**` that hugs its content (`**bold**`).
fn has_strong(text: &str) -> bool {
    let parts: Vec<&str> = text.split("**").collect();
    (1..parts.len().saturating_sub(1)).step_by(2).any(|i| {
        let inner = parts[i];
        !inner.is_empty()
            && !inner.starts_with(char::is_whitespace)
            && !inner.ends_with(char::is_whitespace)
            && !inner.contains('\n')
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detectors_recognize_their_shapes() {
        let jwt = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxIn0.c2ln";
        assert!(is_jwt(jwt));
        assert!(!is_jwt("a.b.c"));
        assert!(is_timestamp("1700000000"));
        assert!(is_timestamp("1700000000000"));
        assert!(!is_timestamp("99999999999"));
        assert!(!is_timestamp("9999999999999"));
        assert!(is_json("{\"a\": [1, 2]}"));
        assert!(!is_json("{\"a\": 1,}"));
        assert!(is_base64_text("aGVsbG8gd29ybGQ"));
        assert!(!is_base64_text("helloworld"));
        assert!(!is_base64_text("/9j/4AAQSkZJRgABAQ"));
        assert!(has_markdown("# Title\n\nbody"));
        assert!(has_markdown("see **this** now"));
        assert!(!has_markdown("2 ** 3 and 4 ** 5"));
        assert!(!has_markdown("#hashtag"));
    }

    #[test]
    fn suggestions_skip_unregistered_and_disabled_transforms() {
        let options = DetectOptions {
            disabled: vec![Detector::Json],
            has_html: true,
        };
        let registered = |d: Detector| catalogue().get(d.transform_id()).is_some();
        let found = detect("{\"a\":1}", &options);
        assert!(found.iter().all(|s| s.detector != Detector::Json));
        assert!(found.iter().all(|s| registered(s.detector)));
        assert!(found.len() <= MAX_SUGGESTIONS);
    }
}
