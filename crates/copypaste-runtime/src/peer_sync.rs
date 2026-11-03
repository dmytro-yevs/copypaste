//! Runtime effects for the shared outgoing peer scheduler.
use crate::Runtime;
use copypaste_p2p::{node::SyncCycle, Peer};
use copypaste_sync::peer_driver::PeerSyncHost;
use std::sync::Arc;

pub(crate) struct RuntimePeerHost(pub(crate) Arc<Runtime>);

impl PeerSyncHost for RuntimePeerHost {
    fn enabled(&self) -> bool {
        self.0.settings.get().sync_enabled
    }
    fn peers(&self) -> Vec<Peer> {
        self.0.p2p.peers().list()
    }
    fn can_dial(&self, peer: &Peer) -> bool {
        !self.0.p2p.node().dial_candidates(peer).is_empty()
    }
    fn reconcile_discovery(&self) {
        self.0.p2p.node().reconcile_discovery();
    }
    async fn sync_peer(&self, peer: Peer, cycle: SyncCycle) -> u64 {
        let result = self.0.sync_one(&peer, &cycle).await;
        if result.received > 0 {
            self.0.note_remote_change();
        }
        u64::from(result.sent) + u64::from(result.received)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::tests::fixture;
    use copypaste_sync::cadence::{MAX_POLL_INTERVAL_WITHOUT_PUSH, MIN_POLL_INTERVAL};
    use std::time::Duration;
    use tokio::sync::watch;

    fn peer_at(runtime: &Runtime, name: &str, addr: &str) {
        let token = copypaste_p2p::PairingToken::generate();
        runtime
            .p2p
            .peers()
            .upsert(Peer {
                pairing_id: token.pairing_id(),
                device_id: None,
                name: name.to_string(),
                psk: token.psk(),
                last_addr: Some(addr.parse().expect("a peer address")),
                last_seen_ms: copypaste_core::now_ms(),
                profile: None,
                profile_observed_at_ms: 0,
            })
            .expect("store a peer");
    }

    /// One pass, with a permit of its own and nothing asking for shutdown.
    async fn one_round(state: &Arc<Runtime>) {
        let permit = state.p2p.try_begin_round().expect("no pass in flight");
        let (_tx, rx) = watch::channel(false);
        state.run_peer_round(permit, rx).await;
    }

    #[tokio::test]
    async fn the_loop_stops_on_shutdown() {
        let (state, _dir) = fixture();
        let (tx, rx) = watch::channel(false);
        let task = tokio::spawn(Arc::clone(&state).run_peer_sync(rx));
        tx.send(true).unwrap();
        tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .expect("the loop must observe shutdown")
            .expect("no panic");
    }

    #[tokio::test]
    async fn a_capture_wake_survives_an_already_running_manual_round() {
        let (state, _dir) = fixture();
        let peer = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        peer_at(&state, "receiver", &peer.local_addr().unwrap().to_string());
        let held = state.p2p.begin_round().await;
        let (tx, rx) = watch::channel(false);
        let task = tokio::spawn(Arc::clone(&state).run_peer_sync(rx));
        state.note_local_change();
        tokio::time::sleep(Duration::from_millis(50)).await;
        drop(held);
        let (stream, _) = tokio::time::timeout(Duration::from_secs(1), peer.accept())
            .await
            .expect("the capture wake must queue rather than wait for a timer")
            .unwrap();
        tx.send(true).unwrap();
        task.await.unwrap();
        drop(stream);
    }

    #[tokio::test]
    async fn a_silent_peer_cannot_delay_other_peers_or_the_next_copy() {
        let (state, _dir) = fixture();
        let first = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let second = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        peer_at(&state, "a-silent", &first.local_addr().unwrap().to_string());
        peer_at(
            &state,
            "b-receiver",
            &second.local_addr().unwrap().to_string(),
        );
        let (tx, rx) = watch::channel(false);
        let task = tokio::spawn(Arc::clone(&state).run_peer_sync(rx));
        state.note_local_change();
        let (first_stream, _) = tokio::time::timeout(Duration::from_secs(1), first.accept())
            .await
            .unwrap()
            .unwrap();
        let (second_stream, _) = tokio::time::timeout(Duration::from_secs(1), second.accept())
            .await
            .expect("another peer must be dialled without waiting for the silent peer")
            .unwrap();
        state.note_local_change();
        let (next_stream, _) = tokio::time::timeout(Duration::from_secs(1), second.accept())
            .await
            .expect("the next copy must not wait for an old silent session")
            .unwrap();
        tx.send(true).unwrap();
        task.await.unwrap();
        drop((first_stream, second_stream, next_stream));
    }

    /// A round with no peers must not touch the cadence: doubling on "there was
    /// nobody to talk to" would leave a freshly paired device waiting out an
    /// interval it drifted to while it had no peers.
    #[tokio::test]
    async fn a_round_with_no_peers_leaves_the_cadence_alone() {
        let (state, _dir) = fixture();
        one_round(&state).await;
        assert_eq!(state.p2p.idle().interval(), MIN_POLL_INTERVAL);
    }

    /// The wake path a local capture uses: the interval snaps back to the floor
    /// and the loop is rung, rather than the capture waiting out a drifted wait.
    #[tokio::test]
    async fn a_local_change_resets_the_cadence_and_rings_the_loop() {
        let (state, _dir) = fixture();
        for _ in 0..8 {
            state.p2p.idle().note_activity(false);
        }
        assert!(state.p2p.idle().interval() > MIN_POLL_INTERVAL);

        state.note_local_change();
        assert_eq!(state.p2p.idle().interval(), MIN_POLL_INTERVAL);
        tokio::time::timeout(Duration::from_secs(1), state.p2p.wake_signal())
            .await
            .expect("the wake must be waiting for the loop");
    }

    /// The ceiling that applies when nothing pushes.
    ///
    /// `Idle::default()` caps at five minutes, and its own doc comment says
    /// that is only defensible because `cloud::realtime` carries the latency in
    /// front of it. Peer sync has no push channel, so an idle pair of laptops
    /// would have waited out five minutes per hop before seeing each other's
    /// clipboard.
    #[test]
    fn the_peer_cadence_stops_at_the_non_push_ceiling() {
        let (state, _dir) = fixture();
        for _ in 0..20 {
            state.p2p.idle().note_activity(false);
        }
        assert_eq!(state.p2p.idle().interval(), MAX_POLL_INTERVAL_WITHOUT_PUSH);
        assert!(MAX_POLL_INTERVAL_WITHOUT_PUSH < Duration::from_secs(300));
    }

    /// A peer that completes the TCP connect and then says nothing. There is no
    /// timeout inside the session, so before the select below this round waited
    /// on it for as long as it stayed quiet — past the app's fifteen-second
    /// quit budget, which then killed the daemon and took the peer flush and
    /// the socket removal with it.
    #[tokio::test]
    async fn a_silent_peer_does_not_hold_shutdown() {
        let (state, _dir) = fixture();
        let silent = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = silent.local_addr().unwrap();
        // Accepted and then never answered, which is what makes it silent
        // rather than merely closed.
        let _accepting = tokio::spawn(async move {
            let mut held = Vec::new();
            while let Ok((stream, _)) = silent.accept().await {
                held.push(stream);
            }
        });
        peer_at(&state, "asleep", &addr.to_string());

        let (tx, rx) = watch::channel(false);
        let permit = state.p2p.try_begin_round().expect("no pass in flight");
        let pass = tokio::spawn({
            let state = Arc::clone(&state);
            async move { state.run_peer_round(permit, rx).await }
        });
        // Long enough for the session to be inside its handshake read.
        tokio::time::sleep(Duration::from_millis(150)).await;
        tx.send(true).unwrap();

        tokio::time::timeout(Duration::from_secs(5), pass)
            .await
            .expect("the round must abandon a silent peer on shutdown")
            .expect("no panic");
    }

    /// The other half of the same rule: cancellation must not become the
    /// ordinary path. A reachable-but-refusing peer still completes the round,
    /// so an unpaired-at-the-far-end device does not look like a shutdown.
    #[tokio::test]
    async fn a_round_nobody_cancels_still_finishes_its_peer_list() {
        let (state, _dir) = fixture();
        // Bound and immediately dropped, so connect is refused rather than left
        // hanging: the round has to come back on its own.
        let closed = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = closed.local_addr().unwrap();
        drop(closed);
        peer_at(&state, "refusing", &addr.to_string());

        let permit = state.p2p.try_begin_round().expect("no pass in flight");
        let (_tx, rx) = watch::channel(false);
        tokio::time::timeout(Duration::from_secs(20), state.run_peer_round(permit, rx))
            .await
            .expect("a refused peer must not wedge the round");
    }

    /// The master switch is read at the top of each round, which is what makes
    /// it live rather than start-up-only.
    #[tokio::test]
    async fn sync_disabled_skips_the_round() {
        let (state, _dir) = fixture();
        state
            .settings
            .apply(&copypaste_ipc::ConfigPatch {
                sync_enabled: Some(false),
                ..Default::default()
            })
            .unwrap();
        // No peers either way; what is asserted is that it returns without
        // consulting the peer list or the cadence.
        one_round(&state).await;
        assert_eq!(state.p2p.idle().interval(), MIN_POLL_INTERVAL);
    }
}
