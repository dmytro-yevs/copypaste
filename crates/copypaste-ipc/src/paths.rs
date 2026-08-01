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

/// Database filename for this release line.
pub fn database_path() -> PathBuf {
    data_dir().join("copypaste-v2.db")
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
