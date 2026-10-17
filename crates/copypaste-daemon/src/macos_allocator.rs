//! Select the measured libmalloc policy before starting runtime or key access.

use std::ffi::{OsStr, OsString};
use std::path::Path;
use std::process::Command;

use anyhow::Context;
use std::os::unix::process::CommandExt;

const LARGE_CACHE: &str = "MallocLargeCache";

pub(crate) fn configure_before_runtime() -> anyhow::Result<()> {
    if !needs_reexec(std::env::var_os(LARGE_CACHE).as_deref()) {
        return Ok(());
    }
    // libmalloc reads this policy before main. Setting it after startup does
    // not release the cached RGBA buffers. exec replaces this same process:
    // PID, inherited stdio and the app-parent lifeline remain intact.
    let executable = std::env::current_exe().context("locate daemon for allocator policy")?;
    let error = configured_command(&executable, std::env::args_os().skip(1)).exec();
    Err(error).context("restart daemon with bounded allocator retention")
}

fn needs_reexec(value: Option<&OsStr>) -> bool {
    value != Some(OsStr::new("0"))
}

fn configured_command(executable: &Path, arguments: impl IntoIterator<Item = OsString>) -> Command {
    let mut command = Command::new(executable);
    command.args(arguments).env(LARGE_CACHE, "0");
    command
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reexec_stops_once_the_startup_policy_is_present() {
        assert!(needs_reexec(None));
        assert!(needs_reexec(Some(OsStr::new("1"))));
        assert!(!needs_reexec(Some(OsStr::new("0"))));
    }

    #[test]
    fn reexec_preserves_runtime_and_worker_arguments() {
        for arguments in [
            vec![
                "--data-dir",
                "/isolated/dev state",
                "--foreground",
                "--app-parent",
            ],
            vec!["--inference-worker"],
        ] {
            let arguments: Vec<OsString> = arguments.into_iter().map(OsString::from).collect();
            let command =
                configured_command(Path::new("/bundle/copypaste-daemon"), arguments.clone());
            assert_eq!(
                command.get_program(),
                OsStr::new("/bundle/copypaste-daemon")
            );
            assert_eq!(
                command.get_args().collect::<Vec<_>>(),
                arguments
                    .iter()
                    .map(OsString::as_os_str)
                    .collect::<Vec<_>>()
            );
            assert_eq!(
                command.get_envs().collect::<Vec<_>>(),
                vec![(OsStr::new(LARGE_CACHE), Some(OsStr::new("0")))]
            );
        }
    }
}
