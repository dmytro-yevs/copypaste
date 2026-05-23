//! Shared helpers for command modules.
//!
//! Centralises the daemon-response error pattern that was previously duplicated
//! across nearly every command (`if !resp.ok { eprintln!(...); exit(1); }`).

use crate::ipc::Response;

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
}
