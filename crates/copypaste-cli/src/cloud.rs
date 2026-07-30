//! Getting the two secrets `cloud sign-in` needs, without putting them in argv.
//!
//! **There is deliberately no `--password` flag.** A process' arguments are
//! readable by any process running as the same user (`/proc/<pid>/cmdline`),
//! they land in shell history, and they show up in `ps` output pasted into bug
//! reports. The password and the sync passphrase are read from the environment
//! or from stdin instead, which is the same trade `docker login --password-stdin`
//! and `gh auth login --with-token` make.
//!
//! Neither secret is echoed, stored by this process, or included in `--json`
//! output: they go into one request and the frame is dropped.

use crate::error::CliError;
use std::io::{IsTerminal, Read};

/// Account password. Checked first so a script can set both and pipe neither.
const ENV_PASSWORD: &str = "COPYPASTE_CLOUD_PASSWORD";
/// Sync passphrase — the one the backend must never be able to derive.
const ENV_PASSPHRASE: &str = "COPYPASTE_SYNC_PASSPHRASE";

const HOW_TO_SUPPLY: &str = "set COPYPASTE_CLOUD_PASSWORD and COPYPASTE_SYNC_PASSPHRASE, \
     or pipe them on stdin as two lines: password, then sync passphrase";

/// The password and the sync passphrase, in that order.
pub fn read_credentials() -> Result<(String, String), CliError> {
    let from_env = (std::env::var(ENV_PASSWORD), std::env::var(ENV_PASSPHRASE));
    if let (Ok(password), Ok(passphrase)) = from_env {
        return check(password, passphrase);
    }

    let stdin = std::io::stdin();
    if stdin.is_terminal() {
        // Reading an unechoed line from a terminal needs a dependency this CLI
        // does not carry, and blocking on a terminal read looks like a hang.
        // Saying how to supply them is more useful than either.
        return Err(CliError::local(format!("no credentials given: {HOW_TO_SUPPLY}")));
    }

    let mut buffer = String::new();
    stdin
        .lock()
        .read_to_string(&mut buffer)
        .map_err(|e| CliError::local(format!("could not read stdin: {e}")))?;

    let mut lines = buffer.lines();
    let password = lines.next().unwrap_or_default();
    let passphrase = lines.next().unwrap_or_default();
    check(password.to_string(), passphrase.to_string())
}

/// Both must be present. An empty passphrase would otherwise be sent, rejected
/// by the daemon's minimum-length check, and read as "the daemon is broken".
fn check(password: String, passphrase: String) -> Result<(String, String), CliError> {
    if password.is_empty() {
        return Err(CliError::local(format!("no password given: {HOW_TO_SUPPLY}")));
    }
    if passphrase.is_empty() {
        return Err(CliError::local(format!(
            "no sync passphrase given: {HOW_TO_SUPPLY}"
        )));
    }
    Ok((password, passphrase))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_secrets_are_required() {
        assert!(check("pw".into(), "phrase".into()).is_ok());
        assert!(check(String::new(), "phrase".into()).is_err());
        assert!(check("pw".into(), String::new()).is_err());
    }

    #[test]
    fn the_advice_names_both_ways_of_supplying_them_and_no_flag() {
        assert!(HOW_TO_SUPPLY.contains(ENV_PASSWORD));
        assert!(HOW_TO_SUPPLY.contains(ENV_PASSPHRASE));
        assert!(HOW_TO_SUPPLY.contains("stdin"));
        assert!(
            !HOW_TO_SUPPLY.contains("--password"),
            "a flag would put the secret in argv"
        );
    }

    #[test]
    fn an_error_never_contains_the_secret() {
        let err = check(String::new(), "hunter2".into()).unwrap_err();
        assert!(!err.user_message().contains("hunter2"));
    }
}
