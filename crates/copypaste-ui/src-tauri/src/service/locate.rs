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
use std::path::PathBuf;

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
    if let Some(explicit) = env::var_os(ENV_OVERRIDE) {
        let path = PathBuf::from(explicit);
        return path.is_file().then_some(path);
    }
    let sibling = env::current_exe().ok()?.parent()?.join(DAEMON_FILE);
    sibling.is_file().then_some(sibling)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// One test, not three, because the override is process-global and cargo
    /// runs tests on parallel threads: three tests setting the same variable
    /// would race each other rather than assert anything.
    #[test]
    fn the_override_resolves_only_a_real_file() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("nope");
        let present = dir.path().join(DAEMON_FILE);
        let as_dir = dir.path().join("daemon-dir");
        fs::write(&present, b"#!/bin/sh\n").unwrap();
        fs::create_dir(&as_dir).unwrap();

        env::set_var(ENV_OVERRIDE, &missing);
        assert_eq!(daemon_binary(), None, "a missing override must not resolve");

        // A stray `copypaste-daemon/` directory is not something to spawn.
        env::set_var(ENV_OVERRIDE, &as_dir);
        assert_eq!(daemon_binary(), None, "a directory must not resolve");

        env::set_var(ENV_OVERRIDE, &present);
        assert_eq!(daemon_binary(), Some(present));

        env::remove_var(ENV_OVERRIDE);
    }
}
