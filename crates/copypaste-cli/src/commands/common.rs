//! Shared helpers for command modules.
//!
//! Centralises the daemon-response error pattern that was previously duplicated
//! across nearly every command (`if !resp.ok { eprintln!(...); exit(1); }`).

use crate::ipc::Response;

/// Print `error: <msg>` to stderr and exit with status 1 if the daemon
/// response indicates failure. Returns to the caller when `resp.ok == true`.
///
/// Centralising this avoids 8+ near-identical copy-paste blocks and ensures
/// a single, consistent error format and exit code across all commands.
pub fn exit_on_err(resp: &Response) {
    if !resp.ok {
        let msg = resp.error.as_deref().unwrap_or_default();
        eprintln!("error: {msg}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_silently_when_ok() {
        let resp = Response {
            id: "1".to_string(),
            ok: true,
            data: None,
            error: None,
        };
        // Should not exit.
        exit_on_err(&resp);
    }

    // Note: the failure path calls `std::process::exit`, which we cannot
    // unit-test directly without spawning a subprocess. Integration tests
    // in tests/cli_integration.rs exercise the exit behaviour end-to-end.
}
