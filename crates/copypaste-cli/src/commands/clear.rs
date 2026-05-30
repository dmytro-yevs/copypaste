use anyhow::Result;
use std::path::Path;

use crate::commands::common::exit_on_err;
use crate::ipc::IpcClient;

/// Exit code used when the user declines the `clear` confirmation prompt.
///
/// Distinct from `1` (the generic CLI error / daemon-failure code emitted by
/// `main.rs` and `exit_on_err`) and from `0` (success). A separate code means
/// `copypaste clear && next` does NOT run `next` on abort, and scripts can tell
/// "user said no" apart from "the clear actually failed".
pub const ABORT_EXIT_CODE: i32 = 2;

/// Returns `true` only when the typed confirmation is exactly "yes"
/// (whitespace-trimmed). Pure so the abort decision is unit-testable without
/// touching stdin.
fn is_confirmed(input: &str) -> bool {
    input.trim() == "yes"
}

pub fn run(socket_path: &Path, force: bool) -> Result<()> {
    if !force {
        eprint!("This will delete ALL clipboard history. Type 'yes' to confirm: ");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        if !is_confirmed(&input) {
            // User declined. Exit with a distinct non-zero code so chained
            // commands (`copypaste clear && next`) don't proceed as if the
            // history were cleared. We exit directly (rather than returning
            // Err, which main.rs maps to code 1) to keep the abort code stable.
            eprintln!("aborted.");
            std::process::exit(ABORT_EXIT_CODE);
        }
    }

    let mut client = IpcClient::connect(socket_path)?;
    let req = serde_json::json!({"id": "1", "method": "delete_all"});
    let resp = client.call(&req)?;
    exit_on_err(&resp);

    let deleted = resp
        .data
        .as_ref()
        .and_then(|d| d["deleted"].as_i64())
        .unwrap_or(0);
    println!("cleared {deleted} items");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_signature_compiles() {
        let _: fn(&Path, bool) -> Result<()> = run;
    }

    /// Only an exact "yes" (after trimming) confirms. Anything else aborts —
    /// which the production path turns into a non-zero exit.
    #[test]
    fn is_confirmed_requires_exact_yes() {
        assert!(is_confirmed("yes"));
        assert!(is_confirmed("yes\n"));
        assert!(is_confirmed("  yes  "));
        assert!(!is_confirmed("y"));
        assert!(!is_confirmed("YES"));
        assert!(!is_confirmed("no"));
        assert!(!is_confirmed(""));
        assert!(!is_confirmed("\n"));
        assert!(!is_confirmed("yes please"));
    }

    /// The abort code must be non-zero and distinct from the generic error
    /// code (1) so chained commands stop and scripts can branch on it.
    #[test]
    fn abort_exit_code_is_distinct_nonzero() {
        assert_ne!(ABORT_EXIT_CODE, 0);
        assert_ne!(ABORT_EXIT_CODE, 1);
    }
}
