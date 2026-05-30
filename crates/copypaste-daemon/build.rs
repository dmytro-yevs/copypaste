//! Build script: stamp a `COPYPASTE_BUILD_VERSION` into the binary.
//!
//! The value is `<crate-version>+<git-short-sha>` when a git checkout is
//! available at build time, else just `<crate-version>`. Clients use this to
//! detect a stale daemon left running after an upgrade (a different build
//! version answering the IPC socket means the on-disk binary changed but the
//! old process is still serving old code).
//!
//! We deliberately keep this dependency-free (no `git2`/`vergen`): a plain
//! `git rev-parse` shell-out that fails gracefully when git or the `.git`
//! directory is absent (e.g. building from a release tarball).

use std::process::Command;

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
        None => version,
    };

    println!("cargo:rustc-env=COPYPASTE_BUILD_VERSION={build_version}");

    // Re-run if HEAD moves so the stamped sha stays accurate across commits.
    // Best-effort: these paths may not exist in a tarball build.
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../.git/refs");
}
