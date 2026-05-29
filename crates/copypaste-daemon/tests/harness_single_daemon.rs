//! E2E harness — Step A: prove the support helper can launch a single, fully
//! isolated `copypaste-daemon` and that it answers an IPC request.
//!
//! We use the `status` method (a non-list request) so the test does not race
//! with the daemon's clipboard monitor populating the item list.

#[path = "support/mod.rs"]
mod support;

use support::Daemon;

/// Spawn one isolated daemon via the harness helper and assert it answers a
/// `status` IPC request successfully.
#[test]
fn single_daemon_spawns_and_answers_status() {
    let daemon = Daemon::spawn();

    let resp = daemon.request(r#"{"id":"a1","method":"status"}"#);

    assert_eq!(
        resp["ok"], true,
        "expected ok=true for status request, got: {resp}"
    );
    assert_eq!(
        resp["data"]["status"], "running",
        "expected status=\"running\", got: {resp}"
    );
}
