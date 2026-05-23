//! Integration tests for the OPAQUE-KE PAKE handshake (W2.4 beta bonus).
//!
//! These exercise the public `copypaste_p2p::pake` API end-to-end without
//! reaching into the module's `#[cfg(test)]` internals. They complement the
//! in-source unit tests by validating the contract surface a real caller
//! (pairing flow) would see: a 3-message handshake, wrong-password handling
//! that does not panic, replay rejection across distinct handshake sessions,
//! and the documented 32-byte session-key size.

use copypaste_p2p::pake::{PakeError, PakeInitiator, PakeResponder, PasswordFile};

/// Run one full 3-message handshake and return both derived session keys.
fn run_handshake(
    password_file: &PasswordFile,
    client_password: &str,
) -> Result<([u8; 32], [u8; 32]), PakeError> {
    let (client, msg1) = PakeInitiator::new(client_password)?;
    let (server, msg2) = PakeResponder::respond(password_file, &msg1)?;
    let (client_key, msg3) = client.finish(&msg2)?;
    let server_key = server.finish(&msg3)?;
    Ok((*client_key.as_bytes(), *server_key.as_bytes()))
}

#[test]
fn client_server_handshake_yields_identical_shared_secret() {
    let password = "pairing-code-123456";
    let pf = PasswordFile::register(password).expect("registration succeeds");

    let (client_key, server_key) =
        run_handshake(&pf, password).expect("happy-path handshake succeeds");

    assert_eq!(
        client_key, server_key,
        "client and server must derive byte-identical SessionKeys"
    );
    // Sanity: a fresh handshake with the same PasswordFile must still match,
    // proving the PasswordFile is reusable (it is — see ADR-008).
    let (client_key2, server_key2) =
        run_handshake(&pf, password).expect("second handshake succeeds");
    assert_eq!(client_key2, server_key2);
}

#[test]
fn wrong_password_yields_different_shared_secret_no_panic() {
    let pf = PasswordFile::register("correct-pairing-code").expect("registration succeeds");

    let (client, msg1) =
        PakeInitiator::new("wrong-pairing-code").expect("client start does not error");
    let (server, msg2) = PakeResponder::respond(&pf, &msg1)
        .expect("server respond does not error mid-handshake (OPAQUE design)");

    // OPAQUE detects the mismatch at the client's `finish` step (envelope
    // decryption fails). Must surface as `InvalidPassword`, never panic.
    let client_res = client.finish(&msg2);
    assert!(
        matches!(client_res, Err(PakeError::InvalidPassword)),
        "expected InvalidPassword on wrong-password finalization, got {:?}",
        client_res.as_ref().err()
    );

    // Server must also reject any forged finalization a malicious client
    // could send after seeing msg2 — no panic, just an error.
    let forged_finalization = vec![0u8; 192];
    let server_res = server.finish(&forged_finalization);
    assert!(
        server_res.is_err(),
        "server must reject forged finalization without panic"
    );
}

#[test]
fn replay_old_client_message_rejected() {
    // Two independent handshakes against the SAME PasswordFile. The
    // finalization message produced in handshake A is bound to A's server
    // nonces — replaying it to handshake B's server must be rejected.
    let password = "replay-test-pw";
    let pf = PasswordFile::register(password).expect("registration succeeds");

    // Handshake A: run to completion to capture the client's final message.
    let (client_a, msg1_a) = PakeInitiator::new(password).expect("client A start");
    let (_server_a, msg2_a) =
        PakeResponder::respond(&pf, &msg1_a).expect("server A respond");
    let (_key_a, msg3_a) = client_a.finish(&msg2_a).expect("client A finish");

    // Handshake B: fresh server, brand-new nonces. Feed it A's stale
    // finalization message instead of B's own.
    let (client_b, msg1_b) = PakeInitiator::new(password).expect("client B start");
    let (server_b, _msg2_b) =
        PakeResponder::respond(&pf, &msg1_b).expect("server B respond");
    drop(client_b); // we don't need B's real finalization

    let replay_res = server_b.finish(&msg3_a);
    assert!(
        replay_res.is_err(),
        "server B must reject a finalization from handshake A (nonce-bound)"
    );
}

#[test]
fn shared_secret_is_32_bytes() {
    // Matches XChaCha20-Poly1305 key size (ADR-001). The compile-time type
    // `&[u8; 32]` already enforces this, but assert at runtime so any future
    // refactor that widens the return type fails loudly here.
    let password = "size-check-pw";
    let pf = PasswordFile::register(password).expect("registration succeeds");

    let (client, msg1) = PakeInitiator::new(password).expect("client start");
    let (server, msg2) = PakeResponder::respond(&pf, &msg1).expect("server respond");
    let (client_key, msg3) = client.finish(&msg2).expect("client finish");
    let server_key = server.finish(&msg3).expect("server finish");

    assert_eq!(client_key.as_bytes().len(), 32, "client SessionKey is 32 bytes");
    assert_eq!(server_key.as_bytes().len(), 32, "server SessionKey is 32 bytes");
}
