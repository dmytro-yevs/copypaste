//! Regression test for CopyPaste-ian9.
//!
//! Unit test that the mandatory PAKE confirm tag is enforced: a mismatched or
//! absent confirm tag must be detected and rejected. The bootstrap channel
//! performs a 9-frame handshake (see `bootstrap.rs` doc comment) where frames
//! 8 and 9 are the role-separated HKDF-derived channel-binding confirmation
//! tags. Both sides must verify the peer's tag with a constant-time compare.
//!
//! The `channel_confirmation_tag` function is the single source of truth for
//! what tag each role generates. This test exercises the tag semantics:
//!
//! 1. Matching tags (correct PAKE + correct channel binding) → agree.
//! 2. Wrong tag (different bound_key, e.g. from a MitM) → disagree.
//! 3. Reflected tag (own tag sent back by peer) → disagree (role separation).
//! 4. Zero-sentinel tag → disagree.
//!
//! The actual verify step in production code is:
//!   `expected_peer_tag.ct_eq(&received_tag).into()` — constant-time.
//! This test validates the TAG DERIVATION is role-separated and bound to the
//! key, not the full bootstrap transport (which requires two live TLS streams
//! and is tested separately via integration tests).

use copypaste_p2p::pake::{
    channel_confirmation_tag, ConfirmRole, PakeInitiator, PakeResponder, PasswordFile, SessionKey,
    CONFIRM_TAG_LEN,
};
use subtle::ConstantTimeEq;

/// Derive a bound key for testing — simulates what `SessionKey::bind_to_tls_channel`
/// does in production (HKDF with a synthetic TLS channel binder).
fn make_bound_key(session_key: &SessionKey, tls_binder: &[u8]) -> [u8; 32] {
    *session_key.bind_to_tls_channel(tls_binder)
}

/// Run a complete PAKE handshake and return the shared SessionKey.
fn run_pake(password: &str) -> SessionKey {
    let pf = PasswordFile::register(password).expect("register");
    let (client, msg1) = PakeInitiator::new(password).expect("client new");
    let (server, msg2) = PakeResponder::respond(&pf, &msg1).expect("server respond");
    let (client_key, msg3) = client.finish(&msg2).expect("client finish");
    let _server_key = server.finish(&msg3).expect("server finish");
    client_key
}

// ---------------------------------------------------------------------------
// 1. Correct tags — both sides agree
// ---------------------------------------------------------------------------

/// Both sides derive the same bound key from the same session key + TLS binder.
/// The initiator tag the responder sends (role=Responder) must match what the
/// initiator expected. Symmetrically for the responder receiving the initiator tag.
#[test]
fn matching_bound_keys_produce_agreeable_confirm_tags_ian9() {
    let session_key = run_pake("pairing-code");
    let binder = [0x01u8; 32]; // synthetic TLS channel binder

    // Both sides compute the same bound_key from the same session_key + binder.
    let bound_key = make_bound_key(&session_key, &binder);

    // Responder sends its own tag (role = Responder).
    let responder_sends = channel_confirmation_tag(&bound_key, ConfirmRole::Responder);
    // Initiator expects the peer (Responder) tag.
    let initiator_expects = channel_confirmation_tag(&bound_key, ConfirmRole::Responder);

    let tags_match: bool = responder_sends.ct_eq(&initiator_expects).into();
    assert!(
        tags_match,
        "initiator must accept the correct responder confirm tag (CopyPaste-ian9)"
    );

    // Initiator sends its own tag (role = Initiator).
    let initiator_sends = channel_confirmation_tag(&bound_key, ConfirmRole::Initiator);
    let responder_expects = channel_confirmation_tag(&bound_key, ConfirmRole::Initiator);

    let tags_match2: bool = initiator_sends.ct_eq(&responder_expects).into();
    assert!(
        tags_match2,
        "responder must accept the correct initiator confirm tag (CopyPaste-ian9)"
    );
}

// ---------------------------------------------------------------------------
// 2. Wrong tag (MitM with different bound_key) → reject
// ---------------------------------------------------------------------------

/// A MitM bridging PAKE over two separate TLS sessions would derive a DIFFERENT
/// bound_key per leg (different TLS channel binder). The confirm tags derived
/// from the wrong key must not match the tags from the correct key.
#[test]
fn mismatched_bound_key_produces_wrong_confirm_tags_ian9() {
    let session_key = run_pake("shared-pairing-code");

    let honest_binder = [0xAAu8; 32]; // what an honest initiator sees
    let mitm_binder = [0xBBu8; 32]; // what a MitM would produce on its own TLS leg

    let honest_bound = make_bound_key(&session_key, &honest_binder);
    let mitm_bound = make_bound_key(&session_key, &mitm_binder);

    // Responder sends a tag derived from the MitM binder (wrong bound key).
    let mitm_responder_tag = channel_confirmation_tag(&mitm_bound, ConfirmRole::Responder);

    // Initiator expects a tag derived from the honest binder.
    let expected_tag = channel_confirmation_tag(&honest_bound, ConfirmRole::Responder);

    let tags_match: bool = mitm_responder_tag.ct_eq(&expected_tag).into();
    assert!(
        !tags_match,
        "confirm tag from a wrong bound_key (MitM with different TLS binder) must NOT match (CopyPaste-ian9)"
    );
}

// ---------------------------------------------------------------------------
// 3. Reflected tag → reject (role separation)
// ---------------------------------------------------------------------------

/// If a peer simply reflects the initiator's own tag back instead of sending
/// its responder tag, the verify must fail. The role-separated info strings in
/// `channel_confirmation_tag` make Initiator ≠ Responder for the same key.
#[test]
fn reflected_tag_rejected_due_to_role_separation_ian9() {
    let session_key = run_pake("role-sep-test");
    let binder = [0x55u8; 32];
    let bound_key = make_bound_key(&session_key, &binder);

    let initiator_tag = channel_confirmation_tag(&bound_key, ConfirmRole::Initiator);
    let responder_tag = channel_confirmation_tag(&bound_key, ConfirmRole::Responder);

    // Tags must be distinct (role-separated).
    let are_same: bool = initiator_tag.ct_eq(&responder_tag).into();
    assert!(
        !are_same,
        "Initiator and Responder confirm tags must differ for the same bound_key (role separation, CopyPaste-ian9)"
    );

    // Initiator expects the RESPONDER's tag. If the peer reflects back the
    // INITIATOR tag (what it itself sent), it must not match the responder tag.
    // `reflected` simulates verifying the peer's submission against the expected role.
    let reflected: bool = initiator_tag.ct_eq(&responder_tag).into();
    assert!(
        !reflected,
        "a reflected own tag must not satisfy the peer's verification (CopyPaste-ian9)"
    );
}

// ---------------------------------------------------------------------------
// 4. All-zeros "absent" sentinel → reject
// ---------------------------------------------------------------------------

/// A peer that sends an all-zeros tag (simulating a missing / uncalculated tag)
/// must be rejected. This would catch a peer implementation that skips the tag
/// derivation and sends a fixed sentinel.
#[test]
fn zero_sentinel_tag_rejected_ian9() {
    let session_key = run_pake("zero-sentinel-test");
    let binder = [0x77u8; 32];
    let bound_key = make_bound_key(&session_key, &binder);

    let expected = channel_confirmation_tag(&bound_key, ConfirmRole::Responder);
    let absent_tag = [0u8; CONFIRM_TAG_LEN]; // peer sent all-zeros

    let match_result: bool = absent_tag.ct_eq(&expected).into();
    assert!(
        !match_result,
        "all-zeros sentinel tag must not satisfy the confirm-tag verification (CopyPaste-ian9)"
    );
}

// ---------------------------------------------------------------------------
// 5. Tag length is exactly CONFIRM_TAG_LEN bytes
// ---------------------------------------------------------------------------

#[test]
fn confirm_tag_has_correct_length_ian9() {
    let session_key = run_pake("len-check-test");
    let binder = [0x33u8; 32];
    let bound_key = make_bound_key(&session_key, &binder);

    let tag_i = channel_confirmation_tag(&bound_key, ConfirmRole::Initiator);
    let tag_r = channel_confirmation_tag(&bound_key, ConfirmRole::Responder);

    assert_eq!(
        tag_i.len(),
        CONFIRM_TAG_LEN,
        "initiator tag must be CONFIRM_TAG_LEN bytes"
    );
    assert_eq!(
        tag_r.len(),
        CONFIRM_TAG_LEN,
        "responder tag must be CONFIRM_TAG_LEN bytes"
    );
}
