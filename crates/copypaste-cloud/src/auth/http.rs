//! The pieces this module shares with [`crate::rest`].
//!
//! Every item here exists to be the *only* implementation of something v1 had
//! several of: one retry policy where v1 had six hand-rolled backoffs, one
//! `Retry-After` reader, one email mask, one clock. They sit together so that a
//! second one would be conspicuous rather than plausible (`CLAUDE.md` rule 1).
//!
//! `rest` reaches in here rather than owning a copy. That direction is
//! deliberate: `auth` is the module that must exist for `rest` to have a bearer
//! at all, so the shared floor belongs on this side of the edge and there is no
//! cycle.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use backon::ExponentialBuilder;
use reqwest::header::HeaderMap;
use serde::Deserialize;

/// Longest error body we will put in a log line.
const MAX_ERROR_SNIPPET: usize = 200;

/// Retry envelope for transient failures: the first attempt, then ~1 s, 2 s and
/// 4 s — the manifest's hard cap of four attempts (§4.6.3).
const RETRY_INITIAL: Duration = Duration::from_secs(1);
const RETRY_MAX_INTERVAL: Duration = Duration::from_secs(30);
const RETRY_MAX_RETRIES: usize = 3;

/// The one retry policy in this crate.
///
/// `rest` uses it too, and [`crate::sync`] deliberately does not add a second
/// ladder on top of it — a transient failure that reaches the sync driver has
/// already spent this budget.
///
/// Jitter is on so that every device of a large account does not retry in
/// lockstep after a backend blip.
pub fn transient_backoff() -> ExponentialBuilder {
    ExponentialBuilder::new()
        .with_min_delay(RETRY_INITIAL)
        .with_max_delay(RETRY_MAX_INTERVAL)
        .with_max_times(RETRY_MAX_RETRIES)
        .with_jitter()
}

/// `Retry-After`, delta-seconds form only.
///
/// The HTTP-date form is deliberately unsupported: Supabase emits integer
/// seconds, and a date parser here would be a second time-handling code path
/// for no behaviour (manifest §4.6.3).
pub(crate) fn retry_after_secs(headers: &HeaderMap) -> Option<u64> {
    headers
        .get(reqwest::header::RETRY_AFTER)?
        .to_str()
        .ok()?
        .trim()
        .parse::<u64>()
        .ok()
}

/// Mask an email for logging: `alice@example.com` -> `a***@example.com`.
///
/// Anything without a usable `@` collapses to `<redacted>` rather than being
/// passed through, so a malformed input cannot become a log-line disclosure.
pub(crate) fn redact_email(email: &str) -> String {
    let trimmed = email.trim();
    let Some((local, domain)) = trimmed.split_once('@') else {
        return "<redacted>".to_string();
    };
    if local.is_empty() || domain.is_empty() || domain.contains('@') {
        return "<redacted>".to_string();
    }
    let first = local.chars().next().expect("local part is non-empty");
    format!("{first}***@{domain}")
}

/// A short, diagnosable description of an error body — for logs only.
///
/// Tries the structured GoTrue envelope first; falls back to a truncated raw
/// snippet so a 502 HTML gateway page reads as something rather than as
/// "unknown error" (manifest AT-38).
pub(super) fn error_detail(body: &str) -> String {
    #[derive(Deserialize)]
    struct GoTrueError {
        error: Option<String>,
        error_description: Option<String>,
        error_code: Option<String>,
        msg: Option<String>,
        message: Option<String>,
    }

    if let Ok(envelope) = serde_json::from_str::<GoTrueError>(body) {
        let message = envelope
            .error_description
            .or(envelope.message)
            .or(envelope.msg)
            .or(envelope.error)
            .or(envelope.error_code);
        if let Some(message) = message {
            if !message.trim().is_empty() {
                return truncate(message.trim());
            }
        }
    }

    let raw = body.trim();
    if raw.is_empty() {
        "<empty body>".to_string()
    } else {
        truncate(raw)
    }
}

/// Truncate to [`MAX_ERROR_SNIPPET`] bytes, on a char boundary.
fn truncate(text: &str) -> String {
    if text.len() <= MAX_ERROR_SNIPPET {
        return text.to_string();
    }
    let mut end = MAX_ERROR_SNIPPET;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &text[..end])
}

/// Wall clock in milliseconds. Saturating, so a clock before the epoch gives 0
/// rather than a negative expiry.
///
/// The whole crate reads the clock through this: the session expiry here, the
/// tombstone restamp in [`crate::rest`], and the future-skew refusal in
/// [`crate::sync`]. One reader means one saturating rule.
pub(crate) fn now_ms() -> i64 {
    unix_millis(SystemTime::now())
}

fn unix_millis(time: SystemTime) -> i64 {
    time.duration_since(UNIX_EPOCH)
        .map(saturating_millis)
        .unwrap_or(0)
}

fn saturating_millis(since_epoch: Duration) -> i64 {
    i64::try_from(since_epoch.as_millis()).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use super::super::testkit::INVALID_GRANT;
    use super::*;

    #[test]
    fn retry_after_parses_delta_seconds_and_ignores_the_http_date_form() {
        let mut headers = HeaderMap::new();
        headers.insert("retry-after", "30".parse().unwrap());
        assert_eq!(retry_after_secs(&headers), Some(30));

        headers.insert("retry-after", " 7 ".parse().unwrap());
        assert_eq!(retry_after_secs(&headers), Some(7));

        // Deliberately unsupported: Supabase emits integer seconds.
        headers.insert(
            "retry-after",
            "Wed, 21 Oct 2026 07:28:00 GMT".parse().unwrap(),
        );
        assert_eq!(retry_after_secs(&headers), None);

        headers.insert("retry-after", "-5".parse().unwrap());
        assert_eq!(retry_after_secs(&headers), None);

        assert_eq!(retry_after_secs(&HeaderMap::new()), None);
    }

    #[test]
    fn emails_are_masked_for_logs() {
        assert_eq!(redact_email("alice@example.com"), "a***@example.com");
        assert_eq!(
            redact_email("  bob@sub.example.org "),
            "b***@sub.example.org"
        );
        assert_eq!(redact_email("not-an-email"), "<redacted>");
        assert_eq!(redact_email("@example.com"), "<redacted>");
        assert_eq!(redact_email("alice@"), "<redacted>");
        assert_eq!(redact_email("a@b@c"), "<redacted>");
        assert_eq!(redact_email(""), "<redacted>");
    }

    // -- error bodies -------------------------------------------------------

    #[test]
    fn a_structured_gotrue_error_reads_as_its_description() {
        assert_eq!(error_detail(INVALID_GRANT), "Invalid login credentials");
        assert_eq!(
            error_detail(r#"{"msg":"User already registered"}"#),
            "User already registered"
        );
        assert_eq!(
            error_detail(r#"{"error":"invalid_grant"}"#),
            "invalid_grant"
        );
    }

    #[test]
    fn a_gateway_html_page_is_truncated_rather_than_lost() {
        let page = format!("<html>{}</html>", "x".repeat(4000));
        let detail = error_detail(&page);
        assert!(
            detail.len() <= MAX_ERROR_SNIPPET + 4,
            "not truncated: {}",
            detail.len()
        );
        assert!(
            detail.starts_with("<html>"),
            "the snippet should be diagnosable"
        );
        assert!(detail.ends_with('…'));
    }

    #[test]
    fn an_empty_error_body_says_so() {
        assert_eq!(error_detail("   "), "<empty body>");
        // A JSON envelope with nothing usable in it falls back to the raw
        // body: "{}" is uninformative, but it is what the server actually
        // said, and inventing a friendlier string would hide that.
        assert_eq!(error_detail("{}"), "{}");
    }

    #[test]
    fn truncation_lands_on_a_char_boundary() {
        let text = "é".repeat(MAX_ERROR_SNIPPET);
        let detail = truncate(&text);
        assert!(detail.ends_with('…'));
        assert!(detail.is_char_boundary(detail.len() - '…'.len_utf8()));
    }

    #[test]
    fn cloud_wall_time_saturates_at_the_wire_integer_limit() {
        // Asserted on the duration, not on `UNIX_EPOCH + d`: a Windows
        // SystemTime is a FILETIME and cannot represent a date 2.9e8 years
        // out, so building that instant panics before unix_millis is reached.
        let overflow = Duration::from_millis(i64::MAX as u64 + 1);
        assert_eq!(saturating_millis(overflow), i64::MAX);
        assert_eq!(unix_millis(UNIX_EPOCH - Duration::from_millis(1)), 0);
    }
}
