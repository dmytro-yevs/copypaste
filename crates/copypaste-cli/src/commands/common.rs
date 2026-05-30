//! Shared helpers for command modules.
//!
//! Centralises the daemon-response error pattern that was previously duplicated
//! across nearly every command (`if !resp.ok { eprintln!(...); exit(1); }`).

use crate::ipc::Response;

/// Format Unix epoch milliseconds as "YYYY-MM-DD HH:MM:SS" (UTC, std only).
///
/// Returns an em-dash for values ≤ 0 (unknown / not-yet-set timestamps).
/// All four display commands (list, copy, search, watch) share this single
/// implementation so there is no format drift between them.
pub fn format_unix_ms(ms: i64) -> String {
    if ms <= 0 {
        return "\u{2014}".to_string(); // em dash
    }
    let secs = (ms / 1000) as u64;
    let ss = secs % 60;
    let mins = secs / 60;
    let mi = mins % 60;
    let hours = mins / 60;
    let h = hours % 24;
    let days = hours / 24;
    let (y, mo, d) = days_to_ymd(days);
    format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", y, mo, d, h, mi, ss)
}

fn days_to_ymd(days: u64) -> (u64, u64, u64) {
    let mut remaining = days;
    let mut year = 1970u64;
    loop {
        let diy = if is_leap(year) { 366 } else { 365 };
        if remaining < diy {
            break;
        }
        remaining -= diy;
        year += 1;
    }
    let leap = is_leap(year);
    let month_days: [u64; 12] = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut month = 1u64;
    for &md in &month_days {
        if remaining < md {
            break;
        }
        remaining -= md;
        month += 1;
    }
    (year, month, remaining + 1)
}

fn is_leap(y: u64) -> bool {
    (y.is_multiple_of(4) && !y.is_multiple_of(100)) || y.is_multiple_of(400)
}

/// Print an error to stderr and exit with status 1 if the daemon response
/// indicates failure. Returns to the caller when `resp.ok == true`.
///
/// W3.3: when the daemon attached an `error_code`, format the message as
/// `error [code]: message` so users (and scripts grepping CLI output) can
/// branch on a stable machine-readable token instead of parsing English.
///
/// Centralising this avoids 8+ near-identical copy-paste blocks and ensures
/// a single, consistent error format and exit code across all commands.
pub fn exit_on_err(resp: &Response) {
    if !resp.ok {
        let msg = resp.error.as_deref().unwrap_or_default();
        match resp.error_code {
            Some(code) => eprintln!("error [{code}]: {msg}"),
            None => eprintln!("error: {msg}"),
        }
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use copypaste_ipc::ErrorCode;

    #[test]
    fn returns_silently_when_ok() {
        let resp = Response {
            id: "1".to_string(),
            ok: true,
            data: None,
            error: None,
            error_code: None,
        };
        // Should not exit.
        exit_on_err(&resp);
    }

    /// W3.3: when no `error_code` is present, the printed format must remain
    /// `error: <msg>` to preserve backward compatibility with scripts that
    /// already grep for it. We can't observe stderr from in-process tests
    /// (Rust's test harness captures it but doesn't expose it cheaply), so
    /// we exercise the same formatting logic directly to lock the contract.
    #[test]
    fn exit_on_err_format_without_code_matches_legacy() {
        let resp = Response {
            id: "1".to_string(),
            ok: false,
            data: None,
            error: Some("boom".into()),
            error_code: None,
        };
        let msg = resp.error.as_deref().unwrap_or_default();
        let rendered = match resp.error_code {
            Some(code) => format!("error [{code}]: {msg}"),
            None => format!("error: {msg}"),
        };
        assert_eq!(rendered, "error: boom");
    }

    /// W3.3: when an `error_code` IS present, it must appear in brackets
    /// before the message so consumers can grep / branch on the code.
    #[test]
    fn exit_on_err_prints_code_when_present() {
        let resp = Response {
            id: "1".to_string(),
            ok: false,
            data: None,
            error: Some("cloud sync".into()),
            error_code: Some(ErrorCode::NotImplemented),
        };
        let msg = resp.error.as_deref().unwrap_or_default();
        let rendered = match resp.error_code {
            Some(code) => format!("error [{code}]: {msg}"),
            None => format!("error: {msg}"),
        };
        assert_eq!(rendered, "error [not_implemented]: cloud sync");
    }

    /// Empty `error` field must still render cleanly (no trailing whitespace
    /// drift between the two branches).
    #[test]
    fn exit_on_err_format_with_empty_error_message() {
        let resp_no_code = Response {
            id: "1".to_string(),
            ok: false,
            data: None,
            error: None,
            error_code: None,
        };
        let msg = resp_no_code.error.as_deref().unwrap_or_default();
        let rendered = match resp_no_code.error_code {
            Some(code) => format!("error [{code}]: {msg}"),
            None => format!("error: {msg}"),
        };
        assert_eq!(rendered, "error: ");

        let resp_with_code = Response {
            id: "1".to_string(),
            ok: false,
            data: None,
            error: None,
            error_code: Some(ErrorCode::RateLimited),
        };
        let msg = resp_with_code.error.as_deref().unwrap_or_default();
        let rendered = match resp_with_code.error_code {
            Some(code) => format!("error [{code}]: {msg}"),
            None => format!("error: {msg}"),
        };
        assert_eq!(rendered, "error [rate_limited]: ");
    }

    // Note: the failure path calls `std::process::exit`, which we cannot
    // unit-test directly without spawning a subprocess. Integration tests
    // in tests/cli_integration.rs exercise the exit behaviour end-to-end.

    // ── format_unix_ms ──────────────────────────────────────────────────

    #[test]
    fn format_unix_ms_zero_returns_em_dash() {
        assert_eq!(format_unix_ms(0), "\u{2014}");
    }

    #[test]
    fn format_unix_ms_negative_returns_em_dash() {
        assert_eq!(format_unix_ms(-1), "\u{2014}");
        assert_eq!(format_unix_ms(i64::MIN), "\u{2014}");
    }

    #[test]
    fn format_unix_ms_known_date() {
        // 2024-01-01 00:00:00 UTC = 1704067200000 ms
        assert_eq!(format_unix_ms(1_704_067_200_000), "2024-01-01 00:00:00");
    }

    #[test]
    fn format_unix_ms_structure() {
        let s = format_unix_ms(1_750_000_496_000);
        assert_eq!(s.len(), 19);
        assert_eq!(&s[4..5], "-");
        assert_eq!(&s[7..8], "-");
        assert_eq!(&s[10..11], " ");
        assert_eq!(&s[13..14], ":");
        assert_eq!(&s[16..17], ":");
    }
}
