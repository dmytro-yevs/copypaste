use copypaste_p2p::discovery::PeerInfo;

use crate::pairing::*;

fn confirmed_sample() -> ConfirmedPairing {
    ConfirmedPairing {
        peer_fingerprint: "abc123".to_string(),
        peer_sync_addr: "10.0.0.2:51515".to_string(),
        session_key: vec![0x42u8; 32],
        peer_provisioning: None,
        // HB-1b (ABI 14): sample peer metadata for the round-trip assertions.
        peer_model: Some("MacBook Air".to_string()),
        peer_os: Some("macOS 15.5".to_string()),
        peer_app_version: Some("0.6.1".to_string()),
        peer_local_ip: Some("10.0.0.2".to_string()),
        peer_public_ip: Some("203.0.113.7".to_string()),
        peer_device_id: Some("device-uuid-abc123".to_string()),
        peer_supabase_account_id: Some("supabase-acct-xyz789".to_string()),
    }
}

#[test]
fn fresh_coordinator_is_idle() {
    let c = PairingCoordinator::new();
    assert!(c.snapshot().is_idle());
    assert_eq!(c.snapshot().as_str(), "idle");
}

#[test]
fn begin_transitions_idle_to_initiating() {
    let c = PairingCoordinator::new();
    assert!(c.try_begin(PairingRole::Initiator));
    let s = c.snapshot();
    assert_eq!(s.as_str(), "initiating");
    assert_eq!(s.role(), Some(PairingRole::Initiator));
    assert!(s.is_active());
}

#[test]
fn concurrent_begin_is_rejected_single_active() {
    let c = PairingCoordinator::new();
    assert!(c.try_begin(PairingRole::Initiator));
    // A second begin while non-idle must be refused (single active pairing).
    assert!(!c.try_begin(PairingRole::Responder));
    assert_eq!(c.snapshot().role(), Some(PairingRole::Initiator));
}

#[test]
fn enter_awaiting_sas_exposes_sas_and_role() {
    let c = PairingCoordinator::new();
    assert!(c.try_begin(PairingRole::Responder));
    let _rx = c.enter_awaiting_sas("123456".to_string(), PairingRole::Responder);
    let s = c.snapshot();
    assert_eq!(s.as_str(), "awaiting_sas");
    assert_eq!(s.sas(), Some("123456"));
    assert_eq!(s.role(), Some(PairingRole::Responder));
}

#[tokio::test]
async fn deliver_decision_accept_fires_oneshot_true() {
    let c = PairingCoordinator::new();
    assert!(c.try_begin(PairingRole::Initiator));
    let rx = c.enter_awaiting_sas("000000".to_string(), PairingRole::Initiator);
    assert!(c.deliver_decision(true));
    assert!(rx.await.unwrap());
}

#[tokio::test]
async fn reject_delivers_false_then_finish_rejected() {
    // A reject must propagate `false` to the handshake so it sends REJECT in
    // frame 10a and drops/zeroizes the session key (no persist, no rotate).
    let c = PairingCoordinator::new();
    assert!(c.try_begin(PairingRole::Initiator));
    let rx = c.enter_awaiting_sas("424242".to_string(), PairingRole::Initiator);
    assert!(c.deliver_decision(false));
    assert!(!rx.await.unwrap());
    c.finish(PairingState::Rejected);
    assert_eq!(c.snapshot().as_str(), "rejected");
    assert!(c.snapshot().is_terminal());
    // A rejected pairing exposes NO key material.
    assert!(c.snapshot().confirmed().is_none());
}

#[tokio::test]
async fn abort_drops_confirm_channel_so_handshake_sees_rejection() {
    // pair_abort must cancel the in-flight handshake: dropping the sender
    // resolves the await with an Err, which the callback treats as reject.
    let c = PairingCoordinator::new();
    assert!(c.try_begin(PairingRole::Responder));
    let rx = c.enter_awaiting_sas("999999".to_string(), PairingRole::Responder);
    c.abort();
    assert!(rx.await.is_err(), "dropping the sender must error the recv");
    assert_eq!(c.snapshot().as_str(), "aborted");
    assert!(c.snapshot().confirmed().is_none());
}

#[tokio::test]
async fn timeout_path_drops_keys() {
    // Simulate the SAS_CONFIRM_TIMEOUT branch: the handshake's confirm
    // closure times out waiting on the oneshot, reports TimedOut, and the
    // key never reaches a Confirmed state.
    let c = PairingCoordinator::new();
    assert!(c.try_begin(PairingRole::Initiator));
    let rx = c.enter_awaiting_sas("555555".to_string(), PairingRole::Initiator);
    // No deliver_decision; emulate the timeout firing.
    let timed_out = tokio::time::timeout(std::time::Duration::from_millis(20), rx).await;
    assert!(timed_out.is_err(), "no decision delivered → recv times out");
    c.finish(PairingState::TimedOut);
    assert_eq!(c.snapshot().as_str(), "timed_out");
    assert!(c.snapshot().confirmed().is_none());
}

#[test]
fn deliver_decision_without_pending_is_false() {
    let c = PairingCoordinator::new();
    assert!(!c.deliver_decision(true));
}

#[test]
fn confirmed_carries_ffi_outputs_for_persistence() {
    let c = PairingCoordinator::new();
    assert!(c.try_begin(PairingRole::Initiator));
    c.finish(PairingState::Confirmed(confirmed_sample()));
    let s = c.snapshot();
    assert_eq!(s.as_str(), "confirmed");
    let out = s.confirmed().expect("confirmed carries outputs");
    assert_eq!(out.peer_fingerprint, "abc123");
    assert_eq!(out.peer_sync_addr, "10.0.0.2:51515");
    assert_eq!(out.session_key.len(), 32);

    // HB-1b (ABI 14): the peer metadata round-trips into ConfirmedPairing.
    assert_eq!(out.peer_model.as_deref(), Some("MacBook Air"));
    assert_eq!(out.peer_os.as_deref(), Some("macOS 15.5"));
    assert_eq!(out.peer_app_version.as_deref(), Some("0.6.1"));
    assert_eq!(out.peer_local_ip.as_deref(), Some("10.0.0.2"));
    assert_eq!(out.peer_public_ip.as_deref(), Some("203.0.113.7"));
    // ABI 19 (CopyPaste-gldr): the peer's Supabase account id round-trips too.
    assert_eq!(
        out.peer_supabase_account_id.as_deref(),
        Some("supabase-acct-xyz789")
    );

    // PairStatus surfaces the peer_* only on confirmed.
    let status = PairStatus::from_state(&s);
    assert_eq!(status.state, "confirmed");
    assert_eq!(status.peer_fingerprint.as_deref(), Some("abc123"));
    assert!(status.session_key.is_some());
    // HB-1b: PairStatus carries the peer metadata through for Kotlin.
    assert_eq!(status.peer_model.as_deref(), Some("MacBook Air"));
    assert_eq!(status.peer_os.as_deref(), Some("macOS 15.5"));
    assert_eq!(status.peer_app_version.as_deref(), Some("0.6.1"));
    assert_eq!(status.peer_local_ip.as_deref(), Some("10.0.0.2"));
    assert_eq!(status.peer_public_ip.as_deref(), Some("203.0.113.7"));
    assert_eq!(
        status.peer_supabase_account_id.as_deref(),
        Some("supabase-acct-xyz789")
    );
}

#[test]
fn pair_status_hides_peer_fields_while_awaiting() {
    let c = PairingCoordinator::new();
    assert!(c.try_begin(PairingRole::Responder));
    let _rx = c.enter_awaiting_sas("121212".to_string(), PairingRole::Responder);
    let status = PairStatus::from_state(&c.snapshot());
    assert_eq!(status.state, "awaiting_sas");
    assert_eq!(status.sas.as_deref(), Some("121212"));
    assert_eq!(status.role.as_deref(), Some("responder"));
    assert!(
        status.peer_fingerprint.is_none(),
        "peer_* only on confirmed"
    );
    assert!(status.session_key.is_none(), "no key before confirmed");
    // HB-1b: peer metadata is likewise withheld until confirmed.
    assert!(
        status.peer_model.is_none(),
        "peer metadata only on confirmed"
    );
    assert!(
        status.peer_local_ip.is_none(),
        "peer metadata only on confirmed"
    );
}

#[test]
fn reset_returns_to_idle_for_next_pairing() {
    let c = PairingCoordinator::new();
    assert!(c.try_begin(PairingRole::Initiator));
    c.finish(PairingState::Confirmed(confirmed_sample()));
    assert_eq!(c.snapshot().as_str(), "confirmed");
    c.reset();
    assert!(c.snapshot().is_idle());
    // A fresh pairing may begin after reset.
    assert!(c.try_begin(PairingRole::Responder));
}

/// After finish(terminal), try_begin must succeed WITHOUT requiring pair_reset().
/// This is the LAN retry regression: stale terminal state must not block new pairings.
#[test]
fn try_begin_succeeds_after_terminal_without_reset() {
    let c = PairingCoordinator::new();

    // Confirmed terminal → next try_begin must succeed (auto-reset).
    assert!(c.try_begin(PairingRole::Initiator));
    c.finish(PairingState::Confirmed(confirmed_sample()));
    assert_eq!(c.snapshot().as_str(), "confirmed");
    assert!(
        c.try_begin(PairingRole::Responder),
        "try_begin must succeed after Confirmed without explicit reset"
    );
    assert_eq!(c.snapshot().as_str(), "initiating");

    // Rejected terminal → next try_begin must also succeed.
    c.finish(PairingState::Rejected);
    assert!(
        c.try_begin(PairingRole::Initiator),
        "try_begin must succeed after Rejected without explicit reset"
    );

    // TimedOut terminal → same.
    c.finish(PairingState::TimedOut);
    assert!(
        c.try_begin(PairingRole::Initiator),
        "try_begin must succeed after TimedOut without explicit reset"
    );
}

/// After abort(), try_begin must succeed (abort() leaves state as Aborted, a terminal).
#[test]
fn try_begin_succeeds_after_abort() {
    let c = PairingCoordinator::new();
    assert!(c.try_begin(PairingRole::Initiator));
    let _rx = c.enter_awaiting_sas("123456".to_string(), PairingRole::Initiator);
    c.abort();
    assert_eq!(c.snapshot().as_str(), "aborted");
    assert!(
        c.try_begin(PairingRole::Responder),
        "try_begin must succeed after abort() without explicit reset"
    );
    assert_eq!(c.snapshot().as_str(), "initiating");
}

/// While genuinely active (Initiating or AwaitingSas), try_begin must still return false.
#[test]
fn try_begin_refused_while_active() {
    let c = PairingCoordinator::new();

    // Refused while Initiating.
    assert!(c.try_begin(PairingRole::Initiator));
    assert_eq!(c.snapshot().as_str(), "initiating");
    assert!(
        !c.try_begin(PairingRole::Responder),
        "try_begin must be refused while Initiating"
    );
    assert_eq!(
        c.snapshot().role(),
        Some(PairingRole::Initiator),
        "state must be unchanged"
    );

    // Refused while AwaitingSas.
    let _rx = c.enter_awaiting_sas("999999".to_string(), PairingRole::Initiator);
    assert_eq!(c.snapshot().as_str(), "awaiting_sas");
    assert!(
        !c.try_begin(PairingRole::Responder),
        "try_begin must be refused while AwaitingSas"
    );
    assert_eq!(
        c.snapshot().as_str(),
        "awaiting_sas",
        "state must be unchanged"
    );
}

#[test]
fn role_wire_strings() {
    assert_eq!(PairingRole::Initiator.as_str(), "initiator");
    assert_eq!(PairingRole::Responder.as_str(), "responder");
}

#[test]
fn ipv4_first_prefers_v4() {
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
    let peer = PeerInfo {
        device_id: "d".into(),
        device_name: "n".into(),
        ip_addrs: vec![
            IpAddr::V6(Ipv6Addr::LOCALHOST),
            IpAddr::V4(Ipv4Addr::new(192, 168, 1, 5)),
        ],
        port: 51515,
        bport: Some(60000),
    };
    let addr = ipv4_first_addr(&peer).expect("addr");
    assert!(addr.ip().is_ipv4(), "IPv4 must be preferred");
    assert_eq!(addr.port(), 60000, "bport dialed when present");
}

#[test]
fn ipv4_first_falls_back_to_port_without_bport() {
    use std::net::{IpAddr, Ipv4Addr};
    let peer = PeerInfo {
        device_id: "d".into(),
        device_name: "n".into(),
        ip_addrs: vec![IpAddr::V4(Ipv4Addr::new(10, 0, 0, 9))],
        port: 51515,
        bport: None,
    };
    let addr = ipv4_first_addr(&peer).expect("addr");
    assert_eq!(addr.port(), 51515, "falls back to sync port without bport");
}

#[test]
fn ipv4_first_none_for_no_addrs() {
    let peer = PeerInfo {
        device_id: "d".into(),
        device_name: "n".into(),
        ip_addrs: vec![],
        port: 51515,
        bport: Some(60000),
    };
    assert!(ipv4_first_addr(&peer).is_none());
}

#[test]
fn discovered_peer_from_peer_info_maps_fields() {
    use std::net::{IpAddr, Ipv4Addr};
    let peer = PeerInfo {
        device_id: "fp123".into(),
        device_name: "Alice's Mac".into(),
        ip_addrs: vec![IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2))],
        port: 51515,
        bport: Some(60000),
    };
    let dp = DiscoveredPeer::from_peer_info(peer, true);
    assert_eq!(dp.device_id, "fp123");
    assert_eq!(dp.ip_addrs, vec!["10.0.0.2".to_string()]);
    assert_eq!(dp.port, 51515);
    assert_eq!(dp.bport, Some(60000));
    assert!(dp.paired);
}

#[test]
fn outcome_mapping() {
    assert!(matches!(
        outcome_for_initiator_error(true),
        PairingState::Rejected
    ));
    assert!(matches!(
        outcome_for_initiator_error(false),
        PairingState::Aborted
    ));
}

/// The SAS this crate surfaces is `copypaste_p2p::pake::derive_sas` — the
/// SAME function the macOS daemon uses. Re-verify its load-bearing
/// properties here (mirroring the p2p `derive_sas_*` tests) so the Android
/// FFI's authentication contract is pinned: deterministic, 6 decimal digits,
/// and domain-separated (a different `bound_key` → a (near-certainly)
/// different SAS, which is what a MitM-per-leg attack trips on).
#[test]
fn sas_is_deterministic_six_digits_and_domain_separated() {
    use copypaste_p2p::pake::derive_sas;

    let key_a = [0x11u8; 32];
    let key_b = [0x22u8; 32];

    let sas_a1 = derive_sas(&key_a);
    let sas_a2 = derive_sas(&key_a);
    let sas_b = derive_sas(&key_b);

    // Deterministic on the same bound_key (both honest endpoints agree).
    assert_eq!(sas_a1, sas_a2, "SAS must be deterministic per bound_key");
    // Exactly 6 decimal digits.
    assert_eq!(sas_a1.len(), 6, "SAS must be 6 chars");
    assert!(
        sas_a1.chars().all(|c| c.is_ascii_digit()),
        "SAS must be all decimal digits"
    );
    // Domain separation: a different bound_key (the MitM-per-leg case)
    // yields a different SAS so the humans see a mismatch and abort.
    assert_ne!(
        sas_a1, sas_b,
        "different bound_keys must derive different SAS"
    );
}

/// The fixed discovery PAKE password is non-empty and stable (both ends must
/// agree on it for opaque-ke to converge — see its docs).
#[test]
fn discovery_password_is_stable_nonempty() {
    assert!(!DISCOVERY_PAIRING_PASSWORD.is_empty());
    assert_eq!(
        DISCOVERY_PAIRING_PASSWORD,
        "copypaste/p2p/lan-sas-discovery/v1"
    );
}
