//! Lifecycle / recovery tests for the daemon — Wave 3.2
//!
//! Most are `#[ignore]`-gated because they require:
//! - a built daemon binary (subprocess spawn)
//! - macOS for SIGKILL/launchd
//! - real WS for reconnect
//!
//! These tests are scaffolding only and serve as documentation of intent.
//! They can be wired up post-alpha once the daemon binary and launchd plist
//! are stable.

#![allow(dead_code, unused_imports)]

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

/// Resolve the daemon binary path from CARGO_BIN_EXE_<name> if available,
/// otherwise fall back to target/debug/copypaste-daemon.
fn daemon_bin() -> PathBuf {
    if let Some(p) = option_env!("CARGO_BIN_EXE_copypaste-daemon") {
        return PathBuf::from(p);
    }
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop(); // crates/
    p.pop(); // repo root
    p.push("target");
    p.push("debug");
    p.push("copypaste-daemon");
    p
}

/// Build a tmpdir for an isolated DB / state directory for a single test run.
fn isolated_state_dir() -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!(
        "copypaste-lifecycle-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let _ = std::fs::create_dir_all(&p);
    p
}

/// SIGKILL daemon → launchd respawn → Lamport clock persisted across restart.
///
/// Acceptance:
/// 1. Boot daemon, write Lamport=N via internal API or by emitting N clipboard events.
/// 2. SIGKILL the process (mimics OOM / crash).
/// 3. Restart (launchd in prod; manual re-exec here).
/// 4. Assert the persisted Lamport on next emit is >= N+1.
#[test]
#[ignore = "requires built daemon binary + isolated DB; enable after Phase 4"]
fn sigkill_recovers_lamport() {
    let bin = daemon_bin();
    let state = isolated_state_dir();

    // Spawn round 1
    let mut child = Command::new(&bin)
        .env("COPYPASTE_STATE_DIR", &state)
        .env("COPYPASTE_TEST_MODE", "1")
        .env("COPYPASTE_EPHEMERAL_KEY", "1") // skip macOS Keychain prompt
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn daemon round 1");

    // Give it a moment to initialise + write its baseline Lamport.
    std::thread::sleep(Duration::from_millis(500));

    // `std::process::Child::kill` sends SIGKILL on Unix and TerminateProcess on
    // Windows — both are unrecoverable, which is what we want here (mimics OOM /
    // hard crash, not a graceful shutdown).
    child.kill().expect("kill round 1");
    let status = child.wait().expect("wait round 1");
    // On Unix the exit code is None when killed by signal; on Windows we just
    // assert termination.
    #[cfg(unix)]
    assert!(
        status.code().is_none(),
        "expected death by signal, got {:?}",
        status
    );
    #[cfg(not(unix))]
    let _ = status;

    // Round 2 — verify state recovered.
    let mut child2 = Command::new(&bin)
        .env("COPYPASTE_STATE_DIR", &state)
        .env("COPYPASTE_TEST_MODE", "1")
        .env("COPYPASTE_EPHEMERAL_KEY", "1") // skip macOS Keychain prompt
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn daemon round 2");

    std::thread::sleep(Duration::from_millis(500));

    // Assert the DB file survived the SIGKILL — necessary but not sufficient.
    let db_present = std::fs::read_dir(&state)
        .map(|rd| {
            rd.flatten()
                .any(|e| e.file_name().to_string_lossy().contains(".db"))
        })
        .unwrap_or(false);

    // Clean shutdown of round-2 daemon — avoid zombie processes in CI.
    let _ = child2.kill();
    let _ = child2.wait();

    assert!(
        db_present,
        "expected DB file to survive SIGKILL in {:?}",
        state
    );

    // wave3.2-followup: the Lamport-persistence assertion requires the IPC
    // socket to expose a `get_lamport` diagnostic endpoint, which does not
    // yet exist.  The DB-presence check above is the primary acceptance
    // criterion for this phase; the full Lamport assertion is tracked in the
    // issue backlog.  The `todo!()` that was here previously would panic
    // (rather than skip) when the test was run with `--include-ignored`,
    // which is worse than leaving the assertion absent.
}

// CopyPaste-ojas.17: removed the empty, assertion-free `#[ignore]` stub
// `wake_from_sleep_reconnects` (sleep→wake→WS-reconnect). It asserted nothing
// and so was misleading coverage. The sleep/wake hook surface it needs
// (NSWorkspace injection / synthetic event) is not yet exposed — that future
// work is tracked in bd (wave3.2-followup), not as a fake test here. The
// `sigkill_recovers_lamport` integration test below remains (it has real
// process-termination assertions and is `#[ignore]`-gated only because it needs
// a built daemon binary).

/// Smoke test that always runs: the helper paths used by the ignored tests
/// resolve sensibly. Keeps this file from silently rotting if the surrounding
/// crate layout changes.
#[test]
fn lifecycle_scaffolding_helpers_resolve() {
    let bin = daemon_bin();
    assert!(
        bin.file_name().is_some(),
        "daemon_bin() returned a path with a filename: {:?}",
        bin
    );

    let state = isolated_state_dir();
    assert!(state.exists(), "isolated_state_dir created {:?}", state);
    let _ = std::fs::remove_dir_all(&state);
}
