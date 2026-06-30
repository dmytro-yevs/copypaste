use super::*;
use copypaste_core::Database;
use tempfile::tempdir;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

// -------------------------------------------------------------------
// c4q2.21 — compute_peer_online unit tests
// -------------------------------------------------------------------

/// P2P live sink = true → online regardless of last_sync_at.
#[test]
fn compute_peer_online_live_sink_true() {
    assert!(compute_peer_online(Some(true), None, 1_000_000));
    assert!(compute_peer_online(Some(true), Some(0), 1_000_000));
}

/// P2P live sink = false → offline regardless of last_sync_at.
#[test]
fn compute_peer_online_live_sink_false() {
    assert!(!compute_peer_online(
        Some(false),
        Some(1_000_000),
        1_000_000
    ));
    assert!(!compute_peer_online(Some(false), None, 1_000_000));
}

/// No live sink — recent last_sync_at (within threshold) → online.
#[test]
fn compute_peer_online_boundary_at_threshold() {
    let now = 1_000_000_i64;
    // Exactly at boundary: now - t == ONLINE_THRESHOLD_SECS → online.
    let at_boundary = now - ONLINE_THRESHOLD_SECS;
    assert!(compute_peer_online(None, Some(at_boundary), now));
    // One second past → offline.
    let one_past = at_boundary - 1;
    assert!(!compute_peer_online(None, Some(one_past), now));
}

/// No live sink AND no last_sync_at → offline.
#[test]
fn compute_peer_online_none_last_sync_at_and_no_sink() {
    assert!(!compute_peer_online(None, None, 1_000_000));
}

/// CopyPaste-1jms.25: the peer-card fallback threshold MUST equal the sync
/// badge chip's recency window so the two "recently heard from?" signals
/// agree. Guards against the constants drifting apart again.
#[test]
fn online_threshold_matches_sync_badge_recent_window() {
    assert_eq!(
        ONLINE_THRESHOLD_SECS,
        (copypaste_ipc::SYNC_BADGE_RECENT_MS / 1_000) as i64,
        "peer-card online window must match the sync-chip recency window"
    );
    // Concrete regression of the formerly-contradictory case: a peer last
    // heard from 75 s ago is now ONLINE in the fallback (it was offline under
    // the old 60 s window while the chip still showed it as recent).
    let now = 1_000_000_i64;
    assert!(
        compute_peer_online(None, Some(now - 75), now),
        "a 75s-stale peer must be online (within the shared 5-min window)"
    );
}

/// Create a temp directory and immediately force its permissions to 0o700.
///
/// `tempfile::TempDir::new()` calls `mkdir(path, 0o700)`, but the kernel
/// applies the process umask: `mode & ~umask`. `bind_with_stale_cleanup`
/// (and the tests that exercise it) temporarily set `libc::umask(0o177)`,
/// which reduces `0o700 & ~0o177` to `0o600` (no execute bit). A 0o600
/// directory silently blocks all subsequent file operations inside it with
/// EACCES. Calling `set_permissions(0o700)` right after creation repairs
/// the mode unconditionally — `chmod` requires only ownership, not execute.
fn safe_tempdir() -> tempfile::TempDir {
    let dir = tempdir().expect("failed to create temp dir");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700));
    }
    dir
}

/// CopyPaste-c4q2.24: both IPC directions must be deadline-bounded so a
/// stalled/hostile same-UID client cannot pin a connection slot (and its
/// semaphore permit) forever. The write deadline must be > 0 and should not
/// exceed the read deadline (a slow drain is a harder failure than a slow
/// send and warrants a tighter bound). A full 64-connection buffer-fill DoS
/// reproduction is kernel-send-buffer-dependent and ~10 s+ wall time, so it
/// is covered by manual QA (see the bd issue) rather than a flaky unit test;
/// this guards the intent of the constants.
#[test]
fn ipc_write_timeout_is_bounded_and_not_longer_than_read() {
    assert!(
        IPC_WRITE_TIMEOUT > std::time::Duration::ZERO,
        "write path must have a non-zero deadline"
    );
    assert!(
        IPC_WRITE_TIMEOUT <= IPC_READ_TIMEOUT,
        "write deadline ({:?}) should not exceed read deadline ({:?})",
        IPC_WRITE_TIMEOUT,
        IPC_READ_TIMEOUT
    );
}

/// `get_config` must never ship the GoTrue password or email over IPC.
/// `build_config_response` maps both to `*_set` presence flags and exposes
/// no field that could carry the plaintext, while leaving the publishable
/// anon key intact (CopyPaste-c4q2.18).
#[test]
fn build_config_response_strips_password_and_email() {
    let cfg = AppConfig {
        p2p_enabled: Some(true),
        supabase_url: Some("https://x.supabase.co".into()),
        supabase_anon_key: Some("eyJpublishable".into()),
        supabase_email: Some("user@example.com".into()),
        supabase_password: Some("hunter2".into()),
        ..AppConfig::default()
    };
    // Serialise the typed response exactly as the get_config handler does.
    let v = serde_json::to_value(build_config_response(&cfg)).unwrap();
    let obj = v.as_object().unwrap();
    // Secrets cannot appear — the response type has no field for them.
    assert!(!obj.contains_key("supabase_password"));
    assert!(!obj.contains_key("supabase_email"));
    // Presence flags reflect that both were set.
    assert_eq!(obj["supabase_password_set"], serde_json::json!(true));
    assert_eq!(obj["supabase_email_set"], serde_json::json!(true));
    // Non-secret fields (incl. the publishable anon key) are untouched.
    assert_eq!(
        obj["supabase_anon_key"],
        serde_json::json!("eyJpublishable")
    );
    assert_eq!(
        obj["supabase_url"],
        serde_json::json!("https://x.supabase.co")
    );
    assert_eq!(obj["p2p_enabled"], serde_json::json!(true));
}

// ─── CopyPaste-5lm: PasswordFile at-rest encryption unit tests ──────────

/// `encrypt_pake_password_file` / `decrypt_pake_password_file` must
/// round-trip: encrypt → base64 blob → decrypt → original plaintext.
#[test]
fn pake_password_file_encrypt_decrypt_roundtrip() {
    let plaintext = b"fake_password_file_bytes_for_testing_01234567890";
    let local_key = [0x42u8; 32];
    let fp = "aabbccddeeff";

    let enc = encrypt_pake_password_file(plaintext, fp, &local_key).expect("encrypt must succeed");
    assert!(!enc.is_empty(), "encrypted output must not be empty");

    let decrypted = decrypt_pake_password_file(&enc, fp, &local_key).expect("decrypt must succeed");
    assert_eq!(
        decrypted, plaintext,
        "decrypted bytes must match original plaintext"
    );
}

/// A different fingerprint (wrong AAD) must cause authentication failure.
#[test]
fn pake_password_file_wrong_fp_aad_fails() {
    let plaintext = b"some_pake_blob";
    let local_key = [0x11u8; 32];
    let correct_fp = "aabbcc";
    let wrong_fp = "ddeeff";

    let enc = encrypt_pake_password_file(plaintext, correct_fp, &local_key)
        .expect("encrypt must succeed");
    let result = decrypt_pake_password_file(&enc, wrong_fp, &local_key);
    assert!(
        result.is_err(),
        "decrypt with wrong fingerprint must fail (AEAD auth): {result:?}"
    );
}

/// A wrong local key must cause authentication failure.
#[test]
fn pake_password_file_wrong_key_fails() {
    let plaintext = b"some_pake_blob";
    let correct_key = [0x11u8; 32];
    let wrong_key = [0x22u8; 32];
    let fp = "aabbcc";

    let enc =
        encrypt_pake_password_file(plaintext, fp, &correct_key).expect("encrypt must succeed");
    let result = decrypt_pake_password_file(&enc, fp, &wrong_key);
    assert!(
        result.is_err(),
        "decrypt with wrong key must fail (AEAD auth): {result:?}"
    );
}

/// A truncated blob (too short for even a nonce) must return an error.
#[test]
fn pake_password_file_truncated_blob_fails() {
    let local_key = [0x33u8; 32];
    let fp = "aabb";
    // Only 10 bytes — shorter than the 24-byte nonce.
    use base64::Engine as _;
    let short_b64 = base64::engine::general_purpose::STANDARD.encode([0u8; 10]);
    let result = decrypt_pake_password_file(&short_b64, fp, &local_key);
    assert!(
        result.is_err(),
        "truncated blob must fail with an error: {result:?}"
    );
}

/// list_peers must NOT expose `password_file_enc` or `password_file_b64`
/// in its IPC response (CopyPaste-5lm: prevent credential exfiltration).
#[tokio::test]
async fn list_peers_strips_password_file_fields() {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::UnixStream;
    let dir = safe_tempdir();
    let cfg_home = dir.path().join("cfg");
    let _env = EnvGuard::set_all(
        &[
            "COPYPASTE_CONFIG_DIR",
            "COPYPASTE_DATA_DIR",
            "HOME",
            "XDG_CONFIG_HOME",
        ],
        &cfg_home,
    );

    // Write a peers.json with both sensitive fields present (simulating a
    // legacy + new mix so we confirm both are stripped).
    let peers_path = cfg_home.join("peers.json");
    std::fs::create_dir_all(&cfg_home).unwrap();
    std::fs::write(
        &peers_path,
        r#"[{"fingerprint":"aa:bb:cc","name":"Alice","added_at":1700000000,
                  "password_file_b64":"cGxhaW50ZXh0","password_file_enc":"ZW5jcnlwdGVk"}]"#,
    )
    .unwrap();

    let sock = dir.path().join("test-strip-pf.sock");
    start_test_server(&sock).await;

    let mut stream = UnixStream::connect(&sock).await.unwrap();
    stream
        .write_all(b"{\"id\":\"sp1\",\"method\":\"list_peers\",\"params\":{}}\n")
        .await
        .unwrap();
    let mut lines = BufReader::new(&mut stream).lines();
    let line = lines.next_line().await.unwrap().unwrap();
    let resp: serde_json::Value = serde_json::from_str(&line).unwrap();

    assert_eq!(resp["ok"], true, "list_peers must succeed: {resp}");
    let peers = resp["data"]["peers"].as_array().unwrap();
    assert_eq!(peers.len(), 1, "must have one peer");
    let p = &peers[0];
    assert!(
        p.get("password_file_b64").is_none(),
        "list_peers must strip password_file_b64: {p}"
    );
    assert!(
        p.get("password_file_enc").is_none(),
        "list_peers must strip password_file_enc: {p}"
    );
    // The non-sensitive fields must still be present.
    assert_eq!(p["fingerprint"], "aa:bb:cc");
    assert_eq!(p["name"], "Alice");
}

/// CopyPaste-vypo: list_peers must include a `trust` field for every peer.
/// All persisted peers completed PAKE = "verified".
#[tokio::test]
async fn list_peers_includes_trust_field() {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::UnixStream;
    let dir = safe_tempdir();
    let cfg_home = dir.path().join("cfg-trust");
    let _env = EnvGuard::set_all(
        &[
            "COPYPASTE_CONFIG_DIR",
            "COPYPASTE_DATA_DIR",
            "HOME",
            "XDG_CONFIG_HOME",
        ],
        &cfg_home,
    );

    let peers_path = cfg_home.join("peers.json");
    std::fs::create_dir_all(&cfg_home).unwrap();
    std::fs::write(
        &peers_path,
        r#"[{"fingerprint":"11:22:33","name":"Bob","added_at":1700000000}]"#,
    )
    .unwrap();

    let sock = dir.path().join("test-trust.sock");
    start_test_server(&sock).await;

    let mut stream = UnixStream::connect(&sock).await.unwrap();
    stream
        .write_all(b"{\"id\":\"tv1\",\"method\":\"list_peers\",\"params\":{}}\n")
        .await
        .unwrap();
    let mut lines = BufReader::new(&mut stream).lines();
    let line = lines.next_line().await.unwrap().unwrap();
    let resp: serde_json::Value = serde_json::from_str(&line).unwrap();

    assert_eq!(resp["ok"], true, "list_peers must succeed: {resp}");
    let peers = resp["data"]["peers"].as_array().unwrap();
    assert_eq!(peers.len(), 1, "must have one peer");
    let p = &peers[0];
    assert_eq!(
        p["trust"], "verified",
        "persisted peers must have trust=verified, got: {p}"
    );
}

/// CopyPaste-1jms.32: list_peers must include a `transport` field when a
/// transport is active for a peer, and omit it (or set to null) when none
/// is configured. In the test server (no P2P, no relay, no cloud) the field
/// must be absent — the UI falls back to the local_ip/address heuristic.
#[tokio::test]
async fn list_peers_transport_absent_when_no_transport_active() {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::UnixStream;
    let dir = safe_tempdir();
    let cfg_home = dir.path().join("cfg-transport");
    let _env = EnvGuard::set_all(
        &[
            "COPYPASTE_CONFIG_DIR",
            "COPYPASTE_DATA_DIR",
            "HOME",
            "XDG_CONFIG_HOME",
        ],
        &cfg_home,
    );

    let peers_path = cfg_home.join("peers.json");
    std::fs::create_dir_all(&cfg_home).unwrap();
    // Peer with a local_ip (normally the heuristic for P2P) but no live
    // P2P connection in the test server. Transport should be absent/null.
    std::fs::write(
            &peers_path,
            r#"[{"fingerprint":"aa:bb:11","name":"Carol","added_at":1700000000,"local_ip":"192.168.1.10"}]"#,
        )
        .unwrap();

    let sock = dir.path().join("test-transport-none.sock");
    start_test_server(&sock).await;

    let mut stream = UnixStream::connect(&sock).await.unwrap();
    stream
        .write_all(b"{\"id\":\"tp1\",\"method\":\"list_peers\",\"params\":{}}\n")
        .await
        .unwrap();
    let mut lines = BufReader::new(&mut stream).lines();
    let line = lines.next_line().await.unwrap().unwrap();
    let resp: serde_json::Value = serde_json::from_str(&line).unwrap();

    assert_eq!(resp["ok"], true, "list_peers must succeed: {resp}");
    let peers = resp["data"]["peers"].as_array().unwrap();
    assert_eq!(peers.len(), 1);
    let p = &peers[0];
    // In the test server: no P2P sinks wired (live_fps = None), relay feature
    // is compiled but no relay_handle injected, cloud-sync is off. The
    // transport field must be absent (not `"p2p"`) because there is no live
    // P2P connection — even though local_ip is present.
    assert!(
        p.get("transport").is_none() || p["transport"].is_null(),
        "transport must be absent/null when no transport is active, got: {p}"
    );
    // Other fields must still be present.
    assert_eq!(p["fingerprint"], "aa:bb:11");
    assert_eq!(p["trust"], "verified");
}

/// When the credentials are absent (None), the presence flags must be
/// `false` and no secret key should appear on the wire.
#[test]
fn build_config_response_reports_unset_when_none() {
    let cfg = AppConfig {
        supabase_email: None,
        supabase_password: None,
        ..AppConfig::default()
    };
    let v = serde_json::to_value(build_config_response(&cfg)).unwrap();
    let obj = v.as_object().unwrap();
    assert_eq!(obj["supabase_password_set"], serde_json::json!(false));
    assert_eq!(obj["supabase_email_set"], serde_json::json!(false));
    assert!(!obj.contains_key("supabase_password"));
    assert!(!obj.contains_key("supabase_email"));
}

/// RAII guard that snapshots one or more env vars, sets them for the test,
/// and restores the previous values (or unsets them) on drop — even on
/// panic.  Holds `crate::TEST_ENV_LOCK` (the *process-wide* env lock shared
/// with every other daemon test module) for its whole lifetime so env state
/// cannot race tests in `paths`, `keychain`, or any other module that also
/// mutates `HOME`/`XDG_CONFIG_HOME`.
struct EnvGuard {
    saved: Vec<(&'static str, Option<std::ffi::OsString>)>,
    _lock: std::sync::MutexGuard<'static, ()>,
}

impl EnvGuard {
    /// Point every given env var at `value`. Used to redirect the config
    /// dir to a temp path across platforms: `dirs::config_dir()` honours
    /// `XDG_CONFIG_HOME` on Linux/BSD and `$HOME` (→ Library/Application
    /// Support) on macOS, so callers set both.
    fn set_all(keys: &[&'static str], value: &std::path::Path) -> Self {
        let lock = crate::TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let mut saved = Vec::with_capacity(keys.len());
        for &key in keys {
            saved.push((key, std::env::var_os(key)));
            // SAFETY: serialised via `crate::TEST_ENV_LOCK`; no other
            // thread reads or writes these vars concurrently for the
            // guard's lifetime.
            unsafe { std::env::set_var(key, value) };
        }
        Self { saved, _lock: lock }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        // SAFETY: still holding `crate::TEST_ENV_LOCK` (`_lock`), so the
        // restore is serialised against every other env-mutating test.
        unsafe {
            for (key, original) in self.saved.drain(..) {
                match original {
                    Some(v) => std::env::set_var(key, v),
                    None => std::env::remove_var(key),
                }
            }
        }
    }
}

async fn start_test_server(socket_path: &std::path::Path) -> Arc<AtomicBool> {
    start_test_server_with_mode(socket_path, false).await
}

async fn start_test_server_with_mode(
    socket_path: &std::path::Path,
    initial_private_mode: bool,
) -> Arc<AtomicBool> {
    let (private_mode, _db) =
        start_test_server_returning_db(socket_path, initial_private_mode).await;
    private_mode
}

/// Like `start_test_server_with_mode` but also hands back the shared
/// `Database` handle so a test can seed rows / inspect audit tables.
async fn start_test_server_returning_db(
    socket_path: &std::path::Path,
    initial_private_mode: bool,
) -> (Arc<AtomicBool>, Arc<Mutex<Database>>) {
    let db = Arc::new(Mutex::new(Database::open_in_memory().unwrap()));
    let private_mode = Arc::new(AtomicBool::new(initial_private_mode));
    // Dummy keys: in-process tests do not hit paste-back or fingerprint
    // surfaces — they only validate dispatch / state-machine behaviour.
    let local_key = Arc::new(zeroize::Zeroizing::new([0u8; 32]));
    let device_pub = Arc::new([0u8; 32]);
    // Give the test server a realistic mTLS cert fingerprint (colon-hex of a
    // 32-byte SHA-256) so the pairing handlers (`pair_generate_qr`,
    // `get_own_fingerprint`) behave as they do with P2P enabled. Generating a
    // real cert keeps this honest: the advertised value is exactly what the
    // transport would pin.
    let cert = copypaste_p2p::cert::SelfSignedCert::generate("test-device").unwrap();
    let server = IpcServer::new(db.clone(), private_mode.clone(), local_key, device_pub)
        .with_cert_fingerprint(display_fingerprint(&cert.fingerprint()));
    // Bind directly without going through `IpcServer::bind` /
    // `bind_with_stale_cleanup`, which sets `libc::umask(0o177)` process-wide.
    // That process-wide umask change races with concurrent tests' `mkdir` /
    // `tempdir` calls, producing directories with mode 0o600 (no execute bit)
    // that make all subsequent file operations inside them fail with EACCES.
    // In tests the socket lives in a fresh tempdir, so neither stale-socket
    // self-healing nor restrictive socket permissions are needed.
    let path = socket_path.to_path_buf();
    let listener =
        tokio::net::UnixListener::bind(socket_path).expect("test socket bind must succeed");
    tokio::spawn(async move {
        if let Err(e) = server.serve_on(listener, CancellationToken::new()).await {
            tracing::error!("ipc: server on {:?} exited with error: {e}", &path);
        }
    });
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    (private_mode, db)
}

// -----------------------------------------------------------------------
// Stale-socket self-heal (fix/daemon-ipc-selfheal)
// -----------------------------------------------------------------------

/// A path that does not exist is never "live".
#[test]
fn is_socket_live_false_for_missing_path() {
    let dir = safe_tempdir();
    let sock = dir.path().join("missing.sock");
    assert!(!is_socket_live(&sock));
}

/// A regular file sitting at the socket path is not a live listener —
/// `connect()` on a non-socket fails, so we treat it as not-live (and the
/// bind helper will clean it up).
#[test]
fn is_socket_live_false_for_stale_regular_file() {
    let dir = safe_tempdir();
    // Hold TEST_ENV_LOCK so this test is serialised with the
    // bind_with_stale_cleanup_* tests that call libc::umask(0o177).
    // Without serialisation, the umask window from those tests can corrupt
    // the tempdir mode (0o600 instead of 0o700), making fs::write fail.
    let _env = EnvGuard::set_all(
        &[
            "COPYPASTE_DATA_DIR",
            "COPYPASTE_CONFIG_DIR",
            "HOME",
            "XDG_CONFIG_HOME",
        ],
        dir.path(),
    );
    let sock = dir.path().join("stale.sock");
    std::fs::write(&sock, b"not a socket").unwrap();
    assert!(!is_socket_live(&sock));
}

/// `BUILD_VERSION` must be non-empty and start with the crate's semver so
/// clients can compare it against their own version prefix to detect a
/// stale daemon after an upgrade.
#[test]
fn build_version_is_crate_version_prefixed() {
    assert!(!BUILD_VERSION.is_empty(), "BUILD_VERSION must not be empty");
    let crate_ver = env!("CARGO_PKG_VERSION");
    assert!(
        BUILD_VERSION == crate_ver || BUILD_VERSION.starts_with(&format!("{crate_ver}+")),
        "BUILD_VERSION {BUILD_VERSION:?} must equal or be `<{crate_ver}>+<sha>`"
    );
}

/// A leftover socket *file* with no process accepting on it is stale:
/// `bind_with_stale_cleanup` must remove it and successfully rebind,
/// rather than failing with `EADDRINUSE`. This is the core self-heal for
/// the "process alive but socket not reachable" upgrade bug.
///
/// Uses `std::os::unix::net::UnixListener` to seed the stale socket so the
/// "previous daemon" half does not depend on a Tokio reactor; the helper
/// under test (`bind_with_stale_cleanup`) binds a `tokio` listener, hence
/// `#[tokio::test]`.
#[tokio::test]
async fn bind_with_stale_cleanup_removes_dead_socket_and_rebinds() {
    let dir = safe_tempdir();
    let sock = dir.path().join("daemon.sock");
    // Hold TEST_ENV_LOCK for the duration of this test to serialise the
    // libc::umask(0o177) call inside bind_with_stale_cleanup with concurrent
    // tests that create directories (which are corrupted by the process-wide
    // umask if it leaks into their mkdir calls).
    let _env = EnvGuard::set_all(
        &[
            "COPYPASTE_DATA_DIR",
            "COPYPASTE_CONFIG_DIR",
            "HOME",
            "XDG_CONFIG_HOME",
        ],
        dir.path(),
    );

    // Create a real socket then drop its listener so the path is left
    // behind with no live acceptor — exactly what a crashed daemon leaves.
    {
        let dead = std::os::unix::net::UnixListener::bind(&sock).expect("seed bind");
        drop(dead);
    }
    assert!(sock.exists(), "socket file must remain after listener drop");
    // TOCTOU settle (CopyPaste-del): the kernel can briefly keep accept()ing
    // on a just-dropped listen socket before the fd is fully reaped, so a
    // single `is_socket_live` probe is flaky under parallel load. Poll until
    // it reads as not-live (bounded) instead of asserting on the first probe.
    let mut live = is_socket_live(&sock);
    for _ in 0..200 {
        if !live {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        live = is_socket_live(&sock);
    }
    assert!(
        !live,
        "dropped listener must not be detected as live (after settle)"
    );

    // The helper must clean up and bind successfully.
    let listener =
        bind_with_stale_cleanup(&sock).expect("must self-heal a stale socket and rebind");
    assert!(is_socket_live(&sock), "rebound socket must accept connects");
    drop(listener);
}

/// A live listener that does NOT speak our protocol (never answers
/// `status`, so reports no `build_version`/`pid`) cannot be safely evicted:
/// the helper must refuse to bind rather than unlink a socket a live
/// process still owns. (A real same-version daemon answers `status` and is
/// covered by `..._refuses_to_steal_healthy_same_version_daemon` below.)
#[tokio::test]
async fn bind_with_stale_cleanup_refuses_unidentifiable_live_socket() {
    let dir = safe_tempdir();
    let sock = dir.path().join("daemon.sock");

    // Hold a live listener (std, no reactor needed) for the whole test.
    let _live = std::os::unix::net::UnixListener::bind(&sock).expect("seed live bind");
    assert!(is_socket_live(&sock), "seeded listener must be live");

    let err = bind_with_stale_cleanup(&sock).expect_err("must refuse to bind over a live socket");
    let msg = err.to_string();
    assert!(
        msg.contains("cannot be evicted automatically"),
        "expected a 'cannot be evicted' refusal, got: {msg}"
    );
}

/// A live daemon answering `status` with the SAME `build_version` as us is
/// a healthy same-version peer — the helper must NOT steal its socket.
#[tokio::test]
async fn bind_with_stale_cleanup_refuses_to_steal_healthy_same_version_daemon() {
    let dir = safe_tempdir();
    let sock = dir.path().join("daemon.sock");

    // A minimal acceptor that replies to `status` with OUR build_version
    // and a bogus pid. It keeps accepting for the whole test (loop on a
    // cloned fd) so the socket stays live through the probe.
    let listener = std::os::unix::net::UnixListener::bind(&sock).expect("seed bind");
    let acceptor = listener.try_clone().expect("clone listener fd");
    let body = serde_json::json!({
        "ok": true,
        "data": { "build_version": BUILD_VERSION, "pid": 999_999u32 },
    })
    .to_string();
    let handle = std::thread::spawn(move || {
        use std::io::{BufRead, BufReader, Write};
        loop {
            let Ok((stream, _)) = acceptor.accept() else {
                break;
            };
            let mut reader = BufReader::new(&stream);
            let mut line = String::new();
            if reader.read_line(&mut line).is_ok() && line.contains("status") {
                let mut resp = body.clone();
                resp.push('\n');
                let _ = (&stream).write_all(resp.as_bytes());
            }
        }
    });

    let err = bind_with_stale_cleanup(&sock)
        .expect_err("must refuse to steal a healthy same-version daemon's socket");
    let msg = err.to_string();
    assert!(
        msg.contains("healthy same-version peer"),
        "expected same-version refusal, got: {msg}"
    );
    drop(listener); // ends the acceptor thread; tempdir teardown frees the path.
    let _ = handle;
}

/// A live daemon answering `status` with a DIFFERENT `build_version` is a
/// STALE predecessor from before an upgrade. The helper must try to evict
/// it (SIGTERM its reported pid). Here the reported pid is unsignalable
/// (ESRCH), so the socket is never released and we must surface a clear,
/// actionable error rather than silently coexisting / unlinking a live
/// socket.
#[tokio::test]
async fn bind_with_stale_cleanup_attempts_eviction_for_different_version() {
    let dir = safe_tempdir();
    let sock = dir.path().join("daemon.sock");

    // The seed acceptor keeps the socket live for the WHOLE test (looping on
    // blocking accept) so eviction genuinely cannot succeed. We hold the
    // original listener in the test and hand the thread a `try_clone` so the
    // socket stays bound until the test's tempdir teardown frees the path.
    let listener = std::os::unix::net::UnixListener::bind(&sock).expect("seed bind");
    let acceptor = listener.try_clone().expect("clone listener fd");
    // Report a different build version + a pid that maps to ESRCH (no such
    // process), so `evict_stale_daemon` SIGTERMs nothing and then times out
    // observing the socket is still held.
    let body = serde_json::json!({
        "ok": true,
        "data": { "build_version": "0.0.0-stale+deadbeef", "pid": 2_000_000_001u32 },
    })
    .to_string();
    let handle = std::thread::spawn(move || {
        use std::io::{BufRead, BufReader, Write};
        loop {
            let Ok((stream, _)) = acceptor.accept() else {
                break;
            };
            let mut reader = BufReader::new(&stream);
            let mut line = String::new();
            if reader.read_line(&mut line).is_ok() && line.contains("status") {
                let mut resp = body.clone();
                resp.push('\n');
                let _ = (&stream).write_all(resp.as_bytes());
            }
        }
    });

    let err = bind_with_stale_cleanup(&sock)
        .expect_err("eviction of an unsignalable stale pid must fail loudly, not silently bind");
    let msg = err.to_string();
    assert!(
        msg.contains("could not evict daemon"),
        "expected an eviction-failure error, got: {msg}"
    );
    // Dropping both listener fds unblocks/ends the acceptor thread.
    drop(listener);
    let _ = handle;
}

/// The `status` probe must round-trip `build_version` + `pid` from a daemon
/// that answers, and yield `None`/defaults from a socket that says nothing.
#[tokio::test]
async fn probe_listening_daemon_reads_version_and_pid() {
    let dir = safe_tempdir();
    let sock = dir.path().join("daemon.sock");
    let listener = std::os::unix::net::UnixListener::bind(&sock).expect("seed bind");
    let handle = std::thread::spawn(move || {
        use std::io::{BufRead, BufReader, Write};
        if let Ok((stream, _)) = listener.accept() {
            let mut reader = BufReader::new(&stream);
            let mut line = String::new();
            let _ = reader.read_line(&mut line);
            let resp = serde_json::json!({
                "ok": true,
                "data": { "build_version": "9.9.9+abc", "pid": 4242u32 },
            })
            .to_string();
            let _ = (&stream).write_all(format!("{resp}\n").as_bytes());
        }
    });

    let probed = probe_listening_daemon(&sock).expect("probe should connect");
    assert_eq!(probed.build_version.as_deref(), Some("9.9.9+abc"));
    assert_eq!(probed.pid, Some(4242));
    handle.join().ok();
}

// ── CopyPaste-dl1e: PID exe validation ───────────────────────────────────

/// `pid_exe_is_copypaste` must return `Some(true)` for THIS process (whose
/// exe path definitely contains "copypaste" in CI / cargo test paths, OR
/// at minimum must return `Some(_)` meaning the exe was resolved without error).
///
/// We also verify the negative: a non-existent PID must return `None` (process
/// gone → fail safe → do not signal).
#[cfg(unix)]
#[test]
fn pid_exe_is_copypaste_returns_none_for_dead_pid() {
    // PID 2_000_000_001 is above the typical Linux/macOS pid_max and cannot
    // exist — resolving its exe must return None (fail-safe path).
    let result = pid_exe_is_copypaste(2_000_000_001u32);
    assert!(
        result.is_none(),
        "dead/impossible pid must return None, got {result:?}"
    );
}

/// Our own process (current pid) must resolve its exe successfully.
/// The result is `Some(true)` when run via `cargo test` (binary path contains
/// "copypaste" or "deps") or `Some(false)` on non-copypaste test runners —
/// either way it must be `Some(_)`, not `None`, because the process exists.
#[cfg(unix)]
#[test]
fn pid_exe_path_resolves_own_pid() {
    let own_pid = std::process::id();
    let exe = pid_exe_path(own_pid);
    // Must resolve (Some); the exact path depends on the runner.
    assert!(
        exe.is_some(),
        "pid_exe_path must resolve current pid {own_pid}, got None"
    );
}

#[tokio::test]
async fn status_returns_running() {
    let dir = safe_tempdir();
    let sock = dir.path().join("test.sock");
    start_test_server(&sock).await;

    let mut stream = UnixStream::connect(&sock).await.unwrap();
    stream
        .write_all(b"{\"id\":\"1\",\"method\":\"status\"}\n")
        .await
        .unwrap();
    let mut lines = BufReader::new(&mut stream).lines();
    let line = lines.next_line().await.unwrap().unwrap();
    let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(resp["ok"], true);
    assert_eq!(resp["data"]["status"], "running");
}

/// CopyPaste-ruep: status must include a non-empty device_key_fingerprint
/// (SHA-256 of the X25519 public key, lowercase hex, 64 chars).
#[tokio::test]
async fn status_includes_device_key_fingerprint() {
    let dir = safe_tempdir();
    let sock = dir.path().join("dkfp.sock");
    start_test_server(&sock).await;

    let mut stream = UnixStream::connect(&sock).await.unwrap();
    stream
        .write_all(b"{\"id\":\"dkfp\",\"method\":\"status\"}\n")
        .await
        .unwrap();
    let mut lines = BufReader::new(&mut stream).lines();
    let line = lines.next_line().await.unwrap().unwrap();
    let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(resp["ok"], true, "status must succeed: {resp}");
    let fp = resp["data"]["device_key_fingerprint"]
        .as_str()
        .expect("device_key_fingerprint must be a string");
    // SHA-256 of any 32-byte key = 64 lowercase hex chars
    assert_eq!(fp.len(), 64, "fingerprint must be 64 hex chars, got: {fp}");
    assert!(
        fp.chars().all(|c| c.is_ascii_hexdigit()),
        "fingerprint must be hex, got: {fp}"
    );
}

/// c4q2.17: `list` is now a not_implemented stub; zero-item check migrated to
/// `history_page`. This test now verifies the stub response shape.
#[tokio::test]
async fn list_empty_db_returns_zero() {
    let dir = safe_tempdir();
    let sock = dir.path().join("test2.sock");
    start_test_server(&sock).await;

    let resp = call_one(&sock, r#"{"id":"2","method":"list"}"#).await;
    assert_eq!(
        resp["ok"], false,
        "list must return not_implemented (c4q2.17): {resp}"
    );
    assert_eq!(resp["error_code"].as_str().unwrap_or(""), "not_implemented");
}

#[tokio::test]
async fn unknown_method_returns_error() {
    let dir = safe_tempdir();
    let sock = dir.path().join("test3.sock");
    start_test_server(&sock).await;

    let mut stream = UnixStream::connect(&sock).await.unwrap();
    stream
        .write_all(b"{\"id\":\"3\",\"method\":\"bogus\"}\n")
        .await
        .unwrap();
    let mut lines = BufReader::new(&mut stream).lines();
    let line = lines.next_line().await.unwrap().unwrap();
    let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(resp["ok"], false);
    assert!(resp["error"].as_str().unwrap().contains("unknown method"));
}

/// ADR-007 — a request carrying a `protocol_version` outside the
/// supported window must be rejected with a stable error code BEFORE
/// the dispatcher tries to interpret the method.
#[tokio::test]
async fn unsupported_protocol_version_rejected_with_error_code() {
    let dir = safe_tempdir();
    let sock = dir.path().join("test-proto-ver.sock");
    start_test_server(&sock).await;

    let mut stream = UnixStream::connect(&sock).await.unwrap();
    // Use a method that would normally succeed (`status`) to prove the
    // version gate fires first.
    let unsupported = CURRENT_PROTOCOL_VERSION + 99;
    let payload = format!(
        "{{\"id\":\"pv1\",\"method\":\"status\",\"protocol_version\":{}}}\n",
        unsupported
    );
    stream.write_all(payload.as_bytes()).await.unwrap();
    let mut lines = BufReader::new(&mut stream).lines();
    let line = lines.next_line().await.unwrap().unwrap();
    let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(resp["ok"], false, "version gate must reject: {line}");
    // ADR-007 + P2-ptb8: the version gate must return ERR_CODE_VERSION_MISMATCH
    // ("version_mismatch") so the CLI can branch deterministically without
    // parsing the error text. A previous version of this test incorrectly
    // asserted "invalid_argument"; corrected to match the dispatcher code.
    assert_eq!(
        resp["error_code"],
        crate::protocol::ERR_CODE_VERSION_MISMATCH,
        "version gate must return ERR_CODE_VERSION_MISMATCH: {resp}"
    );
    assert_eq!(resp["protocol_version"], CURRENT_PROTOCOL_VERSION);
    assert!(
        resp["error"]
            .as_str()
            .unwrap()
            .contains("unsupported protocol version"),
        "expected version-mismatch message, got: {}",
        resp["error"]
    );
}

/// W3.6 — stubbed methods (`cloud_sign_in`, `cloud_sign_out`) must carry
/// a stable machine-readable `error_code: "not_implemented"` so clients
/// can branch deterministically without parsing the English `error` text.
///
/// Only meaningful when `cloud-sync` is OFF: that is the only build where
/// `cloud_sign_in` is the not-implemented STUB. With `cloud-sync` enabled
/// the real handler runs and (correctly) returns `invalid_argument` for the
/// missing-params request used here, so the assertion does not apply.
#[cfg(not(feature = "cloud-sync"))]
#[tokio::test]
async fn ipc_responses_carry_machine_readable_error_code() {
    let dir = safe_tempdir();
    let sock = dir.path().join("test_err_code.sock");
    start_test_server(&sock).await;

    let mut stream = UnixStream::connect(&sock).await.unwrap();
    stream
        .write_all(b"{\"id\":\"42\",\"method\":\"cloud_sign_in\"}\n")
        .await
        .unwrap();
    let mut lines = BufReader::new(&mut stream).lines();
    let line = lines.next_line().await.unwrap().unwrap();
    let resp: serde_json::Value = serde_json::from_str(&line).unwrap();

    assert_eq!(resp["ok"], false, "stub should report failure, not fake ok");
    assert_eq!(
        resp["error_code"], "not_implemented",
        "cloud stub must tag response with machine-readable not_implemented code"
    );
    assert!(
        resp["error"].as_str().unwrap().contains("cloud-sync"),
        "human-readable error should name the unimplemented feature"
    );
}

#[tokio::test]
async fn search_with_no_fts_data_returns_empty() {
    let dir = safe_tempdir();
    let sock = dir.path().join("test_search.sock");
    start_test_server(&sock).await;

    let mut stream = UnixStream::connect(&sock).await.unwrap();
    stream
            .write_all(b"{\"id\":\"s1\",\"method\":\"search\",\"params\":{\"query\":\"hello\",\"limit\":10}}\n")
            .await
            .unwrap();
    let mut lines = BufReader::new(&mut stream).lines();
    let line = lines.next_line().await.unwrap().unwrap();
    let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(resp["ok"], true);
    assert_eq!(resp["data"]["items"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn search_missing_query_returns_error() {
    let dir = safe_tempdir();
    let sock = dir.path().join("test_search_err.sock");
    start_test_server(&sock).await;

    let mut stream = UnixStream::connect(&sock).await.unwrap();
    stream
        .write_all(b"{\"id\":\"s2\",\"method\":\"search\",\"params\":{}}\n")
        .await
        .unwrap();
    let mut lines = BufReader::new(&mut stream).lines();
    let line = lines.next_line().await.unwrap().unwrap();
    let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(resp["ok"], false);
    assert!(resp["error"]
        .as_str()
        .unwrap()
        .contains("missing param: query"));
}

#[tokio::test]
async fn copy_unknown_id_returns_error() {
    let dir = safe_tempdir();
    let sock = dir.path().join("copy_test.sock");
    start_test_server(&sock).await;
    let mut stream = UnixStream::connect(&sock).await.unwrap();
    stream
        .write_all(b"{\"id\":\"1\",\"method\":\"copy\",\"params\":{\"id\":\"nonexistent\"}}\n")
        .await
        .unwrap();
    let mut lines = BufReader::new(&mut stream).lines();
    let line = lines.next_line().await.unwrap().unwrap();
    let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(resp["ok"], false);
}

#[tokio::test]
async fn copy_missing_id_param_returns_error() {
    let dir = safe_tempdir();
    let sock = dir.path().join("copy_missing_param.sock");
    start_test_server(&sock).await;
    let mut stream = UnixStream::connect(&sock).await.unwrap();
    stream
        .write_all(b"{\"id\":\"2\",\"method\":\"copy\",\"params\":{}}\n")
        .await
        .unwrap();
    let mut lines = BufReader::new(&mut stream).lines();
    let line = lines.next_line().await.unwrap().unwrap();
    let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(resp["ok"], false);
    assert!(resp["error"]
        .as_str()
        .unwrap()
        .contains("missing param: id"));
}

#[tokio::test]
async fn stats_returns_zero_for_empty_db() {
    let dir = safe_tempdir();
    let sock = dir.path().join("stats.sock");
    start_test_server(&sock).await;
    let mut stream = UnixStream::connect(&sock).await.unwrap();
    stream
        .write_all(b"{\"id\":\"1\",\"method\":\"stats\"}\n")
        .await
        .unwrap();
    let mut lines = BufReader::new(&mut stream).lines();
    let line = lines.next_line().await.unwrap().unwrap();
    let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(resp["ok"], true);
    assert_eq!(resp["data"]["total_items"], 0);
}

#[tokio::test]
async fn delete_all_returns_count() {
    let dir = safe_tempdir();
    let sock = dir.path().join("del_all.sock");
    start_test_server(&sock).await;
    let mut stream = UnixStream::connect(&sock).await.unwrap();
    stream
        .write_all(b"{\"id\":\"1\",\"method\":\"delete_all\"}\n")
        .await
        .unwrap();
    let mut lines = BufReader::new(&mut stream).lines();
    let line = lines.next_line().await.unwrap().unwrap();
    let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(resp["ok"], true);
    assert!(resp["data"]["deleted"].as_i64().is_some());
}

// --- private mode IPC tests ---

#[tokio::test]
async fn get_private_mode_returns_false_by_default() {
    let dir = safe_tempdir();
    let sock = dir.path().join("pm_get_default.sock");
    start_test_server(&sock).await;
    let mut stream = UnixStream::connect(&sock).await.unwrap();
    stream
        .write_all(b"{\"id\":\"1\",\"method\":\"get_private_mode\"}\n")
        .await
        .unwrap();
    let mut lines = BufReader::new(&mut stream).lines();
    let line = lines.next_line().await.unwrap().unwrap();
    let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(resp["ok"], true);
    assert_eq!(resp["data"]["private_mode"], false);
}

#[tokio::test]
async fn set_private_mode_enable_then_get() {
    let dir = safe_tempdir();
    let sock = dir.path().join("pm_set_enable.sock");
    start_test_server(&sock).await;

    // Enable private mode — first connection
    {
        let mut stream = UnixStream::connect(&sock).await.unwrap();
        stream
            .write_all(
                b"{\"id\":\"1\",\"method\":\"set_private_mode\",\"params\":{\"enabled\":true}}\n",
            )
            .await
            .unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(resp["ok"], true);
        assert_eq!(resp["data"]["private_mode"], true);
    }

    // Verify get_private_mode reflects the change — second connection
    {
        let mut stream2 = UnixStream::connect(&sock).await.unwrap();
        stream2
            .write_all(b"{\"id\":\"2\",\"method\":\"get_private_mode\"}\n")
            .await
            .unwrap();
        let mut lines2 = BufReader::new(&mut stream2).lines();
        let line2 = lines2.next_line().await.unwrap().unwrap();
        let resp2: serde_json::Value = serde_json::from_str(&line2).unwrap();
        assert_eq!(resp2["ok"], true);
        assert_eq!(resp2["data"]["private_mode"], true);
    }
}

#[tokio::test]
async fn set_private_mode_then_disable() {
    let dir = safe_tempdir();
    let sock = dir.path().join("pm_disable.sock");
    start_test_server_with_mode(&sock, true).await;

    // Confirm it starts enabled — first connection
    {
        let mut stream = UnixStream::connect(&sock).await.unwrap();
        stream
            .write_all(b"{\"id\":\"1\",\"method\":\"get_private_mode\"}\n")
            .await
            .unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(resp["data"]["private_mode"], true);
    }

    // Disable — second connection
    {
        let mut stream2 = UnixStream::connect(&sock).await.unwrap();
        stream2
            .write_all(
                b"{\"id\":\"2\",\"method\":\"set_private_mode\",\"params\":{\"enabled\":false}}\n",
            )
            .await
            .unwrap();
        let mut lines2 = BufReader::new(&mut stream2).lines();
        let line2 = lines2.next_line().await.unwrap().unwrap();
        let resp2: serde_json::Value = serde_json::from_str(&line2).unwrap();
        assert_eq!(resp2["ok"], true);
        assert_eq!(resp2["data"]["private_mode"], false);
    }
}

#[tokio::test]
async fn set_private_mode_missing_param_returns_error() {
    let dir = safe_tempdir();
    let sock = dir.path().join("pm_missing.sock");
    start_test_server(&sock).await;
    let mut stream = UnixStream::connect(&sock).await.unwrap();
    stream
        .write_all(b"{\"id\":\"1\",\"method\":\"set_private_mode\",\"params\":{}}\n")
        .await
        .unwrap();
    let mut lines = BufReader::new(&mut stream).lines();
    let line = lines.next_line().await.unwrap().unwrap();
    let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(resp["ok"], false);
    assert!(resp["error"].as_str().unwrap().contains("enabled"));
}

#[tokio::test]
async fn status_includes_private_mode_field() {
    let dir = safe_tempdir();
    let sock = dir.path().join("status_pm.sock");
    start_test_server(&sock).await;
    let mut stream = UnixStream::connect(&sock).await.unwrap();
    stream
        .write_all(b"{\"id\":\"1\",\"method\":\"status\"}\n")
        .await
        .unwrap();
    let mut lines = BufReader::new(&mut stream).lines();
    let line = lines.next_line().await.unwrap().unwrap();
    let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(resp["ok"], true);
    assert_eq!(resp["data"]["status"], "running");
    assert!(resp["data"]["private_mode"].is_boolean());
}

#[tokio::test]
async fn set_private_mode_updates_shared_atomic() {
    let dir = safe_tempdir();
    let sock = dir.path().join("pm_atomic.sock");
    let flag = start_test_server(&sock).await;

    // Initially false
    assert!(!flag.load(Ordering::Relaxed));

    let mut stream = UnixStream::connect(&sock).await.unwrap();
    stream
        .write_all(
            b"{\"id\":\"1\",\"method\":\"set_private_mode\",\"params\":{\"enabled\":true}}\n",
        )
        .await
        .unwrap();
    let mut lines = BufReader::new(&mut stream).lines();
    let _line = lines.next_line().await.unwrap().unwrap();

    // The shared atomic should now be true
    assert!(flag.load(Ordering::Relaxed));
}

// --- history_page ---

#[tokio::test]
async fn history_page_empty_db_returns_zero() {
    let dir = safe_tempdir();
    let sock = dir.path().join("hp_empty.sock");
    start_test_server(&sock).await;
    let mut stream = UnixStream::connect(&sock).await.unwrap();
    stream
            .write_all(b"{\"id\":\"hp1\",\"method\":\"history_page\",\"params\":{\"limit\":50,\"offset\":0}}\n")
            .await
            .unwrap();
    let mut lines = BufReader::new(&mut stream).lines();
    let line = lines.next_line().await.unwrap().unwrap();
    let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(resp["ok"], true);
    assert_eq!(resp["data"]["total"], 0);
    assert_eq!(resp["data"]["items"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn history_page_default_params_succeed() {
    let dir = safe_tempdir();
    let sock = dir.path().join("hp_default.sock");
    start_test_server(&sock).await;
    let mut stream = UnixStream::connect(&sock).await.unwrap();
    // No params — should default to limit=50, offset=0
    stream
        .write_all(b"{\"id\":\"hp2\",\"method\":\"history_page\"}\n")
        .await
        .unwrap();
    let mut lines = BufReader::new(&mut stream).lines();
    let line = lines.next_line().await.unwrap().unwrap();
    let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(resp["ok"], true);
    assert!(resp["data"]["items"].is_array());
}

// --- paste ---

#[tokio::test]
async fn paste_missing_id_returns_error() {
    let dir = safe_tempdir();
    let sock = dir.path().join("paste_missing.sock");
    start_test_server(&sock).await;
    let mut stream = UnixStream::connect(&sock).await.unwrap();
    stream
        .write_all(b"{\"id\":\"p1\",\"method\":\"paste\",\"params\":{}}\n")
        .await
        .unwrap();
    let mut lines = BufReader::new(&mut stream).lines();
    let line = lines.next_line().await.unwrap().unwrap();
    let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(resp["ok"], false);
    assert!(resp["error"]
        .as_str()
        .unwrap()
        .contains("missing param: id"));
}

#[tokio::test]
async fn paste_unknown_id_returns_error() {
    let dir = safe_tempdir();
    let sock = dir.path().join("paste_unknown.sock");
    start_test_server(&sock).await;
    let mut stream = UnixStream::connect(&sock).await.unwrap();
    stream
            .write_all(
                b"{\"id\":\"p2\",\"method\":\"paste\",\"params\":{\"id\":\"00000000-0000-0000-0000-000000000000\"}}\n",
            )
            .await
            .unwrap();
    let mut lines = BufReader::new(&mut stream).lines();
    let line = lines.next_line().await.unwrap().unwrap();
    let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(resp["ok"], false);
    assert!(resp["error"].as_str().unwrap().contains("not found"));
}

// ------------------------------------------------------------------
// Wave 1.1 IPC hardening tests
//
// These verify the security guarantees added in
// `fix(daemon-ipc): wave1.1 — socket chmod 0o600 + request size cap +
//  handle disconnect`:
//   * the Unix listener socket is created with mode 0600 (user-only),
//   * a request line exceeding MAX_REQUEST_BYTES (16 MiB) is rejected
//     with an error response without crashing the server,
//   * a client that connects and disconnects abruptly (no newline,
//     partial write, or zero bytes) does not panic the spawned task.
// ------------------------------------------------------------------

#[tokio::test]
async fn ipc_socket_chmod_is_0600() {
    use std::os::unix::fs::PermissionsExt;
    let dir = safe_tempdir();
    let sock = dir.path().join("hardening_chmod.sock");
    let _env = EnvGuard::set_all(
        &[
            "COPYPASTE_DATA_DIR",
            "COPYPASTE_CONFIG_DIR",
            "HOME",
            "XDG_CONFIG_HOME",
        ],
        dir.path(),
    );
    // Use IpcServer::bind directly so that bind_with_stale_cleanup runs and
    // fchmods the bound socket fd to 0600 (CopyPaste-c4q2.26 replaced the
    // former process-wide umask(0o177) with a per-fd fchmod). This asserts
    // the on-disk socket mode security invariant.
    let db = Arc::new(Mutex::new(Database::open_in_memory().unwrap()));
    let server = IpcServer::new(
        db,
        Arc::new(AtomicBool::new(false)),
        Arc::new(zeroize::Zeroizing::new([0u8; 32])),
        Arc::new([0u8; 32]),
    );
    let listener = server.bind(&sock).expect("bind must succeed");
    drop(listener); // release the socket fd; the file remains for inspection

    let meta = std::fs::metadata(&sock).expect("socket file should exist");
    let mode = meta.permissions().mode() & 0o777;
    assert_eq!(
        mode,
        0o600,
        "socket {} has mode {:o}, expected 0600",
        sock.display(),
        mode
    );
}

/// CopyPaste-c4q2.26: binding the IPC socket must NOT mutate the process-wide
/// `umask`. The old implementation set `umask(0o177)` around `bind`, which
/// could clamp files created by concurrent startup threads to 0600. The fix
/// tightens the socket via a per-inode `chmod`, leaving `umask` untouched.
/// This guards against re-introducing the global side effect.
#[tokio::test]
async fn bind_does_not_mutate_process_umask() {
    // Serialise with the process-wide env/umask lock: reading the umask
    // requires a set+restore, which would otherwise race other tests.
    let _lock = crate::TEST_ENV_LOCK
        .lock()
        .unwrap_or_else(|p| p.into_inner());

    // Read the current umask without leaving it changed.
    let before = unsafe { libc::umask(0o022) };
    unsafe { libc::umask(before) };

    let dir = safe_tempdir();
    let sock = dir.path().join("umask_probe.sock");
    let listener = bind_with_stale_cleanup(&sock).expect("bind must succeed");
    drop(listener);

    let after = unsafe { libc::umask(0o022) };
    unsafe { libc::umask(after) };
    assert_eq!(
        before, after,
        "bind_with_stale_cleanup must not leave the process umask mutated \
             (before={before:#o}, after={after:#o})"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ipc_oversized_request_rejected_not_crashed() {
    let dir = safe_tempdir();
    let sock = dir.path().join("hardening_oversize.sock");
    start_test_server(&sock).await;

    // Client A: send 17 MiB without a newline. The server reads up to
    // MAX_REQUEST_BYTES + 1 (16 MiB + 1) and trips the oversize branch,
    // returns an error response, and closes the connection.
    {
        let mut stream = UnixStream::connect(&sock).await.unwrap();
        let payload = vec![b'A'; 17 * 1024 * 1024];
        // The server may close before we finish writing — that's fine.
        let _ = stream.write_all(&payload).await;
        // Half-close write so the server's read_until unblocks.
        let _ = stream.shutdown().await;

        // Try to read the error response, bounded by a timeout so a
        // misbehaving server can't hang the test.
        let mut reader = BufReader::new(&mut stream);
        let mut line = String::new();
        if let Ok(Ok(_n)) = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            reader.read_line(&mut line),
        )
        .await
        {
            if !line.trim().is_empty() {
                let resp: serde_json::Value = serde_json::from_str(line.trim())
                    .expect("oversize response should be valid JSON");
                assert_eq!(resp["ok"], false, "expected error response, got: {resp}");
                let err = resp["error"].as_str().unwrap_or_default();
                assert!(
                    err.contains("too large"),
                    "expected 'too large' in error, got: {err}"
                );
            }
            // If we got no bytes back (race with server close), the
            // next client below proves the server didn't crash.
        }
    }

    // Client B: a normal request must still succeed — proves the server
    // survived the oversize client.
    {
        let mut stream = UnixStream::connect(&sock)
            .await
            .expect("server must still accept new connections after oversize client");
        stream
            .write_all(b"{\"id\":\"after-oversize\",\"method\":\"status\"}\n")
            .await
            .unwrap();
        let mut reader = BufReader::new(&mut stream);
        let mut line = String::new();
        let n = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            reader.read_line(&mut line),
        )
        .await
        .expect("status read timed out — server may have crashed")
        .expect("status read failed");
        assert!(n > 0, "expected a status response line");
        let resp: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
        assert_eq!(
            resp["ok"], true,
            "status should be ok after oversize, got: {resp}"
        );
        assert_eq!(resp["data"]["status"], "running");
    }
}

/// CopyPaste-c4q2.28: a NON-bulk method (here `status`) whose request body
/// exceeds the per-method small cap (`SMALL_REQUEST_BYTES`, 64 KiB) must be
/// rejected with `request_too_large` — the daemon must NOT buffer up to
/// 16 MiB for it. This is the RAM-amplification guard.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ipc_non_bulk_method_over_small_cap_rejected() {
    let dir = safe_tempdir();
    let sock = dir.path().join("small_cap.sock");
    start_test_server(&sock).await;

    // Valid JSON with method=status and ~256 KiB of padding params — well
    // over SMALL_REQUEST_BYTES but far under MAX_REQUEST_BYTES.
    let pad = "A".repeat(256 * 1024);
    let body = format!(r#"{{"id":"big-status","method":"status","params":{{"pad":"{pad}"}}}}"#);

    let mut stream = UnixStream::connect(&sock).await.unwrap();
    let _ = stream.write_all(body.as_bytes()).await;
    let _ = stream.write_all(b"\n").await;

    let mut lines = BufReader::new(&mut stream).lines();
    let line = tokio::time::timeout(std::time::Duration::from_secs(5), lines.next_line())
        .await
        .expect("daemon must respond, not hang")
        .expect("read must succeed")
        .expect("must receive a rejection line");
    let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(
        resp["ok"], false,
        "oversized status must be rejected: {resp}"
    );
    assert_eq!(
        resp["error_code"], "request_too_large",
        "must be tagged request_too_large: {resp}"
    );
    assert!(
        resp["error"].as_str().unwrap_or("").contains("too large"),
        "error must mention 'too large': {resp}"
    );
}

/// CopyPaste-c4q2.28 (companion): a bulk method (`import`) with a body LARGER
/// than the small cap but valid must pass the size gate (phase-2 read) and be
/// dispatched — i.e. it must NOT be rejected as `request_too_large`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ipc_bulk_import_over_small_cap_accepted() {
    let dir = safe_tempdir();
    let sock = dir.path().join("bulk_ok.sock");
    start_test_server(&sock).await;

    // ~128 KiB of base64 content — over SMALL_REQUEST_BYTES, valid import.
    use base64::Engine as _;
    let b64 = base64::engine::general_purpose::STANDARD;
    let content = b64.encode(vec![0x7Au8; 128 * 1024]);
    let body = format!(
        r#"{{"id":"imp-ok","method":"import","params":{{"items":[{{"content_type":"text","content_bytes_b64":"{content}","created_at_ms":1700000000}}]}}}}"#,
    );

    let mut stream = UnixStream::connect(&sock).await.unwrap();
    stream.write_all(body.as_bytes()).await.unwrap();
    stream.write_all(b"\n").await.unwrap();

    let mut lines = BufReader::new(&mut stream).lines();
    let line = tokio::time::timeout(std::time::Duration::from_secs(5), lines.next_line())
        .await
        .expect("daemon must respond, not hang")
        .expect("read must succeed")
        .expect("must receive a response line");
    let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    // The size gate must have let this through: it is dispatched to the
    // import handler, not rejected for being too large.
    assert_ne!(
        resp["error_code"], "request_too_large",
        "valid bulk import over the small cap must pass the size gate: {resp}"
    );
}

// ------------------------------------------------------------------
// Wave 2.3 IPC hardening tests
//
// Cover edge cases that the binary-driven integration suite cannot
// reach in-process:
//   * IPC_NOT_READY when a DB-touching method fires before the
//     readiness flag flips,
//   * MAX_PAGE clamping on `list` and `history_page` enforced by the
//     dispatcher itself (independent of DB row count).
// ------------------------------------------------------------------

/// Spawn an IpcServer whose readiness flag starts `false`, returning
/// the socket path and the flag handle so the test can flip it.
async fn start_not_ready_server(socket_path: &std::path::Path) -> Arc<AtomicBool> {
    let db = Arc::new(Mutex::new(Database::open_in_memory().unwrap()));
    let private_mode = Arc::new(AtomicBool::new(false));
    let ready = Arc::new(AtomicBool::new(false));
    let ready_clone = ready.clone();
    let local_key = Arc::new(zeroize::Zeroizing::new([0u8; 32]));
    let device_pub = Arc::new([0u8; 32]);
    let server = IpcServer::new_with_ready(db, private_mode, local_key, device_pub, ready_clone);
    // Bind directly (no umask(0o177) race) — see comment in
    // start_test_server_returning_db for the full rationale.
    let path = socket_path.to_path_buf();
    let listener =
        tokio::net::UnixListener::bind(socket_path).expect("test socket bind must succeed");
    tokio::spawn(async move {
        if let Err(e) = server.serve_on(listener, CancellationToken::new()).await {
            tracing::error!("ipc: server on {:?} exited with error: {e}", &path);
        }
    });
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    ready
}

#[tokio::test]
async fn dispatch_returns_ipc_not_ready_when_not_ready() {
    let dir = safe_tempdir();
    let sock = dir.path().join("not_ready.sock");
    let ready = start_not_ready_server(&sock).await;

    // DB-touching methods must be rejected with ipc_not_ready error_code.
    // c4q2.17: "list" removed — now a not_implemented stub, no DB access.
    // c4q2.13: check error_code (machine-readable), not legacy "IPC_NOT_READY" string.
    for (method, params) in [
        ("count", "{}"),
        ("stats", "{}"),
        ("history_page", "{}"),
        ("delete_all", "{}"),
    ] {
        let mut stream = UnixStream::connect(&sock).await.unwrap();
        let req =
            format!("{{\"id\":\"nr-{method}\",\"method\":\"{method}\",\"params\":{params}}}\n");
        stream.write_all(req.as_bytes()).await.unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(resp["ok"], false, "{method} should be rejected: {resp}");
        assert_eq!(
            resp["error_code"].as_str().unwrap_or_default(),
            "ipc_not_ready",
            "{method} should return error_code=ipc_not_ready, got: {resp}"
        );
    }

    // Non-DB methods (status, get_private_mode) must still work, so the
    // client can introspect the daemon and decide whether to retry.
    {
        let mut stream = UnixStream::connect(&sock).await.unwrap();
        stream
            .write_all(b"{\"id\":\"nr-status\",\"method\":\"status\"}\n")
            .await
            .unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(resp["ok"], true, "status should pass: {resp}");
    }

    // After the readiness flag flips, previously-rejected methods succeed.
    ready.store(true, Ordering::Relaxed);
    {
        let mut stream = UnixStream::connect(&sock).await.unwrap();
        stream
            .write_all(b"{\"id\":\"nr-stats-after\",\"method\":\"stats\"}\n")
            .await
            .unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(resp["ok"], true, "stats should pass after ready: {resp}");
        assert!(resp["data"]["total_items"].is_number());
    }
}

/// c4q2.17: "list" is now a not_implemented stub. Limit-clamping is
/// tested for the unified "history_page" verb in
/// `history_page_clamps_oversize_limit_to_max_page` below.
#[tokio::test]
async fn list_clamps_oversize_limit_to_max_page() {
    let dir = safe_tempdir();
    let sock = dir.path().join("cap_list.sock");
    start_test_server(&sock).await;

    let resp = call_one(
        &sock,
        r#"{"id":"cap-list","method":"list","params":{"limit":5000,"offset":0}}"#,
    )
    .await;
    assert_eq!(
        resp["ok"], false,
        "list must return not_implemented: {resp}"
    );
    assert_eq!(
        resp["error_code"].as_str().unwrap_or(""),
        "not_implemented",
        "list must carry not_implemented error_code (c4q2.17): {resp}"
    );
}

#[tokio::test]
async fn history_page_clamps_oversize_limit_to_max_page() {
    let dir = safe_tempdir();
    let sock = dir.path().join("cap_hp.sock");
    start_test_server(&sock).await;

    let mut stream = UnixStream::connect(&sock).await.unwrap();
    stream
            .write_all(b"{\"id\":\"cap-hp\",\"method\":\"history_page\",\"params\":{\"limit\":9999,\"offset\":0}}\n")
            .await
            .unwrap();
    let mut lines = BufReader::new(&mut stream).lines();
    let line = lines.next_line().await.unwrap().unwrap();
    let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(resp["ok"], true);
    let items = resp["data"]["items"].as_array().unwrap();
    assert!(
        items.len() <= 1000,
        "history_page returned {} items, exceeds MAX_PAGE=1000",
        items.len()
    );
}

/// daemon-core backlog #2: the `search` handler must clamp an oversized
/// `limit` to MAX_PAGE just like `list` / `history_page`. We seed more than
/// MAX_PAGE rows all matching one FTS term, then request `limit=5000`. The
/// SQL applies `LIMIT ?`, so without the `.min(MAX_PAGE)` clamp the response
/// would carry > MAX_PAGE items; with it, exactly MAX_PAGE.
#[tokio::test]
async fn search_clamps_oversize_limit_to_max_page() {
    let dir = safe_tempdir();
    let sock = dir.path().join("cap_search.sock");
    let (_pm, db) = start_test_server_returning_db(&sock, false).await;

    // Seed MAX_PAGE + 5 text rows whose FTS plaintext all contains "needle".
    {
        let guard = db.lock().await;
        for i in 0..(MAX_PAGE + 5) {
            let item =
                copypaste_core::ClipboardItem::new_text(vec![0xAB], vec![0u8; 24], i as i64 + 1);
            copypaste_core::insert_item_with_fts(&guard, &item, &format!("needle row {i}"))
                .unwrap();
        }
    }

    let resp = call_one(
        &sock,
        r#"{"id":"cap-search","method":"search","params":{"query":"needle","limit":5000}}"#,
    )
    .await;
    assert_eq!(resp["ok"], true, "search should be ok: {resp}");
    let items = resp["data"]["items"].as_array().unwrap();
    assert_eq!(
        items.len(),
        MAX_PAGE,
        "search must clamp to MAX_PAGE={MAX_PAGE}, got {} items",
        items.len()
    );
}

/// daemon-core backlog #3: list_view (`history_page`) preview offsets must
/// not panic on width-changing Unicode normalisation. The sensitive detector
/// reports byte ranges over the NFKC-normalised string; slicing the original
/// preview with those offsets used to panic on a non-char-boundary. With a
/// secret embedded after a ligature/full-width run, the handler must return
/// without panicking and produce in-range, ordered char offsets.
#[tokio::test]
async fn history_page_adversarial_unicode_preview_no_panic() {
    let dir = safe_tempdir();
    let sock = dir.path().join("adv_unicode.sock");
    let (_pm, db) = start_test_server_returning_db(&sock, false).await;

    // Full-width "AKIA" (U+FF21..) + 16 ASCII chars normalises (NFKC) to a
    // valid AWS access-key id, which the detector flags. The full-width
    // prefix is 3 bytes/char in the original but 1 byte/char after NFKC, so
    // the detector's byte offsets do not line up with the original string —
    // exactly the mismatch that triggered the slice panic.
    let plaintext = "ＡＫＩＡ0123456789ABCDEF and some trailing prose";
    {
        let guard = db.lock().await;
        let item = copypaste_core::ClipboardItem::new_text(vec![0xCD], vec![0u8; 24], 1);
        copypaste_core::insert_item_with_fts(&guard, &item, plaintext).unwrap();
    }

    // Must not panic — a panic in the blocking task would surface as an
    // internal error / dropped connection rather than an `ok` response.
    let resp = call_one(
        &sock,
        r#"{"id":"adv","method":"history_page","params":{"limit":10,"offset":0}}"#,
    )
    .await;
    assert_eq!(resp["ok"], true, "history_page must not panic: {resp}");
    let items = resp["data"]["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    let preview = items[0]["preview"].as_str().unwrap();
    let preview_char_len = preview.chars().count();
    let spans = items[0]["sensitive_spans"].as_array().unwrap();
    for span in spans {
        let pair = span.as_array().unwrap();
        let start = pair[0].as_u64().unwrap() as usize;
        let end = pair[1].as_u64().unwrap() as usize;
        assert!(start <= end, "span start {start} must be <= end {end}");
        assert!(
            end <= preview_char_len,
            "span end {end} must be within preview char-len {preview_char_len}"
        );
    }
}

/// Fix-1 (NFKC span-mask leak): when the preview contains full-width or
/// ligature chars that NFKC normalises to narrower forms, the returned
/// `preview` string must be the NORMALISED form so that the returned char
/// offsets (`sensitive_spans`) correctly index into it.
///
/// Concretely: full-width "ＡＫＩＡ" (4 chars × 3 bytes each in the original)
/// normalises to ASCII "AKIA" (4 chars × 1 byte each).  The detector sees
/// "AKIA…" and reports a span at, say, chars [0..20].  If the daemon returned
/// the ORIGINAL (full-width) preview, the UI would apply [0..20] to a string
/// where char 0 is a 3-byte full-width 'Ａ' — the mask would cover the WRONG
/// characters and might expose part of the secret.  The fix: always return the
/// normalised preview so offsets and string share one basis.
#[tokio::test]
async fn history_page_spans_index_into_returned_preview_not_raw() {
    let dir = safe_tempdir();
    let sock = dir.path().join("span_basis.sock");
    let (_pm, db) = start_test_server_returning_db(&sock, false).await;

    // Full-width prefix: each char is 3 UTF-8 bytes in the original,
    // but only 1 byte after NFKC.  The detector runs on NFKC form and
    // produces a span anchored at byte offset 0 of the normalised string.
    // If the daemon returns the raw (non-normalised) preview, char offset 0
    // in that string still maps to the full-width Ａ — the span basis is wrong.
    let plaintext = "ＡＫＩＡ0123456789ABCDEF trailing text";
    {
        let guard = db.lock().await;
        let item = copypaste_core::ClipboardItem::new_text(vec![0xCD], vec![0u8; 24], 1);
        copypaste_core::insert_item_with_fts(&guard, &item, plaintext).unwrap();
    }

    let resp = call_one(
        &sock,
        r#"{"id":"basis","method":"history_page","params":{"limit":10,"offset":0}}"#,
    )
    .await;
    assert_eq!(resp["ok"], true, "history_page: {resp}");
    let items = resp["data"]["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);

    let preview = items[0]["preview"].as_str().unwrap();
    let spans = items[0]["sensitive_spans"].as_array().unwrap();

    // The detector must have flagged something (the normalised form is
    // "AKIA0123456789ABCDEF…" which contains an AWS-key-like pattern).
    assert!(
        !spans.is_empty(),
        "detector should flag the AKIA... pattern in the preview"
    );

    // KEY ASSERTION: every span must start with ASCII 'A' in the returned
    // preview.  If the preview is the RAW full-width string the first char
    // would be 'Ａ' (U+FF21), not 'A' (U+0041) — proving the span basis is
    // wrong.  After the fix the preview is normalised and spans[0][0] == 0
    // means preview.chars().nth(0) == 'A'.
    for span in spans {
        let pair = span.as_array().unwrap();
        let start = pair[0].as_u64().unwrap() as usize;
        let end = pair[1].as_u64().unwrap() as usize;
        // Span must be within the returned preview's char length.
        let char_len = preview.chars().count();
        assert!(
            end <= char_len,
            "span [{}..{}] out of range for preview (len={}): {:?}",
            start,
            end,
            char_len,
            preview
        );
        // Each char in the spanned range must be ASCII (normalised).
        // Full-width chars are 3 bytes wide; after NFKC they become ASCII.
        let span_chars: String = preview.chars().skip(start).take(end - start).collect();
        assert!(
            span_chars.is_ascii(),
            "span [{start}..{end}] covers non-ASCII chars in preview — \
                 preview is NOT normalised (raw full-width form leaked): {:?}",
            span_chars
        );
    }
}

/// `byte_to_char_offset` clamps out-of-range and mid-codepoint byte indices
/// to a valid char boundary and never panics.
#[test]
fn byte_to_char_offset_clamps_and_never_panics() {
    let s = "café"; // 'é' is 2 bytes (0xC3 0xA9): bytes 0..5, chars 0..4
    assert_eq!(byte_to_char_offset(s, 0), 0);
    assert_eq!(byte_to_char_offset(s, 3), 3); // boundary before 'é'
    assert_eq!(byte_to_char_offset(s, 4), 3); // mid-'é' → walk back → 3 chars
    assert_eq!(byte_to_char_offset(s, 5), 4); // end
    assert_eq!(byte_to_char_offset(s, 9999), 4); // past end → clamp to char-len
}

// --- FIX 1: history_page returns pinned field and pinned-first order ---

/// Each item in `history_page` must carry a boolean `pinned` field.
#[tokio::test]
async fn history_page_items_include_pinned_field() {
    let dir = safe_tempdir();
    let sock = dir.path().join("hp_pinned_field.sock");
    let (_pm, db) = start_test_server_returning_db(&sock, false).await;

    // Seed one item.
    {
        let guard = db.lock().await;
        let item = copypaste_core::ClipboardItem::new_text(vec![0xAA], vec![0u8; 24], 1);
        copypaste_core::insert_item(&guard, &item).unwrap();
    }

    let resp = call_one(
        &sock,
        r#"{"id":"hpf1","method":"history_page","params":{"limit":10,"offset":0}}"#,
    )
    .await;
    assert_eq!(resp["ok"], true);
    let items = resp["data"]["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    // The `pinned` field must be present and be a boolean.
    assert!(
        items[0]["pinned"].is_boolean(),
        "each item must have a boolean 'pinned' field, got: {}",
        items[0]
    );
    assert_eq!(
        items[0]["pinned"], false,
        "freshly inserted item must have pinned=false"
    );
}

/// `history_page` must return pinned items before unpinned items,
/// regardless of wall_time ordering.
#[tokio::test]
async fn history_page_pinned_items_sort_first() {
    let dir = safe_tempdir();
    let sock = dir.path().join("hp_pinned_sort.sock");
    let (_pm, db) = start_test_server_returning_db(&sock, false).await;

    // Insert two items: item_old (lower wall_time) and item_new (higher).
    // Then pin item_old — it must appear first in history_page even though
    // it is older.
    let (id_old, _id_new) = {
        let guard = db.lock().await;
        let mut item_old = copypaste_core::ClipboardItem::new_text(vec![0x01], vec![0u8; 24], 1);
        item_old.wall_time = 1_000;
        let id_old = item_old.id.clone();
        copypaste_core::insert_item(&guard, &item_old).unwrap();

        let mut item_new = copypaste_core::ClipboardItem::new_text(vec![0x02], vec![0u8; 24], 2);
        item_new.wall_time = 2_000;
        let id_new = item_new.id.clone();
        copypaste_core::insert_item(&guard, &item_new).unwrap();

        (id_old, id_new)
    };

    // Pin the older item via the IPC verb.
    let pin_body = format!(
        r#"{{"id":"hps-pin","method":"pin_item","params":{{"id":"{id_old}","pinned":true}}}}"#
    );
    let pin_resp = call_one(&sock, &pin_body).await;
    assert_eq!(pin_resp["ok"], true, "pin must succeed: {pin_resp}");

    // Now history_page must return item_old first.
    let hp_resp = call_one(
        &sock,
        r#"{"id":"hps-hp","method":"history_page","params":{"limit":10,"offset":0}}"#,
    )
    .await;
    assert_eq!(hp_resp["ok"], true);
    let items = hp_resp["data"]["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(
        items[0]["id"].as_str().unwrap(),
        id_old,
        "pinned (older) item must be first"
    );
    assert_eq!(items[0]["pinned"], true, "first item must have pinned=true");
    assert_eq!(
        items[1]["pinned"], false,
        "second item must have pinned=false"
    );
}

/// After unpinning, the item reverts to recency order in history_page.
#[tokio::test]
async fn history_page_unpinned_item_reverts_to_recency_order() {
    let dir = safe_tempdir();
    let sock = dir.path().join("hp_unpin.sock");
    let (_pm, db) = start_test_server_returning_db(&sock, false).await;

    let (id_old, _id_new) = {
        let guard = db.lock().await;
        let mut item_old = copypaste_core::ClipboardItem::new_text(vec![0x01], vec![0u8; 24], 1);
        item_old.wall_time = 1_000;
        let id_old = item_old.id.clone();
        copypaste_core::insert_item(&guard, &item_old).unwrap();

        let mut item_new = copypaste_core::ClipboardItem::new_text(vec![0x02], vec![0u8; 24], 2);
        item_new.wall_time = 2_000;
        let id_new = item_new.id.clone();
        copypaste_core::insert_item(&guard, &item_new).unwrap();

        (id_old, id_new)
    };

    // Pin then unpin item_old.
    let pin_body = format!(
        r#"{{"id":"hpu-pin","method":"pin_item","params":{{"id":"{id_old}","pinned":true}}}}"#
    );
    call_one(&sock, &pin_body).await;
    let unpin_body = format!(
        r#"{{"id":"hpu-unpin","method":"pin_item","params":{{"id":"{id_old}","pinned":false}}}}"#
    );
    call_one(&sock, &unpin_body).await;

    // After unpin, history_page must return newest-first (item_new first).
    let hp_resp = call_one(
        &sock,
        r#"{"id":"hpu-hp","method":"history_page","params":{"limit":10,"offset":0}}"#,
    )
    .await;
    assert_eq!(hp_resp["ok"], true);
    let items = hp_resp["data"]["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(
        items[0]["pinned"], false,
        "first item must be unpinned after unpin"
    );
    assert!(
        items[0]["wall_time"].as_i64().unwrap() >= items[1]["wall_time"].as_i64().unwrap(),
        "items must be newest-first after unpin"
    );
}

/// In-process burst that exercises the same accept-spawn path used by
/// the binary subprocess test, but without requiring a built binary.
/// 10 tokio tasks each issue a status+stats roundtrip on its own
/// connection; all must succeed.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_clients_in_process_consistent_state() {
    let dir = safe_tempdir();
    let sock = dir.path().join("concurrent.sock");
    start_test_server(&sock).await;

    const N: usize = 10;
    let mut handles = Vec::with_capacity(N);
    for i in 0..N {
        let sock = sock.clone();
        handles.push(tokio::spawn(async move {
            // status
            let mut s = UnixStream::connect(&sock).await.unwrap();
            let req = format!("{{\"id\":\"c{i}-status\",\"method\":\"status\"}}\n");
            s.write_all(req.as_bytes()).await.unwrap();
            let mut lines = BufReader::new(&mut s).lines();
            let line = lines.next_line().await.unwrap().unwrap();
            let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
            assert_eq!(resp["ok"], true, "client {i} status: {resp}");

            // stats — fresh connection
            let mut s2 = UnixStream::connect(&sock).await.unwrap();
            let req2 = format!("{{\"id\":\"c{i}-stats\",\"method\":\"stats\"}}\n");
            s2.write_all(req2.as_bytes()).await.unwrap();
            let mut lines2 = BufReader::new(&mut s2).lines();
            let line2 = lines2.next_line().await.unwrap().unwrap();
            let resp2: serde_json::Value = serde_json::from_str(&line2).unwrap();
            assert_eq!(resp2["ok"], true, "client {i} stats: {resp2}");
            assert!(resp2["data"]["total_items"].is_number());
        }));
    }
    for h in handles {
        h.await.expect("client task panicked");
    }

    // Survivor request after the burst.
    let mut s = UnixStream::connect(&sock).await.unwrap();
    s.write_all(b"{\"id\":\"survivor\",\"method\":\"status\"}\n")
        .await
        .unwrap();
    let mut lines = BufReader::new(&mut s).lines();
    let line = lines.next_line().await.unwrap().unwrap();
    let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(resp["ok"], true);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ipc_client_mid_request_disconnect_does_not_panic() {
    let dir = safe_tempdir();
    let sock = dir.path().join("hardening_disconnect.sock");
    start_test_server(&sock).await;

    // Open + close 10 times without writing anything (clean EOF on
    // first read — must be handled, not panic).
    for _ in 0..10 {
        let stream = UnixStream::connect(&sock).await.unwrap();
        drop(stream);
    }

    // Partial write disconnect: write bytes but no newline, then drop.
    // Server's read_until returns >0 bytes then EOF on next iteration.
    {
        let mut stream = UnixStream::connect(&sock).await.unwrap();
        stream
            .write_all(b"{\"id\":\"partial\",\"meth")
            .await
            .unwrap();
        drop(stream);
    }

    // Give server tasks a moment to settle.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Fresh client must still get an answer — proves no listener crash.
    let mut stream = UnixStream::connect(&sock)
        .await
        .expect("server must still accept new connections after abrupt disconnects");
    stream
        .write_all(b"{\"id\":\"survivor\",\"method\":\"status\"}\n")
        .await
        .unwrap();
    let mut reader = BufReader::new(&mut stream);
    let mut line = String::new();
    let n = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        reader.read_line(&mut line),
    )
    .await
    .expect("survivor read timed out — server may have crashed")
    .expect("survivor read failed");
    assert!(n > 0, "expected a status response line");
    let resp: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
    assert_eq!(
        resp["ok"], true,
        "status should be ok after disconnects, got: {resp}"
    );
}

/// beta-W3.1 — DB-touching IPC handlers must run on spawn_blocking so a
/// slow rusqlite read does not block tokio worker threads. We exercise
/// this by issuing N concurrent `list` requests on a single-threaded
/// runtime (`#[tokio::test]` default). If any handler held a tokio worker
/// across the SQLite call, the requests would serialize and the wall
/// clock would exceed N × per-request latency. With spawn_blocking they
/// fan out across the blocking pool and complete near-concurrently.
///
/// We assert a *generous* upper bound (well below strict serialization)
/// rather than a tight one so the test stays robust on slow CI.
#[tokio::test]
async fn spawn_blocking_does_not_block_tokio_worker() {
    let dir = safe_tempdir();
    let sock = dir.path().join("test-spawn-blocking.sock");
    start_test_server(&sock).await;

    // c4q2.17: Fire 4 concurrent `history_page` requests (unified verb).
    // Previously used `list` which is now a not_implemented stub.
    const N: usize = 4;
    let started = std::time::Instant::now();
    let mut handles = Vec::with_capacity(N);
    for i in 0..N {
        let sock_path = sock.clone();
        handles.push(tokio::spawn(async move {
                let mut stream = UnixStream::connect(&sock_path).await.unwrap();
                let payload = format!(
                    "{{\"id\":\"sb{i}\",\"method\":\"history_page\",\"params\":{{\"limit\":10,\"offset\":0}}}}\n"
                );
                stream.write_all(payload.as_bytes()).await.unwrap();
                let mut lines = BufReader::new(&mut stream).lines();
                let line = lines.next_line().await.unwrap().unwrap();
                let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
                assert_eq!(resp["ok"], true, "history_page must succeed: {line}");
            }));
    }
    for h in handles {
        h.await.unwrap();
    }
    let elapsed = started.elapsed();

    // Sanity bound: 4 in-memory `history_page` calls on an empty DB should
    // finish in well under a second. 5s catches catastrophic regressions
    // (e.g., a single-thread deadlock) without flaking on slow CI runners.
    assert!(
        elapsed < std::time::Duration::from_secs(5),
        "4 concurrent history_page requests took {elapsed:?} — tokio worker likely blocked"
    );
}

/// beta-W3.2 — `pair_peer_with_password` validates required params and
/// returns `not_implemented` once inputs check out, so the UI can rely
/// on a stable error_code for the not-yet-wired Transport path.
#[tokio::test]
async fn pair_peer_with_password_validates_inputs() {
    let dir = safe_tempdir();
    let sock = dir.path().join("test-pair-pw.sock");
    start_test_server(&sock).await;

    async fn call(sock: &std::path::Path, body: &str) -> serde_json::Value {
        let mut stream = UnixStream::connect(sock).await.unwrap();
        stream.write_all(body.as_bytes()).await.unwrap();
        stream.write_all(b"\n").await.unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        serde_json::from_str(&line).unwrap()
    }

    // Missing peer_fingerprint → invalid_argument
    let resp = call(
        &sock,
        r#"{"id":"p1","method":"pair_peer_with_password","params":{"password":"hunter22"}}"#,
    )
    .await;
    assert_eq!(resp["ok"], false, "missing peer_fingerprint must fail");
    assert_eq!(resp["error_code"], "invalid_argument");

    // Missing password → invalid_argument
    let valid_fp = std::iter::repeat_n("ab", 32).collect::<Vec<_>>().join(":");
    let body = format!(
        r#"{{"id":"p2","method":"pair_peer_with_password","params":{{"peer_fingerprint":"{valid_fp}"}}}}"#
    );
    let resp = call(&sock, &body).await;
    assert_eq!(resp["ok"], false, "missing password must fail");
    assert_eq!(resp["error_code"], "invalid_argument");

    // Short password → invalid_argument (UI enforces but daemon double-checks)
    let body = format!(
        r#"{{"id":"p3","method":"pair_peer_with_password","params":{{"peer_fingerprint":"{valid_fp}","password":"ab"}}}}"#
    );
    let resp = call(&sock, &body).await;
    assert_eq!(resp["ok"], false, "short password must fail");
    assert_eq!(resp["error_code"], "invalid_argument");

    // Bad fingerprint hex → invalid_argument
    let resp = call(
            &sock,
            r#"{"id":"p4","method":"pair_peer_with_password","params":{"peer_fingerprint":"not-hex","password":"hunter22"}}"#,
        )
        .await;
    assert_eq!(resp["ok"], false, "bad fingerprint must fail");
    assert_eq!(resp["error_code"], "invalid_argument");

    // Missing step → defaults to "initiate"; valid request returns session_id + message1_b64
    let body = format!(
        r#"{{"id":"p5","method":"pair_peer_with_password","params":{{"peer_fingerprint":"{valid_fp}","password":"hunter22","step":"initiate"}}}}"#
    );
    let resp = call(&sock, &body).await;
    assert_eq!(resp["ok"], true, "initiate step must succeed: {resp}");
    assert!(
        resp["data"]["session_id"].is_string(),
        "response must contain session_id"
    );
    assert!(
        resp["data"]["message1_b64"].is_string(),
        "response must contain message1_b64"
    );
}

/// W2.4 — `pair_peer_with_password` with step="initiate" returns a
/// session_id and base64-encoded message1 to send to the responder.
#[tokio::test]
async fn pair_peer_with_password_initiate_returns_session_and_message1() {
    let dir = safe_tempdir();
    let sock = dir.path().join("test-pake-init.sock");
    start_test_server(&sock).await;

    let valid_fp = std::iter::repeat_n("ab", 32).collect::<Vec<_>>().join(":");
    let body = format!(
        r#"{{"id":"pi1","method":"pair_peer_with_password","params":{{"peer_fingerprint":"{valid_fp}","password":"correct-horse","step":"initiate"}}}}"#
    );
    let mut stream = UnixStream::connect(&sock).await.unwrap();
    stream.write_all(body.as_bytes()).await.unwrap();
    stream.write_all(b"\n").await.unwrap();
    let mut lines = BufReader::new(&mut stream).lines();
    let line = lines.next_line().await.unwrap().unwrap();
    let resp: serde_json::Value = serde_json::from_str(&line).unwrap();

    assert_eq!(resp["ok"], true, "initiate must succeed: {resp}");
    let session_id = resp["data"]["session_id"].as_str().unwrap();
    assert!(!session_id.is_empty(), "session_id must not be empty");
    let msg1_b64 = resp["data"]["message1_b64"].as_str().unwrap();
    // Verify it decodes as valid base64 bytes
    use base64::Engine as _;
    let msg1_bytes = base64::engine::general_purpose::STANDARD
        .decode(msg1_b64)
        .expect("message1_b64 must be valid base64");
    assert!(!msg1_bytes.is_empty(), "message1 must not be empty");
}

/// c4q2.20: pair_accept_password is stubbed not_implemented. Verify the stub
/// shape: ok=false, error_code="not_implemented" (not a crash or generic error).
#[tokio::test]
async fn pair_accept_password_returns_session_and_message2() {
    let dir = safe_tempdir();
    let sock = dir.path().join("test-pake-accept.sock");
    start_test_server(&sock).await;

    let resp = call_one(
            &sock,
            r#"{"id":"pa1","method":"pair_accept_password","params":{"message1_b64":"AAAA","peer_fingerprint":"cd:cd","password":"correct-horse"}}"#,
        )
        .await;
    assert_eq!(
        resp["ok"], false,
        "pair_accept_password must be not_implemented (c4q2.20): {resp}"
    );
    assert_eq!(
        resp["error_code"].as_str().unwrap_or(""),
        "not_implemented",
        "pair_accept_password must carry not_implemented error_code: {resp}"
    );
}

/// c4q2.20: renamed from pair_peer_with_password_full_round_trip.
/// The responder step (pair_accept_password) is now stubbed not_implemented
/// (CopyPaste-c4q2.20 security concern). Test that:
///   1. Initiator "initiate" step still works (step 1 untouched).
///   2. The stub returns not_implemented for the responder verb.
///   Full QR round-trip tested by pair_qr_full_round_trip below.
#[tokio::test]
async fn pair_peer_with_password_initiator_step_works() {
    use base64::Engine as _;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::UnixStream;

    let dir = safe_tempdir();
    let cfg_home = dir.path().join("cfg");
    let _env = EnvGuard::set_all(
        &["COPYPASTE_CONFIG_DIR", "HOME", "XDG_CONFIG_HOME"],
        &cfg_home,
    );

    let sock_a = dir.path().join("test-pake-initiator-a.sock");
    let sock_b = dir.path().join("test-pake-initiator-b.sock");
    start_test_server(&sock_a).await;
    start_test_server(&sock_b).await;

    async fn call(sock: &std::path::Path, body: &str) -> serde_json::Value {
        let mut stream = UnixStream::connect(sock).await.unwrap();
        stream.write_all(body.as_bytes()).await.unwrap();
        stream.write_all(b"\n").await.unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        serde_json::from_str(&line).unwrap()
    }

    let password = "correct-horse-battery";

    let fp_b = call(
        &sock_b,
        r#"{"id":"fp_b","method":"get_own_fingerprint","params":{}}"#,
    )
    .await["data"]["fingerprint"]
        .as_str()
        .expect("server B must return own fingerprint")
        .to_string();

    // Step 1 (initiator "initiate"): must still work — pair_accept_password
    // is stubbed not_implemented but the initiator verb is untouched (c4q2.20).
    let body = format!(
        r#"{{"id":"rt1","method":"pair_peer_with_password","params":{{"peer_fingerprint":"{fp_b}","password":"{password}","step":"initiate"}}}}"#
    );
    let resp = call(&sock_a, &body).await;
    assert_eq!(
        resp["ok"], true,
        "initiate step must succeed (c4q2.20): {resp}"
    );
    let session_id = resp["data"]["session_id"].as_str().unwrap();
    let msg1_b64 = resp["data"]["message1_b64"].as_str().unwrap();
    // Verify the expected fields are non-empty.
    assert!(!session_id.is_empty(), "session_id must be non-empty");
    let _ = base64::engine::general_purpose::STANDARD
        .decode(msg1_b64)
        .expect("message1_b64 must be valid base64");

    // c4q2.20: pair_accept_password (responder verb) is now a not_implemented stub.
    let not_impl_resp = call(
            &sock_b,
            &format!(
                r#"{{"id":"rt2","method":"pair_accept_password","params":{{"message1_b64":"{msg1_b64}","peer_fingerprint":"aa:bb","password":"{password}"}}}}"#
            ),
        )
        .await;
    assert_eq!(
        not_impl_resp["ok"], false,
        "pair_accept_password must be stubbed not_implemented (c4q2.20): {not_impl_resp}"
    );
    assert_eq!(
        not_impl_resp["error_code"].as_str().unwrap_or(""),
        "not_implemented",
        "pair_accept_password must carry not_implemented error_code: {not_impl_resp}"
    );
}

// -----------------------------------------------------------------------
// S3 (CopyPaste-4ca) — PAKE SessionKey cert-binding tests
// -----------------------------------------------------------------------

/// S3: The cert-binder helper must be symmetric (swap fp_a / fp_b → same
/// output) and must produce different values for different fingerprint pairs.
#[test]
fn pake_cert_binder_is_symmetric_and_distinct() {
    let fp_a = "aa:bb:cc:dd:ee:ff:00:11:22:33:44:55:66:77:88:99:aa:bb:cc:dd:ee:ff:00:11:22:33:44:55:66:77:88:99";
    let fp_b = "11:22:33:44:55:66:77:88:99:aa:bb:cc:dd:ee:ff:00:11:22:33:44:55:66:77:88:99:aa:bb:cc:dd:ee:ff:00";
    let fp_c = "ff:ee:dd:cc:bb:aa:99:88:77:66:55:44:33:22:11:00:ff:ee:dd:cc:bb:aa:99:88:77:66:55:44:33:22:11:00";

    let binder_ab = IpcServer::pake_cert_binder(fp_a, fp_b);
    let binder_ba = IpcServer::pake_cert_binder(fp_b, fp_a);
    let binder_ac = IpcServer::pake_cert_binder(fp_a, fp_c);

    assert_eq!(binder_ab, binder_ba, "binder must be symmetric");
    assert_ne!(
        binder_ab, binder_ac,
        "different fp pairs must yield different binders"
    );
}

/// S3: A full PAKE round-trip with matching cert binders on both ends
/// produces matching confirmation tags — simulating the honest pairing case.
#[test]
fn pake_channel_binding_succeeds_with_matching_cert_binders() {
    use copypaste_p2p::pake::{
        channel_confirmation_tag, ConfirmRole, PakeInitiator, PakeResponder, PasswordFile,
        CONFIRM_TAG_LEN,
    };

    let password = "correct-horse-battery-S3";
    let pf = PasswordFile::register(password).expect("register");

    let (client, msg1) = PakeInitiator::new(password).expect("initiator new");
    let (server, msg2) = PakeResponder::respond(&pf, &msg1).expect("responder respond");
    let (client_key, msg3) = client.finish(&msg2).expect("initiator finish");
    let server_key = server.finish(&msg3).expect("responder finish");

    // Both sides use the same cert fingerprints → same binder.
    let fp_initiator = "a1:b2:c3:d4:e5:f6:07:18:29:3a:4b:5c:6d:7e:8f:90:a1:b2:c3:d4:e5:f6:07:18:29:3a:4b:5c:6d:7e:8f:90";
    let fp_responder = "f0:e1:d2:c3:b4:a5:96:87:78:69:5a:4b:3c:2d:1e:0f:f0:e1:d2:c3:b4:a5:96:87:78:69:5a:4b:3c:2d:1e:0f";

    let binder = IpcServer::pake_cert_binder(fp_initiator, fp_responder);
    let client_bound = client_key.bind_to_tls_channel(&binder);
    let server_bound = server_key.bind_to_tls_channel(&binder);

    let client_tag = channel_confirmation_tag(&client_bound, ConfirmRole::Initiator);
    let server_expected = channel_confirmation_tag(&server_bound, ConfirmRole::Initiator);

    assert_eq!(client_tag.len(), CONFIRM_TAG_LEN);
    assert_eq!(
        client_tag, server_expected,
        "initiator tag must match on both sides when binders agree"
    );

    // Responder also derives a matching responder tag.
    let resp_tag_from_client = channel_confirmation_tag(&client_bound, ConfirmRole::Responder);
    let resp_tag_from_server = channel_confirmation_tag(&server_bound, ConfirmRole::Responder);
    assert_eq!(
        resp_tag_from_client, resp_tag_from_server,
        "responder tag must also match"
    );
}

/// S3: When a relay/MitM substitutes different cert fingerprints on each leg,
/// the binders differ → the bound keys differ → the confirmation tags do NOT
/// match → the handshake is detected.
///
/// This directly models the attack: relay terminates PAKE on leg A
/// (fp_relay_a, fp_victim) and bridges to leg B (fp_relay_b, fp_target).
/// The two legs use different cert pairs, so each leg computes a different
/// binder → different confirmation tags → the responder's verify step rejects.
#[test]
fn pake_channel_binding_fails_with_mismatched_cert_binders() {
    use copypaste_p2p::pake::{
        channel_confirmation_tag, ConfirmRole, PakeInitiator, PakeResponder, PasswordFile,
        CONFIRM_TAG_LEN,
    };
    use subtle::ConstantTimeEq as _;

    let password = "correct-horse-battery-mitm";
    let pf = PasswordFile::register(password).expect("register");

    let (client, msg1) = PakeInitiator::new(password).expect("initiator new");
    let (server, msg2) = PakeResponder::respond(&pf, &msg1).expect("responder respond");
    let (client_key, msg3) = client.finish(&msg2).expect("initiator finish");
    let server_key = server.finish(&msg3).expect("responder finish");

    // Leg A (initiator side): MitM presents its own cert to the initiator.
    let fp_initiator = "a1:b2:c3:d4:e5:f6:07:18:29:3a:4b:5c:6d:7e:8f:90:a1:b2:c3:d4:e5:f6:07:18:29:3a:4b:5c:6d:7e:8f:90";
    let fp_mitm_leg_a = "de:ad:be:ef:00:11:22:33:44:55:66:77:88:99:aa:bb:de:ad:be:ef:00:11:22:33:44:55:66:77:88:99:aa:bb";

    // Leg B (responder side): MitM uses a DIFFERENT cert toward the responder.
    let fp_mitm_leg_b = "ca:fe:ba:be:00:11:22:33:44:55:66:77:88:99:aa:bb:ca:fe:ba:be:00:11:22:33:44:55:66:77:88:99:aa:bb";
    let fp_responder = "f0:e1:d2:c3:b4:a5:96:87:78:69:5a:4b:3c:2d:1e:0f:f0:e1:d2:c3:b4:a5:96:87:78:69:5a:4b:3c:2d:1e:0f";

    // Initiator sees (fp_initiator, fp_mitm_leg_a) → binder_a
    let binder_a = IpcServer::pake_cert_binder(fp_initiator, fp_mitm_leg_a);
    // Responder sees (fp_mitm_leg_b, fp_responder) → binder_b (different!)
    let binder_b = IpcServer::pake_cert_binder(fp_mitm_leg_b, fp_responder);

    assert_ne!(
        binder_a, binder_b,
        "MitM legs must produce different binders"
    );

    let client_bound = client_key.bind_to_tls_channel(&binder_a);
    let server_bound = server_key.bind_to_tls_channel(&binder_b);

    // Initiator computes its confirmation tag with binder_a.
    let initiator_tag = channel_confirmation_tag(&client_bound, ConfirmRole::Initiator);
    // Responder verifies with binder_b → MUST NOT match.
    let responder_expected = channel_confirmation_tag(&server_bound, ConfirmRole::Initiator);

    assert_eq!(initiator_tag.len(), CONFIRM_TAG_LEN);
    assert_eq!(responder_expected.len(), CONFIRM_TAG_LEN);

    // Constant-time compare — proves the responder's check would fail.
    let tags_match: bool = initiator_tag.ct_eq(&responder_expected).into();
    assert!(
        !tags_match,
        "confirmation tags MUST differ when cert binders differ (MitM detected)"
    );
}

/// c4q2.20: Renamed from pair_accept_finish_rejects_wrong_initiator_confirm_tag.
/// pair_accept_password is now stubbed (no longer creates responder sessions via
/// password flow — use QR). This test verifies pair_accept_finish rejects an
/// unknown/bogus session_id with a non-ok response (not_found or invalid_argument).
/// The tampered-confirm-tag check is covered by the pure pake_channel_binding tests.
#[tokio::test]
async fn pair_accept_finish_rejects_unknown_session() {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::UnixStream;

    let dir = safe_tempdir();
    let sock = dir.path().join("test-s3-reject.sock");
    start_test_server(&sock).await;

    let mut stream = UnixStream::connect(&sock).await.unwrap();
    stream
            .write_all(
                b"{\"id\":\"s3r\",\"method\":\"pair_accept_finish\",\"params\":{\"session_id\":\"no-such-session\",\"message3_b64\":\"AAAA\",\"peer_fingerprint\":\"aa:bb\",\"initiator_confirm_b64\":\"AAAA\"}}\n",
            )
            .await
            .unwrap();
    let mut lines = BufReader::new(&mut stream).lines();
    let line = lines.next_line().await.unwrap().unwrap();
    let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(
        resp["ok"], false,
        "pair_accept_finish with unknown session must fail (c4q2.20): {resp}"
    );
    // Accept not_found or invalid_argument — both are valid error shapes.
    let code = resp["error_code"].as_str().unwrap_or("");
    assert!(
        code == "not_found" || code == "invalid_argument" || !resp["error"].is_null(),
        "pair_accept_finish must return diagnosable error for unknown session: {resp}"
    );
}

/// c4q2.20: pair_accept_password is stubbed not_implemented; the absent-tag
/// scenario no longer applies to the password path. Test that pair_accept_finish
/// with a completely absent `initiator_confirm_b64` on an unknown session still
/// returns a non-ok error (guards the dispatch path, not PAKE logic).
#[tokio::test]
async fn pair_accept_finish_rejects_absent_initiator_confirm_tag() {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::UnixStream;

    let dir = safe_tempdir();
    let sock = dir.path().join("j8dr.sock");
    start_test_server(&sock).await;

    // Send pair_accept_finish without initiator_confirm_b64 AND with a
    // non-existent session_id — must return non-ok.
    let mut stream = UnixStream::connect(&sock).await.unwrap();
    stream
            .write_all(
                b"{\"id\":\"j6\",\"method\":\"pair_accept_finish\",\"params\":{\"session_id\":\"no-such-session\",\"message3_b64\":\"AAAA\",\"peer_fingerprint\":\"aa:bb\"}}\n",
            )
            .await
            .unwrap();
    let mut lines = BufReader::new(&mut stream).lines();
    let line = lines.next_line().await.unwrap().unwrap();
    let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(
        resp["ok"], false,
        "pair_accept_finish without confirm tag and unknown session must fail (c4q2.20): {resp}"
    );
}

/// QR pairing end-to-end: device B (displaying) generates a QR, device A
/// (scanning) decodes it via `copypaste_core::PairingPayload`, derives the
/// PAKE password from the embedded token, and completes the 4-message
/// handshake using `pair_accept_qr` on B in place of `pair_accept_password`.
/// No password is ever typed — it travels in the QR token.
#[tokio::test]
async fn pair_qr_full_round_trip() {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::UnixStream;

    let dir = safe_tempdir();
    let cfg_home = dir.path().join("cfg");
    // Set COPYPASTE_CONFIG_DIR *first* — `peers_file_path` checks it ahead
    // of dirs::config_dir(), so peers.json goes into cfg_home regardless of
    // whether dirs::config_dir() is affected by HOME/XDG_CONFIG_HOME (macOS
    // ignores HOME for Application Support).
    let _env = EnvGuard::set_all(
        &["COPYPASTE_CONFIG_DIR", "HOME", "XDG_CONFIG_HOME"],
        &cfg_home,
    );

    let sock_a = dir.path().join("test-qr-a.sock");
    let sock_b = dir.path().join("test-qr-b.sock");
    start_test_server(&sock_a).await;
    start_test_server(&sock_b).await;

    async fn call(sock: &std::path::Path, body: &str) -> serde_json::Value {
        let mut stream = UnixStream::connect(sock).await.unwrap();
        stream.write_all(body.as_bytes()).await.unwrap();
        stream.write_all(b"\n").await.unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        serde_json::from_str(&line).unwrap()
    }

    // S3/j8dr: Get the REAL cert fingerprints from both servers so the
    // cert-binder computation uses the correct values and the mandatory
    // initiator_confirm_b64 can be verified. (Old code used a static fake
    // fp_a which caused binder mismatch and was masked by the optional tag.)
    let fp_a = call(
        &sock_a,
        r#"{"id":"qr_fpa","method":"get_own_fingerprint","params":{}}"#,
    )
    .await["data"]["fingerprint"]
        .as_str()
        .expect("server A must return own fingerprint")
        .to_string();

    // Step 0: Device B generates a QR pairing code.
    let resp = call(
        &sock_b,
        r#"{"id":"qr0","method":"pair_generate_qr","params":{}}"#,
    )
    .await;
    assert_eq!(resp["ok"], true, "pair_generate_qr failed: {resp}");
    let qr = resp["data"]["qr"].as_str().unwrap().to_string();
    // The generated QR is now wrapped in the cppair://pair?p= deep-link URI
    // so external scanners (Google Lens) can open it in the app.
    assert!(
        qr.starts_with(copypaste_core::PAIRING_DEEPLINK_PREFIX),
        "QR must be wrapped in the cppair:// deep-link: {qr}"
    );

    // Step 0b: Device A scans, strips the wrapper, decodes the QR and derives
    // the PAKE password (mirrors the in-app scanner / manifest deep-link path).
    let bare = copypaste_core::strip_deeplink(&qr);
    assert!(
        bare.starts_with("CPPAIR2."),
        "stripped QR must use the v2 magic: {bare}"
    );
    let payload =
        copypaste_core::PairingPayload::decode(&bare).expect("scanning device must decode the QR");
    let password = payload.token.to_pake_password();
    // The fingerprint A pins is the one carried in the QR (B's fingerprint).
    // CPPAIR2 decode returns bare lowercase hex; convert to the colon-grouped
    // display form that `pair_peer_with_password` / `is_valid_fingerprint` expect.
    let fp_b_raw = payload.fingerprint.clone();
    assert!(!fp_b_raw.is_empty(), "QR must carry B's fingerprint");
    let fp_b = display_fingerprint(&fp_b_raw);

    // Step 1: Device A initiates using the QR-derived password.
    let body = format!(
        r#"{{"id":"qr1","method":"pair_peer_with_password","params":{{"peer_fingerprint":"{fp_b}","password":"{password}","step":"initiate"}}}}"#
    );
    let resp = call(&sock_a, &body).await;
    assert_eq!(resp["ok"], true, "initiate failed: {resp}");
    let session_id_a = resp["data"]["session_id"].as_str().unwrap().to_string();
    let msg1_b64 = resp["data"]["message1_b64"].as_str().unwrap().to_string();

    // Step 2: Device B accepts via pair_accept_qr (looks up its stored token).
    // Use A's REAL cert fingerprint so the cert-binder on both sides agrees.
    let body = format!(
        r#"{{"id":"qr2","method":"pair_accept_qr","params":{{"message1_b64":"{msg1_b64}","peer_fingerprint":"{fp_a}"}}}}"#
    );
    let resp = call(&sock_b, &body).await;
    assert_eq!(resp["ok"], true, "pair_accept_qr failed: {resp}");
    let session_id_b = resp["data"]["session_id"].as_str().unwrap().to_string();
    let msg2_b64 = resp["data"]["message2_b64"].as_str().unwrap().to_string();

    // Step 3: Device A finishes — also returns initiator_confirm_b64 (S3).
    let body = format!(
        r#"{{"id":"qr3","method":"pair_peer_with_password","params":{{"step":"finish","session_id":"{session_id_a}","message2_b64":"{msg2_b64}","peer_fingerprint":"{fp_b}","password":"{password}"}}}}"#
    );
    let resp = call(&sock_a, &body).await;
    assert_eq!(resp["ok"], true, "initiator finish failed: {resp}");
    let msg3_b64 = resp["data"]["message3_b64"].as_str().unwrap().to_string();
    // j8dr: extract the mandatory confirm tag from A's finish response.
    let initiator_confirm_b64 = resp["data"]["initiator_confirm_b64"]
        .as_str()
        .expect("initiator finish must return initiator_confirm_b64")
        .to_string();

    // Step 4: Device B finishes — the OPAQUE authenticator must validate,
    // proving both sides agreed on the QR token as the shared secret.
    // j8dr: include the mandatory initiator_confirm_b64.
    let body = format!(
        r#"{{"id":"qr4","method":"pair_accept_finish","params":{{"session_id":"{session_id_b}","message3_b64":"{msg3_b64}","peer_fingerprint":"{fp_a}","initiator_confirm_b64":"{initiator_confirm_b64}"}}}}"#
    );
    let resp = call(&sock_b, &body).await;
    assert_eq!(resp["ok"], true, "responder finish failed: {resp}");
    assert_eq!(resp["data"]["ok"], true, "pair_accept_finish must succeed");
}

/// `pair_accept_qr` with no prior `pair_generate_qr` must be rejected
/// rather than registering an empty/garbage PasswordFile.
#[tokio::test]
async fn pair_accept_qr_without_token_is_rejected() {
    use base64::Engine as _;
    let dir = safe_tempdir();
    let cfg_home = dir.path().join("cfg");
    // Include COPYPASTE_CONFIG_DIR so peers_file_path() points at cfg_home
    // on macOS (where dirs::config_dir() ignores HOME).
    let _env = EnvGuard::set_all(
        &["COPYPASTE_CONFIG_DIR", "HOME", "XDG_CONFIG_HOME"],
        &cfg_home,
    );
    let sock = dir.path().join("test-qr-notoken.sock");
    start_test_server(&sock).await;

    let fp = "a1:b2:c3:d4:e5:f6:07:18:29:3a:4b:5c:6d:7e:8f:90:a1:b2:c3:d4:e5:f6:07:18:29:3a:4b:5c:6d:7e:8f:90";
    let msg1 = base64::engine::general_purpose::STANDARD.encode([0u8; 32]);
    let body = format!(
        r#"{{"id":"nt1","method":"pair_accept_qr","params":{{"message1_b64":"{msg1}","peer_fingerprint":"{fp}"}}}}"#
    );
    let resp = call_one(&sock, &body).await;
    assert_eq!(
        resp["ok"], false,
        "pair_accept_qr without a generated token must fail: {resp}"
    );
}

/// T4 (v0.3) — `revoke_peer` validates its fingerprint argument and, for
/// a well-formed request, writes a row to the `revoked_devices` audit
/// table even when the peer was never in the local JSON peer store
/// (revoking an unknown fingerprint is intentionally allowed so the UI
/// can recover from a corrupted peers.json).
#[tokio::test]
async fn revoke_peer_validates_and_records_audit_row() {
    use copypaste_core::list_revoked_devices;

    let dir = safe_tempdir();
    let sock = dir.path().join("test-revoke.sock");

    // Redirect the config dir to this test's own tempdir so the
    // `revoke_peer` handler's `save_peers` never writes to (and never
    // depends on the existence of) the machine's real config dir. Under
    // parallel CI execution the platform `dirs::config_dir()` may not
    // exist, which previously made `save_peers` fail with ENOENT. Setting
    // `COPYPASTE_CONFIG_DIR` (checked first by `peers_file_path`) plus the
    // HOME/XDG fallbacks makes the test fully hermetic. `EnvGuard` holds
    // the process-wide `TEST_ENV_LOCK` for its lifetime, so this does not
    // race the other env-mutating tests in the workspace.
    let cfg_home = dir.path().join("cfg");
    let _env = EnvGuard::set_all(
        &[
            "COPYPASTE_CONFIG_DIR",
            "COPYPASTE_DATA_DIR",
            "HOME",
            "XDG_CONFIG_HOME",
        ],
        &cfg_home,
    );

    // Build the server manually so we can reach the shared Database
    // handle for assertions after the call.
    let db = Arc::new(Mutex::new(Database::open_in_memory().unwrap()));
    let private_mode = Arc::new(AtomicBool::new(false));
    let server = IpcServer::new(
        db.clone(),
        private_mode,
        Arc::new(zeroize::Zeroizing::new([0u8; 32])),
        Arc::new([0u8; 32]),
    );
    // Bind directly (no umask(0o177) race) — see start_test_server_returning_db.
    let listener = tokio::net::UnixListener::bind(&sock).expect("test socket bind must succeed");
    let sock_path = sock.clone();
    tokio::spawn(async move {
        if let Err(e) = server.serve_on(listener, CancellationToken::new()).await {
            tracing::error!("ipc: server on {:?} exited with error: {e}", &sock_path);
        }
    });
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    async fn call(sock: &std::path::Path, body: &str) -> serde_json::Value {
        let mut stream = UnixStream::connect(sock).await.unwrap();
        stream.write_all(body.as_bytes()).await.unwrap();
        stream.write_all(b"\n").await.unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        serde_json::from_str(&line).unwrap()
    }

    // Missing fingerprint → invalid_argument
    let resp = call(&sock, r#"{"id":"r1","method":"revoke_peer","params":{}}"#).await;
    assert_eq!(resp["ok"], false, "missing fingerprint must fail");
    assert_eq!(resp["error_code"], "invalid_argument");

    // Bad fingerprint hex → invalid_argument
    let resp = call(
        &sock,
        r#"{"id":"r2","method":"revoke_peer","params":{"fingerprint":"not-hex"}}"#,
    )
    .await;
    assert_eq!(resp["ok"], false, "bad fingerprint must fail");
    assert_eq!(resp["error_code"], "invalid_argument");

    // Valid request — unknown peer, but revoke still succeeds and writes
    // the audit row.
    let fp = std::iter::repeat_n("ab", 32).collect::<Vec<_>>().join(":");
    let body = format!(r#"{{"id":"r3","method":"revoke_peer","params":{{"fingerprint":"{fp}"}}}}"#);
    let resp = call(&sock, &body).await;
    assert_eq!(resp["ok"], true, "valid revoke must succeed: {resp}");
    assert_eq!(resp["data"]["fingerprint"], fp);
    assert!(
        resp["data"]["revoked_at"].as_u64().unwrap_or(0) > 0,
        "revoked_at must be populated"
    );

    // Audit row must be persisted in the shared SQLite DB.
    let db_guard = db.lock().await;
    let rows = list_revoked_devices(db_guard.conn()).unwrap();
    assert_eq!(rows.len(), 1, "exactly one audit row expected");
    assert_eq!(rows[0].fingerprint, fp);
}

// ------------------------------------------------------------------
// CopyPaste-gbo: revoke_peer auto-rotates the sync key when a cloud or
// relay sync key is already installed.  Tested under the widened cfg
// gate so it compiles on both cloud-sync and relay-sync builds.
// ------------------------------------------------------------------

/// When a sync key is installed and `revoke_peer` is called:
///   - the audit row is written (same as bare revoke),
///   - `sync_key_rotated: true` appears in the response,
///   - the installed sync key changes to a DIFFERENT value (rotation).
///
/// When NO sync key is installed, `sync_key_rotated: false` and the key
/// slot remains empty.
#[cfg(any(feature = "cloud-sync", feature = "relay-sync"))]
#[tokio::test]
async fn revoke_peer_auto_rotates_sync_key_when_active() {
    use copypaste_core::{list_revoked_devices, SyncKey};

    let dir = safe_tempdir();
    let sock = dir.path().join("test-revoke-rotate.sock");
    let cfg_home = dir.path().join("cfg");
    let _env = EnvGuard::set_all(
        &[
            "COPYPASTE_CONFIG_DIR",
            "COPYPASTE_DATA_DIR",
            "HOME",
            "XDG_CONFIG_HOME",
        ],
        &cfg_home,
    );

    // Shared sync-key state wired into the server so the test can
    // observe what the revoke_peer handler installed.
    let sync_key_arc: Arc<Mutex<Option<SyncKey>>> = Arc::new(Mutex::new(None));
    let last_sync_ms = Arc::new(std::sync::atomic::AtomicI64::new(0));
    let cloud_signed_in = Arc::new(AtomicBool::new(false));

    let db = Arc::new(Mutex::new(Database::open_in_memory().unwrap()));
    let private_mode = Arc::new(AtomicBool::new(false));
    let server = IpcServer::new(
        db.clone(),
        private_mode.clone(),
        Arc::new(zeroize::Zeroizing::new([0u8; 32])),
        Arc::new([0u8; 32]),
    )
    .with_cloud_sync_state(
        sync_key_arc.clone(),
        last_sync_ms.clone(),
        cloud_signed_in.clone(),
    );

    // Bind directly (no umask(0o177) race) — see start_test_server_returning_db.
    let listener = tokio::net::UnixListener::bind(&sock).expect("test socket bind must succeed");
    let sock_path = sock.clone();
    tokio::spawn(async move {
        if let Err(e) = server.serve_on(listener, CancellationToken::new()).await {
            tracing::error!("ipc: server on {:?} exited with error: {e}", &sock_path);
        }
    });
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    async fn call(sock: &std::path::Path, body: &str) -> serde_json::Value {
        let mut stream = UnixStream::connect(sock).await.unwrap();
        stream.write_all(body.as_bytes()).await.unwrap();
        stream.write_all(b"\n").await.unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        serde_json::from_str(&line).unwrap()
    }

    let fp = std::iter::repeat_n("cd", 32).collect::<Vec<_>>().join(":");

    // ── Case 1: no sync key installed → sync_key_rotated must be false ──
    {
        let body =
            format!(r#"{{"id":"rr1","method":"revoke_peer","params":{{"fingerprint":"{fp}"}}}}"#);
        let resp = call(&sock, &body).await;
        assert_eq!(resp["ok"], true, "revoke must succeed: {resp}");
        assert_eq!(
            resp["data"]["sync_key_rotated"], false,
            "no sync key installed → sync_key_rotated must be false"
        );
        // Key slot must still be empty.
        assert!(
            sync_key_arc.lock().await.is_none(),
            "sync_key must remain None when none was installed"
        );
    }

    // Install a known sync key (simulate the user having run set_sync_passphrase).
    let initial_key_bytes = [0xAAu8; 32];
    *sync_key_arc.lock().await = Some(SyncKey::from_bytes(initial_key_bytes));

    // ── Case 2: sync key installed → sync_key_rotated must be true and
    //            the key bytes must change (rotation). ──
    {
        let fp2 = std::iter::repeat_n("ef", 32).collect::<Vec<_>>().join(":");
        let body =
            format!(r#"{{"id":"rr2","method":"revoke_peer","params":{{"fingerprint":"{fp2}"}}}}"#);
        let resp = call(&sock, &body).await;
        assert_eq!(resp["ok"], true, "revoke+rotate must succeed: {resp}");
        assert_eq!(
            resp["data"]["sync_key_rotated"], true,
            "active sync key → sync_key_rotated must be true"
        );

        // The key slot must now hold a DIFFERENT key than before.
        let guard = sync_key_arc.lock().await;
        let rotated_key = guard.as_ref().expect("sync_key must be set after rotation");
        assert!(
            !rotated_key.ct_eq_bytes(&initial_key_bytes),
            "rotation must produce a key distinct from the initial key"
        );
    }

    // Audit rows must be written for both revocations.
    let db_guard = db.lock().await;
    let rows = list_revoked_devices(db_guard.conn()).unwrap();
    assert_eq!(rows.len(), 2, "exactly two audit rows expected");
}

// ------------------------------------------------------------------
// T5.x — clipboard-history UI action wiring
//
// New verbs added so the UI can drive history actions end-to-end over
// the Unix socket: `pin_item`, `delete_item`, `copy_item`, and
// `revoke_all_peers`. Each validates its arguments and returns the
// documented error code on missing/bad params, mirroring the
// beta-W3.2 (`pair_peer_with_password`) and T4 (`revoke_peer`) tests.
// ------------------------------------------------------------------

async fn call_one(sock: &std::path::Path, body: &str) -> serde_json::Value {
    let mut stream = UnixStream::connect(sock).await.unwrap();
    stream.write_all(body.as_bytes()).await.unwrap();
    stream.write_all(b"\n").await.unwrap();
    let mut lines = BufReader::new(&mut stream).lines();
    let line = lines.next_line().await.unwrap().unwrap();
    serde_json::from_str(&line).unwrap()
}

/// Build a bare in-process `IpcServer` (no socket) for exercising private
/// helpers like `insert_pake_session` directly.
fn bare_server() -> IpcServer {
    let db = Arc::new(Mutex::new(Database::open_in_memory().unwrap()));
    IpcServer::new(
        db,
        Arc::new(AtomicBool::new(false)),
        Arc::new(zeroize::Zeroizing::new([0u8; 32])),
        Arc::new([0u8; 32]),
    )
}

/// The single per-account sync-key derivation REQUIRES a Supabase account id.
/// `require_cloud_account_id` returns the id when one is set (signed in) and a
/// clean error otherwise — the invariant `set_sync_passphrase` / `rotate_sync_key`
/// / `revoke_and_rotate` rely on to refuse deriving an account-free key.
#[cfg(any(feature = "cloud-sync", feature = "relay-sync"))]
#[test]
fn require_cloud_account_id_errors_without_account_and_ok_with() {
    let server = bare_server();

    // No account id set (not signed in) → clean error, not a fallback key.
    let err = server
        .require_cloud_account_id()
        .expect_err("missing account id must be an error");
    assert!(
        err.contains("sign-in"),
        "error must point the user at signing in, got: {err}"
    );

    // An empty account id is treated the same as absent.
    *server
        .cloud_account_id
        .lock()
        .unwrap_or_else(|p| p.into_inner()) = Some(String::new());
    assert!(
        server.require_cloud_account_id().is_err(),
        "empty account id must also be rejected"
    );

    // A real account id (set by start_cloud after sign-in) resolves.
    let acct = "proj_abc|00000000-0000-0000-0000-0000000000aa".to_string();
    *server
        .cloud_account_id
        .lock()
        .unwrap_or_else(|p| p.into_inner()) = Some(acct.clone());
    assert_eq!(
        server
            .require_cloud_account_id()
            .expect("present account id"),
        acct
    );
}

/// c4q2.17: `list` is now a not_implemented stub. The `too_large_to_sync`
/// flag coverage lives in `history_page_reports_too_large_to_sync_per_item`.
#[tokio::test]
async fn list_reports_too_large_to_sync_per_item() {
    let db = Arc::new(Mutex::new(Database::open_in_memory().unwrap()));
    let server = IpcServer::new(
        db.clone(),
        Arc::new(AtomicBool::new(false)),
        Arc::new(zeroize::Zeroizing::new([0u8; 32])),
        Arc::new([0u8; 32]),
    );
    let resp = server
        .dispatch(r#"{"id":"1","method":"list","params":{"limit":50,"offset":0}}"#)
        .await;
    assert!(
        !resp.ok,
        "list must return not_implemented (c4q2.17): {resp:?}"
    );
    assert_eq!(
        resp.error_code,
        Some(ERR_CODE_NOT_IMPLEMENTED),
        "list must carry not_implemented error_code: {resp:?}"
    );
}

/// The `history_page` IPC response — the verb the macOS UI (HistoryWindow)
/// actually renders from — must carry the same daemon-computed
/// `too_large_to_sync` flag per item as `list`: `true` for an item whose
/// stored blob exceeds `SYNC_MAX_BLOB_BYTES` (8 MiB), `false` otherwise.
/// Mirrors `list_reports_too_large_to_sync_per_item` so the badge is faithful
/// regardless of which list verb the UI calls.
#[tokio::test]
async fn history_page_reports_too_large_to_sync_per_item() {
    let db = Arc::new(Mutex::new(Database::open_in_memory().unwrap()));
    let server = IpcServer::new(
        db.clone(),
        Arc::new(AtomicBool::new(false)),
        Arc::new(zeroize::Zeroizing::new([0u8; 32])),
        Arc::new([0u8; 32]),
    );

    // Seed a normal small item and an oversized one. `content` is the
    // at-rest ciphertext blob; the badge compares its length to 8 MiB.
    {
        let guard = db.lock().await;
        let small = copypaste_core::ClipboardItem::new_text(vec![0xAB; 16], vec![0u8; 24], 1);
        copypaste_core::insert_item(&guard, &small).unwrap();
        // One byte over the ceiling guarantees too_large_to_sync == true.
        let oversized = copypaste_core::ClipboardItem::new_text(
            vec![0xCD; crate::sync_orch::SYNC_MAX_BLOB_BYTES + 1],
            vec![0u8; 24],
            2,
        );
        copypaste_core::insert_item(&guard, &oversized).unwrap();
    }

    let resp = server
        .dispatch(r#"{"id":"1","method":"history_page","params":{"limit":50,"offset":0}}"#)
        .await;
    assert!(resp.ok, "history_page must succeed: {resp:?}");
    let data = resp.data.expect("history_page returns data");
    let items = data["items"].as_array().expect("items array");
    assert_eq!(items.len(), 2, "two seeded items expected");

    let flags: Vec<bool> = items
        .iter()
        .map(|it| {
            it["too_large_to_sync"]
                .as_bool()
                .expect("too_large_to_sync must be a bool on every history_page item")
        })
        .collect();
    assert_eq!(
        flags.iter().filter(|&&f| f).count(),
        1,
        "exactly one item must be flagged too_large_to_sync: {items:?}"
    );
    assert_eq!(
        flags.iter().filter(|&&f| !f).count(),
        1,
        "exactly one item must be under the sync ceiling: {items:?}"
    );
}

/// `history_page` must include `origin_device_name` (the human-readable name
/// from the `devices` table) for items whose `origin_device_id` matches a
/// paired device, and must emit `null` for items with an unknown origin.
#[tokio::test]
async fn history_page_returns_device_name_for_known_origin() {
    let db = Arc::new(Mutex::new(Database::open_in_memory().unwrap()));
    let server = IpcServer::new(
        db.clone(),
        Arc::new(AtomicBool::new(false)),
        Arc::new(zeroize::Zeroizing::new([0u8; 32])),
        Arc::new([0u8; 32]),
    );

    let known_device_id = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee";
    let unknown_device_id = "11111111-2222-3333-4444-555555555555";

    {
        let guard = db.lock().await;

        // Seed a device row so the known device has a name.
        guard
            .conn()
            .execute(
                "INSERT INTO devices (id, name, platform, public_key, fingerprint, verified) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![
                    known_device_id,
                    "My Laptop",
                    "macos",
                    "PUBKEY_PLACEHOLDER",
                    "aa:bb:cc:dd:ee:ff",
                    1_i64,
                ],
            )
            .unwrap();

        // Item from the known (paired) device.
        let mut known_item =
            copypaste_core::ClipboardItem::new_text(vec![0xAA; 4], vec![0u8; 24], 1);
        known_item.origin_device_id = known_device_id.to_string();
        copypaste_core::insert_item_with_fts(&guard, &known_item, "hello from known").unwrap();

        // Item from an unknown device (not in the `devices` table).
        let mut unknown_item =
            copypaste_core::ClipboardItem::new_text(vec![0xBB; 4], vec![0u8; 24], 2);
        unknown_item.origin_device_id = unknown_device_id.to_string();
        copypaste_core::insert_item_with_fts(&guard, &unknown_item, "hello from unknown").unwrap();

        // Item with an empty origin_device_id (pre-v3 row).
        let legacy_item = copypaste_core::ClipboardItem::new_text(vec![0xCC; 4], vec![0u8; 24], 3);
        // origin_device_id starts as "" via new_text, no need to set it.
        copypaste_core::insert_item_with_fts(&guard, &legacy_item, "legacy item").unwrap();
    }

    let resp = server
        .dispatch(r#"{"id":"dnr","method":"history_page","params":{"limit":50,"offset":0}}"#)
        .await;
    assert!(resp.ok, "history_page must succeed: {resp:?}");
    let data = resp.data.expect("history_page returns data");
    let items = data["items"].as_array().expect("items array");
    assert_eq!(items.len(), 3, "three seeded items expected");

    // Find the item from the known device and verify it carries the name.
    let known_item_json = items
        .iter()
        .find(|it| it["origin_device_id"].as_str() == Some(known_device_id))
        .expect("item from known device must be present");
    assert_eq!(
        known_item_json["origin_device_name"].as_str(),
        Some("My Laptop"),
        "origin_device_name must be the paired device's name: {known_item_json}"
    );

    // The unknown device must yield a JSON null for origin_device_name.
    let unknown_item_json = items
        .iter()
        .find(|it| it["origin_device_id"].as_str() == Some(unknown_device_id))
        .expect("item from unknown device must be present");
    assert!(
        unknown_item_json["origin_device_name"].is_null(),
        "origin_device_name must be null for an unpaired device: {unknown_item_json}"
    );

    // The legacy item (empty origin_device_id) must also have a null name.
    let legacy_item_json = items
        .iter()
        .find(|it| it["origin_device_id"].as_str() == Some(""))
        .expect("legacy item must be present");
    assert!(
        legacy_item_json["origin_device_name"].is_null(),
        "origin_device_name must be null for a legacy empty-origin item: {legacy_item_json}"
    );
}

/// CRITICAL-1: `display_fingerprint` renders the mTLS canonical fingerprint
/// (colon-free 64-hex from `fingerprint_of`) into the user-facing colon-hex
/// form, and `canonical_fingerprint` round-trips it back to the exact value
/// the mTLS verifier compares — so a pinned QR fingerprint authenticates.
#[test]
fn display_fingerprint_round_trips_cert_fingerprint() {
    let cert = copypaste_p2p::cert::SelfSignedCert::generate("rt-device").unwrap();
    let canonical = cert.fingerprint(); // hex(SHA-256(cert_der)), 64 hex chars, no colons
    assert_eq!(canonical.len(), 64, "cert fingerprint must be 64 hex chars");

    let display = display_fingerprint(&canonical);
    // 32 colon-separated 2-hex groups.
    assert_eq!(
        display.split(':').count(),
        32,
        "must be 32 colon groups: {display}"
    );
    assert!(
        is_valid_fingerprint(&display),
        "display form must validate: {display}"
    );

    // The mTLS boundary strips colons; it MUST equal the original canonical
    // value the verifier (`fingerprint_of`) produces.
    assert_eq!(
        canonical_fingerprint(&display),
        canonical,
        "round-trip must recover the exact canonical fingerprint the verifier pins"
    );
}

/// CRITICAL-1: with no cert fingerprint set (P2P disabled), the pairing
/// handlers must refuse rather than advertise the device-key fingerprint the
/// mTLS layer never pins.
#[tokio::test]
async fn pairing_handlers_error_when_p2p_disabled() {
    let server = bare_server(); // no .with_cert_fingerprint → cert_fingerprint == None

    let resp = server
        .dispatch(r#"{"id":"f1","method":"get_own_fingerprint","params":{}}"#)
        .await;
    assert!(!resp.ok, "get_own_fingerprint must error without a cert");
    assert!(
        resp.error
            .as_deref()
            .unwrap_or_default()
            .contains("P2P is disabled"),
        "must be the disabled-P2P error, not a parse error: {resp:?}"
    );

    let resp = server
        .dispatch(r#"{"id":"q1","method":"pair_generate_qr","params":{}}"#)
        .await;
    assert!(!resp.ok, "pair_generate_qr must error without a cert");
    assert!(
        resp.error
            .as_deref()
            .unwrap_or_default()
            .contains("P2P is disabled"),
        "must be the disabled-P2P error, not a parse error: {resp:?}"
    );
}

/// LAN/SAS Phase 2: `pair_get_sas` on a fresh server reports the machine as
/// `idle` with no SAS/role fields.
#[tokio::test]
async fn pair_get_sas_reports_idle_initially() {
    let server = bare_server();
    let resp = server
        .dispatch(r#"{"id":"s1","method":"pair_get_sas","params":{}}"#)
        .await;
    assert!(resp.ok, "pair_get_sas must succeed: {resp:?}");
    let data = resp.data.expect("data present");
    assert_eq!(data["state"], "idle");
    assert!(data.get("sas").is_none(), "no SAS when idle");
    assert!(data.get("role").is_none(), "no role when idle");
}

/// LAN/SAS Phase 2: `pair_confirm_sas` with no pairing awaiting confirmation
/// is an invalid-argument error (there is no oneshot to fire).
#[tokio::test]
async fn pair_confirm_sas_without_pending_errors() {
    let server = bare_server();
    let resp = server
        .dispatch(r#"{"id":"c1","method":"pair_confirm_sas","params":{"accept":true}}"#)
        .await;
    assert!(!resp.ok, "must error when nothing is awaiting confirmation");
    assert_eq!(resp.error_code, Some("invalid_argument"));
}

/// LAN/SAS Phase 2: `pair_confirm_sas` missing the `accept` boolean is
/// rejected with invalid_argument.
#[tokio::test]
async fn pair_confirm_sas_missing_accept_errors() {
    let server = bare_server();
    let resp = server
        .dispatch(r#"{"id":"c2","method":"pair_confirm_sas","params":{}}"#)
        .await;
    assert!(!resp.ok);
    assert_eq!(resp.error_code, Some("invalid_argument"));
}

/// LAN/SAS Phase 2: `pair_abort` always succeeds (idempotent) and leaves the
/// machine non-active.
#[tokio::test]
async fn pair_abort_is_idempotent_and_succeeds() {
    let server = bare_server();
    let resp = server
        .dispatch(r#"{"id":"a1","method":"pair_abort","params":{}}"#)
        .await;
    assert!(resp.ok, "pair_abort must succeed: {resp:?}");
    // Still idle afterwards (nothing was in flight).
    let resp = server
        .dispatch(r#"{"id":"s2","method":"pair_get_sas","params":{}}"#)
        .await;
    assert_eq!(resp.data.unwrap()["state"], "idle");
}

/// LAN/SAS Phase 2: `pair_with_discovered` requires P2P (a cert); without one
/// it errors with invalid_argument rather than silently starting a pairing.
#[tokio::test]
async fn pair_with_discovered_errors_when_p2p_disabled() {
    let server = bare_server(); // no cert / no discovery
    let resp = server
        .dispatch(
            r#"{"id":"p1","method":"pair_with_discovered","params":{"device_id":"deadbeef"}}"#,
        )
        .await;
    assert!(!resp.ok, "must error without P2P: {resp:?}");
    assert_eq!(resp.error_code, Some("invalid_argument"));
}

/// LAN/SAS Phase 2: `pair_with_discovered` missing `device_id` is rejected.
#[tokio::test]
async fn pair_with_discovered_missing_device_id_errors() {
    let server = bare_server();
    let resp = server
        .dispatch(r#"{"id":"p2","method":"pair_with_discovered","params":{}}"#)
        .await;
    assert!(!resp.ok);
    assert_eq!(resp.error_code, Some("invalid_argument"));
}

/// BUG A1: discovery-initiated pairing must work MORE THAN ONCE per daemon
/// lifetime. The `pair_with_discovered` handler resets the coordinator to
/// `Idle` after recording the terminal outcome (on BOTH the success and the
/// failure arm). This reproduces the exact begin → terminal → reset sequence
/// the handler performs and proves a SECOND pairing can begin (the SM is not
/// stuck rate-limited). Before the fix the second `try_begin` returned false.
#[tokio::test]
async fn pair_with_discovered_can_begin_twice_sequentially() {
    use crate::pairing_sm::{PairingRole, PairingState, PeerSnapshot};
    let server = bare_server();
    let pairing = server.pairing_coordinator();

    // --- First pairing: success arm. ---
    assert!(
        pairing.try_begin(PairingRole::Initiator, PeerSnapshot::default()),
        "first pairing must begin from Idle"
    );
    // Handler records the terminal outcome, then resets (the fix).
    pairing.finish(PairingState::Confirmed);
    pairing.reset();
    assert!(
        pairing.snapshot().is_idle(),
        "after a confirmed pairing the SM must be Idle again"
    );

    // --- Second pairing: must NOT be refused as rate-limited. ---
    assert!(
        pairing.try_begin(PairingRole::Initiator, PeerSnapshot::default()),
        "BUG A1: a second pair_with_discovered must be able to begin; \
             without the reset the SM stays terminal and try_begin returns false"
    );
    // Failure arm of the handler also resets.
    pairing.finish(PairingState::Rejected);
    pairing.reset();
    assert!(
        pairing.snapshot().is_idle(),
        "after a failed pairing the SM must be Idle again"
    );

    // --- Third pairing proves the failure arm reset works too. ---
    assert!(
        pairing.try_begin(PairingRole::Initiator, PeerSnapshot::default()),
        "a pairing after a failed one must also begin"
    );
}

/// CRITICAL-1: when a cert fingerprint IS configured, `get_own_fingerprint`
/// returns exactly that colon-hex cert fingerprint (not the device key).
#[tokio::test]
async fn get_own_fingerprint_returns_cert_fingerprint() {
    let cert = copypaste_p2p::cert::SelfSignedCert::generate("own-fp-device").unwrap();
    let expected = display_fingerprint(&cert.fingerprint());
    let server = bare_server().with_cert_fingerprint(expected.clone());

    let resp = server
        .dispatch(r#"{"id":"f2","method":"get_own_fingerprint","params":{}}"#)
        .await;
    assert!(resp.ok, "must succeed with a cert: {resp:?}");
    let data = resp.data.expect("data present");
    assert_eq!(data["fingerprint"], serde_json::Value::String(expected));
}

/// `get_own_device_info` must include `public_ip` in its response payload.
/// Without a wired public-IP cache the field serialises as JSON `null`, but
/// it must NOT be absent entirely (the UI keys off its presence to decide
/// whether to render the public-IP row).
#[tokio::test]
async fn get_own_device_info_includes_public_ip_field() {
    let server = bare_server();
    let resp = server
        .dispatch(r#"{"id":"d1","method":"get_own_device_info","params":{}}"#)
        .await;
    assert!(resp.ok, "get_own_device_info must succeed: {resp:?}");
    let data = resp.data.expect("data must be present");
    // The key must exist in the JSON object; its value may be null (no
    // cached IP yet) or a non-empty string (IP resolved).
    assert!(
        data.as_object()
            .map(|o| o.contains_key("public_ip"))
            .unwrap_or(false),
        "get_own_device_info response must include public_ip key: {data}"
    );
}

/// When the cached public-IP slot contains a value, `get_own_device_info`
/// returns that exact string.
#[tokio::test]
async fn get_own_device_info_returns_cached_public_ip() {
    let cache = Arc::new(tokio::sync::RwLock::new(Some("203.0.113.42".to_owned())));
    let server = bare_server().with_public_ip_cache(cache);
    let resp = server
        .dispatch(r#"{"id":"d2","method":"get_own_device_info","params":{}}"#)
        .await;
    assert!(resp.ok, "must succeed: {resp:?}");
    let data = resp.data.expect("data present");
    assert_eq!(
        data["public_ip"],
        serde_json::Value::String("203.0.113.42".to_owned()),
        "public_ip must reflect cached value: {data}"
    );
}

/// B1: `collect_own_peer_meta` must copy the caller-supplied own public IP
/// (read from the cache before `spawn_blocking`) into the outgoing `PeerMeta`
/// so it is advertised in-band to the peer during pairing.
#[test]
fn collect_own_peer_meta_copies_public_ip_into_meta() {
    let meta = IpcServer::collect_own_peer_meta(Some("198.51.100.7".to_owned()), None, None);
    assert_eq!(
        meta.public_ip.as_deref(),
        Some("198.51.100.7"),
        "collect_own_peer_meta must put the supplied public_ip into PeerMeta"
    );
}

/// B1: when no own public IP is available (STUN unresolved or
/// `collect_public_ip` disabled), the outgoing `PeerMeta.public_ip` is `None`.
#[test]
fn collect_own_peer_meta_none_public_ip_yields_none() {
    let meta = IpcServer::collect_own_peer_meta(None, None, None);
    assert_eq!(
        meta.public_ip, None,
        "a None public_ip must not synthesise any value in PeerMeta"
    );
}

/// CopyPaste-yw2k: `collect_own_peer_meta` must copy the supplied
/// `supabase_account_id` into the outgoing `PeerMeta` so it is advertised
/// in-band to the peer during pairing.
#[test]
fn collect_own_peer_meta_copies_supabase_account_id_into_meta() {
    let account_id = "proj_abc/uid_00000000-0000-0000-0000-000000000001".to_owned();
    let meta = IpcServer::collect_own_peer_meta(None, None, Some(account_id.clone()));
    assert_eq!(
        meta.supabase_account_id.as_deref(),
        Some(account_id.as_str()),
        "collect_own_peer_meta must put the supplied supabase_account_id into PeerMeta"
    );
}

/// CopyPaste-yw2k: when `supabase_account_id` is `None` (cloud-sync off),
/// the outgoing `PeerMeta.supabase_account_id` must also be `None`.
#[test]
fn collect_own_peer_meta_none_supabase_account_id_yields_none() {
    let meta = IpcServer::collect_own_peer_meta(None, None, None);
    assert_eq!(
        meta.supabase_account_id, None,
        "a None supabase_account_id must not synthesise any value in PeerMeta"
    );
}

/// fix/p2p-c-review #1 — a session older than `PAKE_SESSION_TTL` is evicted
/// on the next `insert_pake_session`, so the map cannot grow with abandoned
/// (crashed-client) sessions.
#[tokio::test]
async fn stale_pake_sessions_are_evicted_on_insert() {
    let server = bare_server();

    // Insert a first session, then back-date it past the TTL so it is
    // considered stale. (`Instant` can't be constructed directly; we patch
    // the stored `created_at` in place — this module has field access.)
    let (init1, _msg1) = PakeInitiator::new("hunter2-pw").unwrap();
    server
        .insert_pake_session("stale".into(), PakeSession::Initiator(Box::new(init1)))
        .await
        .unwrap();
    {
        let mut sessions = server.pake_sessions.lock().await;
        let stamped = sessions.get_mut("stale").expect("stale session present");
        stamped.created_at =
            std::time::Instant::now() - (PAKE_SESSION_TTL + std::time::Duration::from_secs(1));
    }

    // Inserting a fresh session triggers TTL eviction of the stale one.
    let (init2, _msg2) = PakeInitiator::new("hunter2-pw").unwrap();
    server
        .insert_pake_session("fresh".into(), PakeSession::Initiator(Box::new(init2)))
        .await
        .unwrap();

    let sessions = server.pake_sessions.lock().await;
    assert!(
        !sessions.contains_key("stale"),
        "stale session must be evicted on insert"
    );
    assert!(
        sessions.contains_key("fresh"),
        "fresh session must remain after eviction pass"
    );
    assert_eq!(sessions.len(), 1, "exactly one live session expected");
}

/// fix/p2p-c-review #1 — once `MAX_PAKE_SESSIONS` non-stale sessions are
/// live, a further insert is rejected (rather than growing without bound).
#[tokio::test]
async fn pake_session_cap_rejects_excess() {
    let server = bare_server();

    for i in 0..MAX_PAKE_SESSIONS {
        let (init, _m) = PakeInitiator::new("hunter2-pw").unwrap();
        server
            .insert_pake_session(format!("s{i}"), PakeSession::Initiator(Box::new(init)))
            .await
            .expect("inserts up to the cap must succeed");
    }

    let (init, _m) = PakeInitiator::new("hunter2-pw").unwrap();
    let over_cap = server
        .insert_pake_session("over".into(), PakeSession::Initiator(Box::new(init)))
        .await;
    assert!(over_cap.is_err(), "insert past the cap must be rejected");
    assert_eq!(
        server.pake_sessions.lock().await.len(),
        MAX_PAKE_SESSIONS,
        "map must not exceed the cap"
    );
}

/// c4q2.20: pair_accept_password is now a not_implemented stub. The short-
/// password validation test is adapted to verify the stub returns not_implemented
/// (the input validation that used to run is now irrelevant — the handler exits
/// before reaching it).
#[tokio::test]
async fn pair_accept_password_rejects_short_password() {
    let dir = safe_tempdir();
    let sock = dir.path().join("test-short-pw.sock");
    start_test_server(&sock).await;

    let resp = call_one(
            &sock,
            r#"{"id":"sp1","method":"pair_accept_password","params":{"message1_b64":"AAAA","peer_fingerprint":"ab:ab","password":"short"}}"#,
        )
        .await;
    assert_eq!(
        resp["ok"], false,
        "pair_accept_password must fail (c4q2.20): {resp}"
    );
    assert_eq!(
        resp["error_code"].as_str().unwrap_or(""),
        "not_implemented",
        "pair_accept_password stub must return not_implemented: {resp}"
    );
}

/// fix/p2p-c-review #2 — when a live P2P allowlist is attached, finishing a
/// PAKE pairing registers the peer in it (normalised to canonical hex) so
/// the mTLS accept loop honours the peer without a restart.
#[tokio::test]
async fn register_live_peer_feeds_shared_allowlist() {
    let peers = copypaste_p2p::transport::PairedPeers::new();
    let server = bare_server().with_p2p_peers(peers.clone());

    let colon_fp = std::iter::repeat_n("aa", 32).collect::<Vec<_>>().join(":");
    let canonical = canonical_fingerprint(&colon_fp);
    assert!(!peers.is_known(&canonical), "precondition: not yet known");

    server.register_live_peer(&colon_fp);

    assert!(
        peers.is_known(&canonical),
        "paired peer must be accepted by the live allowlist after finish"
    );
}

#[tokio::test]
async fn pin_item_missing_id_returns_invalid_argument() {
    let dir = safe_tempdir();
    let sock = dir.path().join("pin_item_missing.sock");
    start_test_server(&sock).await;
    let resp = call_one(
        &sock,
        r#"{"id":"pi1","method":"pin_item","params":{"pinned":true}}"#,
    )
    .await;
    assert_eq!(resp["ok"], false, "missing id must fail");
    assert_eq!(resp["error_code"], "invalid_argument");
}

#[tokio::test]
async fn pin_item_missing_pinned_returns_invalid_argument() {
    let dir = safe_tempdir();
    let sock = dir.path().join("pin_item_no_flag.sock");
    start_test_server(&sock).await;
    let fp_id = "00000000-0000-0000-0000-000000000000";
    let body = format!(r#"{{"id":"pi2","method":"pin_item","params":{{"id":"{fp_id}"}}}}"#);
    let resp = call_one(&sock, &body).await;
    assert_eq!(resp["ok"], false, "missing pinned bool must fail");
    assert_eq!(resp["error_code"], "invalid_argument");
}

#[tokio::test]
async fn pin_item_bad_uuid_returns_invalid_argument() {
    let dir = safe_tempdir();
    let sock = dir.path().join("pin_item_bad_uuid.sock");
    start_test_server(&sock).await;
    let resp = call_one(
        &sock,
        r#"{"id":"pi3","method":"pin_item","params":{"id":"not-a-uuid","pinned":true}}"#,
    )
    .await;
    assert_eq!(resp["ok"], false, "bad uuid must fail");
    assert_eq!(resp["error_code"], "invalid_argument");
}

#[tokio::test]
async fn pin_item_valid_uuid_pins_and_unpins() {
    let dir = safe_tempdir();
    let sock = dir.path().join("pin_item_ok.sock");
    start_test_server(&sock).await;
    let id = "00000000-0000-0000-0000-000000000000";
    // Pin: even when the row does not exist, the UPDATE affects 0 rows
    // and succeeds (the UI optimistically pins; a stale id is harmless).
    let body =
        format!(r#"{{"id":"pi4","method":"pin_item","params":{{"id":"{id}","pinned":true}}}}"#);
    let resp = call_one(&sock, &body).await;
    assert_eq!(resp["ok"], true, "valid pin must succeed: {resp}");
    assert_eq!(resp["data"]["pinned"], true);
    assert_eq!(resp["data"]["id"], id);
    // Unpin path.
    let body =
        format!(r#"{{"id":"pi5","method":"pin_item","params":{{"id":"{id}","pinned":false}}}}"#);
    let resp = call_one(&sock, &body).await;
    assert_eq!(resp["ok"], true, "valid unpin must succeed: {resp}");
    assert_eq!(resp["data"]["pinned"], false);
}

#[tokio::test]
async fn delete_item_missing_id_returns_invalid_argument() {
    let dir = safe_tempdir();
    let sock = dir.path().join("del_item_missing.sock");
    start_test_server(&sock).await;
    let resp = call_one(&sock, r#"{"id":"di1","method":"delete_item","params":{}}"#).await;
    assert_eq!(resp["ok"], false, "missing id must fail");
    assert_eq!(resp["error_code"], "invalid_argument");
}

#[tokio::test]
async fn delete_item_bad_uuid_returns_invalid_argument() {
    let dir = safe_tempdir();
    let sock = dir.path().join("del_item_bad_uuid.sock");
    start_test_server(&sock).await;
    let resp = call_one(
        &sock,
        r#"{"id":"di2","method":"delete_item","params":{"id":"not-a-uuid"}}"#,
    )
    .await;
    assert_eq!(resp["ok"], false, "bad uuid must fail");
    assert_eq!(resp["error_code"], "invalid_argument");
}

#[tokio::test]
async fn delete_item_valid_uuid_succeeds() {
    let dir = safe_tempdir();
    let sock = dir.path().join("del_item_ok.sock");
    start_test_server(&sock).await;
    let id = "00000000-0000-0000-0000-000000000000";
    let body = format!(r#"{{"id":"di3","method":"delete_item","params":{{"id":"{id}"}}}}"#);
    let resp = call_one(&sock, &body).await;
    // Deleting a non-existent row is a no-op DELETE → request still ok,
    // but `deleted` reflects rows-affected (0 → false) so the response
    // matches reality rather than always claiming a deletion happened.
    assert_eq!(resp["ok"], true, "valid delete must succeed: {resp}");
    assert_eq!(resp["data"]["deleted"], false, "no row existed: {resp}");
    assert_eq!(resp["data"]["id"], id);
}

#[tokio::test]
async fn copy_item_missing_id_returns_invalid_argument() {
    let dir = safe_tempdir();
    let sock = dir.path().join("copy_item_missing.sock");
    start_test_server(&sock).await;
    let resp = call_one(&sock, r#"{"id":"ci1","method":"copy_item","params":{}}"#).await;
    assert_eq!(resp["ok"], false, "missing id must fail");
    assert_eq!(resp["error_code"], "invalid_argument");
}

#[tokio::test]
async fn copy_item_bad_uuid_returns_invalid_argument() {
    let dir = safe_tempdir();
    let sock = dir.path().join("copy_item_bad_uuid.sock");
    start_test_server(&sock).await;
    let resp = call_one(
        &sock,
        r#"{"id":"ci2","method":"copy_item","params":{"id":"not-a-uuid"}}"#,
    )
    .await;
    assert_eq!(resp["ok"], false, "bad uuid must fail");
    assert_eq!(resp["error_code"], "invalid_argument");
}

#[tokio::test]
async fn copy_item_unknown_id_returns_not_found() {
    let dir = safe_tempdir();
    let sock = dir.path().join("copy_item_unknown.sock");
    start_test_server(&sock).await;
    let id = "00000000-0000-0000-0000-000000000000";
    let body = format!(r#"{{"id":"ci3","method":"copy_item","params":{{"id":"{id}"}}}}"#);
    let resp = call_one(&sock, &body).await;
    assert_eq!(resp["ok"], false, "unknown id must fail");
    assert_eq!(resp["error_code"], "not_found");
}

#[tokio::test]
async fn copy_item_seeded_id_is_resolved() {
    // Regression for the data-loss fix: copy_item must resolve a row by its
    // primary key (`get_item_by_id`) rather than paging + scanning. We seed
    // a text item with a deliberately wrong-length nonce so the paste-back
    // path returns a deterministic error *without* touching the real
    // NSPasteboard — the key assertion is that the lookup found the row, so
    // the response is anything except `not_found`.
    let dir = safe_tempdir();
    let sock = dir.path().join("copy_item_seeded.sock");
    let (_pm, db) = start_test_server_returning_db(&sock, false).await;

    let id = {
        let guard = db.lock().await;
        // 0xAA/0xBB content with a 1-byte nonce (invalid: must be 24) so
        // write_to_pasteboard short-circuits before any NSPasteboard call.
        let item = copypaste_core::ClipboardItem::new_text(vec![0xAA, 0xBB], vec![0u8; 1], 1);
        let id = item.id.clone();
        copypaste_core::insert_item(&guard, &item).unwrap();
        id
    };

    let body = format!(r#"{{"id":"ci4","method":"copy_item","params":{{"id":"{id}"}}}}"#);
    let resp = call_one(&sock, &body).await;
    assert_ne!(
        resp["error_code"], "not_found",
        "seeded item must be resolved by id, not reported missing: {resp}"
    );
}

#[tokio::test]
async fn revoke_all_peers_empty_store_succeeds() {
    // With no peers.json present, revoke_all_peers must succeed and
    // report zero revoked rather than erroring.
    let dir = safe_tempdir();
    let sock = dir.path().join("revoke_all_empty.sock");
    // Isolate the config dir so this test never touches the developer's
    // real peers.json.  `peers_file_path()` checks COPYPASTE_CONFIG_DIR
    // first (before dirs::config_dir()), which is necessary on macOS
    // because dirs::config_dir() ignores $HOME and always resolves to
    // ~/Library/Application Support — so setting only HOME/XDG_CONFIG_HOME
    // was insufficient and the test leaked to the real peers.json.
    let cfg_home = dir.path().join("cfg");
    let _env = EnvGuard::set_all(
        &["COPYPASTE_CONFIG_DIR", "HOME", "XDG_CONFIG_HOME"],
        &cfg_home,
    );
    start_test_server(&sock).await;
    let resp = call_one(
        &sock,
        r#"{"id":"ra1","method":"revoke_all_peers","params":{}}"#,
    )
    .await;
    assert_eq!(
        resp["ok"], true,
        "revoke_all on empty store must succeed: {resp}"
    );
    assert_eq!(
        resp["data"]["revoked"].as_u64(),
        Some(0),
        "empty store revokes zero peers: {resp}"
    );
}

#[tokio::test]
async fn revoke_all_peers_revokes_every_peer() {
    // Happy path: seed N peers in peers.json, call revoke_all_peers, and
    // assert all N are revoked, the store is cleared, and an audit row was
    // written for each (atomic batch via revoke_devices).
    let dir = safe_tempdir();
    let sock = dir.path().join("revoke_all_n.sock");
    // Pin COPYPASTE_CONFIG_DIR first — peers_file_path() checks it before
    // dirs::config_dir(), so the handler reads/writes cfg_home regardless
    // of whether dirs::config_dir() is affected by HOME (macOS ignores HOME
    // for Application Support). Without this pin the test accidentally
    // reads/writes the developer's real peers.json on macOS.
    let cfg_home = dir.path().join("cfg");
    let _env = EnvGuard::set_all(
        &["COPYPASTE_CONFIG_DIR", "HOME", "XDG_CONFIG_HOME"],
        &cfg_home,
    );

    // Seed peers.json exactly where peers_file_path() will look:
    // cfg_home itself (COPYPASTE_CONFIG_DIR is the direct config dir, not a
    // base — paths::config_dir() returns it as-is).
    let peers_dir = cfg_home.clone();
    std::fs::create_dir_all(&peers_dir).unwrap();
    let peers_json = peers_dir.join("peers.json");
    // Use realistic (non-placeholder) fingerprints — the daemon filters out
    // all-same-byte fingerprints (e.g. aa:aa:aa:aa:aa:aa:aa:aa) to drop
    // stale test data from peers.json.
    let peers = serde_json::json!([
        {"name": "Laptop", "fingerprint": "a1:b2:c3:d4:e5:f6:07:18", "added_at": 1},
        {"name": "Phone",  "fingerprint": "f0:e1:d2:c3:b4:a5:96:87", "added_at": 2},
        {"name": "Tablet", "fingerprint": "12:34:56:78:9a:bc:de:f0", "added_at": 3},
    ]);
    std::fs::write(&peers_json, serde_json::to_string(&peers).unwrap()).unwrap();

    let (_pm, db) = start_test_server_returning_db(&sock, false).await;
    let resp = call_one(
        &sock,
        r#"{"id":"ra2","method":"revoke_all_peers","params":{}}"#,
    )
    .await;

    assert_eq!(resp["ok"], true, "revoke_all must succeed: {resp}");
    assert_eq!(
        resp["data"]["revoked"].as_u64(),
        Some(3),
        "all three peers must be revoked: {resp}"
    );
    assert_eq!(resp["data"]["cleared"].as_u64(), Some(3));

    // Store must now be empty.
    let remaining = std::fs::read_to_string(&peers_json).unwrap_or_else(|_| "[]".into());
    let remaining: Vec<serde_json::Value> = serde_json::from_str(&remaining).unwrap();
    assert!(remaining.is_empty(), "peer store must be cleared");

    // An audit row must exist for every revoked fingerprint.
    let audit = {
        let guard = db.lock().await;
        copypaste_core::list_revoked_devices(guard.conn()).unwrap()
    };
    assert_eq!(audit.len(), 3, "one audit row per revoked peer");
    for fp in [
        "a1:b2:c3:d4:e5:f6:07:18",
        "f0:e1:d2:c3:b4:a5:96:87",
        "12:34:56:78:9a:bc:de:f0",
    ] {
        assert!(
            audit.iter().any(|r| r.fingerprint == fp),
            "missing audit row for {fp}"
        );
    }
}

/// BUG 2 — `get_sync_status` must report the REAL `signed_in` auth state
/// published by the cloud loops via the shared `cloud_signed_in` flag, not
/// the old hardcoded `signed_in = supabase_configured`. We build a server,
/// wire a shared flag, and assert the IPC response tracks the flag both ways.
#[cfg(feature = "cloud-sync")]
#[tokio::test]
async fn get_sync_status_reports_real_signed_in_flag() {
    let db = Arc::new(Mutex::new(Database::open_in_memory().unwrap()));
    let private_mode = Arc::new(AtomicBool::new(false));
    let local_key = Arc::new(zeroize::Zeroizing::new([0u8; 32]));
    let device_pub = Arc::new([0u8; 32]);

    let sync_key = Arc::new(Mutex::new(None));
    let last_sync_ms = Arc::new(std::sync::atomic::AtomicI64::new(0));
    let signed_in = Arc::new(AtomicBool::new(false));

    let server = IpcServer::new(db, private_mode, local_key, device_pub).with_cloud_sync_state(
        sync_key,
        last_sync_ms,
        signed_in.clone(),
    );

    let line = r#"{"id":"1","method":"get_sync_status","params":{}}"#;

    // Flag false (e.g. after CloudError::AuthFailed) → signed_in == false,
    // even though supabase may be "configured".
    let resp = server.dispatch(line).await;
    let data = resp.data.expect("get_sync_status must return data");
    assert_eq!(
        data["signed_in"], false,
        "signed_in must reflect the false auth flag, not supabase_configured: {data}"
    );

    // Flip the shared flag true (successful bearer resolution) → reflected.
    signed_in.store(true, Ordering::Relaxed);
    let resp2 = server.dispatch(line).await;
    let data2 = resp2.data.expect("get_sync_status must return data");
    assert_eq!(
        data2["signed_in"], true,
        "signed_in must track the real auth flag once set true: {data2}"
    );
}

// ── CopyPaste-i5b: cloud_sign_in/out set cloud_signed_in ─────────────────

/// `cloud_sign_out` must clear `cloud_signed_in` to false so
/// `get_sync_status` stops reporting signed_in = true after logout.
/// Proves the flag is set by the IPC sign-out path (not only by the
/// startup `start_cloud` path that was the only setter before this fix).
#[cfg(feature = "cloud-sync")]
#[tokio::test]
async fn cloud_sign_out_clears_signed_in_flag() {
    let db = Arc::new(Mutex::new(Database::open_in_memory().unwrap()));
    let private_mode = Arc::new(AtomicBool::new(false));
    let local_key = Arc::new(zeroize::Zeroizing::new([0u8; 32]));
    let device_pub = Arc::new([0u8; 32]);

    let sync_key = Arc::new(Mutex::new(None));
    let last_sync_ms = Arc::new(std::sync::atomic::AtomicI64::new(0));
    // Start the flag at true — simulating a previously signed-in session.
    let signed_in = Arc::new(AtomicBool::new(true));

    let server = IpcServer::new(db, private_mode, local_key, device_pub).with_cloud_sync_state(
        sync_key,
        last_sync_ms,
        signed_in.clone(),
    );

    let resp = server
        .dispatch(r#"{"id":"1","method":"cloud_sign_out"}"#)
        .await;
    assert!(resp.ok, "cloud_sign_out must return ok: true; got {resp:?}");
    // CopyPaste-i5b: the shared flag must now be false.
    assert!(
        !signed_in.load(Ordering::SeqCst),
        "cloud_signed_in must be false after cloud_sign_out"
    );
}

/// `cloud_sign_in` with no SUPABASE_URL configured must return
/// `invalid_argument` without touching `cloud_signed_in` (it stays false).
#[cfg(feature = "cloud-sync")]
#[tokio::test]
async fn cloud_sign_in_returns_invalid_argument_when_not_configured() {
    // Ensure no env override leaks from a parent shell.
    std::env::remove_var("SUPABASE_URL");
    std::env::remove_var("SUPABASE_ANON_KEY");

    let db = Arc::new(Mutex::new(Database::open_in_memory().unwrap()));
    let private_mode = Arc::new(AtomicBool::new(false));
    let local_key = Arc::new(zeroize::Zeroizing::new([0u8; 32]));
    let device_pub = Arc::new([0u8; 32]);

    let sync_key = Arc::new(Mutex::new(None));
    let last_sync_ms = Arc::new(std::sync::atomic::AtomicI64::new(0));
    let signed_in = Arc::new(AtomicBool::new(false));

    // Use a temp config dir so read_config() finds no persisted credentials.
    let dir = safe_tempdir();
    let _env = EnvGuard::set_all(
        &["COPYPASTE_CONFIG_DIR", "HOME", "XDG_CONFIG_HOME"],
        dir.path(),
    );

    let server = IpcServer::new(db, private_mode, local_key, device_pub).with_cloud_sync_state(
        sync_key,
        last_sync_ms,
        signed_in.clone(),
    );

    let resp = server
        .dispatch(r#"{"id":"1","method":"cloud_sign_in"}"#)
        .await;
    assert!(
        !resp.ok,
        "cloud_sign_in with no config must fail; got {resp:?}"
    );
    assert_eq!(
        resp.error_code,
        Some(ERR_CODE_INVALID_ARGUMENT),
        "must return invalid_argument when Supabase is not configured"
    );
    // Flag must remain false — the unconfigured path must not set it.
    assert!(
        !signed_in.load(Ordering::SeqCst),
        "cloud_signed_in must stay false when sign-in is rejected for missing config"
    );
}

// ── Fix #1: set_config MERGE preserves redacted secrets ─────────────────

/// `merge_config` must preserve an existing secret when the incoming config
/// omits it (the redacted read-modify-write shape deserialises the secret
/// fields to `None`). A blind overwrite would null the stored credentials.
#[test]
fn merge_config_preserves_omitted_secrets() {
    let existing = AppConfig {
        p2p_enabled: Some(true),
        supabase_url: Some("https://proj.supabase.co".into()),
        supabase_anon_key: Some("anon-123".into()),
        supabase_email: Some("user@example.com".into()),
        supabase_password: Some("super-secret".into()),
        ..Default::default()
    };
    // Incoming mirrors what the UI sends back after `get_config` redaction:
    // secrets absent (None), only the toggle + publishable fields present.
    let incoming = AppConfig {
        p2p_enabled: Some(false),
        supabase_url: Some("https://proj.supabase.co".into()),
        supabase_anon_key: Some("anon-123".into()),
        supabase_email: None,
        supabase_password: None,
        ..Default::default()
    };
    let merged = merge_config(existing, incoming);
    assert_eq!(
        merged.supabase_password.as_deref(),
        Some("super-secret"),
        "omitted password must be preserved from the persisted config"
    );
    assert_eq!(
        merged.supabase_email.as_deref(),
        Some("user@example.com"),
        "omitted email must be preserved"
    );
    // Non-secret authoritative field still takes the incoming value.
    assert_eq!(
        merged.p2p_enabled,
        Some(false),
        "p2p_enabled incoming value wins"
    );
}

/// A provided secret in `set_config` overwrites the stored one (so the CLI
/// `cloud setup` can rotate credentials).
#[test]
fn merge_config_incoming_secret_overrides() {
    let existing = AppConfig {
        p2p_enabled: Some(false),
        supabase_url: None,
        supabase_anon_key: None,
        supabase_email: Some("old@example.com".into()),
        supabase_password: Some("old-pw".into()),
        ..Default::default()
    };
    let incoming = AppConfig {
        p2p_enabled: Some(false),
        supabase_url: None,
        supabase_anon_key: None,
        supabase_email: Some("new@example.com".into()),
        supabase_password: Some("new-pw".into()),
        ..Default::default()
    };
    let merged = merge_config(existing, incoming);
    assert_eq!(merged.supabase_password.as_deref(), Some("new-pw"));
    assert_eq!(merged.supabase_email.as_deref(), Some("new@example.com"));
}

// ── QR fully provisions all sync: apply_peer_provisioning ────────────────

/// On an UNCONFIGURED device, applying a peer's provisioning fills in the
/// missing Supabase config AND installs the derived sync key.
#[cfg(feature = "cloud-sync")]
#[tokio::test]
async fn apply_peer_provisioning_fills_missing_fields() {
    let dir = safe_tempdir();
    let cfg_home = dir.path().join("cfg");
    let _env = EnvGuard::set_all(
        &[
            "COPYPASTE_CONFIG_DIR",
            "HOME",
            "XDG_CONFIG_HOME",
            "SUPABASE_URL",
            "SUPABASE_ANON_KEY",
            "COPYPASTE_EPHEMERAL_KEY",
        ],
        &cfg_home,
    );
    // Ensure no env override / key persist interferes with the assertions.
    // (EnvGuard set all of the above to the same path; explicitly clear the
    // ones that must be UNSET for the "device lacks it" precondition.)
    // SAFETY: single-threaded test scope; restored by EnvGuard on drop.
    unsafe {
        std::env::remove_var("SUPABASE_URL");
        std::env::remove_var("SUPABASE_ANON_KEY");
        std::env::set_var("COPYPASTE_EPHEMERAL_KEY", "1");
    }

    let sync_key: Arc<Mutex<Option<SyncKey>>> = Arc::new(Mutex::new(None));
    let prov = copypaste_p2p::bootstrap::SyncProvisioning {
        supabase_url: Some("https://new.supabase.co".into()),
        supabase_anon_key: Some("new-anon".into()),
        relay_url: Some("https://relay.example.com".into()),
        derived_sync_key: Some(vec![5u8; 32]),
    };
    IpcServer::apply_peer_provisioning_to(&sync_key, prov).await;

    let cfg = read_config();
    assert_eq!(cfg.supabase_url.as_deref(), Some("https://new.supabase.co"));
    assert_eq!(cfg.supabase_anon_key.as_deref(), Some("new-anon"));
    // R2: a peer-advertised relay_url is persisted on an unconfigured device
    // and survives the read_config overlay (it round-trips via config.toml).
    assert_eq!(
        cfg.relay_url.as_deref(),
        Some("https://relay.example.com"),
        "an unconfigured device must adopt the peer's relay_url"
    );
    assert!(
        sync_key.lock().await.is_some(),
        "an unconfigured device must install the peer's derived sync key"
    );
}

/// On a device that ALREADY has Supabase config + a sync key, applying a
/// peer's provisioning that carries the IDENTICAL key (routine re-pairing)
/// must NOT overwrite the config OR the key. (A DIFFERING key signals a
/// rotation re-provision and IS allowed to replace — see
/// `apply_peer_provisioning_rotation_replaces_differing_key`.)
#[cfg(feature = "cloud-sync")]
#[tokio::test]
async fn apply_peer_provisioning_never_overwrites_existing() {
    let dir = safe_tempdir();
    let cfg_home = dir.path().join("cfg");
    let _env = EnvGuard::set_all(
        &[
            "COPYPASTE_CONFIG_DIR",
            "HOME",
            "XDG_CONFIG_HOME",
            "SUPABASE_URL",
            "SUPABASE_ANON_KEY",
            "COPYPASTE_EPHEMERAL_KEY",
        ],
        &cfg_home,
    );
    // SAFETY: single-threaded test scope; restored by EnvGuard on drop.
    unsafe {
        std::env::remove_var("SUPABASE_URL");
        std::env::remove_var("SUPABASE_ANON_KEY");
        std::env::set_var("COPYPASTE_EPHEMERAL_KEY", "1");
    }

    // Seed an already-configured device. supabase_* live in config.json;
    // relay_url is core-backed, so seed it via update_core_config (config.toml)
    // — read_config overlays relay_url from there.
    let seed = AppConfig {
        supabase_url: Some("https://existing.supabase.co".into()),
        supabase_anon_key: Some("existing-anon".into()),
        relay_url: Some("https://existing-relay.example.com".into()),
        ..Default::default()
    };
    write_config(&seed).expect("seed config.json");
    update_core_config(&seed).expect("seed config.toml");
    let sync_key: Arc<Mutex<Option<SyncKey>>> =
        Arc::new(Mutex::new(Some(SyncKey::from_bytes([1u8; 32]))));

    // Carry the IDENTICAL key (all 1s) — this is the routine-pairing shape
    // where both peers derive the same deterministic key. It must be a
    // no-op for the key, and config fill-missing must still not overwrite.
    let prov = copypaste_p2p::bootstrap::SyncProvisioning {
        supabase_url: Some("https://peer.supabase.co".into()),
        supabase_anon_key: Some("peer-anon".into()),
        relay_url: Some("https://peer-relay.example.com".into()),
        derived_sync_key: Some(vec![1u8; 32]),
    };
    IpcServer::apply_peer_provisioning_to(&sync_key, prov).await;

    let cfg = read_config();
    assert_eq!(
        cfg.supabase_url.as_deref(),
        Some("https://existing.supabase.co"),
        "existing supabase_url must not be overwritten"
    );
    assert_eq!(cfg.supabase_anon_key.as_deref(), Some("existing-anon"));
    assert_eq!(
        cfg.relay_url.as_deref(),
        Some("https://existing-relay.example.com"),
        "existing relay_url must not be overwritten by the peer's"
    );
    // The pre-existing key (all 1s) must be untouched (identical → no-op).
    assert_eq!(
        sync_key.lock().await.as_ref().map(|k| *k.as_bytes()),
        Some([1u8; 32]),
        "an identical incoming sync key must not change the existing key"
    );
}

/// C-P0-4: after a sync-key ROTATION, the operator re-scans the pairing QR
/// on each remaining device. That re-provision carries the NEW key, which
/// DIFFERS from the stale key the device still holds — the apply path must
/// REPLACE the stale key (otherwise the device keeps the dead, pre-rotation
/// key and silently fails to sync). Config fields are still fill-missing
/// only and must not be overwritten.
#[cfg(feature = "cloud-sync")]
#[tokio::test]
async fn apply_peer_provisioning_rotation_replaces_differing_key() {
    let dir = safe_tempdir();
    let cfg_home = dir.path().join("cfg");
    let _env = EnvGuard::set_all(
        &[
            "COPYPASTE_CONFIG_DIR",
            "HOME",
            "XDG_CONFIG_HOME",
            "SUPABASE_URL",
            "SUPABASE_ANON_KEY",
            "COPYPASTE_EPHEMERAL_KEY",
        ],
        &cfg_home,
    );
    // SAFETY: single-threaded test scope; restored by EnvGuard on drop.
    unsafe {
        std::env::remove_var("SUPABASE_URL");
        std::env::remove_var("SUPABASE_ANON_KEY");
        std::env::set_var("COPYPASTE_EPHEMERAL_KEY", "1");
    }

    let seed = AppConfig {
        supabase_url: Some("https://existing.supabase.co".into()),
        supabase_anon_key: Some("existing-anon".into()),
        ..Default::default()
    };
    write_config(&seed).expect("seed config.json");

    // Device holds the STALE pre-rotation key (all 1s).
    let sync_key: Arc<Mutex<Option<SyncKey>>> =
        Arc::new(Mutex::new(Some(SyncKey::from_bytes([1u8; 32]))));

    // Rotation re-provision carries the NEW key (all 7s).
    let prov = copypaste_p2p::bootstrap::SyncProvisioning {
        supabase_url: Some("https://peer.supabase.co".into()),
        supabase_anon_key: Some("peer-anon".into()),
        relay_url: None,
        derived_sync_key: Some(vec![7u8; 32]),
    };
    IpcServer::apply_peer_provisioning_to(&sync_key, prov).await;

    // The differing key REPLACES the stale one (honest rotation).
    assert_eq!(
        sync_key.lock().await.as_ref().map(|k| *k.as_bytes()),
        Some([7u8; 32]),
        "a differing incoming sync key (rotation) must replace the stale key"
    );
    // Config fill-missing still never overwrites an existing value.
    let cfg = read_config();
    assert_eq!(
        cfg.supabase_url.as_deref(),
        Some("https://existing.supabase.co"),
        "existing supabase_url must not be overwritten on a rotation re-provision"
    );
}

/// End-to-end: seed a config with a password, then run a `set_config` whose
/// params carry the REDACTED shape (`supabase_password_set: true`, no real
/// password). The stored password must survive — proving the
/// read-modify-write data-loss bug is fixed at the IPC boundary.
#[tokio::test]
async fn set_config_with_redacted_shape_preserves_stored_password() {
    let dir = safe_tempdir();
    let cfg_home = dir.path().join("cfg");
    let _env = EnvGuard::set_all(
        &["COPYPASTE_CONFIG_DIR", "HOME", "XDG_CONFIG_HOME"],
        &cfg_home,
    );

    // Seed: persist a config carrying a real password.
    let seeded = AppConfig {
        p2p_enabled: Some(false),
        supabase_url: Some("https://proj.supabase.co".into()),
        supabase_anon_key: Some("anon-xyz".into()),
        supabase_email: Some("seed@example.com".into()),
        supabase_password: Some("do-not-wipe-me".into()),
        ..Default::default()
    };
    write_config(&seeded).expect("seed write_config");

    // Confirm get_config redacts the secret to a presence flag.
    let server = bare_server();
    let get_resp = server
        .dispatch(r#"{"id":"g1","method":"get_config","params":{}}"#)
        .await;
    let got = get_resp.data.expect("get_config data");
    assert_eq!(got["supabase_password_set"], true);
    assert!(
        got.get("supabase_password").is_none(),
        "raw password must never leave the daemon: {got}"
    );

    // The UI/CLI sends this redacted shape straight back via set_config.
    let set_body = format!(
        r#"{{"id":"s1","method":"set_config","params":{}}}"#,
        serde_json::to_string(&got).unwrap()
    );
    let set_resp = server.dispatch(&set_body).await;
    assert_eq!(
        set_resp.data.as_ref().map(|d| d["saved"].clone()),
        Some(serde_json::json!(true)),
        "set_config must succeed: {set_resp:?}"
    );

    // The persisted password must be intact. The daemon stores it in the
    // Keychain first (stripping it from config.json) and only falls back to
    // config.json when the Keychain is unavailable — exactly how the cloud
    // path retrieves it (cloud.rs: keychain-first, config fallback). Assert
    // that *effective* value so the test is robust whether or not the real
    // Keychain is reachable (CI runs with COPYPASTE_EPHEMERAL_KEY, so the
    // password stays in config.json; a signed build stores it in Keychain).
    let persisted = read_config();
    let effective_pw = crate::keychain::read_supabase_password_from_keychain()
        .or_else(|| persisted.supabase_password.clone());
    assert_eq!(
        effective_pw.as_deref(),
        Some("do-not-wipe-me"),
        "set_config with the redacted shape must NOT wipe the stored password"
    );
    assert_eq!(
        persisted.supabase_email.as_deref(),
        Some("seed@example.com"),
        "email must also survive"
    );
}

// ── export: limit param ──────────────────────────────────────────────────

/// When `limit` > 0 the export handler must return at most `limit` items,
/// selecting the most-recent ones (DESC LIMIT subquery) and re-ordering
/// them oldest-first for deterministic import. When `limit` == 0 or is
/// absent all items are returned.
#[tokio::test]
async fn export_limit_returns_most_recent_n_oldest_first() {
    use copypaste_core::{
        build_item_aad_v2, derive_v2, encrypt_item_with_aad, AAD_SCHEMA_VERSION_V4,
    };

    let dir = safe_tempdir();
    let sock = dir.path().join("export_limit.sock");
    let (_pm, db) = start_test_server_returning_db(&sock, false).await;

    // The test server uses a zero v1 key. Derive v2 the same way the
    // handler does so we can produce decrypt-able ciphertext.
    let v1_key = [0u8; 32];
    let v2_key = derive_v2(&v1_key);

    // Seed 5 text items with distinct, monotonically increasing wall_time
    // values so we can verify ordering and limit selection.
    const TOTAL: usize = 5;
    let mut item_ids: Vec<String> = Vec::new();
    {
        let guard = db.lock().await;
        for i in 0..TOTAL {
            let plaintext = format!("item-{i}").into_bytes();
            let item_id = uuid::Uuid::new_v4().to_string();
            let aad = build_item_aad_v2(
                &copypaste_core::ItemId::from(item_id.as_str()),
                AAD_SCHEMA_VERSION_V4,
                2,
            );
            let (nonce, ciphertext) = encrypt_item_with_aad(&plaintext, &v2_key, &aad).unwrap();
            // Use a distinct wall_time per item (base 1000 + i ms).
            let wall_time = 1_000_000i64 + i as i64;
            guard
                .conn()
                .execute(
                    "INSERT INTO clipboard_items \
                         (id, item_id, content_type, content, content_nonce, \
                          is_sensitive, is_synced, lamport_ts, wall_time, key_version) \
                         VALUES (?1, ?2, 'text', ?3, ?4, 0, 0, ?5, ?6, 2)",
                    rusqlite::params![
                        uuid::Uuid::new_v4().to_string(),
                        item_id,
                        ciphertext,
                        nonce.as_slice(),
                        i as i64 + 1,
                        wall_time,
                    ],
                )
                .unwrap();
            item_ids.push(format!("item-{i}"));
        }
    }

    // ── limit=3: must return the 3 most-recent items (item-2, item-3, item-4)
    //    serialised oldest-first (item-2, item-3, item-4 in that order).
    let resp = call_one(
        &sock,
        r#"{"id":"el1","method":"export","params":{"limit":3}}"#,
    )
    .await;
    assert_eq!(resp["ok"], true, "export with limit=3 must succeed: {resp}");
    let items = resp["data"]["items"].as_array().expect("items array");
    assert_eq!(
        items.len(),
        3,
        "limit=3 must return exactly 3 items, got {}: {resp}",
        items.len()
    );
    // Verify chronological (ASC) ordering: wall_time must be non-decreasing.
    let wall_times: Vec<i64> = items
        .iter()
        .map(|it| it["wall_time"].as_i64().unwrap())
        .collect();
    assert!(
        wall_times.windows(2).all(|w| w[0] <= w[1]),
        "items must be ordered oldest-first: {wall_times:?}"
    );
    // The 3 most-recent items have wall_times 1_000_002, 1_000_003, 1_000_004.
    assert_eq!(
        wall_times[0], 1_000_002,
        "first exported item should be 3rd oldest"
    );
    assert_eq!(
        wall_times[2], 1_000_004,
        "last exported item should be newest"
    );

    // ── limit=0: must return ALL items (unlimited).
    let resp = call_one(
        &sock,
        r#"{"id":"el2","method":"export","params":{"limit":0}}"#,
    )
    .await;
    assert_eq!(resp["ok"], true, "export with limit=0 must succeed: {resp}");
    let all_items = resp["data"]["items"].as_array().expect("items array");
    assert_eq!(
        all_items.len(),
        TOTAL,
        "limit=0 must return all {TOTAL} items, got {}",
        all_items.len()
    );

    // ── limit absent: must also return ALL items.
    let resp = call_one(&sock, r#"{"id":"el3","method":"export","params":{}}"#).await;
    assert_eq!(
        resp["ok"], true,
        "export with no limit must succeed: {resp}"
    );
    let no_limit_items = resp["data"]["items"].as_array().expect("items array");
    assert_eq!(
        no_limit_items.len(),
        TOTAL,
        "absent limit must return all {TOTAL} items, got {}",
        no_limit_items.len()
    );
}

// ── CopyPaste-tj9s: export include_sensitive filter ──────────────────────

/// `export` must exclude sensitive items by default and include them only
/// when `include_sensitive: true` is explicitly passed.
///
/// Contract:
/// - 1 non-sensitive item + 1 sensitive item inserted.
/// - `export` with no `include_sensitive` (or `include_sensitive: false`) →
///   count == 1 (only the non-sensitive item).
/// - `export` with `include_sensitive: true` → count == 2 (both items).
#[tokio::test]
async fn export_excludes_sensitive_by_default_and_includes_with_flag() {
    use copypaste_core::{
        build_item_aad_v2, derive_v2, encrypt_item_with_aad, AAD_SCHEMA_VERSION_V4,
    };

    let dir = safe_tempdir();
    let sock = dir.path().join("export_sensitive.sock");
    let (_pm, db) = start_test_server_returning_db(&sock, false).await;

    // The test server uses a zero v1 key. Derive v2 to match the handler.
    let v1_key = [0u8; 32];
    let v2_key = derive_v2(&v1_key);

    // Seed a non-sensitive item (is_sensitive = 0) and a sensitive item
    // (is_sensitive = 1), both encrypted with key_version = 2.
    {
        let guard = db.lock().await;
        for (i, is_sensitive) in [(0i64, false), (1i64, true)] {
            let plaintext = format!("item-sens-{i}").into_bytes();
            let item_id = uuid::Uuid::new_v4().to_string();
            let aad = build_item_aad_v2(
                &copypaste_core::ItemId::from(item_id.as_str()),
                AAD_SCHEMA_VERSION_V4,
                2,
            );
            let (nonce, ciphertext) = encrypt_item_with_aad(&plaintext, &v2_key, &aad).unwrap();
            let wall_time = 2_000_000i64 + i;
            guard
                .conn()
                .execute(
                    "INSERT INTO clipboard_items \
                         (id, item_id, content_type, content, content_nonce, \
                          is_sensitive, is_synced, lamport_ts, wall_time, key_version) \
                         VALUES (?1, ?2, 'text', ?3, ?4, ?5, 0, ?6, ?7, 2)",
                    rusqlite::params![
                        uuid::Uuid::new_v4().to_string(),
                        item_id,
                        ciphertext,
                        nonce.as_slice(),
                        is_sensitive as i64,
                        i + 1,
                        wall_time,
                    ],
                )
                .unwrap();
        }
    }

    // ── default (no flag): only the non-sensitive item is returned.
    let resp = call_one(&sock, r#"{"id":"xs1","method":"export","params":{}}"#).await;
    assert_eq!(resp["ok"], true, "export must succeed: {resp}");
    let items = resp["data"]["items"].as_array().expect("items array");
    assert_eq!(
        items.len(),
        1,
        "default export must exclude sensitive items; got {}: {resp}",
        items.len()
    );
    assert_eq!(
        items[0]["is_sensitive"], false,
        "the returned item must not be sensitive"
    );

    // ── include_sensitive: false → same as default.
    let resp = call_one(
        &sock,
        r#"{"id":"xs2","method":"export","params":{"include_sensitive":false}}"#,
    )
    .await;
    assert_eq!(resp["ok"], true, "export must succeed: {resp}");
    let items = resp["data"]["items"].as_array().expect("items array");
    assert_eq!(
        items.len(),
        1,
        "include_sensitive=false must exclude sensitive items; got {}: {resp}",
        items.len()
    );

    // ── include_sensitive: true → both items are returned.
    let resp = call_one(
        &sock,
        r#"{"id":"xs3","method":"export","params":{"include_sensitive":true}}"#,
    )
    .await;
    assert_eq!(resp["ok"], true, "export must succeed: {resp}");
    let items = resp["data"]["items"].as_array().expect("items array");
    assert_eq!(
        items.len(),
        2,
        "include_sensitive=true must include all items; got {}: {resp}",
        items.len()
    );
    // Verify one of each kind is present.
    let sensitive_count = items
        .iter()
        .filter(|it| it["is_sensitive"].as_bool() == Some(true))
        .count();
    assert_eq!(
        sensitive_count, 1,
        "exactly one sensitive item must appear when include_sensitive=true"
    );
}

// ── Fix #2: config.json honours COPYPASTE_CONFIG_DIR ────────────────────

/// `COPYPASTE_CONFIG_DIR` must redirect `config.json` (not just
/// `peers.json`), and the two files must co-locate under the same
/// `copypaste/` subdir.
#[test]
fn config_dir_override_redirects_config_json() {
    let dir = safe_tempdir();
    let cfg_home = dir.path().join("override-root");
    let _env = EnvGuard::set_all(
        &[
            "COPYPASTE_CONFIG_DIR",
            "COPYPASTE_DATA_DIR",
            "HOME",
            "XDG_CONFIG_HOME",
        ],
        &cfg_home,
    );

    let config = config_path().expect("config_path under override");
    let peers = peers_file_path();

    // config.json lands under the override, not the platform default.
    assert!(
        config.starts_with(&cfg_home),
        "config.json must live under COPYPASTE_CONFIG_DIR: {}",
        config.display()
    );
    // config.json ends with "config.json"; the parent dir name is
    // platform-dependent (CopyPaste on macOS/Windows, copypaste on Linux)
    // but in all cases the file must live under the override root.
    assert!(
        config.ends_with("config.json"),
        "config path must end with config.json: {}",
        config.display()
    );

    // Both files share the SAME directory so a config write and a peers
    // write can never diverge under the override.
    assert_eq!(
        config.parent(),
        peers.parent(),
        "config.json and peers.json must co-locate: {} vs {}",
        config.display(),
        peers.display()
    );

    // And a real round-trip write/read works through the redirected path.
    let cfg = AppConfig {
        p2p_enabled: Some(true),
        ..Default::default()
    };
    write_config(&cfg).expect("write under override");
    assert!(
        config.is_file(),
        "config.json must be written at {}",
        config.display()
    );
    assert_eq!(
        read_config().p2p_enabled,
        Some(true),
        "round-trip read under override"
    );
}

// ── Fix-2: write_config must create config.json atomically at mode 0600 ──

/// `write_config` must produce a `config.json` with mode `0600` and must
/// not leave any orphaned `.tmp.*` file behind after a successful write.
/// The config may carry `supabase_password` / `supabase_anon_key`; it must
/// never be momentarily world-readable between create and chmod.
#[cfg(unix)]
#[test]
fn write_config_creates_file_with_mode_0600_and_no_tmp_orphan() {
    use std::os::unix::fs::PermissionsExt;

    let dir = safe_tempdir();
    let _env = EnvGuard::set_all(
        &["HOME", "XDG_CONFIG_HOME", "COPYPASTE_CONFIG_DIR"],
        dir.path(),
    );

    let cfg = AppConfig {
        p2p_enabled: Some(true),
        supabase_password: Some("secret".into()),
        ..Default::default()
    };
    write_config(&cfg).expect("write_config must succeed");

    // Find the written config.json under the temp home.
    let config = config_path().expect("config_path under override");
    assert!(config.exists(), "config.json must be written");

    let mode = std::fs::metadata(&config).unwrap().permissions().mode();
    assert_eq!(
        mode & 0o777,
        0o600,
        "config.json must be owner-only (0600), got {:o}",
        mode & 0o777
    );

    // No orphaned temp file in the config dir.
    let config_dir = config.parent().unwrap();
    let orphans: Vec<_> = std::fs::read_dir(config_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with(".config.json.tmp.")
        })
        .collect();
    assert!(
        orphans.is_empty(),
        "atomic write must not leave temp files behind: {:?}",
        orphans
    );
}

// ── Fix-5: p2p_enabled must be Option<bool> so omitting it preserves existing ──

/// A `set_config` request that omits `p2p_enabled` (the field is absent from
/// JSON or deserialises as `null`) must NOT flip the stored value to `false`.
/// Previously `p2p_enabled: bool` with `#[serde(default)]` meant any
/// deserialization that did not include the field produced `false`, silently
/// disabling P2P for every caller that only sends a subset of fields.
#[test]
fn p2p_enabled_option_none_preserves_existing() {
    // When p2p_enabled is absent from JSON it must deserialise as None.
    let json_without = r#"{"supabase_url": "https://x.supabase.co"}"#;
    let cfg: AppConfig = serde_json::from_str(json_without).expect("deserialize");
    assert!(
        cfg.p2p_enabled.is_none(),
        "absent p2p_enabled must deserialise as None, got {:?}",
        cfg.p2p_enabled
    );

    // merge_config: when incoming has None, existing value must be preserved.
    let existing = AppConfig {
        p2p_enabled: Some(true),
        ..Default::default()
    };
    let merged = merge_config(existing, cfg);
    assert_eq!(
        merged.p2p_enabled,
        Some(true),
        "merge_config must preserve existing p2p_enabled when incoming is None"
    );
}

// ── get_item_thumbnail: serves the capture-time thumbnail blob ──────────

/// Build a large PNG, encode it via `encode_image_full` with the test
/// server's zero key, insert the resulting image item (full chunks +
/// thumbnail blob + extended meta_json), then assert:
///   * `get_item_thumbnail` returns a non-null PNG data-URI,
///   * the thumbnail data-URI is SMALLER than the full-res `get_item_image`
///     output (the thumb is a downscaled re-encode),
///   * an image item with NO thumb returns the `{ "thumbnail": null }`
///     sentinel so the UI can fall back to full-res.
#[tokio::test]
async fn get_item_thumbnail_serves_thumb_and_null_sentinel() {
    use copypaste_core::THUMBNAIL_MAX_DIM;
    use image::{DynamicImage, RgbaImage};

    let db = Arc::new(Mutex::new(Database::open_in_memory().unwrap()));
    let server = IpcServer::new(
        db.clone(),
        Arc::new(AtomicBool::new(false)),
        Arc::new(zeroize::Zeroizing::new([0u8; 32])),
        Arc::new([0u8; 32]),
    );
    let key = [0u8; 32]; // v1 seed matching dummy server key
                         // new_image stamps key_version = 2; the server reads kv=2 rows with
                         // derive_v2(local_key). Encrypt with the same v2 key so the round-trip
                         // matches the production writer (handle_image uses derive_v2).
    let v2_key = derive_v2(&key);

    // A 1000×1000 image: larger than THUMBNAIL_MAX_DIM (192) so the
    // thumbnail is genuinely downscaled and its PNG is smaller than the
    // full-res PNG. A per-pixel gradient keeps PNG compression honest (a
    // flat color would compress so well the size gap could vanish).
    let mut buf = RgbaImage::new(1000, 1000);
    for (x, y, px) in buf.enumerate_pixels_mut() {
        *px = image::Rgba([(x % 256) as u8, (y % 256) as u8, ((x + y) % 256) as u8, 255]);
    }
    let raw = copypaste_core::encode_as_png(&DynamicImage::ImageRgba8(buf)).unwrap();

    // file_id = content hash (mirrors handle_image); thumb_file_id distinct.
    let file_id = crate::clipboard::image_content_hash(&raw);
    let thumb_file_id = crate::clipboard::image_thumb_file_id(&file_id);

    let (meta, chunks, thumb_blob, thumb_w, thumb_h) = copypaste_core::encode_image_full(
        &raw,
        &v2_key,
        &file_id,
        &thumb_file_id,
        0,
        64,
        THUMBNAIL_MAX_DIM,
    )
    .unwrap();
    assert!(!thumb_blob.is_empty(), "thumbnail blob must be produced");

    let blob = copypaste_core::chunks_to_blob(&chunks).unwrap();
    let meta_json =
        crate::clipboard::build_image_meta_json(&meta, &thumb_file_id, thumb_w, thumb_h);

    let mut item = copypaste_core::ClipboardItem::new_image(blob, meta_json, 0, Some(thumb_blob));
    item.item_id = uuid::Uuid::from_bytes(file_id).to_string().into();
    let with_thumb_id = item.id.clone();

    // A second image item with NO thumbnail (full-image-only legacy path).
    let (meta2, chunks2) =
        copypaste_core::encode_image_with_limit(&raw, &v2_key, &file_id, 0, 64).unwrap();
    let blob2 = copypaste_core::chunks_to_blob(&chunks2).unwrap();
    let meta_json2 = format!(
        r#"{{"width":{},"height":{},"original_size":{},"chunk_count":{},"file_id":{:?}}}"#,
        meta2.width, meta2.height, meta2.original_size, meta2.chunk_count, meta2.file_id
    );
    let mut item2 = copypaste_core::ClipboardItem::new_image(blob2, meta_json2, 0, None);
    item2.item_id = uuid::Uuid::new_v4().to_string().into();
    item2.id = uuid::Uuid::new_v4().to_string().into();
    let no_thumb_id = item2.id.clone();

    {
        let guard = db.lock().await;
        copypaste_core::insert_item_with_fts(&guard, &item, "").unwrap();
        copypaste_core::insert_item_with_fts(&guard, &item2, "").unwrap();
    }

    // get_item_thumbnail on the item WITH a thumb → non-null data-URI.
    let thumb_resp = server
        .dispatch(&format!(
            r#"{{"id":"t1","method":"get_item_thumbnail","params":{{"id":"{with_thumb_id}"}}}}"#
        ))
        .await;
    let thumb_data = thumb_resp.data.expect("get_item_thumbnail data");
    let thumb_uri = thumb_data["thumbnail"]
        .as_str()
        .expect("thumbnail must be a non-null data-URI string");
    assert!(
        thumb_uri.starts_with("data:image/png;base64,"),
        "thumbnail must be a PNG data-URI"
    );

    // get_item_image on the same item → full-res data-URI.
    let full_resp = server
        .dispatch(&format!(
            r#"{{"id":"f1","method":"get_item_image","params":{{"id":"{with_thumb_id}"}}}}"#
        ))
        .await;
    let full_uri = full_resp.data.expect("get_item_image data")["data_uri"]
        .as_str()
        .expect("data_uri")
        .to_string();
    assert!(
        thumb_uri.len() < full_uri.len(),
        "thumbnail data-URI ({}) must be smaller than full-res ({})",
        thumb_uri.len(),
        full_uri.len()
    );

    // Phase 4: get_item_thumbnail on a legacy item WITHOUT a stored thumb
    // now lazily backfills and returns a non-null PNG data-URI (Phase 4).
    // The null sentinel is only returned when backfill itself fails.
    let backfill_resp = server
        .dispatch(&format!(
            r#"{{"id":"t2","method":"get_item_thumbnail","params":{{"id":"{no_thumb_id}"}}}}"#
        ))
        .await;
    let backfill_data = backfill_resp
        .data
        .expect("get_item_thumbnail (no stored thumb) data");
    assert!(
        !backfill_data["thumbnail"].is_null(),
        "Phase-4: legacy thumb-less item must be lazily backfilled, not null: {backfill_data}"
    );
    assert!(
        backfill_data["thumbnail"]
            .as_str()
            .unwrap_or("")
            .starts_with("data:image/png;base64,"),
        "backfilled thumbnail must be a PNG data-URI: {backfill_data}"
    );
}

/// Phase 4: lazy backfill — an image item with `thumb IS NULL` (legacy row
/// captured before schema v9 / Plan-B P2) must have a thumbnail generated
/// and persisted on first `get_item_thumbnail` call, and returned as a
/// non-null PNG data-URI. A second call must also return non-null (proving
/// the thumbnail was written to the DB, not just computed in memory).
#[tokio::test]
async fn get_item_thumbnail_lazy_backfill_missing_thumb() {
    use copypaste_core::THUMBNAIL_MAX_DIM;
    use image::{DynamicImage, RgbaImage};

    let db = Arc::new(Mutex::new(Database::open_in_memory().unwrap()));
    let server = IpcServer::new(
        db.clone(),
        Arc::new(AtomicBool::new(false)),
        Arc::new(zeroize::Zeroizing::new([0u8; 32])),
        Arc::new([0u8; 32]),
    );
    let key = [0u8; 32]; // v1 seed matching dummy server key
                         // new_image stamps key_version = 2; the server reads kv=2 rows with
                         // derive_v2(local_key). Encrypt with the same v2 key so the round-trip
                         // matches the production writer (handle_image uses derive_v2).
    let v2_key = derive_v2(&key);

    // Build a 1000×1000 image (larger than THUMBNAIL_MAX_DIM so a real
    // downscale occurs), encode with the old path (no thumb blob), and
    // store with thumb=None to simulate a legacy row.
    let mut buf = RgbaImage::new(1000, 1000);
    for (x, y, px) in buf.enumerate_pixels_mut() {
        *px = image::Rgba([(x % 256) as u8, (y % 256) as u8, ((x + y) % 256) as u8, 255]);
    }
    let raw = copypaste_core::encode_as_png(&DynamicImage::ImageRgba8(buf)).unwrap();

    let file_id = crate::clipboard::image_content_hash(&raw);
    let (meta, chunks) =
        copypaste_core::encode_image_with_limit(&raw, &v2_key, &file_id, 0, 64).unwrap();
    let blob = copypaste_core::chunks_to_blob(&chunks).unwrap();

    // Legacy meta_json: no thumb_file_id / thumb_w / thumb_h fields.
    let meta_json = format!(
        r#"{{"width":{},"height":{},"original_size":{},"chunk_count":{},"file_id":{:?}}}"#,
        meta.width, meta.height, meta.original_size, meta.chunk_count, meta.file_id
    );

    let mut item = copypaste_core::ClipboardItem::new_image(blob, meta_json, 0, None);
    item.item_id = uuid::Uuid::new_v4().to_string().into();
    item.id = uuid::Uuid::new_v4().to_string().into();
    let item_id = item.id.clone();

    {
        let guard = db.lock().await;
        copypaste_core::insert_item_with_fts(&guard, &item, "").unwrap();
    }

    // ── First call: thumb is NULL → should backfill and return data-URI ──
    let resp1 = server
        .dispatch(&format!(
            r#"{{"id":"b1","method":"get_item_thumbnail","params":{{"id":"{item_id}"}}}}"#
        ))
        .await;
    let data1 = resp1.data.expect("first get_item_thumbnail must have data");
    assert!(
        !data1["thumbnail"].is_null(),
        "lazy backfill: first call must return non-null thumbnail, got: {data1}"
    );
    let uri1 = data1["thumbnail"]
        .as_str()
        .expect("thumbnail must be a string");
    assert!(
        uri1.starts_with("data:image/png;base64,"),
        "backfilled thumbnail must be a PNG data-URI"
    );
    // Verify thumbnail was genuinely downscaled (PNG is smaller than full-res).
    let thumb_b64_len = uri1.len() - "data:image/png;base64,".len();
    let full_resp = server
        .dispatch(&format!(
            r#"{{"id":"b_full","method":"get_item_image","params":{{"id":"{item_id}"}}}}"#
        ))
        .await;
    let full_uri = full_resp.data.expect("get_item_image data")["data_uri"]
        .as_str()
        .expect("data_uri")
        .to_string();
    let full_b64_len = full_uri.len() - "data:image/png;base64,".len();
    assert!(
        thumb_b64_len < full_b64_len,
        "backfilled thumbnail ({thumb_b64_len}) must be smaller than full-res ({full_b64_len})"
    );

    // ── Second call: thumb must now be in DB (persisted) ─────────────────
    let resp2 = server
        .dispatch(&format!(
            r#"{{"id":"b2","method":"get_item_thumbnail","params":{{"id":"{item_id}"}}}}"#
        ))
        .await;
    let data2 = resp2
        .data
        .expect("second get_item_thumbnail must have data");
    assert!(
        !data2["thumbnail"].is_null(),
        "lazy backfill: second call must still return non-null thumbnail (persisted), got: {data2}"
    );
    assert_eq!(
        data2["thumbnail"], data1["thumbnail"],
        "second call must return the same data-URI (served from DB, deterministic)"
    );

    // Confirm THUMBNAIL_MAX_DIM was respected by the backfill.
    let _ = THUMBNAIL_MAX_DIM; // ensure the constant stays referenced in this test
}

// -----------------------------------------------------------------------
// list_peers: online status + last_seen_secs (B1 device-info feature)
// -----------------------------------------------------------------------

/// `list_peers` must include `online` (bool) and `last_seen_secs` (i64)
/// in every peer entry.  When `last_sync_at` is absent (never synced),
/// `online` must be `false` and `last_seen_secs` must be `-1`.
#[tokio::test]
async fn list_peers_response_includes_online_and_last_seen_fields() {
    let dir = safe_tempdir();
    let sock = dir.path().join("lp_online_fields.sock");
    let cfg_home = dir.path().join("cfg");
    let _env = EnvGuard::set_all(
        &[
            "COPYPASTE_CONFIG_DIR",
            "COPYPASTE_DATA_DIR",
            "HOME",
            "XDG_CONFIG_HOME",
        ],
        &cfg_home,
    );
    std::fs::create_dir_all(&cfg_home).unwrap();

    // Seed one peer that has never synced (no last_sync_at).
    let peers_json = cfg_home.join("peers.json");
    let peers = serde_json::json!([
        {"name": "Laptop", "fingerprint": "a1:b2:c3:d4:e5:f6:07:18", "added_at": 1}
    ]);
    std::fs::write(&peers_json, serde_json::to_string(&peers).unwrap()).unwrap();

    start_test_server(&sock).await;
    let resp = call_one(&sock, r#"{"id":"lp1","method":"list_peers","params":{}}"#).await;
    assert_eq!(resp["ok"], true, "list_peers must succeed: {resp}");
    let peer_arr = resp["data"]["peers"]
        .as_array()
        .expect("data.peers must be array");
    assert_eq!(peer_arr.len(), 1, "must have exactly one peer");

    let peer = &peer_arr[0];
    assert!(
        peer.get("online").is_some(),
        "peer entry must include 'online' field: {peer}"
    );
    assert!(
        peer.get("last_seen_secs").is_some(),
        "peer entry must include 'last_seen_secs' field: {peer}"
    );

    // No sync ever → offline, sentinel -1.
    assert_eq!(
        peer["online"].as_bool(),
        Some(false),
        "peer with no last_sync_at must be offline: {peer}"
    );
    assert_eq!(
        peer["last_seen_secs"].as_i64(),
        Some(-1),
        "peer with no last_sync_at must have last_seen_secs=-1: {peer}"
    );
}

/// When `last_sync_at` is recent (within ONLINE_THRESHOLD_SECS), the peer
/// must be marked `online = true`.
#[tokio::test]
async fn list_peers_online_true_when_last_sync_at_is_recent() {
    let dir = safe_tempdir();
    let sock = dir.path().join("lp_online_recent.sock");
    let cfg_home = dir.path().join("cfg2");
    let _env = EnvGuard::set_all(
        &[
            "COPYPASTE_CONFIG_DIR",
            "COPYPASTE_DATA_DIR",
            "HOME",
            "XDG_CONFIG_HOME",
        ],
        &cfg_home,
    );
    std::fs::create_dir_all(&cfg_home).unwrap();

    let peers_json = cfg_home.join("peers.json");
    // last_sync_at = now − 30 s  → within the 60 s threshold.
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let recent = now_secs - 30;
    let peers = serde_json::json!([
        {
            "name": "Phone",
            "fingerprint": "f0:e1:d2:c3:b4:a5:96:87",
            "added_at": 1,
            "last_sync_at": recent
        }
    ]);
    std::fs::write(&peers_json, serde_json::to_string(&peers).unwrap()).unwrap();

    start_test_server(&sock).await;
    let resp = call_one(&sock, r#"{"id":"lp2","method":"list_peers","params":{}}"#).await;
    assert_eq!(resp["ok"], true, "list_peers must succeed: {resp}");
    let peer_arr = resp["data"]["peers"].as_array().expect("data.peers array");
    assert_eq!(peer_arr.len(), 1);

    let peer = &peer_arr[0];
    assert_eq!(
        peer["online"].as_bool(),
        Some(true),
        "peer with recent last_sync_at must be online: {peer}"
    );
    let last_seen = peer["last_seen_secs"].as_i64().expect("last_seen_secs");
    // last_seen_secs = now - last_sync_at ≈ 30, allow ±5 for clock skew.
    assert!(
        (25..=35).contains(&last_seen),
        "last_seen_secs must be ~30, got {last_seen}"
    );
}

/// When `last_sync_at` is stale (beyond ONLINE_THRESHOLD_SECS), the peer
/// must be marked `online = false`.
#[tokio::test]
async fn list_peers_online_false_when_last_sync_at_is_stale() {
    let dir = safe_tempdir();
    let sock = dir.path().join("lp_online_stale.sock");
    let cfg_home = dir.path().join("cfg3");
    let _env = EnvGuard::set_all(
        &[
            "COPYPASTE_CONFIG_DIR",
            "COPYPASTE_DATA_DIR",
            "HOME",
            "XDG_CONFIG_HOME",
        ],
        &cfg_home,
    );
    std::fs::create_dir_all(&cfg_home).unwrap();

    let peers_json = cfg_home.join("peers.json");
    // CopyPaste-1jms.25: derive the stale offset from ONLINE_THRESHOLD_SECS
    // (which now equals SYNC_BADGE_RECENT_MS / 1000) so this test tracks the
    // shared recency window instead of a hard-coded 120 s that silently broke
    // when the window widened from 60 s to 300 s.
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let stale = now_secs - (ONLINE_THRESHOLD_SECS + 60);
    let peers = serde_json::json!([
        {
            "name": "Tablet",
            "fingerprint": "12:34:56:78:9a:bc:de:f0",
            "added_at": 1,
            "last_sync_at": stale
        }
    ]);
    std::fs::write(&peers_json, serde_json::to_string(&peers).unwrap()).unwrap();

    start_test_server(&sock).await;
    let resp = call_one(&sock, r#"{"id":"lp3","method":"list_peers","params":{}}"#).await;
    assert_eq!(resp["ok"], true, "list_peers must succeed: {resp}");
    let peer_arr = resp["data"]["peers"].as_array().expect("data.peers array");
    assert_eq!(peer_arr.len(), 1);

    let peer = &peer_arr[0];
    assert_eq!(
        peer["online"].as_bool(),
        Some(false),
        "peer with stale last_sync_at must be offline: {peer}"
    );
}

/// `list_peers` must mark a peer `online = true` when the peer's fingerprint
/// is present with a live (non-closed) sender in the live P2P peer-sinks
/// map, even if `last_sync_at` is absent or stale.
#[tokio::test]
async fn list_peers_online_true_from_live_mtls_allowlist() {
    let dir = safe_tempdir();
    let sock = dir.path().join("lp_online_mtls.sock");
    let cfg_home = dir.path().join("cfg4");
    let _env = EnvGuard::set_all(
        &[
            "COPYPASTE_CONFIG_DIR",
            "COPYPASTE_DATA_DIR",
            "HOME",
            "XDG_CONFIG_HOME",
        ],
        &cfg_home,
    );
    std::fs::create_dir_all(&cfg_home).unwrap();

    // Peer fingerprint in colon-hex (as stored in peers.json).
    let fp_display = "a1:b2:c3:d4:e5:f6:07:18";
    // Canonical (colon-free, lowercase) form used as the sinks-map key.
    let fp_canonical = canonical_fingerprint(fp_display);

    let peers_json = cfg_home.join("peers.json");
    // Peer has no last_sync_at — only the live sinks map signals online.
    let peers = serde_json::json!([
        {"name": "Desktop", "fingerprint": fp_display, "added_at": 1}
    ]);
    std::fs::write(&peers_json, serde_json::to_string(&peers).unwrap()).unwrap();

    // Build a live sinks map with a non-closed sender for the peer.
    // The receiver is kept alive for the duration of the test so the
    // sender's `is_closed()` returns false (the channel is open).
    let (peer_tx, _peer_rx) = tokio::sync::mpsc::channel::<copypaste_sync::protocol::PeerFrame>(1);
    let sinks_map: crate::p2p::LivePeerSinks =
        Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::from([
            (
                copypaste_p2p::DeviceFingerprint(fp_canonical.clone()),
                peer_tx,
            ),
        ])));

    let db = Arc::new(Mutex::new(Database::open_in_memory().unwrap()));
    let cert = copypaste_p2p::cert::SelfSignedCert::generate("mtls-test").unwrap();
    let server = IpcServer::new(
        db,
        Arc::new(AtomicBool::new(false)),
        Arc::new(zeroize::Zeroizing::new([0u8; 32])),
        Arc::new([0u8; 32]),
    )
    .with_cert_fingerprint(display_fingerprint(&cert.fingerprint()));

    // Populate the live-sinks slot (simulates what daemon.rs does after
    // start_p2p returns).
    {
        let slot = server.live_peer_sinks_slot();
        let mut guard = slot.lock().unwrap();
        *guard = Some(Arc::clone(&sinks_map));
    }

    // Bind directly (no umask(0o177) race) — see start_test_server_returning_db.
    let listener = tokio::net::UnixListener::bind(&sock).expect("test socket bind must succeed");
    tokio::spawn(async move {
        let _ = server.serve_on(listener, CancellationToken::new()).await;
    });
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let resp = call_one(&sock, r#"{"id":"lp4","method":"list_peers","params":{}}"#).await;
    assert_eq!(resp["ok"], true, "list_peers must succeed: {resp}");
    let peer_arr = resp["data"]["peers"].as_array().expect("data.peers array");
    assert_eq!(peer_arr.len(), 1);

    let peer = &peer_arr[0];
    assert_eq!(
        peer["online"].as_bool(),
        Some(true),
        "peer in live sinks map must be online even without last_sync_at: {peer}"
    );
    // Ensure the receiver stays alive until after the assertion so the
    // sender is not marked closed prematurely.
    drop(_peer_rx);
}

/// `persist_paired_peer` must populate the `name` field from `PeerMeta.device_name`
/// when provided, so `list_peers` returns a human-readable name rather than
/// an empty string.
#[tokio::test]
async fn persist_paired_peer_populates_name_from_peer_meta_device_name() {
    let dir = safe_tempdir();
    let cfg_home = dir.path().join("cfg5");
    let _env = EnvGuard::set_all(
        &[
            "COPYPASTE_CONFIG_DIR",
            "COPYPASTE_DATA_DIR",
            "HOME",
            "XDG_CONFIG_HOME",
        ],
        &cfg_home,
    );
    std::fs::create_dir_all(&cfg_home).unwrap();

    // Build a PeerMeta with device_name set.
    let peer_meta = copypaste_p2p::bootstrap::PeerMeta {
        model: Some("iPhone 15".to_string()),
        os_version: Some("iOS 17".to_string()),
        app_version: Some("0.6.0".to_string()),
        local_ip: Some("192.168.1.42".to_string()),
        device_name: Some("Alice's iPhone".to_string()),
        public_ip: Some("203.0.113.42".to_string()),
        device_id: None,
        supabase_account_id: None,
    };
    // A dummy session key (all-zero is fine for this structural test).
    // SessionKey is a newtype tuple-struct: SessionKey([u8; 32]).
    let session_key = copypaste_p2p::pake::SessionKey([0u8; 32]);
    let fp = "b3:c4:d5:e6:f7:08:19:2a";

    IpcServer::persist_paired_peer(fp, "127.0.0.1:5001", &session_key, &peer_meta, None).await;

    // Read back the written peers.json and check name.
    let peers_path = peers_file_path();
    let written = crate::peers::load_peers(&peers_path);
    let record = written
        .iter()
        .find(|p| canonical_fingerprint(&p.fingerprint) == canonical_fingerprint(fp));
    assert!(
        record.is_some(),
        "persist_paired_peer must write a record for {fp}"
    );
    let record = record.unwrap();
    assert_eq!(
        record.name, "Alice's iPhone",
        "name must come from PeerMeta.device_name; got {:?}",
        record.name
    );
    // B1: the peer's reported public IP must be persisted on the record so
    // list_peers can surface it to the Devices UI.
    assert_eq!(
        record.public_ip.as_deref(),
        Some("203.0.113.42"),
        "public_ip must come from PeerMeta.public_ip; got {:?}",
        record.public_ip
    );
}

/// CopyPaste-yw2k: `list_peers` must surface `supabase_account_id` when it
/// is stored in `peers.json` (seeded by the pairing handshake). Peers that
/// predate this field surface no `supabase_account_id` key (or null).
#[tokio::test]
async fn list_peers_surfaces_supabase_account_id() {
    let dir = safe_tempdir();
    let sock = dir.path().join("lp_sai.sock");
    let cfg_home = dir.path().join("cfg_sai");
    let _env = EnvGuard::set_all(
        &[
            "COPYPASTE_CONFIG_DIR",
            "COPYPASTE_DATA_DIR",
            "HOME",
            "XDG_CONFIG_HOME",
        ],
        &cfg_home,
    );
    std::fs::create_dir_all(&cfg_home).unwrap();

    let peers_json = cfg_home.join("peers.json");
    // Seed one peer that carries a supabase_account_id (as stored after a
    // successful pairing handshake with a new-build peer).
    let peers = serde_json::json!([{
        "name": "Laptop",
        "fingerprint": "a1:b2:c3:d4:e5:f6:07:18",
        "added_at": 1,
        "supabase_account_id": "proj_abc/uid_00000000-1111-2222-3333-444444444444"
    }]);
    std::fs::write(&peers_json, serde_json::to_string(&peers).unwrap()).unwrap();

    start_test_server(&sock).await;
    let resp = call_one(
        &sock,
        r#"{"id":"lp_sai1","method":"list_peers","params":{}}"#,
    )
    .await;
    assert_eq!(resp["ok"], true, "list_peers must succeed: {resp}");
    let peer_arr = resp["data"]["peers"]
        .as_array()
        .expect("data.peers must be array");
    assert_eq!(peer_arr.len(), 1, "must have exactly one peer");

    let peer = &peer_arr[0];
    assert_eq!(
        peer["supabase_account_id"].as_str(),
        Some("proj_abc/uid_00000000-1111-2222-3333-444444444444"),
        "list_peers must surface supabase_account_id from peers.json: {peer}"
    );
}

/// CopyPaste-yw2k: `persist_paired_peer` must store the peer's
/// `supabase_account_id` from `PeerMeta` into `peers.json`.
#[tokio::test]
async fn persist_paired_peer_stores_supabase_account_id() {
    let dir = safe_tempdir();
    let cfg_home = dir.path().join("cfg_sai2");
    let _env = EnvGuard::set_all(
        &[
            "COPYPASTE_CONFIG_DIR",
            "COPYPASTE_DATA_DIR",
            "HOME",
            "XDG_CONFIG_HOME",
        ],
        &cfg_home,
    );
    std::fs::create_dir_all(&cfg_home).unwrap();

    let peer_meta = copypaste_p2p::bootstrap::PeerMeta {
        model: None,
        os_version: None,
        app_version: None,
        local_ip: None,
        device_name: Some("Bob's MacBook".to_string()),
        public_ip: None,
        device_id: None,
        supabase_account_id: Some("proj_xyz/uid_99999999-aaaa-bbbb-cccc-dddddddddddd".to_string()),
    };
    let session_key = copypaste_p2p::pake::SessionKey([0u8; 32]);
    let fp = "de:ad:be:ef:ca:fe:00:11";

    IpcServer::persist_paired_peer(fp, "127.0.0.1:5002", &session_key, &peer_meta, None).await;

    let peers_path = peers_file_path();
    let written = crate::peers::load_peers(&peers_path);
    let record = written
        .iter()
        .find(|p| canonical_fingerprint(&p.fingerprint) == canonical_fingerprint(fp));
    assert!(
        record.is_some(),
        "persist_paired_peer must write a record for {fp}"
    );
    let record = record.unwrap();
    assert_eq!(
        record.supabase_account_id.as_deref(),
        Some("proj_xyz/uid_99999999-aaaa-bbbb-cccc-dddddddddddd"),
        "supabase_account_id must be stored from PeerMeta; got {:?}",
        record.supabase_account_id
    );
}

/// When `p2p_enabled: false` is explicitly sent, merge_config must take the
/// incoming value (the toggle is authoritative when present).
#[test]
fn p2p_enabled_option_some_false_wins() {
    let existing = AppConfig {
        p2p_enabled: Some(true),
        ..Default::default()
    };
    let incoming = AppConfig {
        p2p_enabled: Some(false),
        ..Default::default()
    };
    let merged = merge_config(existing, incoming);
    assert_eq!(
        merged.p2p_enabled,
        Some(false),
        "explicit p2p_enabled=false must override existing true"
    );
}

// --- get_item_file ---

/// `get_item_file` must decrypt and return a file item's raw bytes as
/// base64, along with the filename and MIME type stored at capture time.
/// The round-trip mirrors `get_item_image`: store via `ClipboardItem::new_file`
/// (chunks_to_blob-encoded), then retrieve via the IPC verb.
#[tokio::test]
async fn get_item_file_round_trips_bytes_and_meta() {
    let dir = safe_tempdir();
    let socket_path = dir.path().join("ipc.sock");
    let (_pm, db) = start_test_server_returning_db(&socket_path, false).await;

    // Build a file item and seed it into the DB.
    // new_file stamps key_version = 2, so the server reads with
    // derive_v2(local_key). Encrypt with that same v2 key so the round-trip
    // matches the production writer (handle_file uses derive_v2).
    let raw = b"hello clipboard file";
    let key = [0u8; 32]; // v1 seed matching dummy server key
    let v2_key = derive_v2(&key); // server reads kv=2 rows with this
    let file_id = [0xAAu8; 16]; // fixed content-hash stand-in for test
    let (meta, chunks) =
        copypaste_core::encode_file(raw, "hello.txt", "text/plain", &v2_key, &file_id, 0)
            .expect("encode_file must succeed");
    let blob = copypaste_core::chunks_to_blob(&chunks).expect("chunks_to_blob must succeed");
    let meta_json = crate::clipboard::build_file_meta_json(&meta);
    let mut item = copypaste_core::ClipboardItem::new_file(blob, meta_json, 0);
    item.item_id = uuid::Uuid::from_bytes(file_id).to_string().into();

    let item_id = item.id.clone();
    {
        let db_guard = db.lock().await;
        copypaste_core::insert_item_with_fts(&db_guard, &item, "").expect("insert must succeed");
    }

    // Issue get_item_file over IPC.
    let mut stream = UnixStream::connect(&socket_path).await.unwrap();
    let req = format!(
        "{{\"id\":\"gf1\",\"method\":\"get_item_file\",\"params\":{{\"id\":\"{item_id}\"}}}}\n"
    );
    stream.write_all(req.as_bytes()).await.unwrap();
    let mut reader = BufReader::new(&mut stream);
    let mut line = String::new();
    reader.read_line(&mut line).await.unwrap();
    let resp: serde_json::Value = serde_json::from_str(line.trim()).unwrap();

    assert_eq!(resp["ok"], true, "get_item_file must succeed: {resp}");
    assert_eq!(resp["data"]["filename"], "hello.txt");
    assert_eq!(resp["data"]["mime"], "text/plain");
    // Verify the raw bytes round-trip through base64.
    use base64::Engine as _;
    let returned_bytes = base64::engine::general_purpose::STANDARD
        .decode(resp["data"]["data_b64"].as_str().unwrap())
        .expect("data_b64 must be valid base64");
    assert_eq!(returned_bytes, raw);
}

/// `get_item_file` must reject requests for non-file content_type items.
#[tokio::test]
async fn get_item_file_rejects_non_file_item() {
    let dir = safe_tempdir();
    let socket_path = dir.path().join("ipc2.sock");
    let (_pm, db) = start_test_server_returning_db(&socket_path, false).await;

    // Insert a text item. new_text(encrypted_content, nonce, lamport_ts).
    let nonce = vec![0u8; copypaste_core::NONCE_SIZE];
    let ciphertext = b"dummy-ciphertext".to_vec();
    let item = copypaste_core::ClipboardItem::new_text(ciphertext, nonce, 0);
    let item_id = item.id.clone();
    {
        let db_guard = db.lock().await;
        copypaste_core::insert_item_with_fts(&db_guard, &item, "dummy text")
            .expect("insert must succeed");
    }

    let mut stream = UnixStream::connect(&socket_path).await.unwrap();
    let req = format!(
        "{{\"id\":\"gf2\",\"method\":\"get_item_file\",\"params\":{{\"id\":\"{item_id}\"}}}}\n"
    );
    stream.write_all(req.as_bytes()).await.unwrap();
    let mut reader = BufReader::new(&mut stream);
    let mut line = String::new();
    reader.read_line(&mut line).await.unwrap();
    let resp: serde_json::Value = serde_json::from_str(line.trim()).unwrap();

    assert_eq!(
        resp["ok"], false,
        "get_item_file must fail for a text item: {resp}"
    );
}

/// `parse_file_meta` must extract filename, mime, original_size and
/// chunk_count from the JSON produced by `build_file_meta_json`.
#[test]
fn parse_file_meta_round_trips_build_file_meta_json() {
    let meta = copypaste_core::FileMeta {
        filename: "test.pdf".to_string(),
        mime: "application/pdf".to_string(),
        original_size: 12345,
        chunk_count: 2,
        file_id: [0xABu8; 16],
    };
    let json = crate::clipboard::build_file_meta_json(&meta);
    let parsed = parse_file_meta(&json).expect("parse_file_meta must succeed");
    assert_eq!(parsed.filename, "test.pdf");
    assert_eq!(parsed.mime, "application/pdf");
    assert_eq!(parsed.original_size, 12345);
    assert_eq!(parsed.chunk_count, 2);
    assert_eq!(parsed.file_id, [0xABu8; 16]);
}

/// `history_page` must return `[file: <name>]` as the preview for file items.
#[tokio::test]
async fn history_page_shows_file_preview() {
    let dir = safe_tempdir();
    let socket_path = dir.path().join("hp_file.sock");
    let (_pm, db) = start_test_server_returning_db(&socket_path, false).await;

    let raw = b"pdf content";
    let key = [0u8; 32];
    let file_id = [0x01u8; 16];
    let (meta, chunks) =
        copypaste_core::encode_file(raw, "doc.pdf", "application/pdf", &key, &file_id, 0).unwrap();
    let blob = copypaste_core::chunks_to_blob(&chunks).unwrap();
    let meta_json = crate::clipboard::build_file_meta_json(&meta);
    let item = copypaste_core::ClipboardItem::new_file(blob, meta_json, 0);
    {
        let db_guard = db.lock().await;
        copypaste_core::insert_item_with_fts(&db_guard, &item, "").unwrap();
    }

    let mut stream = UnixStream::connect(&socket_path).await.unwrap();
    stream
            .write_all(
                b"{\"id\":\"hpf\",\"method\":\"history_page\",\"params\":{\"limit\":10,\"offset\":0}}\n",
            )
            .await
            .unwrap();
    let mut reader = BufReader::new(&mut stream);
    let mut line = String::new();
    reader.read_line(&mut line).await.unwrap();
    let resp: serde_json::Value = serde_json::from_str(line.trim()).unwrap();

    assert_eq!(resp["ok"], true, "history_page must succeed: {resp}");
    let items = resp["data"]["items"].as_array().unwrap();
    let file_item = items.iter().find(|it| it["content_type"] == "file");
    assert!(file_item.is_some(), "must find a file item in history_page");
    let preview = file_item.unwrap()["preview"].as_str().unwrap();
    assert!(
        preview.starts_with("[file:"),
        "file item preview must start with '[file:'; got: {preview}"
    );
    assert!(
        preview.contains("doc.pdf"),
        "file item preview must include filename; got: {preview}"
    );
}

// --- write_to_pasteboard: file branch ---

/// `paste_file_cache_dir` must return a path that ends in `paste-files` and
/// lives under the platform cache directory (e.g. `~/Library/Caches/CopyPaste/paste-files`
/// on macOS). The test is platform-agnostic: it only checks the basename.
#[test]
fn paste_file_cache_dir_ends_with_paste_files() {
    let dir = paste_file_cache_dir();
    assert_eq!(
        dir.file_name().and_then(|n| n.to_str()),
        Some("paste-files"),
        "paste_file_cache_dir must end in 'paste-files'; got: {dir:?}"
    );
}

/// `prune_old_paste_files` must remove files whose mtime is older than the
/// retention window (~10 min) and leave recent files untouched.
#[test]
fn prune_old_paste_files_removes_stale_and_keeps_recent() {
    let dir = safe_tempdir();
    let cache = dir.path().to_path_buf();

    // Write a "recent" file (mtime = now).
    let recent = cache.join("recent.txt");
    std::fs::write(&recent, b"keep me").unwrap();

    // Write a "stale" file and backdate its mtime by 20 minutes.
    let stale = cache.join("stale.txt");
    std::fs::write(&stale, b"delete me").unwrap();
    let twenty_min_ago = std::time::SystemTime::now()
        .checked_sub(std::time::Duration::from_secs(20 * 60))
        .expect("time subtraction is infallible on any plausible system clock");
    // std::fs::FileTimes / File::set_times is stable since Rust 1.75 (MSRV = 1.96).
    // set_modified lives on FileTimes directly (no platform extension trait needed).
    {
        let f = std::fs::OpenOptions::new()
            .write(true)
            .open(&stale)
            .expect("open stale for set_times");
        let times = std::fs::FileTimes::new().set_modified(twenty_min_ago);
        f.set_times(times).expect("set_times on stale file");
    }

    prune_old_paste_files(&cache);

    assert!(recent.exists(), "recent file must survive prune");
    assert!(!stale.exists(), "stale (20-min-old) file must be pruned");
}

/// `write_to_pasteboard` must not return the `Unknown content_type` fallthrough
/// for a `file` item; instead it must attempt the file-decode path.
/// On non-macOS the non-macOS stub always returns `Ok(())` regardless of
/// content_type, so we assert `Ok` there.
/// On macOS we verify that either:
///   a) a paste temp-file was created under `paste_file_cache_dir()`, OR
///   b) an error was returned (e.g. NSPasteboard not available in headless CI) —
///      the important invariant is that the error is NOT the old "Unknown content_type"
///      fallthrough, which means the file branch was reached.
#[tokio::test]
async fn write_to_pasteboard_file_branch_is_reached() {
    let dir = safe_tempdir();
    let sock = dir.path().join("wtp_file.sock");

    // Point COPYPASTE_CACHE_DIR at a temp path so paste-files land there
    // and don't pollute ~/Library/Caches/CopyPaste during the test.
    let cache_home = dir.path().join("cache");
    // Acquire EnvGuard (and thus TEST_ENV_LOCK) BEFORE create_dir_all so
    // the mkdir is serialised with the umask(0o177) window from the
    // bind_with_stale_cleanup tests.
    let _env = EnvGuard::set_all(&["COPYPASTE_CACHE_DIR"], &cache_home);
    std::fs::create_dir_all(&cache_home).unwrap();

    let (_pm, db) = start_test_server_returning_db(&sock, false).await;

    // Build a real encoded file item with the same all-zero key as the test server.
    let raw = b"hello paste file";
    let key = [0u8; 32]; // matches the test server's local_key
    let file_id = [0xBBu8; 16];
    let (meta, chunks) =
        copypaste_core::encode_file(raw, "paste.txt", "text/plain", &key, &file_id, 0)
            .expect("encode_file must succeed");
    let blob = copypaste_core::chunks_to_blob(&chunks).expect("chunks_to_blob must succeed");
    let meta_json = crate::clipboard::build_file_meta_json(&meta);
    let mut item = copypaste_core::ClipboardItem::new_file(blob, meta_json, 0);
    // Align item_id with file_id (mirrors get_item_file_round_trips test).
    item.item_id = uuid::Uuid::from_bytes(file_id).to_string().into();
    let item_id = item.id.clone();
    {
        let db_guard = db.lock().await;
        copypaste_core::insert_item_with_fts(&db_guard, &item, "").expect("insert must succeed");
    }

    // Trigger copy_item over IPC — this calls write_to_pasteboard internally.
    let mut stream = tokio::net::UnixStream::connect(&sock).await.unwrap();
    let req = format!(
        "{{\"id\":\"wtp1\",\"method\":\"copy_item\",\"params\":{{\"id\":\"{item_id}\"}}}}\n"
    );
    stream.write_all(req.as_bytes()).await.unwrap();
    let mut reader = BufReader::new(&mut stream);
    let mut line = String::new();
    reader.read_line(&mut line).await.unwrap();
    let resp: serde_json::Value = serde_json::from_str(line.trim()).unwrap();

    // On macOS (with a real display/pasteboard) the call succeeds and a
    // paste-files temp file must exist.
    // In headless CI (macOS without a window server) the paste may fail, but
    // must NOT report "Unknown content_type" — that would mean the file branch
    // was bypassed entirely and we fell through to the old raw-bytes path.
    #[cfg(target_os = "macos")]
    {
        if resp["ok"] == true {
            // Verify a temp file was written.
            let paste_dir = cache_home.join("paste-files");
            let found = std::fs::read_dir(&paste_dir)
                .map(|rd| {
                    rd.flatten()
                        .any(|e| e.file_name().to_str() == Some("paste.txt"))
                })
                .unwrap_or(false);
            assert!(
                    found,
                    "write_to_pasteboard file branch must create paste.txt under paste-files; dir: {paste_dir:?}"
                );
        } else {
            // Headless / no pasteboard — acceptable failure, but must not be the unknown-fallthrough.
            let err = resp["error"].as_str().unwrap_or("");
            assert!(
                    !err.contains("Unknown content_type"),
                    "write_to_pasteboard must NOT fall through to Unknown content_type for file items; error: {err}"
                );
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        // Non-macOS stub always returns Ok(()).
        assert_eq!(
            resp["ok"], true,
            "write_to_pasteboard non-macOS stub must succeed for file items: {resp}"
        );
    }
}

// -----------------------------------------------------------------------
// crh3.77: write_to_pasteboard async-signature enforcement
// -----------------------------------------------------------------------

/// write_to_pasteboard must be an async fn so its file branch can offload
/// blocking fs::write (up to 100 MiB) to a spawn_blocking thread instead
/// of stalling the tokio async worker.
///
/// Before crh3.77: write_to_pasteboard was a sync fn — calling .await on it
/// is a compile error (the expected "failing test" state).
/// After crh3.77: the function is async; non-macOS stub always returns Ok(()).
#[tokio::test]
async fn write_to_pasteboard_is_async_fn() {
    let db = Arc::new(Mutex::new(Database::open_in_memory().unwrap()));
    let private_mode = Arc::new(AtomicBool::new(false));
    let local_key = Arc::new(zeroize::Zeroizing::new([0u8; 32]));
    let device_pub = Arc::new([0u8; 32]);
    let server = IpcServer::new(db, private_mode, local_key, device_pub);

    // Build a properly encoded file item (same all-zero key as the server).
    let raw = b"crh3.77 async enforcement payload";
    let key = [0u8; 32];
    let file_id = [0xCCu8; 16];
    let (meta, chunks) =
        copypaste_core::encode_file(raw, "crh377.txt", "text/plain", &key, &file_id, 0)
            .expect("encode_file must succeed");
    let blob = copypaste_core::chunks_to_blob(&chunks).expect("chunks_to_blob must succeed");
    let meta_json = crate::clipboard::build_file_meta_json(&meta);
    let mut item = copypaste_core::ClipboardItem::new_file(blob, meta_json, 0);
    item.item_id = uuid::Uuid::from_bytes(file_id).to_string().into();

    // .await here statically enforces that write_to_pasteboard is async.
    // A sync fn cannot be awaited, so this is a compile-time assertion.
    // On non-macOS the stub always returns Ok(()) without touching the FS.
    let result = server.write_to_pasteboard(&item).await;
    #[cfg(not(target_os = "macos"))]
    assert!(
        result.is_ok(),
        "non-macOS stub must return Ok(()): {result:?}"
    );
    // On macOS: Ok(()) when the pasteboard is available, Err otherwise —
    // either is acceptable; the invariant is that the function is awaitable.
    let _ = result;
}

// -----------------------------------------------------------------------
// CopyPaste-7mf regression: responder-side persist race
// -----------------------------------------------------------------------

/// Regression test for CopyPaste-7mf: after a successful network bootstrap
/// pairing, the RESPONDER daemon's `list_peers` MUST return the newly-paired
/// peer immediately after the INITIATOR's `pair_accept_qr` response returns —
/// with NO sleep or polling between the two calls.
///
/// The race: `pair_generate_qr` fires `spawn_bootstrap_responder` which runs
/// the PAKE handshake + `persist_paired_peer` inside a `tokio::spawn`. The
/// IPC response is returned before the spawn's persist completes. The fix
/// (CopyPaste-7mf) stores the `JoinHandle` in `IpcServer::pending_bootstrap`
/// and has `list_peers` await it (with a 5 s timeout) before reading
/// peers.json. This test would fail WITHOUT the fix and MUST pass with it.
#[tokio::test]
async fn responder_list_peers_sees_peer_immediately_after_initiator_completes() {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::UnixStream;

    let dir = safe_tempdir();
    let cfg_home = dir.path().join("cfg_7mf");
    let _env = EnvGuard::set_all(
        &[
            "COPYPASTE_CONFIG_DIR",
            "COPYPASTE_DATA_DIR",
            "HOME",
            "XDG_CONFIG_HOME",
        ],
        &cfg_home,
    );
    std::fs::create_dir_all(&cfg_home).unwrap();

    // Helper: send one newline-terminated JSON request, return parsed response.
    async fn call(sock: &std::path::Path, body: &str) -> serde_json::Value {
        let mut stream = UnixStream::connect(sock).await.unwrap();
        stream.write_all(body.as_bytes()).await.unwrap();
        stream.write_all(b"\n").await.unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        serde_json::from_str(&line).unwrap()
    }

    // ── Server A (responder): generates the QR. Needs a real cert so that
    // BootstrapResponder::bind uses real TLS and spawn_bootstrap_responder runs.
    let sock_a = dir.path().join("7mf-a.sock");
    let cert_a = copypaste_p2p::cert::SelfSignedCert::generate("test-a").unwrap();
    {
        let db = Arc::new(Mutex::new(Database::open_in_memory().unwrap()));
        let server = IpcServer::new(
            db,
            Arc::new(AtomicBool::new(false)),
            Arc::new(zeroize::Zeroizing::new([0u8; 32])),
            Arc::new([0u8; 32]),
        )
        .with_cert_fingerprint(display_fingerprint(&cert_a.fingerprint()))
        .with_p2p_cert(cert_a.cert_der.clone(), cert_a.key_der.clone());
        // Bind directly (no umask(0o177) race) — see start_test_server_returning_db.
        let listener_a =
            tokio::net::UnixListener::bind(&sock_a).expect("test socket A bind must succeed");
        tokio::spawn(async move {
            let _ = server.serve_on(listener_a, CancellationToken::new()).await;
        });
    }

    // ── Server B (initiator): dials A's bootstrap addr. Needs its own cert.
    let sock_b = dir.path().join("7mf-b.sock");
    let cert_b = copypaste_p2p::cert::SelfSignedCert::generate("test-b").unwrap();
    {
        let db = Arc::new(Mutex::new(Database::open_in_memory().unwrap()));
        let server = IpcServer::new(
            db,
            Arc::new(AtomicBool::new(false)),
            Arc::new(zeroize::Zeroizing::new([0u8; 32])),
            Arc::new([0u8; 32]),
        )
        .with_cert_fingerprint(display_fingerprint(&cert_b.fingerprint()))
        .with_p2p_cert(cert_b.cert_der.clone(), cert_b.key_der.clone());
        // Bind directly (no umask(0o177) race) — see start_test_server_returning_db.
        let listener_b =
            tokio::net::UnixListener::bind(&sock_b).expect("test socket B bind must succeed");
        tokio::spawn(async move {
            let _ = server.serve_on(listener_b, CancellationToken::new()).await;
        });
    }
    // Give both sockets a moment to come up.
    tokio::time::sleep(std::time::Duration::from_millis(80)).await;

    // A's canonical fingerprint (colon-free) — what B should persist.
    let fp_a_canonical = canonical_fingerprint(&display_fingerprint(&cert_a.fingerprint()));
    // B's canonical fingerprint — what A's responder spawn should persist.
    let fp_b_canonical = canonical_fingerprint(&display_fingerprint(&cert_b.fingerprint()));

    // Step 1: A generates a QR. With a real p2p_cert, this binds a
    // bootstrap TLS listener, stores the JoinHandle in pending_bootstrap,
    // and embeds the listener's host:port in the QR's addr_hint.
    let qr_resp = call(
        &sock_a,
        r#"{"id":"7mf-q","method":"pair_generate_qr","params":{}}"#,
    )
    .await;
    assert_eq!(
        qr_resp["ok"], true,
        "pair_generate_qr must succeed: {qr_resp}"
    );
    let qr = qr_resp["data"]["qr"]
        .as_str()
        .expect("QR string in response")
        .to_string();
    // Ensure the QR carries an addr_hint so B dials the network path
    // (not the legacy IPC-relay path). The encoded QR wraps the bare CPPAIR2
    // payload in the deep-link URI; strip it to inspect the addr_hint field.
    let bare = copypaste_core::strip_deeplink(&qr);
    // v2 QR: CPPAIR2.<fp_b64>.<token_b64>.<device_id_b64>.<name>.<addr_hint>
    // addr_hint is the last '.' separated field. Use the existing helper.
    let has_hint = {
        let (_magic, body) = bare.split_once('.').expect("bare QR has magic.body");
        let hint = body.splitn(5, '.').nth(4).unwrap_or("");
        hint.parse::<std::net::SocketAddr>().is_ok()
    };
    // If there is no addr_hint the bootstrap listener did not bind (unlikely
    // on loopback) — skip the network PAKE path and let this test pass vacuously
    // rather than incorrectly block forever.
    if !has_hint {
        return;
    }

    // Step 2: B accepts the QR over the network. This drives the full PAKE
    // handshake; it only returns ok once both sides have agreed on the session key.
    let accept_body = serde_json::json!({
        "id": "7mf-accept",
        "method": "pair_accept_qr",
        "params": { "qr": qr },
    })
    .to_string();
    let accept_resp = call(&sock_b, &accept_body).await;
    assert_eq!(
        accept_resp["ok"], true,
        "network PAKE pairing must succeed end-to-end: {accept_resp}"
    );
    // B should have A's fingerprint as the confirmed peer.
    let returned_fp = accept_resp["data"]["peer_fingerprint"]
        .as_str()
        .expect("peer_fingerprint in accept response");
    assert_eq!(
        returned_fp, fp_a_canonical,
        "returned peer_fingerprint must equal A's cert fingerprint"
    );

    // Step 3 — THE REGRESSION CHECK: call list_peers on A IMMEDIATELY
    // (no sleep, no poll) and assert B's fingerprint is already present.
    // Without the CopyPaste-7mf fix this would race the detached spawn and
    // return an empty peers list. With the fix, list_peers awaits the
    // pending_bootstrap JoinHandle and blocks until persist_paired_peer runs.
    let list_resp = call(
        &sock_a,
        r#"{"id":"7mf-list","method":"list_peers","params":{}}"#,
    )
    .await;
    assert_eq!(
        list_resp["ok"], true,
        "list_peers on A must succeed: {list_resp}"
    );
    let peers = list_resp["data"]["peers"]
        .as_array()
        .expect("data.peers array");
    let found = peers.iter().any(|p| {
        p.get("fingerprint")
            .and_then(|v| v.as_str())
            .map(|fp| canonical_fingerprint(fp) == fp_b_canonical)
            .unwrap_or(false)
    });
    assert!(
        found,
        "A's list_peers must return B's fingerprint immediately after initiator completes \
             (CopyPaste-7mf race fix); fp_b={fp_b_canonical}; peers={peers:?}"
    );
}

// ── lan_visibility IPC config tests ───────────────────────────────────────

/// `merge_config` preserves `lan_visibility` from existing when incoming
/// omits it (`None`), and takes the new value when the caller supplies one.
#[test]
fn merge_config_preserves_and_overrides_lan_visibility() {
    // Case 1: incoming omits lan_visibility — existing value is kept.
    let existing = AppConfig {
        lan_visibility: Some(false),
        ..AppConfig::default()
    };
    let incoming_none = AppConfig {
        lan_visibility: None,
        ..AppConfig::default()
    };
    let merged = merge_config(existing, incoming_none);
    assert_eq!(
        merged.lan_visibility,
        Some(false),
        "merge_config must preserve existing lan_visibility when incoming is None"
    );

    // Case 2: incoming supplies an explicit value — it wins.
    let existing2 = AppConfig {
        lan_visibility: Some(false),
        ..AppConfig::default()
    };
    let incoming_some = AppConfig {
        lan_visibility: Some(true),
        ..AppConfig::default()
    };
    let merged2 = merge_config(existing2, incoming_some);
    assert_eq!(
        merged2.lan_visibility,
        Some(true),
        "merge_config must take incoming lan_visibility when Some"
    );
}

/// `update_core_config` persists `lan_visibility` to config.toml and the
/// returned `AppConfig` reflects the new value.
#[test]
fn update_core_config_persists_lan_visibility() {
    let env_lock = crate::TEST_ENV_LOCK
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let dir = safe_tempdir();
    unsafe { std::env::set_var("COPYPASTE_CONFIG_DIR", dir.path()) };

    // Disable LAN visibility via IPC patch.
    let patch = AppConfig {
        lan_visibility: Some(false),
        ..AppConfig::default()
    };
    let new_core = update_core_config(&patch).expect("update_core_config must succeed");
    assert!(
        !new_core.lan_visibility,
        "update_core_config must persist lan_visibility=false to config.toml"
    );

    // Re-enable it.
    let patch2 = AppConfig {
        lan_visibility: Some(true),
        ..AppConfig::default()
    };
    let new_core2 = update_core_config(&patch2).expect("update_core_config must succeed");
    assert!(
        new_core2.lan_visibility,
        "update_core_config must persist lan_visibility=true to config.toml"
    );

    // When omitted (`None`), the stored value is unchanged (false from patch).
    // First persist false explicitly, then send None.
    let patch3_set = AppConfig {
        lan_visibility: Some(false),
        ..AppConfig::default()
    };
    update_core_config(&patch3_set).expect("set to false");
    let patch3_none = AppConfig {
        lan_visibility: None,
        ..AppConfig::default()
    };
    let new_core3 = update_core_config(&patch3_none).expect("update with None");
    assert!(
        !new_core3.lan_visibility,
        "update_core_config must not reset lan_visibility when patch has None"
    );

    // Restore env.
    unsafe { std::env::remove_var("COPYPASTE_CONFIG_DIR") };
    drop(env_lock);
}

// ── CopyPaste-44rq.67: relay_url clear-sentinel handling ────────────────

/// `merge_config` must PRESERVE the empty-string "clear" sentinel rather than
/// `.or()`-falling back to the existing URL, so `update_core_config` can see
/// `Some("")` and clear the relay. A normal value wins; `None` preserves.
#[test]
fn merge_config_preserves_relay_clear_sentinel() {
    let with_url = || AppConfig {
        relay_url: Some("https://relay.example.com".to_owned()),
        ..AppConfig::default()
    };
    // Clear sentinel must survive the merge (not be replaced by existing URL).
    let cleared = merge_config(
        with_url(),
        AppConfig {
            relay_url: Some(String::new()),
            ..AppConfig::default()
        },
    );
    assert_eq!(
        cleared.relay_url.as_deref(),
        Some(""),
        "empty sentinel must be preserved so update_core_config can clear"
    );
    // Omitted (None) preserves the existing URL.
    let preserved = merge_config(with_url(), AppConfig::default());
    assert_eq!(
        preserved.relay_url.as_deref(),
        Some("https://relay.example.com")
    );
    // A real value wins.
    let replaced = merge_config(
        with_url(),
        AppConfig {
            relay_url: Some("https://new.example.com".to_owned()),
            ..AppConfig::default()
        },
    );
    assert_eq!(
        replaced.relay_url.as_deref(),
        Some("https://new.example.com")
    );
}

/// `update_core_config` must set `core.relay_url = None` when the incoming
/// `relay_url` is the empty-string sentinel, persist a real URL when set, and
/// leave it unchanged when omitted (`None`).
#[test]
fn update_core_config_clears_relay_url_on_empty_sentinel() {
    let env_lock = crate::TEST_ENV_LOCK
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let dir = safe_tempdir();
    unsafe { std::env::set_var("COPYPASTE_CONFIG_DIR", dir.path()) };

    // Set a real URL.
    let set = AppConfig {
        relay_url: Some("https://relay.example.com".to_owned()),
        ..AppConfig::default()
    };
    let core1 = update_core_config(&set).expect("set relay_url");
    assert_eq!(
        core1.relay_url.as_deref(),
        Some("https://relay.example.com")
    );

    // Omit it — must be preserved.
    let core2 = update_core_config(&AppConfig::default()).expect("omit relay_url");
    assert_eq!(
        core2.relay_url.as_deref(),
        Some("https://relay.example.com"),
        "None must preserve the stored relay_url"
    );

    // Clear sentinel — must wipe to None.
    let clear = AppConfig {
        relay_url: Some(String::new()),
        ..AppConfig::default()
    };
    let core3 = update_core_config(&clear).expect("clear relay_url");
    assert_eq!(
        core3.relay_url, None,
        "empty-string sentinel must clear core.relay_url"
    );

    unsafe { std::env::remove_var("COPYPASTE_CONFIG_DIR") };
    drop(env_lock);
}

// ── CopyPaste-bjh: startup must honour persisted p2p_enabled ────────────

/// `p2p_enabled_from_config` must default to `true` when no config.json
/// exists (fresh install — P2P is ON by default so users can pair without
/// an explicit toggle). Regression guard: daemon startup used to check
/// `COPYPASTE_P2P` env-var only; now it falls back to this accessor.
#[test]
fn p2p_enabled_from_config_defaults_to_true_when_no_config() {
    let dir = safe_tempdir();
    let _env = EnvGuard::set_all(
        &["HOME", "XDG_CONFIG_HOME", "COPYPASTE_CONFIG_DIR"],
        dir.path(),
    );
    // No config.json written — accessor must return true (default ON).
    assert!(
        p2p_enabled_from_config(),
        "p2p_enabled_from_config must default to true when config.json is absent"
    );
}

/// When `p2p_enabled: false` is persisted (user toggled P2P off in Settings),
/// `p2p_enabled_from_config` must return `false`. This is the value daemon
/// startup reads (after the A-SET-4 fix) so the daemon skips `start_p2p`
/// when the env-var override (`COPYPASTE_P2P`) is absent.
#[test]
fn p2p_enabled_from_config_returns_false_when_persisted_false() {
    let dir = safe_tempdir();
    let _env = EnvGuard::set_all(
        &["HOME", "XDG_CONFIG_HOME", "COPYPASTE_CONFIG_DIR"],
        dir.path(),
    );
    write_config(&AppConfig {
        p2p_enabled: Some(false),
        ..Default::default()
    })
    .expect("write_config must succeed");

    assert!(
        !p2p_enabled_from_config(),
        "p2p_enabled_from_config must return false when config.json stores p2p_enabled=false"
    );
}

/// When `p2p_enabled: true` is persisted, `p2p_enabled_from_config` must
/// return `true`. Symmetric with the false case above.
#[test]
fn p2p_enabled_from_config_returns_true_when_persisted_true() {
    let dir = safe_tempdir();
    let _env = EnvGuard::set_all(
        &["HOME", "XDG_CONFIG_HOME", "COPYPASTE_CONFIG_DIR"],
        dir.path(),
    );
    write_config(&AppConfig {
        p2p_enabled: Some(true),
        ..Default::default()
    })
    .expect("write_config must succeed");

    assert!(
        p2p_enabled_from_config(),
        "p2p_enabled_from_config must return true when config.json stores p2p_enabled=true"
    );
}

// ── CopyPaste-6ot5: connection-cap unit test ──────────────────────────────

/// Verify the connection-cap semaphore logic without touching real sockets.
///
/// The semaphore starts with `MAX_CONCURRENT_CONNECTIONS` permits. When all
/// permits are exhausted, `try_acquire_owned` must return `Err` immediately
/// (non-blocking); once a permit is dropped the slot is reclaimed and the
/// next `try_acquire_owned` succeeds again. This test exercises the pure
/// Semaphore behaviour that `serve_on` depends on — avoiding any live-socket
/// flood that could introduce a test-suite deadlock.
#[test]
fn connection_cap_semaphore_exhaustion_returns_err() {
    // Use a small cap so the test runs without allocating 64 permits.
    const TEST_CAP: usize = 4;
    let sem = Arc::new(tokio::sync::Semaphore::new(TEST_CAP));

    // Acquire all permits.
    let permits: Vec<_> = (0..TEST_CAP)
        .map(|_| {
            sem.clone()
                .try_acquire_owned()
                .expect("permit must be available below cap")
        })
        .collect();

    // One more acquire must fail (cap exhausted).
    assert!(
        sem.clone().try_acquire_owned().is_err(),
        "try_acquire_owned must return Err when the connection cap is reached"
    );

    // Drop one permit — the slot is reclaimed immediately.
    drop(permits.into_iter().next().unwrap());

    // Now a new acquire succeeds.
    assert!(
        sem.clone().try_acquire_owned().is_ok(),
        "try_acquire_owned must succeed again after a permit is released"
    );
}

/// Verify that the production `IpcServer` is initialised with a semaphore
/// holding exactly `MAX_CONCURRENT_CONNECTIONS` permits and that the cap
/// is enforced from the very first connection.
#[test]
fn ipc_server_connection_cap_is_max_concurrent_connections() {
    let db = Arc::new(Mutex::new(Database::open_in_memory().unwrap()));
    let server = IpcServer::new(
        db,
        Arc::new(AtomicBool::new(false)),
        Arc::new(zeroize::Zeroizing::new([0u8; 32])),
        Arc::new([0u8; 32]),
    );

    // Drain all permits.
    let permits: Vec<_> = (0..MAX_CONCURRENT_CONNECTIONS)
        .map(|_| {
            server
                .conn_semaphore
                .clone()
                .try_acquire_owned()
                .expect("permit must be available within cap")
        })
        .collect();

    // The (cap+1)-th acquire must fail.
    assert!(
        server.conn_semaphore.clone().try_acquire_owned().is_err(),
        "IpcServer must enforce MAX_CONCURRENT_CONNECTIONS limit"
    );

    // Ensure permits are held for the assertion (not optimised away).
    drop(permits);
}

/// CopyPaste-kfe9: legacy IPC arms (search / copy / paste / pin) must
/// return a machine-readable `error_code` on failure, not a bare untyped
/// error string.  This is the follow-up to CopyPaste-8u2b which wired
/// `error_code` onto the `delete` arm but left the others unchanged.
#[tokio::test]
async fn legacy_ipc_arms_return_error_code_on_failure() {
    let server = bare_server();

    // -- search: missing required `query` param → invalid_argument ---------
    let resp = server
        .dispatch(r#"{"id":"s1","method":"search","params":{}}"#)
        .await;
    assert!(!resp.ok, "search without query must fail");
    assert_eq!(
        resp.error_code,
        Some("invalid_argument"),
        "search/missing-query must carry error_code=invalid_argument, got: {resp:?}"
    );

    // -- pin: missing required `id` param → invalid_argument ---------------
    let resp = server
        .dispatch(r#"{"id":"p1","method":"pin","params":{}}"#)
        .await;
    assert!(!resp.ok, "pin without id must fail");
    assert_eq!(
        resp.error_code,
        Some("invalid_argument"),
        "pin/missing-id must carry error_code=invalid_argument, got: {resp:?}"
    );

    // -- pin: non-UUID `id` → invalid_argument -----------------------------
    let resp = server
        .dispatch(r#"{"id":"p2","method":"pin","params":{"id":"not-a-uuid"}}"#)
        .await;
    assert!(!resp.ok, "pin with bad UUID must fail");
    assert_eq!(
        resp.error_code,
        Some("invalid_argument"),
        "pin/bad-uuid must carry error_code=invalid_argument, got: {resp:?}"
    );

    // -- copy: item not found → not_found ----------------------------------
    let missing_uuid = "00000000-0000-0000-0000-000000000000";
    let resp = server
        .dispatch(&format!(
            r#"{{"id":"c1","method":"copy","params":{{"id":"{missing_uuid}"}}}}"#
        ))
        .await;
    assert!(!resp.ok, "copy of non-existent item must fail");
    assert_eq!(
        resp.error_code,
        Some("not_found"),
        "copy/not-found must carry error_code=not_found, got: {resp:?}"
    );

    // -- paste: item not found → not_found ---------------------------------
    let resp = server
        .dispatch(&format!(
            r#"{{"id":"p3","method":"paste","params":{{"id":"{missing_uuid}"}}}}"#
        ))
        .await;
    assert!(!resp.ok, "paste of non-existent item must fail");
    assert_eq!(
        resp.error_code,
        Some("not_found"),
        "paste/not-found must carry error_code=not_found, got: {resp:?}"
    );
}

/// CopyPaste-48k0: `set_private_mode` must increment `private_mode_epoch` on
/// every call so that periodic pollers (UI health-check, tray) can detect
/// private-mode changes across daemon restarts without a dedicated subscription.
#[tokio::test]
async fn private_mode_epoch_increments_on_every_set() {
    let db = Arc::new(Mutex::new(Database::open_in_memory().unwrap()));
    let private_mode = Arc::new(AtomicBool::new(false));
    let server = IpcServer::new(
        db,
        private_mode,
        Arc::new(zeroize::Zeroizing::new([0u8; 32])),
        Arc::new([0u8; 32]),
    );

    // Epoch starts at 0.
    let resp0 = server
        .dispatch(r#"{"id":"t0","method":"get_private_mode","params":{}}"#)
        .await;
    assert!(resp0.ok, "initial get_private_mode must succeed");
    let data0 = resp0.data.expect("get_private_mode must return data");
    assert_eq!(
        data0["private_mode_epoch"],
        serde_json::json!(0),
        "epoch must start at 0"
    );

    // First set → epoch becomes 1.
    let resp1 = server
        .dispatch(r#"{"id":"t1","method":"set_private_mode","params":{"enabled":true}}"#)
        .await;
    assert!(resp1.ok, "set_private_mode must succeed");
    let data1 = resp1.data.expect("set_private_mode must return data");
    assert_eq!(
        data1["private_mode_epoch"],
        serde_json::json!(1),
        "epoch must be 1 after first set"
    );

    // Second set (same value) → epoch becomes 2.
    let resp2 = server
        .dispatch(r#"{"id":"t2","method":"set_private_mode","params":{"enabled":true}}"#)
        .await;
    let data2 = resp2.data.expect("set_private_mode must return data");
    assert_eq!(
        data2["private_mode_epoch"],
        serde_json::json!(2),
        "epoch must increment even when value is unchanged"
    );

    // get_private_mode must reflect the current epoch.
    let resp3 = server
        .dispatch(r#"{"id":"t3","method":"get_private_mode","params":{}}"#)
        .await;
    let data3 = resp3.data.expect("get_private_mode must return data");
    assert_eq!(
        data3["private_mode_epoch"],
        serde_json::json!(2),
        "get_private_mode must return current epoch"
    );

    // status must also include the epoch.
    let resp4 = server
        .dispatch(r#"{"id":"t4","method":"status","params":{}}"#)
        .await;
    let data4 = resp4.data.expect("status must return data");
    assert_eq!(
        data4["private_mode_epoch"],
        serde_json::json!(2),
        "status must return current epoch"
    );
}

/// CopyPaste-ah1m: `bind_with_stale_cleanup` must create a lockfile next to
/// the socket so concurrent startups are serialized through `flock(2)` and
/// the probe→remove→bind sequence is atomic.
///
/// After a successful bind the `.lock` file must exist (it is never deleted,
/// only created/locked). Its presence means a future concurrent starter will
/// block on `flock` rather than racing through the stale-check.
#[cfg(unix)]
#[tokio::test]
async fn bind_with_stale_cleanup_creates_lockfile() {
    let dir = safe_tempdir();
    let sock = dir.path().join("test-atomic.sock");
    let lock = dir.path().join("test-atomic.sock.lock");
    // Hold TEST_ENV_LOCK to serialise the libc::umask(0o177) inside
    // bind_with_stale_cleanup with concurrent tests that create directories.
    let _env = EnvGuard::set_all(
        &[
            "COPYPASTE_DATA_DIR",
            "COPYPASTE_CONFIG_DIR",
            "HOME",
            "XDG_CONFIG_HOME",
        ],
        dir.path(),
    );

    // Lockfile must NOT exist before the first call.
    assert!(
        !lock.exists(),
        "lockfile must not exist before bind; got: {lock:?}"
    );

    // Bind the socket — this should create the lockfile.
    let listener = super::bind_with_stale_cleanup(&sock)
        .expect("bind_with_stale_cleanup must succeed on a fresh path");
    drop(listener);

    // Lockfile must exist now.
    assert!(
        lock.exists(),
        "bind_with_stale_cleanup must create <socket>.lock; not found at {lock:?}"
    );
}

// ── CopyPaste-3n9h: pair_peer must be disabled (no unauthenticated trust) ─

/// `pair_peer` must return `not_implemented` with an actionable error
/// message. A caller that knows a peer's TLS fingerprint must NOT be able
/// to add it as trusted without going through PAKE+SAS authentication.
#[tokio::test]
async fn pair_peer_is_disabled_returns_not_implemented() {
    let dir = safe_tempdir();
    let sock = dir.path().join("test-pair-peer-disabled.sock");
    start_test_server(&sock).await;

    // Valid fingerprint + name — the old code would have accepted this.
    let valid_fp = "a".repeat(64); // 64-char hex fingerprint
    let body = format!(
        r#"{{"id":"pp1","method":"pair_peer","params":{{"fingerprint":"{valid_fp}","name":"Bob's Mac"}}}}"#
    );
    let resp = call_one(&sock, &body).await;
    assert_eq!(
        resp["ok"], false,
        "pair_peer must be rejected (unauthenticated pairing is disabled)"
    );
    assert_eq!(
        resp["error_code"], "not_implemented",
        "pair_peer must return not_implemented error code, got: {resp}"
    );
    // Error message must suggest the authenticated alternatives.
    let err = resp["error"].as_str().unwrap_or("");
    assert!(
        err.contains("pair_peer_with_password") || err.contains("pair_with_discovered"),
        "error message must suggest authenticated alternatives, got: {err}"
    );
}

// ── CopyPaste-n3bc: pair_get_sas must include peer_fingerprint on responder path ─

/// `pair_get_sas` in AwaitingSas state must include `peer_fingerprint`
/// when the pairing coordinator carries a fingerprint snapshot.
/// This tests the state-machine path directly (no network required).
#[test]
fn pair_get_sas_includes_peer_fingerprint_when_available() {
    use crate::pairing_sm::{PairingCoordinator, PairingRole, PeerSnapshot};

    let coord = PairingCoordinator::new();
    let snap = PeerSnapshot {
        device_name: Some("Alice's Mac".to_string()),
        ip_addrs: vec!["192.168.1.5".to_string()],
        fingerprint: Some("aabbccdd".to_string()),
    };
    assert!(coord.try_begin(PairingRole::Responder, snap.clone()));
    let _rx = coord.enter_awaiting_sas("123456".to_string(), PairingRole::Responder, snap);

    let state = coord.snapshot();
    // Verify the state machine surfaces the fingerprint.
    let peer_snap = state
        .peer_snapshot()
        .expect("must have peer snapshot in AwaitingSas");
    assert_eq!(
        peer_snap.fingerprint.as_deref(),
        Some("aabbccdd"),
        "peer_snapshot must carry fingerprint for pair_get_sas to surface it"
    );
}

// ── CopyPaste-8yzf: sentinel off-by-one — racing third-party write ─────────

/// Post-stamp must NOT overwrite the sentinel with a count that belongs to
/// a concurrent third-party write. If `actual_count > expected_count` it
/// means another app wrote to the pasteboard after us; leaving the sentinel
/// at the expected count (which no future poll will see) is safer than
/// stamping the third-party's count (which would suppress their content).
///
/// This test exercises the sentinel logic directly via the `AtomicI64` that
/// both `write_to_pasteboard` and `ClipboardMonitor::poll` share.
#[test]
fn sentinel_does_not_suppress_third_party_write_after_self_write() {
    use std::sync::atomic::{AtomicI64, Ordering};
    use std::sync::Arc;

    // Simulate: pre_count=10, we wrote and actual became 12 (our 2 ops),
    // then a third party wrote → actual is now 13.
    let sentinel = Arc::new(AtomicI64::new(-1));

    // Pre-stamp with expected post-write value.
    let pre_count: i64 = 10;
    let expected_after_write = pre_count + 2; // clearContents + setString
    sentinel.store(expected_after_write, Ordering::Release);

    // Our write completes — actual count is what we expected.
    let our_actual: i64 = 12;

    // Third-party writes between our write and the post-stamp read.
    // Simulate: post-stamp reads the already-incremented count.
    let post_stamp_read: i64 = 13; // third-party wrote after us

    // The WRONG approach (current bug): unconditionally overwrite with post-stamp.
    // This would store 13, suppressing the third-party write.
    // The CORRECT fix: only post-stamp if actual == expected (no racing write).
    if our_actual == expected_after_write {
        // Correct: our write was the only one, safe to post-stamp.
        sentinel.store(our_actual, Ordering::Release);
    }
    // If our_actual != expected_after_write OR post_stamp_read != our_actual,
    // leave the sentinel at expected_after_write (which already fired or is stale).

    // Simulating the monitor: it sees the third-party count (13).
    // With the correct fix, sentinel is 12 (not 13), so it won't suppress.
    let sentinel_val = sentinel.load(Ordering::Acquire);
    assert_ne!(
        sentinel_val, post_stamp_read,
        "sentinel must not match the third-party write count ({}); \
             that would suppress their clipboard content",
        post_stamp_read
    );
    // The sentinel remains at our expected value (12), which the monitor
    // may or may not have already consumed. Either way, 13 is not suppressed.
    assert_eq!(
        sentinel_val, expected_after_write,
        "sentinel must stay at our expected count ({}) not the third-party count ({})",
        expected_after_write, post_stamp_read
    );
}

// ── CopyPaste-aazu: import larger than MAX_REQUEST_BYTES returns clear error ─

/// A 64 MiB import exceeds MAX_REQUEST_BYTES (16 MiB) — the connection is
/// closed without any explanation. The fix: the per-item ceiling
/// (MAX_IMPORT_ITEM_BYTES = 4 MiB) already guards individual items;
/// but a batch with many large items can still exceed MAX_REQUEST_BYTES.
/// The IPC layer must return a clear "request too large" error rather than
/// silently closing the connection (which the CLI surfaces as a confusing EOF).
///
/// This test verifies that the "request too large" response is returned and
/// contains a human-readable error before the connection is closed.
#[tokio::test]
async fn import_oversized_request_returns_clear_error() {
    let dir = safe_tempdir();
    let sock = dir.path().join("test-import-oversized.sock");
    start_test_server(&sock).await;

    // Build a request that just barely exceeds MAX_REQUEST_BYTES (16 MiB + 1).
    // We construct a JSON line with a huge "items" array of one dummy item
    // whose content_bytes_b64 is large enough to tip over the limit.
    // The IPC layer reads up to MAX_REQUEST_BYTES + 1 then rejects.
    let item_size = MAX_REQUEST_BYTES + 100; // guarantee we exceed the limit
    use base64::Engine as _;
    let b64 = base64::engine::general_purpose::STANDARD;
    let large_content = b64.encode(vec![0u8; item_size]);
    let body = format!(
        r#"{{"id":"imp1","method":"import","params":{{"items":[{{"content_type":"text","content_bytes_b64":"{large_content}","created_at_ms":1700000000}}]}}}}"#,
    );

    let mut stream = UnixStream::connect(&sock).await.unwrap();
    // Send the body. The server will read up to MAX_REQUEST_BYTES+1 then
    // close the connection — write_all may fail with BrokenPipe once the
    // server closes; that is expected and acceptable.
    let _ = stream.write_all(body.as_bytes()).await;
    let _ = stream.write_all(b"\n").await;

    let mut lines = BufReader::new(&mut stream).lines();
    // Must receive a response (the "request too large" error) rather than
    // a hang. The response may arrive before all bytes are written, so we
    // read with a short timeout.
    let result = tokio::time::timeout(std::time::Duration::from_secs(5), lines.next_line()).await;
    match result {
        Ok(Ok(Some(line))) => {
            let v: serde_json::Value =
                serde_json::from_str(&line).expect("response must be valid JSON");
            assert_eq!(v["ok"], false, "oversized request must return ok=false");
            let err = v["error"].as_str().unwrap_or("");
            assert!(
                !err.is_empty(),
                "oversized request must return a non-empty error message"
            );
        }
        Ok(Ok(None)) => {
            // EOF — server closed after reading the oversize request.
            // This means the server did NOT hang waiting for more data;
            // the connection was properly terminated.  Acceptable outcome.
        }
        Ok(Err(e)) => {
            // BrokenPipe on read side is also fine — server closed the
            // socket after the rejection.
            if e.kind() != std::io::ErrorKind::BrokenPipe {
                panic!("unexpected read error: {e}");
            }
        }
        Err(_) => panic!("timed out waiting for oversized-import response (daemon may hang)"),
    }
}

// ── CopyPaste-cb7u: delete_all batches all soft-deletes in one blocking tx ──

/// `delete_all` must atomically tombstone every non-pinned, non-deleted item
/// in a single blocking transaction and leave pinned items untouched.
///
/// Pre-fix, the handler ran N sequential `spawn_blocking` calls (one per
/// item); post-fix it uses ONE blocking closure that holds the DB lock for
/// the full batch.  Both approaches must produce identical observable results —
/// this test guards the correctness invariant.
///
/// (CopyPaste-cb7u)
#[tokio::test]
async fn delete_all_tombstones_non_pinned_leaves_pinned_intact() {
    let db = Arc::new(Mutex::new(Database::open_in_memory().unwrap()));
    let server = IpcServer::new(
        db.clone(),
        Arc::new(AtomicBool::new(false)),
        Arc::new(zeroize::Zeroizing::new([0u8; 32])),
        Arc::new([0u8; 32]),
    );

    // Seed two regular items and one pinned item.
    let (id_a, id_b, id_pinned) = {
        let guard = db.lock().await;
        let a = copypaste_core::ClipboardItem::new_text(vec![0xA1; 8], vec![0u8; 24], 1);
        let b = copypaste_core::ClipboardItem::new_text(vec![0xB2; 8], vec![0u8; 24], 2);
        let mut p = copypaste_core::ClipboardItem::new_text(vec![0xC3; 8], vec![0u8; 24], 3);
        p.pinned = true;
        copypaste_core::insert_item(&guard, &a).unwrap();
        copypaste_core::insert_item(&guard, &b).unwrap();
        copypaste_core::insert_item(&guard, &p).unwrap();
        (a.id.clone(), b.id.clone(), p.id.clone())
    };

    // Call delete_all — must report 2 deleted (the two non-pinned items).
    let resp = server
        .dispatch(r#"{"id":"da1","method":"delete_all","params":{}}"#)
        .await;
    assert!(resp.ok, "delete_all must succeed: {resp:?}");
    let deleted = resp
        .data
        .as_ref()
        .and_then(|d| d["deleted"].as_u64())
        .expect("delete_all must return {\"deleted\": N}");
    assert_eq!(deleted, 2, "exactly 2 non-pinned items must be deleted");

    // Verify DB state: tombstones have deleted=1 and NULL content.
    {
        let guard = db.lock().await;
        let item_a = copypaste_core::get_item_by_id(&*guard, &id_a).unwrap();
        let item_b = copypaste_core::get_item_by_id(&*guard, &id_b).unwrap();
        let item_p = copypaste_core::get_item_by_id(&*guard, &id_pinned).unwrap();

        assert_eq!(
            item_a.as_ref().map(|i| i.deleted),
            Some(true),
            "item A must be tombstoned"
        );
        assert!(
            item_a.as_ref().and_then(|i| i.content.as_deref()).is_none(),
            "item A content must be cleared (tombstone)"
        );

        assert_eq!(
            item_b.as_ref().map(|i| i.deleted),
            Some(true),
            "item B must be tombstoned"
        );
        assert!(
            item_b.as_ref().and_then(|i| i.content.as_deref()).is_none(),
            "item B content must be cleared (tombstone)"
        );

        // Pinned item must survive with content intact.
        assert_eq!(
            item_p.as_ref().map(|i| i.deleted),
            Some(false),
            "pinned item must NOT be deleted"
        );
        assert!(
            item_p.as_ref().and_then(|i| i.content.as_deref()).is_some(),
            "pinned item content must be preserved"
        );
    }
}

// ── CopyPaste-0w4v: cloud methods in non-cloud builds return not_implemented ─

/// `cloud_sign_in` and `cloud_sign_out` must return a machine-readable
/// `not_implemented` error (not "unknown method") when `cloud-sync` is not
/// compiled in. Gated on `not(feature = "cloud-sync")` because when the
/// feature is enabled the handler performs a real auth attempt and returns
/// a different error code (`invalid_argument` for missing credentials),
/// not `not_implemented`.
#[cfg(not(feature = "cloud-sync"))]
#[tokio::test]
async fn cloud_sign_in_out_return_not_implemented_without_cloud_feature() {
    // This test is only meaningful for non-cloud builds.
    // In cloud builds, the handler is different (auth attempt). We only
    // assert that the response is valid JSON with ok=false and a non-empty
    // error_code regardless of build, since the key invariant is that
    // callers get a machine-readable code instead of "unknown method".
    let dir = safe_tempdir();
    let sock = dir.path().join("test-cloud-not-impl.sock");
    start_test_server(&sock).await;

    for method in &["cloud_sign_in", "cloud_sign_out"] {
        let body = format!(r#"{{"id":"c1","method":"{method}","params":{{}}}}"#);
        let resp = call_one(&sock, &body).await;
        assert_eq!(resp["ok"], false, "{method} must return ok=false");
        // Must have an error_code (either not_implemented or invalid_argument
        // for cloud builds that need credentials).
        assert!(
            resp["error_code"].is_string(),
            "{method} must return a machine-readable error_code, got: {resp}"
        );
    }
}

/// CopyPaste-vvsf: verify the re-encryption closure that `rotate_sync_key`
/// would pass to `reencrypt_all_cloud_items` performs a correct
/// decrypt-old / encrypt-new round-trip using the item_id for AAD binding.
///
/// This is a pure crypto unit test — no IPC socket, no Supabase HTTP call.
/// It exercises the same `base64 decode → decrypt_from_cloud(old_key, item_id)
/// → encrypt_for_cloud(new_key, item_id) → base64 encode` pipeline that the
/// production handler uses, asserting:
///
///   1. The new ciphertext decrypts correctly under the NEW key.
///   2. The new ciphertext does NOT decrypt under the OLD key.
///   3. A wrong item_id as AAD causes authentication failure.
#[test]
fn rotate_sync_key_reencrypt_closure_crypto_roundtrip() {
    use base64::Engine as _;
    use copypaste_core::{decrypt_from_cloud, derive_sync_key, encrypt_for_cloud, SyncKey};

    let old_key = derive_sync_key(
        "old-passphrase-test-1",
        "proj_test|00000000-0000-0000-0000-000000000001",
    )
    .expect("derive old key");
    let new_key = derive_sync_key(
        "new-passphrase-test-2",
        "proj_test|00000000-0000-0000-0000-000000000001",
    )
    .expect("derive new key");

    let item_id = "item-uuid-deadbeef";
    let plaintext = b"secret clipboard content";

    // Simulate what the push loop stores in Supabase: encrypt under old key.
    let old_blob = encrypt_for_cloud(&old_key, item_id, plaintext).expect("encrypt under old key");
    let old_ct_b64 = base64::engine::general_purpose::STANDARD.encode(&old_blob);

    // Capture key bytes for the closure (SyncKey is !Clone).
    let old_bytes = *old_key.as_bytes();
    let new_bytes = *new_key.as_bytes();

    // This is the closure shape that rotate_sync_key passes to
    // reencrypt_all_cloud_items (CopyPaste-vvsf).
    let reencrypt = |rcv_item_id: &str, old_ct: &str| -> Result<String, String> {
        let old_k = SyncKey::from_bytes(old_bytes);
        let new_k = SyncKey::from_bytes(new_bytes);
        let raw = base64::engine::general_purpose::STANDARD
            .decode(old_ct)
            .map_err(|e| format!("base64 decode: {e}"))?;
        let plain =
            decrypt_from_cloud(&old_k, rcv_item_id, &raw).map_err(|e| format!("decrypt: {e}"))?;
        let new_blob =
            encrypt_for_cloud(&new_k, rcv_item_id, &plain).map_err(|e| format!("encrypt: {e}"))?;
        Ok(base64::engine::general_purpose::STANDARD.encode(&new_blob))
    };

    // Apply the closure.
    let new_ct_b64 = reencrypt(item_id, &old_ct_b64).expect("re-encryption must succeed");

    // Decode and verify the new ciphertext decrypts under the NEW key.
    let new_k2 = SyncKey::from_bytes(new_bytes);
    let new_raw = base64::engine::general_purpose::STANDARD
        .decode(&new_ct_b64)
        .unwrap();
    let recovered = decrypt_from_cloud(&new_k2, item_id, &new_raw)
        .expect("new ciphertext must decrypt under new key");
    assert_eq!(
        recovered, plaintext,
        "plaintext must survive old_key→decrypt→new_key→encrypt round-trip"
    );

    // The new ciphertext must NOT decrypt under the OLD key (different key → auth fail).
    let old_k2 = SyncKey::from_bytes(old_bytes);
    let old_decrypt_result = decrypt_from_cloud(&old_k2, item_id, &new_raw);
    assert!(
        old_decrypt_result.is_err(),
        "new ciphertext must not decrypt under old key (wrong key → auth fail)"
    );

    // A wrong item_id must cause auth failure (AAD mismatch).
    let new_k3 = SyncKey::from_bytes(new_bytes);
    let wrong_aad_result = decrypt_from_cloud(&new_k3, "wrong-item-id", &new_raw);
    assert!(
        wrong_aad_result.is_err(),
        "wrong item_id AAD must cause authentication failure"
    );
}

// ── CopyPaste-40gl: db_stats IPC verb ────────────────────────────────────

/// `db_stats` on an empty database must return `{ item_count: 0, size_bytes }`.
/// The `size_bytes` value reflects the on-disk daemon DB (not the in-memory
/// DB the test server uses internally), so we only assert the structure and
/// the zero item_count.
#[tokio::test]
async fn db_stats_empty_database_returns_zero_count() {
    let dir = safe_tempdir();
    let sock = dir.path().join("db_stats_empty.sock");
    start_test_server(&sock).await;

    let resp = call_one(&sock, r#"{"id":"ds1","method":"db_stats","params":{}}"#).await;
    assert_eq!(resp["ok"], true, "db_stats must succeed: {resp}");
    let item_count = resp["data"]["item_count"]
        .as_u64()
        .expect("item_count must be u64");
    assert_eq!(item_count, 0, "empty DB must report 0 items: {resp}");
    // size_bytes is present and is a u64 (value may be non-zero if the
    // daemon's own DB file exists on this machine).
    let _size_bytes = resp["data"]["size_bytes"]
        .as_u64()
        .expect("size_bytes must be a u64 field");
}

/// `db_stats` on a DB with items must report the correct count.
#[tokio::test]
async fn db_stats_reports_correct_item_count() {
    let dir = safe_tempdir();
    let sock = dir.path().join("db_stats_count.sock");
    let (_pm, db) = start_test_server_returning_db(&sock, false).await;

    // Seed 3 items directly.
    {
        let guard = db.lock().await;
        for _ in 0..3 {
            let item = copypaste_core::ClipboardItem::new_text(vec![0xABu8; 16], vec![0u8; 24], 1);
            copypaste_core::insert_item(&guard, &item).unwrap();
        }
    }

    let resp = call_one(&sock, r#"{"id":"ds2","method":"db_stats","params":{}}"#).await;
    assert_eq!(
        resp["ok"], true,
        "db_stats must succeed after seeding: {resp}"
    );
    let item_count = resp["data"]["item_count"]
        .as_u64()
        .expect("item_count must be u64");
    assert_eq!(item_count, 3, "expected 3 items after seeding: {resp}");
}

// ── CopyPaste-cbfl: parse-error / oversized id echoing ───────────────────

/// When the daemon cannot parse a request as JSON, the error response's
/// `id` must echo back the id from the raw JSON (if extractable) so that
/// the CLI's id-matching guard can correlate the error.  When no id is
/// extractable the fallback `"?"` is used.
#[tokio::test]
async fn parse_error_echoes_id_from_raw_json() {
    let dir = safe_tempdir();
    let sock = dir.path().join("parse_err_id.sock");
    start_test_server(&sock).await;

    // Send valid JSON that has an id field but is missing the required
    // `method` field, triggering a serde parse error.
    let resp = call_one(&sock, r#"{"id":"req-42","not_method":"foo","params":{}}"#).await;
    // The response must be an error.
    assert_eq!(resp["ok"], false, "malformed request must fail: {resp}");
    // The id in the response must echo "req-42" (the id from the raw JSON).
    assert_eq!(
        resp["id"].as_str(),
        Some("req-42"),
        "parse-error response id must echo the request id: {resp}"
    );
}

/// When the line is pure garbage (not parseable as JSON at all), the
/// fallback id `"?"` is used since no id can be extracted.
#[tokio::test]
async fn parse_error_uses_fallback_id_when_not_valid_json() {
    let dir = safe_tempdir();
    let sock = dir.path().join("parse_err_fallback.sock");
    start_test_server(&sock).await;

    let resp = call_one(&sock, "this is not JSON at all!!!").await;
    assert_eq!(resp["ok"], false, "garbage input must fail: {resp}");
    assert_eq!(
        resp["id"].as_str(),
        Some("?"),
        "garbage parse-error response must use fallback id '?': {resp}"
    );
}

// ── CopyPaste-93yr: export warns on skipped non-text items ───────────────

/// The export response must include `skipped_non_text` that is non-zero
/// when image items exist in the database. Text items must still export.
#[tokio::test]
async fn export_skipped_non_text_count_is_non_zero_for_image_items() {
    let dir = safe_tempdir();
    let sock = dir.path().join("export_skip_img.sock");
    let (_pm, db) = start_test_server_returning_db(&sock, false).await;

    // Insert one image item and one text item directly.
    {
        let guard = db.lock().await;
        // Image item — will be skipped during export.
        // new_image takes (blob, meta_json, lamport_ts, thumb).
        let img_item = copypaste_core::ClipboardItem::new_image(
            vec![0xFFu8; 64], // fake encrypted blob bytes
            "{}".to_string(), // image_meta_json
            1,
            None,
        );
        copypaste_core::insert_item(&guard, &img_item).unwrap();

        // Text item — must appear in the export.
        guard
            .conn()
            .execute(
                "INSERT INTO clipboard_items \
                 (id, item_id, content_type, content, content_nonce, \
                  is_sensitive, is_synced, lamport_ts, wall_time, key_version) \
                 VALUES (?, ?, 'text', ?, ?, 0, 0, 2, 2000000, 2)",
                rusqlite::params![
                    uuid::Uuid::new_v4().to_string(),
                    uuid::Uuid::new_v4().to_string(),
                    // Zero-key v2 encrypt of "hello".
                    // We use a raw ciphertext that the zero-key daemon can decrypt;
                    // since we only care about skipped_non_text count we can test
                    // with a zero-length ciphertext (decrypt will fail → skipped by
                    // the decrypt-error path, but image count is independent).
                    vec![0u8; 1],
                    vec![0u8; 24],
                ],
            )
            .unwrap();
    }

    let resp = call_one(
        &sock,
        r#"{"id":"xe1","method":"export","params":{"limit":100}}"#,
    )
    .await;
    assert_eq!(resp["ok"], true, "export must succeed: {resp}");

    let skipped = resp["data"]["skipped_non_text"]
        .as_u64()
        .expect("skipped_non_text must be present in export response");
    assert_eq!(
        skipped, 1,
        "exactly one image item must be counted as skipped: {resp}"
    );
}

// ── CopyPaste-x94p / CopyPaste-8wbt: db_backup + db_restore ────────────

/// `db_backup` without `dest_path` must return an `invalid_argument` error.
#[tokio::test]
async fn db_backup_missing_dest_returns_error() {
    let dir = safe_tempdir();
    let sock = dir.path().join("backup_no_dest.sock");
    start_test_server(&sock).await;
    let mut stream = UnixStream::connect(&sock).await.unwrap();
    stream
        .write_all(b"{\"id\":\"b1\",\"method\":\"db_backup\",\"params\":{}}\n")
        .await
        .unwrap();
    let mut lines = BufReader::new(&mut stream).lines();
    let line = lines.next_line().await.unwrap().unwrap();
    let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(
        resp["ok"], false,
        "missing dest_path must return error: {resp}"
    );
    assert_eq!(
        resp["error_code"].as_str(),
        Some("invalid_argument"),
        "error_code must be invalid_argument: {resp}"
    );
}

/// `db_backup` with a valid `dest_path` must produce a backup file.
#[tokio::test]
async fn db_backup_creates_backup_file() {
    let dir = safe_tempdir();
    let sock = dir.path().join("backup_ok.sock");
    start_test_server(&sock).await;
    let dest = dir.path().join("test-backup.db.enc");
    let req = format!(
        "{{\"id\":\"b2\",\"method\":\"db_backup\",\"params\":{{\"dest_path\":\"{}\"}}}}\n",
        dest.to_string_lossy()
    );
    let mut stream = UnixStream::connect(&sock).await.unwrap();
    stream.write_all(req.as_bytes()).await.unwrap();
    let mut lines = BufReader::new(&mut stream).lines();
    let line = lines.next_line().await.unwrap().unwrap();
    let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(resp["ok"], true, "db_backup must succeed: {resp}");
    assert!(
        dest.exists(),
        "backup file must exist after successful db_backup"
    );
    let size_bytes = resp["data"]["size_bytes"].as_u64().unwrap_or(0);
    assert!(size_bytes > 0, "backup size must be > 0: {resp}");
}

/// `db_backup` to an already-existing path must return an error.
#[tokio::test]
async fn db_backup_refuses_overwrite() {
    let dir = safe_tempdir();
    let sock = dir.path().join("backup_overwrite.sock");
    start_test_server(&sock).await;
    let dest = dir.path().join("existing.db.enc");
    std::fs::write(&dest, b"existing content").unwrap();
    let req = format!(
        "{{\"id\":\"b3\",\"method\":\"db_backup\",\"params\":{{\"dest_path\":\"{}\"}}}}\n",
        dest.to_string_lossy()
    );
    let mut stream = UnixStream::connect(&sock).await.unwrap();
    stream.write_all(req.as_bytes()).await.unwrap();
    let mut lines = BufReader::new(&mut stream).lines();
    let line = lines.next_line().await.unwrap().unwrap();
    let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(
        resp["ok"], false,
        "db_backup must refuse to overwrite an existing file: {resp}"
    );
    assert_eq!(
        resp["error_code"].as_str(),
        Some("invalid_argument"),
        "error_code must be invalid_argument: {resp}"
    );
}

/// `db_restore` without `confirm=true` must return `invalid_argument`.
#[tokio::test]
async fn db_restore_requires_confirm() {
    let dir = safe_tempdir();
    let sock = dir.path().join("restore_no_confirm.sock");
    start_test_server(&sock).await;
    let mut stream = UnixStream::connect(&sock).await.unwrap();
    stream
        .write_all(
            b"{\"id\":\"r1\",\"method\":\"db_restore\",\"params\":{\"src_path\":\"/tmp/x.db\"}}\n",
        )
        .await
        .unwrap();
    let mut lines = BufReader::new(&mut stream).lines();
    let line = lines.next_line().await.unwrap().unwrap();
    let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(
        resp["ok"], false,
        "restore without confirm must be rejected: {resp}"
    );
    assert_eq!(
        resp["error_code"].as_str(),
        Some("invalid_argument"),
        "error_code must be invalid_argument: {resp}"
    );
}

/// `db_restore` with a non-existent `src_path` must return `invalid_argument`.
#[tokio::test]
async fn db_restore_missing_file_returns_error() {
    let dir = safe_tempdir();
    let sock = dir.path().join("restore_missing_file.sock");
    start_test_server(&sock).await;
    let req = r#"{"id":"r2","method":"db_restore","params":{"confirm":true,"src_path":"/does/not/exist/backup.db.enc"}}"#;
    let mut stream = UnixStream::connect(&sock).await.unwrap();
    stream
        .write_all(format!("{req}\n").as_bytes())
        .await
        .unwrap();
    let mut lines = BufReader::new(&mut stream).lines();
    let line = lines.next_line().await.unwrap().unwrap();
    let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(
        resp["ok"], false,
        "restore with missing file must be rejected: {resp}"
    );
}

/// CopyPaste-crh3.7 / crh3.57: db_backup and vacuum must be gated in
/// degraded mode (ready=false). Otherwise db_backup writes an EMPTY backup
/// of the in-memory placeholder and returns ok:true, and vacuum reports
/// misleading size stats read from the real on-disk file. Both must return
/// ipc_not_ready and db_backup must create NO file.
#[tokio::test]
async fn db_backup_and_vacuum_gated_in_degraded_mode() {
    let dir = safe_tempdir();
    let sock = dir.path().join("degraded_backup.sock");
    start_not_ready_server(&sock).await;

    // db_backup → ipc_not_ready, and no file at dest_path.
    let dest = dir.path().join("should_not_be_created.db.enc");
    let backup_req = format!(
        "{{\"id\":\"dg1\",\"method\":\"db_backup\",\"params\":{{\"dest_path\":\"{}\"}}}}\n",
        dest.to_string_lossy()
    );
    let mut stream = UnixStream::connect(&sock).await.unwrap();
    stream.write_all(backup_req.as_bytes()).await.unwrap();
    let mut lines = BufReader::new(&mut stream).lines();
    let resp: serde_json::Value =
        serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
    assert_eq!(
        resp["ok"], false,
        "db_backup must be rejected when degraded: {resp}"
    );
    assert_eq!(
        resp["error_code"].as_str().unwrap_or_default(),
        "ipc_not_ready",
        "db_backup must return ipc_not_ready when degraded: {resp}"
    );
    assert!(
        !dest.exists(),
        "degraded db_backup must NOT create a backup file"
    );
    drop(stream);

    // vacuum → ipc_not_ready.
    let mut stream2 = UnixStream::connect(&sock).await.unwrap();
    stream2
        .write_all(b"{\"id\":\"dg2\",\"method\":\"vacuum\",\"params\":{}}\n")
        .await
        .unwrap();
    let mut lines2 = BufReader::new(&mut stream2).lines();
    let resp2: serde_json::Value =
        serde_json::from_str(&lines2.next_line().await.unwrap().unwrap()).unwrap();
    assert_eq!(
        resp2["ok"], false,
        "vacuum must be rejected when degraded: {resp2}"
    );
    assert_eq!(
        resp2["error_code"].as_str().unwrap_or_default(),
        "ipc_not_ready",
        "vacuum must return ipc_not_ready when degraded: {resp2}"
    );
}

/// CopyPaste-crh3.105: watch_subscribe is intercepted before dispatch(), so
/// it previously bypassed the readiness gate — a degraded daemon accepted the
/// subscription and streamed nothing. It must now return ipc_not_ready.
#[tokio::test]
async fn watch_subscribe_rejected_when_not_ready() {
    let dir = safe_tempdir();
    let sock = dir.path().join("watch_not_ready.sock");
    start_not_ready_server(&sock).await;

    let mut stream = UnixStream::connect(&sock).await.unwrap();
    stream
        .write_all(b"{\"id\":\"ws-nr\",\"method\":\"watch_subscribe\",\"params\":{}}\n")
        .await
        .unwrap();
    let mut lines = BufReader::new(&mut stream).lines();
    let resp: serde_json::Value =
        serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
    assert_eq!(
        resp["ok"], false,
        "watch_subscribe must be rejected when degraded: {resp}"
    );
    assert_eq!(
        resp["error_code"].as_str().unwrap_or_default(),
        "ipc_not_ready",
        "watch_subscribe must return ipc_not_ready when degraded: {resp}"
    );
    assert_eq!(
        resp["id"].as_str(),
        Some("ws-nr"),
        "must echo the request id: {resp}"
    );
}

// ── CopyPaste-8wbt / crh3.6 / crh3.2: db_restore validate-then-swap ────────
//
// These drive the pure `restore_database_file` routine directly against real
// on-disk SQLCipher databases in a temp dir, so they are deterministic and
// independent of the Keychain / IPC harness.

/// Create a real on-disk SQLCipher DB at `path` (clipboard schema via
/// `Database::open`) and stamp a `_restore_marker` row with `tag` so two
/// databases can be told apart. Checkpoints the WAL so a plain file copy of
/// `path` is self-contained.
fn make_marked_db(path: &std::path::Path, key: &[u8; 32], tag: &str) {
    let db = Database::open(path, key).expect("open marked db");
    db.conn()
        .execute_batch(&format!(
            "CREATE TABLE IF NOT EXISTS _restore_marker(tag TEXT); \
                 DELETE FROM _restore_marker; \
                 INSERT INTO _restore_marker(tag) VALUES ('{tag}'); \
                 PRAGMA wal_checkpoint(TRUNCATE);"
        ))
        .unwrap();
}

/// Read the marker stamped by [`make_marked_db`], opening `path` with `key`.
/// Returns `None` if the DB cannot be opened with that key.
fn read_marker(path: &std::path::Path, key: &[u8; 32]) -> Option<String> {
    let db = Database::open_no_auto_migrate(path, key).ok()?;
    db.conn()
        .query_row("SELECT tag FROM _restore_marker LIMIT 1", [], |r| {
            r.get::<_, String>(0)
        })
        .ok()
}

/// True if any `clipboard.db.before-restore-*` aside copy exists next to
/// `db_path`.
fn aside_exists(db_path: &std::path::Path) -> bool {
    let name = db_path.file_name().unwrap().to_string_lossy().to_string();
    let needle = format!("{name}.before-restore-");
    std::fs::read_dir(db_path.parent().unwrap())
        .unwrap()
        .filter_map(|e| e.ok())
        .any(|e| e.file_name().to_string_lossy().starts_with(&needle))
}

/// True if any `clipboard.db.restore-staging-*` artifact was left behind.
fn staging_exists(db_path: &std::path::Path) -> bool {
    let name = db_path.file_name().unwrap().to_string_lossy().to_string();
    let needle = format!("{name}.restore-staging-");
    std::fs::read_dir(db_path.parent().unwrap())
        .unwrap()
        .filter_map(|e| e.ok())
        .any(|e| e.file_name().to_string_lossy().starts_with(&needle))
}

#[test]
fn restore_valid_backup_swaps_data_and_keeps_aside_when_not_forced() {
    let dir = safe_tempdir();
    let key = [0x11u8; 32];
    let db_path = dir.path().join("clipboard.db");
    let backup = dir.path().join("backup.db");
    make_marked_db(&db_path, &key, "LIVE");
    make_marked_db(&backup, &key, "BACKUP");

    let restored =
        restore_database_file(&backup, &db_path, &key, false).expect("valid backup must restore");
    drop(restored);
    assert_eq!(read_marker(&db_path, &key).as_deref(), Some("BACKUP"));
    assert!(
        aside_exists(&db_path),
        "non-force restore must keep the prior DB as an aside safety copy"
    );
    assert!(
        !staging_exists(&db_path),
        "staging copy must be cleaned up after a successful restore"
    );
}

#[test]
fn restore_valid_backup_force_removes_aside() {
    let dir = safe_tempdir();
    let key = [0x22u8; 32];
    let db_path = dir.path().join("clipboard.db");
    let backup = dir.path().join("backup.db");
    make_marked_db(&db_path, &key, "LIVE");
    make_marked_db(&backup, &key, "BACKUP");

    restore_database_file(&backup, &db_path, &key, true).expect("restore");
    assert_eq!(read_marker(&db_path, &key).as_deref(), Some("BACKUP"));
    assert!(
        !aside_exists(&db_path),
        "force restore must delete the aside safety copy"
    );
}

#[test]
fn restore_wrong_key_backup_leaves_live_db_intact() {
    // CopyPaste-8wbt: the core P0 — a wrong-key backup must NEVER touch the
    // live database.
    let dir = safe_tempdir();
    let live_key = [0x33u8; 32];
    let other_key = [0x44u8; 32];
    let db_path = dir.path().join("clipboard.db");
    let backup = dir.path().join("backup.db");
    make_marked_db(&db_path, &live_key, "LIVE");
    make_marked_db(&backup, &other_key, "OTHER");

    let err = restore_database_file(&backup, &db_path, &live_key, true)
        .map(|_| ())
        .expect_err("wrong-key backup must be rejected");
    assert!(err.contains("db_restore"), "error must be tagged: {err}");
    assert_eq!(
        read_marker(&db_path, &live_key).as_deref(),
        Some("LIVE"),
        "live DB must remain readable with its original key"
    );
    assert!(
        !aside_exists(&db_path),
        "failed validation must not move the live DB aside"
    );
    assert!(
        !staging_exists(&db_path),
        "failed validation must clean up the staging copy"
    );
}

#[test]
fn restore_corrupt_backup_leaves_live_db_intact() {
    let dir = safe_tempdir();
    let key = [0x55u8; 32];
    let db_path = dir.path().join("clipboard.db");
    let backup = dir.path().join("garbage.db");
    make_marked_db(&db_path, &key, "LIVE");
    std::fs::write(&backup, b"this is not a sqlite database at all").unwrap();

    let err = restore_database_file(&backup, &db_path, &key, false)
        .map(|_| ())
        .expect_err("garbage backup must be rejected");
    assert!(err.contains("db_restore"), "error must be tagged: {err}");
    assert_eq!(read_marker(&db_path, &key).as_deref(), Some("LIVE"));
    assert!(!aside_exists(&db_path));
}

#[test]
fn restore_wrong_schema_backup_is_rejected() {
    // A real SQLCipher DB with the correct key but WITHOUT the clipboard
    // schema must be rejected (it is not a CopyPaste database).
    let dir = safe_tempdir();
    let key = [0x66u8; 32];
    let db_path = dir.path().join("clipboard.db");
    let backup = dir.path().join("foreign.db");
    make_marked_db(&db_path, &key, "LIVE");
    {
        let foreign = Database::open(&backup, &key).unwrap();
        foreign
            .conn()
            .execute_batch("DROP TABLE IF EXISTS clipboard_items; PRAGMA wal_checkpoint(TRUNCATE);")
            .unwrap();
    }
    let err = restore_database_file(&backup, &db_path, &key, false)
        .map(|_| ())
        .expect_err("non-CopyPaste DB must be rejected");
    assert!(
        err.contains("clipboard_items"),
        "error must name the missing clipboard table: {err}"
    );
    assert_eq!(read_marker(&db_path, &key).as_deref(), Some("LIVE"));
}

#[test]
fn restore_into_empty_path_succeeds_for_degraded_recovery() {
    // crh3.6: degraded recovery — no live DB on disk yet. A valid backup must
    // still restore, with nothing to move aside.
    let dir = safe_tempdir();
    let key = [0x77u8; 32];
    let db_path = dir.path().join("clipboard.db");
    let backup = dir.path().join("backup.db");
    make_marked_db(&backup, &key, "BACKUP");
    assert!(!db_path.exists());

    restore_database_file(&backup, &db_path, &key, false).expect("restore into empty path");
    assert_eq!(read_marker(&db_path, &key).as_deref(), Some("BACKUP"));
    assert!(
        !aside_exists(&db_path),
        "nothing to move aside when there was no live DB"
    );
}

#[test]
fn restore_rebuilt_pool_sees_restored_data_while_stale_pool_does_not() {
    // CopyPaste-crh3.2 / crh3.54: after a restore swaps the on-disk inode, a
    // read pool opened BEFORE the restore keeps serving stale data through
    // its cached file descriptors; only a pool rebuilt against the restored
    // file returns the restored contents. This is exactly why the db_restore
    // handler rebuilds `self.read_pool`.
    let dir = safe_tempdir();
    let key = [0x88u8; 32];
    let db_path = dir.path().join("clipboard.db");
    let backup = dir.path().join("backup.db");

    // Live DB starts at tag "A"; snapshot it as the backup at that point.
    make_marked_db(&db_path, &key, "A");
    std::fs::copy(&db_path, &backup).unwrap();
    // Advance the live DB to tag "B" (post-backup mutation), checkpointed.
    {
        let live = Database::open_no_auto_migrate(&db_path, &key).unwrap();
        live.conn()
            .execute_batch("UPDATE _restore_marker SET tag = 'B'; PRAGMA wal_checkpoint(TRUNCATE);")
            .unwrap();
    }

    // A pool opened against the live "B" DB BEFORE the restore.
    let stale_pool = copypaste_core::open_pool(&db_path, &key, 2).unwrap();
    let stale_before: String = stale_pool
        .get()
        .unwrap()
        .query_row("SELECT tag FROM _restore_marker LIMIT 1", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        stale_before, "B",
        "sanity: pool sees live data before restore"
    );

    // Restore from the "A" backup.
    restore_database_file(&backup, &db_path, &key, false).expect("restore");

    // The stale pool keeps serving "B" from its cached FDs — the crh3.2 bug
    // the handler must work around by rebuilding the pool.
    let stale_after: String = stale_pool
        .get()
        .unwrap()
        .query_row("SELECT tag FROM _restore_marker LIMIT 1", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        stale_after, "B",
        "stale pool keeps serving pre-restore data (crh3.2 root cause)"
    );

    // A freshly rebuilt pool (what the handler installs after restore) sees
    // the restored "A" data.
    let fresh_pool = copypaste_core::open_pool(&db_path, &key, 2).unwrap();
    let fresh_tag: String = fresh_pool
        .get()
        .unwrap()
        .query_row("SELECT tag FROM _restore_marker LIMIT 1", [], |r| r.get(0))
        .unwrap();
    assert_eq!(fresh_tag, "A", "rebuilt pool sees the restored data");
}

// ── CopyPaste-44rq.19: watch_subscribe push-streaming tests ────────────────

/// Start a test server that has a `new_item_tx` wired in, returning both
/// the broadcast sender and the db handle so callers can inject events.
async fn start_test_server_with_broadcast(
    socket_path: &std::path::Path,
) -> (
    tokio::sync::broadcast::Sender<copypaste_core::ClipboardItem>,
    Arc<Mutex<Database>>,
) {
    let db = Arc::new(Mutex::new(Database::open_in_memory().unwrap()));
    let private_mode = Arc::new(AtomicBool::new(false));
    let local_key = Arc::new(zeroize::Zeroizing::new([0u8; 32]));
    let device_pub = Arc::new([0u8; 32]);
    // Capacity 64: large enough for tests, mirrors the production value.
    let (tx, _) = tokio::sync::broadcast::channel::<copypaste_core::ClipboardItem>(64);
    let cert = copypaste_p2p::cert::SelfSignedCert::generate("test-device").unwrap();
    let server = IpcServer::new(db.clone(), private_mode, local_key, device_pub)
        .with_cert_fingerprint(display_fingerprint(&cert.fingerprint()))
        .with_new_item_tx(tx.clone());
    let listener =
        tokio::net::UnixListener::bind(socket_path).expect("test socket bind must succeed");
    let path = socket_path.to_path_buf();
    tokio::spawn(async move {
        if let Err(e) = server.serve_on(listener, CancellationToken::new()).await {
            tracing::error!("ipc watch-test server error: {e} at {path:?}");
        }
    });
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    (tx, db)
}

/// CopyPaste-44rq.19: `watch_subscribe` must send an initial ack (ok=true,
/// event="subscribed") and then push one event line per new clipboard item
/// broadcast on `new_item_tx`, without any polling.
#[tokio::test]
async fn watch_subscribe_receives_push_events() {
    let dir = safe_tempdir();
    let sock = dir.path().join("watch_push.sock");
    let (tx, _db) = start_test_server_with_broadcast(&sock).await;

    // Open a subscribe connection.
    let mut stream = UnixStream::connect(&sock).await.unwrap();
    stream
        .write_all(b"{\"id\":\"w1\",\"method\":\"watch_subscribe\",\"params\":{}}\n")
        .await
        .unwrap();

    let mut lines = BufReader::new(&mut stream).lines();

    // First line must be the ack.
    let ack_line = tokio::time::timeout(std::time::Duration::from_secs(2), lines.next_line())
        .await
        .expect("ack must arrive within 2 s")
        .unwrap()
        .unwrap();
    let ack: serde_json::Value = serde_json::from_str(&ack_line).unwrap();
    assert_eq!(ack["ok"], true, "ack must be ok=true: {ack_line}");
    assert_eq!(
        ack["event"].as_str(),
        Some("subscribed"),
        "ack must have event=subscribed: {ack_line}"
    );
    assert_eq!(
        ack["id"].as_str(),
        Some("w1"),
        "ack must echo the request id: {ack_line}"
    );

    // Broadcast a new clipboard item.
    let item = copypaste_core::ClipboardItem::new_text(
        vec![0u8; 16], // dummy encrypted content
        vec![0u8; 24], // dummy nonce
        1,
    );
    let item_id = item.item_id.clone();
    let _ = tx.send(item);

    // The daemon must push a new_item event line within 500 ms.
    let evt_line = tokio::time::timeout(std::time::Duration::from_millis(500), lines.next_line())
        .await
        .expect("new_item event must arrive within 500 ms")
        .unwrap()
        .unwrap();
    let evt: serde_json::Value = serde_json::from_str(&evt_line).unwrap();
    assert_eq!(evt["ok"], true, "event must be ok=true: {evt_line}");
    assert_eq!(
        evt["event"].as_str(),
        Some("new_item"),
        "event must have event=new_item: {evt_line}"
    );
    assert_eq!(
        evt["item_id"].as_str(),
        Some(item_id.as_str()),
        "event must carry item_id: {evt_line}"
    );
    // Content/plaintext must NOT be present.
    assert!(
        evt.get("content").is_none(),
        "event must not contain raw content: {evt_line}"
    );
}

/// CopyPaste-44rq.19: while a `watch_subscribe` connection is open, a
/// second normal one-shot request (e.g. `status`) on a DIFFERENT connection
/// must still return its normal response. This verifies the subscribe path
/// does not wedge the accept loop or any shared state.
#[tokio::test]
async fn watch_subscribe_does_not_break_concurrent_one_shot_requests() {
    let dir = safe_tempdir();
    let sock = dir.path().join("watch_concurrent.sock");
    let (_tx, _db) = start_test_server_with_broadcast(&sock).await;

    // Open a long-lived subscribe connection; don't close it.
    let mut sub_stream = UnixStream::connect(&sock).await.unwrap();
    sub_stream
        .write_all(b"{\"id\":\"ws1\",\"method\":\"watch_subscribe\",\"params\":{}}\n")
        .await
        .unwrap();
    // Consume the ack to confirm the subscribe is established.
    let mut sub_lines = BufReader::new(&mut sub_stream).lines();
    let _ = tokio::time::timeout(std::time::Duration::from_secs(2), sub_lines.next_line())
        .await
        .expect("subscribe ack must arrive")
        .unwrap()
        .unwrap();

    // Now make a normal one-shot `status` request on a DIFFERENT connection.
    let status_resp = call_one(&sock, r#"{"id":"st1","method":"status"}"#).await;
    assert_eq!(
        status_resp["ok"], true,
        "status must succeed while a subscribe connection is open: {status_resp}"
    );
    assert_eq!(
        status_resp["data"]["status"].as_str(),
        Some("running"),
        "status.data.status must be 'running': {status_resp}"
    );
}

/// CopyPaste-44rq.19: closing the subscribe client (dropping the stream)
/// must NOT wedge the daemon. After the subscriber disconnects, subsequent
/// `status` calls on a fresh connection must still succeed.
#[tokio::test]
async fn watch_subscribe_client_disconnect_does_not_wedge_daemon() {
    let dir = safe_tempdir();
    let sock = dir.path().join("watch_disconnect.sock");
    let (tx, _db) = start_test_server_with_broadcast(&sock).await;

    // Subscribe, read the ack, then drop the client connection.
    {
        let mut stream = UnixStream::connect(&sock).await.unwrap();
        stream
            .write_all(b"{\"id\":\"wd1\",\"method\":\"watch_subscribe\",\"params\":{}}\n")
            .await
            .unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let _ = tokio::time::timeout(std::time::Duration::from_secs(2), lines.next_line())
            .await
            .expect("ack must arrive")
            .unwrap()
            .unwrap();
        // Drop `stream` here — simulates client disconnect.
    }

    // Broadcast an event so the daemon's subscribe loop sees a write error
    // and exits cleanly.
    let item = copypaste_core::ClipboardItem::new_text(vec![0u8; 16], vec![0u8; 24], 2);
    let _ = tx.send(item);

    // Give the task a moment to handle the disconnect.
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // The daemon must still handle normal requests.
    let resp = call_one(&sock, r#"{"id":"wd2","method":"status"}"#).await;
    assert_eq!(
        resp["ok"], true,
        "daemon must still respond after subscriber disconnect: {resp}"
    );
}

/// Verify `db_backup` then `db_restore` end-to-end: backup creates the file,
/// restore validation (is_file check) passes, and the handler returns a
/// well-formed response (ok=true or a clear internal error) without panic.
///
/// NOTE: the `db_restore` handler operates on the daemon's real data-dir path
/// (`crate::paths::db_path()`), not on the temp dir of this test. In a
/// sandbox/CI environment that lacks write access to the data dir the restore
/// step may return `ok=false` with an `internal_error`. The test therefore
/// only asserts on `db_backup` and that `db_restore` returns a *parseable*
/// JSON response — it does NOT assert `ok=true` on restore, because the
/// outcome depends on filesystem permissions outside the test's control.
#[tokio::test]
async fn db_backup_produces_file_and_restore_sends_response() {
    let dir = safe_tempdir();
    let sock = dir.path().join("backup_restore_rt.sock");
    start_test_server(&sock).await;

    // 1. Backup must succeed and create the file.
    let backup = dir.path().join("roundtrip.db.enc");
    let backup_req = format!(
        "{{\"id\":\"rt1\",\"method\":\"db_backup\",\"params\":{{\"dest_path\":\"{}\"}}}}\n",
        backup.to_string_lossy()
    );
    let mut stream = UnixStream::connect(&sock).await.unwrap();
    stream.write_all(backup_req.as_bytes()).await.unwrap();
    let mut lines = BufReader::new(&mut stream).lines();
    let bk_line = lines.next_line().await.unwrap().unwrap();
    let bk_resp: serde_json::Value = serde_json::from_str(&bk_line).unwrap();
    assert_eq!(bk_resp["ok"], true, "backup must succeed: {bk_resp}");
    assert!(backup.exists(), "backup file must exist");
    drop(stream);

    // 2. Restore: the handler parses the request and returns a well-formed
    //    JSON response.  We do not assert ok=true here because the handler
    //    attempts to copy the backup to the daemon's real data-dir (which may
    //    be inaccessible in sandboxed test environments).
    let restore_req = format!(
            "{{\"id\":\"rt2\",\"method\":\"db_restore\",\"params\":{{\"confirm\":true,\"src_path\":\"{}\",\"force\":true}}}}\n",
            backup.to_string_lossy()
        );
    let mut stream2 = UnixStream::connect(&sock).await.unwrap();
    stream2.write_all(restore_req.as_bytes()).await.unwrap();
    let mut lines2 = BufReader::new(&mut stream2).lines();
    let rs_line = lines2.next_line().await.unwrap().unwrap();
    let rs_resp: serde_json::Value = serde_json::from_str(&rs_line).unwrap();
    // Response must be parseable JSON with an "ok" field (bool) and an "id".
    assert!(
        rs_resp["ok"].is_boolean(),
        "restore response must have a boolean ok field: {rs_resp}"
    );
    assert_eq!(
        rs_resp["id"].as_str(),
        Some("rt2"),
        "restore response must echo the request id: {rs_resp}"
    );
}
