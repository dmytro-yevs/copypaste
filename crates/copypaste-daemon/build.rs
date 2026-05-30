//! Build script: stamp a `COPYPASTE_BUILD_VERSION` into the binary.
//!
//! The value is `<crate-version>+<git-short-sha>` when a git checkout is
//! available at build time. When git is absent (e.g. release tarball, CI
//! without a `.git` dir) we fall back to `<crate-version>+t<unix-seconds>` —
//! a build timestamp. This ensures two different source snapshots built
//! without git always produce different version strings, so the stale-daemon
//! eviction logic (which compares `build_version`) works correctly and does
//! not silently keep an old daemon running after an upgrade.
//!
//! We deliberately keep this dependency-free (no `git2`/`vergen`): a plain
//! `git rev-parse` shell-out that fails gracefully when git or the `.git`
//! directory is absent.

use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    let version = std::env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "unknown".to_string());

    let git_sha = Command::new("git")
        .args(["rev-parse", "--short=12", "HEAD"])
        .output()
        .ok()
        .filter(|out| out.status.success())
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let build_version = match git_sha {
        Some(sha) => format!("{version}+{sha}"),
        None => {
            // No git SHA available (tarball / detached build). Use the current
            // Unix timestamp in seconds as a discriminator so two different
            // builds of different commits produce distinct version strings.
            // The `t` prefix makes it visually distinct from a git sha in logs.
            let ts = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            format!("{version}+t{ts}")
        }
    };

    println!("cargo:rustc-env=COPYPASTE_BUILD_VERSION={build_version}");

    // Re-run if HEAD moves so the stamped sha stays accurate across commits.
    // Best-effort: these paths may not exist in a tarball build.
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../.git/refs");
}
