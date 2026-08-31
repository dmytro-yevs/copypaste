//! Product IPC views of transport-owned P2P values.
//!
//! The daemon and the embedded backend keep their own lifecycle and side
//! effects. This module owns only the stable, total conversion into the one IPC
//! contract both expose.

use copypaste_ipc::{
    DeviceDetails, DeviceEndpointObservation, DeviceObservationProvenance, DeviceObservationTrust,
    DevicePresence, DevicePresenceObservation, DeviceProfileObservation, ErrorCode,
    PairingProgressData, PairingRole, PairingState, PeerInfo, SyncResult,
};
use copypaste_p2p::discovery::{DiscoveredPeer, PEER_TTL};
use copypaste_p2p::peers::Peer;
use copypaste_p2p::sync::SyncOutcome;
use copypaste_p2p::{
    AuthenticatedDeviceProfile, DeviceProfile, NodeError, PairingPhase, PairingStatus,
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
) -> PeerInfo {
    let now = crate::now_ms();
    let presence = peer_presence(peer, discovered, now);
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
    now_ms: i64,
) -> DevicePresenceObservation {
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
        let info = peer_info(&peer, Some(&found), None);
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
            let info = peer_info(&peer, found, None);
            let presence = info.details.as_ref().unwrap().presence.as_ref().unwrap();

            assert_eq!(presence.state, DevicePresence::Unknown);
            assert!(!info.online);
            let encoded = serde_json::to_value(info).unwrap();
            assert_eq!(encoded["online"], false);
            assert_eq!(encoded["details"]["presence"]["state"], "unknown");
        }
    }
}
