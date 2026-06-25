//! Tests for CopyPaste-1jms.22: the `SyncBadgeState::Syncing` variant must be
//! reachable on the live IPC path, not just in the badge-derivation unit tests.
//!
//! ## Acceptance criteria (from bd scope-lock note)
//!
//! 1. `SyncInFlightGuard` flips the `AtomicBool` **true** on creation and
//!    **false** on drop — verified hermetically without any network I/O.
//!
//! 2. `get_sync_status` returns `badge_state: "syncing"` while `sync_in_flight`
//!    is `true` AND no recent sync has been recorded (last_sync_ms == 0, no
//!    passphrase set, so no "recently synced" short-circuit).
//!
//! 3. `get_sync_status` returns a non-"syncing" state once the flag is cleared.
//!
//! ## Compilation note
//!
//! The `get_sync_status` handler is cfg-gated on `cloud-sync`; the IPC badge
//! tests are therefore also gated so they don't compile when the feature is off.
//! The RAII guard tests are feature-independent and always compile.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::sync::Mutex;

use copypaste_core::Database;
use copypaste_daemon::ipc::IpcServer;
use copypaste_daemon::sync_in_flight::SyncInFlightGuard;

// ── RAII guard unit tests ─────────────────────────────────────────────────────
// These tests are NOT gated on cloud-sync: the guard struct has no feature dep.

/// `SyncInFlightGuard::new` must set the flag to `true`; dropping it must
/// reset to `false`. This holds even when the same `Arc` is shared.
#[test]
fn raii_guard_sets_and_clears_atomic_bool() {
    let flag = Arc::new(AtomicBool::new(false));

    assert!(!flag.load(Ordering::Acquire), "flag must start false");

    {
        let _guard = SyncInFlightGuard::new(Arc::clone(&flag));
        assert!(
            flag.load(Ordering::Acquire),
            "flag must be true while guard is alive"
        );
    }

    assert!(
        !flag.load(Ordering::Acquire),
        "flag must be false after guard is dropped"
    );
}

/// Guard resets to `false` even when the scope is exited via an early return.
#[test]
fn raii_guard_resets_on_early_return() {
    let flag = Arc::new(AtomicBool::new(false));

    let result: Result<(), &str> = (|| {
        let _guard = SyncInFlightGuard::new(Arc::clone(&flag));
        assert!(flag.load(Ordering::Acquire), "true while guard alive");
        return Err("simulated early return");
        #[allow(unreachable_code)]
        Ok(())
    })();

    assert!(result.is_err());
    assert!(
        !flag.load(Ordering::Acquire),
        "flag must be false after guard dropped on early return"
    );
}

// ── IPC badge-state tests (require cloud-sync feature) ────────────────────────

#[cfg(feature = "cloud-sync")]
mod badge_state_ipc {
    use super::*;

    /// Boot an in-process IpcServer with the shared in-flight flag wired in.
    /// Returns the shared flag, the TempDir (keeps the socket alive), and the
    /// socket path.
    async fn boot_server_with_inflight(
        in_flight: Arc<AtomicBool>,
    ) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().expect("tempdir");
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let sock = dir.path().join(format!("sif-{suffix}.sock"));

        let db = Arc::new(Mutex::new(
            Database::open_in_memory().expect("in-memory DB"),
        ));
        let private_mode = Arc::new(AtomicBool::new(false));
        let local_key = Arc::new(zeroize::Zeroizing::new([0u8; 32]));
        let device_pub = Arc::new([0u8; 32]);

        // Wire cloud-sync state (all-zero/None stubs — no real cloud ops).
        let sync_key = Arc::new(Mutex::new(None::<copypaste_core::SyncKey>));
        let last_sync_ms = Arc::new(std::sync::atomic::AtomicI64::new(0));
        let signed_in = Arc::new(AtomicBool::new(false));

        let server = IpcServer::new(db, private_mode, local_key, device_pub)
            .with_cloud_sync_state(sync_key, last_sync_ms, signed_in)
            .with_sync_in_flight(in_flight);

        let sock_for_task = sock.clone();
        tokio::spawn(async move {
            let _ = server
                .serve(&sock_for_task, tokio_util::sync::CancellationToken::new())
                .await;
        });

        // Wait until the socket is connectable.
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            if UnixStream::connect(&sock).await.is_ok() {
                return (dir, sock);
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        panic!("server failed to bind {sock:?} within 2s");
    }

    /// Send one IPC request over a fresh connection and return the parsed JSON.
    async fn ipc_call(sock: &std::path::Path, payload: &str) -> serde_json::Value {
        let mut stream = UnixStream::connect(sock).await.expect("connect");
        let mut payload = payload.to_string();
        payload.push('\n');
        stream.write_all(payload.as_bytes()).await.expect("write");
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines
            .next_line()
            .await
            .expect("read")
            .expect("server closed without response");
        serde_json::from_str(&line).expect("parse response JSON")
    }

    /// `get_sync_status` must return `badge_state: "syncing"` when the shared
    /// `sync_in_flight` flag is `true` and no recent sync has completed.
    #[tokio::test]
    async fn get_sync_status_returns_syncing_while_in_flight() {
        // Ensure no Supabase env vars bleed in from the parent shell.
        std::env::remove_var("SUPABASE_URL");
        std::env::remove_var("SUPABASE_ANON_KEY");

        let in_flight = Arc::new(AtomicBool::new(false));
        let (_dir, sock) = boot_server_with_inflight(Arc::clone(&in_flight)).await;

        // No sync in flight: badge must NOT be "syncing".
        let resp_idle = ipc_call(
            &sock,
            r#"{"id":"1","method":"get_sync_status","params":{}}"#,
        )
        .await;
        assert_eq!(
            resp_idle["ok"], true,
            "get_sync_status must succeed: {resp_idle}"
        );
        assert_ne!(
            resp_idle["data"]["badge_state"].as_str(),
            Some("syncing"),
            "badge must NOT be syncing when flag is false: {resp_idle}"
        );

        // Arm the flag (simulating an active sync round-trip).
        in_flight.store(true, Ordering::Release);

        let resp = ipc_call(
            &sock,
            r#"{"id":"2","method":"get_sync_status","params":{}}"#,
        )
        .await;
        assert_eq!(resp["ok"], true, "get_sync_status must succeed: {resp}");
        assert_eq!(
            resp["data"]["badge_state"].as_str(),
            Some("syncing"),
            "badge_state must be 'syncing' while in_flight=true and no recent sync: {resp}"
        );

        // Clear the flag (simulating round-trip completion or error).
        in_flight.store(false, Ordering::Release);

        let resp2 = ipc_call(
            &sock,
            r#"{"id":"3","method":"get_sync_status","params":{}}"#,
        )
        .await;
        assert_eq!(resp2["ok"], true, "get_sync_status must succeed: {resp2}");
        assert_ne!(
            resp2["data"]["badge_state"].as_str(),
            Some("syncing"),
            "badge_state must NOT be syncing after flag cleared: {resp2}"
        );
    }
}
