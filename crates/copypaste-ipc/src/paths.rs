//! Where things live on disk.
//!
//! One resolver per path, in one place: v1 duplicated the socket path in three
//! files and its own module doc admitted it.

use std::path::{Path, PathBuf};

const SOCKET_ENV: &str = "COPYPASTE_SOCKET";

/// Where the daemon socket lives.
///
/// One definition, used by the daemon and the CLI. v1 duplicated this logic in
/// three places and the module doc admitted it.
pub fn socket_path() -> PathBuf {
    socket_path_for_data_dir(None)
}

/// Resolve the daemon socket while optionally relocating its default directory.
///
/// `COPYPASTE_SOCKET` is the canonical explicit override and therefore wins
/// over `data_dir`. Its value is an OS string and is used verbatim, including
/// an empty value; validation remains the operating system's so resolving a
/// not-yet-created socket never needs to touch the filesystem.
pub fn socket_path_for_data_dir(relocated_data_dir: Option<&Path>) -> PathBuf {
    std::env::var_os(SOCKET_ENV).map_or_else(
        || {
            relocated_data_dir
                .map(Path::to_path_buf)
                .unwrap_or_else(data_dir)
                .join("daemon.sock")
        },
        PathBuf::from,
    )
}

/// v2 database filename. Deliberately distinct from v1's, so an existing v0.4.x
/// database is never opened, modified, or reported as corrupt — see CLAUDE.md
/// rule 3.
pub fn database_path() -> PathBuf {
    data_dir().join("copypaste-v2.db")
}

/// Where CopyPaste 0.4 kept its data, so a v0.4 history can be *looked for*.
///
/// A second resolver rather than a variant of [`data_dir`], because the two
/// genuinely differ and that difference is deliberate: v0.4.x built
/// `~/Library/Application Support/CopyPaste` by hand (port manifest 04 §3.1),
/// while [`data_dir`]'s `ProjectDirs::from("com", "copypaste", "CopyPaste")`
/// resolves `~/Library/Application Support/com.copypaste.CopyPaste`. Never
/// touching the old file is half of CLAUDE.md rule 3; knowing where it is, so a
/// v2 build can say it found one, is the other half.
///
/// `copypaste_core::v1_database_in` takes a directory rather than resolving one
/// precisely so that this decision lives here, with the rest of path
/// resolution, instead of being made twice.
///
/// **Read-only, by contract.** Nothing may open, create or write anything under
/// the returned path: a user who downgrades must find their history exactly as
/// they left it.
///
/// `None` when there is no home directory to resolve from. Android is not
/// covered — v0.4's Android history lived in the app's own sandbox, which this
/// process cannot address by a user-level path.
#[must_use]
pub fn v1_data_dir() -> Option<PathBuf> {
    let home = directories::BaseDirs::new()?.home_dir().to_path_buf();
    #[cfg(target_os = "macos")]
    {
        Some(
            home.join("Library")
                .join("Application Support")
                .join("CopyPaste"),
        )
    }
    #[cfg(not(target_os = "macos"))]
    {
        // Linux and the rest of Unix, which is where the tests run. v0.4.x used
        // `$XDG_DATA_HOME/copypaste`, and so does `ProjectDirs` here — the two
        // coincide, and the distinct *filename* is what keeps them apart.
        Some(
            std::env::var_os("XDG_DATA_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".local").join("share"))
                .join("copypaste"),
        )
    }
}

pub fn data_dir() -> PathBuf {
    directories::ProjectDirs::from("com", "copypaste", "CopyPaste")
        .map(|d| d.data_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from(".copypaste"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;
    #[cfg(unix)]
    use std::ffi::OsString;
    #[cfg(unix)]
    use std::os::unix::ffi::OsStringExt;
    use std::process::Command;

    const CHILD_CASE: &str = "COPYPASTE_PATH_TEST_CASE";
    const TEST_NAME: &str = "paths::tests::socket_environment_precedence_is_process_isolated";

    fn run_case(case: &str, socket: Option<&OsStr>) {
        let mut child = Command::new(std::env::current_exe().expect("current test binary"));
        child
            .args(["--exact", TEST_NAME])
            .env(CHILD_CASE, case)
            .env_remove(SOCKET_ENV);
        if let Some(socket) = socket {
            child.env(SOCKET_ENV, socket);
        }
        assert!(child.status().expect("run isolated path test").success());
    }

    #[test]
    fn socket_environment_precedence_is_process_isolated() {
        if let Some(case) = std::env::var_os(CHILD_CASE) {
            let isolated = Path::new("/isolated/copypaste");
            match case.to_str().expect("ASCII test case") {
                "override" => assert_eq!(
                    socket_path_for_data_dir(Some(isolated)),
                    Path::new("/explicit/copypaste.sock")
                ),
                "empty" => assert_eq!(socket_path_for_data_dir(Some(isolated)), PathBuf::new()),
                "data-dir" => assert_eq!(
                    socket_path_for_data_dir(Some(isolated)),
                    isolated.join("daemon.sock")
                ),
                "default" => assert_eq!(socket_path(), data_dir().join("daemon.sock")),
                #[cfg(unix)]
                "non-utf8" => assert_eq!(
                    socket_path(),
                    PathBuf::from(OsString::from_vec(b"/tmp/copypaste-\xff.sock".to_vec()))
                ),
                other => panic!("unknown child case: {other}"),
            }
            return;
        }

        for (case, socket) in [
            ("override", Some(OsStr::new("/explicit/copypaste.sock"))),
            ("empty", Some(OsStr::new(""))),
            ("data-dir", None),
            ("default", None),
        ] {
            run_case(case, socket);
        }
        #[cfg(unix)]
        {
            let non_utf8 = OsString::from_vec(b"/tmp/copypaste-\xff.sock".to_vec());
            run_case("non-utf8", Some(&non_utf8));
        }
    }
}
