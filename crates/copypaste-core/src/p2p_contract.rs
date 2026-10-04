//! Product IPC views of transport-owned P2P values.
//!
//! The daemon and the embedded backend keep their own lifecycle and side
//! effects. This module owns only the stable, total conversion into the one IPC
//! contract both expose.

use copypaste_ipc::{
    DeviceDetails, DeviceEndpointObservation, DeviceLatencyObservation,
    DeviceObservationProvenance, DeviceObservationTrust, DevicePresence, DevicePresenceObservation,
    DeviceProfileObservation, ErrorCode, PairingProgressData, PairingRole, PairingState, PeerInfo,
    SyncResult,
};
use copypaste_p2p::discovery::{DiscoveredPeer, PEER_TTL};
use copypaste_p2p::peers::Peer;
use copypaste_p2p::sync::SyncOutcome;
use copypaste_p2p::{
    AuthenticatedDeviceProfile, AuthenticatedReachability, DeviceProfile, NodeError, PairingPhase,
    PairingStatus, ProbeState,
};

const PRESENCE_FRESH_MS: i64 = 15_000;

#[must_use]
pub fn discovered_device(found: DiscoveredPeer, paired: bool) -> copypaste_ipc::DiscoveredDevice {
    let fresh_until_ms = found
        .last_seen_ms
        .saturating_add(i64::try_from(PEER_TTL.as_millis()).unwrap_or(i64::MAX));
    let details = DeviceDetails {
        profile: Some(profile_observation(
            &found.name,
            found.profile.as_ref(),
            DeviceObservationTrust::Unverified,
            found.last_seen_ms,
            Some(fresh_until_ms),
        )),
        endpoint: Some(DeviceEndpointObservation {
            lan_endpoint: found.addr.to_string(),
            provenance: DeviceObservationProvenance::Observed,
            trust: DeviceObservationTrust::Unverified,
            observed_at_ms: found.last_seen_ms,
            fresh_until_ms: Some(fresh_until_ms),
        }),
        presence: Some(DevicePresenceObservation {
            state: discovery_presence_state(&found, crate::now_ms()),
            last_seen_ms: found.last_seen_ms,
            provenance: DeviceObservationProvenance::Observed,
            trust: DeviceObservationTrust::Local,
            observed_at_ms: found.last_seen_ms,
            fresh_until_ms: Some(fresh_until_ms),
        }),
        ..DeviceDetails::default()
    };
    copypaste_ipc::DiscoveredDevice {
        discovery_id: found.discovery_id,
        name: found.name,
        addr: found.addr.to_string(),
        last_seen_ms: found.last_seen_ms,
        paired,
        details: Some(details),
    }
}

#[must_use]
pub fn peer_info(
    peer: &Peer,
    discovered: Option<&DiscoveredPeer>,
    authenticated: Option<&AuthenticatedDeviceProfile>,
    reachability: Option<&AuthenticatedReachability>,
) -> PeerInfo {
    let now = crate::now_ms();
    let presence = peer_presence(peer, discovered, reachability, now);
    let online = presence.is_current_online_at(now);
    let (profile, profile_trust, profile_at, profile_fresh) = match authenticated {
        Some(observed) => (
            Some(&observed.profile),
            DeviceObservationTrust::Authenticated,
            observed.observed_at_ms,
            Some(observed.fresh_until_ms),
        ),
        None => (
            discovered.and_then(|found| found.profile.as_ref()),
            DeviceObservationTrust::Unverified,
            discovered.map_or(peer.last_seen_ms, |found| found.last_seen_ms),
            discovered.map(|found| {
                found
                    .last_seen_ms
                    .saturating_add(i64::try_from(PEER_TTL.as_millis()).unwrap_or(i64::MAX))
            }),
        ),
    };
    let endpoint = discovered
        .map(|found| (found.addr.to_string(), found.last_seen_ms, false))
        .or_else(|| {
            peer.last_addr
                .map(|addr| (addr.to_string(), peer.last_seen_ms, peer.last_seen_ms > 0))
        })
        .map(
            |(lan_endpoint, observed_at_ms, authenticated)| DeviceEndpointObservation {
                lan_endpoint,
                provenance: DeviceObservationProvenance::Observed,
                trust: if authenticated {
                    DeviceObservationTrust::Authenticated
                } else {
                    DeviceObservationTrust::Unverified
                },
                observed_at_ms,
                fresh_until_ms: discovered.map(|found| {
                    found
                        .last_seen_ms
                        .saturating_add(i64::try_from(PEER_TTL.as_millis()).unwrap_or(i64::MAX))
                }),
            },
        );
    PeerInfo {
        pairing_id: peer.pairing_id.clone(),
        name: peer.name.clone(),
        last_addr: peer.last_addr.map(|addr| addr.to_string()),
        last_seen_ms: peer.last_seen_ms,
        online,
        details: Some(DeviceDetails {
            profile: Some(profile_observation(
                &peer.name,
                profile,
                profile_trust,
                profile_at,
                profile_fresh,
            )),
            endpoint,
            latency: reachability.and_then(|observation| {
                (observation.state_at(now) == ProbeState::Online)
                    .then_some(observation.latency_ms)
                    .flatten()
                    .map(|round_trip_latency_ms| DeviceLatencyObservation {
                        round_trip_latency_ms,
                        provenance: DeviceObservationProvenance::Measured,
                        trust: DeviceObservationTrust::Authenticated,
                        observed_at_ms: observation.observed_at_ms,
                        fresh_until_ms: observation.fresh_until_ms,
                    })
            }),
            presence: Some(presence),
            ..DeviceDetails::default()
        }),
    }
}

#[must_use]
pub fn local_device_details(display_name: &str, endpoint: Option<&str>) -> DeviceDetails {
    let now = crate::now_ms();
    DeviceDetails {
        profile: Some(profile_observation(
            display_name,
            Some(&DeviceProfile::current()),
            DeviceObservationTrust::Local,
            now,
            None,
        )),
        endpoint: endpoint.map(|lan_endpoint| DeviceEndpointObservation {
            lan_endpoint: lan_endpoint.to_string(),
            provenance: DeviceObservationProvenance::Observed,
            trust: DeviceObservationTrust::Local,
            observed_at_ms: now,
            fresh_until_ms: Some(now.saturating_add(PRESENCE_FRESH_MS)),
        }),
        presence: Some(DevicePresenceObservation {
            state: DevicePresence::Online,
            last_seen_ms: now,
            provenance: DeviceObservationProvenance::Observed,
            trust: DeviceObservationTrust::Local,
            observed_at_ms: now,
            fresh_until_ms: Some(now.saturating_add(PRESENCE_FRESH_MS)),
        }),
        ..DeviceDetails::default()
    }
}

fn profile_observation(
    display_name: &str,
    profile: Option<&DeviceProfile>,
    trust: DeviceObservationTrust,
    observed_at_ms: i64,
    fresh_until_ms: Option<i64>,
) -> DeviceProfileObservation {
    let profile = profile.cloned().unwrap_or_default();
    DeviceProfileObservation {
        display_name: display_name.to_string(),
        app_version: profile.app_version,
        protocol_version: profile.protocol_version,
        platform: profile.platform,
        device_class: profile.device_class,
        os_name: profile.os_name,
        os_version: profile.os_version,
        model: profile.model,
        provenance: DeviceObservationProvenance::SelfReported,
        trust,
        observed_at_ms,
        fresh_until_ms,
    }
}

fn discovery_presence_state(found: &DiscoveredPeer, now_ms: i64) -> DevicePresence {
    let ttl_ms = i64::try_from(PEER_TTL.as_millis()).unwrap_or(i64::MAX);
    if now_ms.saturating_sub(found.last_seen_ms) < ttl_ms {
        DevicePresence::Online
    } else {
        DevicePresence::Unknown
    }
}

fn peer_presence(
    peer: &Peer,
    discovered: Option<&DiscoveredPeer>,
    reachability: Option<&AuthenticatedReachability>,
    now_ms: i64,
) -> DevicePresenceObservation {
    if let Some(observation) =
        reachability.filter(|observation| observation.state_at(now_ms) != ProbeState::Unknown)
    {
        return DevicePresenceObservation {
            state: match observation.state_at(now_ms) {
                ProbeState::Online => DevicePresence::Online,
                ProbeState::Offline => DevicePresence::Offline,
                ProbeState::Unknown => DevicePresence::Unknown,
            },
            last_seen_ms: observation.observed_at_ms,
            provenance: DeviceObservationProvenance::Measured,
            trust: DeviceObservationTrust::Authenticated,
            observed_at_ms: observation.observed_at_ms,
            fresh_until_ms: observation.fresh_until_ms,
        };
    }
    match discovered {
        Some(found) if discovery_presence_state(found, now_ms) == DevicePresence::Online => {
            DevicePresenceObservation {
                state: DevicePresence::Online,
                last_seen_ms: found.last_seen_ms,
                provenance: DeviceObservationProvenance::Observed,
                trust: DeviceObservationTrust::Local,
                observed_at_ms: found.last_seen_ms,
                fresh_until_ms: Some(
                    found
                        .last_seen_ms
                        .saturating_add(i64::try_from(PEER_TTL.as_millis()).unwrap_or(i64::MAX)),
                ),
            }
        }
        _ => DevicePresenceObservation {
            state: DevicePresence::Unknown,
            last_seen_ms: peer.last_seen_ms,
            provenance: DeviceObservationProvenance::Observed,
            trust: DeviceObservationTrust::Local,
            observed_at_ms: peer.last_seen_ms,
            fresh_until_ms: None,
        },
    }
}

#[must_use]
pub fn pairing_progress(
    status: PairingStatus,
    known_device: Option<PeerInfo>,
) -> PairingProgressData {
    let peer = status.peer.as_ref();
    PairingProgressData {
        pairing_id: status.pairing_id,
        role: status.role.map(|role| match role {
            copypaste_p2p::PairingRole::Initiator => PairingRole::Initiator,
            copypaste_p2p::PairingRole::Responder => PairingRole::Responder,
        }),
        state: match status.phase {
            PairingPhase::Idle => PairingState::Idle,
            PairingPhase::WaitingForPeer => PairingState::WaitingForPeer,
            PairingPhase::Handshaking => PairingState::Handshaking,
            PairingPhase::AwaitingConfirmation => PairingState::AwaitingConfirmation,
            PairingPhase::Confirmed => PairingState::Confirmed,
            PairingPhase::Rejected => PairingState::Rejected,
            PairingPhase::Cancelled => PairingState::Cancelled,
            PairingPhase::TimedOut => PairingState::TimedOut,
            PairingPhase::Failed => PairingState::Failed,
        },
        expires_in_ms: status.expires_in_ms,
        sas: status.sas,
        peer_device_id: peer.map(|peer| peer.device_id.clone()),
        peer_name: peer.map(|peer| peer.name.clone()),
        peer_addr: peer.and_then(|peer| peer.addr.map(|addr| addr.to_string())),
        known_device,
        error_code: status.error.as_ref().map(node_error_code),
    }
}

#[must_use]
pub fn sync_result(
    peer: &Peer,
    result: Result<SyncOutcome, NodeError>,
    duration: std::time::Duration,
) -> SyncResult {
    let duration_ms = u64::try_from(duration.as_millis()).unwrap_or(u64::MAX);
    match result {
        Ok(outcome) => SyncResult {
            pairing_id: peer.pairing_id.clone(),
            name: outcome.peer_device_name,
            sent: u32::try_from(outcome.stats.sent).unwrap_or(u32::MAX),
            received: u32::try_from(outcome.stats.received).unwrap_or(u32::MAX),
            skipped_too_large: Some(
                u32::try_from(outcome.stats.skipped_too_large).unwrap_or(u32::MAX),
            ),
            duration_ms: Some(duration_ms),
            error: None,
            error_code: None,
        },
        Err(error) => SyncResult {
            pairing_id: peer.pairing_id.clone(),
            name: peer.name.clone(),
            sent: 0,
            received: 0,
            skipped_too_large: None,
            duration_ms: Some(duration_ms),
            error: Some(error.to_string()),
            error_code: Some(node_error_code(&error)),
        },
    }
}

#[must_use]
/// Maps transport failures to the remedy-oriented IPC taxonomy.
///
/// Do not use `NodeError::is_client_error`: it collapses a bad code, an
/// unreachable peer and a full pairing list even though clients render
/// different remedies (post-merge review, finding 4).
pub fn node_error_code(error: &NodeError) -> ErrorCode {
    match error {
        NodeError::BadCode | NodeError::Handshake | NodeError::SelfPairing => {
            ErrorCode::PairingCode
        }
        NodeError::PairingBusy => ErrorCode::RateLimited,
        NodeError::NoPairing => ErrorCode::NotReady,
        NodeError::BadAddress => ErrorCode::PairingAddress,
        NodeError::NoAddress | NodeError::Timeout => ErrorCode::PeerUnreachable,
        NodeError::TooManyPairings => ErrorCode::PairingLimit,
        NodeError::Session | NodeError::PeerStore => ErrorCode::PeerFailed,
        NodeError::PeerVersion => ErrorCode::PeerVersion,
        NodeError::NoPeer => ErrorCode::PeerNotFound,
    }
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    use super::*;
    use copypaste_p2p::discovery::PEER_TTL;
    use copypaste_p2p::sync::{SyncCursor, SyncStats};
    use copypaste_p2p::transport::TOKEN_LEN;

    fn sync_peer() -> Peer {
        Peer {
            pairing_id: "peer-1".into(),
            name: "Phone".into(),
            psk: [1; TOKEN_LEN],
            last_addr: None,
            last_seen_ms: 0,
            profile: None,
            profile_observed_at_ms: 0,
        }
    }

    fn outcome(skipped_too_large: usize) -> SyncOutcome {
        SyncOutcome {
            stats: SyncStats {
                sent: 1,
                received: 2,
                skipped: 3,
                skipped_too_large,
            },
            peer_device_id: "device-1".into(),
            peer_device_name: "Phone".into(),
            peer_profile: None,
            peer_listen_addr: None,
            cursor: SyncCursor::default(),
            applied_floor: None,
        }
    }

    #[test]
    fn successful_sync_projects_a_saturated_size_refusal_count() {
        let result = sync_result(
            &sync_peer(),
            Ok(outcome((u32::MAX as usize).saturating_add(1))),
            std::time::Duration::ZERO,
        );
        assert_eq!(result.skipped_too_large, Some(u32::MAX));
    }

    #[test]
    fn failed_sync_keeps_size_refusal_count_unknown() {
        let result = sync_result(
            &sync_peer(),
            Err(NodeError::Timeout),
            std::time::Duration::ZERO,
        );
        assert_eq!(result.skipped_too_large, None);
    }

    fn peer() -> Peer {
        Peer {
            pairing_id: "peer-1".to_string(),
            name: "Phone".to_string(),
            psk: [1; TOKEN_LEN],
            last_addr: Some(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 47_654)),
            last_seen_ms: 10,
            profile: None,
            profile_observed_at_ms: 0,
        }
    }

    fn discovered(last_seen_ms: i64) -> DiscoveredPeer {
        DiscoveredPeer {
            discovery_id: "discovery-1".to_string(),
            pairing_ids: vec!["peer-1".to_string()],
            name: "Phone".to_string(),
            profile: None,
            addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 47_654),
            last_seen_ms,
        }
    }

    #[test]
    fn peer_online_is_derived_from_a_current_positive_observation() {
        let now = crate::now_ms();
        let peer = peer();
        let found = discovered(now);
        let info = peer_info(&peer, Some(&found), None, None);
        let presence = info.details.as_ref().unwrap().presence.as_ref().unwrap();

        assert_eq!(presence.state, DevicePresence::Online);
        assert!(info.online);
        assert!(presence.is_current_online_at(crate::now_ms()));
        let encoded = serde_json::to_value(info).unwrap();
        assert_eq!(encoded["online"], true);
        assert_eq!(encoded["details"]["presence"]["state"], "online");
    }

    #[test]
    fn absent_or_stale_discovery_fails_closed_to_unknown() {
        let now = crate::now_ms();
        let peer = peer();
        let stale =
            discovered(now.saturating_sub(i64::try_from(PEER_TTL.as_millis()).unwrap_or(i64::MAX)));

        for found in [None, Some(&stale)] {
            let info = peer_info(&peer, found, None, None);
            let presence = info.details.as_ref().unwrap().presence.as_ref().unwrap();

            assert_eq!(presence.state, DevicePresence::Unknown);
            assert!(!info.online);
            let encoded = serde_json::to_value(info).unwrap();
            assert_eq!(encoded["online"], false);
            assert_eq!(encoded["details"]["presence"]["state"], "unknown");
        }
    }

    #[test]
    fn stale_authenticated_profile_remains_identifiable_without_claiming_reachability() {
        let peer = peer();
        let authenticated = AuthenticatedDeviceProfile {
            profile: DeviceProfile {
                model: Some("Pixel 9".to_string()),
                ..DeviceProfile::default()
            },
            observed_at_ms: 1,
            fresh_until_ms: 2,
        };
        let info = peer_info(&peer, None, Some(&authenticated), None);
        let details = info.details.expect("device details");
        let profile = details.profile.expect("authenticated profile");
        let presence = details.presence.expect("presence");

        assert_eq!(profile.model.as_deref(), Some("Pixel 9"));
        assert_eq!(profile.trust, DeviceObservationTrust::Authenticated);
        assert_eq!(profile.observed_at_ms, 1);
        assert_eq!(profile.fresh_until_ms, Some(2));
        assert_eq!(presence.state, DevicePresence::Unknown);
        assert!(!info.online);
    }

    #[test]
    fn authenticated_probe_projects_rtt_and_overrides_discovery() {
        let now = crate::now_ms();
        let reachability = AuthenticatedReachability::online(Some(24), now);
        let info = peer_info(&peer(), None, None, Some(&reachability));
        let details = info.details.unwrap();
        let presence = details.presence.unwrap();
        let latency = details.latency.unwrap();

        assert_eq!(presence.state, DevicePresence::Online);
        assert_eq!(presence.provenance, DeviceObservationProvenance::Measured);
        assert_eq!(presence.trust, DeviceObservationTrust::Authenticated);
        assert_eq!(latency.round_trip_latency_ms, 24);
        assert_eq!(latency.provenance, DeviceObservationProvenance::Measured);
        assert_eq!(latency.trust, DeviceObservationTrust::Authenticated);
    }

    #[test]
    fn failed_probe_is_offline_but_expired_probe_falls_back_to_current_discovery() {
        let now = crate::now_ms();
        let offline = AuthenticatedReachability::offline(now);
        let offline_info = peer_info(&peer(), None, None, Some(&offline));
        assert_eq!(
            offline_info.details.unwrap().presence.unwrap().state,
            DevicePresence::Offline
        );

        let expired = AuthenticatedReachability::online(Some(24), 0);
        let found = discovered(now);
        let info = peer_info(&peer(), Some(&found), None, Some(&expired));
        let details = info.details.unwrap();
        let presence = details.presence.unwrap();
        assert_eq!(presence.state, DevicePresence::Online);
        assert_eq!(presence.trust, DeviceObservationTrust::Local);
        assert!(details.latency.is_none());
    }
}
