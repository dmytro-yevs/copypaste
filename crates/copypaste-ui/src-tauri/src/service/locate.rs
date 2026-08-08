//! Finding the daemon binary this app is allowed to start.
//!
//! Two sources, in order:
//!
//! 1. `COPYPASTE_DAEMON_BIN`, a development and test seam.
//! 2. A file named `copypaste-daemon` beside our own executable.
//!
//! **`PATH` is deliberately not searched.** A clipboard manager that starts the
//! first `copypaste-daemon` it can find on `PATH` will happily start one a
//! user's shell profile put there, and the process it starts holds the database
//! key. The bundle's own sibling is a path this app already trusts completely —
//! it is where its own executable lives — so using it introduces no new trust.
//! `scripts/release/build-macos-app.sh` injects and signs the binary at exactly
//! that location.

use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// The file name the release script injects into `Contents/MacOS`.
const DAEMON_FILE: &str = "copypaste-daemon";

/// Overrides the search. For `npm run tauri dev`, the demo scripts and tests,
/// none of which run from inside a bundle.
const ENV_OVERRIDE: &str = "COPYPASTE_DAEMON_BIN";

/// The daemon binary to start, or `None` when this build has none.
///
/// `None` is a real product state, not an error: a `cargo tauri dev` build and
/// a bundle whose injection step was skipped both reach it, and the UI has
/// copy for it.
pub fn daemon_binary() -> Option<PathBuf> {
    let exe = env::current_exe().ok();
    resolve(
        env::var_os(ENV_OVERRIDE),
        exe.as_deref().and_then(Path::parent),
    )
}

/// Both inputs are passed in so that nothing under test has to set the
/// override, which is process-global: cargo runs tests on parallel threads, and
/// the test that used `set_var` raced every other test reading the service
/// state, turning `NotInstalled` into `Stopped` about once in 300 runs (CI run
/// 31243843538).
fn resolve(explicit: Option<OsString>, beside: Option<&Path>) -> Option<PathBuf> {
    let candidate = match explicit {
        Some(path) => PathBuf::from(path),
        None => beside?.join(DAEMON_FILE),
    };
    candidate.is_file().then_some(candidate)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn the_override_resolves_only_a_real_file() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("nope");
        let present = dir.path().join(DAEMON_FILE);
        let as_dir = dir.path().join("daemon-dir");
        fs::write(&present, b"#!/bin/sh\n").unwrap();
        fs::create_dir(&as_dir).unwrap();

        assert_eq!(
            resolve(Some(missing.into()), None),
            None,
            "a missing override must not resolve"
        );
        // A stray `copypaste-daemon/` directory is not something to spawn.
        assert_eq!(
            resolve(Some(as_dir.into()), None),
            None,
            "a directory must not resolve"
        );
        assert_eq!(resolve(Some(present.clone().into()), None), Some(present));
    }

    #[test]
    fn the_sibling_resolves_only_once_the_binary_is_beside_us() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(resolve(None, Some(dir.path())), None);

        let daemon = dir.path().join(DAEMON_FILE);
        fs::write(&daemon, b"").unwrap();
        assert_eq!(resolve(None, Some(dir.path())), Some(daemon));
    }
}
