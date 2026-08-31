use copypaste_ipc::{
    DeviceDetails, DeviceObservationProvenance, DeviceObservationTrust, DevicePresence,
    DevicePresenceObservation, PeerInfo,
};

pub fn populated_peer() -> PeerInfo {
    PeerInfo {
        pairing_id: "peer-1".into(),
        name: "Phone".into(),
        last_addr: Some("192.0.2.1:47654".into()),
        last_seen_ms: 1,
        online: false,
        details: Some(DeviceDetails {
            presence: Some(DevicePresenceObservation {
                state: DevicePresence::Online,
                last_seen_ms: 1,
                provenance: DeviceObservationProvenance::Observed,
                trust: DeviceObservationTrust::Local,
                observed_at_ms: 0,
                fresh_until_ms: Some(i64::MAX),
            }),
            ..DeviceDetails::default()
        }),
    }
}
